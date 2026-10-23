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
        .h(px(22.))
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(background)
        .border_1()
        .border_color(foreground.opacity(0.35))
        .text_color(foreground)
        .text_size(px(11.))
        .font_medium()
        .child(label.into())
        .into_any_element()
}

pub fn dot(color: Hsla, size: f32) -> AnyElement {
    div()
        .size(px(size))
        .flex_shrink_0()
        .rounded_full()
        .bg(color)
        .into_any_element()
}

pub fn icon_box(icon: IconName, color: Hsla, size: f32) -> AnyElement {
    div()
        .size(px(size))
        .flex_shrink_0()
        .rounded(px(size * 0.28))
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
        .small()
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

pub fn note(
    icon: IconName,
    text: impl Into<SharedString>,
    foreground: Hsla,
    background: Hsla,
) -> AnyElement {
    h_flex()
        .w_full()
        .items_start()
        .gap_2()
        .p_3()
        .rounded(px(10.))
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
                .text_xs()
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
        .small()
        .icon(Icon::new(IconName::X).with_size(Size::Small))
}

pub fn muted_path(text: impl Into<SharedString>) -> AnyElement {
    div()
        .w_full()
        .px_3()
        .py_2()
        .rounded(px(8.))
        .border_1()
        .border_color(theme::line_soft())
        .bg(theme::page_bg())
        .text_sm()
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
