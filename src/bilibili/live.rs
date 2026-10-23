//! Bounded, read-only Bilibili HLS/fMP4 transport. No graphics or decoder calls.
use super::{Api, err, media_url, network};
use crate::{Error, Result, pixels::read_bounded};
use reqwest::{Url, blocking::Client, redirect::Policy};
use std::{
    io::Write,
    time::{Duration, Instant},
};

pub const CHUNK_LIMIT: usize = 16 * 1024 * 1024;
const INIT_LIMIT: usize = 2 * 1024 * 1024;

pub fn room_id(input: &str) -> Result<u64> {
    let input = input.trim();
    if !input.is_empty() && input.bytes().all(|b| b.is_ascii_digit()) {
        return input
            .parse::<u64>()
            .ok()
            .filter(|n| *n > 0)
            .ok_or_else(|| err("直播间 ID 无效"));
    }
    let url = Url::parse(input).map_err(|_| err("请输入直播间数字 ID 或完整链接"))?;
    if url.scheme() != "https"
        || url.host_str() != Some("live.bilibili.com")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port_or_known_default() != Some(443)
    {
        return Err(err("直播间链接无效"));
    }
    let path = url.path().trim_matches('/');
    let path = path.strip_prefix("blanc/").unwrap_or(path);
    if path.contains('/') {
        return Err(err("直播间链接无效"));
    }
    if path.is_empty() || !path.bytes().all(|b| b.is_ascii_digit()) {
        return Err(err("直播间 ID 无效"));
    }
    room_id(path)
}

pub struct Chunk {
    // Closed writer; MF opens its own handle. The path is removed after Reader drops.
    pub path: tempfile::TempPath,
    pub sequence: u64,
    pub discontinuity: bool,
}

struct Segment {
    url: Url,
    sequence: u64,
    key: bool,
}
struct Playlist {
    init: Url,
    segments: Vec<Segment>,
    ended: bool,
}

fn child_url(base: &Url, child: &str) -> Result<Url> {
    let mut url = base.join(child).map_err(|_| err("直播分片地址无效"))?;
    if !media_url(&url) {
        return Err(err("直播 CDN 地址不受支持"));
    }
    // Bilibili relative segment names require the playlist's signed query.
    // Never copy that query to a different host.
    if url.query().is_none() && url.host_str() == base.host_str() {
        url.set_query(base.query());
    }
    Ok(url)
}

fn playlist(base: &Url, text: &str) -> Result<Playlist> {
    if !text.trim_start_matches('\u{feff}').starts_with("#EXTM3U") {
        return Err(err("直播播放列表无效"));
    }
    let mut sequence = None;
    let mut init = None;
    let mut segments = Vec::new();
    let mut key = false;
    let mut independent = false;
    let mut duration = false;
    let mut ended = false;
    for line in text.lines().map(str::trim) {
        if let Some(value) = line.strip_prefix("#EXT-X-MEDIA-SEQUENCE:") {
            sequence = value.parse::<u64>().ok();
        } else if let Some(value) = line.strip_prefix("#EXT-X-MAP:") {
            if value.contains("BYTERANGE") {
                return Err(err("不支持直播字节范围分片"));
            }
            let uri = value
                .strip_prefix("URI=\"")
                .and_then(|v| v.split_once('"'))
                .map(|(v, _)| v)
                .ok_or_else(|| err("直播初始化分片无效"))?;
            let next = child_url(base, uri)?;
            if init.as_ref().is_some_and(|old| old != &next) {
                segments.clear();
            }
            init = Some(next);
        } else if line == "#EXT-X-DISCONTINUITY" {
            segments.clear();
        } else if line.starts_with("#EXT-X-BYTERANGE:")
            || (line.starts_with("#EXT-X-KEY:") && line != "#EXT-X-KEY:METHOD=NONE")
        {
            return Err(err("不支持加密或字节范围直播"));
        } else if line == "#EXT-X-INDEPENDENT-SEGMENTS" {
            independent = true;
        } else if let Some(value) = line.strip_prefix("#EXT-BILI-AUX:") {
            key = value.split('|').nth(1) == Some("K");
        } else if let Some(value) = line.strip_prefix("#EXTINF:") {
            let seconds = value
                .split(',')
                .next()
                .unwrap_or("")
                .parse::<f64>()
                .map_err(|_| err("直播分片时长无效"))?;
            if !seconds.is_finite() || !(0.05..=10.0).contains(&seconds) {
                return Err(Error::Limit);
            }
            duration = true;
        } else if line == "#EXT-X-ENDLIST" {
            ended = true;
        } else if !line.is_empty() && !line.starts_with('#') {
            if !duration || segments.len() >= 120 {
                return Err(err("不支持此直播播放列表"));
            }
            let seq = sequence.ok_or_else(|| err("直播分片缺少序号"))?;
            segments.push(Segment {
                url: child_url(base, line)?,
                sequence: seq,
                key: key || independent,
            });
            sequence = seq.checked_add(1);
            key = false;
            duration = false;
        }
    }
    Ok(Playlist {
        init: init.ok_or_else(|| err("直播没有 fMP4 初始化分片"))?,
        segments,
        ended,
    })
}

fn group(p: &Playlist, next: Option<u64>) -> Option<(usize, usize)> {
    let keys: Vec<_> = p
        .segments
        .iter()
        .enumerate()
        .filter_map(|(i, s)| s.key.then_some(i))
        .collect();
    let groups: Vec<_> = keys.windows(2).map(|w| (w[0], w[1])).collect();
    match next {
        Some(seq) if p.segments.first()?.sequence <= seq => groups
            .into_iter()
            .find(|(start, _)| p.segments[*start].sequence >= seq),
        _ => groups.last().copied(),
    }
}

pub struct Stream {
    room: u64,
    client: Client,
    address: Option<Url>,
    init: Option<(Url, Vec<u8>)>,
    next: Option<u64>,
}
impl Stream {
    pub fn new(room: u64) -> Result<Self> {
        let client = Client::builder()
            .https_only(true)
            .timeout(Duration::from_secs(8))
            .connect_timeout(Duration::from_secs(4))
            .user_agent("Mozilla/5.0")
            .redirect(Policy::custom(|a| {
                if a.previous().len() < 3 && media_url(a.url()) {
                    a.follow()
                } else {
                    a.error("unsupported live CDN redirect")
                }
            }))
            .build()
            .map_err(network)?;
        Ok(Self {
            room,
            client,
            address: None,
            init: None,
            next: None,
        })
    }
    pub fn reset(&mut self) {
        self.address = None;
        self.init = None;
        self.next = None;
    }
    fn resolve(&self) -> Result<Url> {
        let api = Api::saved()?;
        let data = Api::data(api.response(
            "api.live.bilibili.com",
            "/xlive/web-room/v2/index/getRoomPlayInfo",
            &[
                ("room_id", self.room.to_string()),
                ("protocol", "1".into()),
                ("format", "2".into()),
                ("codec", "0".into()),
                ("qn", "250".into()),
                ("platform", "web".into()),
            ],
        )?)?;
        if data["live_status"].as_u64() != Some(1) {
            return Err(err("直播间未开播"));
        }
        for stream in data["playurl_info"]["playurl"]["stream"]
            .as_array()
            .into_iter()
            .flatten()
        {
            if stream["protocol_name"] != "http_hls" {
                continue;
            }
            for format in stream["format"].as_array().into_iter().flatten() {
                if format["format_name"] != "fmp4" {
                    continue;
                }
                for codec in format["codec"].as_array().into_iter().flatten() {
                    if codec["codec_name"] != "avc" {
                        continue;
                    }
                    for info in codec["url_info"].as_array().into_iter().flatten() {
                        if let (Some(host), Some(path), Some(extra)) = (
                            info["host"].as_str(),
                            codec["base_url"].as_str(),
                            info["extra"].as_str(),
                        ) && let Ok(url) = Url::parse(&format!("{host}{path}{extra}"))
                            && media_url(&url)
                        {
                            return Ok(url);
                        }
                    }
                }
            }
        }
        Err(err("直播间没有可用的 H.264 fMP4 流"))
    }
    fn bytes(&self, url: &Url, limit: usize) -> Result<Vec<u8>> {
        if !media_url(url) {
            return Err(err("直播 CDN 地址不受支持"));
        }
        let response = self
            .client
            .get(url.clone())
            .header("Referer", "https://live.bilibili.com/")
            .send()
            .map_err(network)?
            .error_for_status()
            .map_err(network)?;
        read_bounded(response, limit as u64)
    }
    /// At most one complete GOP group, never a whole broadcast. Caller owns pacing/retry.
    pub fn next_chunk(&mut self, cancelled: impl Fn() -> bool) -> Result<Option<Chunk>> {
        if cancelled() {
            return Ok(None);
        }
        if self.address.is_none() {
            self.address = Some(self.resolve()?);
        }
        let base = self.address.as_ref().expect("resolved").clone();
        let raw = self.bytes(&base, 256 * 1024)?;
        let p = playlist(
            &base,
            std::str::from_utf8(&raw).map_err(|_| err("直播列表编码无效"))?,
        )?;
        let Some((start, end)) = group(&p, self.next) else {
            if p.ended {
                self.reset();
                return Err(err("直播流已结束"));
            }
            return Ok(None);
        };
        if self.init.as_ref().is_none_or(|(url, _)| url != &p.init) {
            self.next = None;
            self.init = Some((p.init.clone(), self.bytes(&p.init, INIT_LIMIT)?));
        }
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut file = tempfile::Builder::new()
            .prefix("iidxbg-live-")
            .suffix(".mp4")
            .tempfile()?;
        let init = &self.init.as_ref().expect("initialized").1;
        file.write_all(init)?;
        let mut count = init.len();
        for segment in &p.segments[start..end] {
            if cancelled() {
                return Ok(None);
            }
            if Instant::now() >= deadline {
                return Err(err("直播缓冲超时"));
            }
            let bytes = self.bytes(&segment.url, CHUNK_LIMIT.saturating_sub(count))?;
            count += bytes.len();
            file.write_all(&bytes)?;
        }
        file.flush()?;
        let sequence = p.segments[start].sequence;
        let discontinuity = self.next != Some(sequence);
        self.next = Some(p.segments[end].sequence);
        Ok(Some(Chunk {
            path: file.into_temp_path(),
            sequence,
            discontinuity,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn room_inputs_and_cdn_boundaries() {
        assert_eq!(room_id("24065").unwrap(), 24065);
        assert_eq!(
            room_id("https://live.bilibili.com/blanc/24065?x=1").unwrap(),
            24065
        );
        for invalid in [
            "0",
            "-1",
            "https://evil.test/24065",
            "https://live.bilibili.com@evil.test/24065",
            "https://live.bilibili.com/x/24065",
        ] {
            assert!(room_id(invalid).is_err());
        }
        let base = Url::parse("https://a.bilivideo.com/live/index.m3u8?secret=test").unwrap();
        assert_eq!(
            child_url(&base, "1.m4s").unwrap().query(),
            Some("secret=test")
        );
        assert!(child_url(&base, "https://127.0.0.1/file").is_err());
        assert!(child_url(&base, "https://a.bilivideo.com.evil.test/file").is_err());
        assert_eq!(
            child_url(&base, "https://b.bilivideo.com/file")
                .unwrap()
                .query(),
            None
        );
    }
    #[test]
    fn keyframe_groups_skip_incomplete_tail_and_catch_up() {
        let base = Url::parse("https://a.bilivideo.com/live/index.m3u8").unwrap();
        let text = "#EXTM3U\n#EXT-X-MEDIA-SEQUENCE:10\n#EXT-X-MAP:URI=\"init.m4s\"\n#EXT-BILI-AUX:x|K|x\n#EXTINF:1,\n10.m4s\n#EXTINF:1,\n11.m4s\n#EXT-BILI-AUX:x|K|x\n#EXTINF:1,\n12.m4s\n#EXTINF:1,\n13.m4s\n#EXT-BILI-AUX:x|K|x\n#EXTINF:1,\n14.m4s\n";
        let p = playlist(&base, text).unwrap();
        assert_eq!(group(&p, None), Some((2, 4)));
        assert_eq!(group(&p, Some(10)), Some((0, 2)));
        assert_eq!(group(&p, Some(1)), Some((2, 4)));
        assert_eq!(group(&p, Some(14)), None);
        assert!(playlist(&base, &text.replace("#EXTINF:1,", "#EXTINF:NaN,")).is_err());
        assert!(playlist(&base, &format!("{text}#EXT-X-KEY:METHOD=AES-128\n")).is_err());
        assert!(playlist(&base, &text.replace("10.m4s", "file:///secret")).is_err());
    }
}
