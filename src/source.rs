use crate::{
    Error, Result,
    config::{Config, MoebooruDomain, Source, Surface},
    media::Asset,
    pixels::{self, Prepared},
};
use reqwest::{Url, blocking::Client, redirect::Policy};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime},
};

#[derive(Debug, Deserialize)]
struct Post {
    id: u64,
    #[serde(default)]
    sample_url: String,
    #[serde(default)]
    jpeg_url: String,
    #[serde(default)]
    file_url: String,
}

pub fn allowed_url(domain: MoebooruDomain, url: &Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port_or_known_default() == Some(443)
        && url.host_str().is_some_and(|h| domain.allows_host(h))
}

pub struct Provider {
    config: Config,
    client: Client,
    previous: Option<String>,
    /// Non-recursive local listings are reused while the directory mtime is
    /// unchanged, so repeated scene requests do not rescan the same folder.
    scan: Option<ScanCache>,
    media_cache: Option<(PathBuf, SystemTime, u64, Asset)>,
}

struct ScanCache {
    directory: PathBuf,
    modified: SystemTime,
    paths: Vec<PathBuf>,
}

/// Independent repeat histories and HTTP policies; only the image worker owns this.
pub struct Providers {
    config: Config,
    providers: [Provider; Surface::COUNT],
}

impl Providers {
    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn new(config: Config) -> Result<Self> {
        let providers = Surface::ALL
            .into_iter()
            .map(|s| Provider::new(config.for_surface(s)))
            .collect::<Result<Vec<_>>>()?;
        let providers = providers.try_into().ok().expect("one provider per surface");
        Ok(Self { config, providers })
    }

    pub fn reload(&mut self, path: &Path) -> Result<bool> {
        let config = Config::load(path)?;
        if config == self.config {
            return Ok(false);
        }
        let mut replacement = Self::new(config)?;
        for (new, old) in replacement.providers.iter_mut().zip(&mut self.providers) {
            new.previous = old.previous.take();
        }
        *self = replacement;
        Ok(true)
    }

    pub fn enabled(&self, surface: Surface) -> bool {
        self.providers[surface.index()].enabled()
    }

    pub fn prepare(&mut self, surface: Surface) -> Result<Option<Prepared>> {
        let provider = &mut self.providers[surface.index()];
        if provider.enabled() {
            provider.prepare().map(Some)
        } else {
            Ok(None)
        }
    }

    /// Prepare even when the surface's own switch is off. Used once for the
    /// premium-area image that a concentration-only configuration shares.
    pub fn prepare_unchecked(&mut self, surface: Surface) -> Result<Prepared> {
        self.providers[surface.index()].prepare()
    }

    pub fn prepare_media(&mut self, surface: Surface) -> Result<Asset> {
        self.providers[surface.index()].prepare_media()
    }

    pub fn cleanup(&self, surface: Surface) -> Result<usize> {
        let provider = &self.providers[surface.index()];
        if !provider.enabled() {
            return Ok(0);
        }
        cleanup_downloads(
            &provider.config.moebooru.save_directory,
            provider.config.moebooru.max_images,
        )
    }
}

impl Provider {
    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn new(config: Config) -> Result<Self> {
        let policy = config.moebooru.clone();
        let client = Client::builder()
            .user_agent(concat!(
                "iidxbg/",
                env!("CARGO_PKG_VERSION"),
                " (results-background plugin)"
            ))
            .timeout(Duration::from_secs(config.moebooru.timeout_seconds))
            .connect_timeout(Duration::from_secs(5))
            .https_only(!config.moebooru.posts_url().starts_with("http://"))
            .redirect(Policy::custom(move |attempt| {
                if attempt.previous().len() >= 3 || !policy.allows_url(attempt.url()) {
                    attempt.error("unsupported redirect")
                } else {
                    attempt.follow()
                }
            }))
            .build()?;
        Ok(Self {
            config,
            client,
            previous: None,
            scan: None,
            media_cache: None,
        })
    }

    /// Worker-only: replace the whole validated snapshot and its HTTP policy.
    /// A read, parse, validation or client-build failure leaves this provider intact.
    pub fn reload(&mut self, path: &Path) -> Result<bool> {
        let config = Config::load(path)?;
        if config == self.config {
            return Ok(false);
        }
        let mut replacement = Self::new(config)?;
        replacement.previous = self.previous.take();
        *self = replacement;
        Ok(true)
    }

    pub fn enabled(&self) -> bool {
        self.config.enabled
    }

    pub fn prepare(&mut self) -> Result<Prepared> {
        self.prepare_media().map(|asset| (*asset.first()).clone())
    }

    pub fn prepare_media(&mut self) -> Result<Asset> {
        let result = match self.config.source {
            Source::Bilibili => {
                let path = crate::bilibili::Api::saved()?
                    .prepare(&self.config.bilibili, self.previous.as_deref())?;
                self.load_media(&path.path)
            }
            Source::File => self.load_media(&self.config.media_path.clone()),
            Source::Local => self.local(),
            Source::Remote => self.remote().map(|p| Asset::Image(Arc::new(p))),
            Source::Mixed if fastrand::bool() => self
                .remote()
                .map(|p| Asset::Image(Arc::new(p)))
                .or_else(|_| self.local()),
            Source::Mixed => self
                .local()
                .or_else(|_| self.remote().map(|p| Asset::Image(Arc::new(p)))),
        }?;
        self.previous = Some(result.label.clone());
        Ok(result)
    }

    fn load_media(&mut self, path: &Path) -> Result<Asset> {
        let meta = fs::metadata(path)?;
        let modified = meta.modified()?;
        if let Some((cached, stamp, len, asset)) = &self.media_cache
            && cached == path
            && *stamp == modified
            && *len == meta.len()
            && !matches!(asset, Asset::Video(v) if v.failed())
        {
            return Ok(asset.clone());
        }
        let asset = if crate::media::is_video(path) {
            #[cfg(windows)]
            {
                Asset::Video(crate::video::load(
                    path,
                    self.config.image.width,
                    self.config.image.height,
                )?)
            }
            #[cfg(not(windows))]
            {
                return Err(Error::Video(
                    "local video requires Windows Media Foundation".into(),
                ));
            }
        } else {
            let bytes = pixels::read_bounded(fs::File::open(path)?, self.config.byte_limit())?;
            Asset::Image(Arc::new(pixels::decode(
                &bytes,
                &self.config.image,
                path.to_string_lossy().into_owned(),
            )?))
        };
        self.media_cache = Some((path.to_owned(), modified, meta.len(), asset.clone()));
        Ok(asset)
    }

    fn local(&mut self) -> Result<Asset> {
        let mut paths = self.local_paths()?;
        fastrand::shuffle(&mut paths);
        // Prefer another image, but permit a singleton library.
        paths.sort_by_key(|p| {
            self.previous
                .as_deref()
                .map(|s| s.strip_prefix("video:").unwrap_or(s))
                == Some(&p.to_string_lossy())
        });
        for path in paths.into_iter().take(32) {
            let attempt = self.load_media(&path);
            if let Ok(image) = attempt {
                return Ok(image);
            }
        }
        Err(Error::NoImages)
    }

    fn local_paths(&mut self) -> Result<Vec<PathBuf>> {
        let directory = self.config.local_directory.clone();
        // A recursive walk has no single cheap freshness key; only flat folders
        // (the normal layout) reuse the listing.
        if self.config.recursive {
            let mut paths = Vec::new();
            collect(&directory, true, 0, &mut paths)?;
            return Ok(paths);
        }
        let modified = fs::metadata(&directory).and_then(|m| m.modified()).ok();
        if let (Some(modified), Some(cache)) = (modified, &self.scan)
            && cache.directory == directory
            && cache.modified == modified
        {
            return Ok(cache.paths.clone());
        }
        let mut paths = Vec::new();
        match collect(&directory, false, 0, &mut paths) {
            Ok(()) => {
                if let Some(modified) = modified {
                    self.scan = Some(ScanCache {
                        directory,
                        modified,
                        paths: paths.clone(),
                    });
                }
                Ok(paths)
            }
            Err(e) => {
                self.scan = None;
                Err(e)
            }
        }
    }

    fn remote(&self) -> Result<Prepared> {
        let page = fastrand::u32(1..=self.config.moebooru.page_max);
        let mut posts = self.posts(page)?;
        if posts.is_empty() && page != 1 {
            posts = self.posts(1)?;
        }
        fastrand::shuffle(&mut posts);
        posts.sort_by_key(|p| {
            self.previous.as_deref() == Some(&self.config.moebooru.post_label(p.id))
        });
        let mut last_error = Error::NoImages;
        for post in posts.into_iter().take(3) {
            match self.fetch_post(&post) {
                Ok(image) => return Ok(image),
                Err(e) => last_error = e,
            }
        }
        Err(last_error)
    }

    fn posts(&self, page: u32) -> Result<Vec<Post>> {
        let response = self
            .client
            .get(self.config.moebooru.posts_url())
            .query(&[
                ("limit", "100"),
                ("page", &page.to_string()),
                ("tags", self.config.moebooru.tags.as_str()),
            ])
            .send()?
            .error_for_status()?;
        let bytes = pixels::read_bounded(response, 2 * 1024 * 1024)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    fn fetch_post(&self, post: &Post) -> Result<Prepared> {
        let text = [&post.sample_url, &post.jpeg_url, &post.file_url]
            .into_iter()
            .find(|s| !s.is_empty())
            .ok_or(Error::NoImages)?;
        let url = Url::parse(self.config.moebooru.posts_url())
            .map_err(|_| Error::Url)?
            .join(text)
            .map_err(|_| Error::Url)?;
        if !self.config.moebooru.allows_url(&url) {
            return Err(Error::Url);
        }
        let response = self.client.get(url).send()?.error_for_status()?;
        if response
            .content_length()
            .is_some_and(|n| n > self.config.byte_limit())
        {
            return Err(Error::Limit);
        }
        let bytes = pixels::read_bounded(response, self.config.byte_limit())?;
        let prepared = pixels::decode(
            &bytes,
            &self.config.image,
            self.config.moebooru.post_label(post.id),
        )?;
        let extension = match image::guess_format(&bytes)? {
            image::ImageFormat::Png => "png",
            image::ImageFormat::Jpeg => "jpg",
            _ => return Err(Error::Format),
        };
        let hash = hex::encode(Sha256::digest(&bytes));
        let name = format!(
            "{}-{}-{}.{}",
            self.config.moebooru.namespace(),
            post.id,
            &hash[..20],
            extension
        );
        save(&self.config.moebooru.save_directory, &name, &bytes)?;
        Ok(prepared)
    }
}

fn collect(root: &Path, recursive: bool, depth: usize, out: &mut Vec<PathBuf>) -> Result<()> {
    if depth > 8 || out.len() >= 10000 {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            continue;
        }
        let path = entry.path();
        if kind.is_dir() && recursive {
            // Skip Windows junction/reparse directories as well as symlinks.
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if entry.metadata()?.file_attributes() & 0x400 != 0 {
                    continue;
                }
            }
            collect(&path, recursive, depth + 1, out)?;
        } else if kind.is_file()
            && path.extension().is_some_and(|s| {
                [
                    "jpg", "jpeg", "png", "mp4", "m4v", "mov", "wmv", "avi", "mkv", "webm",
                ]
                .iter()
                .any(|x| s.to_string_lossy().eq_ignore_ascii_case(x))
            })
        {
            out.push(path);
        }
        if out.len() >= 10000 {
            break;
        }
    }
    Ok(())
}

fn save(root: &Path, name: &str, bytes: &[u8]) -> Result<()> {
    fs::create_dir_all(root)?;
    let final_path = root.join(name);
    if final_path.is_file() {
        return Ok(());
    }
    let temp = root.join(format!(".iidxbg-{:016x}.part", fastrand::u64(..)));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    let result = (|| -> std::io::Result<()> {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp, &final_path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result?;
    Ok(())
}

fn managed_image(name: &str) -> bool {
    let Some((stem, extension)) = name.rsplit_once('.') else {
        return false;
    };
    if !matches!(extension, "png" | "jpg") {
        return false;
    }
    let parts: Vec<_> = stem.split('-').collect();
    parts.len() == 3
        && (matches!(parts[0], "konachan" | "yandere")
            || parts[0]
                .strip_prefix("booru")
                .is_some_and(|s| s.len() == 12 && s.bytes().all(|b| b.is_ascii_hexdigit())))
        && !parts[1].is_empty()
        && parts[1].bytes().all(|b| b.is_ascii_digit())
        && parts[2].len() == 20
        && parts[2].bytes().all(|b| b.is_ascii_hexdigit())
}

fn reparse(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

/// Nonrecursive, opt-in pruning of downloader-named regular files only.
/// Timestamp means filesystem modification time; name breaks ties deterministically.
pub fn cleanup_downloads(root: &Path, max_images: usize) -> Result<usize> {
    if max_images == 0 {
        return Ok(0);
    }
    let metadata = match fs::symlink_metadata(root) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e.into()),
    };
    if !metadata.is_dir() || reparse(&metadata) || metadata.file_type().is_symlink() {
        return Err(Error::Config(
            "cleanup directory must be a regular directory, not a link/junction",
        ));
    }
    let mut candidates = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if !entry.file_name().to_str().is_some_and(managed_image) {
            continue;
        }
        let metadata = fs::symlink_metadata(entry.path())?;
        if !metadata.is_file() || reparse(&metadata) || metadata.file_type().is_symlink() {
            continue;
        }
        candidates.push((metadata.modified()?, entry.path()));
    }
    candidates.sort();
    let excess = candidates.len().saturating_sub(max_images);
    let mut removed = 0;
    for (modified, path) in candidates.into_iter().take(excess) {
        // Recheck before removing: skip an entry replaced or changed since enumeration.
        let metadata = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e.into()),
        };
        if metadata.is_file()
            && !metadata.file_type().is_symlink()
            && !reparse(&metadata)
            && metadata.modified()? == modified
        {
            fs::remove_file(&path)?;
            removed += 1;
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_file_is_cached_and_folder_choice_does_not_repeat() {
        let root = std::env::temp_dir().join(format!("iidxbg-media-{:016x}", fastrand::u64(..)));
        fs::create_dir(&root).unwrap();
        for name in ["a.png", "b.png"] {
            image::RgbaImage::from_pixel(8, 8, image::Rgba([20, 40, 60, 255]))
                .save(root.join(name))
                .unwrap();
        }
        let mut config = Config::parse(include_str!("../tests/fixtures/default.toml"))
            .unwrap()
            .for_surface(Surface::Results);
        config.image.width = 64;
        config.image.height = 64;
        config.source = Source::File;
        config.media_path = root.join("a.png");
        config.local_directory = root.clone();
        let mut provider = Provider::new(config.clone()).unwrap();
        let a = provider.prepare_media().unwrap();
        let b = provider.prepare_media().unwrap();
        assert!(Arc::ptr_eq(&a.first(), &b.first()));
        config.source = Source::Local;
        let mut provider = Provider::new(config).unwrap();
        let a = provider.prepare_media().unwrap();
        let b = provider.prepare_media().unwrap();
        assert_ne!(a.label, b.label);
        fs::remove_file(root.join("a.png")).unwrap();
        fs::remove_file(root.join("b.png")).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn compatible_api_fetches_relative_images_and_uses_its_own_cache_namespace() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/post.json", server.local_addr().unwrap());
        server.set_nonblocking(true).unwrap();
        let mut png = std::io::Cursor::new(Vec::new());
        image::RgbaImage::from_pixel(8, 8, image::Rgba([20, 40, 60, 255]))
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let handle = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + Duration::from_secs(8);
            let mut requests = 0;
            while requests < 2 && std::time::Instant::now() < deadline {
                let Ok((mut stream, _)) = server.accept() else {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut request = [0; 4096];
                let count = stream.read(&mut request).unwrap();
                let request = String::from_utf8_lossy(&request[..count]);
                let body = if request.starts_with("GET /post.json?") {
                    assert!(request.contains("tags=rating%3Asafe"));
                    br#"[{"id":42,"sample_url":"/image.png"}]"#.to_vec()
                } else {
                    assert!(request.starts_with("GET /image.png "));
                    png.get_ref().clone()
                };
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .unwrap();
                stream.write_all(&body).unwrap();
                requests += 1;
            }
            requests
        });
        let root = std::env::temp_dir().join(format!("iidxbg-api-{:016x}", fastrand::u64(..)));
        let mut config = Config::parse(include_str!("../tests/fixtures/default.toml"))
            .unwrap()
            .for_surface(Surface::Results);
        config.source = Source::Remote;
        config.image.width = 64;
        config.image.height = 64;
        config.moebooru.api_url = endpoint;
        config.moebooru.page_max = 1;
        config.moebooru.save_directory = root.clone();
        let label = config.moebooru.post_label(42);
        let mut provider = Provider::new(config).unwrap();
        let result = provider.prepare();
        assert_eq!(handle.join().unwrap(), 2);
        assert_eq!(result.unwrap().label, label);
        assert_eq!(cleanup_downloads(&root, 1).unwrap(), 0);
        let entries = fs::read_dir(&root)
            .unwrap()
            .collect::<std::io::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(entries.len(), 1);
        assert!(managed_image(entries[0].file_name().to_str().unwrap()));
        fs::remove_file(entries[0].path()).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn separate_surface_sources_reload_atomically_and_fail_independently() {
        let root = std::env::temp_dir().join(format!("iidxbg-sources-{:016x}", fastrand::u64(..)));
        for surface in Surface::ALL {
            let folder = root.join(surface.name());
            fs::create_dir_all(&folder).unwrap();
            image::RgbImage::from_pixel(64, 64, image::Rgb([20 * surface.index() as u8, 90, 200]))
                .save(folder.join("image.png"))
                .unwrap();
        }
        let path = root.join("iidxbg.toml");
        let mut text = include_str!("../tests/fixtures/default.toml")
            .split("\n[sources.results]")
            .next()
            .unwrap()
            .to_owned();
        for surface in Surface::ALL {
            text.push_str(&format!(
                "\n[sources.{}]\nlocal_directory = '{}'\n",
                surface.name(),
                surface.name()
            ));
        }
        text = text
            .replace("song_select = false", "song_select = true")
            .replace("sudden_plus = false", "sudden_plus = true")
            .replace("lift = false", "lift = true")
            .replace("target_graph = false", "target_graph = true")
            .replace("premium_area = false", "premium_area = true")
            .replace("concentration = false", "concentration = true");
        fs::write(&path, &text).unwrap();
        let mut bank = Providers::new(Config::load(&path).unwrap()).unwrap();
        for surface in Surface::ALL {
            let p = bank.prepare(surface).unwrap().unwrap();
            assert_eq!(
                p.label,
                root.join(surface.name())
                    .join("image.png")
                    .to_string_lossy()
            );
            assert_eq!(p.bgra[2], 20 * surface.index() as u8);
        }
        let changed = text.replace("local_directory = 'lift'", "local_directory = 'missing'");
        fs::write(&path, &changed).unwrap();
        assert!(bank.reload(&path).unwrap());
        assert!(bank.prepare(Surface::Lift).is_err());
        assert!(bank.prepare(Surface::SuddenPlus).unwrap().is_some());
        let old = bank.config.clone();
        fs::write(
            &path,
            format!("{changed}\n[sources.song_select.moebooru]\ntimeout_seconds = 0"),
        )
        .unwrap();
        assert!(bank.reload(&path).is_err());
        assert_eq!(bank.config, old);
        assert!(
            bank.providers[Surface::SuddenPlus.index()]
                .previous
                .is_some()
        );
        fs::write(&path, text.replace("enabled = true", "enabled = false")).unwrap();
        bank.reload(&path).unwrap();
        for surface in Surface::ALL {
            assert!(bank.prepare(surface).unwrap().is_none());
        }
        for surface in Surface::ALL {
            fs::remove_file(root.join(surface.name()).join("image.png")).unwrap();
            fs::remove_dir(root.join(surface.name())).unwrap();
        }
        fs::remove_file(path).unwrap();
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn cleanup_is_opt_in_oldest_first_and_preserves_unmanaged_files() {
        let root = std::env::temp_dir().join(format!("iidxbg-cleanup-{:016x}", fastrand::u64(..)));
        fs::create_dir_all(root.join("nested")).unwrap();
        let mut names = Vec::new();
        for index in 0..5 {
            let domain = if index % 2 == 0 {
                "konachan"
            } else {
                "yandere"
            };
            let name = format!("{domain}-{index}-0123456789abcdef0123.png");
            let file = fs::File::create(root.join(&name)).unwrap();
            file.set_times(
                fs::FileTimes::new()
                    .set_modified(std::time::UNIX_EPOCH + Duration::from_secs(10000 + index)),
            )
            .unwrap();
            names.push(name);
        }
        for name in ["family.png", ".iidxbg-test.part", "konachan-1-badhash.jpg"] {
            fs::write(root.join(name), b"keep").unwrap();
        }
        fs::write(root.join("nested").join(&names[0]), b"keep").unwrap();
        assert_eq!(cleanup_downloads(&root, 0).unwrap(), 0);
        assert_eq!(cleanup_downloads(&root, 5).unwrap(), 0);
        assert_eq!(cleanup_downloads(&root, 2).unwrap(), 3);
        for (i, name) in names.iter().enumerate() {
            assert_eq!(root.join(name).exists(), i >= 3);
        }
        assert_eq!(cleanup_downloads(&root, 2).unwrap(), 0);
        for name in ["family.png", ".iidxbg-test.part", "konachan-1-badhash.jpg"] {
            assert_eq!(fs::read(root.join(name)).unwrap(), b"keep");
            fs::remove_file(root.join(name)).unwrap();
        }
        for name in &names[3..] {
            fs::remove_file(root.join(name)).unwrap();
        }
        assert!(root.join("nested").join(&names[0]).exists());
        fs::remove_file(root.join("nested").join(&names[0])).unwrap();
        fs::remove_dir(root.join("nested")).unwrap();
        fs::remove_dir(&root).unwrap();
        assert_eq!(cleanup_downloads(&root, 1).unwrap(), 0);
    }

    #[test]
    fn reload_applies_complete_config_and_preserves_last_valid_snapshot() {
        let root = std::env::temp_dir().join(format!("iidxbg-reload-{:016x}", fastrand::u64(..)));
        fs::create_dir_all(root.join("first")).unwrap();
        fs::create_dir_all(root.join("second")).unwrap();
        for (folder, color) in [("first", [255, 0, 0]), ("second", [0, 0, 255])] {
            image::RgbImage::from_pixel(64, 64, image::Rgb(color))
                .save(root.join(folder).join("image.png"))
                .unwrap();
        }
        let path = root.join("iidxbg.toml");
        let initial = include_str!("../tests/fixtures/default.toml")
            .replace("local_directory = 'iidxbg'", "local_directory = 'first'")
            .replace("width = 1920", "width = 64")
            .replace("height = 1080", "height = 64");
        fs::write(&path, &initial).unwrap();
        let mut provider = Provider::new(Config::load(&path).unwrap()).unwrap();
        let first = provider.prepare().unwrap();
        assert_eq!(
            first.label,
            root.join("first").join("image.png").to_string_lossy()
        );
        assert!(!provider.reload(&path).unwrap());

        let changed = initial
            .replace("local_directory = 'first'", "local_directory = 'second'")
            .replace("width = 64", "width = 128")
            .replace("domain = \"konachan.net\"", "domain = \"yande.re\"")
            .replace("timeout_seconds = 20", "timeout_seconds = 5")
            .replace(
                "save_directory = 'iidxbg/downloads'",
                "save_directory = 'saved'",
            )
            .replace("tags = \"rating:safe\"", "tags = \"rating:safe landscape\"");
        fs::write(&path, &changed).unwrap();
        assert!(provider.reload(&path).unwrap());
        assert_eq!(provider.previous.as_deref(), Some(first.label.as_str()));
        assert_eq!(provider.config.moebooru.domain, MoebooruDomain::Yandere);
        assert_eq!(provider.config.moebooru.timeout_seconds, 5);
        assert_eq!(provider.config.moebooru.save_directory, root.join("saved"));
        assert_eq!(provider.config.moebooru.tags, "rating:safe landscape");
        let second = provider.prepare().unwrap();
        assert_eq!((second.width, second.height), (128, 64));
        assert_eq!(
            second.label,
            root.join("second").join("image.png").to_string_lossy()
        );
        assert_ne!(first.bgra[..4], second.bgra[..4]);

        let valid = provider.config.clone();
        for invalid in [
            "enabled = [".to_owned(),
            changed.replace("width = 128", "width = 0"),
        ] {
            fs::write(&path, invalid).unwrap();
            assert!(provider.reload(&path).is_err());
            assert_eq!(provider.config, valid);
            assert_eq!(provider.previous.as_deref(), Some(second.label.as_str()));
        }
        fs::remove_file(&path).unwrap();
        assert!(provider.reload(&path).is_err());
        assert_eq!(provider.config, valid);
        assert_eq!(provider.prepare().unwrap().width, 128);

        fs::write(&path, changed.replace("enabled = true", "enabled = false")).unwrap();
        assert!(provider.reload(&path).unwrap());
        assert!(!provider.enabled());
        fs::write(&path, changed).unwrap();
        assert!(provider.reload(&path).unwrap());
        assert!(provider.enabled());
        fs::remove_file(path).unwrap();
        for folder in ["first", "second"] {
            fs::remove_file(root.join(folder).join("image.png")).unwrap();
            fs::remove_dir(root.join(folder)).unwrap();
        }
        fs::remove_dir(root).unwrap();
    }

    #[test]
    fn image_url_boundary() {
        for url in [
            "https://konachan.net/sample/a.jpg",
            "https://cdn.konachan.net/a.png",
            "https://konachan.com/sample/a.jpg",
            "https://cdn.konachan.com/a.png",
        ] {
            for domain in [MoebooruDomain::Net, MoebooruDomain::Com] {
                assert!(allowed_url(domain, &Url::parse(url).unwrap()));
            }
            assert!(!allowed_url(
                MoebooruDomain::Yandere,
                &Url::parse(url).unwrap()
            ));
        }
        for url in [
            "https://yande.re/image/a.jpg",
            "https://files.yande.re/a.png",
        ] {
            assert!(allowed_url(
                MoebooruDomain::Yandere,
                &Url::parse(url).unwrap()
            ));
            for domain in [MoebooruDomain::Net, MoebooruDomain::Com] {
                assert!(!allowed_url(domain, &Url::parse(url).unwrap()));
            }
        }
        for url in [
            "http://konachan.net/a",
            "https://konachan.net.evil.invalid/a",
            "https://localhost/a",
            "https://user@konachan.net/a",
            "https://konachan.net:444/a",
            "http://konachan.com/a",
            "https://konachan.com.evil.invalid/a",
            "https://user@konachan.com/a",
            "https://konachan.com:444/a",
            "http://yande.re/a",
            "https://yande.re.evil.invalid/a",
            "https://evilyande.re/a",
            "https://user@yande.re/a",
            "https://yande.re:444/a",
        ] {
            for domain in [
                MoebooruDomain::Net,
                MoebooruDomain::Com,
                MoebooruDomain::Yandere,
            ] {
                assert!(!allowed_url(domain, &Url::parse(url).unwrap()));
            }
        }
    }

    #[test]
    fn provider_ids_do_not_collide() {
        assert_eq!(
            MoebooruDomain::Net.post_label(42),
            MoebooruDomain::Com.post_label(42)
        );
        assert_ne!(
            MoebooruDomain::Net.post_label(42),
            MoebooruDomain::Yandere.post_label(42)
        );
        assert_ne!(
            MoebooruDomain::Net.namespace(),
            MoebooruDomain::Yandere.namespace()
        );
    }

    #[test]
    fn atomic_save_preserves_existing_file() {
        let root = std::env::temp_dir().join(format!("iidxbg-{:016x}", fastrand::u64(..)));
        save(&root, "image.jpg", b"original").unwrap();
        save(&root, "image.jpg", b"replacement").unwrap();
        assert_eq!(fs::read(root.join("image.jpg")).unwrap(), b"original");
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        fs::remove_file(root.join("image.jpg")).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
