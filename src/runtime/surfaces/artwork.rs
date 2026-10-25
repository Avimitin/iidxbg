//! Exact-build texture draw adapters. Evidence: docs/iidx34-custom-art.md.
//! No archive mutation: the original draw owns geometry, animation and ordering.
use super::*;
use std::cell::RefCell;

mod loose;
mod scenes;
use crate::artwork_shader as shader;
mod shape;

pub(super) fn library_hook(n: &Native) -> Option<(&'static str, usize, *const ())> {
    shape::hook(n)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Target {
    Notes,
    Shell,
    Decision,
    Transition,
    Character,
    Custom(usize),
}

#[derive(Clone)]
struct Tile {
    owner: usize,
    texture: u32,
    uv: [f32; 4], // left, right, top, bottom
    size: [f32; 2],
    target: Target,
    native_mask: bool,
}

#[derive(Default)]
struct State {
    movie: MovieState,
    tiles: Vec<Tile>,
    media: [Option<Arc<GpuMedia>>; 4],
    character: Option<Arc<GpuMedia>>,
    atlases: Vec<Atlas>,
    cached_custom: Vec<Tile>,
    custom_revision: u64,
}
static ART: Mutex<State> = Mutex::new(State {
    movie: MovieState {
        native: 0,
        choice: None,
        media: None,
    },
    tiles: Vec::new(),
    media: [None, None, None, None],
    character: None,
    atlases: Vec::new(),
    cached_custom: Vec::new(),
    custom_revision: 0,
});

thread_local! { static SHADER: RefCell<Option<IDirect3DPixelShader9>> = const { RefCell::new(None) }; }

static REVISION: AtomicU64 = AtomicU64::new(1);
static APPLIED_REVISION: AtomicU64 = AtomicU64::new(0);
static DYNAMIC: AtomicBool = AtomicBool::new(false);
pub(super) fn changed() {
    REVISION.fetch_add(1, Ordering::Release);
}

pub(super) fn hooks(n: &Native, hooks: &mut Vec<(&'static str, *const ())>) {
    if n.profile.rvas.contains_key("artwork_dispatch") {
        hooks.extend([
            ("artwork_login_setup", scenes::login as *const ()),
            ("select_frame_build", scenes::frame_build as *const ()),
            ("artwork_logout_setup", scenes::logout as *const ()),
            ("artwork_layer_create", layer_create as *const ()),
            ("artwork_archive_load", archive_load as *const ()),
            ("artwork_archive_open", archive_open as *const ()),
            ("artwork_archive_release", archive_release as *const ()),
            ("artwork_frame", frame as *const ()),
            ("release_texture", release_texture as *const ()),
            ("artwork_movie_texture", movie_texture as *const ()),
        ]);
        loose::hooks(hooks);
    }
}

pub(super) fn invalidate() {
    scenes::invalidate();
    loose::invalidate();
    shape::invalidate();
    changed();
    SHADER.with(|shader| shader.replace(None));
    if let Ok(mut state) = ART.try_lock() {
        unsafe { detach_all(&mut state) };
        // Native layer mappings remain valid across a device reset.
        state.media = Default::default();
        state.character = None;
        state.movie.media = None;
    }
}

pub(super) fn clear_session() -> bool {
    if !scenes::clear_session() {
        return false;
    }
    if let Ok(mut state) = ART.try_lock() {
        // Invalidation may have skipped this lock while a native metadata
        // callback held it. Do not acknowledge the new context until detached.
        unsafe { detach_all(&mut state) };
        state.media = Default::default();
        state.character = None;
        state.movie.media = None;
        state.movie.choice = None;
        state.cached_custom.clear();
        state.custom_revision = 0;
    } else {
        return false;
    }
    if let Ok(mut state) = GENERIC.try_lock() {
        state.frozen = None;
    } else {
        return false;
    }
    changed();
    true
}

pub(super) fn released(owner: usize) {
    scenes::released(owner);
    if let Ok(mut state) = ART.try_lock() {
        if !state.tiles.iter().any(|t| t.owner == owner) {
            return;
        }
        changed();
        // Metadata only: native layer destruction need not run on the graphics owner.
        state.tiles.retain(|tile| tile.owner != owner);
    }
}

type LayerCreate = unsafe extern "system" fn(usize, usize, *const u8, u32, u32, u32) -> usize;
unsafe extern "system" fn layer_create(
    manager: usize,
    archive: usize,
    name: *const u8,
    priority: u32,
    auto: u32,
    group: u32,
) -> usize {
    let original: LayerCreate = unsafe { original("artwork_layer_create") };
    let layer = unsafe { original(manager, archive, name, priority, auto, group) };
    boundary(|| {
        if layer == 0 || archive == 0 || name.is_null() {
            return;
        }
        unsafe { observe_archive(archive) };
        let name = unsafe { std::ffi::CStr::from_ptr(name.cast()) }.to_bytes();
        unsafe { scenes::created(layer, name, priority) };
        let mut names: Vec<(&str, Target)> = Vec::new();
        match name {
            b"center_frame_sp"
            | b"center_frame_2pp"
            | b"center_frame_dp_1p"
            | b"center_frame_dp_2p"
            | b"1p_frame"
            | b"2p_frame"
            | b"dp_frame_1p"
            | b"dp_frame_2p" => {
                names.extend(
                    ["center_frame_sp", "center_frame_2pp", "unit_body_base"]
                        .map(|name| (name, Target::Shell)),
                );
            }
            b"music_decide_1p" | b"music_decide_1p2p" => {
                names.push(("34decide_bg", Target::Decision));
                names.extend(
                    [
                        "34decide_zinrai_01",
                        "34decide_zinrai_01b",
                        "34decide_zinrai_02",
                        "34decide_zinrai_03",
                        "34decide_zinrai_04",
                        "34decide_zinrai_06",
                        "34decide_zinrai_08",
                        "34decide_zinrai_09",
                        "34decide_zinrai_11",
                        "34decide_zinrai_14",
                    ]
                    .map(|name| (name, Target::Transition)),
                );
            }
            b"frame_chara_01" | b"frame_chara_02" | b"frame_chara_03" => {
                names.extend(
                    [
                        "frame_chara_01",
                        "frame_chara_02",
                        "frame_chara_03",
                        "frame_chara_01_eff_noise",
                        "frame_chara_02_eff_noise",
                        "frame_chara_03_eff_noise",
                    ]
                    .map(|name| (name, Target::Character)),
                );
            }
            _ => return,
        }
        let Some(n) = NATIVE.get() else {
            return;
        };
        let info: unsafe extern "system" fn(*mut u8, u32, *const u8) -> i32 = unsafe {
            std::mem::transmute(field(
                n.base + n.profile.surface_data["artwork_texture_info"],
                0,
            ))
        };
        let archive_id = unsafe { ((archive + 1300) as *const u32).read_unaligned() };
        let mut found = Vec::new();
        for (name, target) in names {
            let mut bytes = [0u8; 56];
            let mut terminated = name.as_bytes().to_vec();
            terminated.push(0);
            if unsafe { info(bytes.as_mut_ptr(), archive_id, terminated.as_ptr()) } != 0 {
                continue;
            }
            let word = |offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
            let uv = [28, 32, 36, 40].map(|offset| f32::from_bits(word(offset)));
            if word(4) == 0 || !uv.iter().all(|v| v.is_finite()) || uv[1] <= uv[0] || uv[3] <= uv[2]
            {
                continue;
            }
            found.push(Tile {
                owner: layer,
                texture: word(4),
                uv,
                size: [word(20) as f32, word(24) as f32],
                target,
                native_mask: target != Target::Character,
            });
        }
        if let Ok(mut state) = ART.try_lock() {
            state.tiles.retain(|tile| tile.owner != layer);
            if state.tiles.len() + found.len() <= 256 {
                event(Event::Diag(format!(
                    "artwork layer={} tiles={}",
                    String::from_utf8_lossy(name),
                    found.len()
                )));
                state.tiles.extend(found);
                changed();
            }
        }
    });
    layer
}

fn media_slot(target: Target) -> Option<usize> {
    match target {
        Target::Notes => Some(0),
        Target::Shell => Some(1),
        Target::Decision => Some(2),
        Target::Transition => Some(3),
        Target::Character | Target::Custom(_) => None,
    }
}

unsafe fn acquire(
    n: &Native,
    state: &mut State,
    target: Target,
    generic: &GenericAssets,
) -> Option<Arc<GpuMedia>> {
    if let Target::Custom(index) = target {
        let asset = generic.get(index)?.1.as_ref()?;
        let media = unsafe { GpuMedia::acquire(n, asset) }.ok()?;
        unsafe { media.update() }.ok()?;
        return Some(media);
    }
    if target == Target::Character
        && !HIDE_FRAME_CHARA.load(Ordering::Acquire)
        && MMD_FRAME_CHARA.load(Ordering::Acquire)
        && crate::mmd::ACTIVE.load(Ordering::Acquire)
        && let Some(model) = crate::mmd::snapshot()
    {
        let media = unsafe { GpuMedia::acquire_model(n, model) }.ok()?;
        unsafe { media.update() }.ok()?;
        state.character = Some(media.clone());
        return Some(media);
    }
    if target == Target::Character && state.character.as_ref().is_some_and(|m| m.model.is_some()) {
        state.character = None;
    }
    let (asset, slot) = if let Some(index) = media_slot(target) {
        (snapshot()?.artwork[index].clone()?, &mut state.media[index])
    } else {
        if HIDE_FRAME_CHARA.load(Ordering::Acquire) {
            return None;
        }
        let pixels = CHARACTER_IMAGE.try_lock().ok()?.clone()?;
        (Asset::Image(pixels), &mut state.character)
    };
    let identity = asset.first();
    if !slot
        .as_ref()
        .is_some_and(|media| Arc::ptr_eq(&media.identity, &identity))
    {
        *slot = unsafe { GpuMedia::acquire(n, &asset) }.ok();
        if slot.is_some() {
            event(Event::Surface(match target {
                Target::Notes => "note skin",
                Target::Shell => "gameplay shell",
                Target::Decision => "decision",
                Target::Transition => "transition",
                Target::Character => "native character",
                Target::Custom(_) => "named artwork",
            }));
        }
    }
    let media = slot.clone()?;
    unsafe { media.update() }.ok()?;
    Some(media)
}

/// Crop coordinates for arbitrary media; transparent characters use contain.
fn crop(source: [f32; 2], target: [f32; 2], contain: bool) -> [f32; 4] {
    let ratio = (target[0] / target[1]) / (source[0] / source[1]);
    let [x, y] = if (ratio < 1.) != contain {
        [ratio, 1.]
    } else {
        [1., 1. / ratio]
    };
    [x, y, (1. - x) * 0.5, (1. - y) * 0.5]
}

/// Place the complete transparent canvas inside the native character tile.
/// Reserve the upper HUD and bottom controls; never change native draw order.
fn model_crop(source: [f32; 2], target: [f32; 2], placement: [f32; 3]) -> [f32; 4] {
    let fit = crop(source, target, true);
    let scale = (placement[0] / 100.).min(0.96 * fit[0]).min(0.77 * fit[1]);
    let sx = fit[0] / scale;
    let sy = fit[1] / scale;
    let cx = (placement[1] / 100.)
        .max(0.02 + 0.5 / sx)
        .min(0.98 - 0.5 / sx);
    let cy = (placement[2] / 100.)
        .max(0.13 + 0.5 / sy)
        .min(0.90 - 0.5 / sy);
    [sx, sy, 0.5 - sx * cx, 0.5 - sy * cy]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn model_placement_preserves_aspect_and_reserves_hud_space() {
        for source in [[512., 768.], [1024., 128.], [128., 1024.]] {
            for target in [[842., 1080.], [998., 1076.]] {
                for placement in [
                    [42., 20., 32.],
                    [100., 0., 0.],
                    [100., 100., 100.],
                    [10., 50., 50.],
                ] {
                    let [sx, sy, ox, oy] = model_crop(source, target, placement);
                    let bounds = [-ox / sx, (1. - ox) / sx, -oy / sy, (1. - oy) / sy];
                    assert!(bounds[0] >= 0.02 - 1e-6 && bounds[1] <= 0.98 + 1e-6);
                    assert!(bounds[2] >= 0.13 - 1e-6 && bounds[3] <= 0.90 + 1e-6);
                    let aspect = (target[0] / sx) / (target[1] / sy);
                    assert!((aspect - source[0] / source[1]).abs() < 1e-5);
                }
            }
        }
        let [sx, sy, ox, oy] = model_crop([512., 768.], [842., 1080.], [42., 20., 32.]);
        assert!(((0.5 - ox) / sx - 0.20).abs() < 1e-6);
        // Upward placement stops at the HUD margin, including animation travel.
        assert!((-oy / sy - 0.13).abs() < 1e-6);
    }
    #[test]
    fn native_archive_paths_match_catalog_keys() {
        for path in [
            "/data/graphic//0/basic_frame.ifs",
            "data/graphic/0/basic_frame.ifs",
            "/0/basic_frame.ifs",
            "0\\basic_frame.ifs",
        ] {
            assert_eq!(archive_key(path), "0/basic_frame.ifs");
        }
        assert_eq!(archive_key("/data/graphic/gmframe34.ifs"), "gmframe34.ifs");
        assert_eq!(
            mounted_archive_key("0/basic_frame.ifs"),
            "0/basic_frame.ifs"
        );
        assert_eq!(
            mounted_archive_key("basic_frame.ifs"),
            "@mounted/basic_frame.ifs"
        );
    }
    #[test]
    fn crop_preserves_aspect_and_centres() {
        assert_eq!(
            crop([1920., 1080.], [40., 15.], false),
            [1., 2. / 3., 0., (1. - 2. / 3.) / 2.]
        );
        let contain = crop([400., 1000.], [842., 1080.], true);
        assert!(contain[0] > 1. && contain[2] < 0.);
        assert_eq!(contain[1], 1.);
    }
}
// The native table owns a transferred COM reference; this owner keeps another.
struct Atlas {
    id: u32,
    original: usize,
    texture: IDirect3DTexture9,
    applied: Vec<(TileKey, Arc<Prepared>)>,
    media: Vec<Arc<GpuMedia>>,
}
// ART serializes metadata access. Only frame/invalidate on the SDK graphics
// owner creates, uses or drops atlases; native free only queues integer IDs.
unsafe impl Send for Atlas {}
#[derive(Clone, PartialEq)]
struct TileKey {
    revision: u64,
    uv: [f32; 4],
    target: Target,
    native_mask: bool,
}
type CompositeTile = (Tile, Arc<GpuMedia>, Arc<Prepared>);
type TextureGet = unsafe extern "system" fn(u32) -> *mut c_void;
type TextureReplace = unsafe extern "system" fn(u32, *mut c_void) -> *mut c_void;

unsafe fn detach(n: &Native, atlas: &Atlas) {
    let effective: TextureGet = unsafe { n.function("effective_texture") };
    if unsafe { effective(atlas.id) } == atlas.texture.as_raw() {
        let replace: TextureReplace = unsafe { n.function("replace_texture") };
        let previous = unsafe { replace(atlas.id, std::ptr::null_mut()) };
        if !previous.is_null() {
            drop(unsafe { IDirect3DTexture9::from_raw(previous) });
        }
    }
}
unsafe fn drain_retired(state: &mut State) {
    let mut retired = RETIRED.lock().unwrap_or_else(|e| e.into_inner());
    for id in retired.drain(..) {
        // The native free already released the table's reference.
        state.atlases.retain(|atlas| atlas.id != id);
    }
}
unsafe fn detach_all(state: &mut State) {
    unsafe { drain_retired(state) };
    if let Some(n) = NATIVE.get() {
        for atlas in state.atlases.drain(..) {
            unsafe { detach(n, &atlas) };
        }
    }
}
unsafe extern "system" fn release_texture(id: u32) -> usize {
    // This observer performs no COM operation, even if native unloading occurs
    // off the graphics thread. The graphics owner drains the retirement queue.
    if let Ok(mut retired) = RETIRED.lock() {
        retired.push(id);
        changed();
    }
    let original: unsafe extern "system" fn(u32) -> usize = unsafe { original("release_texture") };
    unsafe { original(id) }
}

unsafe extern "system" fn frame() -> usize {
    let started = PROFILE.load(Ordering::Relaxed).then(Instant::now);
    boundary(|| {
        if !can_render() {
            return;
        }
        unsafe { scenes::update() };
        super::tick_qpro();
        let revision = REVISION.load(Ordering::Acquire);
        if revision == APPLIED_REVISION.load(Ordering::Relaxed) && !DYNAMIC.load(Ordering::Relaxed)
        {
            return;
        }
        let Some(n) = NATIVE.get() else {
            return;
        };
        let Ok(mut state) = ART.try_lock() else {
            return;
        };
        unsafe { drain_retired(&mut state) };
        let convert: unsafe extern "system" fn(u32) -> u32 = unsafe {
            std::mem::transmute(field(
                n.base + n.profile.surface_data["artwork_native_id"],
                0,
            ))
        };
        let stock: TextureGet = unsafe { n.function("native_texture") };
        let effective: TextureGet = unsafe { n.function("effective_texture") };
        let mut groups: std::collections::BTreeMap<u32, Vec<CompositeTile>> = Default::default();
        let mut candidates = state.tiles.clone();
        let Ok(generic) = GENERIC.try_lock().map(|a| {
            a.frozen
                .as_ref()
                .map(|(_, assets)| assets)
                .unwrap_or(&a.pending)
                .clone()
        }) else {
            return;
        };
        let Some(shape_dynamic) = (unsafe { shape::update(n, &generic, revision) }) else {
            return;
        };
        let Some(loose_dynamic) = (unsafe { loose::update(n, &generic) }) else {
            return;
        };
        if state.custom_revision != revision {
            let Some(custom) = (unsafe { custom_tiles(n, &generic) }) else {
                return;
            };
            state.cached_custom = custom;
            state.custom_revision = revision;
        }
        let custom = state.cached_custom.clone();
        candidates.retain(|tile| {
            !custom
                .iter()
                .any(|c| c.texture == tile.texture && c.uv == tile.uv)
        });
        candidates.extend(custom);
        for (index, target) in [
            Target::Notes,
            Target::Shell,
            Target::Decision,
            Target::Transition,
        ]
        .into_iter()
        .enumerate()
        {
            if !candidates.iter().any(|t| t.target == target) {
                state.media[index] = None;
            }
        }
        if !candidates.iter().any(|t| t.target == Target::Character) {
            state.character = None;
        }
        for tile in candidates {
            let Some(media) = (unsafe { acquire(n, &mut state, tile.target, &generic) }) else {
                continue;
            };
            let pixels = {
                let Ok(playback) = media.playback.try_lock() else {
                    continue;
                };
                playback.pixels.clone()
            };
            let id = unsafe { convert(tile.texture) };
            if id == 0 {
                continue;
            }
            let group = groups.entry(id).or_default();
            if !group.iter().any(|(old, _, _)| old.uv == tile.uv) {
                group.push((tile, media, pixels));
            }
        }
        let mut kept = Vec::new();
        for atlas in state.atlases.drain(..) {
            if groups.contains_key(&atlas.id) {
                kept.push(atlas);
            } else {
                unsafe { detach(n, &atlas) };
            }
        }
        state.atlases = kept;
        for (id, tiles) in groups {
            let original = unsafe { stock(id) };
            if original.is_null() {
                continue;
            }
            let keys: Vec<_> = tiles
                .iter()
                .map(|(t, m, p)| {
                    (
                        TileKey {
                            revision: m.revision.load(Ordering::Relaxed),
                            uv: t.uv,
                            target: t.target,
                            native_mask: t.native_mask,
                        },
                        p.clone(),
                    )
                })
                .collect();
            let index = state
                .atlases
                .iter()
                .position(|a| a.id == id && a.original == original as usize);
            if let Some(index) = index {
                let atlas = &state.atlases[index];
                if atlas.applied.len() == keys.len()
                    && atlas
                        .applied
                        .iter()
                        .zip(&keys)
                        .all(|((a, p), (b, q))| a == b && Arc::ptr_eq(p, q))
                {
                    continue;
                }
                if unsafe { effective(id) } != atlas.texture.as_raw() {
                    continue;
                }
            } else if unsafe { effective(id) } != original {
                continue;
            } // another plugin owns it
            let start = Instant::now();
            let Ok(texture) = (unsafe {
                compose(
                    n,
                    original,
                    index.map(|i| &state.atlases[i].texture),
                    &tiles,
                )
            }) else {
                // An in-place video composition may have failed after writing
                // some tiles. Never leave that partial atlas installed.
                if let Some(index) = index {
                    let atlas = state.atlases.remove(index);
                    unsafe { detach(n, &atlas) };
                }
                continue;
            };
            if let Some(index) = index {
                state.atlases[index].applied = keys;
                state.atlases[index].media = tiles.iter().map(|(_, m, _)| m.clone()).collect();
            } else {
                let replace: TextureReplace = unsafe { n.function("replace_texture") };
                let transferred = texture.clone().into_raw();
                let previous = unsafe { replace(id, transferred) };
                // The table was checked above on this graphics owner.
                if !previous.is_null() {
                    let returned = unsafe { replace(id, previous) };
                    if !returned.is_null() {
                        drop(unsafe { IDirect3DTexture9::from_raw(returned) });
                    }
                    continue;
                }
                event(Event::Diag(format!(
                    "artwork atlas id={id} tiles={} prepare_us={}",
                    tiles.len(),
                    start.elapsed().as_micros()
                )));
                state.atlases.push(Atlas {
                    id,
                    original: original as usize,
                    texture,
                    applied: keys,
                    media: tiles.iter().map(|(_, m, _)| m.clone()).collect(),
                });
            }
        }
        DYNAMIC.store(
            state
                .atlases
                .iter()
                .any(|a| a.media.iter().any(|m| m.dynamic))
                || shape_dynamic
                || loose_dynamic,
            Ordering::Relaxed,
        );
        APPLIED_REVISION.store(revision, Ordering::Relaxed);
    });
    if let Some(started) = started {
        profile_sample(started);
    }
    let original: unsafe extern "system" fn() -> usize = unsafe { original("artwork_frame") };
    unsafe { original() }
}

// State blocks omit render targets/depth surfaces. Save and restore those
// explicitly, including MRT1 used by the cabinet's second display.
unsafe fn compose(
    n: &Native,
    original: *mut c_void,
    existing: Option<&IDirect3DTexture9>,
    tiles: &[CompositeTile],
) -> windows::core::Result<IDirect3DTexture9> {
    let get: Device = unsafe { n.function("get_device") };
    let raw = unsafe { get() };
    let device = unsafe { IDirect3DDevice9::from_raw_borrowed(&raw) }.ok_or_else(|| {
        windows::core::Error::from_hresult(windows::core::HRESULT(0x80004005u32 as i32))
    })?;
    let source = unsafe { IDirect3DTexture9::from_raw_borrowed(&original) }.ok_or_else(|| {
        windows::core::Error::from_hresult(windows::core::HRESULT(0x80004005u32 as i32))
    })?;
    let mut desc = D3DSURFACE_DESC::default();
    unsafe {
        source.GetLevelDesc(0, &mut desc)?;
    }
    let texture = if let Some(existing) = existing {
        existing.clone()
    } else {
        let mut texture = None;
        unsafe {
            device.CreateTexture(
                desc.Width,
                desc.Height,
                1,
                D3DUSAGE_RENDERTARGET as u32,
                D3DFMT_A8R8G8B8,
                D3DPOOL_DEFAULT,
                &mut texture,
                std::ptr::null_mut(),
            )?;
        }
        texture.ok_or_else(|| {
            windows::core::Error::from_hresult(windows::core::HRESULT(0x80004005u32 as i32))
        })?
    };
    let saved = unsafe { device.CreateStateBlock(D3DSBT_ALL)? };
    let rt0 = unsafe { device.GetRenderTarget(0)? };
    let rt1 = unsafe { device.GetRenderTarget(1) }.ok();
    let depth = unsafe { device.GetDepthStencilSurface() }.ok();
    let mut viewport = D3DVIEWPORT9::default();
    unsafe {
        device.GetViewport(&mut viewport)?;
    }
    let target = unsafe { texture.GetSurfaceLevel(0)? };
    let shader = SHADER.with(|slot| -> windows::core::Result<_> {
        let mut slot = slot.borrow_mut();
        if slot.is_none() {
            *slot = Some(unsafe { device.CreatePixelShader(shader::CODE.as_ptr())? });
        }
        Ok(slot.as_ref().unwrap().clone())
    })?;
    let result = unsafe {
        (|| -> windows::core::Result<()> {
            device.SetRenderTarget(1, None)?;
            device.SetDepthStencilSurface(None)?;
            device.SetRenderTarget(0, &target)?;
            device.SetViewport(&D3DVIEWPORT9 {
                Width: desc.Width,
                Height: desc.Height,
                MinZ: 0.,
                MaxZ: 1.,
                ..Default::default()
            })?;
            device.SetVertexShader(None)?;
            device.SetFVF(0x144)?;
            for (kind, value) in [
                (D3DRS_ZENABLE, 0),
                (D3DRS_ZWRITEENABLE, 0),
                (D3DRS_CULLMODE, 1),
                (D3DRS_ALPHATESTENABLE, 0),
                (D3DRS_ALPHABLENDENABLE, 0),
                (D3DRS_SCISSORTESTENABLE, 0),
                (D3DRS_COLORWRITEENABLE, 15),
                (D3DRS_FOGENABLE, 0),
                (D3DRS_STENCILENABLE, 0),
                (D3DRS_SRGBWRITEENABLE, 0),
            ] {
                device.SetRenderState(kind, value)?;
            }
            device.SetTexture(0, source)?;
            for stage in [0, 1] {
                for (kind, value) in [
                    (D3DSAMP_ADDRESSU, 3),
                    (D3DSAMP_ADDRESSV, 3),
                    (D3DSAMP_MINFILTER, 1),
                    (D3DSAMP_MAGFILTER, 1),
                    (D3DSAMP_MIPFILTER, 0),
                    (D3DSAMP_SRGBTEXTURE, 0),
                ] {
                    device.SetSamplerState(stage, kind, value)?;
                }
            }
            // First copy the complete stock atlas, retaining every unconfigured tile.
            device.SetPixelShader(None)?;
            for (kind, value) in [
                (D3DTSS_COLOROP, 2),
                (D3DTSS_COLORARG1, D3DTA_TEXTURE),
                (D3DTSS_ALPHAOP, 2),
                (D3DTSS_ALPHAARG1, D3DTA_TEXTURE),
                (D3DTSS_TEXCOORDINDEX, 0),
                (D3DTSS_TEXTURETRANSFORMFLAGS, 0),
            ] {
                device.SetTextureStageState(0, kind, value)?;
            }
            device.SetTextureStageState(1, D3DTSS_COLOROP, 1)?;
            quad(device, [0., 1., 0., 1.], desc.Width, desc.Height)?;
            device.SetPixelShader(&shader)?;
            device.SetPixelShaderConstantF(0, [0.; 4].as_ptr(), 1)?;
            for (tile, media, _) in tiles {
                let source = &media.texture;
                let character = tile.target == Target::Character;
                let mut crop = crop(
                    [source.width as f32, source.height as f32],
                    tile.size,
                    character,
                );
                // Native character layers extend behind the top/bottom HUD.
                // Inset the transparent cutout while retaining the native motion.
                if character && let Some(placement) = media.model_placement {
                    if media.model_keep_hud_clear {
                        crop = model_crop(
                            [source.width as f32, source.height as f32],
                            tile.size,
                            placement,
                        );
                    } else {
                        crop[0] /= placement[0] / 100.;
                        crop[1] /= placement[0] / 100.;
                        crop[2] = 0.5 - crop[0] * placement[1] / 100.;
                        crop[3] = 0.5 - crop[1] * placement[2] / 100.;
                    }
                } else if character {
                    crop[0] /= 0.82;
                    crop[1] /= 0.82;
                    crop[2] = (1. - crop[0]) * 0.5;
                    crop[3] = (1. - crop[1]) * 0.5;
                }
                let constants = [
                    tile.uv[0],
                    tile.uv[2],
                    1. / (tile.uv[1] - tile.uv[0]),
                    1. / (tile.uv[3] - tile.uv[2]),
                    crop[0],
                    crop[1],
                    crop[2],
                    crop[3],
                    if tile.native_mask { 1. } else { 0. },
                    if character { 1. } else { 0. },
                    if media.model.is_some() { 1. } else { 0. },
                    0.,
                ];
                let raw = source.wrapper[1] as *mut c_void;
                let media_texture = IDirect3DBaseTexture9::from_raw_borrowed(&raw).unwrap();
                device.SetTexture(1, media_texture)?;
                device.SetSamplerState(1, D3DSAMP_MINFILTER, 2)?;
                device.SetSamplerState(1, D3DSAMP_MAGFILTER, 2)?;
                device.SetPixelShaderConstantF(8, constants.as_ptr(), 3)?;
                quad(device, tile.uv, desc.Width, desc.Height)?;
            }
            Ok(())
        })()
    };
    let restored = unsafe {
        (|| -> windows::core::Result<()> {
            device.SetRenderTarget(0, &rt0)?;
            device.SetRenderTarget(1, rt1.as_ref())?;
            device.SetDepthStencilSurface(depth.as_ref())?;
            saved.Apply()?;
            device.SetViewport(&viewport)
        })()
    };
    if restored.is_err() {
        ENABLED.store(false, Ordering::Release);
        event(Event::Fault("artwork atlas graphics restoration failed"));
    }
    result.and(restored)?;
    Ok(texture)
}
unsafe fn quad(
    device: &IDirect3DDevice9,
    uv: [f32; 4],
    width: u32,
    height: u32,
) -> windows::core::Result<()> {
    let [l, r, t, b] = uv;
    let vertices = [(l, t), (r, t), (r, b), (l, b)].map(|(u, v)| Vertex {
        x: u * width as f32 - 0.5,
        y: v * height as f32 - 0.5,
        z: 0.,
        rhw: 1.,
        color: 0xffffffff,
        u,
        v,
    });
    unsafe { device.DrawPrimitiveUP(D3DPT_TRIANGLEFAN, 2, vertices.as_ptr().cast(), 28) }
}
type GenericAssets = Vec<(crate::config::Artwork, Option<Asset>)>;
#[derive(Default)]
struct MovieState {
    native: usize,
    choice: Option<Asset>,
    media: Option<Arc<GpuMedia>>,
}
thread_local! { static MOVIE_TEXTURE: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

/// The movie wrapper uses a direct COM getter, outside the AFP native-ID table.
/// Scope substitution to the original movie draw and keep its geometry/state.
pub(super) unsafe fn draw_movie(this: usize, original: Unary) -> usize {
    let started = PROFILE.load(Ordering::Relaxed).then(Instant::now);
    let mut lease = None;
    boundary(|| {
        if !can_render() {
            return;
        }
        let Some(n) = NATIVE.get() else {
            return;
        };
        if !n.profile.rvas.contains_key("artwork_movie_texture") {
            return;
        }
        let Ok(mut state) = ART.try_lock() else {
            return;
        };
        let native = unsafe { field(this, 0) };
        if state.movie.native != native {
            let Ok(generic) = GENERIC.try_lock() else {
                return;
            };
            let assets = generic
                .frozen
                .as_ref()
                .map(|(_, a)| a)
                .unwrap_or(&generic.pending);
            state.movie = MovieState {
                native,
                choice: assets
                    .iter()
                    .find(|(a, _)| a.enabled && a.archive == "@movie" && a.texture == "main")
                    .and_then(|(_, a)| a.clone()),
                media: None,
            };
        }
        if native == 0
            || unsafe { *((this + 16) as *const u8) == 0 || *((this + 36) as *const u8) == 0 }
        {
            state.movie.media = None;
            return;
        }
        if state.movie.media.is_none()
            && let Some(asset) = &state.movie.choice
        {
            state.movie.media = unsafe { GpuMedia::acquire(n, asset) }.ok();
        }
        if let Some(media) = &state.movie.media
            && unsafe { media.update() }.is_ok()
        {
            lease = Some(media.clone());
        }
    });
    let pointer = lease.as_ref().map_or(0, |m| m.texture.wrapper[1] as usize);
    let previous = MOVIE_TEXTURE.with(|slot| slot.replace(pointer));
    if let Some(started) = started {
        profile_extra(0, started);
    }
    let result = unsafe { original(this) };
    MOVIE_TEXTURE.with(|slot| slot.set(previous));
    result
}

unsafe extern "system" fn movie_texture(
    wrapper: usize,
    output: *mut *mut c_void,
) -> *mut *mut c_void {
    let original: unsafe extern "system" fn(usize, *mut *mut c_void) -> *mut *mut c_void =
        unsafe { original("artwork_movie_texture") };
    let result = unsafe { original(wrapper, output) };
    MOVIE_TEXTURE.with(|slot| {
        let loose = if slot.get() == 0 && can_render() {
            unsafe { loose::texture(*((wrapper + 16) as *const u32)) }
        } else {
            None
        };
        let raw = loose
            .as_ref()
            .map_or(slot.get() as *mut c_void, |t| t.as_raw());
        if raw.is_null() || output.is_null() {
            return;
        }
        // The original getter returns one owned COM reference. Replace it with
        // one owned reference; its caller retains the normal Release contract.
        if let Some(texture) = unsafe { IDirect3DTexture9::from_raw_borrowed(&raw) } {
            let old = unsafe { output.replace(texture.clone().into_raw()) };
            if !old.is_null() {
                drop(unsafe { IDirect3DTexture9::from_raw(old) });
            }
        }
    });
    result
}

struct GenericState {
    pending: Arc<GenericAssets>,
    frozen: Option<(usize, Arc<GenericAssets>)>,
}
static GENERIC: std::sync::LazyLock<Mutex<GenericState>> = std::sync::LazyLock::new(|| {
    Mutex::new(GenericState {
        pending: Arc::new(Vec::new()),
        frozen: None,
    })
});
static ARCHIVES: Mutex<Vec<Archive>> = Mutex::new(Vec::new());
static RETIRED: Mutex<Vec<u32>> = Mutex::new(Vec::new());
struct Archive {
    object: usize,
    id: u32,
    path: String,
}
fn archive_key(path: &str) -> String {
    let path = path.replace('\\', "/");
    path.strip_prefix("/data/graphic/")
        .or_else(|| path.strip_prefix("data/graphic/"))
        .unwrap_or(&path)
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}
pub(super) fn publish(assets: GenericAssets) {
    GENERIC.lock().unwrap_or_else(|e| e.into_inner()).pending = Arc::new(assets);
    changed();
}
pub(super) fn result_visit(owner: usize, enter: bool) {
    let mut state = GENERIC.lock().unwrap_or_else(|e| e.into_inner());
    if enter && state.frozen.is_none() {
        state.frozen = Some((owner, state.pending.clone()));
        changed();
    } else if !enter && state.frozen.as_ref().is_some_and(|(id, _)| *id == owner) {
        state.frozen = None;
        changed();
    }
}
// Mounted internal jobs retain their relative graphic path in the logical name.
// External jobs strip it to a basename; do not guess a catalog path for those.
fn mounted_archive_key(name: &str) -> String {
    let name = archive_key(name);
    if name.contains('/') && !name.split('/').any(|part| part == "..") {
        name
    } else {
        format!("@mounted/{name}")
    }
}

type ArchiveLoad = unsafe extern "system" fn(usize, i32, *const u8, *const u8, i32) -> u8;
unsafe extern "system" fn archive_open(
    object: usize,
    index: i32,
    base: *const u8,
    name: *const u8,
    flags: i32,
    ram: u8,
) -> u8 {
    let original: unsafe extern "system" fn(usize, i32, *const u8, *const u8, i32, u8) -> u8 =
        unsafe { original("artwork_archive_open") };
    let result = unsafe { original(object, index, base, name, flags, ram) };
    if result != 0 {
        boundary(|| unsafe { observe_archive(object) });
    }
    result
}
unsafe extern "system" fn archive_load(
    object: usize,
    index: i32,
    base: *const u8,
    name: *const u8,
    flags: i32,
) -> u8 {
    let original: ArchiveLoad = unsafe { original("artwork_archive_load") };
    let result = unsafe { original(object, index, base, name, flags) };
    boundary(|| {
        if result == 0 {
            return;
        }
        unsafe { observe_archive(object) };
    });
    result
}

unsafe fn observe_archive(object: usize) {
    if unsafe { ((object + 1296) as *const u32).read_unaligned() } != 2 {
        return;
    }
    // Alternate loaders reach the same archive object through layer creation.
    // Native loader stores a bounded, NUL-terminated full virtual path here.
    let bytes = unsafe { std::slice::from_raw_parts((object + 264) as *const u8, 256) };
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    let path = archive_key(&String::from_utf8_lossy(&bytes[..end]));
    let path = if path.is_empty() {
        let bytes = unsafe { std::slice::from_raw_parts((object + 8) as *const u8, 256) };
        let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
        mounted_archive_key(&String::from_utf8_lossy(&bytes[..end]))
    } else {
        path
    };
    let id = unsafe { ((object + 1300) as *const u32).read_unaligned() };
    if let Ok(mut archives) = ARCHIVES.lock() {
        // Retaining an existing resource changes no pixels/identity.
        if archives
            .iter()
            .any(|a| a.object == object && a.id == id && a.path == path)
        {
            return;
        }
        if PROFILE.load(Ordering::Relaxed) {
            event(Event::Diag(format!(
                "artwork archive path={path} id={id:#x}"
            )));
        }
        archives.retain(|a| a.object != object);
        archives.push(Archive { object, id, path });
        changed();
    }
}
unsafe extern "system" fn archive_release(object: usize, force: u8) -> usize {
    // A non-final reference release leaves the archive valid. Hold metadata's
    // lock through destruction to prevent concurrent AFP queries of freed IDs.
    let mut archives = ARCHIVES.lock().unwrap_or_else(|e| e.into_inner());
    let original: unsafe extern "system" fn(usize, u8) -> usize =
        unsafe { original("artwork_archive_release") };
    let result = unsafe { original(object, force) };
    if unsafe { ((object + 1296) as *const u32).read_unaligned() } != 2 {
        archives.retain(|a| a.object != object);
        changed();
    }
    result
}

unsafe fn custom_tiles(n: &Native, assets: &GenericAssets) -> Option<Vec<Tile>> {
    let archives = ARCHIVES.try_lock().ok()?;
    let info: unsafe extern "system" fn(*mut u8, u32, *const u8) -> i32 = unsafe {
        std::mem::transmute(field(
            n.base + n.profile.surface_data["artwork_texture_info"],
            0,
        ))
    };
    let mut tiles = Vec::new();
    if snapshot().is_some_and(|b| b.artwork[0].is_some()) {
        for archive in archives.iter().filter(|a| {
            a.path
                .rsplit('/')
                .next()
                .is_some_and(|p| p.starts_with("notes") && p.ends_with(".ifs"))
        }) {
            for name in [
                c"notes_auto_turn",
                c"notes_black",
                c"notes_bss2_beam0",
                c"notes_bss2_beam1",
                c"notes_bss2_beam2",
                c"notes_bss2_beam3",
                c"notes_bss2_beam4",
                c"notes_bss2_end",
                c"notes_bss2_start",
                c"notes_bss_auto_beam0",
                c"notes_bss_auto_end",
                c"notes_bss_auto_joint",
                c"notes_bss_auto_start",
                c"notes_bss_beam0",
                c"notes_bss_beam1",
                c"notes_bss_beam2",
                c"notes_bss_end",
                c"notes_bss_start",
                c"notes_cn2_black_beam0",
                c"notes_cn2_black_beam1",
                c"notes_cn2_black_beam2",
                c"notes_cn2_black_beam3",
                c"notes_cn2_black_beam4",
                c"notes_cn2_black_end",
                c"notes_cn2_black_start",
                c"notes_cn2_white_beam0",
                c"notes_cn2_white_beam1",
                c"notes_cn2_white_beam2",
                c"notes_cn2_white_beam3",
                c"notes_cn2_white_beam4",
                c"notes_cn2_white_end",
                c"notes_cn2_white_start",
                c"notes_cn_black_beam0",
                c"notes_cn_black_beam1",
                c"notes_cn_black_beam2",
                c"notes_cn_black_end",
                c"notes_cn_black_start",
                c"notes_cn_white_beam0",
                c"notes_cn_white_beam1",
                c"notes_cn_white_beam2",
                c"notes_cn_white_end",
                c"notes_cn_white_start",
                c"notes_mss_end",
                c"notes_mss_middle",
                c"notes_mss_start",
                c"notes_turn",
                c"notes_white",
            ] {
                let mut bytes = [0u8; 56];
                if unsafe { info(bytes.as_mut_ptr(), archive.id, name.as_ptr().cast()) } != 0 {
                    continue;
                }
                let word = |o| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
                let uv = [28, 32, 36, 40].map(|o| f32::from_bits(word(o)));
                if word(4) != 0
                    && uv.iter().all(|v| v.is_finite())
                    && uv[1] > uv[0]
                    && uv[3] > uv[2]
                {
                    tiles.push(Tile {
                        owner: archive.object,
                        texture: word(4),
                        uv,
                        size: [word(20) as f32, word(24) as f32],
                        target: Target::Notes,
                        native_mask: true,
                    });
                }
            }
        }
    }
    for (index, (art, asset)) in assets.iter().enumerate() {
        if !art.enabled
            || !scenes::active(art)
            || asset.is_none()
            || art.texture.starts_with("@shape/")
        {
            continue;
        }
        for archive in archives.iter().filter(|a| a.path == art.archive) {
            let mut name = art.texture.as_bytes().to_vec();
            name.push(0);
            let mut bytes = [0u8; 56];
            if unsafe { info(bytes.as_mut_ptr(), archive.id, name.as_ptr()) } != 0 {
                continue;
            }
            let word = |o| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
            let uv = [28, 32, 36, 40].map(|o| f32::from_bits(word(o)));
            if word(4) == 0 || !uv.iter().all(|v| v.is_finite()) || uv[1] <= uv[0] || uv[3] <= uv[2]
            {
                continue;
            }
            let mut uv = uv;
            let mut size = [word(20) as f32, word(24) as f32];
            if art.bottom_crop {
                // Only the bottom 300 native pixels; retain the upper lane rails.
                let height = size[1].min(300.);
                uv[2] = uv[3] - (uv[3] - uv[2]) * height / size[1];
                size[1] = height;
            }
            tiles.push(Tile {
                owner: archive.object,
                texture: word(4),
                uv,
                size,
                target: Target::Custom(index),
                native_mask: art.native_mask,
            });
        }
    }
    Some(tiles)
}

pub(super) static PROFILE: AtomicBool = AtomicBool::new(false);
thread_local! {static TIMINGS: RefCell<Vec<u64>>=const {RefCell::new(Vec::new())};}
fn profile_sample(started: Instant) {
    let ns = started.elapsed().as_nanos() as u64;
    TIMINGS.with(|samples| {
        let mut samples=samples.borrow_mut();samples.push(ns);
        if samples.len()==1200 {
            samples.sort_unstable();
            event(Event::Diag(format!("artwork CPU callback samples=1200 mean_ns={} p50_ns={} p95_ns={} p99_ns={} max_ns={} dynamic={}",samples.iter().sum::<u64>()/1200,samples[600],samples[1140],samples[1188],samples[1199],DYNAMIC.load(Ordering::Relaxed))));
            samples.clear();
        }
    });
}

thread_local! {
    static EXTRA_TIMINGS: RefCell<[Vec<u64>; 3]> = const { RefCell::new([Vec::new(), Vec::new(), Vec::new()]) };
}
// CPU preparation only: excludes the forwarded native draw and GPU completion.
fn profile_extra(index: usize, started: Instant) {
    let elapsed = started.elapsed().as_nanos() as u64;
    EXTRA_TIMINGS.with(|all| {
        let mut all = all.borrow_mut();
        let samples = &mut all[index];
        samples.push(elapsed);
        if samples.len() == 1200 {
            samples.sort_unstable();
            event(Event::Diag(format!(
                "artwork {} CPU samples=1200 mean_ns={} p95_ns={} p99_ns={} max_ns={}",
                ["movie preparation", "shape dispatch", "loose matched bind"][index],
                samples.iter().sum::<u64>() / 1200,
                samples[1140],
                samples[1188],
                samples[1199]
            )));
            samples.clear();
        }
    });
}
