use gpui::{Hsla, px, rgb, rgba};
use gpui_kit::assets::IconName;
use gpui_kit::component::{Theme, ThemeMode, scroll::ScrollbarMode};
use gpui_kit::*;
use iidxbg::config::{Config, Surface};

#[derive(Clone, Copy, Debug)]
pub struct LayoutMetrics {
    pub sidebar: f32,
    pub padding: f32,
    pub compact: bool,
}

/// The sidebar is an icon rail, not a text panel. Keeping one fixed width
/// leaves the component canvas predictable at every window size.
pub fn layout_metrics(viewport_width: f32) -> LayoutMetrics {
    if viewport_width >= 1280. {
        LayoutMetrics {
            sidebar: 64.,
            padding: 32.,
            compact: false,
        }
    } else if viewport_width >= 900. {
        LayoutMetrics {
            sidebar: 64.,
            padding: 28.,
            compact: false,
        }
    } else {
        LayoutMetrics {
            sidebar: 60.,
            padding: 20.,
            compact: true,
        }
    }
}

pub fn install(cx: &mut App) {
    Theme::update(cx, |theme| {
        theme.mode = ThemeMode::Dark;
        theme.shadow = false;
        theme.colors.background = rgb(0x000000).into();
        theme.colors.foreground = rgb(0xf0f0f0).into();
        theme.colors.border = rgb(0x2a2a2a).into();
        theme.colors.muted = rgb(0x000000).into();
        theme.colors.muted_foreground = rgb(0x8c8c8c).into();
        theme.colors.primary = rgb(0x5fe08e).into();
        theme.colors.primary_hover = rgb(0x78eca4).into();
        theme.colors.primary_active = rgb(0x46c477).into();
        theme.colors.primary_foreground = rgb(0x000000).into();
        theme.colors.button_primary = rgb(0x000000).into();
        theme.colors.button_primary_hover = rgb(0x101010).into();
        theme.colors.button_primary_active = rgb(0x000000).into();
        theme.colors.button_primary_foreground = rgb(0x5fe08e).into();
        theme.colors.button = rgb(0x000000).into();
        theme.colors.button_hover = rgb(0x000000).into();
        theme.colors.button_active = rgb(0x000000).into();
        theme.colors.secondary = rgb(0x000000).into();
        theme.colors.secondary_hover = rgb(0x000000).into();
        theme.colors.secondary_active = rgb(0x000000).into();
        theme.colors.secondary_foreground = rgb(0xc4c4c4).into();
        theme.colors.accent = rgb(0x000000).into();
        theme.colors.accent_foreground = rgb(0x5fe08e).into();
        theme.colors.ring = rgb(0x5fe08e).into();
        theme.colors.sidebar = rgb(0x000000).into();
        theme.colors.sidebar_foreground = rgb(0xc4c4c4).into();
        theme.colors.sidebar_border = rgb(0x2a2a2a).into();
        theme.colors.sidebar_accent = rgb(0x000000).into();
        theme.colors.sidebar_accent_foreground = rgb(0x5fe08e).into();
        theme.colors.sidebar_primary = rgb(0x5fe08e).into();
        theme.colors.sidebar_primary_foreground = rgb(0x000000).into();
        theme.colors.input = rgb(0x333333).into();
        theme.colors.group_box = rgb(0x000000).into();
        theme.colors.switch = rgb(0x2c2c2c).into();
        theme.colors.selection = rgb(0x2a2a2a).into();
        theme.colors.popover = rgb(0x000000).into();
        theme.colors.popover_foreground = rgb(0xf0f0f0).into();
        theme.colors.title_bar = rgb(0x000000).into();
        theme.colors.title_bar_border = rgb(0x2a2a2a).into();
        theme.colors.status_bar = rgb(0x000000).into();
        theme.colors.status_bar_border = rgb(0x2a2a2a).into();
        theme.font_size = px(14.);
        theme.radius = px(8.);
        theme.radius_lg = px(12.);
        theme.tokens = Theme::from(&theme.colors).tokens;
    });
    Theme::set_scrollbar_mode(ScrollbarMode::Always, cx);
}

pub fn page_bg() -> Hsla {
    rgb(0x000000).into()
}

pub fn sidebar_bg() -> Hsla {
    rgb(0x000000).into()
}

pub fn card_bg() -> Hsla {
    rgb(0x000000).into()
}

pub fn ink() -> Hsla {
    rgb(0xf0f0f0).into()
}

pub fn ink_soft() -> Hsla {
    rgb(0xc4c4c4).into()
}

pub fn muted_ink() -> Hsla {
    rgb(0x8c8c8c).into()
}

pub fn faint_ink() -> Hsla {
    rgb(0x5c5c5c).into()
}

pub fn line() -> Hsla {
    rgb(0x2a2a2a).into()
}

pub fn line_soft() -> Hsla {
    rgb(0x1a1a1a).into()
}

pub fn line_strong() -> Hsla {
    rgb(0x414141).into()
}

pub fn accent() -> Hsla {
    rgb(0x5fe08e).into()
}

pub fn accent_hover() -> Hsla {
    rgb(0x7af0a4).into()
}

/// Secondary panels are outlined, never tinted. Keeping these helpers
/// transparent lets callers opt into a border without painting a green wash.
pub fn accent_soft() -> Hsla {
    rgba(0x00000000).into()
}

pub fn success() -> Hsla {
    rgb(0x5fe08e).into()
}

pub fn success_soft() -> Hsla {
    rgba(0x00000000).into()
}

pub fn warning() -> Hsla {
    rgb(0xd9b45f).into()
}

pub fn warning_soft() -> Hsla {
    rgba(0x00000000).into()
}

pub fn danger() -> Hsla {
    rgb(0xff7b72).into()
}

pub fn danger_soft() -> Hsla {
    rgba(0x00000000).into()
}

pub fn overlay() -> Hsla {
    rgba(0x000000e6).into()
}

pub fn hover_panel_bg() -> Hsla {
    rgba(0x000000fa).into()
}

pub fn surface_color(surface: Surface) -> u32 {
    match surface {
        Surface::Results => 0x5fe08e,
        Surface::SongSelect => 0x46c7c1,
        Surface::SuddenPlus => 0xe3b45c,
        Surface::Lift => 0xf07d74,
        Surface::TargetGraph => 0x6be29f,
        Surface::PremiumArea => 0xd6b35f,
        Surface::Concentration => 0xb18cff,
    }
}

pub fn surface_accent(surface: Surface) -> Hsla {
    rgb(surface_color(surface)).into()
}

pub fn surface_icon(surface: Surface) -> IconName {
    match surface {
        Surface::Results => IconName::Trophy,
        Surface::SongSelect => IconName::ListMusic,
        Surface::SuddenPlus => IconName::ChevronsUp,
        Surface::Lift => IconName::ChevronsDown,
        Surface::TargetGraph => IconName::ChartNoAxesColumnIncreasing,
        Surface::PremiumArea => IconName::Crown,
        Surface::Concentration => IconName::ScanEye,
    }
}

pub fn surface_title(surface: Surface) -> &'static str {
    match surface {
        Surface::Results => "结算画面",
        Surface::SongSelect => "选曲画面",
        Surface::SuddenPlus => "SUDDEN+ 覆盖层",
        Surface::Lift => "LIFT 覆盖层",
        Surface::TargetGraph => "目标分数图",
        Surface::PremiumArea => "Premium 区域",
        Surface::Concentration => "集中模式",
    }
}

pub fn surface_description(surface: Surface) -> &'static str {
    match surface {
        Surface::Results => "每首歌曲结算时展示的插画背景",
        Surface::SongSelect => "选曲界面背后的全屏背景",
        Surface::SuddenPlus => "上侧轨道遮罩，HIDDEN/SUDDEN 仍由游戏控制",
        Surface::Lift => "下侧轨道遮罩，保留原生 LIFT 行为",
        Surface::TargetGraph => "目标分数图背后的窄幅图像",
        Surface::PremiumArea => "Premium 区域背景，IIDX 34 专用",
        Surface::Concentration => "集中模式背景，IIDX 34 专用",
    }
}

pub fn surface_size(config: &Config, surface: Surface) -> (u32, u32) {
    let resolved = config.for_surface(surface);
    (resolved.image.width, resolved.image.height)
}

pub fn source_label(source: iidxbg::config::Source) -> &'static str {
    match source {
        iidxbg::config::Source::Local => "本地",
        iidxbg::config::Source::Remote => "远程",
        iidxbg::config::Source::Mixed => "混合",
    }
}

pub fn fit_label(fit: iidxbg::config::Fit) -> &'static str {
    match fit {
        iidxbg::config::Fit::Cover => "裁切填充",
        iidxbg::config::Fit::Contain => "完整显示",
    }
}

pub fn domain_label(domain: iidxbg::config::MoebooruDomain) -> &'static str {
    match domain {
        iidxbg::config::MoebooruDomain::Net => "Konachan.net",
        iidxbg::config::MoebooruDomain::Com => "Konachan.com",
        iidxbg::config::MoebooruDomain::Yandere => "Yande.re",
    }
}

pub fn format_dimensions(config: &Config, surface: Surface) -> String {
    let (width, height) = surface_size(config, surface);
    format!("{width} × {height}")
}

pub fn ratio_label(config: &Config, surface: Surface) -> String {
    let (width, height) = surface_size(config, surface);
    let divisor = gcd(width, height);
    format!("{}:{}", width / divisor, height / divisor)
}

fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        let remainder = a % b;
        a = b;
        b = remainder;
    }
    a.max(1)
}
