//! Shared verified ABI; native addresses/layouts come from exact-build profiles.
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
        PAGE_EXECUTE_READWRITE, PAGE_EXECUTE_WRITECOPY, PAGE_GUARD, PAGE_READWRITE, VirtualProtect,
        VirtualQuery,
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
static PREMIUM_AREA_ENABLED: AtomicBool = AtomicBool::new(false);
static CONCENTRATION_ENABLED: AtomicBool = AtomicBool::new(false);
static PENDING: Mutex<Option<Pending>> = Mutex::new(None);
static GPU: Mutex<Graphics> = Mutex::new(Graphics::new());
static NATIVE: OnceLock<Native> = OnceLock::new();
static EVENTS: OnceLock<SyncSender<Event>> = OnceLock::new();

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

struct Texture {
    // Stable 8-byte-aligned storage. Native constructor initializes all 64 bytes.
    wrapper: Box<[u64; 8]>,
    center: [f32; 2],
}

impl Texture {
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
        };
        if result.wrapper[1] == 0 || result.wrapper[2] as u32 == 0 {
            return Err(Error::Native("AFP texture registration"));
        }
        // Constructor owns a COM reference; local COM owner is now released.
        Ok(result)
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
    attempted: bool,
    failed: bool,
    layout_reported: bool,
}

impl Graphics {
    const fn new() -> Self {
        Self {
            selection: Selection {
                active: false,
                selected: None,
            },
            background: 0,
            texture: None,
            attempted: false,
            failed: false,
            layout_reported: false,
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
            let pending = PENDING.try_lock().ok().and_then(|mut p| p.take());
            self.selection.enter(pending);
            request_images();
            self.background = background;
            self.attempted = false;
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
        if self.texture.is_none() && !self.attempted {
            self.attempted = true;
            if let Some(image) = &self.selection.selected {
                let start = Instant::now();
                match unsafe { Texture::upload(n, image) } {
                    Ok(texture) => {
                        self.texture = Some(texture);
                        event(Event::Applied {
                            width: image.width,
                            height: image.height,
                            upload_us: start.elapsed().as_micros(),
                        });
                    }
                    Err(_) => {
                        event(Event::Fault(
                            "texture upload/registration failed; stock this visit",
                        ));
                    }
                }
            }
        }
        if let Some(texture) = &self.texture {
            let find: FindClip = unsafe { n.function("find_clip") };
            let clip = unsafe {
                find(
                    field(background, n.profile.layout.background_movie),
                    c"illust".as_ptr().cast(),
                )
            };
            if clip == 0 {
                return;
            }
            let set: SetTexture = unsafe { n.function("set_texture") };
            let visible: Visible = unsafe { n.function("set_visible") };
            if unsafe { set(clip, texture.descriptor()) } == 0 {
                unsafe {
                    self.stock(n);
                }
                self.failed = true;
                event(Event::Fault("AFP bind rejected; feature disabled"));
                return;
            }
            // The archive-only center helper uses archive metadata and is skipped
            // by the stock no-archive branch. Center the uploaded texture explicitly
            // after binding; keep the clip's native position/scale/animation.
            let origin: OriginPixels = unsafe { n.function("set_origin_pixels") };
            unsafe {
                origin(clip, texture.center[0], texture.center[1]);
                visible(clip, 1);
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
        self.attempted = false;
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
    let result = unsafe { original(this, tick) };
    if !ENABLED.load(Ordering::Acquire) || !READY.load(Ordering::Acquire) {
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
    result
}

unsafe extern "system" fn cleanup_hook(this: usize) -> usize {
    let n = NATIVE.get().expect("published before hook");
    retire_scene(n, this);
    let original: Cleanup = unsafe { n.function("result_cleanup") };
    unsafe { original(this) }
}

fn retire_scene(n: &Native, this: usize) {
    // Restore the stock binding while this scene still owns valid clips.
    boundary(|| {
        if let Some(mut state) = graphics_state()
            && state.background == this + n.profile.layout.scene_background
        {
            unsafe {
                state.invalidate(n);
            }
            state.background = 0;
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
        let config = Config::load(&config_path)?;
        HIDE_FRAME_CHARA.store(config.hide_frame_chara(), Ordering::Release);
        let mut character = crate::character::LocalImage::default();
        let mut providers = Providers::new(config)?;
        PREMIUM_AREA_ENABLED.store(providers.enabled(Surface::PremiumArea), Ordering::Release);
        CONCENTRATION_ENABLED.store(providers.enabled(Surface::Concentration), Ordering::Release);
        let mut images: [Option<Arc<Prepared>>; Surface::COUNT] = Default::default();
        // Keep lifecycle hooks active even while configured off, so subsequent
        // visits can request a reload and re-enable without reinstalling hooks.
        attach(&root)?;
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
                }
            }
            let generation = REQUEST.load(Ordering::Acquire);
            let new_event = generation != configured;
            if new_event {
                reload_request(&root, &config_path, &mut providers, &mut character);
                configured = generation;
            }
            if generation != completed && Instant::now() >= next_attempt {
                if !new_event {
                    // A failed/deferred image request gets a fresh snapshot on retry.
                    reload_request(&root, &config_path, &mut providers, &mut character);
                }
                let mut failed = false;
                // The concentration background reuses the premium-area image
                // until its own source section gains an override.
                let share = providers.config().concentration_shares_premium()
                    && providers.enabled(Surface::Concentration);
                for surface in Surface::ALL {
                    if STOP.load(Ordering::Acquire) {
                        break;
                    }
                    if share && surface == Surface::Concentration {
                        images[surface.index()] = images[Surface::PremiumArea.index()].clone();
                        continue;
                    }
                    // One accepted config snapshot governs this preparation batch.
                    // Maintenance failures do not discard a usable image or cause a redownload.
                    let cleanup = |providers: &Providers| match providers.cleanup(surface) {
                        Ok(0) => {}
                        Ok(count) => log(
                            &root,
                            &format!(
                                "{}: removed {count} oldest managed downloads",
                                surface.name()
                            ),
                        ),
                        Err(e) => log(
                            &root,
                            &format!("{}: download cleanup failed: {e}", surface.name()),
                        ),
                    };
                    cleanup(&providers);
                    // A concentration-only configuration still prepares one
                    // premium-area image to share.
                    let prepared = if share
                        && surface == Surface::PremiumArea
                        && !providers.enabled(surface)
                    {
                        providers.prepare_unchecked(surface).map(Some)
                    } else {
                        providers.prepare(surface)
                    };
                    match prepared {
                        Ok(image) => {
                            if let Some(image) = &image {
                                log(
                                    &root,
                                    &format!("Prepared {}: {}", surface.name(), image.label),
                                );
                            }
                            images[surface.index()] = image.map(Arc::new);
                        }
                        Err(e) => {
                            failed = true;
                            log(
                                &root,
                                &format!(
                                    "{} preparation failed: {e}; retaining its previous image, retry in 60 seconds",
                                    surface.name()
                                ),
                            );
                        }
                    }
                    cleanup(&providers);
                    // Publish each completed query so a local result does not wait
                    // behind downloads for other surfaces. Active visits stay frozen.
                    for item in Surface::ALL {
                        let needed =
                            providers.enabled(item) || (share && item == Surface::PremiumArea);
                        if !needed {
                            images[item.index()] = None;
                        }
                    }
                    if share {
                        images[Surface::Concentration.index()] =
                            images[Surface::PremiumArea.index()].clone();
                    }
                    if generation == REQUEST.load(Ordering::Acquire)
                        && !STOP.load(Ordering::Acquire)
                    {
                        let pending = images[0]
                            .as_ref()
                            .map(|p| Pending::Image(p.clone()))
                            .unwrap_or(Pending::Disabled);
                        *PENDING.lock().unwrap_or_else(|e| e.into_inner()) = Some(pending);
                        surfaces::publish(&images);
                    }
                }
                if !failed && generation == REQUEST.load(Ordering::Acquire) {
                    completed = generation;
                }
                next_attempt = Instant::now() + Duration::from_secs(if failed { 60 } else { 30 });
            }
            // Sleep until a hook/log/shutdown wakes us. Only an outstanding
            // preparation request schedules a timeout; there is no file watcher.
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
    let config = providers.config();
    HIDE_FRAME_CHARA.store(config.hide_frame_chara(), Ordering::Release);
    PREMIUM_AREA_ENABLED.store(providers.enabled(Surface::PremiumArea), Ordering::Release);
    CONCENTRATION_ENABLED.store(providers.enabled(Surface::Concentration), Ordering::Release);
    match character.refresh(config, root) {
        Ok(crate::character::Update::Unchanged) => {}
        Ok(crate::character::Update::Native) => surfaces::publish_character(None),
        Ok(crate::character::Update::Image(prepared)) => {
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
    ENABLED.store(false, Ordering::Release);
    STOP.store(true, Ordering::Release);
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
        let text = include_str!("../iidxbg.toml");
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
