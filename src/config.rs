use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Local,
    Remote,
    Mixed,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Fit {
    Cover,
    Contain,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub enabled: bool,
    pub source: Source,
    pub local_directory: PathBuf,
    pub recursive: bool,
    pub moebooru: Moebooru,
    pub image: Image,
    #[serde(default)]
    pub surfaces: Surfaces,
    #[serde(default)]
    pub sources: Sources,
    #[serde(default)]
    pub selection: SelectionOptions,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct SelectionOptions {
    pub frame_chara: bool,
    pub frame_chara_image: PathBuf,
}

impl Default for SelectionOptions {
    fn default() -> Self {
        Self {
            frame_chara: true,
            frame_chara_image: PathBuf::new(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Surface {
    Results,
    SongSelect,
    SuddenPlus,
    Lift,
    TargetGraph,
}

impl Surface {
    pub const ALL: [Self; 5] = [
        Self::Results,
        Self::SongSelect,
        Self::SuddenPlus,
        Self::Lift,
        Self::TargetGraph,
    ];
    pub fn index(self) -> usize {
        self as usize
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Results => "results",
            Self::SongSelect => "song_select",
            Self::SuddenPlus => "sudden_plus",
            Self::Lift => "lift",
            Self::TargetGraph => "target_graph",
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Sources {
    pub results: SourceOverride,
    pub song_select: SourceOverride,
    pub sudden_plus: SourceOverride,
    pub lift: SourceOverride,
    pub target_graph: SourceOverride,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct SourceOverride {
    pub source: Option<Source>,
    pub local_directory: Option<PathBuf>,
    pub recursive: Option<bool>,
    pub fit: Option<Fit>,
    pub moebooru: MoebooruOverride,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct MoebooruOverride {
    pub domain: Option<MoebooruDomain>,
    pub tags: Option<String>,
    pub save_directory: Option<PathBuf>,
    pub page_max: Option<u32>,
    pub timeout_seconds: Option<u64>,
    pub max_images: Option<usize>,
}

/// New surfaces are opt-in; existing configurations retain results-only behavior.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Surfaces {
    pub song_select: bool,
    pub sudden_plus: bool,
    pub lift: bool,
    pub target_graph: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Moebooru {
    #[serde(default)]
    pub domain: MoebooruDomain,
    pub tags: String,
    pub save_directory: PathBuf,
    pub page_max: u32,
    pub timeout_seconds: u64,
    #[serde(default)]
    pub max_images: usize,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
pub enum MoebooruDomain {
    #[default]
    #[serde(rename = "konachan.net")]
    Net,
    #[serde(rename = "konachan.com")]
    Com,
    #[serde(rename = "yande.re")]
    Yandere,
}

impl MoebooruDomain {
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

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub fit: Fit,
    pub max_download_mb: u64,
    pub max_source_pixels: u64,
}

impl Config {
    pub fn parse(text: &str) -> Result<Self> {
        let value: Self = toml::from_str(text)?;
        value.validate()?;
        Ok(value)
    }

    pub fn load(path: &Path) -> Result<Self> {
        let mut value = Self::parse(&std::fs::read_to_string(path)?)?;
        let root = path.parent().unwrap_or(Path::new("."));
        if value.local_directory.is_relative() {
            value.local_directory = root.join(&value.local_directory);
        }
        if value.moebooru.save_directory.is_relative() {
            value.moebooru.save_directory = root.join(&value.moebooru.save_directory);
        }
        for source in [
            &mut value.sources.results,
            &mut value.sources.song_select,
            &mut value.sources.sudden_plus,
            &mut value.sources.lift,
            &mut value.sources.target_graph,
        ] {
            for path in [
                &mut source.local_directory,
                &mut source.moebooru.save_directory,
            ]
            .into_iter()
            .flatten()
            {
                if path.is_relative() {
                    *path = root.join(&*path);
                }
            }
        }
        Ok(value)
    }

    pub fn hide_frame_chara(&self) -> bool {
        self.enabled && !self.selection.frame_chara
    }

    pub fn validate(&self) -> Result<()> {
        self.validate_base()?;
        for surface in Surface::ALL {
            self.for_surface(surface).validate_base()?;
        }
        Ok(())
    }

    pub fn for_surface(&self, surface: Surface) -> Self {
        let mut c = self.clone();
        let o = match surface {
            Surface::Results => &self.sources.results,
            Surface::SongSelect => &self.sources.song_select,
            Surface::SuddenPlus => &self.sources.sudden_plus,
            Surface::Lift => &self.sources.lift,
            Surface::TargetGraph => &self.sources.target_graph,
        };
        c.sources = Sources::default();
        c.enabled &= match surface {
            Surface::Results => true,
            Surface::SongSelect => self.surfaces.song_select,
            Surface::SuddenPlus => self.surfaces.sudden_plus,
            Surface::Lift => self.surfaces.lift,
            Surface::TargetGraph => self.surfaces.target_graph,
        };
        if let Some(v) = o.source {
            c.source = v;
        }
        if let Some(v) = &o.local_directory {
            c.local_directory = v.clone();
        }
        if let Some(v) = o.recursive {
            c.recursive = v;
        }
        if let Some(v) = o.fit {
            c.image.fit = v;
        }
        if let Some(v) = o.moebooru.domain {
            c.moebooru.domain = v;
        }
        if let Some(v) = &o.moebooru.tags {
            c.moebooru.tags = v.clone();
        }
        if let Some(v) = &o.moebooru.save_directory {
            c.moebooru.save_directory = v.clone();
        }
        if let Some(v) = o.moebooru.page_max {
            c.moebooru.page_max = v;
        }
        if let Some(v) = o.moebooru.timeout_seconds {
            c.moebooru.timeout_seconds = v;
        }
        if let Some(v) = o.moebooru.max_images {
            c.moebooru.max_images = v;
        }
        match surface {
            Surface::Results => {}
            Surface::SongSelect => {
                c.image.width = 1920;
                c.image.height = 1080;
            }
            Surface::SuddenPlus | Surface::Lift => {
                c.image.width = 432;
                c.image.height = 723;
            }
            Surface::TargetGraph => {
                c.image.width = 432;
                c.image.height = 870;
            }
        }
        c
    }

    fn validate_base(&self) -> Result<()> {
        if !(64..=3840).contains(&self.image.width)
            || !(64..=2160).contains(&self.image.height)
            || !(1..=64).contains(&self.image.max_download_mb)
            || !(1..=64_000_000).contains(&self.image.max_source_pixels)
        {
            return Err(Error::Config("image dimensions/limits out of range"));
        }
        if !(1..=100).contains(&self.moebooru.page_max)
            || !(3..=60).contains(&self.moebooru.timeout_seconds)
            || self.moebooru.tags.len() > 1000
            || self.local_directory.as_os_str().is_empty()
            || self.moebooru.save_directory.as_os_str().is_empty()
            || self.moebooru.max_images > 1_000_000
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
    fn selection_character_defaults_and_strict_live_settings() {
        let shipped = include_str!("../iidxbg.toml");
        let c = Config::parse(shipped).unwrap();
        assert!(!c.hide_frame_chara());
        assert!(c.selection.frame_chara_image.as_os_str().is_empty());
        let mut hidden =
            Config::parse(&shipped.replace("frame_chara = true", "frame_chara = false")).unwrap();
        assert!(hidden.hide_frame_chara());
        hidden.enabled = false;
        assert!(!hidden.hide_frame_chara());
        let mut without: toml::Value = toml::from_str(shipped).unwrap();
        without.as_table_mut().unwrap().remove("selection");
        let without: Config = without.try_into().unwrap();
        assert!(without.selection.frame_chara);
        for text in [
            shipped.replace("frame_chara = true", "frame_chara = 0"),
            shipped.replace("frame_chara = true", "frame_character = false"),
            shipped.replace("frame_chara_image = ''", "frame_chara_image = true"),
        ] {
            assert!(Config::parse(&text).is_err());
        }
    }

    #[test]
    fn surface_overrides_inherit_validate_and_resolve_paths() {
        let text = format!(
            "{}\n[sources.song_select]\nsource = 'remote'\nfit = 'contain'\n\
            [sources.song_select.moebooru]\ndomain = 'yande.re'\nsave_directory = 'selection-cache'\nmax_images = 25\n\
            [sources.lift]\nlocal_directory = 'lower'\n",
            include_str!("../iidxbg.toml")
                .split("[sources.results]")
                .next()
                .unwrap()
        );
        let c: Config = toml::from_str(&text).unwrap();
        c.validate().unwrap();
        let selection = c.for_surface(Surface::SongSelect);
        assert_eq!(selection.source, Source::Remote);
        assert_eq!(selection.moebooru.domain, MoebooruDomain::Yandere);
        assert_eq!(selection.moebooru.max_images, 25);
        assert_eq!(selection.moebooru.tags, c.moebooru.tags);
        assert_eq!(selection.image.fit, Fit::Contain);
        assert_eq!(
            c.for_surface(Surface::Lift).local_directory,
            PathBuf::from("lower")
        );
        assert_eq!(
            c.for_surface(Surface::Results).moebooru.domain,
            MoebooruDomain::Net
        );
        for invalid in [
            text.replace("max_images = 25", "max_images = 1000001"),
            text.replace("local_directory = 'lower'", "local_directory = ''"),
        ] {
            assert!(
                toml::from_str::<Config>(&invalid)
                    .unwrap()
                    .validate()
                    .is_err()
            );
        }
        assert!(
            toml::from_str::<Config>(&text.replace("fit = 'contain'", "fits = 'contain'")).is_err()
        );
    }
    #[test]
    fn surfaces_are_independent_opt_ins_and_old_configs_still_load() {
        let shipped = include_str!("../iidxbg.toml");
        let old = shipped.split("[surfaces]").next().unwrap();
        assert_eq!(
            toml::from_str::<Config>(old).unwrap().surfaces,
            Surfaces::default()
        );
        let mut config: Config =
            toml::from_str(&shipped.replace("song_select = false", "song_select = true")).unwrap();
        assert!(config.surfaces.song_select);
        assert!(!config.surfaces.sudden_plus);
        assert!(!config.surfaces.lift);
        assert!(!config.surfaces.target_graph);
        config.enabled = false;
        config.validate().unwrap();
        assert!(
            toml::from_str::<Config>(
                &shipped.replace("target_graph = false", "target_graph = \"false\"")
            )
            .is_err()
        );
        assert!(toml::from_str::<Config>(&format!("{shipped}\nunknown_surface = true")).is_err());
    }

    #[test]
    fn moebooru_domains_and_rejection_of_removed_config_names() {
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
            assert_eq!(c.moebooru.domain.posts_url(), endpoint);
        }
        let old = shipped.replace("domain = \"konachan.net\"", "");
        let c: Config = toml::from_str(&old).unwrap();
        assert_eq!(c.moebooru.domain, MoebooruDomain::Net);
        for section in ["remote", "konachan"] {
            assert!(
                toml::from_str::<Config>(&shipped.replace("[moebooru]", &format!("[{section}]")))
                    .is_err()
            );
            assert!(
                toml::from_str::<Config>(&format!(
                    "{shipped}\n[sources.lift.{section}]\ndomain = 'yande.re'\n"
                ))
                .is_err()
            );
        }
        assert!(
            toml::from_str::<Config>(
                &shipped.replace("source = \"local\"", "source = \"konachan\"")
            )
            .is_err()
        );
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
        for (surface, folder) in [
            (Surface::Results, "results"),
            (Surface::SongSelect, "song-select"),
            (Surface::SuddenPlus, "sudden-plus"),
            (Surface::Lift, "lift"),
            (Surface::TargetGraph, "target-graph"),
        ] {
            let resolved = c.for_surface(surface);
            assert_eq!(resolved.source, Source::Local);
            assert_eq!(
                resolved.local_directory,
                PathBuf::from(format!("iidxbg/{folder}"))
            );
            assert_eq!(
                resolved.moebooru.save_directory,
                PathBuf::from(format!("iidxbg/downloads/{folder}"))
            );
        }
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
            "local_directory = 'iidxbg'",
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
            Path::new(env!("CARGO_MANIFEST_DIR")).join("iidxbg")
        );
        assert_eq!(
            c.moebooru.save_directory,
            Path::new(env!("CARGO_MANIFEST_DIR")).join("iidxbg/downloads")
        );
    }
}
