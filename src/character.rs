//! Local character image refresh at worker-owned scene request boundaries.
use crate::{
    Result,
    config::Config,
    pixels::{self, Prepared},
};
use std::{
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

#[derive(PartialEq)]
struct Fingerprint {
    path: PathBuf,
    modified: SystemTime,
    len: u64,
    max_bytes: u64,
    max_pixels: u64,
}

pub enum Update {
    Unchanged,
    Native,
    Image(Prepared),
}

#[derive(Default)]
pub struct LocalImage {
    // None = not checked yet; Some(None) = native artwork.
    attempted: Option<Option<Fingerprint>>,
}

impl LocalImage {
    pub fn refresh(&mut self, config: &Config, root: &Path) -> Result<Update> {
        let path = &config.selection.frame_chara_image;
        if !config.enabled || path.as_os_str().is_empty() {
            let changed = self.attempted.as_ref() != Some(&None);
            self.attempted = Some(None);
            return Ok(if changed {
                Update::Native
            } else {
                Update::Unchanged
            });
        }
        let path = if path.is_absolute() {
            path.clone()
        } else {
            root.join(path)
        };
        let metadata = fs::metadata(&path)?;
        let fingerprint = Fingerprint {
            path,
            modified: metadata.modified()?,
            len: metadata.len(),
            max_bytes: config.byte_limit(),
            max_pixels: config.image.max_source_pixels,
        };
        if self.attempted.as_ref().and_then(Option::as_ref) == Some(&fingerprint) {
            return Ok(Update::Unchanged);
        }
        let load = || -> Result<Prepared> {
            let bytes =
                pixels::read_bounded(fs::File::open(&fingerprint.path)?, config.byte_limit())?;
            pixels::decode_character(
                &bytes,
                &config.image,
                fingerprint.path.to_string_lossy().into_owned(),
            )
        };
        let result = load();
        // A rejected image is retried when the file/path/limits change.
        self.attempted = Some(Some(fingerprint));
        result.map(Update::Image)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_path_file_reload_alpha_and_clear_without_network() {
        let root =
            std::env::temp_dir().join(format!("iidxbg-character-{:016x}", fastrand::u64(..)));
        fs::create_dir(&root).unwrap();
        let first = root.join("first.png");
        image::RgbaImage::from_pixel(1920, 1080, image::Rgba([10u8, 20, 30, 128]))
            .save(&first)
            .unwrap();
        let mut config = Config::parse(include_str!("../tests/fixtures/default.toml")).unwrap();
        config.selection.frame_chara_image = PathBuf::from("first.png");
        let mut loader = LocalImage::default();
        let Update::Image(prepared) = loader.refresh(&config, &root).unwrap() else {
            panic!("image")
        };
        assert_eq!((prepared.width, prepared.height), (1920, 1080));
        assert_eq!(&prepared.bgra[..4], &[30, 20, 10, 128]);
        assert!(matches!(
            loader.refresh(&config, &root).unwrap(),
            Update::Unchanged
        ));

        fs::write(&first, b"bad image").unwrap();
        assert!(loader.refresh(&config, &root).is_err());
        assert!(matches!(
            loader.refresh(&config, &root).unwrap(),
            Update::Unchanged
        ));
        image::RgbaImage::from_pixel(8, 8, image::Rgba([80u8, 40, 20, 255]))
            .save(&first)
            .unwrap();
        let Update::Image(prepared) = loader.refresh(&config, &root).unwrap() else {
            panic!("reload")
        };
        assert_eq!(prepared.bgra[3], 0); // Transparent contain margin, no blur.
        let center = (540 * 1920 + 960) * 4;
        assert_eq!(&prepared.bgra[center..center + 4], &[20, 40, 80, 255]);

        config.selection.frame_chara_image = root.join("missing.png");
        assert!(loader.refresh(&config, &root).is_err());
        config.selection.frame_chara_image = first.clone();
        config.enabled = false;
        assert!(matches!(
            loader.refresh(&config, &root).unwrap(),
            Update::Native
        ));
        config.enabled = true;
        assert!(matches!(
            loader.refresh(&config, &root).unwrap(),
            Update::Image(_)
        ));
        config.selection.frame_chara_image = PathBuf::new();
        assert!(matches!(
            loader.refresh(&config, &root).unwrap(),
            Update::Native
        ));
        fs::remove_file(first).unwrap();
        fs::remove_dir(root).unwrap();
    }
}
