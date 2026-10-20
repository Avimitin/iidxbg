use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Local,
    Konachan,
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
    pub konachan: Konachan,
    pub image: Image,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Konachan {
    pub tags: String,
    pub save_directory: PathBuf,
    pub page_max: u32,
    pub timeout_seconds: u64,
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
        if value.konachan.save_directory.is_relative() {
            value.konachan.save_directory = root.join(&value.konachan.save_directory);
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
        if !(1..=100).contains(&self.konachan.page_max)
            || !(3..=60).contains(&self.konachan.timeout_seconds)
            || self.konachan.tags.len() > 1000
            || self.local_directory.as_os_str().is_empty()
            || self.konachan.save_directory.as_os_str().is_empty()
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
            c.konachan.save_directory,
            Path::new(env!("CARGO_MANIFEST_DIR")).join("downloads/konachan")
        );
    }
}
