//! Bilibili web API adapter. All calls belong to provider/UI workers, never hooks.
//! Only complete, bounded H.264 files enter the existing Media Foundation player.
pub mod auth;
use crate::{Error, Result};
use reqwest::{Url, blocking::Client, redirect::Policy};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Video,
    Recommendations,
    Playlist,
    Favourites,
}
impl Mode {
    pub fn key(self) -> &'static str {
        match self {
            Self::Video => "video",
            Self::Recommendations => "recommendations",
            Self::Playlist => "playlist",
            Self::Favourites => "favourites",
        }
    }
}
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Selection {
    pub mode: Mode,
    pub video: String,
    pub playlist: String,
    pub favourites: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub selection: Selection,
    pub cache_directory: PathBuf,
    pub quality: u32,
    pub max_download_mb: u64,
    pub cache_mb: u64,
    pub max_duration_seconds: u64,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            selection: Selection::default(),
            cache_directory: "iidxbg/bilibili-cache".into(),
            quality: 64,
            max_download_mb: 128,
            cache_mb: 2048,
            max_duration_seconds: 600,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<()> {
        if self.cache_directory.as_os_str().is_empty()
            || ![16, 32, 64, 80].contains(&self.quality)
            || !(1..=1024).contains(&self.max_download_mb)
            || !(128..=16384).contains(&self.cache_mb)
            || self.cache_mb < self.max_download_mb
            || !(1..=3600).contains(&self.max_duration_seconds)
            || [
                &self.selection.video,
                &self.selection.playlist,
                &self.selection.favourites,
            ]
            .iter()
            .any(|s| s.len() > 2048)
        {
            return Err(Error::Config("invalid Bilibili settings or cache limits"));
        }
        Ok(())
    }
}
fn err(message: impl Into<String>) -> Error {
    Error::Bilibili(message.into())
}
fn network(_: reqwest::Error) -> Error {
    err("网络请求失败，请稍后重试（地址和登录凭据已隐藏）")
}

pub struct Api {
    client: Client,
    session: auth::Session,
}
impl Api {
    pub fn new(session: auth::Session) -> Result<Self> {
        let client = Client::builder()
            .https_only(true)
            .redirect(Policy::none())
            .timeout(Duration::from_secs(20))
            .connect_timeout(Duration::from_secs(5))
            .user_agent("Mozilla/5.0 iidxbg/0.3")
            .build()
            .map_err(network)?;
        Ok(Self { client, session })
    }
    pub fn saved() -> Result<Self> {
        Self::new(auth::load()?)
    }
    fn response(
        &self,
        host: &str,
        path: &str,
        params: &[(&str, String)],
    ) -> Result<reqwest::blocking::Response> {
        assert!(matches!(host, "api.bilibili.com" | "passport.bilibili.com"));
        let mut request = self
            .client
            .get(format!("https://{host}{path}"))
            .query(params)
            .header("Referer", "https://www.bilibili.com/");
        let cookie = self.session.cookie();
        if !cookie.is_empty() {
            request = request.header("Cookie", cookie);
        }
        request
            .send()
            .map_err(network)?
            .error_for_status()
            .map_err(network)
    }
    fn data(response: reqwest::blocking::Response) -> Result<Value> {
        let bytes = crate::pixels::read_bounded(response, 2 * 1024 * 1024)?;
        let value: Value = serde_json::from_slice(&bytes)?;
        match value.get("code").and_then(Value::as_i64) {
            Some(0) => Ok(value["data"].clone()),
            Some(-101) => Err(err("登录已失效，请在 Bilibili 页面重新扫码登录")),
            Some(code) => Err(err(format!("接口返回 {code}；请检查访问权限或稍后重试"))),
            None => Err(err("接口响应格式已改变")),
        }
    }
    fn get(&self, path: &str, params: &[(&str, String)]) -> Result<Value> {
        Self::data(self.response("api.bilibili.com", path, params)?)
    }
    pub fn account(&self) -> Result<Account> {
        if !self.session.authenticated() {
            return Err(err("尚未登录，请扫码登录"));
        }
        let data = self.get("/x/web-interface/nav", &[])?;
        if data["isLogin"] != true {
            return Err(err("尚未登录"));
        }
        Ok(Account {
            name: data["uname"].as_str().unwrap_or("Bilibili").into(),
            mid: data["mid"].as_u64().ok_or_else(|| err("缺少用户 ID"))?,
        })
    }
    fn signed(&self, path: &str, params: &[(&str, String)]) -> Result<Value> {
        // Anonymous nav returns -101 but still supplies the public WBI keys.
        let response = self.response("api.bilibili.com", "/x/web-interface/nav", &[])?;
        let bytes = crate::pixels::read_bounded(response, 2 * 1024 * 1024)?;
        let envelope: Value = serde_json::from_slice(&bytes)?;
        if !matches!(envelope["code"].as_i64(), Some(0 | -101)) {
            return Err(err("无法取得请求签名参数，请稍后重试"));
        }
        let nav = &envelope["data"];
        let keys = [&nav["wbi_img"]["img_url"], &nav["wbi_img"]["sub_url"]]
            .into_iter()
            .map(|v| {
                v.as_str()
                    .and_then(|s| s.rsplit('/').next())
                    .and_then(|s| s.split('.').next())
                    .unwrap_or("")
            })
            .collect::<String>();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let params = wbi(params, &keys, timestamp)?;
        self.get(
            path,
            &params
                .iter()
                .map(|(k, v)| (k.as_str(), v.clone()))
                .collect::<Vec<_>>(),
        )
    }
    pub fn folders(&self) -> Result<Vec<Folder>> {
        let account = self.account()?;
        let data = self.get(
            "/x/v3/fav/folder/created/list-all",
            &[("up_mid", account.mid.to_string())],
        )?;
        Ok(data["list"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| {
                Some(Folder {
                    id: v["id"].as_u64()?,
                    title: v["title"].as_str()?.into(),
                })
            })
            .collect())
    }
    fn candidates(&self, selection: &Selection) -> Result<Vec<VideoRef>> {
        match selection.mode {
            Mode::Video => Ok(vec![VideoRef::parse(&selection.video)?]),
            Mode::Recommendations => {
                self.account()?;
                let data = self.signed(
                    "/x/web-interface/wbi/index/top/feed/rcmd",
                    &[
                        ("ps", "30".into()),
                        ("fresh_idx", fastrand::u32(1..100).to_string()),
                        ("fresh_type", "4".into()),
                        ("version", "1".into()),
                        ("feed_version", "V8".into()),
                        ("homepage_ver", "1".into()),
                    ],
                )?;
                Ok(items(&data["item"], true))
            }
            Mode::Favourites => {
                let id = favourite_id(&selection.favourites)?;
                let query = |page: u64| {
                    vec![
                        ("media_id", id.to_string()),
                        ("pn", page.to_string()),
                        ("ps", "20".into()),
                        ("order", "mtime".into()),
                        ("type", "0".into()),
                        ("platform", "web".into()),
                    ]
                };
                let first = self.get("/x/v3/fav/resource/list", &query(1u64))?;
                let page = random_page(first["info"]["media_count"].as_u64().unwrap_or(20), 20);
                let data = if page == 1 {
                    first
                } else {
                    self.get("/x/v3/fav/resource/list", &query(page))?
                };
                Ok(items(&data["medias"], false))
            }
            Mode::Playlist => {
                let list = Playlist::parse(&selection.playlist)?;
                let path = if list.series {
                    "/x/series/archives"
                } else {
                    "/x/polymer/web-space/seasons_archives_list"
                };
                let query = |page: u64| {
                    if list.series {
                        vec![
                            ("mid", list.mid.to_string()),
                            ("series_id", list.id.to_string()),
                            ("pn", page.to_string()),
                            ("ps", "30".into()),
                            ("only_normal", "true".into()),
                            ("sort", "desc".into()),
                        ]
                    } else {
                        vec![
                            ("mid", list.mid.to_string()),
                            ("season_id", list.id.to_string()),
                            ("page_num", page.to_string()),
                            ("page_size", "30".into()),
                        ]
                    }
                };
                let first = self.get(path, &query(1u64))?;
                let page = random_page(
                    first["page"]["total"]
                        .as_u64()
                        .or(first["meta"]["total"].as_u64())
                        .unwrap_or(30),
                    30,
                );
                let data = if page == 1 {
                    first
                } else {
                    self.get(path, &query(page))?
                };
                Ok(items(&data["archives"], false))
            }
        }
    }
    pub fn prepare(&self, settings: &Settings, previous: Option<&str>) -> Result<CachedVideo> {
        settings.validate()?;
        let mut candidates = self.candidates(&settings.selection)?;
        if settings.selection.mode != Mode::Video {
            fastrand::shuffle(&mut candidates);
        }
        candidates.sort_by_key(|v| previous.is_some_and(|s| s.contains(&v.id)));
        let mut last = err("列表中没有可播放的视频");
        for video in candidates.into_iter().take(3) {
            match self.cache(&video, settings) {
                Ok(path) => return Ok(path),
                Err(error) => last = error,
            }
        }
        Err(last)
    }
    fn cache(&self, video: &VideoRef, settings: &Settings) -> Result<CachedVideo> {
        let view = self.get("/x/web-interface/view", &[(video.key(), video.id.clone())])?;
        let pages = view["pages"]
            .as_array()
            .ok_or_else(|| err("视频没有可用分 P"))?;
        let page = pages
            .get(video.page - 1)
            .ok_or_else(|| err("指定的分 P 不存在"))?;
        if page["duration"].as_u64().unwrap_or(u64::MAX) > settings.max_duration_seconds {
            return Err(err("视频超过设置的时长上限"));
        }
        let cid = page["cid"].as_u64().ok_or_else(|| err("视频缺少 CID"))?;
        let bvid = view["bvid"]
            .as_str()
            .filter(|s| valid_bvid(s))
            .ok_or_else(|| err("视频缺少 BV 号"))?;
        fs::create_dir_all(&settings.cache_directory)?;
        let _cache_lock = cache_lock(&settings.cache_directory)?;
        let destination = settings
            .cache_directory
            .join(format!("bili-{bvid}-{cid}-q{}.mp4", settings.quality));
        if let Ok(meta) = fs::symlink_metadata(&destination) {
            if regular(&meta)
                && meta.len() > 0
                && meta.len() <= settings.max_download_mb * 1024 * 1024
            {
                return CachedVideo::open(destination);
            }
            return Err(err("缓存路径被其他文件占用"));
        }
        let stream = self.signed(
            "/x/player/wbi/playurl",
            &[
                ("bvid", bvid.into()),
                ("cid", cid.to_string()),
                ("qn", settings.quality.to_string()),
                ("fnval", "4048".into()),
                ("fourk", "0".into()),
            ],
        )?;
        let rendition = choose_avc(&stream, settings.quality)?;
        let raw = rendition["baseUrl"]
            .as_str()
            .or(rendition["base_url"].as_str())
            .ok_or_else(|| err("视频地址缺失"))?;
        let mut url = Url::parse(raw).map_err(|_| err("视频地址无效"))?;
        if url.scheme() == "http" {
            let _ = url.set_scheme("https");
        }
        if !media_url(&url) {
            return Err(err("视频 CDN 地址不受支持"));
        }
        // Separate unauthenticated client: account cookies never reach a CDN.
        let client = Client::builder()
            .https_only(true)
            .timeout(Duration::from_secs(60))
            .connect_timeout(Duration::from_secs(5))
            .redirect(Policy::custom(|a| {
                if a.previous().len() < 3 && media_url(a.url()) {
                    a.follow()
                } else {
                    a.error("unsupported CDN redirect")
                }
            }))
            .user_agent("Mozilla/5.0")
            .build()
            .map_err(network)?;
        let response = client
            .get(url)
            .header("Referer", "https://www.bilibili.com/")
            .send()
            .map_err(network)?
            .error_for_status()
            .map_err(network)?;
        let limit = settings.max_download_mb * 1024 * 1024;
        if response.content_length().is_some_and(|n| n > limit) {
            return Err(Error::Limit);
        }
        prune(
            &settings.cache_directory,
            settings.cache_mb * 1024 * 1024,
            limit,
        )?;
        let mut temp = tempfile::NamedTempFile::new_in(&settings.cache_directory)?;
        let bytes = std::io::copy(&mut response.take(limit + 1), &mut temp)?;
        if bytes == 0 || bytes > limit {
            return Err(Error::Limit);
        }
        temp.flush()?;
        temp.as_file().sync_all()?;
        publish(temp, destination)
    }
}
fn publish(temp: tempfile::NamedTempFile, destination: PathBuf) -> Result<CachedVideo> {
    match temp.persist_noclobber(&destination) {
        Ok(file) => {
            // Close the writer before opening a read-only Windows lease.
            drop(file);
            CachedVideo::open(destination)
        }
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
            CachedVideo::open(destination)
        }
        Err(error) => Err(Error::Io(error.error)),
    }
}
/// Bridges preparation and decoder startup without allowing concurrent eviction.
pub struct CachedVideo {
    pub path: PathBuf,
    _lease: fs::File,
}
impl CachedVideo {
    fn open(path: PathBuf) -> Result<Self> {
        let mut options = fs::OpenOptions::new();
        options.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(1);
        }
        Ok(Self {
            _lease: options.open(&path)?,
            path,
        })
    }
}
fn cache_lock(dir: &Path) -> Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0);
    }
    options
        .open(dir.join(".iidxbg-cache.lock"))
        .map_err(|_| err("缓存正在由另一个组件准备，请稍后重试"))
}
#[derive(Clone)]
pub struct Account {
    pub name: String,
    pub mid: u64,
}
#[derive(Clone)]
pub struct Folder {
    pub id: u64,
    pub title: String,
}
#[derive(Debug, PartialEq)]
struct VideoRef {
    id: String,
    page: usize,
}
impl VideoRef {
    fn key(&self) -> &'static str {
        if self.id.starts_with("BV") {
            "bvid"
        } else {
            "aid"
        }
    }
    fn parse(text: &str) -> Result<Self> {
        let text = text.trim();
        let (id, page) = if text.starts_with("https://") {
            let url = bili_url(text)?;
            if url.host_str() != Some("www.bilibili.com")
                && url.host_str() != Some("m.bilibili.com")
            {
                return Err(err("请使用完整的 bilibili.com/video/ 链接或 BV/av 号"));
            }
            let mut segments = url.path_segments().ok_or_else(|| err("视频链接无效"))?;
            if segments.next() != Some("video") {
                return Err(err("请使用视频详情页链接"));
            }
            let id = segments.next().unwrap_or("").to_string();
            let page = url
                .query_pairs()
                .find(|(k, _)| k == "p")
                .map(|(_, v)| v.parse::<usize>())
                .transpose()
                .map_err(|_| err("分 P 必须是正整数"))?
                .unwrap_or(1);
            (id, page)
        } else {
            (text.to_owned(), 1)
        };
        if page == 0 || page > 10000 {
            return Err(err("分 P 超出范围"));
        }
        let id = if valid_bvid(&id) {
            id
        } else if let Some(aid) = id.strip_prefix("av").filter(|s| positive_id(s).is_ok()) {
            aid.to_owned()
        } else {
            return Err(err("请输入完整视频链接、BV 号或 av 号"));
        };
        Ok(Self { id, page })
    }
}
fn valid_bvid(s: &str) -> bool {
    s.len() == 12 && s.starts_with("BV") && s.bytes().all(|b| b.is_ascii_alphanumeric())
}
fn positive_id(s: &str) -> Result<u64> {
    s.parse::<u64>()
        .ok()
        .filter(|n| *n > 0)
        .ok_or_else(|| err("列表 ID 必须是正整数"))
}
fn bili_url(text: &str) -> Result<Url> {
    let url = Url::parse(text).map_err(|_| err("链接无效"))?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port_or_known_default() != Some(443)
        || !matches!(
            url.host_str(),
            Some("www.bilibili.com" | "m.bilibili.com" | "space.bilibili.com")
        )
    {
        return Err(err("请输入完整的 HTTPS Bilibili 链接"));
    }
    Ok(url)
}
fn favourite_id(text: &str) -> Result<u64> {
    if !text.trim().starts_with("https://") {
        return positive_id(text.trim());
    }
    let url = bili_url(text.trim())?;
    let pairs = url.query_pairs().collect::<BTreeMap<_, _>>();
    if let Some(id) = pairs.get("fid").or(pairs.get("media_id")) {
        return positive_id(id);
    }
    let parts = url
        .path_segments()
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    if parts.first() == Some(&"list") && pairs.get("type").is_some_and(|v| v == "fav") {
        return positive_id(parts.get(1).copied().unwrap_or(""));
    }
    if parts.get(1) == Some(&"lists") && pairs.get("type").is_some_and(|v| v == "fav") {
        return positive_id(parts.get(2).copied().unwrap_or(""));
    }
    Err(err("请粘贴带 fid 的收藏夹链接或收藏夹 ID"))
}
#[derive(Debug, PartialEq)]
struct Playlist {
    mid: u64,
    id: u64,
    series: bool,
}
impl Playlist {
    fn parse(text: &str) -> Result<Self> {
        let url = bili_url(text.trim())?;
        if url.host_str() != Some("space.bilibili.com") {
            return Err(err("请粘贴 UP 主空间中的合集或系列链接"));
        }
        let parts = url
            .path_segments()
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        let mid = positive_id(parts.first().copied().unwrap_or(""))?;
        let pairs = url.query_pairs().collect::<BTreeMap<_, _>>();
        let series =
            parts.contains(&"seriesdetail") || pairs.get("type").is_some_and(|s| s == "series");
        let id = if parts.get(1) == Some(&"lists") {
            parts.get(2).copied().unwrap_or("")
        } else {
            pairs
                .get("sid")
                .or(pairs.get("season_id"))
                .or(pairs.get("series_id"))
                .map(|v| v.as_ref())
                .unwrap_or("")
        };
        Ok(Self {
            mid,
            id: positive_id(id)?,
            series,
        })
    }
}
fn items(value: &Value, feed: bool) -> Vec<VideoRef> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter(|v| !feed || v["goto"] == "av")
        .filter_map(|v| v["bvid"].as_str())
        .filter(|s| valid_bvid(s))
        .map(|s| VideoRef {
            id: s.into(),
            page: 1,
        })
        .collect()
}
fn random_page(total: u64, size: u64) -> u64 {
    fastrand::u64(1..=total.div_ceil(size).clamp(1, 1000))
}
fn choose_avc(data: &Value, quality: u32) -> Result<&Value> {
    data["dash"]["video"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|v| {
            v["codecs"].as_str().is_some_and(|s| s.starts_with("avc1"))
                && v["id"].as_u64().is_some_and(|q| q <= quality as u64)
        })
        .max_by_key(|v| v["id"].as_u64())
        .ok_or_else(|| err("此视频没有可访问的 H.264 版本；请登录或选择其他视频"))
}
fn media_url(url: &Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port_or_known_default() == Some(443)
        && url.host_str().is_some_and(|h| {
            ["bilivideo.com", "bilivideo.cn"]
                .iter()
                .any(|d| h == *d || h.strip_suffix(d).is_some_and(|p| p.ends_with('.')))
        })
}
fn regular(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.is_file() && meta.file_attributes() & 0x400 == 0
    }
    #[cfg(not(windows))]
    {
        meta.is_file() && !meta.file_type().is_symlink()
    }
}
fn cache_name(name: &str) -> bool {
    let Some(name) = name
        .strip_prefix("bili-")
        .and_then(|s| s.strip_suffix(".mp4"))
    else {
        return false;
    };
    let parts = name.split('-').collect::<Vec<_>>();
    parts.len() == 3
        && valid_bvid(parts[0])
        && positive_id(parts[1]).is_ok()
        && parts[2]
            .strip_prefix('q')
            .is_some_and(|q| ["16", "32", "64", "80"].contains(&q))
}
fn prune(dir: &Path, limit: u64, reserve: u64) -> Result<()> {
    let mut files = Vec::new();
    let mut total = 0u64;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if !entry.file_name().to_str().is_some_and(cache_name) {
            continue;
        }
        let meta = fs::symlink_metadata(entry.path())?;
        if !regular(&meta) {
            continue;
        }
        total = total.saturating_add(meta.len());
        files.push((
            meta.modified().unwrap_or(UNIX_EPOCH),
            entry.path(),
            meta.len(),
        ));
    }
    files.sort_by_key(|f| f.0);
    for (_, path, len) in files {
        if total.saturating_add(reserve) <= limit {
            break;
        }
        // Active video files hold a Windows read lease denying deletion.
        if fs::symlink_metadata(&path).is_ok_and(|m| regular(&m)) && fs::remove_file(path).is_ok() {
            total = total.saturating_sub(len);
        }
    }
    if total.saturating_add(reserve) > limit {
        return Err(err("视频缓存已满，正在播放的文件暂时无法清理"));
    }
    Ok(())
}
fn percent(text: &str) -> String {
    let mut out = String::new();
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}
fn wbi(params: &[(&str, String)], keys: &str, timestamp: u64) -> Result<Vec<(String, String)>> {
    const ORDER: [usize; 64] = [
        46, 47, 18, 2, 53, 8, 23, 32, 15, 50, 10, 31, 58, 3, 45, 35, 27, 43, 5, 49, 33, 9, 42, 19,
        29, 28, 14, 39, 12, 38, 41, 13, 37, 48, 7, 16, 24, 55, 40, 61, 26, 17, 0, 1, 60, 51, 30, 4,
        22, 25, 54, 21, 56, 59, 6, 63, 57, 62, 11, 36, 20, 34, 44, 52,
    ];
    if keys.len() != 64 || !keys.bytes().all(|c| c.is_ascii_alphanumeric()) {
        return Err(err("无法取得请求签名参数"));
    }
    let salt = ORDER[..32]
        .iter()
        .map(|i| keys.as_bytes()[*i] as char)
        .collect::<String>();
    let mut values = params
        .iter()
        .map(|(k, v)| {
            (
                k.to_string(),
                v.chars()
                    .filter(|c| !"!'()*".contains(*c))
                    .collect::<String>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    values.insert("wts".into(), timestamp.to_string());
    let query = values
        .iter()
        .map(|(k, v)| format!("{}={}", percent(k), percent(v)))
        .collect::<Vec<_>>()
        .join("&");
    values.insert(
        "w_rid".into(),
        format!("{:x}", md5::compute(format!("{query}{salt}"))),
    );
    Ok(values.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn links_ids_and_multipart_selection() {
        assert_eq!(
            VideoRef::parse("https://www.bilibili.com/video/BV1Pqi4BzEkP/?p=2&share_source=copy")
                .unwrap(),
            VideoRef {
                id: "BV1Pqi4BzEkP".into(),
                page: 2
            }
        );
        assert_eq!(VideoRef::parse("av170001").unwrap().id, "170001");
        for invalid in [
            "https://evil.invalid/video/BV1Pqi4BzEkP",
            "https://user@www.bilibili.com/video/BV1Pqi4BzEkP",
            "https://www.bilibili.com/video/BV1Pqi4BzEkP?p=0",
            "BV/../../file",
            "BV1Pqi4BzEkP?x",
        ] {
            assert!(VideoRef::parse(invalid).is_err(), "{invalid}");
        }
        assert_eq!(
            favourite_id("https://space.bilibili.com/123/favlist?fid=456").unwrap(),
            456
        );
        assert_eq!(
            favourite_id("https://www.bilibili.com/list/456?type=fav").unwrap(),
            456
        );
        assert_eq!(favourite_id("456").unwrap(), 456);
        assert_eq!(
            favourite_id("https://space.bilibili.com/123/lists/456?type=fav").unwrap(),
            456
        );
        assert!(favourite_id("0").is_err());
        assert_eq!(
            Playlist::parse("https://space.bilibili.com/123/lists/456?type=season").unwrap(),
            Playlist {
                mid: 123,
                id: 456,
                series: false
            }
        );
        assert!(
            Playlist::parse("https://space.bilibili.com/123/channel/seriesdetail?sid=456")
                .unwrap()
                .series
        );
        assert!(
            !Playlist::parse("https://space.bilibili.com/123/channel/collectiondetail?sid=456")
                .unwrap()
                .series
        );
    }
    #[test]
    fn codec_selection_and_cdn_policy() {
        let data = serde_json::json!({"dash":{"video":[{"id":80,"codecs":"hvc1"},{"id":80,"codecs":"avc1.high"},{"id":32,"codecs":"avc1.low"},{"id":64,"codecs":"av01"}]}});
        assert_eq!(choose_avc(&data, 64).unwrap()["id"], 32);
        assert_eq!(choose_avc(&data, 80).unwrap()["id"], 80);
        assert!(choose_avc(&data, 16).is_err());
        assert!(media_url(
            &Url::parse("https://upos-sz.bilivideo.com/video.m4s").unwrap()
        ));
        for s in [
            "http://upos.bilivideo.com/a",
            "https://bilivideo.com.evil.invalid/a",
            "https://user@bilivideo.com/a",
            "https://bilivideo.com:444/a",
            "https://127.0.0.1/a",
        ] {
            assert!(!media_url(&Url::parse(s).unwrap()));
        }
    }
    #[test]
    fn signing_is_sorted_escaped_and_filters_forbidden_characters() {
        let keys = "7cd084941338484aae1ad9425b84077c4932ad7a770c849cba1942a50d1b17d4";
        let signed = wbi(
            &[
                ("foo", "114".into()),
                ("bar", "514".into()),
                ("baz", "1919810".into()),
            ],
            keys,
            1702204169,
        )
        .unwrap()
        .into_iter()
        .collect::<BTreeMap<_, _>>();
        // Independently calculated with Python hashlib for this fixed query/salt.
        assert_eq!(signed["w_rid"], "037599307680561764bb9893b2b33236");
        let filtered = wbi(&[("q", "a !'()*中".into())], keys, 1).unwrap();
        assert!(filtered.contains(&("q".into(), "a 中".into())));
        assert_eq!(percent("a 中"), "a%20%E4%B8%AD");
        assert!(wbi(&[], "bad", 1).is_err());
    }
    #[test]
    fn cache_pruning_preserves_other_files_and_pinned_media() {
        let dir = tempfile::tempdir().unwrap();
        let owned = dir.path().join("bili-BV1Pqi4BzEkP-1-q64.mp4");
        fs::write(&owned, [1; 16]).unwrap();
        fs::write(dir.path().join("personal.mp4"), [2; 16]).unwrap();
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            let lease = fs::OpenOptions::new()
                .read(true)
                .share_mode(1)
                .open(&owned)
                .unwrap();
            assert!(prune(dir.path(), 16, 16).is_err());
            assert!(owned.exists());
            drop(lease);
        }
        prune(dir.path(), 16, 16).unwrap();
        assert!(!owned.exists());
        assert!(dir.path().join("personal.mp4").exists());
    }
    #[test]
    fn publishing_closes_writer_and_pins_complete_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bili-BV1Pqi4BzEkP-1-q64.mp4");
        let mut temp = tempfile::NamedTempFile::new_in(dir.path()).unwrap();
        temp.write_all(b"complete fixture").unwrap();
        let cached = publish(temp, path.clone()).unwrap();
        assert_eq!(fs::read(&cached.path).unwrap(), b"complete fixture");
        #[cfg(windows)]
        assert!(fs::remove_file(&path).is_err());
        drop(cached);
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn config_binding_and_invalid_limits() {
        let text = include_str!("../tests/fixtures/default.toml");
        let mut config = crate::config::Config::parse(text).unwrap();
        config.sources.lift.source = Some(crate::config::Source::Bilibili);
        config.sources.lift.bilibili = Some(Selection {
            mode: Mode::Favourites,
            favourites: "123".into(),
            ..Default::default()
        });
        config.sources.sudden_plus.bind = Some(crate::config::Surface::Lift);
        let resolved = config.for_surface(crate::config::Surface::SuddenPlus);
        assert_eq!(resolved.bilibili.selection.favourites, "123");
        assert_eq!(resolved.source, crate::config::Source::Bilibili);
        config.bilibili.max_download_mb = 2048;
        assert!(config.validate().is_err());
        assert!(
            crate::config::Config::parse(&format!("{text}\n[bilibili]\nunknown=true")).is_err()
        );
    }
}
