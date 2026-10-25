use crate::{app::AppView, store, theme, widgets};
use gpui::{
    AnyElement, Entity, IntoElement, MouseButton, ParentElement, SharedString, Styled,
    Subscription, Window, prelude::FluentBuilder as _, px,
};
use gpui_kit::component::{
    Sizable, StyledExt,
    button::Button,
    h_flex,
    input::{Input, InputEvent, InputState},
    scroll::ScrollableElement,
    switch::Switch,
    v_flex,
};
use gpui_kit::*;
use iidxbg::config::{Config, Fit, MoebooruDomain, Source, Surface};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SurfaceTextField {
    MediaPath,
    LocalDirectory,
    ApiUrl,
    MediaHosts,
    Tags,
    SaveDirectory,
    PageMax,
    TimeoutSeconds,
    MaxImages,
    FrameCharaImage,
    MmdModel,
    MmdMotion,
    MmdMotionBpm,
    MmdDisplayScale,
    MmdPositionX,
    MmdPositionY,
    MmdQproHeight,
    MmdScrollUp,
    MmdScrollDown,
    MmdHappyMotion,
    MmdOtherResult,
    MmdHappy,
    MmdLook,
    MmdLookYaw,
    MmdLookHold,
    MmdBlink,
    BiliVideo,
    BiliPlaylist,
    BiliFavourites,
    BiliRoom,
}
impl SurfaceTextField {
    const ALL: [Self; 30] = [
        Self::MediaPath,
        Self::LocalDirectory,
        Self::ApiUrl,
        Self::MediaHosts,
        Self::Tags,
        Self::SaveDirectory,
        Self::PageMax,
        Self::TimeoutSeconds,
        Self::MaxImages,
        Self::FrameCharaImage,
        Self::MmdModel,
        Self::MmdMotion,
        Self::MmdMotionBpm,
        Self::MmdDisplayScale,
        Self::MmdPositionX,
        Self::MmdPositionY,
        Self::MmdQproHeight,
        Self::MmdScrollUp,
        Self::MmdScrollDown,
        Self::MmdHappyMotion,
        Self::MmdOtherResult,
        Self::MmdHappy,
        Self::MmdLook,
        Self::MmdLookYaw,
        Self::MmdLookHold,
        Self::MmdBlink,
        Self::BiliVideo,
        Self::BiliPlaylist,
        Self::BiliFavourites,
        Self::BiliRoom,
    ];
    fn key(self) -> &'static str {
        match self {
            Self::MediaPath => "media_path",
            Self::LocalDirectory => "local_directory",
            Self::ApiUrl => "api_url",
            Self::MediaHosts => "media_hosts",
            Self::Tags => "tags",
            Self::SaveDirectory => "save_directory",
            Self::PageMax => "page_max",
            Self::TimeoutSeconds => "timeout_seconds",
            Self::MaxImages => "max_images",
            Self::FrameCharaImage => "frame_chara_image",
            Self::MmdModel => "model_path",
            Self::MmdMotion => "motion_path",
            Self::MmdMotionBpm => "motion_bpm",
            Self::MmdDisplayScale => "display_scale",
            Self::MmdPositionX => "position_x",
            Self::MmdPositionY => "position_y",
            Self::MmdQproHeight => "qpro_height",
            Self::MmdScrollUp => "scroll_up_motion_path",
            Self::MmdScrollDown => "scroll_down_motion_path",
            Self::MmdHappyMotion => "happy_motion_path",
            Self::MmdOtherResult => "other_result_motion_path",
            Self::MmdHappy => "happy_morph",
            Self::MmdLook => "look_bone",
            Self::MmdLookYaw => "look_yaw",
            Self::MmdLookHold => "look_hold_seconds",
            Self::MmdBlink => "blink_morph",
            Self::BiliVideo => "video",
            Self::BiliPlaylist => "playlist",
            Self::BiliFavourites => "favourites",
            Self::BiliRoom => "room",
        }
    }
    fn path(self, surface: Surface) -> Vec<&'static str> {
        match self {
            Self::FrameCharaImage => vec!["selection", self.key()],
            Self::MmdModel
            | Self::MmdMotion
            | Self::MmdMotionBpm
            | Self::MmdDisplayScale
            | Self::MmdPositionX
            | Self::MmdPositionY
            | Self::MmdQproHeight
            | Self::MmdScrollUp
            | Self::MmdScrollDown
            | Self::MmdHappyMotion
            | Self::MmdOtherResult
            | Self::MmdHappy
            | Self::MmdLook
            | Self::MmdLookYaw
            | Self::MmdLookHold
            | Self::MmdBlink => {
                vec!["mmd", self.key()]
            }
            Self::BiliVideo | Self::BiliPlaylist | Self::BiliFavourites | Self::BiliRoom => {
                vec!["sources", surface.name(), "bilibili", self.key()]
            }
            Self::MediaPath | Self::LocalDirectory => vec!["sources", surface.name(), self.key()],
            _ => vec!["sources", surface.name(), "moebooru", self.key()],
        }
    }
    fn value(self, config: &Config, surface: Surface) -> String {
        let c = config.for_surface(surface);
        match self {
            Self::MediaPath => c.media_path.to_string_lossy().into_owned(),
            Self::LocalDirectory => c.local_directory.to_string_lossy().into_owned(),
            Self::ApiUrl => c.moebooru.api_url,
            Self::MediaHosts => c.moebooru.media_hosts.join(", "),
            Self::Tags => c.moebooru.tags,
            Self::SaveDirectory => c.moebooru.save_directory.to_string_lossy().into_owned(),
            Self::PageMax => c.moebooru.page_max.to_string(),
            Self::TimeoutSeconds => c.moebooru.timeout_seconds.to_string(),
            Self::MaxImages => c.moebooru.max_images.to_string(),
            Self::FrameCharaImage => c.selection.frame_chara_image.to_string_lossy().into_owned(),
            Self::MmdModel => c.mmd.model_path.to_string_lossy().into_owned(),
            Self::MmdMotion => c.mmd.motion_path.to_string_lossy().into_owned(),
            Self::MmdMotionBpm => c.mmd.motion_bpm.to_string(),
            Self::MmdDisplayScale => c.mmd.display_scale.to_string(),
            Self::MmdPositionX => c.mmd.position_x.to_string(),
            Self::MmdPositionY => c.mmd.position_y.to_string(),
            Self::MmdQproHeight => c.mmd.qpro_height.to_string(),
            Self::MmdScrollUp => c.mmd.scroll_up_motion_path.to_string_lossy().into_owned(),
            Self::MmdScrollDown => c.mmd.scroll_down_motion_path.to_string_lossy().into_owned(),
            Self::MmdHappyMotion => c.mmd.happy_motion_path.to_string_lossy().into_owned(),
            Self::MmdOtherResult => c
                .mmd
                .other_result_motion_path
                .to_string_lossy()
                .into_owned(),
            Self::MmdHappy => c.mmd.happy_morph,
            Self::MmdLook => c.mmd.look_bone,
            Self::MmdLookYaw => c.mmd.look_yaw.to_string(),
            Self::MmdLookHold => c.mmd.look_hold_seconds.to_string(),
            Self::MmdBlink => c.mmd.blink_morph,
            Self::BiliVideo => c.bilibili.selection.video,
            Self::BiliPlaylist => c.bilibili.selection.playlist,
            Self::BiliFavourites => c.bilibili.selection.favourites,
            Self::BiliRoom => c.bilibili.selection.room,
        }
    }
}
pub(crate) struct SurfaceEditor {
    pub surface: Surface,
    inputs: Vec<Entity<InputState>>,
    subscriptions: Vec<Subscription>,
    pub error: Option<SharedString>,
}
impl SurfaceEditor {
    pub fn new(
        surface: Surface,
        config: &Config,
        window: &mut Window,
        cx: &mut Context<AppView>,
    ) -> Self {
        let inputs = SurfaceTextField::ALL
            .into_iter()
            .map(|field| {
                let value = field.value(config, surface);
                cx.new(|cx| InputState::new(window, cx).default_value(value))
            })
            .collect();
        let mut editor = Self {
            surface,
            inputs,
            subscriptions: Vec::new(),
            error: None,
        };
        for field in SurfaceTextField::ALL {
            let sub = cx.subscribe_in(
                editor.input_for(field),
                window,
                move |view, input, event, _, cx| {
                    if matches!(event, InputEvent::Change) {
                        view.on_surface_text(
                            surface,
                            field,
                            input.read(cx).value().to_string(),
                            cx,
                        );
                    }
                },
            );
            editor.subscriptions.push(sub);
        }
        editor
    }
    pub(crate) fn input_for(&self, field: SurfaceTextField) -> &Entity<InputState> {
        &self.inputs[field as usize]
    }
}
impl AppView {
    pub(crate) fn on_surface_text(
        &mut self,
        surface: Surface,
        field: SurfaceTextField,
        value: String,
        cx: &mut Context<Self>,
    ) {
        if !self
            .editor
            .as_ref()
            .is_some_and(|editor| editor.surface == surface)
        {
            return;
        }
        let value = value.trim_end_matches(['\r', '\n']);
        let limits = match field {
            SurfaceTextField::PageMax => Some((1, 100)),
            SurfaceTextField::TimeoutSeconds => Some((3, 60)),
            SurfaceTextField::MaxImages => Some((0, 1_000_000)),
            _ => None,
        };
        if let Some((min, max)) = limits {
            let number = value
                .trim()
                .parse::<u64>()
                .ok()
                .filter(|n| (min..=max).contains(n));
            if let Some(number) = number {
                self.commit_surface(500, cx, |doc| {
                    store::set_u64(doc, &field.path(surface), number)
                });
            } else {
                if let Some(editor) = &mut self.editor {
                    editor.error = Some(format!("请输入 {min}–{max} 之间的整数").into());
                }
                cx.notify();
            }
        } else if matches!(
            field,
            SurfaceTextField::MmdLookYaw
                | SurfaceTextField::MmdLookHold
                | SurfaceTextField::MmdMotionBpm
                | SurfaceTextField::MmdDisplayScale
                | SurfaceTextField::MmdPositionX
                | SurfaceTextField::MmdPositionY
                | SurfaceTextField::MmdQproHeight
        ) {
            let (min, max) = match field {
                SurfaceTextField::MmdLookYaw => (0., 80.),
                SurfaceTextField::MmdMotionBpm => (1., 1000.),
                SurfaceTextField::MmdDisplayScale => (10., 100.),
                SurfaceTextField::MmdQproHeight => (100., 800.),
                SurfaceTextField::MmdPositionX | SurfaceTextField::MmdPositionY => (0., 100.),
                _ => (0.1, 5.),
            };
            if let Some(number) = value
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|n| n.is_finite() && (min..=max).contains(n))
            {
                self.commit_surface(500, cx, |doc| {
                    store::set_f64(doc, &field.path(surface), number)
                });
            } else {
                if let Some(editor) = &mut self.editor {
                    editor.error = Some(format!("请输入 {min}–{max} 之间的数值").into());
                }
                cx.notify();
            }
        } else if field == SurfaceTextField::MediaHosts {
            let hosts = value
                .split([',', ' ', '\n'])
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>();
            self.commit_surface(500, cx, |doc| {
                store::set_strings(doc, &field.path(surface), &hosts)
            });
        } else {
            self.commit_surface(500, cx, |doc| {
                store::set_string(doc, &field.path(surface), value);
                if surface == Surface::Results && field == SurfaceTextField::MediaPath {
                    store::remove_value(doc, &["results_video"]);
                }
            });
        }
    }
    fn set_surface_enabled(&mut self, surface: Surface, enabled: bool, cx: &mut Context<Self>) {
        self.commit_surface(0, cx, |doc| {
            store::set_surface_enabled(doc, surface, enabled)
        });
    }
    fn set_surface_source(&mut self, surface: Surface, source: Source, cx: &mut Context<Self>) {
        let old = self.editor_config().for_surface(surface);
        self.commit_surface(0, cx, |doc| {
            store::set_string(
                doc,
                &["sources", surface.name(), "source"],
                store::source_key(source),
            );
            if surface == Surface::Results {
                if source == Source::File && !old.media_path.as_os_str().is_empty() {
                    store::set_string(
                        doc,
                        &["sources", surface.name(), "media_path"],
                        &old.media_path.to_string_lossy(),
                    );
                }
                store::remove_value(doc, &["results_video"]);
            }
        });
    }
    fn bind_surface(
        &mut self,
        surface: Surface,
        target: Option<Surface>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let source = self.editor_config().for_surface(surface).source;
        if self.commit_surface(0, cx, |doc| {
            let path = ["sources", surface.name(), "bind"];
            if let Some(target) = target {
                store::set_string(doc, &path, target.name());
            } else {
                store::remove_value(doc, &path);
                if doc
                    .get("sources")
                    .and_then(|item| item.get(surface.name()))
                    .and_then(|item| item.get("source"))
                    .is_none()
                {
                    store::set_string(
                        doc,
                        &["sources", surface.name(), "source"],
                        store::source_key(source),
                    );
                }
            }
        }) {
            self.editor = Some(SurfaceEditor::new(
                surface,
                &self.editor_config(),
                window,
                cx,
            ));
        }
    }
}
fn field_row(
    editor: &SurfaceEditor,
    field: SurfaceTextField,
    label: &str,
    picker: bool,
    cx: &mut Context<AppView>,
) -> AnyElement {
    h_flex()
        .w_full()
        .items_center()
        .gap_3()
        .child(
            div()
                .w(px(144.))
                .flex_shrink_0()
                .text_base()
                .child(label.to_owned()),
        )
        .child(
            Input::new(editor.input_for(field))
                .large()
                .flex_1()
                .min_w_0(),
        )
        .when(picker, |row| {
            row.child(
                Button::new(SharedString::from(format!("pick-{}", field.key())))
                    .large()
                    .outline()
                    .label("浏览…")
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.pick_surface_directory(field, window, cx)
                    })),
            )
        })
        .into_any_element()
}
pub(crate) fn render_editor(
    view: &mut AppView,
    editor: &SurfaceEditor,
    window: &mut Window,
    cx: &mut Context<AppView>,
) -> AnyElement {
    let surface = editor.surface;
    let config = view.editor_config();
    let c = config.for_surface(surface);
    let root = config.binding_root(surface).unwrap_or(surface);
    let bound = root != surface;
    let custom_artwork = view.artwork_editor;
    let mut body = v_flex()
        .id("surface-settings")
        .w_full()
        .p_7()
        .gap_6()
        .flex_1()
        .min_h_0()
        .overflow_y_scrollbar();
    if let Some(error) = editor
        .error
        .clone()
        .or_else(|| view.store.error().map(|e| e.to_owned().into()))
    {
        body = body.child(widgets::error_banner(error));
    }
    if let Some(index) = custom_artwork {
        let art = &view.store.config().artwork[index];
        body = body.child(
            div()
                .text_base()
                .child(format!("{} / {}", art.archive, art.texture)),
        );
        if !art.texture.starts_with("@shape/") && art.archive != "@movie" {
            body = body.child(
                h_flex().gap_3().child("保留原素材透明轮廓").child(
                    Switch::new("artwork-mask")
                        .checked(art.native_mask)
                        .on_change(cx.listener(move |view, checked, _, cx| {
                            view.store.edit(|doc| {
                                if let Some(t) = doc["artwork"]
                                    .as_array_of_tables_mut()
                                    .and_then(|a| a.get_mut(index))
                                {
                                    t.insert("native_mask", toml_edit::value(*checked));
                                }
                            });
                            cx.notify();
                        })),
                ),
            );
        }
    }
    let mut bindings = h_flex().gap_3().flex_wrap().child(
        widgets::segment_button("bind-independent", "独立设置", !bound).on_click(
            cx.listener(move |view, _, window, cx| view.bind_surface(surface, None, window, cx)),
        ),
    );
    for target in Surface::ALL.into_iter().filter(|s| surface.can_bind(*s)) {
        bindings = bindings.child(
            widgets::segment_button(
                SharedString::from(format!("bind-{}", target.name())),
                theme::surface_title(target),
                config.source_override(surface).bind == Some(target)
                    || (bound && config.source_override(surface).bind.is_none() && root == target),
            )
            .on_click(cx.listener(move |view, _, window, cx| {
                view.bind_surface(surface, Some(target), window, cx)
            })),
        );
    }
    body = body
        .child(div().text_base().child("绑定到组件"))
        .child(bindings);
    if bound {
        body = body
            .child(
                div()
                    .text_base()
                    .text_color(theme::muted_ink())
                    .child(format!(
                        "共享「{}」的来源和当前媒体；该组件的开关保持独立。",
                        theme::surface_title(root)
                    )),
            )
            .child(
                Button::new("edit-binding")
                    .outline()
                    .label("编辑绑定来源")
                    .on_click(
                        cx.listener(move |view, _, window, cx| view.open_editor(root, window, cx)),
                    ),
            );
    } else {
        let mut sources = h_flex().gap_3().flex_wrap();
        for (source, label) in [
            (Source::File, "固定图片 / 视频"),
            (Source::Local, "文件夹随机"),
            (Source::Remote, "Moebooru API"),
            (Source::Mixed, "文件夹 + API"),
            (Source::Bilibili, "Bilibili"),
        ] {
            sources = sources.child(
                widgets::segment_button(
                    SharedString::from(format!("source-{}", store::source_key(source))),
                    label,
                    c.source == source,
                )
                .on_click(
                    cx.listener(move |view, _, _, cx| view.set_surface_source(surface, source, cx)),
                ),
            );
        }
        body = body.child(sources);
        if c.source == Source::Bilibili {
            use iidxbg::bilibili::Mode;
            let mut modes = h_flex().gap_3().flex_wrap();
            for (mode, label) in [
                (Mode::Video, "固定视频"),
                (Mode::Recommendations, "随机推荐"),
                (Mode::Playlist, "合集 / 系列"),
                (Mode::Favourites, "收藏夹"),
                (Mode::Live, "直播间"),
            ] {
                modes = modes.child(
                    widgets::segment_button(
                        SharedString::from(format!("bili-mode-{}", mode.key())),
                        label,
                        c.bilibili.selection.mode == mode,
                    )
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.commit_surface(0, cx, |doc| {
                            store::set_string(
                                doc,
                                &["sources", surface.name(), "bilibili", "mode"],
                                mode.key(),
                            )
                        });
                    })),
                );
            }
            body = body.child(modes);
            match c.bilibili.selection.mode {
                Mode::Live => {
                    body = body.child(field_row(
                        editor,
                        SurfaceTextField::BiliRoom,
                        "直播间链接 / ID",
                        false,
                        cx,
                    ));
                }
                Mode::Video => {
                    body = body.child(field_row(
                        editor,
                        SurfaceTextField::BiliVideo,
                        "视频链接 / BV",
                        false,
                        cx,
                    ))
                }
                Mode::Playlist => {
                    body = body.child(field_row(
                        editor,
                        SurfaceTextField::BiliPlaylist,
                        "合集 / 系列链接",
                        false,
                        cx,
                    ))
                }
                Mode::Favourites => {
                    body = body.child(field_row(
                        editor,
                        SurfaceTextField::BiliFavourites,
                        "收藏夹链接 / ID",
                        false,
                        cx,
                    ));
                    let mut folders = h_flex().gap_3().flex_wrap();
                    for folder in &view.bili.folders {
                        let id = folder.id.to_string();
                        folders = folders.child(
                            Button::new(SharedString::from(format!("fav-{}", folder.id)))
                                .large()
                                .outline()
                                .label(folder.title.clone())
                                .on_click(cx.listener(move |view, _, window, cx| {
                                    view.commit_surface(0, cx, |doc| {
                                        store::set_string(
                                            doc,
                                            &["sources", surface.name(), "bilibili", "favourites"],
                                            &id,
                                        )
                                    });
                                    // The overlay temporarily owns its editor during render.
                                    if let Some(editor) = &view.editor {
                                        editor
                                            .input_for(SurfaceTextField::BiliFavourites)
                                            .update(cx, |state, cx| {
                                                state.set_value(id.clone(), window, cx)
                                            });
                                    }
                                })),
                        );
                    }
                    body = body.child(folders);
                }
                Mode::Recommendations => {
                    body = body.child(
                        div()
                            .text_base()
                            .child("使用已登录账号的推荐流，每次准备随机选择一个视频。"),
                    )
                }
            }
            body = body.child(
                div()
                    .text_sm()
                    .text_color(theme::muted_ink())
                    .child(if c.bilibili.selection.mode == Mode::Live {
                        "直播使用短分片缓冲，不保存整场直播；未开播时保留原画面，播放中断后自动重连。无音频。"
                    } else {
                        "先缓存 H.264 视频再播放。登录和收藏夹刷新请前往侧栏 Bilibili 页面。"
                    }),
            );
        }
        if c.source == Source::File {
            body = body.child(field_row(
                editor,
                SurfaceTextField::MediaPath,
                "媒体文件",
                true,
                cx,
            ));
        }
        if matches!(c.source, Source::Local | Source::Mixed) {
            body = body
                .child(field_row(
                    editor,
                    SurfaceTextField::LocalDirectory,
                    "随机文件夹",
                    true,
                    cx,
                ))
                .child(
                    h_flex()
                        .gap_3()
                        .child(div().text_base().child("包含子文件夹"))
                        .child(Switch::new("recursive").checked(c.recursive).on_change(
                            cx.listener(move |view, checked, _, cx| {
                                view.commit_surface(0, cx, |doc| {
                                    store::set_bool(
                                        doc,
                                        &["sources", surface.name(), "recursive"],
                                        *checked,
                                    )
                                });
                            }),
                        )),
                );
        }
        if matches!(c.source, Source::Remote | Source::Mixed) {
            let mut domains = h_flex().gap_3();
            for (domain, label) in [
                (MoebooruDomain::Net, "Konachan.net"),
                (MoebooruDomain::Com, "Konachan.com"),
                (MoebooruDomain::Yandere, "Yande.re"),
            ] {
                domains = domains.child(
                    widgets::segment_button(
                        SharedString::from(format!("domain-{label}")),
                        label,
                        c.moebooru.api_url.is_empty() && c.moebooru.domain == domain,
                    )
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.commit_surface(0, cx, |doc| {
                            store::set_string(
                                doc,
                                &["sources", surface.name(), "moebooru", "domain"],
                                store::domain_key(domain),
                            );
                            store::set_string(
                                doc,
                                &["sources", surface.name(), "moebooru", "api_url"],
                                "",
                            );
                        });
                        view.editor = Some(SurfaceEditor::new(
                            surface,
                            &view.editor_config(),
                            window,
                            cx,
                        ));
                    })),
                );
            }
            body = body
                .child(domains)
                .child(field_row(
                    editor,
                    SurfaceTextField::ApiUrl,
                    "自定义 API",
                    false,
                    cx,
                ))
                .child(
                    div()
                        .text_sm()
                        .text_color(theme::muted_ink())
                        .child("填完整 post.json 地址；留空使用上面选择的网站。"),
                )
                .child(field_row(
                    editor,
                    SurfaceTextField::MediaHosts,
                    "图片 CDN 域名",
                    false,
                    cx,
                ))
                .child(field_row(
                    editor,
                    SurfaceTextField::Tags,
                    "搜索标签",
                    false,
                    cx,
                ))
                .child(field_row(
                    editor,
                    SurfaceTextField::SaveDirectory,
                    "下载目录",
                    true,
                    cx,
                ))
                .child(field_row(
                    editor,
                    SurfaceTextField::PageMax,
                    "随机页数上限",
                    false,
                    cx,
                ))
                .child(field_row(
                    editor,
                    SurfaceTextField::TimeoutSeconds,
                    "超时（秒）",
                    false,
                    cx,
                ))
                .child(field_row(
                    editor,
                    SurfaceTextField::MaxImages,
                    "保留图片数量",
                    false,
                    cx,
                ));
        }
        if custom_artwork.is_none() {
            let mut fit = h_flex()
                .gap_3()
                .child(div().text_base().child("图片适配（视频拉伸）"));
            for (value, label) in [
                (Fit::Cover, "裁切铺满"),
                (Fit::Contain, "完整显示 + 模糊边缘"),
            ] {
                fit = fit.child(
                    widgets::segment_button(
                        SharedString::from(format!("fit-{}", store::fit_key(value))),
                        label,
                        c.image.fit == value,
                    )
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.commit_surface(0, cx, |doc| {
                            store::set_string(
                                doc,
                                &["sources", surface.name(), "fit"],
                                store::fit_key(value),
                            )
                        });
                    })),
                );
            }
            body = body.child(fit);
        }
    }
    if surface == Surface::SongSelect {
        body = body.child(
            h_flex().gap_3().child("实时 MMD 角色（PMX）").child(
                Switch::new("mmd-enabled")
                    .checked(config.mmd.enabled)
                    .on_change(cx.listener(|view, checked, _, cx| {
                        view.commit_surface(0, cx, |doc| {
                            store::set_bool(doc, &["mmd", "enabled"], *checked)
                        });
                    })),
            ),
        );
        if config.mmd.enabled {
            body=body.child(field_row(editor,SurfaceTextField::MmdModel,"PMX 模型",true,cx))
                .child(field_row(editor,SurfaceTextField::MmdMotion,"平时循环 VMD（可选）",true,cx))
                .child(div().text_sm().text_color(theme::muted_ink()).child("选择已解压的 PMX，保留旁边的贴图文件夹。VMD 静音循环；留空显示待机姿势。模型与动作均由用户提供。当前支持已验证的 IIDX 34 / 010-0917 构建。"));
            body = body.child(h_flex().gap_3().child("待机舞蹈跟随所选歌曲 BPM").child(
                Switch::new("mmd-bpm-sync")
                    .checked(config.mmd.bpm_sync)
                    .on_change(cx.listener(|view, checked, _, cx| {
                        view.commit_surface(0, cx, |doc| {
                            store::set_bool(doc, &["mmd", "bpm_sync"], *checked)
                        });
                    })),
            )).child(field_row(editor, SurfaceTextField::MmdMotionBpm, "平时动作原始 BPM", false, cx))
              .child(div().text_sm().text_color(theme::muted_ink()).child("仅在选曲待机时调速；进入演奏仍隐藏角色。VMD 不含 BPM，请填写动作原曲速度。变速曲取最高 BPM；未知时为原速，倍率限制 0.25–4 倍。搓盘与成绩动作保持原速。"));
            body = body.child(h_flex().gap_3().child("角色位置").child(
                widgets::segment_button("mmd-top-left", "左上角（避开顶栏）", false)
                    .on_click(cx.listener(|view, _, window, cx| {
                        if view.commit_surface(0, cx, |doc| {
                            store::set_f64(doc, &["mmd", "display_scale"], 42.);
                            store::set_f64(doc, &["mmd", "position_x"], 20.);
                            store::set_f64(doc, &["mmd", "position_y"], 32.);
                            store::set_bool(doc, &["mmd", "keep_hud_clear"], true);
                        }) {
                            view.editor = Some(SurfaceEditor::new(
                                Surface::SongSelect, view.store.config(), window, cx,
                            ));
                        }
                    })),
            ))
                .child(widgets::segment_button("mmd-original-size", "恢复大角色原始大小与位置", false)
                    .on_click(cx.listener(|view, _, window, cx| {
                        if view.commit_surface(0, cx, |doc| {
                            store::set_f64(doc, &["mmd", "display_scale"], 82.);
                            store::set_f64(doc, &["mmd", "position_x"], 50.);
                            store::set_f64(doc, &["mmd", "position_y"], 50.);
                            store::set_bool(doc, &["mmd", "keep_hud_clear"], false);
                        }) {
                            view.editor = Some(SurfaceEditor::new(Surface::SongSelect, view.store.config(), window, cx));
                        }
                    })))
                .child(field_row(editor, SurfaceTextField::MmdDisplayScale, "显示大小（10–100%）", false, cx))
                .child(field_row(editor, SurfaceTextField::MmdPositionX, "水平中心（0 左 → 100 右）", false, cx))
                .child(field_row(editor, SurfaceTextField::MmdPositionY, "垂直中心（0 上 → 100 下）", false, cx))
                .child(h_flex().gap_3().child("大角色避开顶栏与底栏").child(Switch::new("mmd-hud-clear")
                    .checked(config.mmd.keep_hud_clear).on_change(cx.listener(|view, value, _, cx| {
                        view.commit_surface(0, cx, |doc| store::set_bool(doc, &["mmd", "keep_hud_clear"], *value));
                    }))))
                .child(div().text_sm().text_color(theme::muted_ink()).child("大角色位置相对于原角色区域；避让开启时限制大小与位置。原始大小预设保留先前布局，歌曲详情仍在前景。"))
                .child(h_flex().gap_3().child("选曲 Qpro 替换为小 MMD").child(Switch::new("mmd-qpro")
                    .checked(config.mmd.replace_qpro).on_change(cx.listener(|view, value, _, cx| {
                        view.commit_surface(0, cx, |doc| store::set_bool(doc, &["mmd", "replace_qpro"], *value));
                    }))))
                .child(field_row(editor, SurfaceTextField::MmdQproHeight, "小 MMD 显示范围（100–800）", false, cx))
                .child(div().text_sm().text_color(theme::muted_ink()).child("小角色跟随原 Qpro 位置，与大角色共享模型和动作；大小独立。仅普通选曲生效。关闭下方的大角色开关可只显示小角色。"));
            let mut rate = h_flex().gap_3().child("角色帧率");
            for fps in [30, 60] {
                rate = rate.child(
                    widgets::segment_button(
                        SharedString::from(format!("mmd-fps-{fps}")),
                        if fps == 30 { "30 fps" } else { "60 fps" },
                        config.mmd.fps == fps,
                    )
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.commit_surface(0, cx, |doc| {
                            store::set_u64(doc, &["mmd", "fps"], fps.into())
                        });
                    })),
                );
            }
            body = body.child(rate).child(
                h_flex()
                    .gap_3()
                    .child("响应滚动与上次成绩（A / AA / AAA）")
                    .child(
                        Switch::new("mmd-reactions")
                            .checked(config.mmd.reactions)
                            .on_change(cx.listener(|view, checked, _, cx| {
                                view.commit_surface(0, cx, |doc| {
                                    store::set_bool(doc, &["mmd", "reactions"], *checked)
                                });
                            })),
                    ),
            );
            if config.mmd.reactions {
                body = body
                    .child(field_row(editor,SurfaceTextField::MmdScrollUp,"向上搓盘 / 抬头 VMD",true,cx))
                    .child(field_row(editor,SurfaceTextField::MmdScrollDown,"向下搓盘 / 低头 VMD",true,cx))
                    .child(field_row(editor,SurfaceTextField::MmdHappyMotion,"A / AA / AAA 后 VMD",true,cx))
                    .child(field_row(editor,SurfaceTextField::MmdOtherResult,"B～F 后 VMD",true,cx))
                    .child(div().text_sm().text_color(theme::muted_ink()).child("选曲时读取转盘方向，触发对应动作，播放一次后回到平时循环。留空则继续平时动作。自定义动作期间不叠加自动转头。成绩动作在返回选曲时触发，并优先于转盘动作。"))
                    .child(h_flex().gap_3().child("反转转盘动作方向").child(
                        Switch::new("mmd-scratch-reverse")
                            .checked(config.mmd.scratch_reverse)
                            .on_change(cx.listener(|view, checked, _, cx| {
                                view.commit_surface(0, cx, |doc| {
                                    store::set_bool(doc, &["mmd", "scratch_reverse"], *checked)
                                });
                            })),
                    ));
            }
            body=body.child(field_row(editor,SurfaceTextField::MmdLook,"转头骨骼名",false,cx))
                .child(field_row(editor,SurfaceTextField::MmdLookYaw,"向右看角度（度）",false,cx))
                .child(field_row(editor,SurfaceTextField::MmdLookHold,"停止滚动后保持（秒）",false,cx))
                .child(field_row(editor,SurfaceTextField::MmdHappy,"开心表情名",false,cx))
                .child(field_row(editor,SurfaceTextField::MmdBlink,"眨眼表情名",false,cx))
                .child(div().text_sm().text_color(theme::muted_ink()).child("名称须与模型一致，留空可禁用该动作。当前不模拟刚体 / 布料物理，也不执行 MME 特效。导入失败时保留原角色。"));
        }
        body = body
            .child(
                h_flex()
                    .gap_3()
                    .child(div().text_base().child("显示选曲角色"))
                    .child(
                        Switch::new("frame-chara")
                            .checked(config.selection.frame_chara)
                            .on_change(cx.listener(|view, checked, _, cx| {
                                view.commit_surface(0, cx, |doc| {
                                    store::set_bool(doc, &["selection", "frame_chara"], *checked)
                                });
                            })),
                    ),
            )
            .child(field_row(
                editor,
                SurfaceTextField::FrameCharaImage,
                "角色图片",
                true,
                cx,
            ))
            .child(
                div()
                    .text_sm()
                    .text_color(theme::muted_ink())
                    .child("提供透明背景的人物图片即可；自动去除透明空白，并适配原生角色位置。"),
            );
    }
    body = body.child(div().text_sm().text_color(theme::muted_ink()).child("自动保存。下次进入组件时生效；随机媒体在一次访问中保持不变。视频静音循环，预览显示首帧。"))
        .child(Button::new("refresh-preview").large().outline().label("刷新预览").on_click(cx.listener(move |view, _, _, cx| view.refresh_surface(surface, cx))));
    if custom_artwork.is_some() {
        if view.catalog.preview_loading {
            body = body.child(div().child("正在准备素材预览…"));
        }
        if let Some(error) = &view.catalog.preview_error {
            body = body.child(div().child(error.clone()));
        }
        if let Some(image) = &view.catalog.preview {
            body = body.child(
                div()
                    .text_sm()
                    .child("媒体预览；游戏内按原素材尺寸裁切，并保留原生动画。"),
            );
            body = body.child(
                gpui::img(image.clone())
                    .w_full()
                    .max_h(px(260.))
                    .object_fit(gpui::ObjectFit::Contain),
            );
        }
    }
    div()
        .id("surface-editor-overlay")
        .absolute()
        .inset_0()
        .bg(theme::overlay())
        .flex()
        .items_center()
        .justify_center()
        .p_4()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|view, _, _, cx| view.close_editor(cx)),
        )
        .child(
            v_flex()
                .id("surface-editor")
                .w(px(
                    (window.viewport_size().width.as_f32() - 32.).clamp(480., 960.)
                ))
                .h(px(
                    (window.viewport_size().height.as_f32() - 48.).clamp(360., 900.)
                ))
                .bg(theme::card_bg())
                .border_1()
                .border_color(theme::line())
                .overflow_hidden()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    h_flex()
                        .p_7()
                        .gap_6()
                        .items_center()
                        .border_b_1()
                        .border_color(theme::line())
                        .child(
                            div().flex_1().text_2xl().font_semibold().child(
                                view.artwork_editor
                                    .map(|index| view.store.config().artwork[index].texture.clone())
                                    .unwrap_or_else(|| theme::surface_title(surface).to_owned()),
                            ),
                        )
                        .child(
                            Switch::new("surface-enabled")
                                .checked(config.surface_enabled(surface))
                                .on_change(cx.listener(move |view, checked, _, cx| {
                                    view.set_surface_enabled(surface, *checked, cx)
                                })),
                        )
                        .child(
                            widgets::close_button("editor-close")
                                .on_click(cx.listener(|view, _, _, cx| view.close_editor(cx))),
                        ),
                )
                .child(body),
        )
        .into_any_element()
}
