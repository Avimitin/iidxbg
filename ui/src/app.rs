use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use async_channel::Sender;
use gpui::{
    App, Context, IntoElement, ObjectFit, ParentElement, PathPromptOptions, RenderImage,
    SharedString, Styled, Window, div, img, prelude::FluentBuilder as _, px, relative,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::{StyledExt, TitleBar, h_flex, v_flex};
use gpui_kit::*;
use iidxbg::config::Surface;
use toml_edit::DocumentMut;

use crate::{
    editor,
    editor::{SurfaceEditor, SurfaceTextField},
    preview,
    preview::{PreviewFingerprint, PreviewOutcome, PreviewRequest, PreviewSpec},
    store::{self, ConfigStore},
    theme, widgets,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PreviewStatus {
    Empty,
    Loading,
    Ready,
    Failed,
    Disabled,
}

pub(crate) struct PreviewSlot {
    pub status: PreviewStatus,
    pub generation: u64,
    pub image: Option<Arc<RenderImage>>,
    pub detail: Option<SharedString>,
    pub error: Option<SharedString>,
    pub fingerprint: Option<PreviewFingerprint>,
    /// A copied loading preview has no job of its own. If its source is disabled,
    /// the new leader must enqueue a job instead of waiting on a cancelled one.
    requesting: bool,
}

impl Default for PreviewSlot {
    fn default() -> Self {
        Self {
            status: PreviewStatus::Empty,
            generation: 0,
            image: None,
            detail: None,
            error: None,
            fingerprint: None,
            requesting: false,
        }
    }
}

pub struct AppView {
    pub(crate) bili: crate::bilibili::State,
    pub(crate) bili_page: bool,
    pub(crate) artwork_page: bool,
    pub(crate) artwork_editor: Option<usize>,
    pub(crate) catalog: crate::artwork::Catalog,
    pub(crate) store: ConfigStore,
    pub(crate) editor: Option<SurfaceEditor>,
    pub(crate) previews: [PreviewSlot; Surface::COUNT],
    pub(crate) defaults: [Arc<RenderImage>; Surface::COUNT],
    pub(crate) hovered_surface: Option<Surface>,
    jobs: Sender<PreviewRequest>,
    preview_sync_seq: u64,
}

impl AppView {
    pub fn new(store: ConfigStore, window: &mut Window, cx: &mut Context<Self>) -> Self {
        window.set_window_title("iidxbg 配置");
        let config = store.config().clone();
        let defaults: [Arc<RenderImage>; Surface::COUNT] = std::array::from_fn(|index| {
            let surface = Surface::ALL[index];
            let (width, height) = theme::surface_size(&config, surface);
            let scale = 720.0 / width.max(height) as f32;
            preview::default_image(
                (width as f32 * scale).round().max(2.0) as u32,
                (height as f32 * scale).round().max(2.0) as u32,
            )
        });

        let (job_tx, job_rx) = async_channel::unbounded::<PreviewRequest>();
        let (outcome_tx, outcome_rx) = async_channel::unbounded::<PreviewOutcome>();
        preview::spawn_worker(store.path().to_path_buf(), job_rx, outcome_tx);

        let mut this = Self {
            bili: crate::bilibili::State::new(cx),
            bili_page: false,
            artwork_page: false,
            artwork_editor: None,
            catalog: crate::artwork::Catalog::new(window, cx),
            store,
            editor: None,
            previews: std::array::from_fn(|_| PreviewSlot::default()),
            defaults,
            hovered_surface: None,
            jobs: job_tx,
            preview_sync_seq: 0,
        };

        cx.spawn(async move |this, cx| {
            while let Ok(outcome) = outcome_rx.recv().await {
                let alive = this
                    .update(cx, |this, cx| this.on_preview(outcome, cx))
                    .is_ok();
                if !alive {
                    break;
                }
            }
        })
        .detach();

        this.restore_catalog(cx);
        this.schedule_preview_sync(cx, 120);
        this
    }

    pub(crate) fn commit_surface(
        &mut self,
        delay_ms: u64,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut DocumentMut),
    ) -> bool {
        let ok = if let Some(index) = self.artwork_editor {
            self.store.edit_artwork(index, edit)
        } else {
            self.store.edit(edit)
        };
        if let Some(editor) = &mut self.editor {
            if ok {
                editor.error = None;
            } else {
                editor.error = Some(
                    self.store
                        .error()
                        .unwrap_or("配置无效，尚未写入")
                        .to_owned()
                        .into(),
                );
            }
        }
        if ok {
            if self.artwork_editor.is_some() {
                self.catalog.invalidate_preview();
            }
            self.schedule_preview_sync(cx, delay_ms);
        }
        cx.notify();
        ok
    }

    pub(crate) fn pick_surface_directory(
        &mut self,
        field: SurfaceTextField,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(surface) = self.editor.as_ref().map(|e| e.surface) else {
            return;
        };
        let prompt = match field {
            SurfaceTextField::MediaPath => "选择图片或视频",
            SurfaceTextField::FrameCharaImage => "选择角色图片",
            SurfaceTextField::LocalDirectory => "选择组件本地图片目录",
            SurfaceTextField::SaveDirectory => "选择组件下载保存目录",
            _ => return,
        };
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: matches!(
                field,
                SurfaceTextField::MediaPath | SurfaceTextField::FrameCharaImage
            ),
            directories: !matches!(
                field,
                SurfaceTextField::MediaPath | SurfaceTextField::FrameCharaImage
            ),
            multiple: false,
            prompt: Some(prompt.into()),
        });
        let window_handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = receiver.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let value = path.to_string_lossy().into_owned();
            let _ = window_handle.update(cx, |_, window, app| {
                let _ = this.update(app, |this, cx| {
                    if this.editor.as_ref().is_some_and(|e| e.surface == surface) {
                        this.apply_picked_surface_directory(field, value, window, cx);
                    }
                });
            });
        })
        .detach();
    }

    fn apply_picked_surface_directory(
        &mut self,
        field: SurfaceTextField,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some((surface, input)) = self
            .editor
            .as_ref()
            .map(|editor| (editor.surface, editor.input_for(field).clone()))
        else {
            return;
        };
        set_input(&input, value.clone(), window, cx);
        self.on_surface_text(surface, field, value, cx);
    }

    pub(crate) fn open_editor(
        &mut self,
        surface: Surface,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.artwork_editor = None;
        self.hovered_surface = None;
        if self
            .editor
            .as_ref()
            .is_some_and(|editor| editor.surface == surface)
        {
            return;
        }
        let editor = SurfaceEditor::new(surface, self.store.config(), window, cx);
        self.editor = Some(editor);
        cx.notify();
    }

    pub(crate) fn close_editor(&mut self, cx: &mut Context<Self>) {
        self.editor = None;
        self.artwork_editor = None;
        cx.notify();
    }

    fn schedule_preview_sync(&mut self, cx: &mut Context<Self>, delay_ms: u64) {
        self.preview_sync_seq = self.preview_sync_seq.wrapping_add(1);
        let sequence = self.preview_sync_seq;
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(delay_ms))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.preview_sync_seq == sequence {
                    this.sync_previews(cx);
                }
            });
        })
        .detach();
    }

    fn sync_previews(&mut self, cx: &mut Context<Self>) {
        let specs = preview::specs(self.store.config());
        for spec in &specs {
            if !spec.enabled {
                self.disable_slot(spec.surface, spec.fingerprint.clone());
            }
        }
        for root in Surface::ALL {
            let Some(leader) = specs
                .iter()
                .find(|s| s.enabled && s.surface == root && s.target == root)
                .or_else(|| specs.iter().find(|s| s.enabled && s.target == root))
            else {
                continue;
            };
            if !self.slot_is_current(leader.surface.index(), leader) {
                self.start_preview(leader.surface, root, leader.fingerprint.clone(), cx);
            }
            self.copy_bound_previews(leader.surface, cx);
        }
    }

    pub(crate) fn refresh_bilibili_previews(&mut self, cx: &mut Context<Self>) {
        for surface in Surface::ALL {
            if self.store.config().for_surface(surface).source == iidxbg::config::Source::Bilibili {
                self.previews[surface.index()].fingerprint = None;
            }
        }
        self.schedule_preview_sync(cx, 0);
    }

    fn slot_is_current(&self, index: usize, spec: &PreviewSpec) -> bool {
        let slot = &self.previews[index];
        slot.fingerprint.as_ref() == Some(&spec.fingerprint)
            && (slot.status != PreviewStatus::Loading || slot.requesting)
            && matches!(
                slot.status,
                PreviewStatus::Loading | PreviewStatus::Ready | PreviewStatus::Failed
            )
    }

    fn disable_slot(&mut self, surface: Surface, fingerprint: PreviewFingerprint) {
        let slot = &mut self.previews[surface.index()];
        slot.status = PreviewStatus::Disabled;
        slot.requesting = false;
        slot.image = None;
        slot.detail = None;
        slot.error = None;
        slot.generation = slot.generation.wrapping_add(1);
        slot.fingerprint = Some(fingerprint);
    }

    fn start_preview(
        &mut self,
        surface: Surface,
        target: Surface,
        fingerprint: PreviewFingerprint,
        cx: &mut Context<Self>,
    ) {
        let index = surface.index();
        let slot = &mut self.previews[index];
        slot.generation = slot.generation.wrapping_add(1);
        slot.status = PreviewStatus::Loading;
        slot.requesting = true;
        slot.image = None;
        slot.detail = None;
        slot.error = None;
        slot.fingerprint = Some(fingerprint);
        let request = PreviewRequest {
            surface,
            target,
            generation: slot.generation,
        };
        let _ = self.jobs.try_send(request);
        cx.notify();
    }

    pub(crate) fn refresh_surface(&mut self, surface: Surface, cx: &mut Context<Self>) {
        if self.artwork_editor.is_some() {
            self.refresh_artwork_preview(cx);
            return;
        }
        let config = self.store.config().clone();
        if !store::effective_enabled(&config, surface) {
            return;
        }
        let specs = preview::specs(&config);
        let spec = &specs[surface.index()];
        let target = spec.target;
        let fingerprint = spec.fingerprint.clone();
        self.start_preview(surface, target, fingerprint, cx);
        self.copy_bound_previews(surface, cx);
    }

    fn on_preview(&mut self, outcome: PreviewOutcome, cx: &mut Context<Self>) {
        let index = outcome.surface.index();
        let slot = &mut self.previews[index];
        if slot.generation != outcome.generation || slot.status != PreviewStatus::Loading {
            return;
        }
        slot.requesting = false;
        match outcome.result {
            Ok(prepared) => {
                let detail = preview::short_label(&prepared.label);
                let image = preview::render_image(prepared);
                slot.status = PreviewStatus::Ready;
                slot.image = Some(image);
                slot.detail = Some(detail);
                slot.error = None;
            }
            Err(error) => {
                slot.status = PreviewStatus::Failed;
                slot.image = None;
                slot.detail = None;
                slot.error = Some(error.into());
            }
        }
        self.copy_bound_previews(outcome.surface, cx);
        cx.notify();
    }

    fn copy_bound_previews(&mut self, leader: Surface, cx: &mut Context<Self>) {
        let specs = preview::specs(self.store.config());
        let slot = &self.previews[leader.index()];
        let Some(fingerprint) = slot.fingerprint.clone() else {
            return;
        };
        let (image, detail, error, status) = (
            slot.image.clone(),
            slot.detail.clone(),
            slot.error.clone(),
            slot.status,
        );
        for spec in &specs {
            if spec.surface == leader
                || !spec.enabled
                || !fingerprint.same_source(&spec.fingerprint)
            {
                continue;
            }
            let slot = &mut self.previews[spec.surface.index()];
            slot.generation = slot.generation.wrapping_add(1);
            slot.fingerprint = Some(spec.fingerprint.clone());
            slot.image = image.clone();
            slot.detail = detail.clone();
            slot.error = error.clone();
            slot.status = status;
            slot.requesting = false;
        }
        cx.notify();
    }

    fn render_editor_overlay(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let editor = self.editor.take()?;
        let element = editor::render_editor(self, &editor, window, cx);
        self.editor = Some(editor);
        Some(element)
    }
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let overlay = self.render_editor_overlay(window, cx);
        let content = if self.artwork_page {
            self.render_artwork(window, cx)
        } else if self.bili_page {
            self.render_bilibili(cx)
        } else {
            self.render_gallery(window, cx)
        };
        let sidebar = v_flex()
            .w(px(theme::SIDEBAR_WIDTH))
            .h_full()
            .flex_shrink_0()
            .py_4()
            .items_center()
            .gap_4()
            .bg(theme::sidebar_bg())
            .border_r_1()
            .border_color(theme::line())
            .child(
                widgets::navigation_button(
                    "page-surfaces",
                    IconName::LayoutGrid,
                    "显示组件",
                    !self.bili_page && !self.artwork_page,
                )
                .on_click(cx.listener(|view, _, _, cx| {
                    view.bili_page = false;
                    view.artwork_page = false;
                    view.editor = None;
                    cx.notify();
                })),
            )
            .child(
                widgets::navigation_button(
                    "page-bilibili",
                    IconName::TvMinimalPlay,
                    "Bilibili",
                    self.bili_page,
                )
                .on_click(cx.listener(|view, _, _, cx| {
                    view.bili_page = true;
                    view.artwork_page = false;
                    view.editor = None;
                    cx.notify();
                })),
            );
        let sidebar = sidebar.child(
            widgets::navigation_button(
                "page-artwork",
                IconName::Image,
                "全部素材",
                self.artwork_page,
            )
            .on_click(cx.listener(|view, _, _, cx| {
                view.artwork_page = true;
                view.bili_page = false;
                view.editor = None;
                view.artwork_editor = None;
                cx.notify();
            })),
        );
        let title_bar = TitleBar::new()
            .h(px(theme::TITLE_BAR_HEIGHT))
            .pl_6()
            .bg(theme::sidebar_bg())
            .border_color(theme::line())
            .child(
                div()
                    .text_size(px(18.))
                    .font_semibold()
                    .child("iidxbg 配置"),
            );

        v_flex()
            .id("iidxbg-root")
            .relative()
            .size_full()
            .bg(theme::page_bg())
            .text_color(theme::ink())
            .child(title_bar)
            .child(
                h_flex()
                    .w_full()
                    .flex_1()
                    .min_h_0()
                    .child(sidebar)
                    .child(v_flex().flex_1().min_w_0().h_full().child(content)),
            )
            .when_some(overlay, |this, overlay| this.child(overlay))
    }
}

fn set_input(
    input: &gpui::Entity<gpui_kit::component::input::InputState>,
    value: impl Into<SharedString>,
    window: &mut Window,
    cx: &mut App,
) {
    input.update(cx, |state, cx| {
        state.set_value(value.into(), window, cx);
    });
}

pub(crate) fn config_path_from_args() -> Result<PathBuf, String> {
    let mut args = std::env::args().skip(1);
    let mut explicit = None;
    while let Some(argument) = args.next() {
        if argument == "--config" {
            explicit = args.next();
        } else if let Some(value) = argument.strip_prefix("--config=") {
            explicit = Some(value.to_owned());
        }
    }
    if let Some(path) = explicit.or_else(|| std::env::var("IIDXBG_CONFIG").ok()) {
        return Ok(PathBuf::from(path));
    }

    let mut candidates = Vec::new();
    if let Some(directory) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    {
        candidates.push(directory.join("iidxbg.toml"));
    }
    if let Ok(directory) = std::env::current_dir() {
        candidates.push(directory.join("iidxbg.toml"));
    }
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("iidxbg.toml"));
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("iidxbg.toml"),
    );
    candidates
        .into_iter()
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| {
            "找不到 iidxbg.toml。请把配置文件放在程序旁边，或用 --config <路径> 指定。".to_owned()
        })
}

pub struct ErrorView {
    pub message: String,
    pub path: Option<PathBuf>,
}

impl Render for ErrorView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .bg(theme::page_bg())
            .child(
                v_flex()
                    .w(px(560.))
                    .p_8()
                    .gap_4()
                    .bg(theme::card_bg())
                    .border_1()
                    .border_color(theme::line())
                    .child(widgets::icon_box(
                        IconName::TriangleAlert,
                        theme::danger(),
                        44.,
                    ))
                    .child(
                        div()
                            .text_xl()
                            .font_semibold()
                            .child("无法打开 iidxbg 配置"),
                    )
                    .child(
                        div()
                            .text_base()
                            .line_height(relative(1.55))
                            .text_color(theme::muted_ink())
                            .child(self.message.clone()),
                    )
                    .when_some(self.path.as_ref(), |this, path| {
                        this.child(widgets::muted_path(path.display().to_string()))
                    })
                    .child(
                        div()
                            .text_sm()
                            .line_height(relative(1.5))
                            .text_color(theme::faint_ink())
                            .child(
                                "启动参数示例：iidxbg-config.exe --config D:/path/to/iidxbg.toml",
                            ),
                    ),
            )
    }
}

pub(crate) fn open_window_with_config(
    config: Result<ConfigStore, String>,
    path: Option<PathBuf>,
    cx: &mut App,
) -> Result<(), String> {
    // GPUI display and window bounds are both logical pixels, so use the
    // display size directly. Dividing by the DPI scale here would halve the
    // window on a 4K display running at 200%.
    let (available_width, available_height) = cx
        .primary_display()
        .map(|display| {
            let bounds = display.visible_bounds();
            (bounds.size.width.as_f32(), bounds.size.height.as_f32())
        })
        .unwrap_or((1440., 900.));
    let width = 1600.0_f32.min((available_width - 32.).max(640.));
    let height = 960.0_f32.min((available_height - 24.).max(420.));
    let min_width = 1200.0_f32.min(width);
    let min_height = 720.0_f32.min(height);
    let window_bounds = Some(WindowBounds::centered(size(px(width), px(height)), cx));
    let window_min_size = Some(size(px(min_width), px(min_height)));
    match config {
        Ok(store) => {
            let options = WindowOptions {
                window_bounds,
                window_min_size,
                ..TitleBar::window_options()
            };
            gpui_kit::open_window(options, cx, move |window, cx| {
                cx.new(|cx| AppView::new(store, window, cx))
            })
            .map_err(|error| format!("无法创建窗口：{error}"))?;
        }
        Err(message) => {
            let view = ErrorView { message, path };
            let options = WindowOptions {
                window_bounds,
                window_min_size,
                titlebar: Some(TitlebarOptions {
                    title: Some("iidxbg 配置".into()),
                    ..Default::default()
                }),
                ..Default::default()
            };
            gpui_kit::open_window(options, cx, move |_, cx| cx.new(|_| view))
                .map_err(|error| format!("无法创建窗口：{error}"))?;
        }
    }
    Ok(())
}

pub(crate) fn install_theme(cx: &mut App) {
    theme::install(cx);
}

pub(crate) fn image_element(image: Arc<RenderImage>, cover: bool) -> gpui::Img {
    let element = img(gpui::ImageSource::Render(image));
    if cover {
        element.object_fit(ObjectFit::Cover)
    } else {
        element
    }
}
