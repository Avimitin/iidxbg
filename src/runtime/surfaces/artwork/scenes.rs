//! Native background-layer substitution; graphics objects stay on the owner.
use super::*;
use std::cell::Cell;
thread_local! { static SCOPE: Cell<Option<Surface>> = const { Cell::new(None) }; }
#[derive(Clone, Copy)]
struct Background {
    owner: usize,
    clip: u32,
    surface: Surface,
    priority: u32,
    serial: u64,
}
static OWNERS: Mutex<Vec<Background>> = Mutex::new(Vec::new());
static SERIAL: AtomicU64 = AtomicU64::new(1);
static SPRITE: AtomicUsize = AtomicUsize::new(0);
static RETIRED: AtomicBool = AtomicBool::new(false);
#[derive(Default)]
struct Backdrop {
    owner: usize,
    serial: u64,
    choice: Option<Asset>,
    sprite: Option<Sprite>,
    hidden: Vec<(usize, u32, u64, u8)>,
    failed: bool,
}
static BACKDROP: Mutex<Backdrop> = Mutex::new(Backdrop {
    owner: 0,
    serial: 0,
    choice: None,
    sprite: None,
    hidden: Vec::new(),
    failed: false,
});

pub(super) fn active(_: &crate::config::Artwork) -> bool {
    true
}
pub(super) unsafe fn created(layer: usize, name: &[u8], priority: u32) {
    let scope = SCOPE.with(Cell::get);
    let selected = matches!(
        (scope, name),
        (Some(Surface::Login | Surface::Logout), b"card_bg2")
            | (Some(Surface::ModeSelect), b"frame_layer2")
    );
    if !selected {
        return;
    }
    let Some(n) = NATIVE.get() else {
        return;
    };
    if unsafe { field(layer, 0) } != n.base + n.profile.surface_data["layer_vtable"] {
        return;
    }
    let clip = unsafe { field(layer, 528) } as u32;
    if clip & 0x8000_0000 != 0 {
        return;
    }
    let surface = scope.unwrap();
    if let Ok(mut owners) = OWNERS.lock() {
        owners.retain(|b| b.owner != layer);
        owners.push(Background {
            owner: layer,
            clip,
            surface,
            priority,
            serial: SERIAL.fetch_add(1, Ordering::Relaxed),
        });
    }
    changed();
    request_images();
    event(Event::Diag(format!(
        "preset background {} {} owner={layer:#x} priority={priority}",
        surface.name(),
        String::from_utf8_lossy(name)
    )));
}
pub(super) fn released(layer: usize) {
    if SPRITE.load(Ordering::Acquire) == layer {
        RETIRED.store(true, Ordering::Release);
    }
    if let Ok(mut owners) = OWNERS.lock() {
        owners.retain(|b| b.owner != layer);
    }
}
unsafe fn restore(state: &mut Backdrop, owners: &[Background], n: &Native) {
    if RETIRED.swap(false, Ordering::AcqRel)
        && let Some(sprite) = &mut state.sprite
    {
        sprite.node = 0;
    }
    SPRITE.store(0, Ordering::Release);
    for (owner, clip, serial, value) in state.hidden.drain(..) {
        if owners
            .iter()
            .any(|b| b.owner == owner && b.clip == clip && b.serial == serial)
        {
            unsafe { set_clip_visible(n, clip, value) };
        }
    }
    state.sprite = None;
    state.choice = None;
    state.owner = 0;
    state.serial = 0;
    state.failed = false;
}
pub(super) fn invalidate() {
    if let Some(n) = NATIVE.get()
        && let Ok(owners) = OWNERS.lock()
        && let Ok(mut state) = BACKDROP.lock()
    {
        let owner = state.owner;
        let serial = state.serial;
        let choice = state.choice.clone();
        unsafe { restore(&mut state, &owners, n) };
        state.owner = owner;
        state.serial = serial;
        state.choice = choice;
    }
}
pub(super) unsafe fn update() {
    let Some(n) = NATIVE.get() else {
        return;
    };
    let Ok(owners) = OWNERS.try_lock() else {
        return;
    };
    let Ok(mut state) = BACKDROP.try_lock() else {
        return;
    };
    let latest = owners.last().copied();
    let owner = latest.map_or(0, |b| b.owner);
    let serial = latest.map_or(0, |b| b.serial);
    if state.serial != serial || RETIRED.load(Ordering::Acquire) {
        unsafe { restore(&mut state, &owners, n) };
        state.owner = owner;
        state.serial = serial;
    }
    let Some(latest) = latest else {
        return;
    };
    if state.failed {
        return;
    }
    let index = match latest.surface {
        Surface::Login => 0,
        Surface::ModeSelect => 1,
        Surface::Logout => 2,
        _ => return,
    };
    if state.choice.is_none() {
        state.choice = snapshot().and_then(|b| b.scenes[index].clone());
    }
    if state.sprite.is_none()
        && let Some(asset) = &state.choice
    {
        state.sprite =
            unsafe { Sprite::media(n, asset, latest.priority.saturating_add(1), 1920., 1080.) }
                .ok();
        if let Some(sprite) = &state.sprite {
            SPRITE.store(sprite.node, Ordering::Release);
        } else {
            state.failed = true;
        }
    }
    let Some(sprite) = &state.sprite else {
        return;
    };
    if !unsafe { sprite.update() } {
        unsafe { restore(&mut state, &owners, n) };
        state.owner = owner;
        state.serial = serial;
        state.failed = true;
        return;
    }
    let node = sprite.node;
    for b in owners.iter().filter(|b| b.surface == latest.surface) {
        if !state.hidden.iter().any(|(owner, clip, serial, _)| {
            *owner == b.owner && *clip == b.clip && *serial == b.serial
        }) {
            let Some(value) = (unsafe { clip_visible(n, b.clip) }) else {
                continue;
            };
            state.hidden.push((b.owner, b.clip, b.serial, value));
        }
        unsafe { set_clip_visible(n, b.clip, 0) };
    }
    unsafe { visible(node, true) };
}
type Setup = unsafe extern "system" fn(usize, usize) -> usize;
unsafe fn setup(name: &str, surface: Surface, this: usize, argument: usize) -> usize {
    let previous = SCOPE.with(|scope| scope.replace(Some(surface)));
    let original: Setup = unsafe { original(name) };
    let result = unsafe { original(this, argument) };
    SCOPE.with(|scope| scope.set(previous));
    result
}
pub(super) unsafe extern "system" fn login(this: usize, argument: usize) -> usize {
    unsafe { setup("artwork_login_setup", Surface::Login, this, argument) }
}
pub(super) unsafe extern "system" fn logout(this: usize, argument: usize) -> usize {
    unsafe { setup("artwork_logout_setup", Surface::Logout, this, argument) }
}
pub(super) unsafe extern "system" fn frame_build(this: usize) -> usize {
    let mode = unsafe { ((this + 44) as *const u32).read_unaligned() };
    let previous = SCOPE.with(|scope| scope.replace((mode == 1).then_some(Surface::ModeSelect)));
    let original: unsafe extern "system" fn(usize) -> usize =
        unsafe { original("select_frame_build") };
    let result = unsafe { original(this) };
    SCOPE.with(|scope| scope.set(previous));
    result
}
