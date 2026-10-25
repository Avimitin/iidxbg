use gpui::{Hsla, px, rgb, rgba};
use gpui_kit::assets::IconName;
use gpui_kit::component::{Theme, ThemeMode, scroll::ScrollbarMode};
use gpui_kit::*;
use iidxbg::config::{Config, Surface};

pub const SIDEBAR_WIDTH: f32 = 64.;
pub const TITLE_BAR_HEIGHT: f32 = 36.;

pub fn install(cx: &mut App) {
    // Loading a mode replaces its palette; apply our colors afterwards.
    Theme::change(ThemeMode::Dark, None, cx);
    Theme::update(cx, |theme| {
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
        theme.colors.button_primary = rgba(0x00000000).into();
        theme.colors.button_primary_hover = rgba(0x00000000).into();
        theme.colors.button_primary_active = rgba(0x00000000).into();
        theme.colors.button_primary_foreground = rgb(0x5fe08e).into();
        theme.colors.button = rgba(0x00000000).into();
        theme.colors.button_hover = rgba(0x00000000).into();
        theme.colors.button_active = rgba(0x00000000).into();
        theme.colors.secondary = rgba(0x00000000).into();
        theme.colors.secondary_hover = rgba(0x00000000).into();
        theme.colors.secondary_active = rgba(0x00000000).into();
        theme.colors.secondary_foreground = rgb(0xc4c4c4).into();
        theme.colors.accent = rgba(0x00000000).into();
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
        theme.font_size = px(16.);
        theme.radius = px(0.);
        theme.radius_lg = px(0.);
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

pub fn accent() -> Hsla {
    rgb(0x5fe08e).into()
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

pub fn surface_color(surface: Surface) -> u32 {
    match surface {
        Surface::Results => 0x5fe08e,
        Surface::SongSelect => 0x46c7c1,
        Surface::SuddenPlus => 0xe3b45c,
        Surface::Lift => 0xf07d74,
        Surface::TargetGraph => 0x6be29f,
        Surface::PremiumArea => 0xd6b35f,
        Surface::Concentration => 0xb18cff,
        Surface::NoteSkin => 0x77baff,
        Surface::GameplayShell => 0xe5b469,
        Surface::Decision => 0x72dcc7,
        Surface::Transition => 0xe88bbc,
        Surface::Title => 0x72dcc7,
        Surface::Login => 0x72dcc7,
        Surface::ModeSelect => 0x72dcc7,
        Surface::GameplayBottom => 0x72dcc7,
        Surface::Logout => 0x72dcc7,
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
        Surface::NoteSkin => IconName::ListMusic,
        Surface::GameplayShell => IconName::ScanEye,
        Surface::Decision => IconName::Trophy,
        Surface::Transition => IconName::ChevronsUp,
        Surface::Title => IconName::ScanEye,
        Surface::Login => IconName::ScanEye,
        Surface::ModeSelect => IconName::ScanEye,
        Surface::GameplayBottom => IconName::ScanEye,
        Surface::Logout => IconName::ScanEye,
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
        Surface::NoteSkin => "音符皮肤",
        Surface::GameplayShell => "演奏框体",
        Surface::Decision => "选曲决定画面",
        Surface::Transition => "过场动画插画",
        Surface::Title => "标题背景",
        Surface::Login => "登录背景",
        Surface::ModeSelect => "模式选择背景",
        Surface::GameplayBottom => "演奏区域底部框体",
        Surface::Logout => "退出结算背景",
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
        Surface::NoteSkin => "自动裁切普通音符与转盘音符，保留原生位置和尺寸",
        Surface::GameplayShell => "原生框体形状内填充图像，保留轨道与信息",
        Surface::Decision => "选曲决定背景，保留标题与难度信息",
        Surface::Transition => "替换决定画面中移动的插画，保留原生运动",
        Surface::Title => "标题画面的背景，保留原生标志与提示",
        Surface::Login => "读卡与登录画面的背景",
        Surface::ModeSelect => "模式选择背景，保留原生选项",
        Surface::GameplayBottom => "底部装饰框体，保留原生分数与仪表",
        Surface::Logout => "退出与最终汇总画面的背景",
    }
}

pub fn surface_size(config: &Config, surface: Surface) -> (u32, u32) {
    let resolved = config.for_surface(surface);
    (resolved.image.width, resolved.image.height)
}
