use super::*;
use gpui::{Div, KeyDownEvent};
use iidxbg::config::CharacterSource;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum EditorPage {
    Media,
    Display,
    Character,
    Qpro,
    Model,
    Reactions,
    Advanced,
}
impl EditorPage {
    const ALL: [Self; 7] = [
        Self::Media,
        Self::Display,
        Self::Character,
        Self::Qpro,
        Self::Model,
        Self::Reactions,
        Self::Advanced,
    ];
    fn key(self) -> &'static str {
        match self {
            Self::Media => "media",
            Self::Display => "display",
            Self::Character => "character",
            Self::Qpro => "qpro",
            Self::Model => "model",
            Self::Reactions => "reactions",
            Self::Advanced => "advanced",
        }
    }
    fn title(self) -> &'static str {
        match self {
            Self::Media => "媒体来源",
            Self::Display => "画面与共享",
            Self::Character => "大角色",
            Self::Qpro => "Qpro 小角色",
            Self::Model => "MMD 模型与舞蹈",
            Self::Reactions => "互动动作",
            Self::Advanced => "高级设置",
        }
    }
    fn description(self) -> &'static str {
        match self {
            Self::Media => "选择图片、视频或在线来源，只显示当前来源需要的选项。",
            Self::Display => "调整图片的显示方式，或与另一个组件共用媒体。",
            Self::Character => "独立选择大角色：原版、图片或 PMX。不会改变 Qpro。",
            Self::Qpro => "保留原版 Qpro，或用小尺寸的 PMX 替换。不会改变大角色。",
            Self::Model => "选择 PMX 与待机舞蹈。使用 MMD 的角色共享这里的模型和动作。",
            Self::Reactions => "为搓盘和上次成绩分配不同动作，结束后回到待机舞蹈。",
            Self::Advanced => "按需调整下载、性能、骨骼与表情；通常可以保留当前值。",
        }
    }
    fn available(self, surface: Surface) -> bool {
        surface == Surface::SongSelect
            || matches!(self, Self::Media | Self::Display | Self::Advanced)
    }
}

fn note(text: impl Into<SharedString>) -> AnyElement {
    div()
        .text_sm()
        .text_color(theme::muted_ink())
        .child(text.into())
        .into_any_element()
}
fn group(title: &str) -> Div {
    v_flex()
        .w_full()
        .gap_4()
        .pb_6()
        .border_b_1()
        .border_color(theme::line_soft())
        .child(div().text_base().font_semibold().child(title.to_owned()))
}
fn jump(
    id: &'static str,
    label: &'static str,
    page: EditorPage,
    cx: &mut Context<AppView>,
) -> Button {
    Button::new(id)
        .outline()
        .label(label)
        .on_click(cx.listener(move |view, _, window, cx| {
            view.editor_page(page, window, cx);
        }))
}
fn toggle(
    id: &'static str,
    label: &str,
    detail: &str,
    checked: bool,
    path: Vec<&'static str>,
    cx: &mut Context<AppView>,
) -> AnyElement {
    h_flex()
        .gap_4()
        .items_center()
        .justify_between()
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_1()
                .child(label.to_owned())
                .when(!detail.is_empty(), |d| d.child(note(detail.to_owned()))),
        )
        .child(Switch::new(id).checked(checked).on_change(cx.listener(
            move |view, checked, _, cx| {
                view.commit_surface(0, cx, |doc| store::set_bool(doc, &path, *checked));
            },
        )))
        .into_any_element()
}
fn model_link(config: &Config, id: &'static str, cx: &mut Context<AppView>) -> AnyElement {
    let label = config
        .mmd
        .model_path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "尚未选择 PMX 模型".to_owned());
    v_flex()
        .gap_3()
        .child(note(format!("当前模型：{label}")))
        .child(jump(id, "选择模型与待机舞蹈 →", EditorPage::Model, cx))
        .into_any_element()
}

fn media(
    view: &AppView,
    editor: &SurfaceEditor,
    config: &Config,
    cx: &mut Context<AppView>,
) -> AnyElement {
    let surface = editor.surface;
    let c = config.for_surface(surface);
    let root = config.binding_root(surface).unwrap_or(surface);
    if root != surface {
        return group("正在共享媒体")
            .child(note(format!(
                "当前使用「{}」的来源与当前媒体。其他设置仍然独立。",
                theme::surface_title(root)
            )))
            .child(jump(
                "media-sharing",
                "更改共享关系 →",
                EditorPage::Display,
                cx,
            ))
            .child(
                Button::new("edit-binding")
                    .outline()
                    .label("编辑来源组件")
                    .on_click(
                        cx.listener(move |view, _, window, cx| view.open_editor(root, window, cx)),
                    ),
            )
            .into_any_element();
    }
    let mut choices = h_flex().gap_2().flex_wrap();
    for (source, label) in [
        (Source::File, "图片 / 视频"),
        (Source::Local, "随机文件夹"),
        (Source::Remote, "图片网站"),
        (Source::Mixed, "文件夹 + 网站"),
        (Source::Bilibili, "Bilibili"),
    ] {
        choices = choices.child(
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
    let mut content = v_flex().gap_6().child(group("来源类型").child(choices));
    if c.source == Source::File {
        content = content.child(
            group("本地媒体")
                .child(field_row(
                    editor,
                    SurfaceTextField::MediaPath,
                    "图片或视频文件",
                    true,
                    cx,
                ))
                .child(note("图片支持 PNG / JPEG。视频静音循环，预览显示首帧。")),
        );
    }
    if matches!(c.source, Source::Local | Source::Mixed) {
        content = content.child(
            group("随机文件夹")
                .child(field_row(
                    editor,
                    SurfaceTextField::LocalDirectory,
                    "媒体所在文件夹",
                    true,
                    cx,
                ))
                .child(toggle(
                    "recursive",
                    "包含子文件夹",
                    "从这个文件夹及其子文件夹中选取媒体。",
                    c.recursive,
                    vec!["sources", surface.name(), "recursive"],
                    cx,
                )),
        );
    }
    if matches!(c.source, Source::Remote | Source::Mixed) {
        let mut domains = h_flex().gap_2().flex_wrap();
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
                    if view.commit_surface(0, cx, |doc| {
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
                    }) {
                        view.rebuild_editor(window, cx);
                    }
                })),
            );
        }
        content = content.child(
            group("图片网站")
                .child(domains)
                .when(!c.moebooru.api_url.is_empty(), |g| {
                    g.child(note("正在使用自定义 API；选择上面的网站可切换回内置来源。"))
                })
                .child(field_row(
                    editor,
                    SurfaceTextField::Tags,
                    "搜索标签",
                    false,
                    cx,
                ))
                .child(jump(
                    "api-options",
                    "连接与下载选项 →",
                    EditorPage::Advanced,
                    cx,
                )),
        );
    }
    if c.source == Source::Bilibili {
        use iidxbg::bilibili::Mode;
        let mut modes = h_flex().gap_2().flex_wrap();
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
        let mut bili = group("Bilibili 内容").child(modes);
        let field = match c.bilibili.selection.mode {
            Mode::Video => Some((SurfaceTextField::BiliVideo, "视频链接 / BV")),
            Mode::Playlist => Some((SurfaceTextField::BiliPlaylist, "合集 / 系列链接")),
            Mode::Favourites => Some((SurfaceTextField::BiliFavourites, "收藏夹链接 / ID")),
            Mode::Live => Some((SurfaceTextField::BiliRoom, "直播间链接 / ID")),
            Mode::Recommendations => None,
        };
        if let Some((field, label)) = field {
            bili = bili.child(field_row(editor, field, label, false, cx));
        }
        if c.bilibili.selection.mode == Mode::Favourites {
            let mut folders = h_flex().gap_2().flex_wrap();
            for folder in &view.bili.folders {
                let id = folder.id.to_string();
                folders = folders.child(
                    Button::new(SharedString::from(format!("fav-{}", folder.id)))
                        .outline()
                        .label(folder.title.clone())
                        .on_click(cx.listener(move |view, _, window, cx| {
                            if view.commit_surface(0, cx, |doc| {
                                store::set_string(
                                    doc,
                                    &["sources", surface.name(), "bilibili", "favourites"],
                                    &id,
                                )
                            }) && let Some(editor) = &view.editor
                            {
                                editor
                                    .input_for(SurfaceTextField::BiliFavourites)
                                    .update(cx, |s, cx| s.set_value(id.clone(), window, cx));
                            }
                        })),
                );
            }
            bili = bili.child(folders);
        }
        bili = bili.child(note(match c.bilibili.selection.mode {
            Mode::Live => "直播断开后自动重连，未开播时保留原画面。无音频。",
            Mode::Recommendations => "使用已登录账号的推荐流。请先在主侧栏的 Bilibili 页面登录。",
            _ => "先缓存视频再播放。登录和刷新收藏夹请前往主侧栏的 Bilibili 页面。",
        }));
        content = content.child(bili);
    }
    content.into_any_element()
}

fn display(
    view: &AppView,
    editor: &SurfaceEditor,
    config: &Config,
    cx: &mut Context<AppView>,
) -> AnyElement {
    let surface = editor.surface;
    let root = config.binding_root(surface).unwrap_or(surface);
    let mut bindings = h_flex().gap_2().flex_wrap().child(
        widgets::segment_button("bind-independent", "独立设置", root == surface).on_click(
            cx.listener(move |view, _, window, cx| view.bind_surface(surface, None, window, cx)),
        ),
    );
    for target in Surface::ALL.into_iter().filter(|s| surface.can_bind(*s)) {
        bindings = bindings.child(
            widgets::segment_button(
                SharedString::from(format!("bind-{}", target.name())),
                theme::surface_title(target),
                config.source_override(surface).bind == Some(target)
                    || (root != surface
                        && config.source_override(surface).bind.is_none()
                        && root == target),
            )
            .on_click(cx.listener(move |view, _, window, cx| {
                view.bind_surface(surface, Some(target), window, cx)
            })),
        );
    }
    let mut content = v_flex().gap_6().child(
        group("共享媒体")
            .child(note(
                "绑定后共享来源和当前媒体；开关保持独立。解除绑定会恢复自己的设置。",
            ))
            .child(bindings),
    );
    if let Some(index) = view.artwork_editor {
        let art = &view.store.config().artwork[index];
        if !art.texture.starts_with("@shape/") && art.archive != "@movie" {
            content = content.child(
                group("透明轮廓").child(
                    h_flex()
                        .gap_4()
                        .justify_between()
                        .child("保留原素材透明轮廓")
                        .child(
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
                ),
            );
        }
    } else if root == surface {
        let mut fit = h_flex().gap_2().flex_wrap();
        for (value, label) in [
            (Fit::Cover, "裁切铺满"),
            (Fit::Contain, "完整显示 + 模糊边缘"),
        ] {
            fit = fit.child(
                widgets::segment_button(
                    SharedString::from(format!("fit-{}", store::fit_key(value))),
                    label,
                    config.for_surface(surface).image.fit == value,
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
        content = content.child(
            group("图片适配")
                .child(fit)
                .child(note("此选项只影响图片。视频会拉伸到组件画面。")),
        );
    }
    content.into_any_element()
}

fn character(editor: &SurfaceEditor, config: &Config, cx: &mut Context<AppView>) -> AnyElement {
    let mut sources = h_flex().gap_2().flex_wrap();
    for (source, id, label) in [
        (CharacterSource::Stock, "stock", "原版角色"),
        (CharacterSource::Image, "image", "自选图片"),
        (CharacterSource::Mmd, "mmd", "PMX 模型"),
    ] {
        sources = sources.child(
            widgets::segment_button(
                SharedString::from(format!("character-{id}")),
                label,
                config.character_source() == source,
            )
            .on_click(cx.listener(move |view, _, _, cx| {
                view.commit_surface(0, cx, |doc| store::set_character_source(doc, source));
            })),
        );
    }
    let mut content = v_flex().gap_6().child(
        group("显示与来源")
            .child(toggle(
                "frame-chara",
                "显示大角色",
                "关闭后，Qpro 仍按自己的设置显示。",
                config.selection.frame_chara,
                vec!["selection", "frame_chara"],
                cx,
            ))
            .child(sources),
    );
    match config.character_source() {
        CharacterSource::Image => {
            content = content.child(group("角色图片")
                .child(field_row(editor, SurfaceTextField::FrameCharaImage, "透明背景图片", true, cx))
                .child(note("自动去除透明空白，并适配原生角色区域。切换到原版或 PMX 后，图片路径仍会保留。")));
        }
        CharacterSource::Mmd => {
            content = content.child(model_link(config, "character-model", cx));
            let mut presets = h_flex().gap_2().flex_wrap();
            for (id, label, size, x, y, safe) in [
                ("original", "原始大小与位置", 82., 50., 50., false),
                ("top-left", "左上角 · 避开顶栏", 42., 20., 32., true),
            ] {
                presets = presets.child(
                    widgets::segment_button(SharedString::from(format!("mmd-{id}")), label, false)
                        .on_click(cx.listener(move |view, _, window, cx| {
                            if view.commit_surface(0, cx, |doc| {
                                store::set_f64(doc, &["mmd", "display_scale"], size);
                                store::set_f64(doc, &["mmd", "position_x"], x);
                                store::set_f64(doc, &["mmd", "position_y"], y);
                                store::set_bool(doc, &["mmd", "keep_hud_clear"], safe);
                            }) {
                                view.rebuild_editor(window, cx);
                            }
                        })),
                );
            }
            content = content.child(
                group("大小与位置")
                    .child(presets)
                    .child(
                        h_flex()
                            .gap_4()
                            .items_start()
                            .child(field_row(
                                editor,
                                SurfaceTextField::MmdDisplayScale,
                                "大小（10–100%）",
                                false,
                                cx,
                            ))
                            .child(field_row(
                                editor,
                                SurfaceTextField::MmdPositionX,
                                "水平中心（0–100）",
                                false,
                                cx,
                            ))
                            .child(field_row(
                                editor,
                                SurfaceTextField::MmdPositionY,
                                "垂直中心（0–100）",
                                false,
                                cx,
                            )),
                    )
                    .child(toggle(
                        "mmd-hud-clear",
                        "避开顶栏与底栏",
                        "开启时自动限制大小与位置；歌曲详情仍在前景。",
                        config.mmd.keep_hud_clear,
                        vec!["mmd", "keep_hud_clear"],
                        cx,
                    )),
            );
        }
        _ => {
            content = content.child(note(
                "使用游戏原有角色。Qpro 可同时使用 PMX；已选图片和模型路径不会被清除。",
            ));
        }
    }
    content.into_any_element()
}

fn qpro(editor: &SurfaceEditor, config: &Config, cx: &mut Context<AppView>) -> AnyElement {
    let mut choices = h_flex().gap_2();
    for (enabled, id, label) in [(false, "stock", "原版 Qpro"), (true, "mmd", "PMX 模型")] {
        choices = choices.child(
            widgets::segment_button(
                SharedString::from(format!("qpro-{id}")),
                label,
                config.mmd.replace_qpro == enabled,
            )
            .on_click(cx.listener(move |view, _, _, cx| {
                let current = view.editor_config().character_source();
                view.commit_surface(0, cx, |doc| store::set_qpro(doc, enabled, current));
            })),
        );
    }
    let mut content = v_flex()
        .gap_6()
        .child(group("Qpro 来源").child(choices).child(note(
            "仅替换普通选曲画面的 Qpro；跟随原 Qpro 的位置。大角色可以使用原版、图片或 PMX。",
        )));
    if config.mmd.replace_qpro {
        content = content.child(model_link(config, "qpro-model", cx)).child(
            group("小角色大小")
                .child(field_row(
                    editor,
                    SurfaceTextField::MmdQproHeight,
                    "显示范围（100–800）",
                    false,
                    cx,
                ))
                .child(note(
                    "默认 360。大小独立于大角色；模型周围的透明空白也会影响可见大小。",
                )),
        );
    }
    content.into_any_element()
}

fn model(editor: &SurfaceEditor, config: &Config, cx: &mut Context<AppView>) -> AnyElement {
    let users = match (
        config.uses_mmd_character(),
        config.mmd.enabled && config.mmd.replace_qpro,
    ) {
        (true, true) => "大角色与 Qpro 都在使用此模型。",
        (true, false) => "大角色在使用此模型；Qpro 保持原版。",
        (false, true) => "Qpro 在使用此模型；大角色使用独立来源。",
        (false, false) => "可以先配置模型，再到「大角色」或「Qpro 小角色」选择 PMX。",
    };
    let mut tempo = group("跟随歌曲速度").child(toggle(
        "mmd-bpm-sync",
        "待机舞蹈跟随所选歌曲 BPM",
        "只在选曲待机时调速。搓盘和成绩动作保持原速。",
        config.mmd.bpm_sync,
        vec!["mmd", "bpm_sync"],
        cx,
    ));
    if config.mmd.bpm_sync {
        tempo = tempo.child(field_row(editor, SurfaceTextField::MmdMotionBpm, "动作原曲 BPM", false, cx))
            .child(note("VMD 不含 BPM，需填写动作原曲速度。变速曲取最高 BPM；这会匹配速度，不会对齐每个节拍。"));
    }
    v_flex().gap_6().child(note(users))
        .child(group("模型与待机动作")
            .child(field_row(editor, SurfaceTextField::MmdModel, "PMX 模型", true, cx))
            .child(field_row(editor, SurfaceTextField::MmdMotion, "待机循环 VMD（可选）", true, cx))
            .child(note("选择已解压的 PMX，并保留旁边的贴图文件夹。VMD 留空时显示默认姿势；进入演奏后隐藏角色。")))
        .child(tempo).into_any_element()
}

fn reactions(editor: &SurfaceEditor, config: &Config, cx: &mut Context<AppView>) -> AnyElement {
    let mut content = v_flex().gap_6().child(group("响应选曲操作").child(toggle(
        "mmd-reactions",
        "启用互动动作",
        "仅影响使用 MMD 的角色。指定动作播放一次后回到待机舞蹈。",
        config.mmd.reactions,
        vec!["mmd", "reactions"],
        cx,
    )));
    if config.mmd.reactions {
        content = content.child(group("搓盘方向")
            .child(field_row(editor, SurfaceTextField::MmdScrollUp, "向上搓盘 / 抬头 VMD", true, cx))
            .child(field_row(editor, SurfaceTextField::MmdScrollDown, "向下搓盘 / 低头 VMD", true, cx))
            .child(toggle("mmd-scratch-reverse", "反转转盘动作方向", "实际方向与预期相反时开启。", config.mmd.scratch_reverse,
                vec!["mmd", "scratch_reverse"], cx)))
            .child(group("上次成绩")
                .child(field_row(editor, SurfaceTextField::MmdHappyMotion, "A / AA / AAA 后 VMD", true, cx))
                .child(field_row(editor, SurfaceTextField::MmdOtherResult, "B–F 后 VMD", true, cx))
                .child(note("返回选曲时触发，优先于搓盘动作。动作路径留空时使用待机动作与已配置的自动反应。")));
    } else {
        content = content.child(note("已暂停互动。动作路径仍然保留，重新开启后继续使用。"));
    }
    content.into_any_element()
}

fn advanced(editor: &SurfaceEditor, config: &Config, cx: &mut Context<AppView>) -> AnyElement {
    let surface = editor.surface;
    let c = config.for_surface(surface);
    let network = config.binding_root(surface).unwrap_or(surface) == surface
        && matches!(c.source, Source::Remote | Source::Mixed);
    let mut content = v_flex().gap_6();
    if network {
        content = content
            .child(
                group("自定义网站连接")
                    .child(field_row(
                        editor,
                        SurfaceTextField::ApiUrl,
                        "自定义 post.json API（留空使用内置网站）",
                        false,
                        cx,
                    ))
                    .child(field_row(
                        editor,
                        SurfaceTextField::MediaHosts,
                        "允许的图片 CDN 域名（逗号分隔）",
                        false,
                        cx,
                    )),
            )
            .child(
                group("下载与缓存")
                    .child(field_row(
                        editor,
                        SurfaceTextField::SaveDirectory,
                        "下载目录",
                        true,
                        cx,
                    ))
                    .child(
                        h_flex()
                            .gap_4()
                            .items_start()
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
                                "保留数量 · 0 不限",
                                false,
                                cx,
                            )),
                    ),
            );
    }
    if surface == Surface::SongSelect {
        let mut rate = h_flex().gap_2();
        for fps in [30u64, 60] {
            rate = rate.child(
                widgets::segment_button(
                    SharedString::from(format!("mmd-fps-{fps}")),
                    if fps == 30 { "30 fps" } else { "60 fps" },
                    u64::from(config.mmd.fps) == fps,
                )
                .on_click(cx.listener(move |view, _, _, cx| {
                    view.commit_surface(0, cx, |doc| store::set_u64(doc, &["mmd", "fps"], fps));
                })),
            );
        }
        content = content
            .child(group("MMD 性能").child(rate).child(note(
                "30 fps 通常已足够。当前不模拟刚体或布料物理，也不执行 MME 特效。",
            )))
            .child(
                group("自动转头与表情")
                    .child(note(
                        "名称须与模型一致，留空可禁用。自定义互动动作播放期间不会叠加自动转头。",
                    ))
                    .child(field_row(
                        editor,
                        SurfaceTextField::MmdLook,
                        "转头骨骼名",
                        false,
                        cx,
                    ))
                    .child(
                        h_flex()
                            .gap_4()
                            .items_start()
                            .child(field_row(
                                editor,
                                SurfaceTextField::MmdLookYaw,
                                "向右看角度（度）",
                                false,
                                cx,
                            ))
                            .child(field_row(
                                editor,
                                SurfaceTextField::MmdLookHold,
                                "停止滚动后保持（秒）",
                                false,
                                cx,
                            )),
                    )
                    .child(
                        h_flex()
                            .gap_4()
                            .items_start()
                            .child(field_row(
                                editor,
                                SurfaceTextField::MmdHappy,
                                "开心表情名",
                                false,
                                cx,
                            ))
                            .child(field_row(
                                editor,
                                SurfaceTextField::MmdBlink,
                                "眨眼表情名",
                                false,
                                cx,
                            )),
                    ),
            );
    } else if !network {
        content = content.child(note(
            "当前来源没有需要调整的高级选项。图片网站的连接与下载选项会在选择该来源后显示。",
        ));
    }
    content.into_any_element()
}

// Search returns destinations rather than expanding the entire form again.
struct SearchEntry {
    page: EditorPage,
    label: &'static str,
    keywords: &'static str,
}
fn search_entries(surface: Surface) -> Vec<SearchEntry> {
    use EditorPage::*;
    let rows = [
        (
            Media,
            "媒体来源 / 图片与视频",
            "source media_path file 本地 固定 图片 视频",
        ),
        (
            Media,
            "随机文件夹与子文件夹",
            "local_directory recursive folder 文件夹",
        ),
        (
            Media,
            "图片网站与搜索标签",
            "tags domain konachan yandere moebooru API 网站",
        ),
        (
            Media,
            "Bilibili 视频、合集、收藏夹与直播",
            "bilibili video playlist favourites room live 推荐",
        ),
        (Display, "共享其他组件的媒体", "bind binding 绑定 共享 来源"),
        (
            Display,
            "图片适配与透明轮廓",
            "fit cover contain native_mask 裁切 模糊 完整 透明",
        ),
        (
            Character,
            "大角色显示与来源",
            "frame_chara frame_chara_source stock 原版 PMX 图片 隐藏",
        ),
        (
            Character,
            "大角色图片路径",
            "frame_chara_image image 图片 文件",
        ),
        (
            Character,
            "大角色大小与位置预设",
            "display_scale position_x position_y keep_hud_clear 大小 左上 顶栏 原始 水平 垂直 位置",
        ),
        (
            Qpro,
            "Qpro 来源与独立大小",
            "replace_qpro qpro_height PMX 原版 小角色 大小",
        ),
        (
            Model,
            "PMX 模型与待机循环 VMD",
            "model_path motion_path 模型 动作 路径 导入 舞蹈 待机",
        ),
        (
            Model,
            "舞蹈跟随歌曲 BPM",
            "bpm_sync motion_bpm tempo 原曲 速度 节奏",
        ),
        (
            Reactions,
            "搓盘方向动作",
            "scroll_up_motion_path scroll_down_motion_path scratch_reverse reactions 抬头 低头 向上 向下 反转 转盘 VMD",
        ),
        (
            Reactions,
            "成绩触发动作",
            "happy_motion_path other_result_motion_path result A AA AAA B F 成绩 开心 VMD",
        ),
        (
            Advanced,
            "自定义 API 与图片 CDN",
            "api_url media_hosts post.json 域名 连接 网站",
        ),
        (
            Advanced,
            "下载目录与缓存限制",
            "save_directory page_max timeout_seconds max_images 下载 保存 页数 超时 数量 缓存",
        ),
    ];
    let mut entries: Vec<_> = rows
        .into_iter()
        .filter(|(p, _, _)| p.available(surface))
        .map(|(page, label, keywords)| SearchEntry {
            page,
            label,
            keywords,
        })
        .collect();
    if surface == Surface::SongSelect {
        entries.extend([
            SearchEntry { page: Advanced, label: "MMD 帧率", keywords: "fps performance 30 60 性能 帧率" },
            SearchEntry { page: Advanced, label: "自动转头与表情", keywords: "look_bone look_yaw look_hold_seconds happy_morph blink_morph 骨骼 名称 角度 保持 开心 眨眼 表情" },
        ]);
    }
    entries
}
fn matches(entry: &SearchEntry, query: &str) -> bool {
    let haystack =
        format!("{} {} {}", entry.page.title(), entry.label, entry.keywords).to_lowercase();
    query
        .split_whitespace()
        .all(|word| haystack.contains(&word.to_lowercase()))
}

pub(super) fn render(
    view: &mut AppView,
    editor: &SurfaceEditor,
    window: &mut Window,
    cx: &mut Context<AppView>,
) -> AnyElement {
    let surface = editor.surface;
    let config = view.editor_config();
    let width = (window.viewport_size().width.as_f32() - 32.).clamp(1., 1120.);
    let height = (window.viewport_size().height.as_f32() - 40.).clamp(1., 900.);
    let compact = width < 800.;
    let searching = !editor.query.is_empty();
    let mut body = v_flex()
        .id(SharedString::from(format!(
            "settings-{}-{searching}",
            editor.page.key()
        )))
        .flex_1()
        .min_w_0()
        .min_h_0()
        .p_6()
        .gap_6()
        .overflow_y_scrollbar();
    if searching {
        let entries = search_entries(surface);
        let results: Vec<_> = entries
            .iter()
            .filter(|e| matches(e, &editor.query))
            .collect();
        body = body.child(
            div()
                .text_xl()
                .font_semibold()
                .child(format!("搜索结果 · {}", results.len())),
        );
        if results.is_empty() {
            body = body.child(note(
                "未找到匹配设置。试试「Qpro」「BPM」「图片」或「下载」。",
            ));
        }
        for (index, result) in results.into_iter().enumerate() {
            let page = result.page;
            body = body.child(
                Button::new(SharedString::from(format!("setting-result-{index}")))
                    .outline()
                    .large()
                    .w_full()
                    .justify_start()
                    .label(format!("{}  ›  {}", page.title(), result.label))
                    .on_click(
                        cx.listener(move |view, _, window, cx| view.editor_page(page, window, cx)),
                    ),
            );
        }
    } else {
        body = body.child(
            v_flex()
                .gap_2()
                .child(div().text_xl().font_semibold().child(editor.page.title()))
                .child(note(editor.page.description())),
        );
        body = body.child(match editor.page {
            EditorPage::Media => media(view, editor, &config, cx),
            EditorPage::Display => display(view, editor, &config, cx),
            EditorPage::Character => character(editor, &config, cx),
            EditorPage::Qpro => qpro(editor, &config, cx),
            EditorPage::Model => model(editor, &config, cx),
            EditorPage::Reactions => reactions(editor, &config, cx),
            EditorPage::Advanced => advanced(editor, &config, cx),
        });
        if editor.page == EditorPage::Media && view.artwork_editor.is_some() {
            if view.catalog.preview_loading {
                body = body.child(note("正在准备素材预览…"));
            }
            if let Some(error) = &view.catalog.preview_error {
                body = body.child(widgets::error_banner(error.clone()));
            }
            if let Some(image) = &view.catalog.preview {
                body = body.child(
                    gpui::img(image.clone())
                        .w_full()
                        .max_h(px(240.))
                        .object_fit(gpui::ObjectFit::Contain),
                );
            }
        }
    }
    let mut navigation = div()
        .flex()
        .gap_2()
        .p_3()
        .flex_shrink_0()
        .when(compact, |d| d.flex_row().flex_wrap().border_b_1())
        .when(!compact, |d| d.flex_col().w(px(192.)).border_r_1())
        .border_color(theme::line());
    for page in EditorPage::ALL.into_iter().filter(|p| p.available(surface)) {
        navigation = navigation.child(
            widgets::segment_button(
                SharedString::from(format!("page-{}", page.key())),
                page.title(),
                !searching && editor.page == page,
            )
            .when(!compact, |b| b.w_full().justify_start())
            .on_click(cx.listener(move |view, _, window, cx| view.editor_page(page, window, cx))),
        );
    }
    let error = editor
        .error
        .clone()
        .or_else(|| view.store.error().map(|e| e.to_owned().into()));
    let status = if let Some(error) = error {
        div()
            .text_sm()
            .text_color(theme::danger())
            .child(format!("未保存：{error}"))
            .into_any_element()
    } else {
        note("有效修改自动保存。下次进入组件时生效。")
    };
    let title = view
        .artwork_editor
        .map(|i| view.store.config().artwork[i].texture.clone())
        .unwrap_or_else(|| theme::surface_title(surface).to_owned());
    div()
        .id("surface-editor-overlay")
        .absolute()
        .inset_0()
        .bg(theme::overlay())
        .flex()
        .items_center()
        .justify_center()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|view, _, _, cx| view.close_editor(cx)),
        )
        .child(
            v_flex()
                .id("surface-editor")
                .track_focus(&editor.focus)
                .w(px(width))
                .h(px(height))
                .bg(theme::card_bg())
                .border_1()
                .border_color(theme::line())
                .overflow_hidden()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_key_down(cx.listener(|view, event: &KeyDownEvent, window, cx| {
                    if event.keystroke.key == "escape" {
                        if view.editor.as_ref().is_some_and(|e| !e.query.is_empty()) {
                            let page = view.editor.as_ref().unwrap().page;
                            view.editor_page(page, window, cx);
                        } else {
                            view.close_editor(cx);
                        }
                        cx.stop_propagation();
                    } else if event.keystroke.modifiers.control && event.keystroke.key == "f" {
                        if let Some(editor) = &view.editor {
                            editor.search.read(cx).focus_handle(cx).focus(window, cx);
                        }
                        cx.stop_propagation();
                    }
                }))
                .child(
                    h_flex()
                        .px_6()
                        .py_4()
                        .gap_4()
                        .items_center()
                        .flex_shrink_0()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_2xl()
                                .font_semibold()
                                .text_ellipsis()
                                .child(title),
                        )
                        .child(div().text_sm().text_color(theme::muted_ink()).child(
                            if surface == Surface::SongSelect {
                                "启用背景"
                            } else {
                                "启用组件"
                            },
                        ))
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
                .child(
                    div()
                        .px_6()
                        .pb_4()
                        .border_b_1()
                        .border_color(theme::line())
                        .flex_shrink_0()
                        .child(Input::new(&editor.search).large().cleanable(true).w_full()),
                )
                .child(
                    div()
                        .flex()
                        .flex_1()
                        .min_h_0()
                        .min_w_0()
                        .when(compact, |d| d.flex_col())
                        .when(!compact, |d| d.flex_row())
                        .child(navigation)
                        .child(body),
                )
                .child(
                    h_flex()
                        .px_6()
                        .py_3()
                        .gap_4()
                        .border_t_1()
                        .border_color(theme::line())
                        .flex_shrink_0()
                        .child(div().flex_1().min_w_0().child(status))
                        .child(
                            Button::new("refresh-preview")
                                .outline()
                                .label("刷新预览")
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.refresh_surface(surface, cx)
                                })),
                        ),
                ),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{EditorPage, matches, search_entries};
    use iidxbg::config::Surface;
    #[test]
    fn settings_search_finds_independent_character_and_qpro_controls() {
        let entries = search_entries(Surface::SongSelect);
        for (query, page) in [
            ("QPRO", EditorPage::Qpro),
            ("图片 角色", EditorPage::Character),
            ("motion_bpm", EditorPage::Model),
            ("向下", EditorPage::Reactions),
            ("CDN", EditorPage::Advanced),
        ] {
            assert!(
                entries.iter().any(|e| e.page == page && matches(e, query)),
                "{query}"
            );
        }
        let plain = search_entries(Surface::Lift);
        assert!(
            !plain
                .iter()
                .any(|e| matches(e, "Qpro") || matches(e, "PMX"))
        );
        assert!(plain.iter().any(|e| matches(e, "视频")));
    }
}
