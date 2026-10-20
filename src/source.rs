use crate::{
    Error, Result,
    config::{Config, Source},
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

pub fn allowed_url(url: &Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port_or_known_default() == Some(443)
        && url.host_str().is_some_and(|h| {
            h == "konachan.net"
                || h == "konachan.com"
                || h.ends_with(".konachan.net")
                || h.ends_with(".konachan.com")
        })
}

pub struct Provider {
    config: Config,
    client: Client,
    previous: Option<String>,
}

impl Provider {
    pub fn new(config: Config) -> Result<Self> {
        let client = Client::builder()
            .user_agent("iidxbg/0.1 (results-background plugin)")
            .timeout(Duration::from_secs(config.konachan.timeout_seconds))
            .connect_timeout(Duration::from_secs(5))
            .https_only(true)
            .redirect(Policy::custom(|attempt| {
                if attempt.previous().len() >= 3 || !allowed_url(attempt.url()) {
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

    pub fn prepare(&mut self) -> Result<Prepared> {
        let result = match self.config.source {
            Source::Local => self.local(),
            Source::Konachan => self.remote(),
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
        let page = fastrand::u32(1..=self.config.konachan.page_max);
        let mut posts = self.posts(page)?;
        if posts.is_empty() && page != 1 {
            posts = self.posts(1)?;
        }
        fastrand::shuffle(&mut posts);
        posts.sort_by_key(|p| self.previous.as_deref() == Some(&format!("konachan:{}", p.id)));
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
            .get("https://konachan.net/post.json")
            .query(&[
                ("limit", "100"),
                ("page", &page.to_string()),
                ("tags", self.config.konachan.tags.as_str()),
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
        if !allowed_url(&url) {
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
        let prepared = pixels::decode(&bytes, &self.config.image, format!("konachan:{}", post.id))?;
        let extension = match image::guess_format(&bytes)? {
            image::ImageFormat::Png => "png",
            image::ImageFormat::Jpeg => "jpg",
            _ => return Err(Error::Format),
        };
        let hash = hex::encode(Sha256::digest(&bytes));
        let name = format!("{}-{}.{}", post.id, &hash[..20], extension);
        save(&self.config.konachan.save_directory, &name, &bytes)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_url_boundary() {
        for url in [
            "https://konachan.net/sample/a.jpg",
            "https://cdn.konachan.net/a.png",
        ] {
            assert!(allowed_url(&Url::parse(url).unwrap()));
        }
        for url in [
            "http://konachan.net/a",
            "https://konachan.net.evil.invalid/a",
            "https://localhost/a",
            "https://user@konachan.net/a",
            "https://konachan.net:444/a",
        ] {
            assert!(!allowed_url(&Url::parse(url).unwrap()));
        }
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
