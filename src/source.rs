use crate::{
    Error, Result,
    config::{Config, MoebooruDomain, Source, Surface},
    pixels::{self, Prepared},
};
use reqwest::{Url, blocking::Client, redirect::Policy};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
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
        let domain = config.moebooru.domain;
        let client = Client::builder()
            .user_agent(concat!(
                "iidxbg/",
                env!("CARGO_PKG_VERSION"),
                " (results-background plugin)"
            ))
            .timeout(Duration::from_secs(config.moebooru.timeout_seconds))
            .connect_timeout(Duration::from_secs(5))
            .https_only(true)
            .redirect(Policy::custom(move |attempt| {
                if attempt.previous().len() >= 3 || !allowed_url(domain, attempt.url()) {
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
        let result = match self.config.source {
            Source::Local => self.local(),
            Source::Remote => self.remote(),
            Source::Mixed if fastrand::bool() => self.remote().or_else(|_| self.local()),
            Source::Mixed => self.local().or_else(|_| self.remote()),
        }?;
        self.previous = Some(result.label.clone());
        Ok(result)
    }

    fn local(&self) -> Result<Prepared> {
        let mut paths = Vec::new();
        collect(
            &self.config.local_directory,
            self.config.recursive,
            0,
            &mut paths,
        )?;
        fastrand::shuffle(&mut paths);
        // Prefer another image, but permit a singleton library.
        paths.sort_by_key(|p| self.previous.as_deref() == Some(&p.to_string_lossy()));
        for path in paths.into_iter().take(32) {
            let attempt = (|| {
                let file = fs::File::open(&path)?;
                let bytes = pixels::read_bounded(file, self.config.byte_limit())?;
                pixels::decode(
                    &bytes,
                    &self.config.image,
                    path.to_string_lossy().into_owned(),
                )
            })();
            if let Ok(image) = attempt {
                return Ok(image);
            }
        }
        Err(Error::NoImages)
    }

    fn remote(&self) -> Result<Prepared> {
        let page = fastrand::u32(1..=self.config.moebooru.page_max);
        let mut posts = self.posts(page)?;
        if posts.is_empty() && page != 1 {
            posts = self.posts(1)?;
        }
        fastrand::shuffle(&mut posts);
        posts.sort_by_key(|p| {
            self.previous.as_deref() == Some(&self.config.moebooru.domain.post_label(p.id))
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
            .get(self.config.moebooru.domain.posts_url())
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
        let url = Url::parse(text).map_err(|_| Error::Url)?;
        if !allowed_url(self.config.moebooru.domain, &url) {
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
            self.config.moebooru.domain.post_label(post.id),
        )?;
        let extension = match image::guess_format(&bytes)? {
            image::ImageFormat::Png => "png",
            image::ImageFormat::Jpeg => "jpg",
            _ => return Err(Error::Format),
        };
        let hash = hex::encode(Sha256::digest(&bytes));
        let name = format!(
            "{}-{}-{}.{}",
            self.config.moebooru.domain.namespace(),
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
                ["jpg", "jpeg", "png"]
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
        && matches!(parts[0], "konachan" | "yandere")
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
        let mut text = include_str!("../iidxbg.toml")
            .split("[sources.results]")
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
        let initial = include_str!("../iidxbg.toml")
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
