use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use async_channel::Sender;
use gpui::{
    App, Context, IntoElement, ObjectFit, ParentElement, PathPromptOptions, RenderImage,
    SharedString, Styled, Subscription, Window, div, img, prelude::FluentBuilder as _, px,
    relative,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    Icon, Placement, Selectable, Sizable, Size, StyledExt, TitleBar,
    button::{Button, ButtonVariants},
    h_flex, v_flex,
};
use gpui_kit::*;
use iidxbg::config::{Config, Fit, MoebooruDomain, Source, Surface};
use toml_edit::DocumentMut;

use crate::{
    editor,
    editor::{SurfaceEditor, SurfaceTextField},
    global::GlobalInputs,
    preview,
    preview::{PreviewFingerprint, PreviewOutcome, PreviewRequest, PreviewSpec},
    store::{self, ConfigStore},
    theme, widgets,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Page {
    Gallery,
    Global,
}

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
        }
    }
}

pub struct AppView {
    pub(crate) store: ConfigStore,
    pub(crate) page: Page,
    pub(crate) editor: Option<SurfaceEditor>,
    pub(crate) previews: [PreviewSlot; Surface::COUNT],
    pub(crate) defaults: [Arc<RenderImage>; Surface::COUNT],
    pub(crate) global_inputs: GlobalInputs,
    global_subscriptions: Vec<Subscription>,
    pub(crate) global_error: Option<SharedString>,
    pub(crate) hovered_surface: Option<Surface>,
    pub(crate) hovered_global: bool,
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

        let global_inputs = GlobalInputs::new(&config, window, cx);
        let (job_tx, job_rx) = async_channel::unbounded::<PreviewRequest>();
        let (outcome_tx, outcome_rx) = async_channel::unbounded::<PreviewOutcome>();
        preview::spawn_worker(store.path().to_path_buf(), job_rx, outcome_tx);

        let mut this = Self {
            store,
            page: Page::Gallery,
            editor: None,
            previews: std::array::from_fn(|_| PreviewSlot::default()),
            defaults,
            global_inputs,
            global_subscriptions: Vec::new(),
            global_error: None,
            hovered_surface: None,
            hovered_global: false,
            jobs: job_tx,
            preview_sync_seq: 0,
        };
        this.bind_global_inputs(window, cx);

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

        this.schedule_preview_sync(cx, 120);
        this
    }

    fn bind_global_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use GlobalTextField::*;

        for (field, input) in [
            (LocalDirectory, &self.global_inputs.local_directory),
            (SaveDirectory, &self.global_inputs.save_directory),
            (Tags, &self.global_inputs.tags),
            (Width, &self.global_inputs.width),
            (Height, &self.global_inputs.height),
            (MaxDownloadMb, &self.global_inputs.max_download_mb),
            (MaxSourcePixels, &self.global_inputs.max_source_pixels),
            (PageMax, &self.global_inputs.page_max),
            (TimeoutSeconds, &self.global_inputs.timeout_seconds),
            (MaxImages, &self.global_inputs.max_images),
            (FrameCharaImage, &self.global_inputs.frame_chara_image),
        ] {
            let subscription =
                cx.subscribe_in(input, window, move |this, state, event, _window, cx| {
                    if matches!(event, gpui_kit::component::input::InputEvent::Change) {
                        let value = state.read(cx).value().to_string();
                        this.on_global_text(field, value, cx);
                    }
                });
            self.global_subscriptions.push(subscription);
        }
    }

    fn set_global_inputs_from_config(&self, window: &mut Window, cx: &mut Context<Self>) {
        let config = self.store.config();
        set_input(
            &self.global_inputs.local_directory,
            config.local_directory.to_string_lossy(),
            window,
            cx,
        );
        set_input(
            &self.global_inputs.save_directory,
            config.moebooru.save_directory.to_string_lossy(),
            window,
            cx,
        );
        set_input(
            &self.global_inputs.tags,
            config.moebooru.tags.clone(),
            window,
            cx,
        );
        set_input(
            &self.global_inputs.width,
            config.image.width.to_string(),
            window,
            cx,
        );
        set_input(
            &self.global_inputs.height,
            config.image.height.to_string(),
            window,
            cx,
        );
        set_input(
            &self.global_inputs.max_download_mb,
            config.image.max_download_mb.to_string(),
            window,
            cx,
        );
        set_input(
            &self.global_inputs.max_source_pixels,
            config.image.max_source_pixels.to_string(),
            window,
            cx,
        );
        set_input(
            &self.global_inputs.frame_chara_image,
            config.selection.frame_chara_image.to_string_lossy(),
            window,
            cx,
        );
    }

    pub(crate) fn commit_global(
        &mut self,
        delay_ms: u64,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut DocumentMut),
    ) -> bool {
        let ok = self.store.edit(edit);
        if ok {
            self.global_error = None;
            self.schedule_preview_sync(cx, delay_ms);
        } else {
            self.global_error = Some(
                self.store
                    .error()
                    .unwrap_or("配置无效，尚未写入")
                    .to_owned()
                    .into(),
            );
        }
        cx.notify();
        ok
    }

    pub(crate) fn commit_surface(
        &mut self,
        delay_ms: u64,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut DocumentMut),
    ) -> bool {
        let ok = self.store.edit(edit);
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
            self.schedule_preview_sync(cx, delay_ms);
        }
        cx.notify();
        ok
    }

    fn on_global_text(&mut self, field: GlobalTextField, value: String, cx: &mut Context<Self>) {
        let value = value.trim_end_matches(['\r', '\n']).to_owned();
        let invalid = |this: &mut Self, message: &str, cx: &mut Context<Self>| {
            this.global_error = Some(message.to_owned().into());
            cx.notify();
        };
        match field {
            GlobalTextField::LocalDirectory => {
                if value.trim().is_empty() {
                    return invalid(self, "本地目录不能为空", cx);
                }
                self.commit_global(500, cx, |doc| {
                    store::set_string(doc, &["local_directory"], &value);
                });
            }
            GlobalTextField::SaveDirectory => {
                if value.trim().is_empty() {
                    return invalid(self, "下载目录不能为空", cx);
                }
                self.commit_global(500, cx, |doc| {
                    store::set_string(doc, &["moebooru", "save_directory"], &value);
                });
            }
            GlobalTextField::Tags => {
                if value.chars().count() > 1000 {
                    return invalid(self, "标签长度不能超过 1000 个字符", cx);
                }
                self.commit_global(500, cx, |doc| {
                    store::set_string(doc, &["moebooru", "tags"], &value);
                });
            }
            GlobalTextField::FrameCharaImage => {
                self.commit_global(500, cx, |doc| {
                    store::set_string(doc, &["selection", "frame_chara_image"], &value);
                });
            }
            GlobalTextField::Width | GlobalTextField::Height => {
                let (label, min, max, key) = match field {
                    GlobalTextField::Width => ("宽度", 64u64, 3840, "width"),
                    _ => ("高度", 64u64, 2160, "height"),
                };
                let Some(number) = parse_number(&value, label, min, max, cx, self) else {
                    return;
                };
                self.commit_global(500, cx, |doc| {
                    store::set_u32(doc, &["image", key], number as u32);
                });
            }
            GlobalTextField::MaxDownloadMb => {
                let Some(number) = parse_number(&value, "单文件上限", 1, 64, cx, self) else {
                    return;
                };
                self.commit_global(500, cx, |doc| {
                    store::set_u64(doc, &["image", "max_download_mb"], number);
                });
            }
            GlobalTextField::MaxSourcePixels => {
                let Some(number) = parse_number(&value, "源像素上限", 1, 64_000_000, cx, self)
                else {
                    return;
                };
                self.commit_global(500, cx, |doc| {
                    store::set_u64(doc, &["image", "max_source_pixels"], number);
                });
            }
            GlobalTextField::PageMax => {
                let Some(number) = parse_number(&value, "页数上限", 1, 100, cx, self) else {
                    return;
                };
                self.commit_global(500, cx, |doc| {
                    store::set_u32(doc, &["moebooru", "page_max"], number as u32);
                });
            }
            GlobalTextField::TimeoutSeconds => {
                let Some(number) = parse_number(&value, "请求超时", 3, 60, cx, self) else {
                    return;
                };
                self.commit_global(500, cx, |doc| {
                    store::set_u64(doc, &["moebooru", "timeout_seconds"], number);
                });
            }
            GlobalTextField::MaxImages => {
                let Some(number) = parse_number(&value, "下载保留上限", 0, 1_000_000, cx, self)
                else {
                    return;
                };
                self.commit_global(500, cx, |doc| {
                    store::set_usize(doc, &["moebooru", "max_images"], number as usize);
                });
            }
        }
    }

    pub(crate) fn set_master_enabled(&mut self, value: bool, cx: &mut Context<Self>) {
        self.commit_global(0, cx, |doc| {
            store::set_bool(doc, &["enabled"], value);
        });
    }

    pub(crate) fn set_global_source(&mut self, source: Source, cx: &mut Context<Self>) {
        self.commit_global(0, cx, |doc| {
            store::set_string(doc, &["source"], store::source_key(source));
        });
    }

    pub(crate) fn set_global_recursive(&mut self, value: bool, cx: &mut Context<Self>) {
        self.commit_global(0, cx, |doc| {
            store::set_bool(doc, &["recursive"], value);
        });
    }

    pub(crate) fn set_global_fit(&mut self, fit: Fit, cx: &mut Context<Self>) {
        self.commit_global(0, cx, |doc| {
            store::set_string(doc, &["image", "fit"], store::fit_key(fit));
        });
    }

    pub(crate) fn set_global_domain(&mut self, domain: MoebooruDomain, cx: &mut Context<Self>) {
        self.commit_global(0, cx, |doc| {
            store::set_string(doc, &["moebooru", "domain"], store::domain_key(domain));
        });
    }

    pub(crate) fn set_frame_chara(&mut self, value: bool, cx: &mut Context<Self>) {
        self.commit_global(0, cx, |doc| {
            store::set_bool(doc, &["selection", "frame_chara"], value);
        });
    }

    pub(crate) fn pick_global_directory(
        &mut self,
        field: GlobalTextField,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let prompt = match field {
            GlobalTextField::LocalDirectory => "选择本地图片目录",
            GlobalTextField::SaveDirectory => "选择下载保存目录",
            _ => return,
        };
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
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
                    this.apply_picked_global_directory(field, value, window, cx);
                });
            });
        })
        .detach();
    }

    pub(crate) fn pick_surface_directory(
        &mut self,
        field: SurfaceTextField,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let prompt = match field {
            SurfaceTextField::LocalDirectory => "选择组件本地图片目录",
            SurfaceTextField::SaveDirectory => "选择组件下载保存目录",
            _ => return,
        };
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
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
                    this.apply_picked_surface_directory(field, value, window, cx);
                });
            });
        })
        .detach();
    }

    fn apply_picked_global_directory(
        &mut self,
        field: GlobalTextField,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = match field {
            GlobalTextField::LocalDirectory => &self.global_inputs.local_directory,
            GlobalTextField::SaveDirectory => &self.global_inputs.save_directory,
            _ => return,
        };
        set_input(input, value.clone(), window, cx);
        self.on_global_text(field, value, cx);
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
        self.hovered_surface = None;
        self.hovered_global = false;
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
        cx.notify();
    }

    pub(crate) fn go_to_global(&mut self, cx: &mut Context<Self>) {
        self.page = Page::Global;
        self.editor = None;
        self.hovered_surface = None;
        self.hovered_global = false;
        cx.notify();
    }

    pub(crate) fn go_to_gallery(&mut self, cx: &mut Context<Self>) {
        self.page = Page::Gallery;
        self.editor = None;
        self.hovered_surface = None;
        self.hovered_global = false;
        cx.notify();
    }

    pub(crate) fn reload_from_disk(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.store.reload() {
            self.global_error = Some(
                self.store
                    .error()
                    .unwrap_or("重新载入失败")
                    .to_owned()
                    .into(),
            );
            cx.notify();
            return;
        }
        self.global_error = None;
        self.set_global_inputs_from_config(window, cx);
        if let Some(editor) = &self.editor {
            editor.set_inputs_from_config(self.store.config(), window, cx);
        }
        self.schedule_preview_sync(cx, 0);
        cx.notify();
    }

    pub(crate) fn reveal_config(&mut self) {
        let parent = self.store.dir();
        let _ = std::process::Command::new("explorer").arg(parent).spawn();
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
        let config = self.store.config().clone();
        let specs = preview::specs(&config);
        let shares = config.concentration_shares_premium();

        for (index, spec) in specs.iter().enumerate().take(Surface::COUNT - 1) {
            let surface = spec.surface;
            if !spec.enabled {
                self.disable_slot(surface, spec.fingerprint.clone());
                continue;
            }
            if self.slot_is_current(index, spec) {
                continue;
            }
            self.start_preview(surface, spec.target, spec.fingerprint.clone(), cx);
        }

        let concentration_index = Surface::Concentration.index();
        let concentration_spec = &specs[concentration_index];
        if !concentration_spec.enabled {
            self.disable_slot(
                Surface::Concentration,
                concentration_spec.fingerprint.clone(),
            );
            return;
        }

        if shares {
            let premium_index = Surface::PremiumArea.index();
            let premium_slot = &self.previews[premium_index];
            let same_source = premium_slot
                .fingerprint
                .as_ref()
                .is_some_and(|fingerprint| {
                    fingerprint.same_source(&concentration_spec.fingerprint)
                });
            if same_source && premium_slot.status == PreviewStatus::Ready {
                self.copy_premium_to_concentration(cx);
                return;
            }
            if same_source && premium_slot.status == PreviewStatus::Loading {
                let slot = &mut self.previews[concentration_index];
                slot.fingerprint = Some(concentration_spec.fingerprint.clone());
                slot.status = PreviewStatus::Loading;
                slot.image = None;
                slot.error = None;
                slot.detail = None;
                cx.notify();
                return;
            }
            // The premium job is either disabled or failed. Ask for a
            // concentration-target job, which still resolves the premium-area
            // source but updates the concentration card.
            if !self.slot_is_current(concentration_index, concentration_spec) {
                self.start_preview(
                    Surface::Concentration,
                    Surface::PremiumArea,
                    concentration_spec.fingerprint.clone(),
                    cx,
                );
            }
            return;
        }

        if !self.slot_is_current(concentration_index, concentration_spec) {
            self.start_preview(
                Surface::Concentration,
                concentration_spec.target,
                concentration_spec.fingerprint.clone(),
                cx,
            );
        }
    }

    fn slot_is_current(&self, index: usize, spec: &PreviewSpec) -> bool {
        let slot = &self.previews[index];
        slot.fingerprint.as_ref() == Some(&spec.fingerprint)
            && matches!(
                slot.status,
                PreviewStatus::Loading | PreviewStatus::Ready | PreviewStatus::Failed
            )
    }

    fn disable_slot(&mut self, surface: Surface, fingerprint: PreviewFingerprint) {
        let slot = &mut self.previews[surface.index()];
        slot.status = PreviewStatus::Disabled;
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
        let config = self.store.config().clone();
        if !store::effective_enabled(&config, surface) {
            return;
        }
        let specs = preview::specs(&config);
        let spec = &specs[surface.index()];
        let target = spec.target;
        let fingerprint = spec.fingerprint.clone();
        self.start_preview(surface, target, fingerprint, cx);
    }

    fn on_preview(&mut self, outcome: PreviewOutcome, cx: &mut Context<Self>) {
        let index = outcome.surface.index();
        let slot = &mut self.previews[index];
        if slot.generation != outcome.generation || slot.status != PreviewStatus::Loading {
            return;
        }
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
        if outcome.surface == Surface::PremiumArea {
            self.copy_premium_to_concentration(cx);
        }
        cx.notify();
    }

    fn copy_premium_to_concentration(&mut self, cx: &mut Context<Self>) {
        let config = self.store.config().clone();
        if !config.concentration_shares_premium()
            || !store::effective_enabled(&config, Surface::Concentration)
        {
            return;
        }
        let specs = preview::specs(&config);
        let concentration_spec = &specs[Surface::Concentration.index()];
        let premium = &self.previews[Surface::PremiumArea.index()];
        let same_source = premium
            .fingerprint
            .as_ref()
            .is_some_and(|fingerprint| fingerprint.same_source(&concentration_spec.fingerprint));
        if !same_source {
            return;
        }
        let image = premium.image.clone();
        let detail = premium.detail.clone();
        let error = premium.error.clone();
        let status = premium.status;
        let concentration = &mut self.previews[Surface::Concentration.index()];
        concentration.fingerprint = Some(concentration_spec.fingerprint.clone());
        match status {
            PreviewStatus::Ready => {
                concentration.status = PreviewStatus::Ready;
                concentration.image = image;
                concentration.detail = Some(match detail {
                    Some(detail) => format!("复用 premium-area · {detail}").into(),
                    None => "复用 premium-area 图片".into(),
                });
                concentration.error = None;
            }
            PreviewStatus::Failed => {
                concentration.status = PreviewStatus::Failed;
                concentration.image = None;
                concentration.detail = None;
                concentration.error = error;
            }
            _ => {}
        }
        cx.notify();
    }

    pub(crate) fn enabled_count(&self) -> usize {
        let config = self.store.config();
        Surface::ALL
            .iter()
            .filter(|surface| store::effective_enabled(config, **surface))
            .count()
    }

    fn render_sidebar(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let metrics = theme::layout_metrics(window.viewport_size().width.as_f32());
        let page = self.page;
        let config_ok = !self.store.has_draft_error() && self.global_error.is_none();

        let brand = div()
            .size(px(36.))
            .flex_shrink_0()
            .rounded(px(10.))
            .bg(theme::page_bg())
            .border_1()
            .border_color(theme::line_strong())
            .flex()
            .items_center()
            .justify_center()
            .child(
                Icon::new(IconName::Images)
                    .with_size(Size::Medium)
                    .text_color(theme::accent()),
            );

        let gallery = Button::new("rail-gallery")
            .ghost()
            .icon(Icon::new(IconName::LayoutDashboard).with_size(Size::Medium))
            .tooltip("组件图库")
            .tooltip_placement(Placement::Right)
            .selected(page == Page::Gallery)
            .w(px(38.))
            .h(px(38.))
            .rounded(px(9.))
            .border_1()
            .when(page == Page::Gallery, |this| {
                this.bg(theme::page_bg())
                    .border_color(theme::line_strong())
                    .text_color(theme::accent())
            })
            .when(page != Page::Gallery, |this| {
                this.border_color(theme::line_soft())
                    .text_color(theme::ink_soft())
            })
            .on_click(cx.listener(|this, _, _, cx| this.go_to_gallery(cx)));

        let global = Button::new("rail-global")
            .ghost()
            .icon(Icon::new(IconName::Settings2).with_size(Size::Medium))
            .tooltip("全局选项")
            .tooltip_placement(Placement::Right)
            .selected(page == Page::Global)
            .w(px(38.))
            .h(px(38.))
            .rounded(px(9.))
            .border_1()
            .when(page == Page::Global, |this| {
                this.bg(theme::page_bg())
                    .border_color(theme::line_strong())
                    .text_color(theme::accent())
            })
            .when(page != Page::Global, |this| {
                this.border_color(theme::line_soft())
                    .text_color(theme::ink_soft())
            })
            .on_click(cx.listener(|this, _, _, cx| this.go_to_global(cx)));

        v_flex()
            .w(px(metrics.sidebar))
            .h_full()
            .flex_shrink_0()
            .items_center()
            .py_4()
            .gap_2()
            .bg(theme::sidebar_bg())
            .border_r_1()
            .border_color(theme::line())
            .child(brand)
            .child(div().h(px(14.)).flex_shrink_0())
            .child(gallery)
            .child(global)
            .child(div().flex_1())
            .child(widgets::dot(
                if config_ok {
                    theme::success()
                } else {
                    theme::danger()
                },
                6.,
            ))
            .child(div().h(px(4.)).flex_shrink_0())
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
        let content = match self.page {
            Page::Gallery => self.render_gallery(window, cx),
            Page::Global => self.render_global(window, cx),
        };
        let title_bar = TitleBar::new()
            .bg(theme::sidebar_bg())
            .border_color(theme::line())
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(widgets::dot(theme::accent(), 5.))
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme::muted_ink())
                            .child("iidxbg 配置"),
                    ),
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
                    .child(self.render_sidebar(window, cx))
                    .child(v_flex().flex_1().min_w_0().h_full().child(content)),
            )
            .when_some(overlay, |this, overlay| this.child(overlay))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GlobalTextField {
    LocalDirectory,
    SaveDirectory,
    Tags,
    Width,
    Height,
    PageMax,
    TimeoutSeconds,
    MaxImages,
    MaxDownloadMb,
    MaxSourcePixels,
    FrameCharaImage,
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

pub(crate) fn parse_number(
    value: &str,
    label: &str,
    min: u64,
    max: u64,
    cx: &mut Context<AppView>,
    view: &mut AppView,
) -> Option<u64> {
    let parsed = value.trim().parse::<u64>().ok();
    let Some(number) = parsed else {
        view.global_error = Some(format!("{label}必须是数字").into());
        cx.notify();
        return None;
    };
    if !(min..=max).contains(&number) {
        view.global_error = Some(format!("{label}必须在 {min}–{max} 之间").into());
        cx.notify();
        return None;
    }
    Some(number)
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
                    .rounded(px(16.))
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
                            .text_sm()
                            .line_height(relative(1.55))
                            .text_color(theme::muted_ink())
                            .child(self.message.clone()),
                    )
                    .when_some(self.path.as_ref(), |this, path| {
                        this.child(widgets::muted_path(path.display().to_string()))
                    })
                    .child(
                        div()
                            .text_xs()
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

pub(crate) fn surface_override_source(config: &Config, surface: Surface) -> Option<Source> {
    match surface {
        Surface::Results => config.sources.results.source,
        Surface::SongSelect => config.sources.song_select.source,
        Surface::SuddenPlus => config.sources.sudden_plus.source,
        Surface::Lift => config.sources.lift.source,
        Surface::TargetGraph => config.sources.target_graph.source,
        Surface::PremiumArea => config.sources.premium_area.source,
        Surface::Concentration => config.sources.concentration.source,
    }
}

pub(crate) fn surface_override_fit(config: &Config, surface: Surface) -> Option<Fit> {
    match surface {
        Surface::Results => config.sources.results.fit,
        Surface::SongSelect => config.sources.song_select.fit,
        Surface::SuddenPlus => config.sources.sudden_plus.fit,
        Surface::Lift => config.sources.lift.fit,
        Surface::TargetGraph => config.sources.target_graph.fit,
        Surface::PremiumArea => config.sources.premium_area.fit,
        Surface::Concentration => config.sources.concentration.fit,
    }
}

pub(crate) fn surface_override(
    config: &Config,
    surface: Surface,
) -> &iidxbg::config::SourceOverride {
    match surface {
        Surface::Results => &config.sources.results,
        Surface::SongSelect => &config.sources.song_select,
        Surface::SuddenPlus => &config.sources.sudden_plus,
        Surface::Lift => &config.sources.lift,
        Surface::TargetGraph => &config.sources.target_graph,
        Surface::PremiumArea => &config.sources.premium_area,
        Surface::Concentration => &config.sources.concentration,
    }
}
