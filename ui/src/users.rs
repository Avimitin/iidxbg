use crate::{
    app::{AppView, Page},
    store::{self, Scope},
    theme, widgets,
};
use gpui::{
    AnyElement, Entity, IntoElement, MouseButton, ParentElement, ScrollHandle, SharedString,
    Styled, Subscription, Window, div, prelude::FluentBuilder as _, px,
};
use gpui_kit::{
    assets::IconName,
    component::{
        Disableable, Icon, Sizable,
        button::ButtonVariants,
        h_flex,
        input::{Input, InputEvent, InputState},
        scroll::ScrollableElement,
        v_flex,
    },
    *,
};
use iidxbg::{config::Config, session::Status};
use std::time::Duration;

pub(crate) struct State {
    pub selected: Option<String>,
    pub menu_open: bool,
    name: Entity<InputState>,
    search: Entity<InputState>,
    scroll: ScrollHandle,
    rename: Entity<InputState>,
    card: Entity<InputState>,
    status: Option<Status>,
    deleting: Option<String>,
    _subscriptions: Vec<Subscription>,
}

impl State {
    pub fn new(selected: Option<String>, window: &mut Window, cx: &mut Context<AppView>) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("创建新用户"));
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("搜索用户名称或卡号"));
        let subscriptions = vec![
            cx.subscribe_in(&search, window, |view, _, event, window, cx| {
                if matches!(event, InputEvent::Change) {
                    view.users.scroll.set_offset(Default::default());
                }
                if matches!(event, InputEvent::PressEnter { .. }) {
                    let ids = matching_users(
                        view.store.document_config(),
                        view.users.selected.as_deref(),
                        view.users.search.read(cx).value().as_ref(),
                    );
                    if let Some(id) = ids.first() {
                        view.select_user(id.clone(), window, cx);
                    }
                }
                cx.notify();
            }),
            cx.subscribe_in(&name, window, |view, _, event, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    view.create_user(window, cx);
                }
                cx.notify();
            }),
        ];
        Self {
            selected,
            menu_open: false,
            name,
            search,
            scroll: ScrollHandle::default(),
            rename: cx.new(|cx| InputState::new(window, cx).placeholder("用户名称")),
            card: cx.new(|cx| InputState::new(window, cx).placeholder("16 位卡号，例如 E004…")),
            status: None,
            deleting: None,
            _subscriptions: subscriptions,
        }
    }
}

pub(crate) fn user_scope(selected: Option<&str>) -> Scope {
    selected
        .map(|id| Scope::User(id.into()))
        .unwrap_or(Scope::PreLogin)
}

pub(crate) fn page_scope(page: Page, selected: Option<&str>) -> Scope {
    if page == Page::Assets {
        Scope::Assets
    } else {
        user_scope(selected)
    }
}

pub(crate) fn resolve_user(config: &Config, selected: Option<&str>) -> Option<String> {
    selected
        .filter(|id| config.users.contains_key(*id))
        .map(str::to_owned)
        .or_else(|| matching_users(config, None, "").into_iter().next())
}

/// Current user first, then stable case-insensitive name order. All keywords
/// must match the name, stable ID or cards; searching never changes selection.
pub(crate) fn matching_users(config: &Config, selected: Option<&str>, query: &str) -> Vec<String> {
    let query = query.to_lowercase();
    let words: Vec<_> = query.split_whitespace().collect();
    let mut users: Vec<_> = config
        .users
        .iter()
        .filter(|(id, user)| {
            let key = format!("{} {} {}", user.name, id, user.cards.join(" ")).to_lowercase();
            words.iter().all(|word| key.contains(word))
        })
        .collect();
    users.sort_by_key(|(id, user)| {
        (
            Some(id.as_str()) != selected,
            user.name.to_lowercase(),
            id.as_str(),
        )
    });
    users.into_iter().map(|(id, _)| id.clone()).collect()
}

impl AppView {
    pub(crate) fn selected_user_label(&self) -> String {
        self.users
            .selected
            .as_ref()
            .and_then(|id| self.store.document_config().users.get(id))
            .map(|u| u.name.clone())
            .unwrap_or_else(|| "尚未选择用户".into())
    }
    pub(crate) fn scope_label(&self) -> String {
        match self.store.scope() {
            Scope::Assets => "游戏素材".into(),
            Scope::PreLogin => "登录前 · 所有用户共用".into(),
            Scope::User(_) => self.selected_user_label(),
            Scope::Legacy => "旧版配置".into(),
        }
    }
    pub(crate) fn select_scope(&mut self, scope: Scope, cx: &mut Context<Self>) {
        if self.store.scope() == &scope {
            return;
        }
        self.store.set_scope(scope);
        self.invalidate_user_previews(cx);
    }
    fn invalidate_user_previews(&mut self, cx: &mut Context<Self>) {
        self.editor = None;
        self.artwork_editor = None;
        self.catalog.invalidate_preview();
        for slot in &mut self.previews {
            let generation = slot.generation.wrapping_add(1);
            *slot = Default::default();
            slot.generation = generation;
        }
        self.schedule_preview_sync(cx, 0);
        cx.notify();
    }
    fn sync_user_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.users.deleting = None;
        let name = self
            .users
            .selected
            .as_ref()
            .and_then(|id| self.store.document_config().users.get(id))
            .map(|u| u.name.clone())
            .unwrap_or_default();
        self.users
            .rename
            .update(cx, |input, cx| input.set_value(name, window, cx));
        self.users
            .card
            .update(cx, |input, cx| input.set_value("", window, cx));
    }
    pub(crate) fn navigate(&mut self, page: Page, window: &mut Window, cx: &mut Context<Self>) {
        self.page = page;
        self.close_editor(cx);
        self.users.menu_open = false;
        self.sync_user_inputs(window, cx);
        cx.notify();
    }
    fn select_user(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        if !self.store.document_config().users.contains_key(&id) {
            return;
        }
        self.editor = None;
        self.artwork_editor = None;
        self.users.selected = Some(id);
        self.select_scope(page_scope(self.page, self.users.selected.as_deref()), cx);
        self.sync_bilibili_user();
        self.users.menu_open = false;
        self.sync_user_inputs(window, cx);
        cx.notify();
    }
    fn create_user(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.users.name.read(cx).value().to_string();
        if name.trim().is_empty() {
            return;
        }
        if let Some(id) = self.store.create_user(&name) {
            self.page = Page::Account;
            self.select_user(id, window, cx);
            self.users
                .name
                .update(cx, |input, cx| input.set_value("", window, cx));
        }
        cx.notify();
    }
    fn reconcile_user(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.users.selected =
            resolve_user(self.store.document_config(), self.users.selected.as_deref());
        self.store
            .set_scope(page_scope(self.page, self.users.selected.as_deref()));
        self.sync_bilibili_user();
        self.invalidate_user_previews(cx);
        self.sync_user_inputs(window, cx);
    }

    fn sync_bilibili_user(&mut self) {
        self.bili.select(
            self.store
                .document_config()
                .for_user(self.users.selected.as_deref())
                .bilibili
                .account,
        );
    }
    pub(crate) fn reload_config(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.store.reload() {
            self.reconcile_user(window, cx);
        }
        cx.notify();
    }
    pub(crate) fn toggle_user_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.users.menu_open = !self.users.menu_open;
        if self.users.menu_open {
            self.users.scroll.set_offset(Default::default());
            self.users
                .search
                .update(cx, |input, cx| input.set_value("", window, cx));
            let input = if self.store.document_config().users.is_empty() {
                &self.users.name
            } else {
                &self.users.search
            };
            input.read(cx).focus_handle(cx).focus(window, cx);
        }
        cx.notify();
    }
    pub(crate) fn render_user_menu(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let ids = matching_users(
            self.store.document_config(),
            self.users.selected.as_deref(),
            self.users.search.read(cx).value().as_ref(),
        );
        let list_height = (window.viewport_size().height.as_f32() - 250.)
            .clamp(96., 306.)
            .min((ids.len().max(1) * 34) as f32);
        let mut list = v_flex()
            .id("user-switcher-list")
            .relative()
            .w_full()
            .pr_3()
            .h(px(list_height))
            .flex_shrink_0()
            .overflow_y_scroll()
            .track_scroll(&self.users.scroll)
            .vertical_scrollbar(&self.users.scroll);
        for id in &ids {
            let user = &self.store.document_config().users[id];
            let current = self.users.selected.as_ref() == Some(id);
            let switch_id = id.clone();
            let manage_id = id.clone();
            list = list.child(
                h_flex()
                    .w_full()
                    .gap_1()
                    .h(px(34.))
                    .flex_shrink_0()
                    .child(
                        div()
                            .id(SharedString::from(format!("switch-{id}")))
                            .flex_1()
                            .min_w_0()
                            .h(px(34.))
                            .px_3()
                            .flex()
                            .items_center()
                            .gap_2()
                            .cursor_pointer()
                            .border_l_2()
                            .border_color(if current {
                                theme::accent()
                            } else {
                                theme::card_bg()
                            })
                            .text_color(if current {
                                theme::accent()
                            } else {
                                theme::ink()
                            })
                            .when(current, |row| row.bg(gpui::rgba(0x5fe08e18)))
                            .hover(|row| row.text_color(theme::accent()))
                            .child(
                                Icon::new(if current {
                                    IconName::Check
                                } else {
                                    IconName::User
                                })
                                .small(),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_ellipsis()
                                    .overflow_hidden()
                                    .child(user.name.clone()),
                            )
                            .child(div().text_xs().child(format!("{} 张卡", user.cards.len())))
                            .on_click(cx.listener(move |view, _, window, cx| {
                                view.select_user(switch_id.clone(), window, cx)
                            })),
                    )
                    .child(
                        widgets::button(SharedString::from(format!("manage-{id}")))
                            .ghost()
                            .small()
                            .icon(IconName::Settings)
                            .tooltip("用户与卡片设置")
                            .on_click(cx.listener(move |view, _, window, cx| {
                                view.select_user(manage_id.clone(), window, cx);
                                view.navigate(Page::Account, window, cx);
                            })),
                    ),
            );
        }
        if ids.is_empty() {
            list = list.child(
                div()
                    .px_3()
                    .h(px(34.))
                    .flex()
                    .items_center()
                    .text_sm()
                    .text_color(theme::muted_ink())
                    .child(if self.store.document_config().users.is_empty() {
                        "暂无用户"
                    } else {
                        "没有匹配的用户"
                    }),
            );
        }
        let panel = v_flex()
            .id("user-menu-panel")
            .absolute()
            .left(px(theme::SIDEBAR_WIDTH + 8.))
            .bottom(px(12.))
            .w(px(300.))
            .text_sm()
            .p_2()
            .gap_2()
            .bg(theme::card_bg())
            .border_1()
            .border_color(theme::line())
            .shadow_lg()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .child(
                h_flex()
                    .gap_2()
                    .child(Input::new(&self.users.name).small())
                    .child(
                        widgets::button("create-user")
                            .ghost()
                            .border_1()
                            .border_color(theme::line())
                            .small()
                            .icon(IconName::Plus)
                            .tooltip("创建用户")
                            .disabled(self.users.name.read(cx).value().trim().is_empty())
                            .on_click(
                                cx.listener(|view, _, window, cx| view.create_user(window, cx)),
                            ),
                    ),
            )
            .child(
                Input::new(&self.users.search)
                    .small()
                    .prefix(Icon::new(IconName::Search).small()),
            )
            .child(list)
            .when_some(self.store.error(), |v, error| {
                v.child(widgets::error_banner(error.to_owned()))
            })
            .child(
                widgets::button("manage-current-user")
                    .ghost()
                    .small()
                    .label("用户与卡片 · Bilibili")
                    .on_click(
                        cx.listener(|view, _, window, cx| view.navigate(Page::Account, window, cx)),
                    ),
            );
        div()
            .id("user-menu-overlay")
            .absolute()
            .inset_0()
            .occlude()
            .on_key_down(cx.listener(|view, event: &gpui::KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    view.users.menu_open = false;
                    cx.stop_propagation();
                    cx.notify();
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _, _, cx| {
                    view.users.menu_open = false;
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|view, _, _, cx| {
                    view.users.menu_open = false;
                    cx.notify();
                }),
            )
            .child(panel)
            .into_any_element()
    }
    pub(crate) fn render_no_user(&self, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .size_full()
            .items_center()
            .justify_center()
            .gap_4()
            .child(widgets::icon_box(IconName::Users, theme::accent(), 48.))
            .child(div().text_lg().child("尚无用户"))
            .child(
                widgets::button("choose-user")
                    .ghost()
                    .border_1()
                    .border_color(theme::line())
                    .label("创建 / 选择用户")
                    .on_click(cx.listener(|view, _, window, cx| view.toggle_user_menu(window, cx))),
            )
            .into_any_element()
    }
    pub(crate) fn watch_login(&mut self, cx: &mut Context<Self>) {
        let path = self.store.path().to_owned();
        let (tx, rx) = async_channel::bounded(1);
        std::thread::spawn(move || {
            loop {
                if tx.send_blocking(Status::read(&path).ok()).is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        });
        cx.spawn(async move |view, cx| {
            while let Ok(status) = rx.recv().await {
                if view
                    .update(cx, |view, cx| {
                        view.users.status = status;
                        if view.page == Page::Account {
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
    }

    pub(crate) fn render_users(&self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let selected = self.users.selected.clone();
        if selected.is_none() {
            return self.render_no_user(cx);
        }
        let mut body = v_flex()
            .id("user-management")
            .size_full()
            .overflow_y_scrollbar()
            .p_6()
            .gap_4();
        if let Some(id) = selected
            .as_ref()
            .filter(|id| self.store.document_config().users.contains_key(*id))
        {
            let user = &self.store.document_config().users[id];
            let rename_id = id.clone();
            let delete_id = id.clone();
            body = body.child(div().text_lg().child("名称")).child(
                h_flex()
                    .gap_3()
                    .flex_wrap()
                    .child(Input::new(&self.users.rename).w(px(300.)))
                    .child(
                        widgets::button("rename-user")
                            .ghost()
                            .border_1()
                            .border_color(theme::line())
                            .label("保存名称")
                            .on_click(cx.listener(move |view, _, _, cx| {
                                let name = view.users.rename.read(cx).value().to_string();
                                view.store.edit_root(|doc| {
                                    store::set_string(
                                        doc,
                                        &["users", &rename_id, "name"],
                                        name.trim(),
                                    )
                                });
                                cx.notify();
                            })),
                    )
                    .child(
                        widgets::button("delete-user")
                            .ghost()
                            .border_1()
                            .border_color(theme::line())
                            .label(if self.users.deleting.as_ref() == Some(id) {
                                "确认删除用户及绑定"
                            } else {
                                "删除用户"
                            })
                            .on_click(cx.listener(move |view, _, window, cx| {
                                if view.users.deleting.as_ref() == Some(&delete_id) {
                                    if view.store.edit_root(|doc| {
                                        store::remove_value(doc, &["users", &delete_id]);
                                    }) {
                                        view.bili.command(crate::bilibili::Command::Logout);
                                        view.reconcile_user(window, cx);
                                    }
                                    view.users.deleting = None;
                                } else {
                                    view.users.deleting = Some(delete_id.clone());
                                }
                                cx.notify();
                            })),
                    ),
            );
            body = body.child(div().text_lg().child("绑定卡片"));
            if user.cards.is_empty() {
                body = body.child(
                    div()
                        .text_sm()
                        .text_color(theme::muted_ink())
                        .child("暂无绑定卡片"),
                );
            }
            for card in &user.cards {
                let card = card.clone();
                let unbind_id = id.clone();
                body = body.child(
                    h_flex()
                        .gap_3()
                        .child(div().flex_1().child(card.clone()))
                        .child(
                            widgets::button(SharedString::from(format!("unbind-{card}")))
                                .ghost()
                                .border_1()
                                .border_color(theme::line())
                                .label("解除绑定")
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    let cards: Vec<_> = view.store.document_config().users
                                        [&unbind_id]
                                        .cards
                                        .iter()
                                        .filter(|c| **c != card)
                                        .cloned()
                                        .collect();
                                    view.store.edit_root(|doc| {
                                        store::set_strings(
                                            doc,
                                            &["users", &unbind_id, "cards"],
                                            &cards,
                                        )
                                    });
                                    cx.notify();
                                })),
                        ),
                );
            }
            let bind_id = id.clone();
            body = body.child(
                h_flex()
                    .gap_3()
                    .flex_wrap()
                    .child(Input::new(&self.users.card).w(px(300.)))
                    .child(
                        widgets::button("bind-manual-card")
                            .ghost()
                            .border_1()
                            .border_color(theme::line())
                            .label("绑定卡号")
                            .on_click(cx.listener(move |view, _, window, cx| {
                                let card = view.users.card.read(cx).value().to_string();
                                if view.store.bind_card(&bind_id, &card) {
                                    view.users
                                        .card
                                        .update(cx, |input, cx| input.set_value("", window, cx));
                                }
                                cx.notify();
                            })),
                    ),
            );
        }
        body = body.child(div().text_xl().child("当前游戏登录"));
        let status = self.users.status.as_ref().filter(|s| s.fresh());
        match status {
            Some(status) if status.supported => {
                for side in 0..2 {
                    let card = status.login.cards[side].clone();
                    let text = card
                        .as_ref()
                        .map(|card| {
                            let user = self
                                .store
                                .document_config()
                                .user_for_card(card)
                                .and_then(|id| self.store.document_config().users.get(id));
                            format!(
                                "P{} · {} · {}",
                                side + 1,
                                card,
                                user.map(|u| u.name.as_str())
                                    .unwrap_or("未绑定，使用原始模块")
                            )
                        })
                        .unwrap_or_else(|| {
                            format!(
                                "P{} · {}",
                                side + 1,
                                if status.login.joined[side] {
                                    "游客或尚未完成登录"
                                } else {
                                    "未登录"
                                }
                            )
                        });
                    let bind = card.zip(selected.clone());
                    body = body.child(
                        h_flex()
                            .gap_3()
                            .child(div().flex_1().child(text))
                            .when_some(bind, |row, (card, id)| {
                                row.child(
                                    widgets::button(SharedString::from(format!(
                                        "bind-current-{side}"
                                    )))
                                    .ghost()
                                    .border_1()
                                    .border_color(theme::line())
                                    .label("绑定到当前用户")
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        if view.users.status.as_ref().is_some_and(|s| {
                                            s.fresh() && s.login.cards[side].as_ref() == Some(&card)
                                        }) {
                                            view.store.bind_card(&id, &card);
                                        }
                                        cx.notify();
                                    })),
                                )
                            }),
                    );
                }
            }
            Some(_) => body = body.child("此版本不支持读取登录信息"),
            None => body = body.child("未检测到游戏登录"),
        }
        body = body.child(self.render_bilibili(cx));
        body.into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{matching_users, resolve_user, user_scope};
    use crate::store::Scope;
    use iidxbg::config::Config;

    fn config() -> Config {
        Config::parse(&format!(
            "{}{}",
            include_str!("testdata/minimal.toml"),
            r#"
[users.z]
name = "Alice"
cards = ["E004010000000001"]
[users.a]
name = "bob"
cards = ["E004010000000002"]
[users.c]
name = "玩家 三"
"#,
        ))
        .unwrap()
    }

    #[test]
    fn menu_pins_current_user_and_filters_names_cards_and_keywords() {
        let config = config();
        assert_eq!(matching_users(&config, Some("a"), ""), ["a", "z", "c"]);
        assert_eq!(matching_users(&config, None, ""), ["z", "a", "c"]);
        assert_eq!(matching_users(&config, Some("a"), " ALIce e00401 "), ["z"]);
        assert_eq!(matching_users(&config, Some("a"), "00000002"), ["a"]);
        assert_eq!(matching_users(&config, None, "三 玩家"), ["c"]);
        assert!(matching_users(&config, Some("a"), "alice missing").is_empty());
    }

    #[test]
    fn missing_selection_falls_back_without_entering_legacy_scope() {
        let mut config = config();
        assert_eq!(resolve_user(&config, Some("a")), Some("a".into()));
        config.users.remove("a");
        let selected = resolve_user(&config, Some("a"));
        assert_eq!(selected.as_deref(), Some("z"));
        assert_eq!(user_scope(selected.as_deref()), Scope::User("z".into()));
        config.users.clear();
        assert_eq!(resolve_user(&config, selected.as_deref()), None);
        assert_eq!(user_scope(None), Scope::PreLogin);
    }
}
