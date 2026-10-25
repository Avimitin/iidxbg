//! Bounded reads and status-file IO live on a dedicated observer, never a draw.
use super::*;
use crate::session::{Login, Status};

static LOGIN: Mutex<Login> = Mutex::new(Login {
    joined: [false; 2],
    cards: [None, None],
});
static CONTEXT: AtomicU64 = AtomicU64::new(1);
static PREPARED: AtomicU64 = AtomicU64::new(0);
static GRAPHICS: AtomicU64 = AtomicU64::new(0);

pub(super) fn snapshot() -> (u64, Login) {
    let login = LOGIN.lock().unwrap_or_else(|e| e.into_inner());
    (CONTEXT.load(Ordering::Acquire), login.clone())
}
pub(super) fn changed() -> u64 {
    CONTEXT.fetch_add(1, Ordering::AcqRel) + 1
}
pub(super) fn rebind(expected: u64) -> Option<u64> {
    // A config edit must not label an outdated login snapshot as the newest
    // context when the observer changed cards between snapshot and resolution.
    CONTEXT
        .compare_exchange(expected, expected + 1, Ordering::AcqRel, Ordering::Acquire)
        .ok()
        .map(|previous| previous + 1)
}
pub(super) fn prepared(context: u64) {
    PREPARED.store(context, Ordering::Release);
}
pub(super) fn ready() -> bool {
    let context = CONTEXT.load(Ordering::Acquire);
    PREPARED.load(Ordering::Acquire) == context && GRAPHICS.load(Ordering::Acquire) == context
}

pub(super) fn start(path: PathBuf) -> Result<()> {
    thread::Builder::new()
        .name("iidxbg-login".into())
        .spawn(move || {
            let n = NATIVE.get().expect("attached before observer");
            let mut last_write = Instant::now() - Duration::from_secs(2);
            let mut last_error = Instant::now() - Duration::from_secs(30);
            let mut modified = None;
            while !STOP.load(Ordering::Acquire) {
                let login = n
                    .profile
                    .session
                    .as_ref()
                    .map(|layout| {
                        crate::session::sample(layout, |rva, bytes| {
                            let mut copied = 0;
                            unsafe {
                                ReadProcessMemory(
                                    GetCurrentProcess(),
                                    (n.base + rva) as _,
                                    bytes.as_mut_ptr().cast(),
                                    bytes.len(),
                                    &mut copied,
                                ) != 0
                                    && copied == bytes.len()
                            }
                        })
                    })
                    .unwrap_or_default();
                let did_change = {
                    let mut current = LOGIN.lock().unwrap_or_else(|e| e.into_inner());
                    if *current != login {
                        *current = login.clone();
                        changed();
                        true
                    } else {
                        false
                    }
                };
                if did_change {
                    request_images();
                }
                if did_change || last_write.elapsed() >= Duration::from_secs(1) {
                    let status = Status {
                        process_id: std::process::id(),
                        updated_at: crate::session::now(),
                        supported: n.profile.session.is_some(),
                        login,
                    };
                    if let Err(error) = status.write(&path) {
                        // Rate limited by this observer's heartbeat; no card values in logs.
                        if last_error.elapsed() >= Duration::from_secs(30) {
                            event(Event::Diag(format!("Login status write failed: {error}")));
                            last_error = Instant::now();
                        }
                    }
                    let next = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
                    if next != modified {
                        modified = next;
                        request_images();
                    }
                    last_write = Instant::now();
                }
                thread::sleep(Duration::from_millis(100));
            }
            // An absent heartbeat marks the retained snapshot stale even on abnormal exit.
            let _ = Status {
                process_id: std::process::id(),
                updated_at: 0,
                supported: n.profile.session.is_some(),
                login: Login::default(),
            }
            .write(&path);
        })?;
    Ok(())
}

/// Called on the SDK graphics owner. Reset choices only on identity changes;
/// ordinary settings edits retain the established result-visit freeze.
pub(super) fn graphics() {
    let context = CONTEXT.load(Ordering::Acquire);
    if GRAPHICS.load(Ordering::Acquire) == context {
        return;
    }
    if let Some(n) = NATIVE.get() {
        {
            let Some(mut state) = graphics_state() else {
                return;
            };
            unsafe { state.invalidate(n) };
            state.selection = Selection::default();
            state.playback = None;
            state.background = 0;
        }
        if surfaces::clear_session() {
            GRAPHICS.store(context, Ordering::Release);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_login_rejects_old_rebinding_and_prepared_context() {
        let first = CONTEXT.load(Ordering::Acquire);
        PREPARED.store(first, Ordering::Release);
        GRAPHICS.store(first, Ordering::Release);
        assert!(ready());
        let second = changed();
        assert!(!ready());
        assert_eq!(rebind(first), None);
        prepared(first);
        assert!(!ready());
        let rebound = rebind(second).unwrap();
        prepared(rebound);
        assert!(!ready());
        GRAPHICS.store(rebound, Ordering::Release);
        assert!(ready());
    }
}
