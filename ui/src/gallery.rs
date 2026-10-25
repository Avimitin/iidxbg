use gpui::{
    AnyElement, IntoElement, ObjectFit, ParentElement, SharedString, Styled, Window,
    prelude::FluentBuilder as _, px,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::component::{Icon, Sizable, Size, spinner::Spinner, v_flex};
use gpui_kit::*;
use iidxbg::config::Surface;

use crate::{
    app::{self, AppView, PreviewStatus},
    store, theme,
};

const PREVIEW_INSET: f32 = 12.;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Region {
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
}

impl AppView {
    pub(crate) fn render_gallery(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let width = (window.viewport_size().width.as_f32() - theme::SIDEBAR_WIDTH).max(240.);
        let mut placed: Vec<Region> = Vec::new();
        let mut modules = Vec::with_capacity(Surface::COUNT);
        // Unequal panel widths form one adjoining board, not rows, columns or
        // detached cards. A panel sits against the nearest available edge.
        let widths = [
            0.44, 0.56, 0.22, 0.24, 0.20, 0.40, 0.46, 0.60, 0.56, 0.44, 0.48, 0.52, 0.48, 0.55,
            0.44, 0.50,
        ];
        for surface in Surface::ALL {
            if !self.store.accepts_surface(surface) {
                continue;
            }
            let (w, h) = theme::surface_size(self.store.config(), surface);
            let panel_width = (width * widths[surface.index()]).max(150.).min(width);
            let panel_height = (panel_width - PREVIEW_INSET * 2.) * h as f32 / w.max(1) as f32
                + PREVIEW_INSET * 2.;
            let region = adjacent_region(&placed, width, panel_width, panel_height);
            placed.push(region);
            modules.push(self.render_surface_module(surface, region, window, cx));
        }
        let height = placed.iter().map(|r| r.y + r.height).fold(0_f32, f32::max);
        div()
            .id("gallery-board")
            .size_full()
            .overflow_y_scrollbar()
            .bg(theme::page_bg())
            .child(div().relative().w_full().h(px(height)).children(modules))
            .into_any_element()
    }

    pub(crate) fn render_surface_module(
        &mut self,
        surface: Surface,
        region: Region,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let config = self.store.config();
        let index = surface.index();
        let enabled = store::effective_enabled(config, surface);
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
        let dense = region.width < 280. || region.height < 180.;
        let inset = PREVIEW_INSET;

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
            .border_1()
            .border_color(theme::line())
            .overflow_hidden()
            .cursor_pointer()
            .hover(|style| style.border_color(theme::accent().opacity(0.4)))
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
            .child(module_label(theme::surface_title(surface), dense));

        module.into_any_element()
    }
}

/// Offset glyph copies provide a black outline and a drop shadow even on
/// bright media. GPUI's text style does not expose a glyph stroke/shadow.
fn module_label(title: &str, dense: bool) -> AnyElement {
    let label = || {
        div()
            .absolute()
            .w_full()
            .text_ellipsis()
            .overflow_hidden()
            .text_size(px(if dense { 11. } else { 12. }))
            .font_weight(gpui::FontWeight::LIGHT)
            .child(title.to_owned())
    };
    let mut outline = div()
        .absolute()
        .top(px(10.))
        .left(px(12.))
        .right(px(12.))
        .h(px(26.));
    for (x, y, alpha) in [
        (1., 2., 0.4),
        (2., 3., 0.15),
        (-0.5, -0.5, 0.85),
        (0., -0.5, 0.85),
        (0.5, -0.5, 0.85),
        (-0.5, 0., 0.85),
        (0.5, 0., 0.85),
        (-0.5, 0.5, 0.85),
        (0., 0.5, 0.85),
        (0.5, 0.5, 0.85),
    ] {
        outline = outline.child(
            label()
                .left(px(x))
                .top(px(y))
                .text_color(gpui::black().opacity(alpha)),
        );
    }
    outline
        .child(label().text_color(gpui::white()))
        .into_any_element()
}

/// Fill the lowest exposed edge of the board. Neighbours can have different
/// widths and heights, but no empty pocket is left between their frames.
pub(crate) fn adjacent_region(
    placed: &[Region],
    board_width: f32,
    width: f32,
    height: f32,
) -> Region {
    let mut edges = vec![0., board_width];
    for region in placed {
        edges.extend([region.x, region.x + region.width]);
    }
    edges.sort_by(f32::total_cmp);
    edges.dedup_by(|a, b| (*a - *b).abs() < 0.01);
    let mut skyline: Vec<(f32, f32, f32)> = Vec::new();
    for pair in edges.windows(2) {
        let middle = (pair[0] + pair[1]) / 2.;
        let top = placed
            .iter()
            .filter(|r| middle >= r.x && middle < r.x + r.width)
            .map(|r| r.y + r.height)
            .fold(0_f32, f32::max);
        if let Some(last) = skyline.last_mut()
            && (last.2 - top).abs() < 0.01
        {
            last.1 = pair[1];
        } else {
            skyline.push((pair[0], pair[1], top));
        }
    }
    let (left, right, top) = skyline
        .into_iter()
        .min_by(|a, b| a.2.total_cmp(&b.2).then(a.0.total_cmp(&b.0)))
        .expect("board always has an exposed edge");
    let available = right - left;
    let actual_width = if available - width < 120. {
        available
    } else {
        width
    };
    let actual_height = (actual_width - PREVIEW_INSET * 2.) * (height - PREVIEW_INSET * 2.)
        / (width - PREVIEW_INSET * 2.)
        + PREVIEW_INSET * 2.;
    Region {
        x: left,
        y: top,
        width: actual_width,
        height: actual_height,
    }
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
