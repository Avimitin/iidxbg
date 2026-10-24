use crate::{app::AppView, editor::SurfaceEditor, theme, widgets};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, Context, Entity, IntoElement, ParentElement, PathPromptOptions, SharedString,
    Styled, Subscription, Window, px,
};
use gpui_kit::{
    component::{
        button::Button,
        h_flex,
        input::{Input, InputEvent, InputState},
        scroll::ScrollableElement,
        v_flex,
    },
    *,
};
use iidxbg::config::{Config, Surface};
use serde::Deserialize;

#[derive(Clone, Deserialize)]
pub(crate) struct Entry {
    pub archive: String,
    pub texture: String,
    pub width: u32,
    pub height: u32,
    #[serde(skip)]
    search_key: String,
    #[serde(default)]
    preview: Option<std::path::PathBuf>,
}
pub(crate) struct Catalog {
    entries: Vec<Entry>,
    filtered: Vec<usize>,
    detail: String,
    search: Entity<InputState>,
    query: String,
    page: usize,
    error: Option<String>,
    pub preview: Option<std::sync::Arc<gpui::RenderImage>>,
    pub preview_error: Option<String>,
    pub preview_loading: bool,
    preview_generation: u64,
    scan_cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    scan_status: String,
    scan_report: Option<std::path::PathBuf>,
    _subscription: Subscription,
}
impl Catalog {
    pub(crate) fn invalidate_preview(&mut self) {
        self.preview = None;
        self.preview_error = None;
        self.preview_loading = false;
        self.preview_generation = self.preview_generation.wrapping_add(1);
    }
    fn filter(&mut self) {
        let words: Vec<_> = self.query.split_whitespace().collect();
        self.filtered = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| words.iter().all(|word| e.search_key.contains(word)))
            .map(|(i, _)| i)
            .collect();
    }
    pub fn new(window: &mut Window, cx: &mut Context<AppView>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("搜索素材名或归档路径"));
        let subscription = cx.subscribe(&search, |view, input, event, cx| {
            if matches!(event, InputEvent::Change) {
                view.catalog.query = input.read(cx).value().to_lowercase();
                view.catalog.page = 0;
                view.catalog.filter();
                cx.notify();
            }
        });
        Self {
            entries: Vec::new(),
            filtered: Vec::new(),
            detail: String::new(),
            search,
            query: String::new(),
            page: 0,
            error: None,
            preview: None,
            preview_error: None,
            preview_loading: false,
            preview_generation: 0,
            scan_cancel: None,
            scan_status: String::new(),
            scan_report: None,
            _subscription: subscription,
        }
    }
}
impl Drop for Catalog {
    fn drop(&mut self) {
        if let Some(cancel) = &self.scan_cancel {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }
}
impl AppView {
    fn scan_artwork(&mut self, cx: &mut Context<Self>) {
        if let Some(cancel) = &self.catalog.scan_cancel {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            self.catalog.scan_status = "正在取消扫描…".into();
            cx.notify();
            return;
        }
        if let Some(root) = crate::scanner::detect(self.store.path()) {
            self.start_artwork_scan(root, cx);
            return;
        }
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("选择 IIDX 安装目录（包含 data/graphic）".into()),
        });
        cx.spawn(async move |view, cx| {
            if let Ok(Ok(Some(paths))) = prompt.await
                && let Some(path) = paths.into_iter().next()
            {
                let _ = view.update(cx, |view, cx| view.start_artwork_scan(path, cx));
            }
        })
        .detach();
    }
    fn start_artwork_scan(&mut self, root: std::path::PathBuf, cx: &mut Context<Self>) {
        if self.catalog.scan_cancel.is_some() {
            return;
        }
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.catalog.scan_cancel = Some(cancel.clone());
        self.catalog.scan_status = format!("准备扫描 {}", root.display());
        self.catalog.scan_report = None;
        self.catalog.error = None;
        let (tx, rx) = async_channel::bounded(16);
        std::thread::spawn(move || {
            let result = crate::scanner::run(&root, &cancel, |status| {
                let _ = tx.send_blocking(crate::scanner::Event::Progress(status));
            });
            let _ = tx.send_blocking(crate::scanner::Event::Finished(result));
        });
        cx.spawn(async move |view, cx| {
            while let Ok(event) = rx.recv().await {
                let finished = matches!(&event, crate::scanner::Event::Finished(_));
                if view
                    .update(cx, |view, cx| {
                        match event {
                            crate::scanner::Event::Progress(status) => {
                                view.catalog.scan_status = status
                            }
                            crate::scanner::Event::Finished(result) => {
                                view.catalog.scan_cancel = None;
                                match result {
                                    Ok(path) => {
                                        view.catalog.scan_report =
                                            Some(path.with_file_name("scan-report.json"));
                                        view.load_catalog(path, cx);
                                    }
                                    Err(e) => {
                                        view.catalog.error = Some(e);
                                        view.catalog.scan_status.clear();
                                    }
                                }
                            }
                        }
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
                if finished {
                    break;
                }
            }
        })
        .detach();
        cx.notify();
    }
    pub(crate) fn editor_config(&self) -> Config {
        if let Some(index) = self.artwork_editor {
            Config::parse(&self.store.artwork_document(index).to_string())
                .unwrap_or_else(|_| self.store.config().clone())
        } else {
            self.store.config().clone()
        }
    }
    fn open_artwork(&mut self, entry: Entry, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self.store.add_artwork(&entry.archive, &entry.texture) {
            self.store.edit(|doc| {
                if let Some(table) = doc["artwork"]
                    .as_array_of_tables_mut()
                    .and_then(|a| a.get_mut(index))
                {
                    let mut size = toml_edit::Array::new();
                    size.push(i64::from(entry.width));
                    size.push(i64::from(entry.height));
                    table.insert("native_size", toml_edit::value(size));
                }
            });
            self.artwork_editor = Some(index);
            self.catalog.invalidate_preview();
            self.editor = Some(SurfaceEditor::new(
                Surface::Transition,
                &self.editor_config(),
                window,
                cx,
            ));
        }
        cx.notify();
    }
    pub(crate) fn refresh_artwork_preview(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self.artwork_editor else {
            return;
        };
        if self.catalog.preview_loading {
            return;
        }
        self.catalog.preview_generation = self.catalog.preview_generation.wrapping_add(1);
        let generation = self.catalog.preview_generation;
        self.catalog.preview_loading = true;
        self.catalog.preview = None;
        self.catalog.preview_error = None;
        let path = self.store.path().to_path_buf();
        let (tx, rx) = async_channel::bounded(1);
        std::thread::spawn(move || {
            let _ = tx.send_blocking(crate::preview::prepare_artwork(&path, index));
        });
        cx.spawn(async move |view, cx| {
            let Ok(result) = rx.recv().await else {
                return;
            };
            let _ = view.update(cx, |view, cx| {
                if view.artwork_editor != Some(index)
                    || view.catalog.preview_generation != generation
                {
                    return;
                }
                view.catalog.preview_loading = false;
                match result {
                    Ok(pixels) => view.catalog.preview = Some(crate::preview::render_image(pixels)),
                    Err(error) => view.catalog.preview_error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn import_catalog(&mut self, cx: &mut Context<Self>) {
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("选择素材目录 JSON".into()),
        });
        cx.spawn(async move |view, cx| {
            let Ok(Ok(Some(paths))) = prompt.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let _ = view.update(cx, |view, cx| view.load_catalog(path, cx));
        })
        .detach();
    }
    pub(crate) fn restore_catalog(&mut self, cx: &mut Context<Self>) {
        let saved = self.store.path().with_extension("catalog-path");
        if let Ok(path) = std::fs::read_to_string(saved) {
            self.load_catalog(path.into(), cx);
        }
    }
    fn load_catalog(&mut self, path: std::path::PathBuf, cx: &mut Context<Self>) {
        let saved = self.store.path().with_extension("catalog-path");
        cx.spawn(async move |view, cx| {
            let (tx, rx) = async_channel::bounded(1);
            std::thread::spawn(move || {
                let result = (|| -> Result<Vec<Entry>, String> {
                    let metadata = std::fs::metadata(&path).map_err(|e| e.to_string())?;
                    if metadata.len() > 32 * 1024 * 1024 {
                        return Err("目录超过 32 MiB".into());
                    }
                    let mut entries: Vec<Entry> =
                        serde_json::from_slice(&std::fs::read(&path).map_err(|e| e.to_string())?)
                            .map_err(|e| e.to_string())?;
                    if entries.len() > 200_000
                        || entries.iter().any(|e| {
                            e.width == 0 || e.height == 0 || e.width > 16384 || e.height > 16384
                        })
                    {
                        return Err("素材尺寸或数量无效".into());
                    }
                    for entry in &mut entries {
                        if let Some(preview) = &mut entry.preview
                            && preview.is_relative()
                        {
                            *preview = path
                                .parent()
                                .unwrap_or(std::path::Path::new("."))
                                .join(&*preview);
                        }
                        entry.search_key = format!(
                            "{} {} {}",
                            entry.archive,
                            entry.texture,
                            crate::scanner::mapping(&entry.archive, &entry.texture)
                        )
                        .to_lowercase();
                    }
                    std::fs::write(saved, path.to_string_lossy().as_bytes())
                        .map_err(|e| e.to_string())?;
                    Ok(entries)
                })();
                let _ = tx.send_blocking(result);
            });
            if let Ok(result) = rx.recv().await {
                let _ = view.update(cx, |view, cx| {
                    match result {
                        Ok(entries) => {
                            view.catalog.entries = entries;
                            view.catalog.filter();
                            view.catalog.error = None;
                            view.catalog.page = 0;
                        }
                        Err(e) => view.catalog.error = Some(e),
                    };
                    cx.notify();
                });
            }
        })
        .detach();
    }
    pub(crate) fn render_artwork(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let total = if self.catalog.entries.is_empty() {
            self.store.config().artwork.len()
        } else {
            self.catalog.filtered.len()
        };
        let page = self.catalog.page.min(total.saturating_sub(1) / 40);
        self.catalog.page = page;
        let entries: Vec<Entry> = if self.catalog.entries.is_empty() {
            self.store
                .config()
                .artwork
                .iter()
                .skip(page * 40)
                .take(40)
                .map(|a| Entry {
                    archive: a.archive.clone(),
                    texture: a.texture.clone(),
                    width: a.native_size.unwrap_or([1920, 1080])[0],
                    height: a.native_size.unwrap_or([1920, 1080])[1],
                    search_key: String::new(),
                    preview: None,
                })
                .collect()
        } else {
            self.catalog
                .filtered
                .iter()
                .skip(page * 40)
                .take(40)
                .map(|i| self.catalog.entries[*i].clone())
                .collect()
        };
        let toolbar = h_flex()
            .p_4()
            .gap_4()
            .child(Input::new(&self.catalog.search).w(px(420.)))
            .child(
                Button::new("scan-artwork")
                    .label(if self.catalog.scan_cancel.is_some() {
                        "取消扫描"
                    } else {
                        "扫描游戏素材"
                    })
                    .on_click(cx.listener(|view, _, _, cx| view.scan_artwork(cx))),
            )
            .child(
                Button::new("import-artwork")
                    .label("导入素材目录")
                    .on_click(cx.listener(|view, _, _, cx| view.import_catalog(cx))),
            )
            .child(
                Button::new("art-prev")
                    .label("上一页")
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.catalog.page = view.catalog.page.saturating_sub(1);
                        cx.notify();
                    })),
            )
            .child(
                Button::new("art-next")
                    .label("下一页")
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.catalog.page += 1;
                        cx.notify();
                    })),
            )
            .child(format!(
                "{} 项 · {} / {}",
                total,
                page + 1,
                total.div_ceil(40).max(1)
            ));
        let width = (window.viewport_size().width.as_f32() - theme::SIDEBAR_WIDTH).max(240.);
        let mut placed = Vec::new();
        let mut modules = Vec::new();
        for (i, entry) in entries.into_iter().enumerate() {
            let ratio = entry.width as f32 / entry.height as f32;
            let w = (width * [0.37, 0.28, 0.43, 0.32][i % 4])
                .min(440.)
                .min(400. * ratio)
                .max(90.);
            let h = (w - 24.) / ratio + 24.;
            let region = crate::gallery::adjacent_region(&placed, width, w, h);
            placed.push(region);
            let configured = self
                .store
                .config()
                .artwork
                .iter()
                .find(|a| a.archive == entry.archive && a.texture == entry.texture);
            let enabled = configured.is_some_and(|a| a.enabled);
            let label = format!(
                "{}\n{}\n{} × {}",
                entry.texture, entry.archive, entry.width, entry.height
            );
            let detail = format!(
                "{} / {}",
                crate::scanner::mapping(&entry.archive, &entry.texture),
                label.replace('\n', " / ")
            );
            let show_label = entry.preview.is_none() && region.height > 90. && region.width > 170.;
            modules.push(
                div()
                    .id(SharedString::from(format!("art-{i}")))
                    .absolute()
                    .left(px(region.x))
                    .top(px(region.y))
                    .w(px(region.width))
                    .h(px(region.height))
                    .border_1()
                    .border_color(theme::line())
                    .overflow_hidden()
                    .cursor_pointer()
                    .child(
                        div()
                            .absolute()
                            .inset(px(12.))
                            .border_1()
                            .border_color(if enabled {
                                theme::accent()
                            } else {
                                theme::muted_ink()
                            })
                            .flex()
                            .items_center()
                            .justify_center()
                            .when_some(entry.preview.clone(), |v, path| {
                                v.child(
                                    gpui::img(path)
                                        .size_full()
                                        .object_fit(gpui::ObjectFit::Contain),
                                )
                            })
                            .when(show_label, |v| {
                                v.child(div().p_2().text_size(px(14.)).child(label))
                            }),
                    )
                    .on_hover(cx.listener(move |view, entered, _, cx| {
                        if *entered {
                            view.catalog.detail = detail.clone();
                            cx.notify();
                        }
                    }))
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.open_artwork(entry.clone(), window, cx)
                    }))
                    .into_any_element(),
            );
        }
        let height = placed.iter().map(|r| r.y + r.height).fold(0_f32, f32::max);
        let cache = match crate::scanner::cache_root() {
            Ok(path) => v_flex()
                .px_4()
                .py_2()
                .gap_2()
                .child(
                    h_flex()
                        .gap_4()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .child(format!("素材缓存目录：{}", path.display())),
                        )
                        .child(
                            Button::new("open-artwork-cache")
                                .label("打开缓存目录")
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    match std::fs::create_dir_all(&path) {
                                        Ok(()) => cx.open_with_system(&path),
                                        Err(error) => {
                                            view.catalog.error =
                                                Some(format!("无法打开缓存目录：{error}"));
                                            cx.notify();
                                        }
                                    }
                                })),
                        ),
                )
                .child(div().text_color(theme::muted_ink()).child(
                    "可在文件夹中清理素材缓存；清理后点击「扫描游戏素材」重新生成目录和预览。",
                ))
                .into_any_element(),
            Err(error) => {
                widgets::error_banner(format!("素材缓存目录不可用：{error}")).into_any_element()
            }
        };
        v_flex().size_full().child(toolbar)
            .child(cache)
            .when(!self.catalog.scan_status.is_empty(), |v| v.child(h_flex().px_4().py_2().gap_4().child(self.catalog.scan_status.clone())
                .when_some(self.catalog.scan_report.clone(), |v, path| v.child(Button::new("scan-report").label("查看报告").on_click(move |_, _, cx| cx.reveal_path(&path))))))
            .child(div().px_4().h(px(32.)).overflow_hidden().child(self.catalog.detail.clone())).when_some(self.catalog.error.clone(),|view,e|view.child(widgets::error_banner(e))).when(total==0,|view|view.child(div().p_6().child("点击「扫描游戏素材」读取本机游戏，也可导入已有目录。扫描仅在点击后进行，不修改游戏文件。每项支持独立图片、视频、文件夹、API 或组件绑定。")))
            .child(div().id("artwork-scroll").flex_1().min_h_0().overflow_y_scrollbar().child(div().relative().w_full().h(px(height)).children(modules))).into_any_element()
    }
}
