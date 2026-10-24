//! Loose PNG/JPEG loader and direct-handle binding. Exact-build evidence is in
//! docs/iidx34-custom-art.md. Loader/free hooks only publish integer metadata.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone)]
struct Loaded {
    path: String,
    serial: u64,
    original: usize,
    size: [f32; 2],
}
struct Replacement {
    serial: u64,
    pixels: Arc<Prepared>,
    media: Arc<GpuMedia>,
    texture: IDirect3DTexture9,
    mask: bool,
}
// Only graphics-owner update/invalidate/bind touches GPU references.
unsafe impl Send for Replacement {}
static LOADED: Mutex<BTreeMap<u32, Loaded>> = Mutex::new(BTreeMap::new());
static GPU: Mutex<BTreeMap<u32, Replacement>> = Mutex::new(BTreeMap::new());
static SERIAL: AtomicU64 = AtomicU64::new(1);
static ACTIVE: AtomicBool = AtomicBool::new(false);

pub(super) fn hooks(hooks: &mut Vec<(&'static str, *const ())>) {
    hooks.extend([
        ("artwork_file_load", load as *const ()),
        ("artwork_direct_release", release as *const ()),
        ("artwork_direct_bind", bind as *const ()),
    ]);
}

pub(super) fn invalidate() {
    ACTIVE.store(false, Ordering::Release);
    GPU.lock().unwrap_or_else(|e| e.into_inner()).clear();
}

unsafe extern "system" fn load(loader: usize, output: *mut usize, path: *const u8) -> usize {
    let original: unsafe extern "system" fn(usize, *mut usize, *const u8) -> usize =
        unsafe { original("artwork_file_load") };
    let result = unsafe { original(loader, output, path) };
    boundary(|| {
        if output.is_null() || path.is_null() {
            return;
        }
        let wrapper = unsafe { *output };
        if wrapper == 0 {
            return;
        }
        let name = unsafe { std::ffi::CStr::from_ptr(path.cast()) }.to_string_lossy();
        let Some(path) = file_key(&name) else {
            return;
        };
        let id = unsafe { *((wrapper + 16) as *const u32) };
        let original = unsafe { field(wrapper, 8) };
        let size = [28, 32].map(|offset| unsafe { *((wrapper + offset) as *const u32) as f32 });
        if id == 0 || original == 0 || size.iter().any(|v| *v <= 0. || *v > 16384.) {
            return;
        }
        if let Ok(mut loaded) = LOADED.lock() {
            loaded.insert(
                id,
                Loaded {
                    path: path.clone(),
                    original,
                    size,
                    serial: SERIAL.fetch_add(1, Ordering::Relaxed),
                },
            );
            changed();
            if PROFILE.load(Ordering::Relaxed) {
                event(Event::Diag(format!(
                    "artwork loose path={path} id={id} size={size:?}"
                )));
            }
        }
    });
    result
}

fn file_key(path: &str) -> Option<String> {
    let normalized = path.replace('\\', "/");
    let relative = normalized.strip_prefix("/data/graphic/")?;
    if relative.is_empty()
        || relative
            .split('/')
            .any(|s| s.is_empty() || s == ".." || s == ".")
    {
        return None;
    }
    Some(relative.to_owned())
}

unsafe extern "system" fn release(id: u32) -> usize {
    // Remove before the wrapper releases its COM reference; the graphics owner
    // can acquire an owned reference under this same lock while it is live.
    if LOADED
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&id)
        .is_some()
    {
        changed();
    }
    let original: unsafe extern "system" fn(u32) -> usize =
        unsafe { original("artwork_direct_release") };
    unsafe { original(id) }
}

pub(super) unsafe fn update(n: &Native, assets: &GenericAssets) -> Option<bool> {
    let loaded = LOADED.try_lock().ok()?.clone();
    let mut gpu = GPU.try_lock().ok()?;
    let mut keep = BTreeSet::new();
    for (index, (art, asset)) in assets.iter().enumerate() {
        if !art.enabled || art.archive != "@file" {
            continue;
        }
        let Some(asset) = asset else {
            continue;
        };
        for (&id, item) in loaded.iter().filter(|(_, item)| item.path == art.texture) {
            let Ok(media) = (unsafe { GpuMedia::acquire(n, asset) }) else {
                continue;
            };
            if unsafe { media.update() }.is_err() {
                continue;
            }
            let pixels = media.playback.try_lock().ok()?.pixels.clone();
            if gpu.get(&id).is_some_and(|old| {
                old.serial == item.serial
                    && old.mask == art.native_mask
                    && Arc::ptr_eq(&old.pixels, &pixels)
            }) {
                keep.insert(id);
                continue;
            }
            let original = {
                let current = LOADED.try_lock().ok()?;
                if !current.get(&id).is_some_and(|v| v.serial == item.serial) {
                    continue;
                }
                let raw = item.original as *mut c_void;
                // Reference acquisition only on graphics owner, serialized with
                // unregister. No borrowed native pointer survives this scope.
                unsafe { IDirect3DTexture9::from_raw_borrowed(&raw) }?.clone()
            };
            let tiles = [(
                Tile {
                    owner: 0,
                    texture: id,
                    uv: [0., 1., 0., 1.],
                    size: item.size,
                    target: Target::Custom(index),
                    native_mask: art.native_mask,
                },
                media.clone(),
                pixels.clone(),
            )];
            let existing = gpu
                .get(&id)
                .filter(|v| v.serial == item.serial)
                .map(|v| &v.texture);
            let Ok(texture) = (unsafe { compose(n, original.as_raw(), existing, &tiles) }) else {
                gpu.remove(&id); // Failed in-place composition must fall back.
                continue;
            };
            gpu.insert(
                id,
                Replacement {
                    serial: item.serial,
                    pixels,
                    media,
                    texture,
                    mask: art.native_mask,
                },
            );
            keep.insert(id);
        }
    }
    gpu.retain(|id, _| keep.contains(id));
    ACTIVE.store(!gpu.is_empty(), Ordering::Release);
    Some(gpu.values().any(|v| v.media.dynamic))
}

pub(super) unsafe fn texture(id: u32) -> Option<IDirect3DTexture9> {
    if !ACTIVE.load(Ordering::Acquire) || !can_render() {
        return None;
    }
    let gpu = GPU.try_lock().ok()?;
    let replacement = gpu.get(&id)?;
    let loaded = LOADED.try_lock().ok()?;
    (loaded.get(&id)?.serial == replacement.serial).then(|| replacement.texture.clone())
}

unsafe extern "system" fn bind(renderer: usize, id: u32, parameters: usize) -> usize {
    let original: unsafe extern "system" fn(usize, u32, usize) -> usize =
        unsafe { original("artwork_direct_bind") };
    let result = unsafe { original(renderer, id, parameters) };
    if !ACTIVE.load(Ordering::Acquire) {
        return result;
    }
    boundary(|| {
        let started = PROFILE.load(Ordering::Relaxed).then(Instant::now);
        let Some(texture) = (unsafe { texture(id) }) else {
            return;
        };
        let raw = unsafe { field(renderer, 98304) } as *mut c_void;
        if let Some(device) = unsafe { IDirect3DDevice9::from_raw_borrowed(&raw) } {
            let _ = unsafe { device.SetTexture(0, &texture) };
        }
        if let Some(started) = started {
            profile_extra(2, started);
        }
    });
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn loose_paths_are_scoped_to_graphic_resources() {
        assert_eq!(
            file_key("/data/graphic/assistant_chara/assistant_chara000.png").as_deref(),
            Some("assistant_chara/assistant_chara000.png")
        );
        for path in [
            "/data/font/a.png",
            "a.png",
            "/data/graphic/../font/a.png",
            "/data/graphic/",
        ] {
            assert_eq!(file_key(path), None);
        }
    }
}
