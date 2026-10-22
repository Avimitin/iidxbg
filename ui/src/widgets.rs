use gpui::{
    AnyElement, Hsla, IntoElement, ParentElement, SharedString, Styled, div,
    prelude::FluentBuilder as _, px, relative,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Icon, Selectable, Sizable, Size, StyledExt,
    button::{Button, ButtonVariants},
    h_flex, v_flex,
};
use gpui_kit::*;

use crate::theme;

pub fn panel(
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    children: impl IntoIterator<Item = AnyElement>,
) -> AnyElement {
    v_flex()
        .w_full()
        .p_5()
        .gap_4()
        .bg(theme::card_bg())
        .border_1()
        .border_color(theme::line())
        .rounded(px(12.0))
        .overflow_hidden()
        .child(panel_header(title, description, None))
        .child(v_flex().w_full().gap_0().children(children))
        .into_any_element()
}

pub fn panel_with_right(
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    right: impl IntoElement,
    children: impl IntoIterator<Item = AnyElement>,
) -> AnyElement {
    v_flex()
        .w_full()
        .p_5()
        .gap_4()
        .bg(theme::card_bg())
        .border_1()
        .border_color(theme::line())
        .rounded(px(12.0))
        .overflow_hidden()
        .child(panel_header(
            title,
            description,
            Some(right.into_any_element()),
        ))
        .child(v_flex().w_full().gap_0().children(children))
        .into_any_element()
}

pub fn panel_header(
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    right: Option<AnyElement>,
) -> AnyElement {
    h_flex()
        .w_full()
        .justify_between()
        .items_start()
        .gap_4()
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_1()
                .child(
                    div()
                        .text_base()
                        .font_semibold()
                        .text_color(theme::ink())
                        .child(title.into()),
                )
                .child(
                    div()
                        .text_sm()
                        .line_height(relative(1.45))
                        .text_color(theme::muted_ink())
                        .child(description.into()),
                ),
        )
        .when_some(right, |this, right| this.child(right))
        .into_any_element()
}

pub fn setting_row(
    label: impl Into<SharedString>,
    help: impl Into<SharedString>,
    control: impl IntoElement,
) -> AnyElement {
    h_flex()
        .w_full()
        .justify_between()
        .items_start()
        .gap_5()
        .py_3()
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .font_medium()
                        .text_color(theme::ink())
                        .child(label.into()),
                )
                .child(
                    div()
                        .text_xs()
                        .line_height(relative(1.5))
                        .text_color(theme::muted_ink())
                        .child(help.into()),
                ),
        )
        .child(div().w(px(300.)).flex_shrink_0().child(control))
        .into_any_element()
}

pub fn setting_row_stacked(
    label: impl Into<SharedString>,
    help: impl Into<SharedString>,
    control: impl IntoElement,
) -> AnyElement {
    v_flex()
        .w_full()
        .gap_2()
        .py_3()
        .child(
            v_flex()
                .w_full()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .font_medium()
                        .text_color(theme::ink())
                        .child(label.into()),
                )
                .child(
                    div()
                        .w_full()
                        .text_xs()
                        .line_height(relative(1.5))
                        .text_color(theme::muted_ink())
                        .child(help.into()),
                ),
        )
        .child(div().w_full().child(control))
        .into_any_element()
}

pub fn divider() -> AnyElement {
    div()
        .w_full()
        .h(px(1.))
        .bg(theme::line_soft())
        .into_any_element()
}

/// Interleave hairlines between rows without forcing every caller to append
/// them by hand.
pub fn rows(rows: Vec<AnyElement>) -> Vec<AnyElement> {
    let mut output = Vec::with_capacity(rows.len() * 2);
    for (index, row) in rows.into_iter().enumerate() {
        if index > 0 {
            output.push(divider());
        }
        output.push(row);
    }
    output
}

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
        .bg(color.opacity(0.1))
        .flex()
        .items_center()
        .justify_center()
        .child(Icon::new(icon).with_size(Size::Small).text_color(color))
        .into_any_element()
}

pub fn segmented(buttons: Vec<Button>, cx: &App) -> AnyElement {
    h_flex()
        .p_1()
        .gap_1()
        .rounded(px(9.))
        .bg(cx.theme().colors.muted)
        .border_1()
        .border_color(theme::line_soft())
        .children(buttons)
        .into_any_element()
}

pub fn segmented_wrap(buttons: Vec<Button>, cx: &App) -> AnyElement {
    h_flex()
        .p_1()
        .gap_1()
        .flex_wrap()
        .rounded(px(9.))
        .bg(cx.theme().colors.muted)
        .border_1()
        .border_color(theme::line_soft())
        .children(buttons)
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
