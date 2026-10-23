//! Account network work is serialized on a dedicated thread; UI receives no cookies.
use crate::{app::AppView, preview, theme, widgets};
use gpui::{
    AnyElement, IntoElement, ObjectFit, ParentElement, RenderImage, SharedString, Styled, div, img,
    px,
};
use gpui_kit::{
    component::{Sizable, button::Button, h_flex, scroll::ScrollableElement, v_flex},
    *,
};
use iidxbg::{
    bilibili::{
        Account, Api, Folder,
        auth::{self, Poll},
    },
    pixels::Prepared,
};
use std::{
    sync::{Arc, mpsc},
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
}
pub(crate) struct State {
    tx: mpsc::Sender<(u64, Command)>,
    generation: u64,
    pub status: SharedString,
    pub qr: Option<Arc<RenderImage>>,
    pub account: Option<Account>,
    pub folders: Vec<Folder>,
}
impl State {
    pub fn new(cx: &mut Context<AppView>) -> Self {
        let (tx, rx) = mpsc::channel();
        let (events, outcomes) = async_channel::unbounded();
        std::thread::Builder::new()
            .name("iidxbg-bilibili-login".into())
            .spawn(move || {
                let mut pending: Option<(u64, Api, auth::Qr, Instant)> = None;
                loop {
                    let result = rx.recv_timeout(Duration::from_secs(2));
                    let (generation, event) = match result {
                        Ok((generation, command)) => {
                            pending = None;
                            let result = (|| -> iidxbg::Result<Event> {
                                match command {
                                    Command::Login => {
                                        let api = Api::new(auth::Session::default())?;
                                        let qr = api.qr()?;
                                        let image = qr_image(&qr.url)?;
                                        pending = Some((generation, api, qr, Instant::now()));
                                        Ok(Event::Qr(image))
                                    }
                                    Command::Refresh => {
                                        let api = Api::saved()?;
                                        let account = api.account()?;
                                        let folders = api.folders()?;
                                        Ok(Event::Account(account, folders))
                                    }
                                    Command::Logout => {
                                        auth::logout()?;
                                        Ok(Event::Clear("已退出本机登录".into()))
                                    }
                                    Command::Cancel => Ok(Event::Message("已取消扫码".into())),
                                }
                            })();
                            (generation, result)
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            let Some((generation, api, qr, started)) = &pending else {
                                continue;
                            };
                            let generation = *generation;
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
                                    auth::save(session).map(|account| {
                                        let folders = Api::saved()
                                            .and_then(|a| a.folders())
                                            .unwrap_or_default();
                                        Event::Account(account, folders)
                                    })
                                }
                                Err(error) => {
                                    pending = None;
                                    Err(error)
                                }
                            };
                            (generation, event)
                        }
                    };
                    let event = event.unwrap_or_else(|error| Event::Clear(error.to_string()));
                    if events.send_blocking((generation, event)).is_err() {
                        break;
                    }
                }
            })
            .expect("start account worker");
        cx.spawn(async move |view, cx| {
            while let Ok((generation, event)) = outcomes.recv().await {
                if view
                    .update(cx, |view, cx| {
                        if view.bili.generation != generation {
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
        let state = Self {
            tx,
            generation: 0,
            status: "正在检查登录状态…".into(),
            qr: None,
            account: None,
            folders: Vec::new(),
        };
        let _ = state.tx.send((0, Command::Refresh));
        state
    }
    pub fn command(&mut self, command: Command) {
        self.generation = self.generation.wrapping_add(1);
        self.qr = None;
        self.status = "正在处理…".into();
        let _ = self.tx.send((self.generation, command));
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
        let mut body=v_flex().id("bilibili-account-page").size_full().overflow_y_scrollbar().p_8().gap_6()
            .child(div().text_2xl().child("Bilibili"))
            .child(div().text_base().text_color(theme::muted_ink()).child("扫码登录后，可在每个组件中选择推荐、合集或收藏夹。请在运行游戏的机器上登录；登录信息仅对本机当前 Windows 用户有效。"))
            .child(div().text_base().child(self.bili.status.clone()));
        let mut buttons = h_flex().gap_3().flex_wrap();
        for (id, label, command) in [
            ("login", "扫码登录", Command::Login),
            ("refresh", "刷新账号 / 收藏夹", Command::Refresh),
            ("cancel", "取消扫码", Command::Cancel),
            ("logout", "退出本机登录", Command::Logout),
        ] {
            buttons = buttons.child(Button::new(id).large().outline().label(label).on_click(
                cx.listener(move |view, _, _, cx| {
                    view.bili.command(command);
                    cx.notify();
                }),
            ));
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
        body = body.child(div().text_base().child("我的收藏夹"));
        for folder in &self.bili.folders {
            body = body.child(widgets::muted_path(format!(
                "{} · ID {}",
                folder.title, folder.id
            )));
        }
        body=body.child(div().text_sm().text_color(theme::muted_ink()).child("在手机 App 中维护收藏夹即可更新视频来源。组件设置中可直接选择这里的收藏夹，也可粘贴其他可访问的收藏夹链接。视频静音循环播放，不提交观看进度。"));
        body.into_any_element()
    }
}
