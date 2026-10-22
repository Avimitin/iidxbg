use gpui::{
    AnyElement, Entity, IntoElement, ParentElement, SharedString, Styled, Window,
    prelude::FluentBuilder as _, px, relative,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    Icon, Sizable, Size, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputState},
    scroll::ScrollableElement,
    switch::Switch,
    v_flex,
};
use gpui_kit::*;
use iidxbg::config::{Config, Fit, MoebooruDomain, Source, Surface};

use crate::{
    app::{self, AppView},
    store, theme, widgets,
};

pub(crate) struct GlobalInputs {
    pub local_directory: Entity<InputState>,
    pub save_directory: Entity<InputState>,
    pub tags: Entity<InputState>,
    pub width: Entity<InputState>,
    pub height: Entity<InputState>,
    pub page_max: Entity<InputState>,
    pub timeout_seconds: Entity<InputState>,
    pub max_images: Entity<InputState>,
    pub max_download_mb: Entity<InputState>,
    pub max_source_pixels: Entity<InputState>,
    pub frame_chara_image: Entity<InputState>,
}

impl GlobalInputs {
    pub fn new(config: &Config, window: &mut Window, cx: &mut Context<AppView>) -> Self {
        Self {
            local_directory: input(
                config.local_directory.to_string_lossy(),
                "例如 iidxbg/results",
                window,
                cx,
            ),
            save_directory: input(
                config.moebooru.save_directory.to_string_lossy(),
                "下载图片的保存目录",
                window,
                cx,
            ),
            tags: input(
                &config.moebooru.tags,
                "例如 rating:safe landscape",
                window,
                cx,
            ),
            width: input(config.image.width.to_string(), "1920", window, cx),
            height: input(config.image.height.to_string(), "1080", window, cx),
            page_max: input(config.moebooru.page_max.to_string(), "5", window, cx),
            timeout_seconds: input(
                config.moebooru.timeout_seconds.to_string(),
                "20",
                window,
                cx,
            ),
            max_images: input(config.moebooru.max_images.to_string(), "0", window, cx),
            max_download_mb: input(config.image.max_download_mb.to_string(), "24", window, cx),
            max_source_pixels: input(
                config.image.max_source_pixels.to_string(),
                "40000000",
                window,
                cx,
            ),
            frame_chara_image: input(
                config.selection.frame_chara_image.to_string_lossy(),
                "留空使用游戏原角色",
                window,
                cx,
            ),
        }
    }
}

fn input(
    value: impl Into<SharedString>,
    placeholder: impl Into<SharedString>,
    window: &mut Window,
    cx: &mut Context<AppView>,
) -> Entity<InputState> {
    let value = value.into();
    let placeholder = placeholder.into();
    cx.new(move |cx| {
        InputState::new(window, cx)
            .default_value(value)
            .placeholder(placeholder)
    })
}

impl AppView {
    pub(crate) fn render_global(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let config = self.store.config().clone();
        let viewport = window.viewport_size().width.as_f32();
        let metrics = theme::layout_metrics(viewport);
        let content_width = (viewport - metrics.sidebar - metrics.padding * 2.).max(300.);
        let wide = content_width >= 960.;
        let error = self
            .global_error
            .clone()
            .or_else(|| self.store.error().map(|error| error.to_owned().into()));

        let hero = v_flex()
            .w_full()
            .gap_3()
            .mb_2()
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .text_size(px(if metrics.compact { 24. } else { 30. }))
                            .font_semibold()
                            .text_color(theme::ink())
                            .child("全局选项"),
                    )
                    .child(widgets::status_pill(
                        "自动保存",
                        theme::success(),
                        theme::success_soft(),
                    )),
            )
            .child(
                div()
                    .max_w(px(660.))
                    .text_sm()
                    .line_height(relative(1.6))
                    .text_color(theme::muted_ink())
                    .child("这里是组件的默认来源与图像处理设置。每个组件都可以在弹窗中单独覆盖；已有的 [sources.*] 显式值优先，未覆盖的字段会实时继承。"),
            );

        let form = v_flex()
            .flex_1()
            .min_w_0()
            .gap_6()
            .when_some(error, |this, error| {
                this.child(widgets::error_banner(error))
            })
            .child(self.master_panel(&config, cx))
            .child(self.source_panel(&config, cx))
            .child(self.moebooru_panel(&config, cx))
            .child(self.image_panel(&config, cx))
            .child(self.selection_panel(&config, cx));

        let aside = if wide {
            v_flex().w(px(304.)).flex_shrink_0().gap_4().children([
                self.save_status_panel(cx),
                self.config_file_panel(cx),
                self.surface_status_panel(cx),
                self.help_panel(),
            ])
        } else {
            v_flex().w_full().gap_4().children([
                self.save_status_panel(cx),
                self.config_file_panel(cx),
                self.surface_status_panel(cx),
                self.help_panel(),
            ])
        };

        let body = if wide {
            h_flex()
                .w_full()
                .items_start()
                .gap_6()
                .child(form)
                .child(aside)
        } else {
            v_flex().w_full().gap_6().child(form).child(aside)
        };

        div()
            .id("global-scroll")
            .w_full()
            .flex_1()
            .min_h_0()
            .overflow_y_scrollbar()
            .child(
                div()
                    .w_full()
                    .p(px(metrics.padding))
                    .child(v_flex().w_full().gap_7().child(hero).child(body)),
            )
            .into_any_element()
    }

    fn master_panel(&self, config: &Config, cx: &mut Context<Self>) -> AnyElement {
        let enabled = config.enabled;
        let effective = self.enabled_count();
        widgets::panel(
            "总开关",
            "关闭后所有组件立即使用游戏原图；生命周期钩子会保留，重新开启不需要重启游戏。",
            widgets::rows(vec![
                widgets::setting_row(
                    "启用 iidxbg",
                    "结算图与所有额外背景共享这个总开关。",
                    Switch::new("global-enabled")
                        .checked(enabled)
                        .color(theme::accent())
                        .on_change(cx.listener(|this, checked, _, cx| {
                            this.set_master_enabled(*checked, cx);
                        })),
                ),
                widgets::setting_row(
                    "当前生效组件",
                    "受总开关与各组件开关共同控制。",
                    div()
                        .text_sm()
                        .font_semibold()
                        .text_color(if effective > 0 {
                            theme::success()
                        } else {
                            theme::muted_ink()
                        })
                        .child(format!("{effective} / {}", Surface::COUNT)),
                ),
            ]),
        )
    }

    fn source_panel(&self, config: &Config, cx: &mut Context<Self>) -> AnyElement {
        let source = config.source;
        let inheriting = Surface::ALL
            .iter()
            .filter(|surface| app::surface_override_source(config, **surface).is_none())
            .count();
        let buttons = vec![
            widgets::segment_button("global-source-local", "本地", source == Source::Local)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.set_global_source(Source::Local, cx);
                })),
            widgets::segment_button("global-source-remote", "远程", source == Source::Remote)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.set_global_source(Source::Remote, cx);
                })),
            widgets::segment_button("global-source-mixed", "混合", source == Source::Mixed)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.set_global_source(Source::Mixed, cx);
                })),
        ];
        widgets::panel(
            "图像来源",
            "本地从文件夹随机选择，远程从 Moebooru 下载；混合模式会随机选择并在失败时尝试另一侧。",
            widgets::rows(vec![
                widgets::setting_row(
                    "来源模式",
                    "每个组件都可以单独覆盖；[sources.*] 中已有的显式值优先。",
                    widgets::segmented(buttons, cx),
                ),
                widgets::setting_row(
                    "本地目录",
                    "PNG/JPEG 文件夹。相对路径按 iidxbg.toml 所在目录解析。",
                    h_flex()
                        .w_full()
                        .items_center()
                        .gap_2()
                        .child(
                            Input::new(&self.global_inputs.local_directory)
                                .flex_1()
                                .min_w_0()
                                .cleanable(true),
                        )
                        .child(
                            Button::new("browse-global-local")
                                .small()
                                .ghost()
                                .icon(Icon::new(IconName::FolderOpen).with_size(Size::Small))
                                .tooltip("选择文件夹")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.pick_global_directory(
                                        app::GlobalTextField::LocalDirectory,
                                        window,
                                        cx,
                                    );
                                })),
                        ),
                ),
                widgets::setting_row(
                    "递归扫描子目录",
                    "最多 8 层、10,000 个候选；符号链接与目录联接会被跳过。",
                    Switch::new("global-recursive")
                        .checked(config.recursive)
                        .color(theme::accent())
                        .on_change(cx.listener(|this, checked, _, cx| {
                            this.set_global_recursive(*checked, cx);
                        })),
                ),
                widgets::note(
                    IconName::Info,
                    format!(
                        "{inheriting} / {} 个组件的 source 仍继承这里；其余组件使用自己的 [sources.*] 覆盖。",
                        Surface::COUNT
                    ),
                    theme::accent(),
                    theme::accent_soft(),
                ),
            ]),
        )
    }

    fn moebooru_panel(&self, config: &Config, cx: &mut Context<Self>) -> AnyElement {
        let domain = config.moebooru.domain;
        let buttons = vec![
            widgets::segment_button(
                "global-domain-net",
                "Konachan.net",
                domain == MoebooruDomain::Net,
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.set_global_domain(MoebooruDomain::Net, cx);
            })),
            widgets::segment_button(
                "global-domain-com",
                "Konachan.com",
                domain == MoebooruDomain::Com,
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.set_global_domain(MoebooruDomain::Com, cx);
            })),
            widgets::segment_button(
                "global-domain-yandere",
                "Yande.re",
                domain == MoebooruDomain::Yandere,
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.set_global_domain(MoebooruDomain::Yandere, cx);
            })),
        ];
        widgets::panel(
            "Moebooru 下载",
            "远程模式使用的公开 API 与下载保留策略。API 不读取账号，也不修改站点内容。",
            widgets::rows(vec![
                widgets::setting_row(
                    "站点",
                    "选择后同时决定 API 端点、允许的图片域名与缓存文件命名空间。",
                    widgets::segmented(buttons, cx),
                ),
                widgets::setting_row(
                    "搜索标签",
                    "用空格分隔；默认 rating:safe。留空会请求站点默认列表。",
                    Input::new(&self.global_inputs.tags)
                        .cleanable(true)
                        .w_full(),
                ),
                widgets::setting_row(
                    "下载保存目录",
                    "成功解码后写入这里。相对路径按 iidxbg.toml 所在目录解析。",
                    h_flex()
                        .w_full()
                        .items_center()
                        .gap_2()
                        .child(
                            Input::new(&self.global_inputs.save_directory)
                                .flex_1()
                                .min_w_0()
                                .cleanable(true),
                        )
                        .child(
                            Button::new("browse-global-save")
                                .small()
                                .ghost()
                                .icon(Icon::new(IconName::FolderOpen).with_size(Size::Small))
                                .tooltip("选择文件夹")
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.pick_global_directory(
                                        app::GlobalTextField::SaveDirectory,
                                        window,
                                        cx,
                                    );
                                })),
                        ),
                ),
                widgets::setting_row(
                    "随机页数上限",
                    "范围 1–100；从 1 到该值之间随机选一页，再打乱该页结果。",
                    number_input(&self.global_inputs.page_max, "页"),
                ),
                widgets::setting_row(
                    "请求超时",
                    "单次 HTTP 请求的超时，范围 3–60 秒。失败后 60 秒再重试。",
                    number_input(&self.global_inputs.timeout_seconds, "秒"),
                ),
                widgets::setting_row(
                    "下载保留上限",
                    "只统计下载器命名的文件；0 表示不清理。范围 0–1,000,000。",
                    number_input(&self.global_inputs.max_images, "张"),
                ),
            ]),
        )
    }

    fn image_panel(&self, config: &Config, cx: &mut Context<Self>) -> AnyElement {
        let fit = config.image.fit;
        let fit_buttons = vec![
            widgets::segment_button("global-fit-cover", "裁切填充", fit == Fit::Cover).on_click(
                cx.listener(|this, _, _, cx| {
                    this.set_global_fit(Fit::Cover, cx);
                }),
            ),
            widgets::segment_button("global-fit-contain", "完整显示", fit == Fit::Contain)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.set_global_fit(Fit::Contain, cx);
                })),
        ];
        let size_control = h_flex()
            .gap_2()
            .items_center()
            .child(
                Input::new(&self.global_inputs.width)
                    .w(px(92.))
                    .cleanable(false),
            )
            .child(div().text_sm().text_color(theme::faint_ink()).child("×"))
            .child(
                Input::new(&self.global_inputs.height)
                    .w(px(92.))
                    .cleanable(false),
            )
            .child(div().text_xs().text_color(theme::faint_ink()).child("px"));
        widgets::panel(
            "图像处理",
            "输出纹理的画布尺寸与适配策略。单个组件可以使用自己的画布比例，这里主要影响结算图。",
            widgets::rows(vec![
                widgets::setting_row(
                    "结算图画布",
                    "宽度 64–3840，高度 64–2160。组件预览会按这个比例展示。",
                    size_control,
                ),
                widgets::setting_row(
                    "适配方式",
                    "裁切填充会铺满画布并裁掉多余边缘；完整显示会保留整张图并用模糊背景填满空白。",
                    widgets::segmented(fit_buttons, cx),
                ),
                widgets::setting_row(
                    "单文件大小上限",
                    "本地与远程共用，范围 1–64 MiB。",
                    number_input(&self.global_inputs.max_download_mb, "MiB"),
                ),
                widgets::setting_row(
                    "解码像素上限",
                    "在缩放前限制源图宽×高，最大 64,000,000。",
                    number_input(&self.global_inputs.max_source_pixels, "px"),
                ),
            ]),
        )
    }

    fn selection_panel(&self, config: &Config, cx: &mut Context<Self>) -> AnyElement {
        widgets::panel(
            "选择界面角色",
            "控制选曲界面的原生角色图层；可以隐藏，也可以替换为一张本地 PNG/JPEG。",
            widgets::rows(vec![
                widgets::setting_row(
                    "显示角色图层",
                    "关闭后隐藏选曲界面的角色；开启不会凭空创建游戏没有的角色。",
                    Switch::new("global-frame-chara")
                        .checked(config.selection.frame_chara)
                        .color(theme::accent())
                        .on_change(cx.listener(|this, checked, _, cx| {
                            this.set_frame_chara(*checked, cx);
                        })),
                ),
                widgets::setting_row(
                    "替换角色图片",
                    "留空使用游戏原角色。路径相对 iidxbg.toml；透明 PNG 效果最好。",
                    Input::new(&self.global_inputs.frame_chara_image)
                        .cleanable(true)
                        .w_full(),
                ),
            ]),
        )
    }

    fn save_status_panel(&self, _cx: &mut Context<Self>) -> AnyElement {
        let error = self
            .global_error
            .clone()
            .or_else(|| self.store.error().map(|error| error.to_owned().into()));
        let (icon, color, title, detail) = if let Some(error) = error {
            (
                IconName::TriangleAlert,
                theme::danger(),
                "尚未写入",
                error.to_string(),
            )
        } else {
            (
                IconName::CircleCheck,
                theme::success(),
                "自动保存已开启",
                if self.store.saved_at().is_some() {
                    "最后一次修改已经写入 iidxbg.toml。".to_owned()
                } else {
                    "每次有效修改都会立即写入 iidxbg.toml，不需要保存按钮。".to_owned()
                },
            )
        };
        v_flex()
            .w_full()
            .p_5()
            .gap_3()
            .rounded(px(14.))
            .bg(theme::card_bg())
            .border_1()
            .border_color(theme::line())
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(Icon::new(icon).with_size(Size::Small).text_color(color))
                    .child(
                        div()
                            .text_sm()
                            .font_semibold()
                            .text_color(theme::ink())
                            .child(title),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .line_height(relative(1.55))
                    .text_color(theme::muted_ink())
                    .child(detail),
            )
            .into_any_element()
    }

    fn config_file_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let path = self.store.path().display().to_string();
        v_flex()
            .w_full()
            .p_5()
            .gap_3()
            .rounded(px(14.))
            .bg(theme::card_bg())
            .border_1()
            .border_color(theme::line())
            .child(
                div()
                    .text_sm()
                    .font_semibold()
                    .text_color(theme::ink())
                    .child("配置文件"),
            )
            .child(widgets::muted_path(path))
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Button::new("reload-config")
                            .small()
                            .outline()
                            .label("重新载入")
                            .icon(Icon::new(IconName::RotateCcw).with_size(Size::XSmall))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.reload_from_disk(window, cx);
                            })),
                    )
                    .child(
                        Button::new("reveal-config")
                            .small()
                            .ghost()
                            .label("打开文件夹")
                            .icon(Icon::new(IconName::FolderOpen).with_size(Size::XSmall))
                            .on_click(cx.listener(|this, _, _, _| this.reveal_config())),
                    ),
            )
            .into_any_element()
    }

    fn surface_status_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let config = self.store.config().clone();
        let rows = Surface::ALL
            .iter()
            .map(|surface| {
                let surface = *surface;
                let enabled = store::effective_enabled(&config, surface);
                h_flex()
                    .id(SharedString::from(format!(
                        "global-status-{}",
                        surface.name()
                    )))
                    .w_full()
                    .items_center()
                    .gap_2()
                    .py_1()
                    .cursor_pointer()
                    .hover(|this| this.opacity(0.75))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.open_editor(surface, window, cx);
                    }))
                    .child(widgets::dot(
                        if enabled {
                            theme::success()
                        } else {
                            theme::faint_ink()
                        },
                        7.,
                    ))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_xs()
                            .text_color(if enabled {
                                theme::ink_soft()
                            } else {
                                theme::muted_ink()
                            })
                            .child(theme::surface_title(surface)),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(if enabled {
                                theme::success()
                            } else {
                                theme::faint_ink()
                            })
                            .child(if enabled { "生效" } else { "关闭" }),
                    )
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        v_flex()
            .w_full()
            .p_5()
            .gap_3()
            .rounded(px(14.))
            .bg(theme::card_bg())
            .border_1()
            .border_color(theme::line())
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .font_semibold()
                            .text_color(theme::ink())
                            .child("组件状态"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::faint_ink())
                            .child("点击可打开"),
                    ),
            )
            .child(v_flex().w_full().gap_0().children(rows))
            .into_any_element()
    }

    fn help_panel(&self) -> AnyElement {
        v_flex()
            .w_full()
            .p_5()
            .gap_3()
            .rounded(px(14.))
            .bg(theme::page_bg())
            .border_1()
            .border_color(theme::accent().opacity(0.28))
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Icon::new(IconName::Info)
                            .with_size(Size::Small)
                            .text_color(theme::accent()),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_semibold()
                            .text_color(theme::accent_hover())
                            .child("继承与覆盖"),
                    ),
            )
            .child(
                v_flex()
                    .gap_1()
                    .text_xs()
                    .line_height(relative(1.6))
                    .text_color(theme::muted_ink())
                    .child("• 组件的空字段始终读取这里的值。")
                    .child("• [sources.*] 显式值优先于本页；在弹窗里恢复继承后才会跟随。")
                    .child("• 无效数字不会写盘；修正后自动保存。")
                    .child("• TOML 注释会被保留，只改动对应键。"),
            )
            .into_any_element()
    }
}

fn number_input(input: &Entity<InputState>, suffix: &'static str) -> AnyElement {
    h_flex()
        .gap_2()
        .items_center()
        .child(Input::new(input).w(px(112.)).cleanable(false))
        .child(div().text_xs().text_color(theme::faint_ink()).child(suffix))
        .into_any_element()
}
