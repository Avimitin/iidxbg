use gpui::{
    AnyElement, Hsla, IntoElement, ParentElement, SharedString, Styled, div,
    prelude::FluentBuilder as _, px, relative,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    Icon, Selectable, Sizable, Size, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
};
use gpui_kit::*;

use crate::theme;

pub fn status_pill(
    label: impl Into<SharedString>,
    foreground: Hsla,
    background: Hsla,
) -> AnyElement {
    h_flex()
        .px_2()
        .h(px(26.))
        .items_center()
        .justify_center()
        .bg(background)
        .border_1()
        .border_color(foreground.opacity(0.35))
        .text_color(foreground)
        .text_size(px(13.))
        .font_medium()
        .child(label.into())
        .into_any_element()
}

pub fn icon_box(icon: IconName, color: Hsla, size: f32) -> AnyElement {
    div()
        .size(px(size))
        .flex_shrink_0()
        .bg(theme::page_bg())
        .border_1()
        .border_color(color.opacity(0.32))
        .flex()
        .items_center()
        .justify_center()
        .child(Icon::new(icon).with_size(Size::Small).text_color(color))
        .into_any_element()
}

pub fn segment_button(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    selected: bool,
) -> Button {
    Button::new(id)
        .large()
        .ghost()
        .label(label)
        .selected(selected)
        .when(selected, |this| {
            this.bg(theme::page_bg())
                .border_1()
                .border_color(theme::accent().opacity(0.8))
                .text_color(theme::accent())
        })
}

pub fn navigation_button(
    id: &'static str,
    icon: IconName,
    label: &'static str,
    selected: bool,
) -> Button {
    let color = if selected {
        theme::accent()
    } else {
        theme::muted_ink()
    };
    Button::new(id)
        .ghost()
        .with_size(Size::Size(px(56.)))
        .w(px(56.))
        .selected(selected)
        .text_color(color)
        .icon(
            Icon::new(icon)
                .with_size(Size::Size(px(24.)))
                .text_color(color),
        )
        .tooltip(label)
}

pub fn note(
    icon: IconName,
    text: impl Into<SharedString>,
    foreground: Hsla,
    background: Hsla,
) -> AnyElement {
    h_flex()
        .w_full()
        .items_start()
        .gap_3()
        .p_4()
        .bg(background)
        .border_1()
        .border_color(foreground.opacity(0.3))
        .text_color(foreground)
        .child(
            Icon::new(icon)
                .with_size(Size::XSmall)
                .text_color(foreground),
        )
        .child(
            div()
                .flex_1()
                .text_sm()
                .line_height(relative(1.5))
                .child(text.into()),
        )
        .into_any_element()
}

pub fn error_banner(message: impl Into<SharedString>) -> AnyElement {
    note(
        IconName::TriangleAlert,
        message,
        theme::danger(),
        theme::danger_soft(),
    )
}

pub fn close_button(id: impl Into<gpui::ElementId>) -> Button {
    Button::new(id)
        .ghost()
        .large()
        .icon(Icon::new(IconName::X).with_size(Size::Small))
}

pub fn muted_path(text: impl Into<SharedString>) -> AnyElement {
    div()
        .w_full()
        .px_4()
        .py_3()
        .border_1()
        .border_color(theme::line_soft())
        .bg(theme::page_bg())
        .text_base()
        .text_color(theme::muted_ink())
        .overflow_hidden()
        .child(
            div()
                .w_full()
                .overflow_hidden()
                .text_ellipsis()
                .child(text.into()),
        )
        .into_any_element()
}
