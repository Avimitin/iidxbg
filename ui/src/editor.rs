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

mod pages;
use pages::EditorPage;

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
    page: EditorPage,
    search: Entity<InputState>,
    query: String,
    focus: gpui::FocusHandle,
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
            page: EditorPage::Media,
            search: cx.new(|cx| {
                InputState::new(window, cx).placeholder(if surface == Surface::SongSelect {
                    "搜索设置，如 Qpro、BPM、图片…"
                } else {
                    "搜索设置，如图片、共享、下载…"
                })
            }),
            query: String::new(),
            focus: cx.focus_handle(),
            inputs,
            subscriptions: Vec::new(),
            error: None,
        };
        let search_subscription = cx.subscribe(&editor.search, |view, input, event, cx| {
            if matches!(event, InputEvent::Change) {
                if let Some(editor) = &mut view.editor {
                    editor.query = input.read(cx).value().trim().to_lowercase();
                }
                cx.notify();
            }
        });
        editor.subscriptions.push(search_subscription);
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
    pub(crate) fn focus_handle(&self) -> &gpui::FocusHandle {
        &self.focus
    }
}
impl AppView {
    fn editor_page(&mut self, page: EditorPage, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.editor {
            editor.page = page;
            editor.query.clear();
            editor
                .search
                .update(cx, |input, cx| input.set_value("", window, cx));
            editor.focus.focus(window, cx);
        }
        cx.notify();
    }

    fn rebuild_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(old) = self.editor.take() {
            let mut editor = SurfaceEditor::new(old.surface, &self.editor_config(), window, cx);
            editor.page = old.page;
            editor.query = old.query;
            editor.search.update(cx, |input, cx| {
                input.set_value(editor.query.clone(), window, cx)
            });
            self.editor = Some(editor);
        }
    }
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
            self.rebuild_editor(window, cx);
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
    v_flex()
        .w_full()
        .min_w_0()
        .flex_1()
        .gap_2()
        .child(
            div()
                .text_sm()
                .text_color(theme::muted_ink())
                .child(label.to_owned()),
        )
        .child(
            h_flex()
                .gap_2()
                .w_full()
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
                }),
        )
        .into_any_element()
}

pub(crate) fn render_editor(
    view: &mut AppView,
    editor: &SurfaceEditor,
    window: &mut Window,
    cx: &mut Context<AppView>,
) -> AnyElement {
    pages::render(view, editor, window, cx)
}
