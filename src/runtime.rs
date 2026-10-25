//! Shared verified ABI; native addresses/layouts come from exact-build profiles.
mod reactions;
mod session;
mod surfaces;
use crate::{
    Error, Result,
    config::{Config, Surface},
    hook_chain,
    pixels::Prepared,
    profile,
    source::Providers,
    state::{Pending, Selection},
};
use sha2::{Digest, Sha256};
use std::{
    ffi::c_void,
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, MutexGuard, OnceLock, TryLockError,
        atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering},
        mpsc::{self, SyncSender},
    },
    thread,
    time::{Duration, Instant},
};
use windows::{Win32::Graphics::Direct3D9::*, core::Interface};
use windows_sys::Win32::System::{
    Diagnostics::Debug::ReadProcessMemory,
    LibraryLoader::{
        GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_PIN,
        GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT, GetModuleFileNameW, GetModuleHandleExW,
        GetModuleHandleW,
    },
    Memory::{
        MEM_COMMIT, MEMORY_BASIC_INFORMATION, PAGE_EXECUTE, PAGE_EXECUTE_READ,
        PAGE_EXECUTE_READWRITE, PAGE_EXECUTE_WRITECOPY, PAGE_GUARD, PAGE_NOACCESS, PAGE_READWRITE,
        VirtualProtect, VirtualQuery,
    },
    Threading::{
        GetCurrentProcess, GetCurrentThread, GetCurrentThreadId, SetThreadPriority,
        THREAD_PRIORITY_BELOW_NORMAL,
    },
};

static MODULE: AtomicUsize = AtomicUsize::new(0);
static STARTED: AtomicBool = AtomicBool::new(false);
static STOP: AtomicBool = AtomicBool::new(false);
static ENABLED: AtomicBool = AtomicBool::new(false);
static READY: AtomicBool = AtomicBool::new(false);
static OWNER: AtomicU32 = AtomicU32::new(0);
static REQUEST: AtomicU64 = AtomicU64::new(1);
static WORKER: OnceLock<thread::Thread> = OnceLock::new();
static HIDE_FRAME_CHARA: AtomicBool = AtomicBool::new(false);
static MMD_FRAME_CHARA: AtomicBool = AtomicBool::new(false);
static MMD_QPRO: AtomicBool = AtomicBool::new(false);
static PREMIUM_AREA_ENABLED: AtomicBool = AtomicBool::new(false);
static CONCENTRATION_ENABLED: AtomicBool = AtomicBool::new(false);
static PENDING: Mutex<Option<Pending>> = Mutex::new(None);
static GPU: Mutex<Graphics> = Mutex::new(Graphics::new());
static NATIVE: OnceLock<Native> = OnceLock::new();
static EVENTS: OnceLock<SyncSender<Event>> = OnceLock::new();
/// Exact `illust` clip captured from the native
/// `set_archive(clip, archive, "result_bg")` call. Binding this object avoids a
/// second named lookup returning a different CMovieClip wrapper.
pub(super) static RESULTS_CLIP: AtomicUsize = AtomicUsize::new(0);
pub(super) static RESULTS_STOCK_REBINDS: AtomicU32 = AtomicU32::new(0);

fn graphics_state() -> Option<MutexGuard<'static, Graphics>> {
    match GPU.try_lock() {
        Ok(state) => Some(state),
        Err(TryLockError::Poisoned(error)) => Some(error.into_inner()),
        Err(TryLockError::WouldBlock) => None,
    }
}

#[derive(Debug)]
enum Event {
    Applied {
        width: u32,
        height: u32,
        upload_us: u128,
    },
    Centered {
        x: f32,
        y: f32,
    },
    Stock,
    Fault(&'static str),
    Invalidated,
    Surface(&'static str),
    Diag(String),
}

fn wake_worker() {
    if let Some(worker) = WORKER.get() {
        worker.unpark();
    }
}

fn request_images() {
    REQUEST.fetch_add(1, Ordering::AcqRel);
    wake_worker();
}

fn event(value: Event) {
    if let Some(tx) = EVENTS.get() {
        let _ = tx.try_send(value);
        wake_worker();
    }
}

pub(crate) fn trace_mmd_scratch(side: usize, direction: i32) {
    event(Event::Diag(format!(
        "MMD Spice scratch P{}: {} motion trigger",
        side + 1,
        if direction < 0 { "up" } else { "down" }
    )));
}

struct Native {
    base: usize,
    profile: profile::Profile,
}

impl Native {
    /// Only concrete function-pointer types whose ABI is documented may be used.
    unsafe fn function<T: Copy>(&self, name: &str) -> T {
        assert_eq!(std::mem::size_of::<T>(), std::mem::size_of::<usize>());
        let address = self.base + self.profile.rvas[name];
        unsafe { std::mem::transmute_copy(&address) }
    }
}

type Background = unsafe extern "system" fn(usize, u32) -> usize;
type Cleanup = unsafe extern "system" fn(usize) -> usize;
type SceneDelete = unsafe extern "system" fn(usize, u8) -> usize;
type FindClip = unsafe extern "system" fn(usize, *const u8) -> usize;
type SetArchive = unsafe extern "system" fn(usize, usize, *const u8) -> u8;
type SetTexture = unsafe extern "system" fn(usize, *const u8) -> u8;
type Visible = unsafe extern "system" fn(usize, u8);
type Origin = unsafe extern "system" fn(usize, i32, i32) -> usize;
type OriginPixels = unsafe extern "system" fn(usize, f32, f32);
type Wrap = unsafe extern "system" fn(*mut u64, *mut c_void) -> usize;
type Destroy = unsafe extern "system" fn(*mut u64) -> usize;
type Device = unsafe extern "system" fn() -> *mut c_void;

unsafe fn field(object: usize, offset: usize) -> usize {
    unsafe { ((object + offset) as *const usize).read() }
}

/// Best-effort user-mode readability probe for pointers read out of game
/// objects. Some native fields are IDs or flags rather than addresses; blindly
/// dereferencing them faults the game process (Rust panic containment cannot
/// catch an access violation).
fn readable(address: usize) -> bool {
    if !(0x10000..=0x0000_7fff_ffff_ffff).contains(&address) {
        return false;
    }
    let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
    if unsafe { VirtualQuery(address as _, &mut info, std::mem::size_of_val(&info)) } == 0 {
        return false;
    }
    info.State == MEM_COMMIT
        && info.Protect & PAGE_GUARD == 0
        && info.Protect & 0xff != PAGE_NOACCESS
}

/// Read a pointer-sized field only when both the object and the field address
/// are committed and readable. Returns `None` for ID/flag fields, which is the
/// normal case for several frame offsets.
fn pointer_field(object: usize, offset: usize) -> Option<usize> {
    let address = object.checked_add(offset)?;
    if !readable(object) || !readable(address) || !readable(address.saturating_add(7)) {
        return None;
    }
    Some(unsafe { (address as *const usize).read() })
}

struct Texture {
    // Stable 8-byte-aligned storage. Native constructor initializes all 64 bytes.
    wrapper: Box<[u64; 8]>,
    center: [f32; 2],
    width: u32,
    height: u32,
}

impl Texture {
    unsafe fn wrap(
        native: &Native,
        texture: &IDirect3DTexture9,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        let mut wrapper = Box::new([0u64; 8]);
        let wrap: Wrap = unsafe { native.function("texture_wrap") };
        unsafe {
            wrap(wrapper.as_mut_ptr(), texture.as_raw());
        }
        let result = Self {
            wrapper,
            center: [width as f32 * 0.5, height as f32 * 0.5],
            width,
            height,
        };
        if result.wrapper[1] == 0 || result.wrapper[2] as u32 == 0 {
            return Err(Error::Native("AFP model texture registration"));
        }
        Ok(result)
    }
    unsafe fn upload(native: &Native, image: &Prepared) -> Result<Self> {
        let get_device: Device = unsafe { native.function("get_device") };
        let raw = unsafe { get_device() };
        let device = unsafe { IDirect3DDevice9::from_raw_borrowed(&raw) }
            .ok_or(Error::Native("no D3D9 device"))?;
        let mut texture = None;
        unsafe {
            device.CreateTexture(
                image.width,
                image.height,
                1,
                D3DUSAGE_DYNAMIC as u32,
                D3DFMT_A8R8G8B8,
                D3DPOOL_DEFAULT,
                &mut texture,
                std::ptr::null_mut(),
            )
        }
        .map_err(|_| Error::Native("CreateTexture"))?;
        let texture = texture.ok_or(Error::Native("null texture"))?;
        let mut locked = D3DLOCKED_RECT::default();
        unsafe { texture.LockRect(0, &mut locked, std::ptr::null(), D3DLOCK_DISCARD as u32) }
            .map_err(|_| Error::Native("LockRect"))?;
        let row_bytes = image.width as usize * 4;
        if locked.pBits.is_null() || locked.Pitch < row_bytes as i32 {
            let _ = unsafe { texture.UnlockRect(0) };
            return Err(Error::Native("invalid texture pitch"));
        }
        for y in 0..image.height as usize {
            unsafe {
                std::ptr::copy_nonoverlapping(
                    image.bgra.as_ptr().add(y * row_bytes),
                    (locked.pBits as *mut u8).add(y * locked.Pitch as usize),
                    row_bytes,
                );
            }
        }
        unsafe { texture.UnlockRect(0) }.map_err(|_| Error::Native("UnlockRect"))?;
        let mut wrapper = Box::new([0u64; 8]);
        let wrap: Wrap = unsafe { native.function("texture_wrap") };
        unsafe {
            wrap(wrapper.as_mut_ptr(), texture.as_raw());
        }
        let result = Self {
            wrapper,
            center: [image.width as f32 * 0.5, image.height as f32 * 0.5],
            width: image.width,
            height: image.height,
        };
        if result.wrapper[1] == 0 || result.wrapper[2] as u32 == 0 {
            return Err(Error::Native("AFP texture registration"));
        }
        // Constructor owns a COM reference; local COM owner is now released.
        Ok(result)
    }

    /// Reuse the existing D3D texture when the canvas size is unchanged; only
    /// the pixels are rewritten. Returns true when the texture had to be
    /// recreated (the caller must rebind the descriptor).
    unsafe fn refresh(&mut self, native: &Native, image: &Prepared) -> Result<bool> {
        if self.width != image.width || self.height != image.height {
            *self = unsafe { Self::upload(native, image) }?;
            return Ok(true);
        }
        unsafe { self.write_pixels(image) }?;
        Ok(false)
    }

    /// Graphics-owner-only update; shared descriptors keep a fixed canvas.
    unsafe fn write_pixels(&self, image: &Prepared) -> Result<()> {
        if self.width != image.width || self.height != image.height {
            return Err(Error::Native("shared texture dimensions changed"));
        }
        let raw = self.wrapper[1] as *mut c_void;
        let texture = unsafe { IDirect3DTexture9::from_raw_borrowed(&raw) }
            .ok_or(Error::Native("no D3D9 texture"))?;
        let mut locked = D3DLOCKED_RECT::default();
        unsafe { texture.LockRect(0, &mut locked, std::ptr::null(), D3DLOCK_DISCARD as u32) }
            .map_err(|_| Error::Native("LockRect"))?;
        let row_bytes = image.width as usize * 4;
        if locked.pBits.is_null() || locked.Pitch < row_bytes as i32 {
            let _ = unsafe { texture.UnlockRect(0) };
            return Err(Error::Native("invalid texture pitch"));
        }
        for y in 0..image.height as usize {
            unsafe {
                std::ptr::copy_nonoverlapping(
                    image.bgra.as_ptr().add(y * row_bytes),
                    (locked.pBits as *mut u8).add(y * locked.Pitch as usize),
                    row_bytes,
                );
            }
        }
        unsafe { texture.UnlockRect(0) }.map_err(|_| Error::Native("UnlockRect"))?;
        Ok(())
    }

    /// Diagnostic-only: rewrite the existing texture from the same image and
    /// report lock / row-copy / unlock microseconds separately. The pixels are
    /// unchanged, so the visible frame is unaffected. This measures whether
    /// per-frame video uploads fit the game thread; it is never called on a
    /// production hot path.
    unsafe fn probe_refresh(texture: &Texture, image: &Prepared) -> Option<(u128, u128, u128)> {
        if texture.width != image.width || texture.height != image.height {
            return None;
        }
        let raw = texture.wrapper[1] as *mut c_void;
        let texture = unsafe { IDirect3DTexture9::from_raw_borrowed(&raw) }?;
        let mut locked = D3DLOCKED_RECT::default();
        let lock_start = Instant::now();
        unsafe { texture.LockRect(0, &mut locked, std::ptr::null(), D3DLOCK_DISCARD as u32) }
            .ok()?;
        let lock_us = lock_start.elapsed().as_micros();
        let row_bytes = image.width as usize * 4;
        if locked.pBits.is_null() || locked.Pitch < row_bytes as i32 {
            let _ = unsafe { texture.UnlockRect(0) };
            return None;
        }
        let copy_start = Instant::now();
        for y in 0..image.height as usize {
            unsafe {
                std::ptr::copy_nonoverlapping(
                    image.bgra.as_ptr().add(y * row_bytes),
                    (locked.pBits as *mut u8).add(y * locked.Pitch as usize),
                    row_bytes,
                );
            }
        }
        let copy_us = copy_start.elapsed().as_micros();
        let unlock_start = Instant::now();
        unsafe { texture.UnlockRect(0) }.ok()?;
        let unlock_us = unlock_start.elapsed().as_micros();
        Some((lock_us, copy_us, unlock_us))
    }

    fn descriptor(&self) -> *const u8 {
        unsafe { self.wrapper.as_ptr().cast::<u8>().add(24) }
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        if let Some(native) = NATIVE.get() {
            let destroy: Destroy = unsafe { native.function("texture_destroy") };
            unsafe {
                destroy(self.wrapper.as_mut_ptr());
            }
        }
    }
}

struct Graphics {
    selection: Selection,
    background: usize,
    texture: Option<Texture>,
    /// Current-update wrapper only; the native lookup pool is reset/reused.
    clip: usize,
    /// Bounds retries while the AFP timeline is still creating its children.
    probe: u32,
    /// One scene scan per results visit (diagnostic only).
    scene_diag: bool,
    attempted: bool,
    failed: bool,
    layout_reported: bool,
    playback: Option<crate::media::Playback>,
}

impl Graphics {
    const fn new() -> Self {
        Self {
            selection: Selection {
                active: false,
                selected: None,
                video: None,
            },
            background: 0,
            texture: None,
            clip: 0,
            probe: 0,
            scene_diag: false,
            attempted: false,
            failed: false,
            layout_reported: false,
            playback: None,
        }
    }

    unsafe fn stock(&self, n: &Native) {
        if self.background == 0 {
            return;
        }
        let movie = unsafe { field(self.background, n.profile.layout.background_movie) };
        let find: FindClip = unsafe { n.function("find_clip") };
        let clip = unsafe { find(movie, c"illust".as_ptr().cast()) };
        if clip == 0 {
            return;
        }
        let archive = unsafe { field(self.background, n.profile.layout.background_archive) };
        let visible: Visible = unsafe { n.function("set_visible") };
        if archive != 0 {
            let set: SetArchive = unsafe { n.function("set_archive") };
            let origin: Origin = unsafe { n.function("set_origin") };
            unsafe {
                set(clip, archive, c"result_bg".as_ptr().cast());
                origin(clip, 3, 3);
                visible(clip, 1);
            }
        } else {
            unsafe {
                visible(clip, 0);
            }
        }
    }

    unsafe fn update(&mut self, n: &Native, background: usize) {
        if self.failed {
            return;
        }
        if !self.selection.active {
            unsafe { reactions::result(n) };
            let pending = PENDING.try_lock().ok().and_then(|mut p| p.take());
            self.selection.enter(pending);
            self.playback = self
                .selection
                .video
                .as_ref()
                .map(|video| crate::media::Asset::Video(video.clone()).play());
            request_images();
            self.background = background;
            self.attempted = false;
            self.clip = 0;
            self.probe = 0;
            self.scene_diag = false;
            // Do not reuse a clip captured from the previous results instance;
            // the original callback for this frame has already run and will
            // re-capture it when it binds its archive texture.
            RESULTS_CLIP.store(0, Ordering::Release);
            RESULTS_STOCK_REBINDS.store(0, Ordering::Relaxed);
            self.layout_reported = false;
            if self.selection.selected.is_none() {
                event(Event::Stock);
            }
        }
        if self.background != background {
            self.failed = true;
            event(Event::Fault("unexpected simultaneous results instance"));
            return;
        }
        // Setup runs later than the first component update. Never cache a
        // failed lookup against a null movie as the visit's permanent target.
        let movie = unsafe { field(background, n.profile.layout.background_movie) };
        if movie == 0 {
            return;
        }
        if !self.scene_diag {
            self.scene_diag = true;
            if background >= n.profile.layout.scene_background {
                // Probe after setup, not against the uninitialized component.
                unsafe {
                    surfaces::diag_results(n, background);
                };
            }
        }
        if !self.attempted {
            self.attempted = true;
            if let Some(image) = &self.selection.selected {
                let start = Instant::now();
                let mut failed = false;
                if let Some(texture) = self.texture.as_mut() {
                    if unsafe { texture.refresh(n, image) }.is_err() {
                        failed = true;
                    }
                } else {
                    match unsafe { Texture::upload(n, image) } {
                        Ok(texture) => self.texture = Some(texture),
                        Err(_) => failed = true,
                    }
                }
                if failed {
                    self.texture = None;
                    self.clip = 0;
                    event(Event::Fault(
                        "texture upload/registration failed; stock this visit",
                    ));
                } else {
                    event(Event::Applied {
                        width: image.width,
                        height: image.height,
                        upload_us: start.elapsed().as_micros(),
                    });
                }
            }
        }
        if let (Some(playback), Some(texture)) = (&mut self.playback, self.texture.as_mut())
            && playback.update()
        {
            if unsafe { texture.refresh(n, &playback.pixels) }.is_err() {
                unsafe { self.stock(n) };
                self.texture = None;
                self.failed = true;
                event(Event::Fault("video texture refresh failed; stock fallback"));
                return;
            }
            self.selection.selected = Some(playback.pixels.clone());
        }
        if let Some(texture) = &self.texture {
            let archive = unsafe { field(background, n.profile.layout.background_archive) };
            let game_clip = RESULTS_CLIP.load(Ordering::Acquire);
            // Named wrappers belong to a resettable native pool. Acquire the
            // current frame's wrapper; never reuse one from a previous update.
            self.clip = if archive != 0 && game_clip != 0 {
                game_clip
            } else {
                let find: FindClip = unsafe { n.function("find_clip") };
                unsafe { find(movie, c"illust".as_ptr().cast()) }
            };
            self.probe = self.probe.saturating_add(1);
            let node = pointer_field(self.clip, 512).unwrap_or(usize::MAX) as u32;
            if self.probe == 1 {
                event(Event::Diag(format!(
                    "results ready probe: movie={movie:#x} archive={archive:#x} clip={:#x} node={node:#x}",
                    self.clip
                )));
            }
            if node & 0x8000_0000 != 0 {
                return;
            }
            let set: SetTexture = unsafe { n.function("set_texture") };
            let visible: Visible = unsafe { n.function("set_visible") };
            if unsafe { set(self.clip, texture.descriptor()) } == 0 {
                unsafe { self.stock(n) };
                self.failed = true;
                event(Event::Fault("AFP bind rejected; feature disabled"));
                return;
            }
            // Both native branches use illust. The no-archive branch hides it
            // each frame; the archive branch rebinds it each frame.
            let origin: OriginPixels = unsafe { n.function("set_origin_pixels") };
            unsafe {
                origin(self.clip, texture.center[0], texture.center[1]);
                visible(self.clip, 1);
            }
            if !self.layout_reported {
                self.layout_reported = true;
                event(Event::Centered {
                    x: texture.center[0],
                    y: texture.center[1],
                });
            }
        }
    }

    unsafe fn invalidate(&mut self, n: &Native) {
        unsafe {
            self.stock(n);
        }
        self.texture = None;
        self.clip = 0;
        self.attempted = false;
        RESULTS_CLIP.store(0, Ordering::Release);
    }
}

fn boundary(f: impl FnOnce()) {
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).is_err() {
        ENABLED.store(false, Ordering::Release);
        event(Event::Fault("Rust panic; custom rendering disabled"));
    }
}

unsafe extern "system" fn background_hook(this: usize, tick: u32) -> usize {
    let n = NATIVE.get().expect("published before hook");
    let original: Background = unsafe { n.function("background_update") };
    RESULTS_CLIP.store(0, Ordering::Release);
    if this >= n.profile.layout.scene_background
        && unsafe { field(this - n.profile.layout.scene_background, 0) }
            == n.base + n.profile.layout.scene_vtable
    {
        surfaces::artwork_result_visit(this - n.profile.layout.scene_background, true);
    }
    let result = unsafe { original(this, tick) };
    if !ENABLED.load(Ordering::Acquire) || !READY.load(Ordering::Acquire) || !session::ready() {
        return result;
    }
    boundary(|| {
        // Restrict the shared component table to ordinary CStageResultScene.
        let layout = &n.profile.layout;
        if this < layout.scene_background
            || unsafe { field(this - layout.scene_background, 0) } != n.base + layout.scene_vtable
        {
            return;
        }
        if unsafe { GetCurrentThreadId() } != OWNER.load(Ordering::Acquire) {
            ENABLED.store(false, Ordering::Release);
            event(Event::Fault("results and presentation thread differ"));
            return;
        }
        if let Some(mut state) = graphics_state() {
            unsafe {
                state.update(n, this);
            }
        }
    });
    // The session-global premium nodes can be visible on results; rebuild the
    // replacement here if the scene sprite pool retired it.
    surfaces::tick_premium();
    result
}

unsafe extern "system" fn cleanup_hook(this: usize) -> usize {
    let n = NATIVE.get().expect("published before hook");
    retire_scene(n, this);
    let original: Cleanup = unsafe { n.function("result_cleanup") };
    unsafe { original(this) }
}

fn retire_scene(n: &Native, this: usize) {
    surfaces::artwork_result_visit(this, false);
    // Restore the stock binding while this scene still owns valid clips.
    boundary(|| {
        if let Some(mut state) = graphics_state()
            && state.background == this + n.profile.layout.scene_background
        {
            unsafe {
                state.invalidate(n);
            }
            state.background = 0;
            state.playback = None;
            state.selection.leave();
            request_images();
        }
    });
}

unsafe extern "system" fn delete_hook(this: usize, flags: u8) -> usize {
    let n = NATIVE.get().expect("published before hook");
    // Also cover destruction that did not run the normal scene cleanup callback.
    retire_scene(n, this);
    let original: SceneDelete = unsafe { n.function("result_delete") };
    unsafe { original(this, flags) }
}

fn module_path(module: usize) -> Result<PathBuf> {
    let mut buffer = vec![0u16; 32768];
    let size = unsafe { GetModuleFileNameW(module as _, buffer.as_mut_ptr(), buffer.len() as u32) };
    if size == 0 || size as usize >= buffer.len() {
        return Err(Error::Native("module path"));
    }
    use std::os::windows::ffi::OsStringExt;
    Ok(std::ffi::OsString::from_wide(&buffer[..size as usize]).into())
}

unsafe fn patch_slot(address: usize, expected: usize, replacement: usize) -> Result<()> {
    let mut old = 0;
    if unsafe { VirtualProtect(address as _, 8, PAGE_READWRITE, &mut old) } == 0 {
        return Err(Error::Native("vtable protection"));
    }
    let slot = unsafe { &*(address as *const AtomicUsize) };
    let exchanged =
        slot.compare_exchange(expected, replacement, Ordering::SeqCst, Ordering::SeqCst);
    let mut unused = 0;
    let restored = unsafe { VirtualProtect(address as _, 8, old, &mut unused) };
    if exchanged.is_err() {
        return Err(Error::Native("vtable already hooked"));
    }
    if restored == 0 {
        return Err(Error::Native("vtable protection restoration"));
    }
    Ok(())
}

// Bounded read-only inspection; never execute a discovered target.
// Acceptance is decided separately by hook_chain's structural checks.
fn read_live<const N: usize>(address: usize) -> Option<[u8; N]> {
    let mut bytes = [0; N];
    let mut read = 0;
    let ok = unsafe {
        ReadProcessMemory(
            GetCurrentProcess(),
            address as _,
            bytes.as_mut_ptr().cast(),
            N,
            &mut read,
        )
    };
    (ok != 0 && read == N).then_some(bytes)
}

fn executable(address: usize) -> bool {
    let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
    (unsafe { VirtualQuery(address as _, &mut info, std::mem::size_of_val(&info)) }) != 0
        && info.State == MEM_COMMIT
        && info.Protect & PAGE_GUARD == 0
        && matches!(
            info.Protect & 0xff,
            PAGE_EXECUTE | PAGE_EXECUTE_READ | PAGE_EXECUTE_READWRITE | PAGE_EXECUTE_WRITECOPY
        )
}

fn code_module(address: usize) -> Option<(usize, String)> {
    let mut module = std::ptr::null_mut();
    if unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            address as *const u16,
            &mut module,
        )
    } == 0
    {
        return None;
    }
    Some((
        module as usize,
        module_path(module as usize).ok()?.display().to_string(),
    ))
}

fn describe_entry_jump(root: &Path, mut address: usize) {
    for _ in 0..4 {
        let Some(bytes) = read_live::<16>(address) else {
            return;
        };
        let target = if bytes[0] == 0xe9 {
            address.checked_add(5).and_then(|next|
                next.checked_add_signed(i32::from_le_bytes(bytes[1..5].try_into().unwrap()) as isize))
        } else if bytes[..2] == [0xff, 0x25] {
            address
                .checked_add(6)
                .and_then(|next| {
                    next.checked_add_signed(
                        i32::from_le_bytes(bytes[2..6].try_into().unwrap()) as isize
                    )
                })
                .and_then(read_live::<8>)
                .map(usize::from_le_bytes)
        } else {
            return;
        };
        let Some(target) = target else { return };
        let owner = code_module(target)
            .map(|(_, path)| path)
            .unwrap_or_else(|| "no loaded module (possibly a trampoline)".into());
        log(
            root,
            &format!("Entry jump {address:#x} -> {target:#x}: {owner}"),
        );
        if target == address {
            return;
        }
        address = target;
    }
}

fn attach(root: &Path) -> Result<()> {
    let name: Vec<u16> = "bm2dx.dll\0".encode_utf16().collect();
    let base = unsafe { GetModuleHandleW(name.as_ptr()) } as usize;
    if base == 0 {
        return Err(Error::Build("bm2dx.dll not loaded"));
    }
    let data = std::fs::read(module_path(base)?)?;
    let profile = profile::verify_file(&data)?;
    log(
        root,
        &format!(
            "Verified {} on disk (SHA-256 {}); loaded base {base:#x}. Checking live code.",
            profile.label, profile.sha256,
        ),
    );
    // Memory is the already loaded, fingerprinted PE. Only known mapped .text/.rdata
    // regions are read. Preserve only the validated MinHook form; unknown changes
    // still fail before any vtable mutation.
    let mut mismatch = None;
    for guard in &profile.guards {
        let live = unsafe {
            std::slice::from_raw_parts((base + guard.rva) as *const u8, guard.hex.len() / 2)
        };
        let name = profile
            .rvas
            .iter()
            .find(|(_, rva)| **rva == guard.rva)
            .map(|(name, _)| name.as_str())
            .unwrap_or("unknown");
        if let Err(error) = guard.check(name, live) {
            let original = hex::decode(&guard.hex).map_err(|_| Error::Build("embedded guard"))?;
            match hook_chain::inspect(
                base + guard.rva,
                &original,
                live,
                |addr, size| {
                    (size == 14)
                        .then(|| read_live::<14>(addr))
                        .flatten()
                        .map(|b| b.to_vec())
                },
                executable,
                |addr| {
                    code_module(addr)
                        .filter(|(module, _)| *module != base)
                        .map(|(_, path)| path)
                },
            ) {
                Ok(Some(chain)) => log(
                    root,
                    &format!(
                        "Preserving MinHook chain at {name} (RVA {:#x}): relay {:#x} -> {} ({:#x}); calls remain through the game entry",
                        guard.rva, chain.relay, chain.module, chain.target,
                    ),
                ),
                Ok(None) => {}
                Err(reason) => {
                    log(root, &error.to_string());
                    log(root, &format!("Hook chain rejected: {reason}"));
                    describe_entry_jump(root, base + guard.rva);
                    mismatch.get_or_insert(error);
                }
            }
        }
    }
    for table in &profile.tables {
        let actual = unsafe { field(base + table.rva, table.slot * 8) };
        if actual != base + table.target {
            let error = Error::TableMismatch {
                rva: table.rva,
                slot: table.slot,
                expected: base + table.target,
                actual,
            };
            log(root, &error.to_string());
            mismatch.get_or_insert(error);
        }
    }
    if let Some(error) = mismatch {
        log(
            root,
            "Attachment refused before installing hooks. A live-code difference is not a malformed DLL file.",
        );
        return Err(error);
    }
    NATIVE
        .set(Native { base, profile })
        .map_err(|_| Error::Native("already attached"))?;
    let n = NATIVE.get().unwrap();
    let replacements = [
        background_hook as *const () as usize,
        cleanup_hook as *const () as usize,
        delete_hook as *const () as usize,
    ];
    for (index, table) in n.profile.tables.iter().enumerate() {
        let result = unsafe {
            patch_slot(
                base + table.rva + table.slot * 8,
                base + table.target,
                replacements[index],
            )
        };
        if let Err(e) = result {
            // Include the failed slot in case exchange succeeded but protection
            // restoration failed. CAS never overwrites somebody else's value.
            for (table, replacement) in n.profile.tables[..=index].iter().zip(replacements) {
                let _ = unsafe {
                    patch_slot(
                        base + table.rva + table.slot * 8,
                        replacement,
                        base + table.target,
                    )
                };
            }
            return Err(e);
        }
    }
    surfaces::attach(n)?;
    ENABLED.store(!STOP.load(Ordering::Acquire), Ordering::Release);
    Ok(())
}

fn log(root: &Path, text: &str) {
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(root.join("iidxbg.log"))
    {
        let _ = writeln!(f, "{text}");
    }
}

fn worker(root: PathBuf, rx: mpsc::Receiver<Event>) {
    let _ = WORKER.set(thread::current());
    unsafe {
        SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL);
    }
    let run = || -> Result<()> {
        let config_path = root.join("iidxbg.toml");
        // Identify the exact loaded build in every log; this makes a stale
        // deployment immediately visible when diagnosing in-game reports.
        if let Ok(path) = module_path(MODULE.load(Ordering::Acquire))
            && let Ok(bytes) = std::fs::read(&path)
        {
            log(
                &root,
                &format!(
                    "iidxbg build SHA-256 {}",
                    hex::encode(Sha256::digest(&bytes))
                ),
            );
        }
        let mut document = Config::load(&config_path)?;
        let config = document.for_user(None);
        HIDE_FRAME_CHARA.store(config.hide_frame_chara(), Ordering::Release);
        let mut character = crate::character::LocalImage::default();
        let mut providers = Providers::new(config)?;
        PREMIUM_AREA_ENABLED.store(providers.enabled(Surface::PremiumArea), Ordering::Release);
        CONCENTRATION_ENABLED.store(providers.enabled(Surface::Concentration), Ordering::Release);
        let mut images: [Option<crate::media::Asset>; Surface::COUNT] = Default::default();
        // Keep lifecycle hooks active even while configured off, so subsequent
        // visits can request a reload and re-enable without reinstalling hooks.
        attach(&root)?;
        session::start(config_path.clone())?;
        let mut context = 0;
        let mut selected_user: Option<String> = None;
        log(
            &root,
            &format!(
                "Attached {}. Results and optional surface hooks installed; new surfaces await in-game validation.",
                NATIVE.get().unwrap().profile.label
            ),
        );
        let mut completed = 0;
        let mut configured = 0;
        let mut next_attempt = Instant::now();
        while !STOP.load(Ordering::Acquire) {
            for message in rx.try_iter().take(64) {
                match message {
                    Event::Applied {
                        width,
                        height,
                        upload_us,
                    } => log(
                        &root,
                        &format!("Texture ready: {width}x{height}; upload {upload_us} us"),
                    ),
                    Event::Stock => log(&root, "Using stock artwork this results visit"),
                    Event::Centered { x, y } => log(
                        &root,
                        &format!("Custom illustration origin set to ({x}, {y}) texture pixels"),
                    ),
                    Event::Fault(text) => log(&root, text),
                    Event::Invalidated => log(&root, "Graphics resources invalidated"),
                    Event::Surface(name) => log(&root, &format!("Custom {name} artwork ready")),
                    Event::Diag(text) => log(&root, &text),
                }
            }
            let generation = REQUEST.load(Ordering::Acquire);
            let new_event = generation != configured;
            if new_event {
                match Config::load(&config_path) {
                    Ok(config) => document = config,
                    Err(error) => log(&root, &format!("Configuration reload rejected: {error}")),
                }
                let (mut current, login) = session::snapshot();
                let user = document
                    .user_for_session(&login.cards, &login.joined)
                    .map(str::to_owned);
                if current == context && user != selected_user {
                    let Some(rebound) = session::rebind(current) else {
                        request_images();
                        continue;
                    };
                    current = rebound;
                }
                let changed = current != context;
                if changed {
                    images = Default::default();
                    let _ = character.mmd.refresh(&providers.config().mmd, &root, false);
                    character = crate::character::LocalImage::default();
                    *PENDING.lock().unwrap_or_else(|e| e.into_inner()) = Some(Pending::Disabled);
                    surfaces::publish(&images);
                    surfaces::publish_artwork(Vec::new());
                    surfaces::publish_character(None);
                    next_attempt = Instant::now();
                    context = current;
                    selected_user = user;
                }
                let effective = document.for_user(selected_user.as_deref());
                if let Err(error) = providers.configure(effective, !changed) {
                    // A different user may never inherit the previous user's provider.
                    if changed {
                        return Err(error);
                    }
                    log(&root, &format!("Configuration providers rejected: {error}"));
                }
                refresh_character(&root, &providers, &mut character);
                session::prepared(context);
                configured = generation;
            }
            if generation != completed && Instant::now() >= next_attempt {
                if !new_event {
                    // A failed/deferred image request gets a fresh snapshot on retry.
                    // Retry through the same complete configuration/session resolver.
                    request_images();
                    continue;
                }
                let mut failed = false;
                let config = providers.config().clone();
                surfaces::profile_artwork(config.profile_artwork);
                let roots: [Surface; Surface::COUNT] = std::array::from_fn(|i| {
                    config
                        .binding_root(Surface::ALL[i])
                        .expect("validated binding graph")
                });
                let mut needed = [false; Surface::COUNT];
                for surface in Surface::ALL {
                    if config.surface_enabled(surface) {
                        needed[roots[surface.index()].index()] = true;
                    }
                }
                // Direct artwork resolves its bindings through global providers;
                // it must not request a personal module's source here.
                // Prepare each binding root exactly once. Bound consumers receive
                // the same Arc, random choice, decoder and GPU cache identity.
                for root_surface in Surface::ALL {
                    if STOP.load(Ordering::Acquire) || generation != REQUEST.load(Ordering::Acquire)
                    {
                        break;
                    }
                    if needed[root_surface.index()] {
                        match providers.prepare_media(root_surface) {
                            Ok(asset) => {
                                log(
                                    &root,
                                    &format!("Prepared {}: {}", root_surface.name(), asset.label),
                                );
                                images[root_surface.index()] = Some(asset);
                            }
                            Err(e) => {
                                failed = true;
                                log(
                                    &root,
                                    &format!(
                                        "{}: {e}; retaining previous media",
                                        root_surface.name()
                                    ),
                                );
                            }
                        }
                        if let Err(e) = providers.cleanup(root_surface) {
                            log(&root, &format!("{} cleanup: {e}", root_surface.name()));
                        }
                    } else {
                        images[root_surface.index()] = None;
                    }
                    if generation == REQUEST.load(Ordering::Acquire)
                        && !STOP.load(Ordering::Acquire)
                    {
                        let choices: [Option<crate::media::Asset>; Surface::COUNT] =
                            std::array::from_fn(|i| {
                                config
                                    .surface_enabled(Surface::ALL[i])
                                    .then(|| images[roots[i].index()].clone())
                                    .flatten()
                            });
                        *PENDING.lock().unwrap_or_else(|e| e.into_inner()) = match &choices[0] {
                            Some(crate::media::Asset::Image(p)) => Some(Pending::Image(p.clone())),
                            Some(crate::media::Asset::Video(v)) => Some(Pending::Video(v.clone())),
                            None if !config.surface_enabled(Surface::Results) => {
                                Some(Pending::Disabled)
                            }
                            // Preparation failure is not an explicit disable.
                            None => None,
                        };
                        surfaces::publish(&choices);
                    }
                }
                let (artwork, errors) = providers.prepare_artwork(&images);
                failed |= !errors.is_empty();
                for error in errors {
                    log(&root, &error);
                }
                if generation == REQUEST.load(Ordering::Acquire) && !STOP.load(Ordering::Acquire) {
                    surfaces::publish_artwork(artwork);
                }
                let superseded = generation != REQUEST.load(Ordering::Acquire);
                if !failed && !superseded {
                    completed = generation;
                }
                next_attempt = Instant::now()
                    + Duration::from_secs(if failed {
                        60
                    } else if superseded {
                        // Process the newer request immediately.
                        0
                    } else {
                        30
                    });
            }
            // Sleep until a hook/log/session observer/shutdown wakes us. Only
            // an outstanding preparation request schedules a worker timeout.
            if REQUEST.load(Ordering::Acquire) != completed {
                thread::park_timeout(next_attempt.saturating_duration_since(Instant::now()));
            } else {
                thread::park();
            }
        }
        Ok(())
    };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(run)) {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            ENABLED.store(false, Ordering::Release);
            log(&root, &format!("Startup failed: {e}"));
        }
        Err(_) => {
            ENABLED.store(false, Ordering::Release);
            log(&root, "Worker panic; feature disabled");
        }
    }
}

/// Called only for a scene request or its deferred/retry preparation.
#[cfg(test)]
fn reload_request(
    root: &Path,
    path: &Path,
    providers: &mut Providers,
    character: &mut crate::character::LocalImage,
) {
    match providers.reload(path) {
        Ok(true) => log(root, "Configuration reloaded for scene image request"),
        Ok(false) => {}
        Err(e) => log(
            root,
            &format!("Configuration reload rejected: {e}; keeping last valid settings"),
        ),
    }
    refresh_character(root, providers, character);
}

fn refresh_character(
    root: &Path,
    providers: &Providers,
    character: &mut crate::character::LocalImage,
) {
    let config = providers.config();
    HIDE_FRAME_CHARA.store(config.hide_frame_chara(), Ordering::Release);
    PREMIUM_AREA_ENABLED.store(providers.enabled(Surface::PremiumArea), Ordering::Release);
    CONCENTRATION_ENABLED.store(providers.enabled(Surface::Concentration), Ordering::Release);
    MMD_QPRO.store(config.uses_mmd_qpro(), Ordering::Release);
    if MMD_FRAME_CHARA.swap(config.uses_mmd_character(), Ordering::AcqRel)
        != config.uses_mmd_character()
    {
        surfaces::character_changed();
    }
    let model_supported = NATIVE
        .get()
        .is_some_and(|n| n.profile.rvas.contains_key("reaction_grade"));
    match character
        .mmd
        .refresh(&config.mmd, root, config.uses_mmd() && model_supported)
    {
        Ok(true) => {
            surfaces::character_changed();
            if let Some(model) = crate::mmd::snapshot() {
                log(
                    root,
                    &format!(
                        "MMD prepared: {} ({} vertices)",
                        model.first.label,
                        model.parsed.geometry.positions.len() / 3
                    ),
                );
                for warning in &model.warnings {
                    log(root, &format!("MMD: {warning}"));
                }
            }
        }
        Ok(false) => {}
        Err(e) => log(
            root,
            &format!("MMD import rejected: {e}; retaining previous character"),
        ),
    }
    match character.refresh(config, root) {
        Ok(crate::character::Update::Unchanged) => {}
        Ok(crate::character::Update::Native) => surfaces::publish_character(None),
        Ok(crate::character::Update::Image(prepared)) => {
            let prepared = if NATIVE
                .get()
                .is_some_and(|n| n.profile.rvas.contains_key("artwork_dispatch"))
            {
                prepared
            } else {
                crate::pixels::character_canvas(prepared)
            };
            log(
                root,
                &format!("Prepared selection frame character: {}", prepared.label),
            );
            surfaces::publish_character(Some(Arc::new(prepared)));
        }
        Err(e) => log(
            root,
            &format!("Frame character image rejected: {e}; retaining previous artwork"),
        ),
    }
}

#[repr(C)]
struct Frame {
    size: u32,
    device: *mut c_void,
    window: *mut c_void,
    width: u32,
    height: u32,
}
type Register =
    unsafe extern "C" fn(extern "C" fn(u32, *const Frame, *mut c_void), *mut c_void) -> i32;

extern "C" fn graphics(event_id: u32, _: *const Frame, _: *mut c_void) {
    boundary(|| {
        if event_id == 0 {
            OWNER.store(unsafe { GetCurrentThreadId() }, Ordering::Release);
            READY.store(true, Ordering::Release);
        } else if event_id == 1 {
            session::graphics();
        } else if event_id == 2 || event_id == 3 {
            READY.store(false, Ordering::Release);
            if let Some(n) = NATIVE.get() {
                if let Some(mut state) = graphics_state() {
                    unsafe {
                        state.invalidate(n);
                    }
                    surfaces::invalidate();
                    // A device can be destroyed/recreated during a result visit.
                    // Keep the CPU selection frozen; scene cleanup/destruction,
                    // not the graphics device, owns the visit boundary.
                } else {
                    ENABLED.store(false, Ordering::Release);
                    event(Event::Fault("graphics invalidation reentry"));
                }
            }
            event(Event::Invalidated);
        }
    });
}

extern "C" fn shutdown() {
    crate::mmd::input::configure(0, 0);
    crate::mmd::INTERACTIVE.store(false, Ordering::Release);
    crate::mmd::ACTIVE.store(false, Ordering::Release);
    crate::mmd::SELECTED_BPM.store(0, Ordering::Release);
    ENABLED.store(false, Ordering::Release);
    STOP.store(true, Ordering::Release);
    if let Some(state) = graphics_state()
        && let Some(video) = &state.selection.video
    {
        video.stop();
    }
    wake_worker();
    // SDK has already sent DESTROY and stopped rendering. DLL is pinned:
    // vtable callbacks remain valid and simply call their original functions.
}

#[repr(C)]
struct Sdk {
    size: u32,
    functions: [usize; 18],
}
type Init = unsafe extern "C" fn(u32, extern "C" fn(), *mut c_void) -> i32;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn spice_sdk_entry_point(init: Option<Init>) -> i32 {
    let Some(init) = init else {
        return 1;
    };
    if STARTED.swap(true, Ordering::AcqRel) {
        return 1;
    }
    let mut sdk = Sdk {
        size: std::mem::size_of::<Sdk>() as u32,
        functions: [0; 18],
    };
    let code = unsafe { init(0, shutdown, (&mut sdk as *mut Sdk).cast()) };
    if code != 0 {
        return code;
    }
    let Ok(path) = module_path(MODULE.load(Ordering::Acquire)) else {
        return 1;
    };
    let root = path.parent().unwrap().to_owned();
    if sdk.functions[17] == 0 {
        log(
            &root,
            "Spice SDK v0.4 register_d3d9 is required; no hooks installed",
        );
        return 1;
    }
    let mut pinned = std::ptr::null_mut();
    if unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
            spice_sdk_entry_point as *const () as *const u16,
            &mut pinned,
        )
    } == 0
    {
        return 1;
    }
    let register: Register = unsafe { std::mem::transmute(sdk.functions[17]) };
    if unsafe { register(graphics, std::ptr::null_mut()) } != 0 {
        log(
            &root,
            "Spice graphics registration failed; no hooks installed",
        );
        return 1;
    }
    let (tx, rx) = mpsc::sync_channel(64);
    let _ = EVENTS.set(tx);
    // SPICE_SDK_V0: get_analog slot 5, get_button slot 3 (v0.1 ABI).
    crate::mmd::input::configure(sdk.functions[5], sdk.functions[3]);
    match thread::Builder::new()
        .name("iidxbg-images".into())
        .spawn(move || worker(root, rx))
    {
        Ok(_) => 0,
        Err(_) => 1,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllMain(module: *mut c_void, reason: u32, _: *mut c_void) -> i32 {
    if reason == 1 {
        MODULE.store(module as usize, Ordering::Release);
    }
    if reason == 0 {
        STOP.store(true, Ordering::Release);
    }
    1
}

#[cfg(test)]
mod selection_reload_tests {
    use super::*;

    #[test]
    fn scene_request_reloads_character_settings_and_retains_invalid_edits() {
        let root = std::env::temp_dir().join(format!("iidxbg-event-{:016x}", fastrand::u64(..)));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("iidxbg.toml");
        let text = include_str!("../tests/fixtures/default.toml");
        std::fs::write(&path, text).unwrap();
        let mut providers = Providers::new(Config::load(&path).unwrap()).unwrap();
        let mut character = crate::character::LocalImage::default();
        reload_request(&root, &path, &mut providers, &mut character);
        assert!(!HIDE_FRAME_CHARA.load(Ordering::Acquire));
        let hidden = text.replace("frame_chara = true", "frame_chara = false");
        std::fs::write(&path, &hidden).unwrap();
        // Saving alone has no effect; the next hook request consumes the edit.
        assert!(!HIDE_FRAME_CHARA.load(Ordering::Acquire));
        reload_request(&root, &path, &mut providers, &mut character);
        assert!(HIDE_FRAME_CHARA.load(Ordering::Acquire));
        std::fs::write(&path, "enabled = [").unwrap();
        reload_request(&root, &path, &mut providers, &mut character);
        assert!(HIDE_FRAME_CHARA.load(Ordering::Acquire));
        std::fs::write(&path, hidden.replace("enabled = true", "enabled = false")).unwrap();
        reload_request(&root, &path, &mut providers, &mut character);
        assert!(!HIDE_FRAME_CHARA.load(Ordering::Acquire));
        std::fs::write(&path, text).unwrap();
        reload_request(&root, &path, &mut providers, &mut character);
        assert!(!HIDE_FRAME_CHARA.load(Ordering::Acquire));
        std::fs::remove_file(path).unwrap();
        std::fs::remove_file(root.join("iidxbg.log")).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
