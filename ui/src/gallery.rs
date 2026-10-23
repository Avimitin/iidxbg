use gpui::{
    AnyElement, Hsla, IntoElement, ObjectFit, ParentElement, SharedString, Styled, Window,
    prelude::FluentBuilder as _, px,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::{Icon, Sizable, Size, StyledExt, h_flex, spinner::Spinner, v_flex};
use gpui_kit::*;
use iidxbg::config::Surface;

use crate::{
    app::{self, AppView, PreviewStatus},
    store, theme, widgets,
};

const GRID_COLUMNS: f32 = 4.;
const TITLE_BAR_HEIGHT: f32 = 34.;
const WIDE_RATIO: f32 = 16. / 9.;

const TOP_ROW: [Surface; 4] = [
    Surface::Results,
    Surface::SongSelect,
    Surface::PremiumArea,
    Surface::Concentration,
];
const BOTTOM_ROW: [Surface; 3] = [Surface::SuddenPlus, Surface::Lift, Surface::TargetGraph];

#[derive(Clone, Copy, Debug)]
struct Region {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl AppView {
    pub(crate) fn render_gallery(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let viewport = window.viewport_size();
        // The board is deliberately full-bleed: no page padding and no hero.
        // The custom title bar is the only chrome around it.
        let width = viewport.width.as_f32().max(320.);
        let height = (viewport.height.as_f32() - TITLE_BAR_HEIGHT).max(240.);
        let cell_width = width / GRID_COLUMNS;

        // The top band has four 16:9 tiles. Give it at least 30% of the content
        // height so the wide surfaces get more breathing room; the remaining
        // lower band becomes three portrait columns. The cap keeps a very short
        // window from turning the top band into a second full-height row.
        let top_height = (cell_width / WIDE_RATIO).clamp(height * 0.30, height * 0.38);
        let bottom_height = height - top_height;

        let mut modules = Vec::with_capacity(TOP_ROW.len() + BOTTOM_ROW.len() + 1);
        for (column, surface) in TOP_ROW.iter().enumerate() {
            let region = Region {
                x: column as f32 * cell_width,
                y: 0.,
                width: cell_width,
                height: top_height,
            };
            modules.push(self.render_surface_module(*surface, region, window, cx));
        }
        for (column, surface) in BOTTOM_ROW.iter().enumerate() {
            let region = Region {
                x: column as f32 * (width / 3.),
                y: top_height,
                width: (width / 3.),
                height: bottom_height,
            };
            modules.push(self.render_surface_module(*surface, region, window, cx));
        }

        div()
            .id("gallery-grid")
            .relative()
            .size_full()
            .overflow_hidden()
            .bg(theme::page_bg())
            .child(grid_lines(width, height, cell_width, top_height))
            .children(modules)
            .into_any_element()
    }

    fn render_surface_module(
        &mut self,
        surface: Surface,
        region: Region,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let config = self.store.config();
        let index = surface.index();
        let enabled = store::effective_enabled(config, surface);
        let resolved = config.for_surface(surface);
        let slot = &self.previews[index];
        let preview_ready = enabled && slot.status == PreviewStatus::Ready;
        let image = if preview_ready {
            slot.image
                .clone()
                .unwrap_or_else(|| self.defaults[index].clone())
        } else {
            self.defaults[index].clone()
        };
        let error = slot.error.clone();
        let loading =
            enabled && matches!(slot.status, PreviewStatus::Empty | PreviewStatus::Loading);
        let hovered = self.hovered_surface == Some(surface);
        let dense = region.width < 280. || region.height < 180.;
        let accent = theme::surface_accent(surface);
        let inset = (region.width.min(region.height) * 0.025).clamp(4., 12.);

        let (status_label, status_fg, status_bg) = if enabled && error.is_some() {
            ("准备失败", theme::danger(), theme::danger_soft())
        } else if loading {
            ("准备中", theme::accent(), theme::accent_soft())
        } else {
            card_status(config, surface, enabled)
        };
        let root = config.binding_root(surface).unwrap_or(surface);
        let source_line = if root != surface {
            format!("绑定到 {}", theme::surface_title(root))
        } else {
            format!(
                "{} · {}",
                theme::source_label(resolved.source),
                theme::fit_label(resolved.image.fit)
            )
        };
        let description = if !enabled {
            "组件未启用，点击打开配置。".to_owned()
        } else if loading {
            "正在准备媒体预览…".to_owned()
        } else if let Some(error) = &error {
            format!("准备失败：{error}")
        } else {
            theme::surface_description(surface).to_owned()
        };
        let meta = if dense {
            format!("{status_label} · {source_line}")
        } else {
            source_line
        };

        // The region frame owns only geometry and interaction. It has no
        // background, so the page's solid colour is the background of every
        // cell; the image module below simply centres itself in this box.
        let module = div()
            .id(SharedString::from(format!(
                "surface-module-{}",
                surface.name()
            )))
            .absolute()
            .left(px(region.x))
            .top(px(region.y))
            .w(px(region.width))
            .h(px(region.height))
            .overflow_hidden()
            .cursor_pointer()
            .on_hover(cx.listener(move |this, entered, _, cx| {
                let next = if *entered {
                    Some(surface)
                } else if this.hovered_surface == Some(surface) {
                    None
                } else {
                    this.hovered_surface
                };
                if this.hovered_surface != next {
                    this.hovered_surface = next;
                    cx.notify();
                }
            }))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.open_editor(surface, window, cx);
            }))
            .child(
                div()
                    .absolute()
                    .top(px(inset))
                    .left(px(inset))
                    .right(px(inset))
                    .bottom(px(inset))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        app::image_element(image, false)
                            .size_full()
                            .object_fit(ObjectFit::Contain)
                            .grayscale(!enabled)
                            .opacity(if enabled { 1. } else { 0.55 }),
                    ),
            )
            .when(!enabled, |this| this.child(disabled_overlay(surface)))
            .when(loading, |this| this.child(loading_overlay()))
            .when(enabled && error.is_some(), |this| {
                this.child(error_overlay())
            })
            .when(hovered, |this| {
                this.child(
                    div()
                        .absolute()
                        .inset(px(1.))
                        .border_1()
                        .border_color(theme::accent().opacity(0.4)),
                )
                .child(info_panel(
                    theme::surface_icon(surface),
                    theme::surface_title(surface),
                    Some((status_label, status_fg, status_bg)),
                    &description,
                    &meta,
                    "打开配置",
                    accent,
                    dense,
                ))
            });

        module.into_any_element()
    }
}

fn grid_lines(width: f32, height: f32, cell_width: f32, top_height: f32) -> AnyElement {
    let mut lines = Vec::with_capacity(4);
    for column in 1..GRID_COLUMNS as usize {
        lines.push(
            div()
                .absolute()
                .top_0()
                .left(px(column as f32 * cell_width))
                .w(px(1.))
                .h(px(top_height))
                .bg(theme::line_strong())
                .into_any_element(),
        );
    }
    for column in 1..3 {
        lines.push(
            div()
                .absolute()
                .top(px(top_height))
                .left(px(column as f32 * width / 3.))
                .w(px(1.))
                .h(px(height - top_height))
                .bg(theme::line_strong())
                .into_any_element(),
        );
    }
    lines.push(
        div()
            .absolute()
            .top(px(top_height))
            .left_0()
            .w(px(width))
            .h(px(1.))
            .bg(theme::line_strong())
            .into_any_element(),
    );

    div()
        .absolute()
        .inset_0()
        .children(lines)
        .into_any_element()
}

fn card_status(
    _config: &iidxbg::config::Config,
    _surface: Surface,
    enabled: bool,
) -> (&'static str, Hsla, Hsla) {
    if enabled {
        ("ON", theme::success(), theme::success_soft())
    } else {
        ("OFF", theme::muted_ink(), theme::card_bg())
    }
}

#[allow(clippy::too_many_arguments)]
fn info_panel(
    icon: IconName,
    title: &str,
    status: Option<(&str, Hsla, Hsla)>,
    description: &str,
    meta: &str,
    action: &str,
    accent: Hsla,
    dense: bool,
) -> AnyElement {
    let margin = if dense { 8. } else { 10. };
    let padding = if dense { 8. } else { 10. };

    v_flex()
        .absolute()
        .left(px(margin))
        .right(px(margin))
        .bottom(px(margin))
        .p(px(padding))
        .gap_1()
        .rounded(px(6.))
        .bg(theme::hover_panel_bg())
        .border_1()
        .border_color(theme::accent().opacity(0.38))
        .child(
            h_flex()
                .w_full()
                .items_center()
                .gap_2()
                .child(
                    Icon::new(icon)
                        .with_size(if dense { Size::XSmall } else { Size::Small })
                        .text_color(accent),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .text_ellipsis()
                        .text_size(px(if dense { 12. } else { 14. }))
                        .font_semibold()
                        .text_color(theme::ink())
                        .child(title.to_owned()),
                )
                .when_some(status.filter(|_| !dense), |this, (label, fg, bg)| {
                    this.child(widgets::status_pill(label, fg, bg))
                }),
        )
        .child(
            div()
                .w_full()
                .overflow_hidden()
                .text_size(px(if dense { 10. } else { 12. }))
                .line_height(relative(1.35))
                .text_color(theme::muted_ink())
                .when(!dense, |this| this.max_h(px(34.)))
                .child(description.to_owned()),
        )
        .child(
            h_flex()
                .w_full()
                .items_center()
                .justify_between()
                .gap_2()
                .text_size(px(if dense { 9. } else { 11. }))
                .child(
                    div()
                        .min_w_0()
                        .overflow_hidden()
                        .text_ellipsis()
                        .text_color(theme::faint_ink())
                        .child(meta.to_owned()),
                )
                .child(
                    h_flex()
                        .flex_shrink_0()
                        .items_center()
                        .gap_1()
                        .font_medium()
                        .text_color(accent)
                        .child(action.to_owned())
                        .child(Icon::new(IconName::ChevronRight).with_size(Size::XSmall)),
                ),
        )
        .into_any_element()
}

fn disabled_overlay(surface: Surface) -> AnyElement {
    v_flex()
        .absolute()
        .inset_0()
        .items_center()
        .justify_center()
        .gap_2()
        .child(
            Icon::new(theme::surface_icon(surface))
                .with_size(Size::Size(px(42.)))
                .text_color(theme::surface_accent(surface).opacity(0.4)),
        )
        .child(
            Icon::new(IconName::EyeOff)
                .with_size(Size::Small)
                .text_color(theme::faint_ink().opacity(0.9)),
        )
        .into_any_element()
}

fn loading_overlay() -> AnyElement {
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .child(
            Spinner::new()
                .with_size(Size::Medium)
                .color(theme::accent()),
        )
        .into_any_element()
}

fn error_overlay() -> AnyElement {
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .child(
            Icon::new(IconName::TriangleAlert)
                .with_size(Size::Medium)
                .text_color(theme::danger()),
        )
        .into_any_element()
}
