//! Account network work is serialized on a dedicated thread; UI receives no cookies.
use crate::{app::AppView, preview, theme, widgets};
use gpui::{
    AnyElement, IntoElement, ObjectFit, ParentElement, RenderImage, SharedString, Styled, div, img,
    px,
};
use gpui_kit::{
    component::{button::ButtonVariants, h_flex, v_flex},
    *,
};
use iidxbg::{
    bilibili::{
        Account, Api, Folder,
        auth::{self, AccountKey, Poll},
    },
    pixels::Prepared,
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

#[derive(Clone, Copy)]
pub(crate) enum Command {
    Refresh,
    Login,
    Cancel,
    Logout,
}
enum Event {
    Message(String),
    Qr(Prepared),
    Account(Account, Vec<Folder>),
    Clear(String),
    SignedOut,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Ticket {
    generation: u64,
    key: Option<AccountKey>,
}
impl Ticket {
    fn current(&self, latest: &AtomicU64) -> bool {
        latest.load(Ordering::Acquire) == self.generation
    }
}
pub(crate) struct State {
    tx: mpsc::Sender<(Ticket, Command)>,
    ticket: Ticket,
    latest: Arc<AtomicU64>,
    pub status: SharedString,
    pub qr: Option<Arc<RenderImage>>,
    pub account: Option<Account>,
    pub folders: Vec<Folder>,
}
impl Drop for State {
    fn drop(&mut self) {
        self.latest.fetch_add(1, Ordering::AcqRel);
    }
}
impl State {
    pub fn new(key: Option<AccountKey>, cx: &mut Context<AppView>) -> Self {
        let (tx, rx) = mpsc::channel::<(Ticket, Command)>();
        let (events, outcomes) = async_channel::unbounded();
        let latest = Arc::new(AtomicU64::new(0));
        let worker_latest = latest.clone();
        std::thread::Builder::new()
            .name("iidxbg-bilibili-login".into())
            .spawn(move || {
                let mut pending: Option<(Ticket, Api, auth::Qr, Instant)> = None;
                loop {
                    let result = rx.recv_timeout(Duration::from_secs(2));
                    let (ticket, event) = match result {
                        Ok((ticket, command)) => {
                            pending = None;
                            // Explicit logout/deletion must finish for its captured user,
                            // even if selection has moved on while network IO was pending.
                            if !ticket.current(&worker_latest)
                                && !matches!(command, Command::Logout)
                            {
                                continue;
                            }
                            let result = (|| -> iidxbg::Result<Event> {
                                let Some(key) = ticket.key.as_ref() else {
                                    return Ok(Event::Clear("请选择用户".into()));
                                };
                                match command {
                                    Command::Login => {
                                        let api = Api::new(auth::Session::default())?;
                                        let qr = api.qr()?;
                                        let image = qr_image(&qr.url)?;
                                        pending = Some((ticket.clone(), api, qr, Instant::now()));
                                        Ok(Event::Qr(image))
                                    }
                                    Command::Refresh => {
                                        let api = Api::saved(Some(key))?;
                                        let account = api.account()?;
                                        let folders = api.folders()?;
                                        Ok(Event::Account(account, folders))
                                    }
                                    Command::Logout => {
                                        auth::logout(key)?;
                                        Ok(Event::SignedOut)
                                    }
                                    Command::Cancel => Ok(Event::Message("已取消扫码".into())),
                                }
                            })();
                            (ticket, result)
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            let Some((ticket, api, qr, started)) = &pending else {
                                continue;
                            };
                            let ticket = ticket.clone();
                            if !ticket.current(&worker_latest) {
                                pending = None;
                                continue;
                            }
                            let result = if started.elapsed() > Duration::from_secs(180) {
                                Ok(Poll::Expired)
                            } else {
                                api.poll(qr)
                            };
                            let event = match result {
                                Ok(Poll::Waiting) => {
                                    Ok(Event::Message("请使用哔哩哔哩手机 App 扫码".into()))
                                }
                                Ok(Poll::Scanned) => {
                                    Ok(Event::Message("已扫码，请在手机上确认登录".into()))
                                }
                                Ok(Poll::Expired) => {
                                    pending = None;
                                    Ok(Event::Clear("二维码已过期，请重新获取".into()))
                                }
                                Ok(Poll::Ready(session)) => {
                                    pending = None;
                                    if !ticket.current(&worker_latest) {
                                        continue;
                                    }
                                    let verified = Api::new(session.clone()).and_then(|api| {
                                        let account = api.account()?;
                                        Ok((api, account))
                                    });
                                    // Verification can block. Check again immediately before
                                    // saving; the key always belongs to the QR's original user.
                                    if !ticket.current(&worker_latest) {
                                        continue;
                                    }
                                    verified.and_then(|(api, account)| {
                                        auth::save(
                                            ticket.key.as_ref().expect("login user"),
                                            &session,
                                        )?;
                                        Ok(Event::Account(
                                            account,
                                            api.folders().unwrap_or_default(),
                                        ))
                                    })
                                }
                                Err(error) => {
                                    pending = None;
                                    Err(error)
                                }
                            };
                            (ticket, event)
                        }
                    };
                    let event = event.unwrap_or_else(|error| Event::Clear(error.to_string()));
                    if events.send_blocking((ticket, event)).is_err() {
                        break;
                    }
                }
            })
            .expect("start account worker");
        cx.spawn(async move |view, cx| {
            while let Ok((ticket, event)) = outcomes.recv().await {
                if view
                    .update(cx, |view, cx| {
                        if view.bili.ticket != ticket {
                            return;
                        }
                        match event {
                            Event::Message(text) => view.bili.status = text.into(),
                            Event::Clear(text) => {
                                view.bili.status = text.into();
                                view.bili.qr = None;
                                view.bili.account = None;
                                view.bili.folders.clear();
                            }
                            Event::SignedOut => {
                                view.bili.status = "未绑定账号".into();
                                view.bili.clear_account();
                                view.refresh_bilibili_previews(cx);
                            }
                            Event::Qr(pixels) => {
                                view.bili.qr = Some(preview::render_image(pixels));
                                view.bili.status = "请使用哔哩哔哩手机 App 扫码".into();
                            }
                            Event::Account(account, folders) => {
                                view.bili.status =
                                    format!("已登录：{} · UID {}", account.name, account.mid)
                                        .into();
                                view.bili.account = Some(account);
                                view.bili.folders = folders;
                                view.bili.qr = None;
                                view.refresh_bilibili_previews(cx);
                            }
                        }
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        let mut state = Self {
            tx,
            ticket: Ticket { generation: 0, key },
            latest,
            status: "正在检查登录状态…".into(),
            qr: None,
            account: None,
            folders: Vec::new(),
        };
        state.command(Command::Refresh);
        state
    }
    fn clear_account(&mut self) {
        self.qr = None;
        self.account = None;
        self.folders.clear();
    }
    pub fn select(&mut self, key: Option<AccountKey>) {
        if self.ticket.key == key {
            return;
        }
        self.ticket.key = key;
        self.clear_account();
        self.command(Command::Refresh);
    }
    pub fn command(&mut self, command: Command) {
        self.ticket.generation = self.ticket.generation.wrapping_add(1);
        self.latest.store(self.ticket.generation, Ordering::Release);
        self.qr = None;
        if matches!(command, Command::Logout) {
            self.clear_account();
        }
        self.status = if self.ticket.key.is_some() {
            "正在处理…"
        } else {
            "请选择用户"
        }
        .into();
        let _ = self.tx.send((self.ticket.clone(), command));
    }
}

fn qr_image(url: &str) -> iidxbg::Result<Prepared> {
    let code =
        qrcode::QrCode::new(url).map_err(|_| iidxbg::Error::Bilibili("无法生成二维码".into()))?;
    let modules = code.width();
    let scale = 5;
    let width = (modules + 8) * scale;
    let mut bgra = vec![255; width * width * 4];
    for y in 0..modules {
        for x in 0..modules {
            if code[(x, y)] == qrcode::Color::Dark {
                for dy in 0..scale {
                    for dx in 0..scale {
                        let pos = (((y + 4) * scale + dy) * width + (x + 4) * scale + dx) * 4;
                        bgra[pos..pos + 3].fill(0);
                    }
                }
            }
        }
    }
    Ok(Prepared {
        label: "Bilibili login".into(),
        width: width as u32,
        height: width as u32,
        bgra,
    })
}
impl AppView {
    pub(crate) fn render_bilibili(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut body = v_flex()
            .id("bilibili-account-section")
            .w_full()
            .mt_4()
            .pt_5()
            .border_t_1()
            .border_color(theme::line())
            .gap_4()
            .child(
                h_flex()
                    .gap_2()
                    .child(div().text_lg().child("Bilibili"))
                    .child(widgets::help(
                        "bilibili-account-help",
                        "扫码绑定到当前用户；切换用户会使用对应账号。",
                    )),
            )
            .child(div().text_base().child(self.bili.status.clone()));
        let mut buttons = h_flex().gap_3().flex_wrap();
        for (id, label, command) in [
            ("login", "扫码登录", Command::Login),
            ("refresh", "刷新账号 / 收藏夹", Command::Refresh),
            ("cancel", "取消扫码", Command::Cancel),
            ("logout", "解除绑定", Command::Logout),
        ] {
            buttons = buttons.child(
                widgets::button(id)
                    .ghost()
                    .border_1()
                    .border_color(theme::line())
                    .label(label)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.bili.command(command);
                        cx.notify();
                    })),
            );
        }
        body = body.child(buttons);
        if let Some(qr) = &self.bili.qr {
            body = body.child(
                img(qr.clone())
                    .w(px(300.))
                    .h(px(300.))
                    .object_fit(ObjectFit::Contain),
            );
        }
        if !self.bili.folders.is_empty() {
            body = body.child(
                h_flex()
                    .gap_2()
                    .child(div().text_base().child("我的收藏夹"))
                    .child(widgets::help(
                        "bilibili-folders-help",
                        "在 Bilibili App 中管理收藏夹，在组件设置中选择视频来源。",
                    )),
            );
        }
        for folder in &self.bili.folders {
            body = body.child(widgets::muted_path(format!(
                "{} · ID {}",
                folder.title, folder.id
            )));
        }
        body.into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{Command, State, Ticket};
    use std::sync::{Arc, atomic::AtomicU64, mpsc};

    #[test]
    fn switching_discards_account_data_and_late_qr_but_keeps_logout_target() {
        let mut config =
            iidxbg::config::Config::parse(include_str!("../../tests/fixtures/default.toml"))
                .unwrap();
        config.config_path = "cabinet.toml".into();
        for id in ["alice", "bob"] {
            config.users.insert(
                id.into(),
                iidxbg::users::User {
                    name: id.into(),
                    ..Default::default()
                },
            );
        }
        let alice = config.for_user(Some("alice")).bilibili.account;
        let bob = config.for_user(Some("bob")).bilibili.account;
        let (tx, rx) = mpsc::channel();
        let mut state = State {
            tx,
            ticket: Ticket {
                generation: 0,
                key: alice.clone(),
            },
            latest: Arc::new(AtomicU64::new(0)),
            status: "".into(),
            qr: None,
            account: Some(iidxbg::bilibili::Account {
                mid: 1,
                name: "Alice".into(),
            }),
            folders: vec![iidxbg::bilibili::Folder {
                id: 12,
                title: "Alice's folder".into(),
            }],
        };
        state.command(Command::Login);
        let (qr, _) = rx.recv().unwrap();
        state.command(Command::Logout);
        state.select(bob.clone());
        let (logout, command) = rx.recv().unwrap();
        assert!(matches!(command, Command::Logout));
        assert_eq!(logout.key, alice);
        let (refresh, command) = rx.recv().unwrap();
        assert!(matches!(command, Command::Refresh));
        assert_eq!(refresh.key, bob);
        assert!(!qr.current(&state.latest));
        assert!(refresh.current(&state.latest));
        assert!(state.account.is_none());
        assert!(state.folders.is_empty());
        assert!(state.qr.is_none());
        state.select(None);
        assert!(!refresh.current(&state.latest));
        assert!(state.ticket.key.is_none());
    }
}
