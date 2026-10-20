use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Local,
    #[serde(alias = "konachan")]
    Remote,
    Mixed,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Fit {
    Cover,
    Contain,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub enabled: bool,
    pub source: Source,
    pub local_directory: PathBuf,
    pub recursive: bool,
    #[serde(alias = "konachan")]
    pub remote: Remote,
    pub image: Image,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Remote {
    #[serde(default)]
    pub domain: RemoteDomain,
    pub tags: String,
    pub save_directory: PathBuf,
    pub page_max: u32,
    pub timeout_seconds: u64,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
pub enum RemoteDomain {
    #[default]
    #[serde(rename = "konachan.net")]
    Net,
    #[serde(rename = "konachan.com")]
    Com,
    #[serde(rename = "yande.re")]
    Yandere,
}

impl RemoteDomain {
    pub fn posts_url(self) -> &'static str {
        match self {
            Self::Net => "https://konachan.net/post.json",
            Self::Com => "https://konachan.com/post.json",
            Self::Yandere => "https://yande.re/post.json",
        }
    }

    pub fn namespace(self) -> &'static str {
        match self {
            Self::Net | Self::Com => "konachan",
            Self::Yandere => "yandere",
        }
    }

    pub fn post_label(self, id: u64) -> String {
        format!("{}:{id}", self.namespace())
    }

    pub fn allows_host(self, host: &str) -> bool {
        let domains: &[&str] = match self {
            Self::Net | Self::Com => &["konachan.net", "konachan.com"],
            Self::Yandere => &["yande.re"],
        };
        domains.iter().any(|domain| {
            host == *domain
                || host
                    .strip_suffix(domain)
                    .is_some_and(|prefix| prefix.ends_with('.'))
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub fit: Fit,
    pub max_download_mb: u64,
    pub max_source_pixels: u64,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let mut value: Self = toml::from_str(&std::fs::read_to_string(path)?)?;
        value.validate()?;
        let root = path.parent().unwrap_or(Path::new("."));
        if value.local_directory.is_relative() {
            value.local_directory = root.join(&value.local_directory);
        }
        if value.remote.save_directory.is_relative() {
            value.remote.save_directory = root.join(&value.remote.save_directory);
        }
        Ok(value)
    }

    pub fn validate(&self) -> Result<()> {
        if !(64..=3840).contains(&self.image.width)
            || !(64..=2160).contains(&self.image.height)
            || !(1..=64).contains(&self.image.max_download_mb)
            || !(1..=64_000_000).contains(&self.image.max_source_pixels)
        {
            return Err(Error::Config("image dimensions/limits out of range"));
        }
        if !(1..=100).contains(&self.remote.page_max)
            || !(3..=60).contains(&self.remote.timeout_seconds)
            || self.remote.tags.len() > 1000
            || self.local_directory.as_os_str().is_empty()
            || self.remote.save_directory.as_os_str().is_empty()
        {
            return Err(Error::Config("invalid source settings"));
        }
        Ok(())
    }

    pub fn byte_limit(&self) -> u64 {
        self.image.max_download_mb * 1024 * 1024
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_domain_selection_and_existing_config() {
        let shipped = include_str!("../iidxbg.toml");
        for (domain, endpoint) in [
            ("konachan.net", "https://konachan.net/post.json"),
            ("konachan.com", "https://konachan.com/post.json"),
            ("yande.re", "https://yande.re/post.json"),
        ] {
            let c: Config = toml::from_str(&shipped.replace(
                "domain = \"konachan.net\"",
                &format!("domain = \"{domain}\""),
            ))
            .unwrap();
            c.validate().unwrap();
            assert_eq!(c.remote.domain.posts_url(), endpoint);
        }
        let old = shipped.replace("domain = \"konachan.net\"", "");
        let c: Config = toml::from_str(&old).unwrap();
        assert_eq!(c.remote.domain, RemoteDomain::Net);
        let legacy = old
            .replace("[remote]", "[konachan]")
            .replace("source = \"local\"", "source = \"konachan\"");
        let c: Config = toml::from_str(&legacy).unwrap();
        c.validate().unwrap();
        assert_eq!(c.source, Source::Remote);
        assert_eq!(c.remote.domain, RemoteDomain::Net);
        assert!(toml::from_str::<Config>(&format!("{shipped}\n[konachan]\n")).is_err());
        for invalid in [
            "example.com",
            "https://konachan.com",
            "konachan.com.evil.invalid",
            "yande.re.evil.invalid",
        ] {
            assert!(
                toml::from_str::<Config>(&shipped.replace(
                    "domain = \"konachan.net\"",
                    &format!("domain = \"{invalid}\"")
                ))
                .is_err()
            );
        }
    }

    #[test]
    fn shipped_config_and_strict_limits() {
        let mut c: Config = toml::from_str(include_str!("../iidxbg.toml")).unwrap();
        c.validate().unwrap();
        c.image.width = 0;
        assert!(c.validate().is_err());
        assert!(
            toml::from_str::<Config>(
                &include_str!("../iidxbg.toml").replace("\"local\"", "\"typo\"")
            )
            .is_err()
        );
    }

    #[test]
    fn toml_comments_paths_and_errors() {
        let shipped = include_str!("../iidxbg.toml");
        let text = shipped.replace(
            "local_directory = 'images'",
            r"local_directory = 'D:\Pictures\背景' # literal Windows path",
        );
        let c: Config = toml::from_str(&text).unwrap();
        assert_eq!(c.local_directory, PathBuf::from(r"D:\Pictures\背景"));
        for invalid in [
            format!("unknown = true\n{shipped}"),
            shipped.replace("[image]", "[image]\nunknown = true"),
            shipped.replace("enabled = true", "enabled = \"true\""),
            shipped.replace("enabled = true", "enabled = true\nenabled = false"),
            shipped.replace("enabled = true", "enabled = ["),
        ] {
            assert!(toml::from_str::<Config>(&invalid).is_err());
        }
        let c = Config::load(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("iidxbg.toml")
                .as_path(),
        )
        .unwrap();
        assert_eq!(
            c.local_directory,
            Path::new(env!("CARGO_MANIFEST_DIR")).join("images")
        );
        assert_eq!(
            c.remote.save_directory,
            Path::new(env!("CARGO_MANIFEST_DIR")).join("downloads/remote")
        );
    }
}
