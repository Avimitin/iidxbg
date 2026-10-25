use gpui::{
    AnyElement, Hsla, IntoElement, ParentElement, SharedString, Styled, div,
    prelude::FluentBuilder as _, px, relative,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    Icon, Selectable, Sizable, Size,
    button::{Button, ButtonVariants},
    h_flex,
};
use gpui_kit::*;

use crate::theme;

/// Buttons keep their background clear in normal, hover, pressed and disabled states.
pub fn button(id: impl Into<gpui::ElementId>) -> Button {
    Button::new(id).ghost()
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
    button(id)
        .large()
        .ghost()
        .label(label)
        .selected(selected)
        .when(selected, |this| {
            this.border_1()
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
    button(id)
        .text()
        .with_size(Size::Size(px(40.)))
        .w(px(40.))
        .text_color(color)
        .icon(
            Icon::new(icon)
                .with_size(Size::Size(px(20.)))
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
    button(id)
        .ghost()
        .large()
        .icon(Icon::new(IconName::X).with_size(Size::Small))
}

pub fn help(id: impl Into<gpui::ElementId>, text: impl Into<SharedString>) -> Button {
    button(id)
        .ghost()
        .small()
        .icon(IconName::Info)
        .text_color(theme::muted_ink())
        .tooltip(text)
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
