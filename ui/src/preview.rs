//! Background preview preparation.
//!
//! The gallery must show the next image the plugin would use, not a static
//! mock. Preparation uses the plugin's own provider, decoder and fit policy on
//! a worker thread; only the finished BGRA pixels cross back to the UI thread.

use async_channel::{Receiver, Sender};
use gpui_kit::gpui::{RenderImage, SharedString};
use iidxbg::{
    config::{Config, Fit, MoebooruDomain, Source, Surface},
    pixels::Prepared,
    source::Provider,
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Clone, Debug, PartialEq)]
pub struct PreviewFingerprint {
    pub bilibili: iidxbg::bilibili::Settings,
    pub enabled: bool,
    pub target: Surface,
    pub source: Source,
    pub media_path: PathBuf,
    pub api_url: String,
    pub media_hosts: Vec<String>,
    pub local_directory: PathBuf,
    pub recursive: bool,
    pub fit: Fit,
    pub domain: MoebooruDomain,
    pub tags: String,
    pub save_directory: PathBuf,
    pub page_max: u32,
    pub timeout_seconds: u64,
    pub max_images: usize,
    pub width: u32,
    pub height: u32,
    pub max_download_mb: u64,
    pub max_source_pixels: u64,
}

impl PreviewFingerprint {
    /// Same provider inputs and output geometry, ignoring the effective enable
    /// state. Used to copy the premium-area image to concentration mode.
    pub fn same_source(&self, other: &Self) -> bool {
        self.target == other.target
            && self.bilibili == other.bilibili
            && self.source == other.source
            && self.media_path == other.media_path
            && self.api_url == other.api_url
            && self.media_hosts == other.media_hosts
            && self.local_directory == other.local_directory
            && self.recursive == other.recursive
            && self.fit == other.fit
            && self.domain == other.domain
            && self.tags == other.tags
            && self.save_directory == other.save_directory
            && self.page_max == other.page_max
            && self.timeout_seconds == other.timeout_seconds
            && self.max_images == other.max_images
            && self.width == other.width
            && self.height == other.height
            && self.max_download_mb == other.max_download_mb
            && self.max_source_pixels == other.max_source_pixels
    }
}

#[derive(Clone, Debug)]
pub struct PreviewSpec {
    pub surface: Surface,
    pub target: Surface,
    pub enabled: bool,
    pub fingerprint: PreviewFingerprint,
}

pub fn specs(config: &Config) -> [PreviewSpec; Surface::COUNT] {
    std::array::from_fn(|index| {
        let surface = Surface::ALL[index];
        let target = config.binding_root(surface).unwrap_or(surface);
        let resolved = config.for_surface(target);
        let enabled = config.surface_enabled(surface);
        PreviewSpec {
            surface,
            target,
            enabled,
            fingerprint: PreviewFingerprint {
                bilibili: resolved.bilibili,
                enabled,
                target,
                source: resolved.source,
                media_path: resolved.media_path,
                api_url: resolved.moebooru.api_url,
                media_hosts: resolved.moebooru.media_hosts,
                local_directory: resolved.local_directory,
                recursive: resolved.recursive,
                fit: resolved.image.fit,
                domain: resolved.moebooru.domain,
                tags: resolved.moebooru.tags,
                save_directory: resolved.moebooru.save_directory,
                page_max: resolved.moebooru.page_max,
                timeout_seconds: resolved.moebooru.timeout_seconds,
                max_images: resolved.moebooru.max_images,
                width: resolved.image.width,
                height: resolved.image.height,
                max_download_mb: resolved.image.max_download_mb,
                max_source_pixels: resolved.image.max_source_pixels,
            },
        }
    })
}

#[derive(Debug)]
pub struct PreviewRequest {
    pub config: Config,
    pub surface: Surface,
    pub target: Surface,
    pub generation: u64,
}

#[derive(Debug)]
pub struct PreviewOutcome {
    pub surface: Surface,
    pub generation: u64,
    pub result: Result<Prepared, String>,
}

pub fn spawn_worker(
    config_path: PathBuf,
    requests: Receiver<PreviewRequest>,
    outcomes: Sender<PreviewOutcome>,
) {
    std::thread::Builder::new()
        .name("iidxbg-preview".into())
        .spawn(move || {
            while let Ok(request) = requests.recv_blocking() {
                let outcome = PreviewOutcome {
                    surface: request.surface,
                    generation: request.generation,
                    result: prepare(&config_path, request.target, request.config),
                };
                if outcomes.send_blocking(outcome).is_err() {
                    break;
                }
            }
        })
        .expect("start preview worker");
}

fn prepare(config_path: &Path, target: Surface, mut config: Config) -> Result<Prepared, String> {
    config.resolve_paths(config_path.parent().unwrap_or(Path::new(".")));
    if !config.enabled {
        return Err("全局总开关已关闭".into());
    }
    if target == Surface::SongSelect && config.uses_mmd_character() {
        let model = iidxbg::mmd::load(&config.mmd, config_path.parent().unwrap_or(Path::new(".")))
            .map_err(|e| describe(&e))?;
        return iidxbg::mmd::render::preview(model, 0., [0., 0.], 0.).map_err(|e| describe(&e));
    }
    let resolved = config.for_surface(target);
    if resolved.source == Source::Local && !resolved.local_directory.is_dir() {
        return Err(format!(
            "本地目录不存在：{}",
            resolved.local_directory.display()
        ));
    }
    let mut provider = Provider::new(resolved).map_err(|error| describe(&error))?;
    provider.prepare().map_err(|error| describe(&error))
}

fn artwork_config(config: &Config, index: usize) -> Result<Config, String> {
    let art = config
        .artwork
        .get(index)
        .ok_or("Artwork entry no longer exists")?;
    let mut resolved = config.for_artwork(art);
    // A preview is allowed before enabling a replacement in the game.
    resolved.enabled = true;
    Ok(resolved)
}

pub(crate) fn prepare_artwork(
    config_path: &Path,
    index: usize,
    mut config: Config,
) -> Result<Prepared, String> {
    config.resolve_paths(config_path.parent().unwrap_or(Path::new(".")));
    let mut provider =
        Provider::new(artwork_config(&config, index)?).map_err(|error| describe(&error))?;
    provider.prepare().map_err(|error| describe(&error))
}

fn describe(error: &iidxbg::Error) -> String {
    match error {
        iidxbg::Error::NoImages => "没有找到可用的 PNG/JPEG 图片".into(),
        iidxbg::Error::Limit => "图片超过配置的尺寸或体积限制".into(),
        iidxbg::Error::Format => "图片格式不受支持（仅 PNG/JPEG）".into(),
        iidxbg::Error::Http(error) => format!("网络请求失败：{error}"),
        iidxbg::Error::Io(error) => format!("读取失败：{error}"),
        iidxbg::Error::Config(message) => format!("配置无效：{message}"),
        iidxbg::Error::Url => "图源返回了不受支持的图片地址".into(),
        iidxbg::Error::Video(message) if message.contains("0xC00D5212") => {
            "Windows 缺少此视频所需的解码器（0xC00D5212）。请先将视频转换为 H.264 / AVC MP4，再选择转换后的文件；HEVC / H.265 视频需要额外的 Windows 解码器。修改文件名或扩展名不能转换编码。".into()
        }
        iidxbg::Error::Video(message) => format!("视频预览失败：{message}"),
        other => other.to_string(),
    }
}

pub fn render_image(prepared: Prepared) -> Arc<RenderImage> {
    let Prepared {
        width,
        height,
        bgra,
        ..
    } = prepared;
    // gpui's `RenderImage` stores BGRA bytes even though its `Frame` wrapper is
    // typed as `RgbaImage`; pass the prepared buffer through unchanged.
    let bgra = image::RgbaImage::from_raw(width, height, bgra)
        .unwrap_or_else(|| image::RgbaImage::new(1, 1));
    Arc::new(RenderImage::new(vec![image::Frame::new(bgra)]))
}

/// Build an original, quiet placeholder image for a card. It is transparent
/// except for a one-pixel neutral outline: the page itself stays black, and the
/// card overlays its icon/status in GPUI so it stays sharp at every scale.
pub fn default_image(width: u32, height: u32) -> Arc<RenderImage> {
    let width = width.clamp(2, 1280);
    let height = height.clamp(2, 1280);
    let rgba = image::RgbaImage::from_fn(width, height, |x, y| {
        let border = x == 0 || y == 0 || x + 1 == width || y + 1 == height;
        if border {
            image::Rgba([255, 255, 255, 22])
        } else {
            image::Rgba([0, 0, 0, 0])
        }
    });
    Arc::new(RenderImage::new(vec![image::Frame::new(rgba)]))
}

pub fn short_label(label: &str) -> SharedString {
    if label.contains(':') && !label.contains(['\\', '/']) {
        return label.to_owned().into();
    }
    let path = Path::new(label);
    if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
        return name.to_owned().into();
    }
    label.to_owned().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("testdata/minimal.toml");

    #[test]
    fn missing_decoder_explains_conversion_for_localized_windows_errors() {
        let error = iidxbg::Error::Video("未找到合适的转换 (0xC00D5212)".into());
        let message = describe(&error);
        assert!(message.contains("H.264 / AVC MP4"));
        assert!(message.contains("Windows 解码器"));
        assert!(message.contains("修改文件名或扩展名不能转换编码"));
        let unrelated = iidxbg::Error::Video("bad timestamp".into());
        assert_eq!(describe(&unrelated), "视频预览失败：bad timestamp");
    }

    #[test]
    fn render_image_keeps_bgra_bytes_for_gpui() {
        let prepared = Prepared {
            label: "solid-red".into(),
            width: 1,
            height: 1,
            bgra: vec![0, 0, 255, 255],
        };
        let rendered = render_image(prepared);
        assert_eq!(rendered.as_bytes(0), Some(&[0, 0, 255, 255][..]));
    }

    #[test]
    fn artwork_preview_uses_its_source_dimensions_and_explicit_binding() {
        let mut config = Config::parse(&format!("{FIXTURE}\n[[artwork]]\narchive='@file'\ntexture='assistant.png'\nnative_size=[128,150]\n[artwork.source]\nsource='file'\nmedia_path='portrait.png'\n")).unwrap();
        config.sources.transition.media_path = Some("unrelated.png".into());
        let resolved = artwork_config(&config, 0).unwrap();
        assert_eq!(resolved.media_path, PathBuf::from("portrait.png"));
        assert_eq!((resolved.image.width, resolved.image.height), (128, 150));
        assert!(resolved.image.resource);
        config.sources.decision.source = Some(Source::File);
        config.sources.decision.media_path = Some("bound.mp4".into());
        config.artwork[0].source.bind = Some(Surface::Decision);
        assert_eq!(
            artwork_config(&config, 0).unwrap().media_path,
            PathBuf::from("bound.mp4")
        );
        assert!(artwork_config(&config, 1).is_err());
    }

    #[test]
    fn surface_canvases_follow_the_plugin_resolution_rules() {
        let mut config = Config::parse(FIXTURE).unwrap();
        config.image.width = 1280;
        config.image.height = 1024;
        let specs = specs(&config);

        let results = &specs[Surface::Results.index()];
        assert!(results.enabled);
        assert_eq!(
            (results.fingerprint.width, results.fingerprint.height),
            (1280, 1024)
        );

        let song = &specs[Surface::SongSelect.index()];
        assert!(!song.enabled);
        assert_eq!(
            (song.fingerprint.width, song.fingerprint.height),
            (1920, 1080)
        );

        let sudden = &specs[Surface::SuddenPlus.index()];
        assert_eq!(
            (sudden.fingerprint.width, sudden.fingerprint.height),
            (432, 723)
        );

        let graph = &specs[Surface::TargetGraph.index()];
        assert_eq!(
            (graph.fingerprint.width, graph.fingerprint.height),
            (432, 870)
        );

        let premium = &specs[Surface::PremiumArea.index()];
        assert_eq!(
            (premium.fingerprint.width, premium.fingerprint.height),
            (1280, 720)
        );
    }

    #[test]
    fn concentration_reuses_premium_until_it_gets_an_override() {
        let mut config = Config::parse(FIXTURE).unwrap();
        config.surfaces.concentration = true;
        assert!(config.concentration_shares_premium());
        let shared = specs(&config);
        let concentration = &shared[Surface::Concentration.index()];
        assert_eq!(concentration.target, Surface::PremiumArea);
        assert!(concentration.enabled);
        assert!(
            concentration
                .fingerprint
                .same_source(&shared[Surface::PremiumArea.index()].fingerprint)
        );

        config.sources.concentration.source = Some(Source::Remote);
        assert!(!config.concentration_shares_premium());
        let independent = specs(&config);
        assert_eq!(
            independent[Surface::Concentration.index()].target,
            Surface::Concentration
        );
    }

    #[test]
    fn same_source_ignores_the_enable_flag() {
        let config = Config::parse(FIXTURE).unwrap();
        let specs = specs(&config);
        let mut premium = specs[Surface::PremiumArea.index()].fingerprint.clone();
        let mut concentration = specs[Surface::Concentration.index()].fingerprint.clone();
        premium.enabled = true;
        concentration.enabled = false;
        assert!(premium.same_source(&concentration));
        concentration.target = Surface::Concentration;
        assert!(!premium.same_source(&concentration));
    }
}
