use gpui::{
    AnyElement, Entity, IntoElement, MouseButton, ParentElement, SharedString, Styled,
    Subscription, Window, prelude::FluentBuilder as _, px, relative,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    Disableable, Icon, Sizable, Size, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState},
    scroll::ScrollableElement,
    switch::Switch,
    v_flex,
};
use gpui_kit::*;
use iidxbg::config::{Config, Fit, MoebooruDomain, Source, Surface};

use crate::{
    app::{self, AppView, PreviewStatus},
    store, theme, widgets,
};

fn element_id(prefix: &str, suffix: impl std::fmt::Display) -> SharedString {
    SharedString::from(format!("{prefix}-{suffix}"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SurfaceTextField {
    LocalDirectory,
    Tags,
    SaveDirectory,
    PageMax,
    TimeoutSeconds,
    MaxImages,
}

pub(crate) struct SurfaceInputs {
    pub local_directory: Entity<InputState>,
    pub tags: Entity<InputState>,
    pub save_directory: Entity<InputState>,
    pub page_max: Entity<InputState>,
    pub timeout_seconds: Entity<InputState>,
    pub max_images: Entity<InputState>,
}

pub(crate) struct SurfaceEditor {
    pub surface: Surface,
    pub inputs: SurfaceInputs,
    subscriptions: Vec<Subscription>,
    pub error: Option<SharedString>,
    pub advanced_open: bool,
}

impl SurfaceEditor {
    pub fn new(
        surface: Surface,
        config: &Config,
        window: &mut Window,
        cx: &mut Context<AppView>,
    ) -> Self {
        let resolved = config.for_surface(surface);
        let source = app::surface_override(config, surface);
        let local_directory = source
            .local_directory
            .as_ref()
            .unwrap_or(&resolved.local_directory)
            .to_string_lossy()
            .into_owned();
        let tags = source
            .moebooru
            .tags
            .as_ref()
            .unwrap_or(&resolved.moebooru.tags)
            .clone();
        let save_directory = source
            .moebooru
            .save_directory
            .as_ref()
            .unwrap_or(&resolved.moebooru.save_directory)
            .to_string_lossy()
            .into_owned();
        let page_max = source
            .moebooru
            .page_max
            .unwrap_or(resolved.moebooru.page_max)
            .to_string();
        let timeout_seconds = source
            .moebooru
            .timeout_seconds
            .unwrap_or(resolved.moebooru.timeout_seconds)
            .to_string();
        let max_images = source
            .moebooru
            .max_images
            .unwrap_or(resolved.moebooru.max_images)
            .to_string();
        let advanced_open = source.source.is_some_and(|source| source != Source::Local)
            || source.moebooru.domain.is_some()
            || source.moebooru.tags.is_some()
            || source.moebooru.save_directory.is_some()
            || source.moebooru.page_max.is_some()
            || source.moebooru.timeout_seconds.is_some()
            || source.moebooru.max_images.is_some();

        let inputs = SurfaceInputs {
            local_directory: text_input(&local_directory, "例如 iidxbg/results", window, cx),
            tags: text_input(&tags, "继承全局标签", window, cx),
            save_directory: text_input(&save_directory, "继承全局目录", window, cx),
            page_max: text_input(&page_max, "5", window, cx),
            timeout_seconds: text_input(&timeout_seconds, "20", window, cx),
            max_images: text_input(&max_images, "0", window, cx),
        };

        let mut editor = Self {
            surface,
            inputs,
            subscriptions: Vec::new(),
            error: None,
            advanced_open,
        };
        for (field, input) in [
            (
                SurfaceTextField::LocalDirectory,
                &editor.inputs.local_directory,
            ),
            (SurfaceTextField::Tags, &editor.inputs.tags),
            (
                SurfaceTextField::SaveDirectory,
                &editor.inputs.save_directory,
            ),
            (SurfaceTextField::PageMax, &editor.inputs.page_max),
            (
                SurfaceTextField::TimeoutSeconds,
                &editor.inputs.timeout_seconds,
            ),
            (SurfaceTextField::MaxImages, &editor.inputs.max_images),
        ] {
            let subscription =
                cx.subscribe_in(input, window, move |this, state, event, _window, cx| {
                    if matches!(event, InputEvent::Change) {
                        let value = state.read(cx).value().to_string();
                        this.on_surface_text(surface, field, value, cx);
                    }
                });
            editor.subscriptions.push(subscription);
        }
        editor
    }

    pub fn set_inputs_from_config(&self, config: &Config, window: &mut Window, cx: &mut App) {
        let resolved = config.for_surface(self.surface);
        let source = app::surface_override(config, self.surface);
        set_input(
            &self.inputs.local_directory,
            source
                .local_directory
                .as_ref()
                .unwrap_or(&resolved.local_directory)
                .to_string_lossy(),
            window,
            cx,
        );
        set_input(
            &self.inputs.tags,
            source
                .moebooru
                .tags
                .as_ref()
                .unwrap_or(&resolved.moebooru.tags)
                .clone(),
            window,
            cx,
        );
        set_input(
            &self.inputs.save_directory,
            source
                .moebooru
                .save_directory
                .as_ref()
                .unwrap_or(&resolved.moebooru.save_directory)
                .to_string_lossy(),
            window,
            cx,
        );
        set_input(
            &self.inputs.page_max,
            source
                .moebooru
                .page_max
                .unwrap_or(resolved.moebooru.page_max)
                .to_string(),
            window,
            cx,
        );
        set_input(
            &self.inputs.timeout_seconds,
            source
                .moebooru
                .timeout_seconds
                .unwrap_or(resolved.moebooru.timeout_seconds)
                .to_string(),
            window,
            cx,
        );
        set_input(
            &self.inputs.max_images,
            source
                .moebooru
                .max_images
                .unwrap_or(resolved.moebooru.max_images)
                .to_string(),
            window,
            cx,
        );
    }

    pub(crate) fn input_for(&self, field: SurfaceTextField) -> &Entity<InputState> {
        match field {
            SurfaceTextField::LocalDirectory => &self.inputs.local_directory,
            SurfaceTextField::Tags => &self.inputs.tags,
            SurfaceTextField::SaveDirectory => &self.inputs.save_directory,
            SurfaceTextField::PageMax => &self.inputs.page_max,
            SurfaceTextField::TimeoutSeconds => &self.inputs.timeout_seconds,
            SurfaceTextField::MaxImages => &self.inputs.max_images,
        }
    }
}

fn text_input(
    value: &str,
    placeholder: &str,
    window: &mut Window,
    cx: &mut Context<AppView>,
) -> Entity<InputState> {
    let value = value.to_owned();
    let placeholder = placeholder.to_owned();
    cx.new(move |cx| {
        InputState::new(window, cx)
            .default_value(value)
            .placeholder(placeholder)
    })
}

fn set_input(
    input: &Entity<InputState>,
    value: impl Into<SharedString>,
    window: &mut Window,
    cx: &mut App,
) {
    input.update(cx, |state, cx| state.set_value(value.into(), window, cx));
}

fn text_path(surface: Surface, field: SurfaceTextField) -> Vec<&'static str> {
    let mut path = vec!["sources", surface.name()];
    match field {
        SurfaceTextField::LocalDirectory => path.push("local_directory"),
        SurfaceTextField::Tags => {
            path.push("moebooru");
            path.push("tags");
        }
        SurfaceTextField::SaveDirectory => {
            path.push("moebooru");
            path.push("save_directory");
        }
        SurfaceTextField::PageMax => {
            path.push("moebooru");
            path.push("page_max");
        }
        SurfaceTextField::TimeoutSeconds => {
            path.push("moebooru");
            path.push("timeout_seconds");
        }
        SurfaceTextField::MaxImages => {
            path.push("moebooru");
            path.push("max_images");
        }
    }
    path
}

fn source_path(surface: Surface) -> Vec<&'static str> {
    vec!["sources", surface.name(), "source"]
}

fn fit_path(surface: Surface) -> Vec<&'static str> {
    vec!["sources", surface.name(), "fit"]
}

fn recursive_path(surface: Surface) -> Vec<&'static str> {
    vec!["sources", surface.name(), "recursive"]
}

fn domain_path(surface: Surface) -> Vec<&'static str> {
    vec!["sources", surface.name(), "moebooru", "domain"]
}

impl AppView {
    pub(crate) fn on_surface_text(
        &mut self,
        surface: Surface,
        field: SurfaceTextField,
        value: String,
        cx: &mut Context<Self>,
    ) {
        let value = value.trim_end_matches(['\r', '\n']).to_owned();
        let invalid = |this: &mut Self, message: &str, cx: &mut Context<Self>| {
            if let Some(editor) = &mut this.editor {
                editor.error = Some(message.to_owned().into());
            }
            cx.notify();
        };
        match field {
            SurfaceTextField::LocalDirectory => {
                if value.trim().is_empty() {
                    return invalid(self, "本地目录不能为空", cx);
                }
                self.commit_surface(500, cx, |doc| {
                    store::set_string(doc, &text_path(surface, field), &value);
                });
            }
            SurfaceTextField::Tags => {
                if value.chars().count() > 1000 {
                    return invalid(self, "标签长度不能超过 1000 个字符", cx);
                }
                self.commit_surface(500, cx, |doc| {
                    store::set_string(doc, &text_path(surface, field), &value);
                });
            }
            SurfaceTextField::SaveDirectory => {
                if value.trim().is_empty() {
                    return invalid(self, "下载目录不能为空", cx);
                }
                self.commit_surface(500, cx, |doc| {
                    store::set_string(doc, &text_path(surface, field), &value);
                });
            }
            SurfaceTextField::PageMax => {
                let Some(number) = self.parse_surface_number(&value, "页数上限", 1, 100, cx)
                else {
                    return;
                };
                self.commit_surface(500, cx, |doc| {
                    store::set_u32(doc, &text_path(surface, field), number as u32);
                });
            }
            SurfaceTextField::TimeoutSeconds => {
                let Some(number) = self.parse_surface_number(&value, "请求超时", 3, 60, cx)
                else {
                    return;
                };
                self.commit_surface(500, cx, |doc| {
                    store::set_u64(doc, &text_path(surface, field), number);
                });
            }
            SurfaceTextField::MaxImages => {
                let Some(number) =
                    self.parse_surface_number(&value, "下载保留上限", 0, 1_000_000, cx)
                else {
                    return;
                };
                self.commit_surface(500, cx, |doc| {
                    store::set_usize(doc, &text_path(surface, field), number as usize);
                });
            }
        }
    }

    fn parse_surface_number(
        &mut self,
        value: &str,
        label: &str,
        min: u64,
        max: u64,
        cx: &mut Context<Self>,
    ) -> Option<u64> {
        let parsed = value.trim().parse::<u64>().ok();
        let Some(number) = parsed else {
            if let Some(editor) = &mut self.editor {
                editor.error = Some(format!("{label}必须是数字").into());
            }
            cx.notify();
            return None;
        };
        if !(min..=max).contains(&number) {
            if let Some(editor) = &mut self.editor {
                editor.error = Some(format!("{label}必须在 {min}–{max} 之间").into());
            }
            cx.notify();
            return None;
        }
        Some(number)
    }

    pub(crate) fn set_surface_enabled(
        &mut self,
        surface: Surface,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        self.commit_surface(0, cx, |doc| {
            if surface == Surface::Results {
                store::set_bool(doc, &["enabled"], enabled);
            } else {
                store::set_bool(doc, &["surfaces", surface.name()], enabled);
            }
        });
    }

    pub(crate) fn set_surface_source(
        &mut self,
        surface: Surface,
        source: Option<Source>,
        cx: &mut Context<Self>,
    ) {
        self.commit_surface(0, cx, |doc| {
            let path = source_path(surface);
            match source {
                Some(source) => store::set_string(doc, &path, store::source_key(source)),
                None => {
                    store::remove_value(doc, &path);
                }
            }
        });
    }

    pub(crate) fn set_surface_fit(
        &mut self,
        surface: Surface,
        fit: Option<Fit>,
        cx: &mut Context<Self>,
    ) {
        self.commit_surface(0, cx, |doc| {
            let path = fit_path(surface);
            match fit {
                Some(fit) => store::set_string(doc, &path, store::fit_key(fit)),
                None => {
                    store::remove_value(doc, &path);
                }
            }
        });
    }

    pub(crate) fn set_surface_recursive(
        &mut self,
        surface: Surface,
        recursive: Option<bool>,
        cx: &mut Context<Self>,
    ) {
        self.commit_surface(0, cx, |doc| {
            let path = recursive_path(surface);
            match recursive {
                Some(value) => store::set_bool(doc, &path, value),
                None => {
                    store::remove_value(doc, &path);
                }
            }
        });
    }

    pub(crate) fn set_surface_domain(
        &mut self,
        surface: Surface,
        domain: Option<MoebooruDomain>,
        cx: &mut Context<Self>,
    ) {
        self.commit_surface(0, cx, |doc| {
            let path = domain_path(surface);
            match domain {
                Some(domain) => store::set_string(doc, &path, store::domain_key(domain)),
                None => {
                    store::remove_value(doc, &path);
                }
            }
        });
    }

    pub(crate) fn begin_surface_override(
        &mut self,
        surface: Surface,
        field: SurfaceTextField,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let config = self.store.config().clone();
        let value = effective_text(&config, surface, field);
        let path = text_path(surface, field);
        let ok = self.commit_surface(0, cx, |doc| match field {
            SurfaceTextField::PageMax => {
                if let Ok(number) = value.parse::<u32>() {
                    store::set_u32(doc, &path, number);
                }
            }
            SurfaceTextField::TimeoutSeconds => {
                if let Ok(number) = value.parse::<u64>() {
                    store::set_u64(doc, &path, number);
                }
            }
            SurfaceTextField::MaxImages => {
                if let Ok(number) = value.parse::<usize>() {
                    store::set_usize(doc, &path, number);
                }
            }
            _ => store::set_string(doc, &path, &value),
        });
        if ok {
            let value = effective_text(self.store.config(), surface, field);
            if let Some(editor) = &self.editor {
                set_input(editor.input_for(field), value, window, cx);
            }
        }
    }

    pub(crate) fn clear_surface_override(
        &mut self,
        surface: Surface,
        field: SurfaceTextField,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let path = text_path(surface, field);
        let ok = self.commit_surface(0, cx, |doc| {
            store::remove_value(doc, &path);
        });
        if ok {
            let value = effective_text(self.store.config(), surface, field);
            if let Some(editor) = &self.editor {
                set_input(editor.input_for(field), value, window, cx);
            }
        }
    }

    pub(crate) fn begin_surface_switch(&mut self, surface: Surface, cx: &mut Context<Self>) {
        let recursive = self.store.config().for_surface(surface).recursive;
        self.set_surface_recursive(surface, Some(recursive), cx);
    }

    pub(crate) fn make_concentration_independent(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ok = self.commit_surface(0, cx, |doc| {
            store::set_string(doc, &["sources", "concentration", "source"], "local");
            store::set_string(
                doc,
                &["sources", "concentration", "local_directory"],
                "iidxbg/concentration",
            );
        });
        if ok {
            if let Some(editor) = &mut self.editor {
                editor.advanced_open = false;
            }
            let config = self.store.config().clone();
            if let Some(editor) = &self.editor {
                editor.set_inputs_from_config(&config, window, cx);
            }
        }
    }

    pub(crate) fn restore_concentration_shared(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ok = self.commit_surface(0, cx, |doc| {
            for key in [
                "source",
                "local_directory",
                "recursive",
                "fit",
                "domain",
                "tags",
                "save_directory",
                "page_max",
                "timeout_seconds",
                "max_images",
            ] {
                let path = if matches!(
                    key,
                    "domain"
                        | "tags"
                        | "save_directory"
                        | "page_max"
                        | "timeout_seconds"
                        | "max_images"
                ) {
                    vec!["sources", "concentration", "moebooru", key]
                } else {
                    vec!["sources", "concentration", key]
                };
                store::remove_value(doc, &path);
            }
        });
        if ok {
            let config = self.store.config().clone();
            if let Some(editor) = &self.editor {
                editor.set_inputs_from_config(&config, window, cx);
            }
        }
    }
}

fn effective_text(config: &Config, surface: Surface, field: SurfaceTextField) -> String {
    let resolved = config.for_surface(surface);
    let source = app::surface_override(config, surface);
    match field {
        SurfaceTextField::LocalDirectory => source
            .local_directory
            .as_ref()
            .unwrap_or(&resolved.local_directory)
            .to_string_lossy()
            .into_owned(),
        SurfaceTextField::Tags => source
            .moebooru
            .tags
            .as_ref()
            .unwrap_or(&resolved.moebooru.tags)
            .clone(),
        SurfaceTextField::SaveDirectory => source
            .moebooru
            .save_directory
            .as_ref()
            .unwrap_or(&resolved.moebooru.save_directory)
            .to_string_lossy()
            .into_owned(),
        SurfaceTextField::PageMax => source
            .moebooru
            .page_max
            .unwrap_or(resolved.moebooru.page_max)
            .to_string(),
        SurfaceTextField::TimeoutSeconds => source
            .moebooru
            .timeout_seconds
            .unwrap_or(resolved.moebooru.timeout_seconds)
            .to_string(),
        SurfaceTextField::MaxImages => source
            .moebooru
            .max_images
            .unwrap_or(resolved.moebooru.max_images)
            .to_string(),
    }
}

pub(crate) fn render_editor(
    view: &mut AppView,
    editor: &SurfaceEditor,
    window: &mut Window,
    cx: &mut Context<AppView>,
) -> AnyElement {
    let surface = editor.surface;
    let config = view.store.config().clone();
    let resolved = config.for_surface(surface);
    let shares_premium = surface == Surface::Concentration && config.concentration_shares_premium();
    let enabled = store::effective_enabled(&config, surface);
    let switch_value = store::surface_switch_value(&config, surface);
    let switch_enabled = true;
    let viewport = window.viewport_size();
    let width = (viewport.width.as_f32() - 32.).clamp(480., 760.);
    let height = (viewport.height.as_f32() - 48.).clamp(360., 720.);
    let accent = theme::surface_accent(surface);

    let header = h_flex()
        .w_full()
        .justify_between()
        .items_start()
        .gap_4()
        .p_5()
        .bg(theme::card_bg())
        .border_b_1()
        .border_color(theme::line())
        .child(
            h_flex()
                .flex_1()
                .min_w_0()
                .gap_3()
                .items_start()
                .child(widgets::icon_box(theme::surface_icon(surface), accent, 44.))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_1()
                        .child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .text_xl()
                                        .font_semibold()
                                        .text_color(theme::ink())
                                        .child(theme::surface_title(surface)),
                                )
                                .child(widgets::status_pill(
                                    if surface == Surface::Results {
                                        "跟随全局"
                                    } else {
                                        surface.name()
                                    },
                                    theme::muted_ink(),
                                    theme::page_bg(),
                                )),
                        )
                        .child(
                            div()
                                .text_sm()
                                .line_height(relative(1.45))
                                .text_color(theme::muted_ink())
                                .child(theme::surface_description(surface)),
                        )
                        .child(
                            h_flex()
                                .items_center()
                                .gap_2()
                                .text_xs()
                                .text_color(theme::faint_ink())
                                .child(Icon::new(IconName::Monitor).with_size(Size::XSmall))
                                .child(format!(
                                    "{} · {} · {}",
                                    theme::format_dimensions(&config, surface),
                                    theme::ratio_label(&config, surface),
                                    theme::fit_label(resolved.image.fit)
                                )),
                        ),
                ),
        )
        .child(
            h_flex()
                .flex_shrink_0()
                .items_center()
                .gap_3()
                .child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(div().text_xs().text_color(theme::muted_ink()).child(
                            if surface == Surface::Results {
                                "全局总开关"
                            } else {
                                "启用"
                            },
                        ))
                        .child(
                            Switch::new("editor-enabled")
                                .checked(switch_value)
                                .color(theme::accent())
                                .disabled(!switch_enabled)
                                .on_change(cx.listener(move |this, checked, _, cx| {
                                    this.set_surface_enabled(surface, *checked, cx);
                                })),
                        ),
                )
                .child(
                    widgets::close_button("editor-close")
                        .on_click(cx.listener(|this, _, _, cx| this.close_editor(cx))),
                ),
        );

    let mut body = v_flex()
        .w_full()
        .p_4()
        .gap_4()
        .bg(theme::page_bg())
        .flex_1()
        .min_h_0()
        .overflow_y_scrollbar();

    if surface == Surface::Results {
        body = body.child(widgets::note(
            IconName::Info,
            "结算图没有独立开关，跟随全局总开关；关闭它会影响所有组件。",
            theme::accent(),
            theme::accent_soft(),
        ));
    } else if !config.enabled {
        body = body.child(widgets::note(
            IconName::TriangleAlert,
            "全局总开关当前关闭。这里可以预先启用，等全局开启后生效。",
            theme::warning(),
            theme::warning_soft(),
        ));
    }

    if let Some(error) = &editor.error {
        body = body.child(widgets::error_banner(error.clone()));
    }

    if shares_premium {
        body = body.child(concentration_share_panel(cx));
    } else {
        if surface == Surface::Concentration {
            body = body.child(concentration_independent_panel(cx));
        }
        body = body.child(source_panel(editor, &config, cx));
        body = body.child(moebooru_panel(editor, &config, cx));
    }

    body = body.child(preview_panel(view, &config, surface, enabled, cx));

    let footer_error = editor
        .error
        .clone()
        .or_else(|| view.store.error().map(|error| error.to_owned().into()));
    let footer = h_flex()
        .w_full()
        .justify_between()
        .items_center()
        .gap_4()
        .p_4()
        .px_5()
        .bg(theme::card_bg())
        .border_t_1()
        .border_color(theme::line())
        .child(
            h_flex()
                .flex_1()
                .min_w_0()
                .items_center()
                .gap_2()
                .child(widgets::dot(
                    if footer_error.is_some() {
                        theme::danger()
                    } else {
                        theme::success()
                    },
                    7.,
                ))
                .child(
                    div()
                        .text_xs()
                        .text_color(if footer_error.is_some() {
                            theme::danger()
                        } else {
                            theme::muted_ink()
                        })
                        .child(
                            footer_error
                                .map(|error| format!("未保存：{error}"))
                                .unwrap_or_else(|| {
                                    "修改会立即写入 iidxbg.toml，无需保存".to_owned()
                                }),
                        ),
                ),
        )
        .child(
            Button::new("editor-done")
                .small()
                .primary()
                .bg(theme::page_bg())
                .border_1()
                .border_color(theme::accent())
                .text_color(theme::accent())
                .hover(|this| {
                    this.bg(theme::page_bg())
                        .border_color(theme::accent_hover())
                        .text_color(theme::accent_hover())
                })
                .label("完成")
                .on_click(cx.listener(|this, _, _, cx| this.close_editor(cx))),
        );

    div()
        .id("surface-editor-overlay")
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .bg(theme::overlay())
        .flex()
        .items_center()
        .justify_center()
        .p(px(16.))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _, _, cx| this.close_editor(cx)),
        )
        .child(
            v_flex()
                .id("surface-editor")
                .w(px(width))
                .h(px(height))
                .overflow_hidden()
                .rounded(px(14.))
                .bg(theme::card_bg())
                .border_1()
                .border_color(theme::line())
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(header)
                .child(body)
                .child(footer),
        )
        .into_any_element()
}

fn concentration_share_panel(cx: &mut Context<AppView>) -> AnyElement {
    v_flex()
        .w_full()
        .p_5()
        .gap_3()
        .rounded(px(12.))
        .overflow_hidden()
        .bg(theme::page_bg())
        .border_1()
        .border_color(theme::accent().opacity(0.28))
        .child(
            h_flex()
                .items_center()
                .gap_2()
                .child(
                    Icon::new(IconName::Link2)
                        .with_size(Size::Small)
                        .text_color(theme::accent()),
                )
                .child(
                    div()
                        .text_sm()
                        .font_semibold()
                        .text_color(theme::accent_hover())
                        .child("复用 Premium 区域图源"),
                ),
        )
        .child(
            div()
                .text_xs()
                .line_height(relative(1.55))
                .text_color(theme::muted_ink())
                .child("[sources.concentration] 仍为空，因此集中模式直接使用已准备好的 premium-area 图片，不会重复下载。"),
        )
        .child(
            h_flex()
                .gap_2()
                .child(
                    Button::new("concentration-independent")
                        .small()
                        .outline()
                        .label("使用独立图源")
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.make_concentration_independent(window, cx);
                        })),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::faint_ink())
                        .child("会写入 source 与 local_directory，之后再单独配置"),
                ),
        )
        .into_any_element()
}

fn concentration_independent_panel(cx: &mut Context<AppView>) -> AnyElement {
    h_flex()
        .w_full()
        .justify_between()
        .items_center()
        .gap_3()
        .p_4()
        .rounded(px(12.))
        .overflow_hidden()
        .bg(theme::card_bg())
        .border_1()
        .border_color(theme::line())
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
                        .child("当前为独立图源"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme::muted_ink())
                        .child("[sources.concentration] 已包含覆盖项，不再复用 Premium 图片。"),
                ),
        )
        .child(
            Button::new("concentration-restore")
                .small()
                .ghost()
                .label("恢复复用 Premium")
                .on_click(cx.listener(|this, _, window, cx| {
                    this.restore_concentration_shared(window, cx);
                })),
        )
        .into_any_element()
}

fn source_panel(editor: &SurfaceEditor, config: &Config, cx: &mut Context<AppView>) -> AnyElement {
    let surface = editor.surface;
    let source_override = app::surface_override_source(config, surface);
    let fit_override = app::surface_override_fit(config, surface);
    let recursive_override = app::surface_override(config, surface).recursive;
    let resolved = config.for_surface(surface);
    let source_buttons = vec![
        widgets::segment_button(
            element_id("surface-source-inherit", surface.name()),
            "继承",
            source_override.is_none(),
        )
        .on_click(cx.listener(move |this, _, _, cx| {
            this.set_surface_source(surface, None, cx);
        })),
        widgets::segment_button(
            element_id("surface-source-local", surface.name()),
            "本地",
            source_override == Some(Source::Local),
        )
        .on_click(cx.listener(move |this, _, _, cx| {
            this.set_surface_source(surface, Some(Source::Local), cx);
        })),
        widgets::segment_button(
            element_id("surface-source-remote", surface.name()),
            "远程",
            source_override == Some(Source::Remote),
        )
        .on_click(cx.listener(move |this, _, _, cx| {
            this.set_surface_source(surface, Some(Source::Remote), cx);
        })),
        widgets::segment_button(
            element_id("surface-source-mixed", surface.name()),
            "混合",
            source_override == Some(Source::Mixed),
        )
        .on_click(cx.listener(move |this, _, _, cx| {
            this.set_surface_source(surface, Some(Source::Mixed), cx);
        })),
    ];
    let fit_buttons = vec![
        widgets::segment_button(
            element_id("surface-fit-inherit", surface.name()),
            "继承",
            fit_override.is_none(),
        )
        .on_click(cx.listener(move |this, _, _, cx| {
            this.set_surface_fit(surface, None, cx);
        })),
        widgets::segment_button(
            element_id("surface-fit-cover", surface.name()),
            "裁切填充",
            fit_override == Some(Fit::Cover),
        )
        .on_click(cx.listener(move |this, _, _, cx| {
            this.set_surface_fit(surface, Some(Fit::Cover), cx);
        })),
        widgets::segment_button(
            element_id("surface-fit-contain", surface.name()),
            "完整显示",
            fit_override == Some(Fit::Contain),
        )
        .on_click(cx.listener(move |this, _, _, cx| {
            this.set_surface_fit(surface, Some(Fit::Contain), cx);
        })),
    ];
    widgets::panel(
        "图源与适配",
        "空字段继承全局；已有显式覆盖优先，只有点过“自定义”的字段才会写入此处。",
        widgets::rows(vec![
            widgets::setting_row_stacked(
                "来源模式",
                inheritance_note(
                    source_override.is_some(),
                    theme::source_label(config.source),
                ),
                widgets::segmented(source_buttons, cx),
            ),
            widgets::setting_row_stacked(
                "本地目录",
                "相对路径按 iidxbg.toml 所在目录解析。",
                override_path_control(
                    editor,
                    SurfaceTextField::LocalDirectory,
                    source_override_path(config, surface),
                    &resolved.local_directory.to_string_lossy(),
                    cx,
                ),
            ),
            widgets::setting_row_stacked(
                "递归扫描",
                inheritance_note(
                    recursive_override.is_some(),
                    if resolved.recursive {
                        "开启"
                    } else {
                        "关闭"
                    },
                ),
                override_switch_control(config, surface, resolved.recursive, cx),
            ),
            widgets::setting_row_stacked(
                "适配方式",
                inheritance_note(fit_override.is_some(), theme::fit_label(resolved.image.fit)),
                widgets::segmented(fit_buttons, cx),
            ),
        ]),
    )
}

fn inheritance_note(overridden: bool, value: impl std::fmt::Display) -> String {
    format!("当前{}：{value}", if overridden { "覆盖" } else { "继承" })
}

fn source_override_path(config: &Config, surface: Surface) -> Option<std::path::PathBuf> {
    app::surface_override(config, surface)
        .local_directory
        .clone()
}

fn override_path_control(
    editor: &SurfaceEditor,
    field: SurfaceTextField,
    value: Option<std::path::PathBuf>,
    effective: &str,
    cx: &mut Context<AppView>,
) -> AnyElement {
    let surface = editor.surface;
    if value.is_some() {
        h_flex()
            .gap_2()
            .items_center()
            .child(
                Input::new(editor.input_for(field))
                    .flex_1()
                    .min_w_0()
                    .cleanable(true),
            )
            .child(
                Button::new(element_id("surface-browse", field_name(field)))
                    .small()
                    .ghost()
                    .icon(Icon::new(IconName::FolderOpen).with_size(Size::Small))
                    .tooltip("选择文件夹")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.pick_surface_directory(field, window, cx);
                    })),
            )
            .child(
                Button::new(element_id("surface-inherit", field_name(field)))
                    .small()
                    .ghost()
                    .label("继承")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.clear_surface_override(surface, field, window, cx);
                    })),
            )
            .into_any_element()
    } else {
        h_flex()
            .gap_2()
            .items_center()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(widgets::muted_path(effective.to_owned())),
            )
            .child(
                Button::new(element_id("surface-browse", field_name(field)))
                    .small()
                    .ghost()
                    .icon(Icon::new(IconName::FolderOpen).with_size(Size::Small))
                    .tooltip("选择文件夹")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.pick_surface_directory(field, window, cx);
                    })),
            )
            .child(
                Button::new(element_id("surface-custom", field_name(field)))
                    .small()
                    .ghost()
                    .label("自定义")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.begin_surface_override(surface, field, window, cx);
                    })),
            )
            .into_any_element()
    }
}

fn override_switch_control(
    config: &Config,
    surface: Surface,
    effective: bool,
    cx: &mut Context<AppView>,
) -> AnyElement {
    let override_value = match surface {
        Surface::Results => config.sources.results.recursive,
        Surface::SongSelect => config.sources.song_select.recursive,
        Surface::SuddenPlus => config.sources.sudden_plus.recursive,
        Surface::Lift => config.sources.lift.recursive,
        Surface::TargetGraph => config.sources.target_graph.recursive,
        Surface::PremiumArea => config.sources.premium_area.recursive,
        Surface::Concentration => config.sources.concentration.recursive,
    };
    let checked = override_value.unwrap_or(effective);
    h_flex()
        .gap_3()
        .items_center()
        .child(
            Button::new(element_id("surface-recursive-inherit", surface.name()))
                .xsmall()
                .ghost()
                .label(if override_value.is_some() {
                    "继承"
                } else {
                    "自定义"
                })
                .on_click(cx.listener(move |this, _, _, cx| match override_value {
                    Some(_) => this.set_surface_recursive(surface, None, cx),
                    None => this.begin_surface_switch(surface, cx),
                })),
        )
        .child(
            Switch::new(element_id("surface-recursive", surface.name()))
                .checked(checked)
                .color(theme::accent())
                .disabled(override_value.is_none())
                .on_change(cx.listener(move |this, checked, _, cx| {
                    this.set_surface_recursive(surface, Some(*checked), cx);
                })),
        )
        .into_any_element()
}

fn moebooru_panel(
    editor: &SurfaceEditor,
    config: &Config,
    cx: &mut Context<AppView>,
) -> AnyElement {
    let surface = editor.surface;
    let source = app::surface_override(config, surface);
    let resolved = config.for_surface(surface);
    let override_count = [
        source.moebooru.domain.is_some(),
        source.moebooru.tags.is_some(),
        source.moebooru.save_directory.is_some(),
        source.moebooru.page_max.is_some(),
        source.moebooru.timeout_seconds.is_some(),
        source.moebooru.max_images.is_some(),
    ]
    .into_iter()
    .filter(|value| *value)
    .count();
    let toggle = Button::new(element_id("surface-moebooru-toggle", surface.name()))
        .small()
        .ghost()
        .label(if editor.advanced_open {
            "收起"
        } else {
            "展开"
        })
        .icon(
            Icon::new(if editor.advanced_open {
                IconName::ChevronUp
            } else {
                IconName::ChevronDown
            })
            .with_size(Size::XSmall),
        )
        .on_click(cx.listener(|this, _, _, cx| {
            if let Some(editor) = &mut this.editor {
                editor.advanced_open = !editor.advanced_open;
            }
            cx.notify();
        }));
    let summary = format!(
        "远程下载覆盖 {override_count} 项 · 当前生效 {} / {} 秒 / 保留 {} 张",
        theme::domain_label(resolved.moebooru.domain),
        resolved.moebooru.timeout_seconds,
        resolved.moebooru.max_images
    );
    if !editor.advanced_open {
        return widgets::panel_with_right(
            "Moebooru 下载",
            summary,
            toggle,
            widgets::rows(vec![widgets::setting_row_stacked(
                "下载参数",
                "展开后可以覆盖站点、标签、缓存目录、页数、超时和保留上限。",
                div()
                    .text_xs()
                    .text_color(theme::faint_ink())
                    .child(if override_count > 0 {
                        "已有覆盖项"
                    } else {
                        "全部继承全局"
                    }),
            )]),
        );
    }

    let domain = source.moebooru.domain;
    let domain_buttons = vec![
        widgets::segment_button(
            element_id("surface-domain-inherit", surface.name()),
            "继承",
            domain.is_none(),
        )
        .on_click(cx.listener(move |this, _, _, cx| {
            this.set_surface_domain(surface, None, cx);
        })),
        widgets::segment_button(
            element_id("surface-domain-net", surface.name()),
            "Konachan.net",
            domain == Some(MoebooruDomain::Net),
        )
        .on_click(cx.listener(move |this, _, _, cx| {
            this.set_surface_domain(surface, Some(MoebooruDomain::Net), cx);
        })),
        widgets::segment_button(
            element_id("surface-domain-com", surface.name()),
            "Konachan.com",
            domain == Some(MoebooruDomain::Com),
        )
        .on_click(cx.listener(move |this, _, _, cx| {
            this.set_surface_domain(surface, Some(MoebooruDomain::Com), cx);
        })),
        widgets::segment_button(
            element_id("surface-domain-yandere", surface.name()),
            "Yande.re",
            domain == Some(MoebooruDomain::Yandere),
        )
        .on_click(cx.listener(move |this, _, _, cx| {
            this.set_surface_domain(surface, Some(MoebooruDomain::Yandere), cx);
        })),
    ];
    widgets::panel_with_right(
        "Moebooru 下载",
        "每项留空即继承全局；覆盖只影响这一个组件。",
        toggle,
        widgets::rows(vec![
            widgets::setting_row_stacked(
                "站点",
                inheritance_note(
                    domain.is_some(),
                    theme::domain_label(resolved.moebooru.domain),
                ),
                widgets::segmented_wrap(domain_buttons, cx),
            ),
            widgets::setting_row_stacked(
                "搜索标签",
                "空格分隔；空字符串也是有效值。",
                override_text_control(
                    editor,
                    SurfaceTextField::Tags,
                    source.moebooru.tags.as_ref().map(|_| ()),
                    &resolved.moebooru.tags,
                    cx,
                ),
            ),
            widgets::setting_row_stacked(
                "下载保存目录",
                "相对路径按 iidxbg.toml 所在目录解析。",
                override_path_control(
                    editor,
                    SurfaceTextField::SaveDirectory,
                    source.moebooru.save_directory.clone(),
                    &resolved.moebooru.save_directory.to_string_lossy(),
                    cx,
                ),
            ),
            widgets::setting_row_stacked(
                "随机页数上限",
                inheritance_note(
                    source.moebooru.page_max.is_some(),
                    resolved.moebooru.page_max,
                ),
                override_number_control(
                    editor,
                    SurfaceTextField::PageMax,
                    source.moebooru.page_max.map(|_| ()),
                    &resolved.moebooru.page_max.to_string(),
                    "页",
                    cx,
                ),
            ),
            widgets::setting_row_stacked(
                "请求超时",
                inheritance_note(
                    source.moebooru.timeout_seconds.is_some(),
                    format!("{} 秒", resolved.moebooru.timeout_seconds),
                ),
                override_number_control(
                    editor,
                    SurfaceTextField::TimeoutSeconds,
                    source.moebooru.timeout_seconds.map(|_| ()),
                    &resolved.moebooru.timeout_seconds.to_string(),
                    "秒",
                    cx,
                ),
            ),
            widgets::setting_row_stacked(
                "下载保留上限",
                inheritance_note(
                    source.moebooru.max_images.is_some(),
                    format!("{} 张，0 表示不清理", resolved.moebooru.max_images),
                ),
                override_number_control(
                    editor,
                    SurfaceTextField::MaxImages,
                    source.moebooru.max_images.map(|_| ()),
                    &resolved.moebooru.max_images.to_string(),
                    "张",
                    cx,
                ),
            ),
        ]),
    )
}

fn override_text_control(
    editor: &SurfaceEditor,
    field: SurfaceTextField,
    overridden: Option<()>,
    effective: &str,
    cx: &mut Context<AppView>,
) -> AnyElement {
    let surface = editor.surface;
    if overridden.is_some() {
        h_flex()
            .gap_2()
            .items_center()
            .child(
                Input::new(editor.input_for(field))
                    .flex_1()
                    .min_w_0()
                    .cleanable(true),
            )
            .child(
                Button::new(element_id("surface-text-inherit", field_name(field)))
                    .small()
                    .ghost()
                    .label("继承")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.clear_surface_override(surface, field, window, cx);
                    })),
            )
            .into_any_element()
    } else {
        h_flex()
            .gap_2()
            .items_center()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(widgets::muted_path(effective.to_owned())),
            )
            .child(
                Button::new(element_id("surface-text-custom", field_name(field)))
                    .small()
                    .ghost()
                    .label("自定义")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.begin_surface_override(surface, field, window, cx);
                    })),
            )
            .into_any_element()
    }
}

fn override_number_control(
    editor: &SurfaceEditor,
    field: SurfaceTextField,
    overridden: Option<()>,
    effective: &str,
    suffix: &'static str,
    cx: &mut Context<AppView>,
) -> AnyElement {
    let surface = editor.surface;
    if overridden.is_some() {
        h_flex()
            .gap_2()
            .items_center()
            .child(
                Input::new(editor.input_for(field))
                    .w(px(104.))
                    .cleanable(false),
            )
            .child(div().text_xs().text_color(theme::faint_ink()).child(suffix))
            .child(
                Button::new(element_id("surface-number-inherit", field_name(field)))
                    .small()
                    .ghost()
                    .label("继承")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.clear_surface_override(surface, field, window, cx);
                    })),
            )
            .into_any_element()
    } else {
        h_flex()
            .gap_2()
            .items_center()
            .child(
                div()
                    .text_sm()
                    .text_color(theme::muted_ink())
                    .child(format!("{effective} {suffix}")),
            )
            .child(
                Button::new(element_id("surface-number-custom", field_name(field)))
                    .small()
                    .ghost()
                    .label("自定义")
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.begin_surface_override(surface, field, window, cx);
                    })),
            )
            .into_any_element()
    }
}

fn preview_panel(
    view: &mut AppView,
    config: &Config,
    surface: Surface,
    enabled: bool,
    cx: &mut Context<AppView>,
) -> AnyElement {
    let slot = &view.previews[surface.index()];
    let (icon, color, title, detail) = if !enabled {
        (
            IconName::EyeOff,
            theme::muted_ink(),
            "未启用".to_owned(),
            "启用后才会按当前配置获取下一张图片。".to_owned(),
        )
    } else {
        match slot.status {
            PreviewStatus::Loading => (
                IconName::Loader,
                theme::accent(),
                "正在获取下一张".to_owned(),
                "预览在工作线程中读取同一份配置，不阻塞界面。".to_owned(),
            ),
            PreviewStatus::Ready => (
                IconName::Image,
                theme::success(),
                "已获取预览".to_owned(),
                slot.detail
                    .clone()
                    .map(|detail| detail.to_string())
                    .unwrap_or_else(|| "来自当前图源".to_owned()),
            ),
            PreviewStatus::Failed => (
                IconName::TriangleAlert,
                theme::danger(),
                "预览获取失败".to_owned(),
                slot.error
                    .clone()
                    .map(|error| error.to_string())
                    .unwrap_or_else(|| "可以修正路径或网络后重试".to_owned()),
            ),
            PreviewStatus::Empty => (
                IconName::Image,
                theme::muted_ink(),
                "等待预览".to_owned(),
                "配置变化后会自动刷新。".to_owned(),
            ),
            PreviewStatus::Disabled => (
                IconName::EyeOff,
                theme::muted_ink(),
                "未启用".to_owned(),
                "启用后才会获取预览。".to_owned(),
            ),
        }
    };
    let shared = surface == Surface::Concentration && config.concentration_shares_premium();
    v_flex()
        .w_full()
        .p_5()
        .gap_3()
        .rounded(px(12.))
        .overflow_hidden()
        .bg(theme::card_bg())
        .border_1()
        .border_color(theme::line())
        .child(
            h_flex()
                .justify_between()
                .items_start()
                .gap_4()
                .child(
                    h_flex()
                        .flex_1()
                        .gap_3()
                        .items_start()
                        .child(Icon::new(icon).with_size(Size::Small).text_color(color))
                        .child(
                            v_flex()
                                .flex_1()
                                .gap_1()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_semibold()
                                        .text_color(theme::ink())
                                        .child(title),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .line_height(relative(1.5))
                                        .text_color(theme::muted_ink())
                                        .child(detail),
                                ),
                        ),
                )
                .child(
                    Button::new(element_id("surface-refresh", surface.name()))
                        .small()
                        .outline()
                        .label("换一张")
                        .icon(Icon::new(IconName::RefreshCw).with_size(Size::XSmall))
                        .disabled(!enabled)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.refresh_surface(surface, cx);
                        })),
                ),
        )
        .when(shared, |this| {
            this.child(
                div()
                    .text_xs()
                    .text_color(theme::faint_ink())
                    .child("集中模式与 Premium 区域共享图源；只需配置一次。"),
            )
        })
        .into_any_element()
}

fn field_name(field: SurfaceTextField) -> &'static str {
    match field {
        SurfaceTextField::LocalDirectory => "local-directory",
        SurfaceTextField::Tags => "tags",
        SurfaceTextField::SaveDirectory => "save-directory",
        SurfaceTextField::PageMax => "page-max",
        SurfaceTextField::TimeoutSeconds => "timeout",
        SurfaceTextField::MaxImages => "max-images",
    }
}
