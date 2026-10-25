use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Bilibili,
    File,
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
    #[serde(default)]
    pub media_path: PathBuf,
    pub recursive: bool,
    pub moebooru: Moebooru,
    #[serde(default)]
    pub bilibili: crate::bilibili::Settings,
    pub image: Image,
    #[serde(default)]
    pub surfaces: Surfaces,
    #[serde(default)]
    pub sources: Sources,
    #[serde(default)]
    pub selection: SelectionOptions,
    #[serde(default)]
    pub mmd: crate::mmd::Settings,
    /// Optional silent local video for ordinary song results. Empty uses images.
    #[serde(default)]
    pub results_video: PathBuf,
    #[serde(default)]
    pub artwork: Vec<Artwork>,
    #[serde(default)]
    pub profile_artwork: bool,
}

/// A named image within an AFP archive. Native geometry and timelines stay intact.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Artwork {
    pub archive: String,
    pub texture: String,
    /// Catalog dimensions bound decode/upload work to the native resource size.
    #[serde(default)]
    pub native_size: Option<[u32; 2]>,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub source: SourceOverride,
    /// Retain the native silhouette; disable for arbitrary transparent cutouts.
    #[serde(default = "default_mask")]
    pub native_mask: bool,
    /// Internal preset scene gate; never accepted from or written to TOML.
    #[serde(skip)]
    pub(crate) scene_scope: Option<Surface>,
    #[serde(skip)]
    pub(crate) bottom_crop: bool,
}
fn default_mask() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct SelectionOptions {
    pub frame_chara: bool,
    pub frame_chara_image: PathBuf,
    pub frame_chara_source: CharacterSource,
}

/// Auto preserves configurations written before the character/Qpro split.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CharacterSource {
    #[default]
    Auto,
    Stock,
    Image,
    Mmd,
}

impl Default for SelectionOptions {
    fn default() -> Self {
        Self {
            frame_chara: true,
            frame_chara_image: PathBuf::new(),
            frame_chara_source: CharacterSource::Auto,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Surface {
    Results,
    SongSelect,
    SuddenPlus,
    Lift,
    TargetGraph,
    PremiumArea,
    Concentration,
    NoteSkin,
    GameplayShell,
    Decision,
    Transition,
    Title,
    Login,
    ModeSelect,
    GameplayBottom,
    Logout,
}

impl Surface {
    pub const ALL: [Self; 16] = [
        Self::Results,
        Self::SongSelect,
        Self::SuddenPlus,
        Self::Lift,
        Self::TargetGraph,
        Self::PremiumArea,
        Self::Concentration,
        Self::NoteSkin,
        Self::GameplayShell,
        Self::Decision,
        Self::Transition,
        Self::Title,
        Self::Login,
        Self::ModeSelect,
        Self::GameplayBottom,
        Self::Logout,
    ];
    pub const COUNT: usize = Self::ALL.len();
    /// Native canvas of the IIDX 34 premium-area and concentration backgrounds.
    pub const NATIVE_WIDTH: u32 = 1280;
    pub const NATIVE_HEIGHT: u32 = 720;
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
            Self::PremiumArea => "premium_area",
            Self::Concentration => "concentration",
            Self::NoteSkin => "note_skin",
            Self::GameplayShell => "gameplay_shell",
            Self::Decision => "decision",
            Self::Transition => "transition",
            Self::Title => "title",
            Self::Login => "login",
            Self::ModeSelect => "mode_select",
            Self::GameplayBottom => "gameplay_bottom",
            Self::Logout => "logout",
        }
    }

    /// Native adapters can share a descriptor only within compatible layouts.
    pub fn can_bind(self, target: Self) -> bool {
        fn group(surface: Surface) -> u8 {
            match surface {
                Surface::Results
                | Surface::SongSelect
                | Surface::GameplayShell
                | Surface::Decision
                | Surface::Transition
                | Surface::Title
                | Surface::Login
                | Surface::ModeSelect
                | Surface::GameplayBottom
                | Surface::Logout => 0,
                Surface::NoteSkin => 3,
                Surface::SuddenPlus | Surface::Lift | Surface::TargetGraph => 1,
                Surface::PremiumArea | Surface::Concentration => 2,
            }
        }
        self != target && group(self) == group(target)
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
    pub premium_area: SourceOverride,
    pub concentration: SourceOverride,
    pub note_skin: SourceOverride,
    pub gameplay_shell: SourceOverride,
    pub decision: SourceOverride,
    pub transition: SourceOverride,
    pub title: SourceOverride,
    pub login: SourceOverride,
    pub mode_select: SourceOverride,
    pub gameplay_bottom: SourceOverride,
    pub logout: SourceOverride,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct SourceOverride {
    pub bilibili: Option<crate::bilibili::Selection>,
    pub bind: Option<Surface>,
    pub source: Option<Source>,
    pub media_path: Option<PathBuf>,
    pub local_directory: Option<PathBuf>,
    pub recursive: Option<bool>,
    pub fit: Option<Fit>,
    pub moebooru: MoebooruOverride,
}

impl SourceOverride {
    /// An all-default override inherits the shared premium-area image instead of
    /// requesting an independent one.
    pub fn is_empty(&self) -> bool {
        self.bind.is_none()
            && self.bilibili.is_none()
            && self.source.is_none()
            && self.media_path.is_none()
            && self.local_directory.is_none()
            && self.recursive.is_none()
            && self.fit.is_none()
            && self.moebooru.domain.is_none()
            && self.moebooru.api_url.is_none()
            && self.moebooru.media_hosts.is_none()
            && self.moebooru.tags.is_none()
            && self.moebooru.save_directory.is_none()
            && self.moebooru.page_max.is_none()
            && self.moebooru.timeout_seconds.is_none()
            && self.moebooru.max_images.is_none()
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct MoebooruOverride {
    pub api_url: Option<String>,
    pub media_hosts: Option<Vec<String>>,
    pub domain: Option<MoebooruDomain>,
    pub tags: Option<String>,
    pub save_directory: Option<PathBuf>,
    pub page_max: Option<u32>,
    pub timeout_seconds: Option<u64>,
    pub max_images: Option<usize>,
}

/// New surfaces are opt-in; existing configurations retain results-only behavior.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Surfaces {
    pub results: bool,
    pub song_select: bool,
    pub sudden_plus: bool,
    pub lift: bool,
    pub target_graph: bool,
    pub premium_area: bool,
    pub concentration: bool,
    pub note_skin: bool,
    pub gameplay_shell: bool,
    pub decision: bool,
    pub transition: bool,
    pub title: bool,
    pub login: bool,
    pub mode_select: bool,
    pub gameplay_bottom: bool,
    pub logout: bool,
}

impl Default for Surfaces {
    fn default() -> Self {
        Self {
            results: true,
            song_select: false,
            sudden_plus: false,
            lift: false,
            target_graph: false,
            premium_area: false,
            concentration: false,
            note_skin: false,
            gameplay_shell: false,
            decision: false,
            transition: false,
            title: false,
            login: false,
            mode_select: false,
            gameplay_bottom: false,
            logout: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Moebooru {
    /// Empty selects the built-in domain's post.json endpoint.
    #[serde(default)]
    pub api_url: String,
    /// Additional exact CDN hostnames for a custom API.
    #[serde(default)]
    pub media_hosts: Vec<String>,
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

impl Moebooru {
    pub fn posts_url(&self) -> &str {
        if self.api_url.is_empty() {
            self.domain.posts_url()
        } else {
            &self.api_url
        }
    }

    pub fn namespace(&self) -> String {
        if self.api_url.is_empty() {
            return self.domain.namespace().into();
        }
        use sha2::{Digest, Sha256};
        format!(
            "booru{}",
            &hex::encode(Sha256::digest(self.api_url.as_bytes()))[..12]
        )
    }

    pub fn post_label(&self, id: u64) -> String {
        format!("{}:{id}", self.namespace())
    }

    pub fn allows_url(&self, url: &reqwest::Url) -> bool {
        if self.api_url.is_empty() {
            return crate::source::allowed_url(self.domain, url);
        }
        let Ok(endpoint) = reqwest::Url::parse(&self.api_url) else {
            return false;
        };
        matches!(url.scheme(), "http" | "https")
            && url.username().is_empty()
            && url.password().is_none()
            && (url.origin() == endpoint.origin()
                || (url.scheme() == "https"
                    && url.port_or_known_default() == Some(443)
                    && url.host_str().is_some_and(|h| {
                        self.media_hosts.iter().any(|v| v.eq_ignore_ascii_case(h))
                    })))
    }

    fn validate(&self) -> Result<()> {
        if !self.api_url.is_empty() {
            let url = reqwest::Url::parse(&self.api_url)
                .map_err(|_| Error::Config("invalid Moebooru API URL"))?;
            if !matches!(url.scheme(), "https" | "http")
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
                || url.fragment().is_some()
            {
                return Err(Error::Config(
                    "API URL must be HTTP(S) without embedded credentials or fragment",
                ));
            }
        }
        if self.media_hosts.len() > 32
            || self.media_hosts.iter().any(|h| {
                h.is_empty()
                    || h.contains(['/', ':', '@', '*'])
                    || h.chars().any(char::is_whitespace)
            })
        {
            return Err(Error::Config(
                "media_hosts must contain exact CDN hostnames",
            ));
        }
        Ok(())
    }
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
    /// Resource textures retain aspect and alpha; set by the adapter, not TOML.
    #[serde(skip)]
    pub resource: bool,
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
        if value.bilibili.cache_directory.is_relative() {
            value.bilibili.cache_directory = root.join(&value.bilibili.cache_directory);
        }
        if !value.media_path.as_os_str().is_empty() && value.media_path.is_relative() {
            value.media_path = root.join(&value.media_path);
        }
        if !value.results_video.as_os_str().is_empty() && value.results_video.is_relative() {
            value.results_video = root.join(&value.results_video);
        }
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
            &mut value.sources.premium_area,
            &mut value.sources.concentration,
            &mut value.sources.note_skin,
            &mut value.sources.gameplay_shell,
            &mut value.sources.decision,
            &mut value.sources.transition,
            &mut value.sources.title,
            &mut value.sources.login,
            &mut value.sources.mode_select,
            &mut value.sources.gameplay_bottom,
            &mut value.sources.logout,
        ]
        .into_iter()
        .chain(value.artwork.iter_mut().map(|art| &mut art.source))
        {
            for path in [
                &mut source.media_path,
                &mut source.local_directory,
                &mut source.moebooru.save_directory,
            ]
            .into_iter()
            .flatten()
            {
                if !path.as_os_str().is_empty() && path.is_relative() {
                    *path = root.join(&*path);
                }
            }
        }
        Ok(value)
    }

    pub fn hide_frame_chara(&self) -> bool {
        self.enabled && !self.selection.frame_chara
    }

    pub fn character_source(&self) -> CharacterSource {
        match self.selection.frame_chara_source {
            CharacterSource::Auto if self.mmd.enabled => CharacterSource::Mmd,
            CharacterSource::Auto if !self.selection.frame_chara_image.as_os_str().is_empty() => {
                CharacterSource::Image
            }
            CharacterSource::Auto => CharacterSource::Stock,
            source => source,
        }
    }

    pub fn uses_mmd_character(&self) -> bool {
        self.enabled
            && self.selection.frame_chara
            && self.mmd.enabled
            && self.character_source() == CharacterSource::Mmd
    }

    pub fn uses_character_image(&self) -> bool {
        self.enabled
            && self.selection.frame_chara
            && matches!(
                self.selection.frame_chara_source,
                CharacterSource::Auto | CharacterSource::Image
            )
    }

    pub fn uses_mmd(&self) -> bool {
        self.enabled && self.mmd.enabled && (self.uses_mmd_character() || self.mmd.replace_qpro)
    }

    pub fn uses_mmd_qpro(&self) -> bool {
        self.enabled && self.mmd.enabled && self.mmd.replace_qpro
    }

    pub fn validate(&self) -> Result<()> {
        self.validate_base()?;
        self.mmd.validate()?;
        if self.artwork.len() > 512 {
            return Err(Error::Config("at most 512 custom artwork entries"));
        }
        let mut keys = std::collections::HashSet::new();
        for art in &self.artwork {
            if art
                .native_size
                .is_some_and(|size| size.iter().any(|v| *v == 0 || *v > 16384))
            {
                return Err(Error::Config("invalid artwork native dimensions"));
            }
            if art.archive.is_empty()
                || art.archive.len() > 255
                || art.archive.contains(['\\', '\0'])
                || art.archive.starts_with('/')
                || art.archive.split('/').any(|p| p == "..")
                || art.texture.is_empty()
                || art.texture.len() > 255
                || art.texture.contains('\0')
            {
                return Err(Error::Config("invalid artwork archive/texture name"));
            }
            if !keys.insert((&art.archive, &art.texture)) {
                return Err(Error::Config("duplicate artwork archive/texture"));
            }
            self.for_artwork(art).validate_base()?;
        }
        if !self.results_video.as_os_str().is_empty()
            && (self.image.width > 1920 || self.image.height > 1080)
        {
            return Err(Error::Config("video canvas must not exceed 1920x1080"));
        }
        for surface in Surface::ALL {
            self.binding_root(surface)?;
            self.for_surface(surface).validate_base()?;
        }
        Ok(())
    }

    pub fn for_artwork(&self, art: &Artwork) -> Self {
        let mut base = self.clone();
        base.artwork.clear();
        base.sources.transition = art.source.clone();
        // Explicit bindings share the selected module's prepared media at runtime.
        base.sources.transition.bind = None;
        base.surfaces.transition = art.enabled;
        let mut config = base.for_unbound_surface(Surface::Transition);
        config.image.resource = true;
        if let Some([width, height]) = art.native_size {
            let scale = (1920. / width.max(1) as f64)
                .min(1080. / height.max(1) as f64)
                .min(1.);
            config.image.width = (width as f64 * scale).round().max(2.) as u32;
            config.image.height = (height as f64 * scale).round().max(2.) as u32;
        }
        config
    }

    pub fn for_surface(&self, surface: Surface) -> Self {
        let root = self.binding_root(surface).unwrap_or(surface);
        let mut c = self.for_unbound_surface(root);
        c.enabled = self.surface_enabled(surface);
        c
    }

    pub fn source_override(&self, surface: Surface) -> &SourceOverride {
        match surface {
            Surface::Results => &self.sources.results,
            Surface::SongSelect => &self.sources.song_select,
            Surface::SuddenPlus => &self.sources.sudden_plus,
            Surface::Lift => &self.sources.lift,
            Surface::TargetGraph => &self.sources.target_graph,
            Surface::PremiumArea => &self.sources.premium_area,
            Surface::Concentration => &self.sources.concentration,
            Surface::NoteSkin => &self.sources.note_skin,
            Surface::GameplayShell => &self.sources.gameplay_shell,
            Surface::Decision => &self.sources.decision,
            Surface::Transition => &self.sources.transition,
            Surface::Title => &self.sources.title,
            Surface::Login => &self.sources.login,
            Surface::ModeSelect => &self.sources.mode_select,
            Surface::GameplayBottom => &self.sources.gameplay_bottom,
            Surface::Logout => &self.sources.logout,
        }
    }

    pub fn binding_root(&self, mut surface: Surface) -> Result<Surface> {
        let mut seen = [false; Surface::COUNT];
        loop {
            if seen[surface.index()] {
                return Err(Error::Config("circular surface binding"));
            }
            seen[surface.index()] = true;
            let next = self.source_override(surface).bind.or_else(|| {
                (surface == Surface::Concentration && self.concentration_shares_premium())
                    .then_some(Surface::PremiumArea)
            });
            match next {
                Some(next) if surface.can_bind(next) => surface = next,
                Some(_) => return Err(Error::Config("incompatible surface binding")),
                None => return Ok(surface),
            }
        }
    }

    pub fn surface_enabled(&self, surface: Surface) -> bool {
        self.enabled
            && match surface {
                Surface::Results => self.surfaces.results,
                Surface::SongSelect => self.surfaces.song_select,
                Surface::SuddenPlus => self.surfaces.sudden_plus,
                Surface::Lift => self.surfaces.lift,
                Surface::TargetGraph => self.surfaces.target_graph,
                Surface::PremiumArea => self.surfaces.premium_area,
                Surface::Concentration => self.surfaces.concentration,
                Surface::NoteSkin => self.surfaces.note_skin,
                Surface::GameplayShell => self.surfaces.gameplay_shell,
                Surface::Decision => self.surfaces.decision,
                Surface::Transition => self.surfaces.transition,
                Surface::Title => self.surfaces.title,
                Surface::Login => self.surfaces.login,
                Surface::ModeSelect => self.surfaces.mode_select,
                Surface::GameplayBottom => self.surfaces.gameplay_bottom,
                Surface::Logout => self.surfaces.logout,
            }
    }

    fn for_unbound_surface(&self, surface: Surface) -> Self {
        let mut c = self.clone();
        let o = match surface {
            Surface::Results => &self.sources.results,
            Surface::SongSelect => &self.sources.song_select,
            Surface::SuddenPlus => &self.sources.sudden_plus,
            Surface::Lift => &self.sources.lift,
            Surface::TargetGraph => &self.sources.target_graph,
            Surface::PremiumArea => &self.sources.premium_area,
            Surface::Concentration => &self.sources.concentration,
            Surface::NoteSkin => &self.sources.note_skin,
            Surface::GameplayShell => &self.sources.gameplay_shell,
            Surface::Decision => &self.sources.decision,
            Surface::Transition => &self.sources.transition,
            Surface::Title => &self.sources.title,
            Surface::Login => &self.sources.login,
            Surface::ModeSelect => &self.sources.mode_select,
            Surface::GameplayBottom => &self.sources.gameplay_bottom,
            Surface::Logout => &self.sources.logout,
        };
        c.sources = Sources::default();
        c.enabled &= match surface {
            Surface::Results => true,
            Surface::SongSelect => self.surfaces.song_select,
            Surface::SuddenPlus => self.surfaces.sudden_plus,
            Surface::Lift => self.surfaces.lift,
            Surface::TargetGraph => self.surfaces.target_graph,
            Surface::PremiumArea => self.surfaces.premium_area,
            Surface::Concentration => self.surfaces.concentration,
            Surface::NoteSkin => self.surfaces.note_skin,
            Surface::GameplayShell => self.surfaces.gameplay_shell,
            Surface::Decision => self.surfaces.decision,
            Surface::Transition => self.surfaces.transition,
            Surface::Title => self.surfaces.title,
            Surface::Login => self.surfaces.login,
            Surface::ModeSelect => self.surfaces.mode_select,
            Surface::GameplayBottom => self.surfaces.gameplay_bottom,
            Surface::Logout => self.surfaces.logout,
        };
        if let Some(v) = o.source {
            c.source = v;
        }
        if let Some(v) = &o.bilibili {
            c.bilibili.selection = v.clone();
        }
        if let Some(v) = &o.media_path {
            c.media_path = v.clone();
        }
        // Preserve old result-only configuration until a surface source is chosen.
        if surface == Surface::Results
            && o.source.is_none()
            && !self.results_video.as_os_str().is_empty()
        {
            c.source = Source::File;
            c.media_path = self.results_video.clone();
        }
        c.results_video = PathBuf::new();
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
        if let Some(v) = &o.moebooru.api_url {
            c.moebooru.api_url = v.clone();
        }
        if let Some(v) = &o.moebooru.media_hosts {
            c.moebooru.media_hosts = v.clone();
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
            Surface::GameplayBottom => {
                c.image.width = 622;
                c.image.height = 300;
            }
            Surface::NoteSkin => {
                c.image.width = 540;
                c.image.height = 90;
            }
            Surface::SongSelect
            | Surface::GameplayShell
            | Surface::Decision
            | Surface::Transition
            | Surface::Title
            | Surface::Login
            | Surface::ModeSelect
            | Surface::Logout => {
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
            Surface::PremiumArea | Surface::Concentration => {
                // Both native displays are 1280x720; keeping the canvas at the
                // native size preserves the game's own placement and upscaling.
                c.image.width = Surface::NATIVE_WIDTH;
                c.image.height = Surface::NATIVE_HEIGHT;
            }
        }
        c
    }

    /// The concentration background reuses the premium-area image while its own
    /// `[sources.concentration]` section stays empty.
    pub fn concentration_shares_premium(&self) -> bool {
        self.sources.concentration.is_empty()
    }

    fn validate_base(&self) -> Result<()> {
        self.bilibili.validate()?;
        self.moebooru.validate()?;
        let minimum = if self.image.resource { 2 } else { 64 };
        if !(minimum..=3840).contains(&self.image.width)
            || !(minimum..=2160).contains(&self.image.height)
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
    fn character_source_is_independent_of_qpro_and_preserves_legacy_selection() {
        let mut c = Config::parse(include_str!("../tests/fixtures/default.toml")).unwrap();
        c.mmd.enabled = true;
        c.mmd.replace_qpro = true;
        c.selection.frame_chara_image = "saved-image.png".into();
        assert_eq!(c.character_source(), CharacterSource::Mmd);
        assert!(c.uses_mmd_character());
        for source in [
            CharacterSource::Stock,
            CharacterSource::Image,
            CharacterSource::Mmd,
        ] {
            c.selection.frame_chara_source = source;
            assert!(c.uses_mmd(), "Qpro must retain its model");
            assert!(c.uses_mmd_qpro());
            assert_eq!(c.uses_mmd_character(), source == CharacterSource::Mmd);
            assert_eq!(c.uses_character_image(), source == CharacterSource::Image);
            let saved = serde_json::to_string(&c).unwrap();
            let restored: Config = serde_json::from_str(&saved).unwrap();
            assert_eq!(restored.character_source(), source);
            assert_eq!(
                restored.selection.frame_chara_image,
                c.selection.frame_chara_image
            );
        }
        c.selection.frame_chara = false;
        assert!(!c.uses_mmd_character());
        assert!(c.uses_mmd());
        c.mmd.replace_qpro = false;
        assert!(!c.uses_mmd_qpro());
        assert!(!c.uses_mmd());
        c.selection.frame_chara = true;
        c.selection.frame_chara_source = CharacterSource::Auto;
        c.mmd.enabled = false;
        assert_eq!(c.character_source(), CharacterSource::Image);
        c.selection.frame_chara_image.clear();
        assert_eq!(c.character_source(), CharacterSource::Stock);
    }
    #[test]
    fn binding_chains_use_root_settings_but_independent_enable_and_reject_cycles() {
        let mut c = Config::parse(include_str!("../tests/fixtures/default.toml")).unwrap();
        c.sources.sudden_plus.bind = Some(Surface::Lift);
        c.sources.lift.bind = Some(Surface::TargetGraph);
        c.sources.target_graph.source = Some(Source::File);
        c.sources.target_graph.media_path = Some("movie.mp4".into());
        c.surfaces.sudden_plus = true;
        c.surfaces.lift = false;
        c.surfaces.target_graph = false;
        c.validate().unwrap();
        assert_eq!(
            c.binding_root(Surface::SuddenPlus).unwrap(),
            Surface::TargetGraph
        );
        let resolved = c.for_surface(Surface::SuddenPlus);
        assert!(resolved.enabled);
        assert_eq!(resolved.source, Source::File);
        assert_eq!(resolved.media_path, PathBuf::from("movie.mp4"));
        assert!(!c.for_surface(Surface::Lift).enabled);
        c.sources.target_graph.bind = Some(Surface::SuddenPlus);
        assert!(c.validate().is_err());
        c.sources.target_graph.bind = Some(Surface::TargetGraph);
        assert!(c.validate().is_err());
    }

    #[test]
    fn custom_api_policy_is_scoped_to_endpoint_and_explicit_cdn_hosts() {
        let mut c = Config::parse(include_str!("../tests/fixtures/default.toml")).unwrap();
        c.moebooru.api_url = "http://localhost:18888/post.json".into();
        c.moebooru.media_hosts = vec!["cdn.example.org".into()];
        c.validate().unwrap();
        for url in [
            "http://localhost:18888/a.png",
            "https://cdn.example.org/a.png",
        ] {
            assert!(c.moebooru.allows_url(&reqwest::Url::parse(url).unwrap()));
        }
        for url in [
            "http://localhost:18889/a.png",
            "http://cdn.example.org/a.png",
            "https://cdn.example.org.evil/a.png",
            "https://user@cdn.example.org/a.png",
        ] {
            assert!(!c.moebooru.allows_url(&reqwest::Url::parse(url).unwrap()));
        }
        let first = c.moebooru.namespace();
        c.moebooru.api_url = "https://example.org/post.json".into();
        assert_ne!(first, c.moebooru.namespace());
        c.moebooru.api_url = "file:///post.json".into();
        assert!(c.validate().is_err());
    }
    #[test]
    fn selection_character_defaults_and_strict_live_settings() {
        let shipped = include_str!("../tests/fixtures/default.toml");
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
            include_str!("../tests/fixtures/default.toml")
                .split("\n[sources.results]")
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
        let shipped = include_str!("../tests/fixtures/default.toml");
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
        let shipped = include_str!("../tests/fixtures/default.toml");
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
        let shipped = include_str!("../tests/fixtures/default.toml");
        let mut c: Config = toml::from_str(shipped).unwrap();
        c.validate().unwrap();
        for (surface, folder) in [
            (Surface::Results, "results"),
            (Surface::SongSelect, "song-select"),
            (Surface::SuddenPlus, "sudden-plus"),
            (Surface::Lift, "lift"),
            (Surface::TargetGraph, "target-graph"),
            (Surface::PremiumArea, "premium-area"),
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
        assert!(c.concentration_shares_premium());
        let concentration = c.for_surface(Surface::Concentration);
        assert_eq!(
            concentration.local_directory,
            PathBuf::from("iidxbg/premium-area")
        );
        for surface in [Surface::PremiumArea, Surface::Concentration] {
            let resolved = c.for_surface(surface);
            assert_eq!(
                (resolved.image.width, resolved.image.height),
                (Surface::NATIVE_WIDTH, Surface::NATIVE_HEIGHT)
            );
        }
        let separate: Config = toml::from_str(&shipped.replace(
            "[sources.concentration]",
            "[sources.concentration]\nlocal_directory = 'iidxbg/other'",
        ))
        .unwrap();
        assert!(!separate.concentration_shares_premium());
        assert_eq!(
            separate.for_surface(Surface::Concentration).local_directory,
            PathBuf::from("iidxbg/other")
        );
        c.image.width = 0;
        assert!(c.validate().is_err());
        assert!(toml::from_str::<Config>(&shipped.replace("\"local\"", "\"typo\"")).is_err());
    }

    #[test]
    fn toml_comments_paths_and_errors() {
        let shipped = include_str!("../tests/fixtures/default.toml");
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
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let c = Config::load(&root.join("default.toml")).unwrap();
        assert_eq!(c.local_directory, root.join("iidxbg"));
        assert_eq!(c.moebooru.save_directory, root.join("iidxbg/downloads"));
    }
}
#[cfg(test)]
mod artwork_tests {
    use super::*;
    #[test]
    fn tiny_artwork_uses_resource_limits_without_relaxing_surface_limits() {
        let base = include_str!("../tests/fixtures/default.toml");
        let c = Config::parse(&format!(
            "{base}\n[[artwork]]\narchive='0/common.ifs'\ntexture='tiny'\nnative_size=[1,16]\n"
        ))
        .unwrap();
        let resource = c.for_artwork(&c.artwork[0]);
        assert_eq!((resource.image.width, resource.image.height), (2, 16));
        assert!(resource.validate_base().is_ok());
        let mut ordinary = c;
        ordinary.image.width = 2;
        assert!(ordinary.validate().is_err());
    }

    #[test]
    #[ignore = "requires the private catalog; set IIDXBG_ARTWORK_CATALOG"]
    fn every_catalog_entry_is_configurable() {
        let path = std::env::var_os("IIDXBG_ARTWORK_CATALOG").expect("IIDXBG_ARTWORK_CATALOG");
        let entries: Vec<serde_json::Value> =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let mut config = Config::parse(include_str!("../tests/fixtures/default.toml")).unwrap();
        for entry in &entries {
            config.artwork = vec![Artwork {
                scene_scope: None,
                bottom_crop: false,
                archive: entry["archive"].as_str().unwrap().into(),
                texture: entry["texture"].as_str().unwrap().into(),
                native_size: Some([
                    entry["width"].as_u64().unwrap() as u32,
                    entry["height"].as_u64().unwrap() as u32,
                ]),
                enabled: false,
                source: SourceOverride::default(),
                native_mask: true,
            }];
            config
                .validate()
                .unwrap_or_else(|error| panic!("{entry}: {error}"));
        }
        assert!(!entries.is_empty());
        eprintln!("Validated all {} catalog configurations", entries.len());
    }
    #[test]
    fn named_artwork_is_strict_independent_and_resolves_paths() {
        let base = include_str!("../tests/fixtures/default.toml");
        let entry = "\n[[artwork]]\narchive='gmframe34.ifs'\ntexture='lane_base'\nenabled=true\n[artwork.source]\nsource='file'\nmedia_path='custom.png'\n";
        let c = Config::parse(&format!("{base}{entry}")).unwrap();
        assert_eq!(c.artwork.len(), 1);
        assert!(c.artwork[0].native_mask);
        assert_eq!(c.for_artwork(&c.artwork[0]).source, Source::File);
        assert_eq!(
            c.for_artwork(&c.artwork[0]).media_path,
            PathBuf::from("custom.png")
        );
        assert!(Config::parse(&format!("{base}{entry}{entry}")).is_err());
        assert!(
            Config::parse(&format!(
                "{base}{}",
                entry.replace("gmframe34.ifs", "../gmframe34.ifs")
            ))
            .is_err()
        );
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, format!("{base}{entry}")).unwrap();
        let c = Config::load(&path).unwrap();
        assert_eq!(
            c.artwork[0].source.media_path,
            Some(dir.path().join("custom.png"))
        );
    }
}
