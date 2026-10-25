//! Exact-build surface hooks. See docs/*-background-surfaces.md for ABI evidence.
use super::*;
use crate::media::{Asset, Playback};
use minhook::MinHook;
mod artwork;
mod qpro;

#[derive(Default)]
pub(super) struct Bundle {
    select: Option<Asset>,
    upper: Option<Asset>,
    lower: Option<Asset>,
    graph: Option<Asset>,
    premium_area: Option<Asset>,
    concentration: Option<Asset>,
    artwork: [Option<Asset>; 4],
    scenes: [Option<Asset>; 3],
}

static LATEST: Mutex<Option<Arc<Bundle>>> = Mutex::new(None);
static CHARACTER_IMAGE: Mutex<Option<Arc<Prepared>>> = Mutex::new(None);
static STATE: Mutex<Surfaces> = Mutex::new(Surfaces::new());
static HOOKS: OnceLock<Vec<(&'static str, usize)>> = OnceLock::new();
/// Diagnostic rate limits. The selection round logs every 120 movie draws
/// (about once a second at 120 Hz); premium transform failures every 600
/// attempts so a broken premium path cannot fill the log.
static SELECT_DRAW_TICKS: AtomicU32 = AtomicU32::new(0);
static SELECT_PROBE_TICKS: AtomicU32 = AtomicU32::new(0);
static PREMIUM_FAULTS: AtomicU32 = AtomicU32::new(0);

/// Worker publishes already decoded, independently sourced surface canvases.
pub(super) fn publish(images: &[Option<Asset>; Surface::COUNT]) {
    artwork::changed();
    *LATEST.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(Bundle {
        select: images[Surface::SongSelect.index()].clone(),
        upper: images[Surface::SuddenPlus.index()].clone(),
        lower: images[Surface::Lift.index()].clone(),
        graph: images[Surface::TargetGraph.index()].clone(),
        premium_area: images[Surface::PremiumArea.index()].clone(),
        concentration: images[Surface::Concentration.index()].clone(),
        scenes: [Surface::Login, Surface::ModeSelect, Surface::Logout]
            .map(|s| images[s.index()].clone()),
        artwork: [
            Surface::NoteSkin,
            Surface::GameplayShell,
            Surface::Decision,
            Surface::Transition,
        ]
        .map(|surface| images[surface.index()].clone()),
    }));
}

pub(super) fn profile_artwork(enabled: bool) {
    artwork::PROFILE.store(enabled, Ordering::Relaxed);
}
pub(super) fn artwork_result_visit(owner: usize, enter: bool) {
    artwork::result_visit(owner, enter);
}

pub(super) fn publish_artwork(artwork: Vec<(crate::config::Artwork, Option<Asset>)>) {
    artwork::publish(artwork);
}

pub(super) fn publish_character(image: Option<Arc<Prepared>>) {
    artwork::changed();
    *CHARACTER_IMAGE.lock().unwrap_or_else(|e| e.into_inner()) = image;
}

fn snapshot() -> Option<Arc<Bundle>> {
    LATEST.try_lock().ok().and_then(|p| p.clone())
}

fn state() -> Option<MutexGuard<'static, Surfaces>> {
    match STATE.try_lock() {
        Ok(s) => Some(s),
        Err(TryLockError::Poisoned(e)) => Some(e.into_inner()),
        Err(TryLockError::WouldBlock) => None,
    }
}

fn can_render() -> bool {
    ENABLED.load(Ordering::Acquire)
        && READY.load(Ordering::Acquire)
        && unsafe { GetCurrentThreadId() } == OWNER.load(Ordering::Acquire)
}

unsafe fn original<T: Copy>(name: &str) -> T {
    let address = HOOKS
        .get()
        .expect("trampolines published")
        .iter()
        .find(|(n, _)| *n == name)
        .expect("known hook")
        .1;
    assert_eq!(std::mem::size_of::<T>(), std::mem::size_of::<usize>());
    unsafe { std::mem::transmute_copy(&address) }
}

type Unary = unsafe extern "system" fn(usize) -> usize;

// Exact 010-0917 primitive ABI: this+40 is {u32 count, u32 padding,
// pointer}. Native update copies all 16 bytes to AFP without initializing
// padding. A stale 0x10000 there is mistaken for a 48-byte shader descriptor.
// Initialize it before the synchronous native copy; retain the original call
// and return value. Do this even when background replacement is disabled.
// See docs/iidx34-crash-20261023.md for hashes, RVAs and lifetime evidence.
unsafe fn update_primitive(this: usize, update: Unary) -> usize {
    unsafe {
        if ((this + 8) as *const u32).read_unaligned() != 0 {
            ((this + 44) as *mut u32).write_unaligned(0);
        }
        update(this)
    }
}

unsafe extern "system" fn box_fill_update(this: usize) -> usize {
    unsafe { update_primitive(this, original("box_fill_update")) }
}

unsafe extern "system" fn box_outline_update(this: usize) -> usize {
    unsafe { update_primitive(this, original("box_outline_update")) }
}

unsafe extern "system" fn line_update(this: usize) -> usize {
    unsafe { update_primitive(this, original("line_update")) }
}

type Release = unsafe extern "system" fn(usize);
type SelectUpdate = unsafe extern "system" fn(usize, u8, usize) -> usize;
type CoverBuild = unsafe extern "system" fn(usize, usize, i32, u8, u8);
type Factory = unsafe extern "system" fn(usize, *const u8, u32) -> usize;
type Manager = unsafe extern "system" fn() -> usize;
type Scale = unsafe extern "system" fn(usize, f32, f32);
type Position = unsafe extern "system" fn(usize, i32, i32);
type Clip = unsafe extern "system" fn(usize, *const i32);
// Native virtual +160 forwards alpha, red, green, blue (last float on stack).
type Alpha = unsafe extern "system" fn(usize, f32, f32, f32, f32);

type PropertyGet = unsafe extern "system" fn(u32, u32, *mut u8) -> i32;
type PropertySet = unsafe extern "system" fn(u32, u32, usize) -> i32;

#[derive(Clone, Copy)]
struct HiddenFrameChara {
    scene: usize,
    layer: usize,
    clip: u32,
    visible: u8,
}

/// Last transform written to the character replacement sprite. Identical reads
/// skip the AFP setters; the native root is still read every update so native
/// animation continues to line up.
#[derive(Clone, Copy, PartialEq)]
struct FramePlacement {
    position: [f32; 2],
    origin: [f32; 2],
    scale: [f32; 2],
    color: [f32; 4],
    width: i32,
    height: i32,
}

/// Only the character's root clip is masked. Never cache an owning native pointer.
#[derive(Default)]
struct FrameChara {
    hidden: Option<HiddenFrameChara>,
    image: Option<Arc<Prepared>>,
    sprite: Option<Sprite>,
    layer: usize,
    attempted: bool,
    placed: Option<FramePlacement>,
}

impl FrameChara {
    unsafe fn current(n: &Native, scene: usize) -> Option<(usize, u32)> {
        if scene == 0
            || unsafe { field(scene, 0) } != n.base + n.profile.surface_data["select_vtable"]
        {
            return None;
        }
        let layer = unsafe { field(scene, n.profile.surface_data["select_frame_chara"]) };
        if layer == 0
            || unsafe { field(layer, 0) } != n.base + n.profile.surface_data["layer_vtable"]
            || unsafe { ((layer + 8) as *const u32).read() } == 0
        {
            return None;
        }
        let clip = unsafe { ((layer + 528) as *const u32).read() };
        ((clip as i32) >= 0).then_some((layer, clip))
    }

    unsafe fn set(n: &Native, clip: u32, visible: u8) -> bool {
        let set: PropertySet = unsafe {
            std::mem::transmute(field(
                n.base + n.profile.surface_data["afp_property_set"],
                0,
            ))
        };
        if unsafe { set(clip, 4103, usize::from(visible)) } < 0 {
            return false;
        }
        unsafe { set(clip, 4126, 1) };
        true
    }

    unsafe fn hide(&mut self, n: &Native, scene: usize) {
        // A missed restore must never overwrite the saved native visibility.
        if self.hidden.is_some() {
            return;
        }
        let Some((layer, clip)) = (unsafe { Self::current(n, scene) }) else {
            return;
        };
        let get: PropertyGet = unsafe {
            std::mem::transmute(field(
                n.base + n.profile.surface_data["afp_property_get"],
                0,
            ))
        };
        let mut visible = 0;
        if unsafe { get(clip, 4103, &mut visible) } >= 0 && unsafe { Self::set(n, clip, 0) } {
            self.hidden = Some(HiddenFrameChara {
                scene,
                layer,
                clip,
                visible,
            });
        }
    }

    unsafe fn restore(&mut self, n: &Native, scene: usize) {
        if let Some(sprite) = &self.sprite {
            unsafe { visible(sprite.node, false) };
        }
        let Some(saved) = self.hidden.take() else {
            return;
        };
        // Read only the currently supplied scene. Never dereference a stale owner.
        if saved.scene == scene
            && unsafe { Self::current(n, scene) } == Some((saved.layer, saved.clip))
        {
            unsafe { Self::set(n, saved.clip, saved.visible) };
        }
    }

    fn released(&mut self, layer: usize) -> Option<Arc<Texture>> {
        if self.hidden.is_some_and(|saved| saved.layer == layer) {
            self.hidden = None;
        }
        if self.layer == layer {
            self.layer = 0;
        }
        if let Some(sprite) = &mut self.sprite
            && sprite.node == layer
        {
            sprite.node = 0;
            self.attempted = false;
            self.placed = None;
            return sprite.texture.take();
        }
        None
    }

    unsafe fn update(&mut self, n: &Native, scene: usize, hide: bool) {
        if let Ok(image) = CHARACTER_IMAGE.try_lock() {
            let same = match (&self.image, &*image) {
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            };
            if !same {
                self.sprite = None;
                self.placed = None;
                self.image = image.clone();
                self.attempted = false;
            }
        }
        let Some((layer, clip)) = (unsafe { Self::current(n, scene) }) else {
            return;
        };
        if self.layer != layer {
            self.layer = layer;
            self.sprite = None;
            self.placed = None;
            self.attempted = false;
        }
        if hide {
            unsafe { self.hide(n, scene) };
            return;
        }
        // This build replaces the native character texture in its animated
        // geometry. The older root-canvas adapter must not hide that geometry.
        if n.profile.rvas.contains_key("artwork_dispatch") {
            return;
        }
        if !self.attempted
            && let Some(image) = &self.image
        {
            self.attempted = true;
            let priority = unsafe {
                ((scene + n.profile.surface_data["select_frame_priority"]) as *const u32).read()
            };
            match unsafe { Sprite::create(n, image, priority) } {
                Ok(sprite) => {
                    self.sprite = Some(sprite);
                    self.placed = None;
                    event(Event::Surface("frame character"));
                }
                Err(_) => event(Event::Fault(
                    "Frame character texture upload failed; using stock",
                )),
            }
        }
        if let Some(sprite) = &self.sprite {
            let node = sprite.node;
            if !unsafe { self.place_sprite(n, layer, clip, node) } {
                return;
            }
            unsafe { self.hide(n, scene) };
            if self.hidden.is_some_and(|saved| saved.visible != 0) {
                unsafe { visible(node, true) };
            }
        }
    }

    unsafe fn place_sprite(&mut self, n: &Native, layer: usize, clip: u32, sprite: usize) -> bool {
        let get: PropertyGet = unsafe {
            std::mem::transmute(field(
                n.base + n.profile.surface_data["afp_property_get"],
                0,
            ))
        };
        let mut position = [0f32; 2];
        let mut origin = [0f32; 2];
        let mut scale = [0f32; 2];
        let mut color = [0f32; 4];
        for (property, data) in [
            (4104, position.as_mut_slice()),
            (4123, origin.as_mut_slice()),
            (4099, scale.as_mut_slice()),
            (4106, color.as_mut_slice()),
        ] {
            if unsafe { get(clip, property, data.as_mut_ptr().cast()) } < 0
                || !data.iter().all(|v| v.is_finite() && v.abs() <= 1_000_000.0)
            {
                return false;
            }
        }
        let dimensions: unsafe extern "system" fn(usize, *mut i32, *mut i32) =
            unsafe { n.function("layer_dimensions") };
        let (mut width, mut height) = (0, 0);
        unsafe { dimensions(layer, &mut width, &mut height) };
        if !(1..=16384).contains(&width) || !(1..=16384).contains(&height) {
            return false;
        }
        let ratio = [width as f32 / 1920.0, height as f32 / 1080.0];
        let placement = FramePlacement {
            position,
            origin,
            scale,
            color,
            width,
            height,
        };
        if self.placed == Some(placement) {
            return true;
        }
        let set_origin: Position = unsafe { n.function("sprite_origin") };
        let set_position: Position = unsafe { n.function("sprite_position") };
        let set_scale: Scale = unsafe { n.function("sprite_scale") };
        let set_color: Alpha = unsafe { n.function("sprite_alpha") };
        unsafe {
            set_origin(
                sprite,
                (origin[0] / ratio[0]).round() as i32,
                (origin[1] / ratio[1]).round() as i32,
            );
            set_position(
                sprite,
                position[0].round() as i32,
                position[1].round() as i32,
            );
            set_scale(sprite, scale[0] * ratio[0], scale[1] * ratio[1]);
            set_color(sprite, color[3], color[0], color[1], color[2]);
        }
        self.placed = Some(placement);
        true
    }
}

unsafe fn visible(object: usize, enabled: bool) {
    if object != 0 {
        let f: Visible = unsafe { std::mem::transmute(field(field(object, 0), 40)) };
        unsafe { f(object, u8::from(enabled)) };
    }
}

struct Sprite {
    node: usize,
    texture: Option<Arc<Texture>>,
    media: Option<Arc<GpuMedia>>,
}

/// Weak cache owns no GPU objects. Only the graphics owner creates/uses/drops
/// these leases; bound surfaces upload once and use one native descriptor.
static MEDIA: Mutex<Vec<std::sync::Weak<GpuMedia>>> = Mutex::new(Vec::new());
struct GpuMedia {
    identity: Arc<Prepared>,
    dynamic: bool,
    texture: Arc<Texture>,
    playback: Mutex<Playback>,
    failed: AtomicBool,
    model: Option<Mutex<ModelGpu>>,
    model_placement: Option<[f32; 3]>,
    model_keep_hud_clear: bool,
    revision: AtomicU64,
}

struct ModelGpu {
    renderer: crate::mmd::render::Renderer,
    session: Arc<crate::mmd::Session>,
    straight: Option<Arc<Texture>>,
}
// Same owner invariant as Atlas: only graphics-owner frame/invalidate callbacks
// access or drop this object. Workers receive only owned CPU model/pose data.
unsafe impl Send for ModelGpu {}

impl GpuMedia {
    unsafe fn acquire(n: &Native, asset: &Asset) -> Result<Arc<Self>> {
        let identity = asset.first();
        let mut cache = MEDIA
            .try_lock()
            .map_err(|_| Error::Native("media cache busy"))?;
        cache.retain(|entry| entry.strong_count() != 0);
        if let Some(shared) = cache
            .iter()
            .filter_map(std::sync::Weak::upgrade)
            .find(|entry| Arc::ptr_eq(&entry.identity, &identity))
        {
            return Ok(shared);
        }
        let playback = asset.play();
        let texture = Arc::new(unsafe { Texture::upload(n, &playback.pixels) }?);
        let shared = Arc::new(Self {
            dynamic: matches!(asset, Asset::Video(_)),
            identity,
            texture,
            playback: Mutex::new(playback),
            failed: AtomicBool::new(false),
            model: None,
            model_placement: None,
            model_keep_hud_clear: true,
            revision: AtomicU64::new(0),
        });
        cache.push(Arc::downgrade(&shared));
        Ok(shared)
    }

    unsafe fn update(&self) -> Result<()> {
        if self.failed.load(Ordering::Relaxed) {
            return Err(Error::Native("shared texture unavailable until reset"));
        }
        if let Some(model) = &self.model {
            if !crate::mmd::ACTIVE.load(Ordering::Acquire) {
                return Ok(());
            }
            if let Ok(mut model) = model.try_lock() {
                if model.session.failed() {
                    self.failed.store(true, Ordering::Relaxed);
                    return Err(Error::Mmd("animation worker stopped".into()));
                }
                let Some(frame) = model.session.frame() else {
                    return Ok(());
                };
                let n = NATIVE.get().ok_or(Error::Native("model native owner"))?;
                let get: Device = unsafe { n.function("get_device") };
                let raw = unsafe { get() };
                let device = unsafe { IDirect3DDevice9::from_raw_borrowed(&raw) }
                    .ok_or(Error::Native("model device"))?;
                if let Err(e) = unsafe { model.renderer.draw(device, &frame) } {
                    self.failed.store(true, Ordering::Relaxed);
                    return Err(Error::Mmd(e.to_string()));
                }
                self.revision
                    .store(model.renderer.serial, Ordering::Relaxed);
                if model.straight.is_some() {
                    unsafe { model.renderer.straight(device) }
                        .map_err(|e| Error::Mmd(e.to_string()))?;
                }
            }
            return Ok(());
        }
        if let Ok(mut playback) = self.playback.try_lock()
            && playback.update()
            && let Err(error) = unsafe { self.texture.write_pixels(&playback.pixels) }
        {
            self.failed.store(true, Ordering::Relaxed);
            return Err(error);
        }
        Ok(())
    }

    unsafe fn acquire_model(n: &Native, model: Arc<crate::mmd::Model>) -> Result<Arc<Self>> {
        let mut cache = MEDIA
            .try_lock()
            .map_err(|_| Error::Native("media cache busy"))?;
        cache.retain(|entry| entry.strong_count() != 0);
        if let Some(shared) = cache
            .iter()
            .filter_map(std::sync::Weak::upgrade)
            .find(|m| Arc::ptr_eq(&m.identity, &model.first))
        {
            return Ok(shared);
        }
        let get: Device = unsafe { n.function("get_device") };
        let raw = unsafe { get() };
        let device = unsafe { IDirect3DDevice9::from_raw_borrowed(&raw) }
            .ok_or(Error::Native("model device"))?;
        let session = model.lease()?;
        let mut renderer = unsafe { crate::mmd::render::Renderer::new(device, model.clone()) }
            .map_err(|e| Error::Mmd(e.to_string()))?;
        let first = session.frame().ok_or(Error::Native("model first pose"))?;
        unsafe { renderer.draw(device, &first) }.map_err(|e| Error::Mmd(e.to_string()))?;
        let texture = Arc::new(unsafe {
            Texture::wrap(n, &renderer.target, model.first.width, model.first.height)
        }?);
        let shared = Arc::new(Self {
            identity: model.first.clone(),
            dynamic: true,
            texture,
            playback: Mutex::new(Asset::Image(model.first.clone()).play()),
            failed: AtomicBool::new(false),
            revision: AtomicU64::new(renderer.serial),
            model: Some(Mutex::new(ModelGpu {
                renderer,
                session,
                straight: None,
            })),
            model_placement: Some([
                model.settings.display_scale,
                model.settings.position_x,
                model.settings.position_y,
            ]),
            model_keep_hud_clear: model.settings.keep_hud_clear,
        });
        cache.push(Arc::downgrade(&shared));
        Ok(shared)
    }

    unsafe fn straight_model_texture(&self, n: &Native) -> Result<Arc<Texture>> {
        let mut model = self
            .model
            .as_ref()
            .ok_or(Error::Native("not a model"))?
            .try_lock()
            .map_err(|_| Error::Native("model busy"))?;
        let get: Device = unsafe { n.function("get_device") };
        let raw = unsafe { get() };
        let device = unsafe { IDirect3DDevice9::from_raw_borrowed(&raw) }
            .ok_or(Error::Native("model device"))?;
        let target =
            unsafe { model.renderer.straight(device) }.map_err(|e| Error::Mmd(e.to_string()))?;
        if model.straight.is_none() {
            model.straight = Some(Arc::new(unsafe {
                Texture::wrap(n, &target, self.identity.width, self.identity.height)
            }?));
        }
        Ok(model.straight.as_ref().unwrap().clone())
    }
}

pub(super) fn character_changed() {
    artwork::changed();
}

impl Sprite {
    unsafe fn create(n: &Native, image: &Prepared, priority: u32) -> Result<Self> {
        let texture = Arc::new(unsafe { Texture::upload(n, image) }?);
        unsafe { Self::from_texture(n, texture, priority, None) }
    }

    unsafe fn media(
        n: &Native,
        asset: &Asset,
        priority: u32,
        width: f32,
        height: f32,
    ) -> Result<Self> {
        let media = unsafe { GpuMedia::acquire(n, asset) }?;
        let sprite =
            unsafe { Self::from_texture(n, media.texture.clone(), priority, Some(media)) }?;
        let scale: Scale = unsafe { n.function("sprite_scale") };
        unsafe {
            scale(
                sprite.node,
                width / asset.width as f32,
                height / asset.height as f32,
            )
        };
        event(Event::Diag(format!(
            "media sprite priority={priority} descriptor={:p} canvas={}x{}",
            sprite.texture.as_ref().unwrap().descriptor(),
            asset.width,
            asset.height
        )));
        Ok(sprite)
    }

    unsafe fn update(&self) -> bool {
        let ready = self
            .media
            .as_ref()
            .is_none_or(|media| unsafe { media.update() }.is_ok());
        if !ready {
            unsafe { visible(self.node, false) };
        }
        ready
    }

    unsafe fn from_texture(
        n: &Native,
        texture: Arc<Texture>,
        priority: u32,
        media: Option<Arc<GpuMedia>>,
    ) -> Result<Self> {
        let manager: Manager = unsafe { n.function("sprite_manager") };
        let create: Factory = unsafe { n.function("sprite_create") };
        let get_group: unsafe extern "system" fn() -> u32 = unsafe {
            std::mem::transmute(field(n.base + n.profile.surface_data["afp_group_get"], 0))
        };
        let set_group: unsafe extern "system" fn(u32) = unsafe {
            std::mem::transmute(field(n.base + n.profile.surface_data["afp_group_set"], 0))
        };
        let group = unsafe { get_group() };
        unsafe { set_group(0) };
        let node = unsafe { create(manager(), texture.descriptor(), priority) };
        unsafe { set_group(group) };
        if node == 0 {
            return Err(Error::Native("custom sprite allocation"));
        }
        // The descriptor overload does not set the archive factory's group field.
        unsafe { ((node + 600) as *mut u32).write(0) };
        unsafe { visible(node, false) };
        Ok(Self {
            node,
            texture: Some(texture),
            media,
        })
    }
}

impl Drop for Sprite {
    fn drop(&mut self) {
        if self.node != 0 {
            // Bypass our release observer; caller already owns STATE.
            let release: Release = unsafe { original("sprite_release") };
            unsafe { release(self.node) };
        }
        // Texture unregisters only after the AFP node has been released.
    }
}

#[derive(Clone, Copy)]
struct PairInfo {
    pair: usize,
    mirror: bool,
    flip: bool,
}

/// Stack-owned restoration also runs if Rust unwinds inside a callback.
struct SavedPointers {
    address: usize,
    values: [usize; 2],
    count: usize,
}

impl Drop for SavedPointers {
    fn drop(&mut self) {
        for i in 0..self.count {
            unsafe { ((self.address + i * 8) as *mut usize).write(self.values[i]) };
        }
    }
}

struct Cover {
    object: usize,
    choice: Option<Arc<Bundle>>,
    upper: Option<Sprite>,
    lower: Option<Sprite>,
    attempted: bool,
}

struct Graph {
    object: usize,
    choice: Option<Arc<Bundle>>,
    sprite: Option<Sprite>,
    attempted: bool,
    stock_hidden: bool,
}

/// Native visibility captured before substituting a background.
#[derive(Clone, Copy)]
struct NativeClip {
    handle: u32,
    visible: u8,
}

#[derive(Clone, Copy)]
struct Transform {
    position: [f32; 2],
    origin: [f32; 2],
    scale: [f32; 2],
    color: [f32; 4],
}

unsafe fn clip_visible(n: &Native, handle: u32) -> Option<u8> {
    let get: PropertyGet = unsafe {
        std::mem::transmute(field(
            n.base + n.profile.surface_data["afp_property_get"],
            0,
        ))
    };
    let mut visible = 0u8;
    (unsafe { get(handle, 4103, &mut visible) } >= 0).then_some(visible)
}

unsafe fn set_clip_visible(n: &Native, handle: u32, value: u8) -> bool {
    let set: PropertySet = unsafe {
        std::mem::transmute(field(
            n.base + n.profile.surface_data["afp_property_set"],
            0,
        ))
    };
    if unsafe { set(handle, 4103, usize::from(value)) } < 0 {
        return false;
    }
    unsafe { set(handle, 4126, 1) };
    true
}

/// Read the native canvas transform so the replacement keeps the game's own
/// placement and upscaling instead of rebuilding a guessed screen rectangle.
unsafe fn read_transform(n: &Native, handle: u32) -> Option<Transform> {
    let get: PropertyGet = unsafe {
        std::mem::transmute(field(
            n.base + n.profile.surface_data["afp_property_get"],
            0,
        ))
    };
    let mut transform = Transform {
        position: [0.0; 2],
        origin: [0.0; 2],
        scale: [1.0; 2],
        color: [1.0; 4],
    };
    for (property, out) in [
        (4104, &mut transform.position[..]),
        (4123, &mut transform.origin[..]),
        (4099, &mut transform.scale[..]),
        (4106, &mut transform.color[..]),
    ] {
        if unsafe { get(handle, property, out.as_mut_ptr().cast()) } < 0
            || !out.iter().all(|v| v.is_finite() && v.abs() <= 1_000_000.0)
        {
            return None;
        }
    }
    // A zero scale never renders the stock art either; treat it as unusable
    // geometry instead of silently creating an invisible replacement.
    if transform.scale[0] == 0.0 || transform.scale[1] == 0.0 {
        return None;
    }
    Some(transform)
}

unsafe fn apply_transform(n: &Native, sprite: usize, transform: &Transform) {
    let set_origin: Position = unsafe { n.function("sprite_origin") };
    let set_position: Position = unsafe { n.function("sprite_position") };
    let set_scale: Scale = unsafe { n.function("sprite_scale") };
    let set_color: Alpha = unsafe { n.function("sprite_alpha") };
    unsafe {
        set_origin(
            sprite,
            transform.origin[0].round() as i32,
            transform.origin[1].round() as i32,
        );
        set_position(
            sprite,
            transform.position[0].round() as i32,
            transform.position[1].round() as i32,
        );
        set_scale(sprite, transform.scale[0], transform.scale[1]);
        set_color(
            sprite,
            transform.color[3],
            transform.color[0],
            transform.color[1],
            transform.color[2],
        );
    }
}

/// Premium-area background (`PremiumAreaBG`). The game creates its background
/// nodes once per session, so the replacement is rebuilt only when the prepared
/// image itself changes or the native visible node switches between the base
/// layer and the customization sprite.
#[derive(Default)]
struct PremiumArea {
    object: usize,
    base: usize,
    custom: usize,
    priority: u32,
    image: Option<Arc<Prepared>>,
    sprite: Option<Sprite>,
    hidden: [Option<NativeClip>; 2],
    /// The native pool retired our sprite. Native visibility stays hidden and
    /// the next draw/result hook rebuilds it; restoring here would flash stock.
    pending: bool,
}

impl PremiumArea {
    unsafe fn observe(&mut self, n: &Native, object: usize, priority: u32) {
        // Construction establishes a fresh lifetime even when the allocator
        // reuses the same object address.
        // A new owner replaces our previous nodes without dereferencing them.
        self.sprite = None;
        self.hidden = [None, None];
        self.image = None;
        self.pending = false;
        self.object = object;
        self.priority = priority;
        let (Some(base), Some(custom)) = (
            n.profile.surface_data.get("premium_area_base"),
            n.profile.surface_data.get("premium_area_custom"),
        ) else {
            return;
        };
        self.base = unsafe { field(object, *base) };
        self.custom = unsafe { field(object, *custom) };
        event(Event::Diag(format!(
            "premium area background: object={object:#x} base={:#x} custom={:#x} priority={}",
            self.base, self.custom, self.priority
        )));
    }

    unsafe fn refresh(&mut self, n: &Native, bundle: Option<Arc<Bundle>>, enabled: bool) {
        // Native pool retirement can destroy the source layer as well as our
        // sprite. Do not read a transform from its recycled address. Retire GPU
        // state here on the graphics owner; only a fresh constructor re-arms it.
        if self.base == 0 && self.custom == 0 {
            unsafe { self.restore(n) };
            return;
        }
        let asset = bundle
            .as_ref()
            .and_then(|b| b.premium_area.clone())
            .filter(|_| enabled);
        let Some(asset) = asset else {
            unsafe { self.restore(n) };
            return;
        };
        let image = asset.first();
        if self.object == 0 {
            return;
        }
        let current = self
            .image
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, &image));
        if current && self.sprite.as_ref().is_some_and(|sprite| sprite.node != 0) {
            if !unsafe { self.sprite.as_ref().unwrap().update() } {
                unsafe { self.restore(n) };
            }
            return;
        }
        let target = if self.custom != 0 {
            self.custom
        } else {
            self.base
        };
        if target == 0 {
            return;
        }
        if self.pending {
            // The native nodes are already hidden from the previous sprite;
            // keep them hidden and rebuild in place instead of restoring stock
            // for a frame.
            self.sprite = None;
            self.image = None;
        } else {
            unsafe { self.restore(n) };
        }
        let handle = unsafe { ((target + 528) as *const u32).read() };
        let Some(transform) = (unsafe { read_transform(n, handle) }) else {
            // The premium nodes are session-global and this path runs every
            // movie draw; a single broken transform must not fill the log at
            // frame rate. Log roughly once every five seconds at 120 Hz.
            if PREMIUM_FAULTS
                .fetch_add(1, Ordering::Relaxed)
                .is_multiple_of(600)
            {
                event(Event::Fault(
                    "premium area native transform unavailable; using stock",
                ));
            }
            return;
        };
        match unsafe {
            Sprite::media(
                n,
                &asset,
                self.priority,
                asset.width as f32,
                asset.height as f32,
            )
        } {
            Ok(sprite) => {
                unsafe { apply_transform(n, sprite.node, &transform) };
                // Both native nodes occupy the same 1280x720 priority-58 area.
                // Hide each one independently so a later native show cannot
                // replace our sprite, and restore the saved state on release.
                for (slot, node) in [self.base, self.custom].into_iter().enumerate() {
                    if node == 0 {
                        continue;
                    }
                    let native_handle = unsafe { ((node + 528) as *const u32).read() };
                    let Some(native_visible) = (unsafe { clip_visible(n, native_handle) }) else {
                        continue;
                    };
                    if unsafe { set_clip_visible(n, native_handle, 0) } {
                        self.hidden[slot] = Some(NativeClip {
                            handle: native_handle,
                            visible: native_visible,
                        });
                    }
                }
                unsafe { visible(sprite.node, true) };
                self.sprite = Some(sprite);
                self.image = Some(image);
                self.pending = false;
                event(Event::Surface("premium area"));
            }
            Err(_) => event(Event::Fault(
                "premium area sprite creation failed; using stock",
            )),
        }
    }

    unsafe fn restore(&mut self, n: &Native) {
        // Drop our sprite first so the native nodes never point at freed art.
        self.sprite = None;
        self.image = None;
        self.pending = false;
        for saved in self.hidden.iter_mut().filter_map(Option::take) {
            unsafe { set_clip_visible(n, saved.handle, saved.visible) };
        }
    }

    fn retire_source(&mut self, node: usize) -> bool {
        if node == 0 || (node != self.base && node != self.custom) {
            return false;
        }
        self.base = 0;
        self.custom = 0;
        // These handles belong to the retiring native lifetime. Never restore
        // their saved visibility into a subsequently recycled AFP node.
        self.hidden = [None, None];
        self.pending = false;
        true
    }

    unsafe fn released(&mut self, _n: &Native, node: usize) -> Option<Arc<Texture>> {
        if self.retire_source(node) {
            event(Event::Diag(
                "premium area native source retired; waiting for constructor".into(),
            ));
        }
        if let Some(sprite) = &mut self.sprite
            && sprite.node == node
        {
            sprite.node = 0;
            self.image = None;
            let texture = sprite.texture.take();
            // The scene sprite pool is retired while the premium nodes are
            // session-global. Keep them hidden and let the next draw/result hook
            // rebuild in place; restoring here would flash stock art for a frame.
            self.pending = true;
            event(Event::Diag(
                "premium area sprite released; waiting for rebuild".into(),
            ));
            return texture;
        }
        None
    }
}

/// One native clip rebound to our prepared texture. The original archive/name
/// binding is saved so disabling can return stock art through the same native
/// setter the game itself uses for its customization backgrounds.
struct BoundClip {
    clip: usize,
    node: u32,
    hidden: Option<u8>,
    /// Kept alive so the AFP node never references unregistered art.
    texture: Arc<Texture>,
    media: Option<Arc<GpuMedia>>,
    archive: usize,
    name: [u8; 256],
    restorable: bool,
}

impl BoundClip {
    unsafe fn bind(n: &Native, clip: usize, asset: &Asset) -> Result<Self> {
        // A CMovieClip stores its 256-byte name at +0, the AFP node id at +512
        // and the archive pointer at +520. Only a complete archive/name pair can
        // be rebound by the native archive setter.
        let archive = unsafe { ((clip + 520) as *const usize).read() };
        let mut name = [0u8; 256];
        unsafe { std::ptr::copy_nonoverlapping(clip as *const u8, name.as_mut_ptr(), name.len()) };
        let restorable = archive != 0
            && name.contains(&0)
            && name[0].is_ascii_graphic()
            && name.iter().all(|b| (0x20..0x7f).contains(b) || *b == 0);
        let node = unsafe { ((clip + 512) as *const u32).read() };
        if node & 0x8000_0000 != 0 {
            // The named clip exists in the layout but has no AFP node yet; a
            // texture bind would silently do nothing.
            return Err(Error::Native("concentration clip has no AFP node"));
        }
        let media = unsafe { GpuMedia::acquire(n, asset) }?;
        let texture = media.texture.clone();
        let set: SetTexture = unsafe { n.function("set_texture") };
        if unsafe { set(clip, texture.descriptor()) } == 0 {
            return Err(Error::Native("concentration texture bind"));
        }
        let visible = unsafe { clip_visible(n, node) };
        event(Event::Diag(format!(
            "concentration bind clip={clip:#x} node={node:#x} visible={visible:?} restorable={restorable}"
        )));
        Ok(Self {
            clip,
            node,
            hidden: None,
            texture,
            media: Some(media),
            archive,
            name,
            restorable,
        })
    }

    unsafe fn restore(&self, n: &Native) {
        if !self.restorable {
            return;
        }
        let set: SetArchive = unsafe { n.function("set_archive") };
        unsafe { set(self.clip, self.archive, self.name.as_ptr().cast()) };
    }

    unsafe fn hide_unrestorable(&mut self, n: &Native) {
        // A dynamic texture on the default layer root survives the native hide
        // animation. Explicitly mask that retained descriptor until next show.
        if self.hidden.is_none()
            && unsafe { ((self.clip + 512) as *const u32).read() } == self.node
            && let Some(visible) = unsafe { clip_visible(n, self.node) }
            && unsafe { set_clip_visible(n, self.node, 0) }
        {
            self.hidden = Some(visible);
        }
    }
}

/// Concentration-mode background. Both concentration window elements are
/// rebound to the prepared texture; the native clip/layer keeps its own
/// transform and draw order, so no screen rectangle is reconstructed. When an
/// element has no named background clip, its root layer clip is bound instead
/// (the default view may keep its art on the layer itself).
#[derive(Default)]
struct Concentration {
    window: usize,
    index: u32,
    layers: [usize; 2],
    clips: [usize; 2],
    fallback: [bool; 2],
    names: [u8; 2],
    visible: bool,
    image: Option<Asset>,
    bound_image: Option<Arc<Prepared>>,
    bound: Vec<BoundClip>,
}

impl Concentration {
    /// Both concentration window elements are probed for the customization
    /// background group and for the default `subbg0_concentration` layer used by
    /// the customize preview. The first hit per element is rebound; when neither
    /// exists the layer root is used.
    unsafe fn resolve(n: &Native, window: usize) -> ([usize; 2], [usize; 2], [bool; 2], [u8; 2]) {
        let mut layers = [0usize; 2];
        let mut clips = [0usize; 2];
        let mut fallback = [false; 2];
        let mut names = [0u8; 2];
        if window == 0 {
            return (layers, clips, fallback, names);
        }
        let getter: unsafe extern "system" fn(usize, *mut u32) -> *const usize =
            unsafe { n.function("concentration_layer") };
        let find: FindClip = unsafe { n.function("find_clip") };
        let candidates: [&std::ffi::CStr; 2] =
            [c"concentration_bg_set/bg", c"subbg0_concentration"];
        for index in 0..2 {
            let mut key = index as u32;
            let entry = unsafe { getter(window + 8, &mut key) };
            if entry.is_null() {
                continue;
            }
            let layer = unsafe { *entry };
            if layer == 0 {
                continue;
            }
            layers[index] = layer;
            for (name_index, candidate) in candidates.iter().enumerate() {
                let clip = unsafe { find(layer, candidate.as_ptr().cast()) };
                if clip != 0 {
                    clips[index] = clip;
                    names[index] = name_index as u8 + 1;
                    break;
                }
            }
            if clips[index] == 0 {
                // CMovieClip layout at the layer root: name at +0, node id at
                // +512 and archive at +520. layer +16 is the root clip object.
                clips[index] = layer + 16;
                fallback[index] = true;
            }
        }
        (layers, clips, fallback, names)
    }

    /// Resolve the window elements again. The native show/hide path creates and
    /// retires the clip nodes, so a clip found while the window was hidden can
    /// have no AFP node by the time it is shown. Resolving immediately before a
    /// bind keeps the clip entry and its node consistent.
    /// Both clip entries keep their AFP node between shows in the common case,
    /// so a repeated show can skip `find_clip` (which allocates a pool entry).
    fn clips_valid(&self) -> bool {
        self.clips.iter().filter(|clip| **clip != 0).all(|clip| {
            let node = unsafe { ((clip + 512) as *const u32).read() };
            node & 0x8000_0000 == 0
        })
    }

    unsafe fn observe(&mut self, n: &Native, window: usize, force: bool) {
        if !force
            && self.window == window
            && (self.clips[0] != 0 || self.clips[1] != 0)
            && self.clips_valid()
        {
            return;
        }
        unsafe { self.release(n) };
        self.window = window;
        (self.layers, self.clips, self.fallback, self.names) = unsafe { Self::resolve(n, window) };
        // Diagnostic: the named-clip helper allocates a 536-byte entry per
        // successful call, so the pool level shows whether repeated resolves
        // keep growing it.
        let pool = n
            .profile
            .surface_data
            .get("clip_pool_count")
            .map(|rva| unsafe { ((n.base + rva) as *const u32).read() })
            .unwrap_or(0);
        event(Event::Diag(format!(
            "concentration background: index={} pool={} element0={:#x} layer={:#x} name={} fallback={} element1={:#x} layer={:#x} name={} fallback={}",
            self.index,
            pool,
            self.clips[0],
            self.layers[0],
            self.names[0],
            self.fallback[0],
            self.clips[1],
            self.layers[1],
            self.names[1],
            self.fallback[1],
        )));
    }

    /// Log which element the game actually shows so the next report tells us
    /// whether the default or the customization view is active.
    unsafe fn log_visibility(&self, n: &Native, show: u8) {
        let mut visible = [None; 2];
        for (index, layer) in self.layers.iter().copied().enumerate() {
            if layer != 0 {
                let handle = unsafe { ((layer + 528) as *const u32).read() };
                visible[index] = unsafe { clip_visible(n, handle) };
            }
        }
        event(Event::Diag(format!(
            "concentration show={show} index={} element0_visible={:?} element1_visible={:?}",
            self.index, visible[0], visible[1]
        )));
    }

    /// The game restores its own archive texture through `set_archive`. When
    /// that happens on one of our bound clips while the overlay is visible,
    /// immediately rebind our already-uploaded texture instead of waiting for a
    /// new show event (this is the native "restore" the user asked about).
    unsafe fn reassert_after_native(&self, n: &Native, clip: usize) {
        if !self.visible || self.bound.is_empty() {
            return;
        }
        let Some(bound) = self.bound.iter().find(|bound| bound.clip == clip) else {
            return;
        };
        let node = unsafe { ((clip + 512) as *const u32).read() };
        if node & 0x8000_0000 != 0 {
            return;
        }
        let set: SetTexture = unsafe { n.function("set_texture") };
        unsafe { set(clip, bound.texture.descriptor()) };
        event(Event::Diag(format!(
            "concentration texture reasserted after native restore clip={clip:#x}"
        )));
    }

    unsafe fn refresh(&mut self, n: &Native, enabled: bool) {
        let asset = self.image.clone().filter(|_| enabled && self.visible);
        let Some(asset) = asset else {
            unsafe { self.release(n) };
            return;
        };
        if self.clips.iter().all(|clip| *clip == 0) {
            return;
        }
        let image = asset.first();
        let current = self
            .bound_image
            .as_ref()
            .is_some_and(|bound| Arc::ptr_eq(bound, &image));
        if current && !self.bound.is_empty() {
            if self.bound.iter().any(|bound| {
                bound
                    .media
                    .as_ref()
                    .is_some_and(|media| unsafe { media.update() }.is_err())
            }) {
                unsafe { self.release(n) };
            }
            return;
        }
        let mut previous = std::mem::take(&mut self.bound);
        for clip in self.clips.iter().copied().filter(|clip| *clip != 0) {
            match unsafe { BoundClip::bind(n, clip, &asset) } {
                Ok(mut bound) => {
                    // Keep the original native binding across source changes.
                    if let Some(old) = previous.iter().find(|old| old.clip == clip) {
                        bound.archive = old.archive;
                        bound.name = old.name;
                        bound.restorable = old.restorable;
                        if old.node == bound.node
                            && let Some(visible) = old.hidden
                        {
                            unsafe { set_clip_visible(n, bound.node, visible) };
                        }
                    }
                    // The node now points at the new texture; the old wrapper for
                    // this clip can be dropped safely.
                    previous.retain(|old| old.clip != clip);
                    if !bound.restorable {
                        event(Event::Diag(format!(
                            "concentration clip {clip:#x} has no restorable archive/name"
                        )));
                    }
                    self.bound.push(bound);
                }
                Err(_) => {
                    if let Some(position) = previous.iter().position(|old| old.clip == clip) {
                        self.bound.push(previous.remove(position));
                    }
                    event(Event::Fault(
                        "concentration texture bind failed; using stock this stage",
                    ));
                }
            }
        }
        for old in previous {
            unsafe { old.restore(n) };
        }
        if self.bound.is_empty() {
            self.bound_image = None;
        } else {
            self.bound_image = Some(image);
            event(Event::Surface("concentration background"));
        }
    }

    unsafe fn release(&mut self, n: &Native) {
        // Restorable clips return to their native archive texture. A clip without
        // that information keeps our wrapper alive so the AFP node never points
        // at unregistered art; it is invisible while the window is hidden.
        let mut keep = Vec::new();
        for mut bound in self.bound.drain(..) {
            if bound.restorable {
                unsafe { bound.restore(n) };
                // Dropping now unregisters our texture after the native rebind.
            } else {
                // Keep the descriptor alive, but stop hidden playback/IO.
                bound.media = None;
                unsafe { bound.hide_unrestorable(n) };
                keep.push(bound);
            }
        }
        self.bound = keep;
        self.bound_image = None;
    }
}

#[derive(Default)]
struct SelectBackdrop {
    hidden: Option<HiddenFrameChara>,
    sprite: Option<Sprite>,
    attempted: bool,
}

impl SelectBackdrop {
    unsafe fn current(n: &Native, scene: usize) -> Option<(usize, u32)> {
        let offset = *n.profile.surface_data.get("select_background")?;
        if pointer_field(scene, 0)? != n.base + n.profile.surface_data["select_vtable"] {
            return None;
        }
        let layer = pointer_field(scene, offset)?;
        if pointer_field(layer, 0)? != n.base + n.profile.surface_data["layer_vtable"]
            || pointer_field(layer, 8)? as u32 == 0
        {
            return None;
        }
        let clip = pointer_field(layer, 528)? as u32;
        (clip & 0x8000_0000 == 0).then_some((layer, clip))
    }

    unsafe fn restore(&mut self, n: &Native, scene: usize) {
        if let Some(sprite) = &self.sprite {
            unsafe { visible(sprite.node, false) };
        }
        if let Some(saved) = self.hidden.take()
            && saved.scene == scene
            && unsafe { Self::current(n, scene) } == Some((saved.layer, saved.clip))
        {
            unsafe { set_clip_visible(n, saved.clip, saved.visible) };
        }
    }

    unsafe fn update(&mut self, n: &Native, scene: usize, asset: Option<&Asset>) {
        let Some((layer, clip)) = (unsafe { Self::current(n, scene) }) else {
            return;
        };
        let Some(asset) = asset else { return };
        if !self.attempted {
            self.attempted = true;
            let priority = unsafe {
                ((scene + n.profile.surface_data["select_background_priority"]) as *const u32)
                    .read()
            };
            self.sprite =
                unsafe { Sprite::media(n, asset, priority.saturating_add(1), 1920., 1080.) }.ok();
            if self.sprite.is_some() {
                event(Event::Surface("selection AFP background"));
            }
        }
        if let Some(sprite) = &self.sprite
            && sprite.node != 0
            && unsafe { sprite.update() }
            && let Some(saved) = unsafe { clip_visible(n, clip) }
            && unsafe { set_clip_visible(n, clip, 0) }
        {
            self.hidden = Some(HiddenFrameChara {
                scene,
                layer,
                clip,
                visible: saved,
            });
            unsafe { visible(sprite.node, saved != 0) };
        }
    }
}

struct Surfaces {
    scene: usize,
    select_choice: Option<Arc<Bundle>>,
    select_texture: Option<Texture>,
    select_attempted: bool,
    pairs: Vec<PairInfo>,
    covers: Vec<Cover>,
    graphs: Vec<Graph>,
    frame_chara: FrameChara,
    qpro: qpro::Replacement,
    backdrop: SelectBackdrop,
    premium_area: PremiumArea,
    concentration: Concentration,
}

impl Surfaces {
    const fn new() -> Self {
        Self {
            scene: 0,
            select_choice: None,
            select_texture: None,
            select_attempted: false,
            pairs: Vec::new(),
            covers: Vec::new(),
            graphs: Vec::new(),
            backdrop: SelectBackdrop {
                hidden: None,
                sprite: None,
                attempted: false,
            },
            frame_chara: FrameChara {
                hidden: None,
                image: None,
                sprite: None,
                layer: 0,
                attempted: false,
                placed: None,
            },
            qpro: qpro::Replacement::new(),
            premium_area: PremiumArea {
                object: 0,
                base: 0,
                custom: 0,
                priority: 0,
                image: None,
                sprite: None,
                hidden: [None, None],
                pending: false,
            },
            concentration: Concentration {
                window: 0,
                index: 0,
                layers: [0, 0],
                clips: [0, 0],
                fallback: [false, false],
                names: [0, 0],
                visible: false,
                image: None,
                bound_image: None,
                bound: Vec::new(),
            },
        }
    }

    fn leave_select(&mut self, scene: usize) {
        if self.scene == scene {
            if let Some(n) = NATIVE.get() {
                // The concentration overlay is re-applied per stage, so release
                // its bindings here. The premium background is also used outside
                // selection: keep the replacement and let the sprite-release
                // observer restore native visibility only when it is reclaimed.
                unsafe { self.concentration.release(n) };
            }
            self.concentration.visible = false;
            self.concentration.image = None;
            self.frame_chara = FrameChara::default();
            self.qpro = qpro::Replacement::new();
            super::reactions::leave();
            artwork::changed();
            self.backdrop = SelectBackdrop::default();
            self.scene = 0;
            // Keep the selection texture: the next visit refreshes it in place
            // instead of creating a new D3D texture. Device loss drops it.
            self.select_choice = None;
            self.select_attempted = false;
            request_images();
        }
    }

    unsafe fn invalidate(&mut self) {
        if let Some(n) = NATIVE.get() {
            unsafe { self.frame_chara.restore(n, self.scene) };
            unsafe { self.qpro.restore(n, self.scene) };
            unsafe { self.backdrop.restore(n, self.scene) };
            unsafe { self.premium_area.restore(n) };
            unsafe { self.concentration.release(n) };
        }
        self.frame_chara.sprite = None;
        self.qpro = qpro::Replacement::new();
        self.frame_chara.attempted = false;
        self.select_texture = None;
        self.backdrop.sprite = None;
        self.backdrop.attempted = false;
        self.select_attempted = false;
        for cover in &mut self.covers {
            cover.upper = None;
            cover.lower = None;
            cover.attempted = false;
        }
        for graph in &mut self.graphs {
            graph.sprite = None;
            graph.attempted = false;
            if graph.stock_hidden {
                unsafe { visible(field(graph.object, 64), true) };
                graph.stock_hidden = false;
            }
        }
    }
}

pub(super) fn invalidate() {
    super::reactions::leave();
    artwork::invalidate();
    if let Some(mut s) = state() {
        unsafe { s.invalidate() };
    } else {
        ENABLED.store(false, Ordering::Release);
        event(Event::Fault("surface invalidation reentry"));
    }
}

fn tick_qpro() {
    if let Some(n) = NATIVE.get()
        && let Some(mut s) = state()
    {
        let scene = s.scene;
        unsafe { s.qpro.update(n, scene) };
    }
}

unsafe extern "system" fn select_update(this: usize, tick: u8, context: usize) -> usize {
    restore_frame_chara(this);
    let f: SelectUpdate = unsafe { original("select_update") };
    let result = unsafe { f(this, tick, context) };
    observe_select(this);
    result
}

unsafe extern "system" fn select_update_short(this: usize) -> usize {
    restore_frame_chara(this);
    let f: Unary = unsafe { original("select_update") };
    let result = unsafe { f(this) };
    observe_select(this);
    result
}

/// Rebuild the premium replacement if the native scene pool retired it. Kept
/// separate from the per-frame movie path so results can also rebuild it.
pub(super) fn tick_premium() {
    boundary(|| {
        if !can_render() {
            return;
        }
        let Some(n) = NATIVE.get() else { return };
        let Some(mut s) = state() else { return };
        unsafe {
            s.concentration
                .refresh(n, CONCENTRATION_ENABLED.load(Ordering::Acquire))
        };
        if s.premium_area.object != 0 {
            unsafe {
                s.premium_area
                    .refresh(n, snapshot(), PREMIUM_AREA_ENABLED.load(Ordering::Acquire))
            };
        }
    });
}

fn observe_select(this: usize) {
    let mut fresh = 0usize;
    boundary(|| {
        let n = NATIVE.get().unwrap();
        let bundle = snapshot();
        if can_render()
            && unsafe { field(this, 0) } == n.base + n.profile.surface_data["select_vtable"]
            && let Some(mut s) = state()
        {
            if s.scene != this {
                let old = s.scene;
                if old != 0 {
                    s.leave_select(old);
                }
                s.scene = this;
                s.select_choice = snapshot();
                request_images();
                fresh = this;
            }
            unsafe { super::reactions::selection(n, this) };
            let choice = s.select_choice.clone();
            unsafe {
                s.backdrop
                    .update(n, this, choice.as_ref().and_then(|b| b.select.as_ref()))
            };
            unsafe {
                s.frame_chara
                    .update(n, this, HIDE_FRAME_CHARA.load(Ordering::Acquire))
            };
            if s.premium_area.object != 0 {
                unsafe {
                    s.premium_area
                        .refresh(n, bundle, PREMIUM_AREA_ENABLED.load(Ordering::Acquire))
                };
            }
        }
    });
    // Diagnostics run after the STATE guard is released. Holding it during the
    // AFP probes made a concurrent device-reset callback see a held lock
    // (`surface invalidation reentry`) and disable the feature.
    if fresh != 0
        && let Some(n) = NATIVE.get()
    {
        boundary(|| unsafe { diag_selection(n, fresh) });
    }
}

/// Read-only scan of a scene's AFP layer slots and candidate named clips. Used
/// once per results/selection visit to identify the stock background layer.
/// Pointers are only dereferenced after a `VirtualQuery` readability check and
/// a game-image vtable check; named lookups use the game's null-checked helper
/// on validated CLayer objects.
pub(super) unsafe fn diag_scene(n: &Native, scene: usize, label: &str) {
    if !readable(scene) {
        return;
    }
    let layer_vtable = n.base + n.profile.surface_data["layer_vtable"];
    let dimensions: unsafe extern "system" fn(usize, *mut i32, *mut i32) =
        unsafe { n.function("layer_dimensions") };
    let find: FindClip = unsafe { n.function("find_clip") };
    let names = [
        "bg",
        "background",
        "base",
        "back",
        "wall",
        "wallpaper",
        "select",
        "select_bg",
        "music_sel_bg",
        "result_bg",
        "illust",
        "bg1_clear",
        "bg1_failed",
        "header_clear",
        "main_bg",
        "black",
        "cover",
        "frame_layer1",
        "frame_layer2",
        "frame_layer3",
        "frame_chara",
        "frame_chara_usr",
    ];
    let scene_vtable = pointer_field(scene, 0).unwrap_or(0);
    event(Event::Diag(format!(
        "{label} scene scan: scene={scene:#x} vt={scene_vtable:#x}"
    )));
    let mut layers = 0u32;
    for offset in (0..1024).step_by(8) {
        let Some(value) = pointer_field(scene, offset) else {
            continue;
        };
        if value == 0 {
            continue;
        }
        let Some(vtable) = pointer_field(value, 0) else {
            continue;
        };
        if !(n.base..n.base + n.profile.image_size).contains(&vtable) {
            continue;
        }
        if vtable != layer_vtable {
            event(Event::Diag(format!(
                "{label} scene+{offset}={value:#x} vt={vtable:#x}"
            )));
            continue;
        }
        layers += 1;
        let (mut width, mut height) = (0, 0);
        unsafe { dimensions(value, &mut width, &mut height) };
        let handle = if readable(value.saturating_add(528)) {
            unsafe { ((value + 528) as *const u32).read() }
        } else {
            0
        };
        let visible = unsafe { clip_visible(n, handle) };
        event(Event::Diag(format!(
            "{label} layer scene+{offset}={value:#x} dims={width}x{height} handle={handle:#x} visible={visible:?}"
        )));
        for name in names {
            let mut c_name = Vec::with_capacity(name.len() + 1);
            c_name.extend_from_slice(name.as_bytes());
            c_name.push(0);
            let clip = unsafe { find(value, c_name.as_ptr()) };
            if clip == 0 {
                continue;
            }
            let node = if readable(clip.saturating_add(512)) {
                unsafe { ((clip + 512) as *const u32).read() }
            } else {
                0x8000_0000
            };
            let visible = unsafe { clip_visible(n, node) };
            let transform = if node & 0x8000_0000 == 0 {
                unsafe { read_transform(n, node) }
            } else {
                None
            };
            event(Event::Diag(format!(
                "{label} layer scene+{offset} clip '{name}'={clip:#x} node={node:#x} visible={visible:?} transform={}",
                match transform {
                    Some(t) => format!(
                        "pos={:?} origin={:?} scale={:?} color={:?}",
                        t.position, t.origin, t.scale, t.color
                    ),
                    None => "none".to_owned(),
                }
            )));
        }
    }
    event(Event::Diag(format!("{label} scene scan: {layers} layers")));
    if let Some(rva) = n.profile.surface_data.get("clip_pool_count")
        && readable(n.base.saturating_add(*rva))
    {
        let pool = unsafe { ((n.base + rva) as *const u32).read() };
        event(Event::Diag(format!("{label} scene scan: pool={pool}")));
    }
}

unsafe fn diag_selection(n: &Native, scene: usize) {
    unsafe { diag_scene(n, scene, "selection") };
}

/// Inspect only the verified background movie after setup. No scene-wide
/// pointer scanning and no speculative layer suppression.
pub(super) unsafe fn diag_results(n: &Native, background: usize) {
    let movie = unsafe { field(background, n.profile.layout.background_movie) };
    let archive = unsafe { field(background, n.profile.layout.background_archive) };
    let root = pointer_field(movie, 528).unwrap_or(usize::MAX) as u32;
    event(Event::Diag(format!(
        "results initialized: background={background:#x} movie={movie:#x} archive={archive:#x} root={root:#x}"
    )));
}

/// Current backbuffer size. Only called from a `movie_draw` diagnostic, never
/// from a hot path.
pub(super) unsafe fn render_target_size(n: &Native) -> (u32, u32) {
    let get: Device = unsafe { n.function("get_device") };
    let raw = unsafe { get() };
    let Some(device) = (unsafe { IDirect3DDevice9::from_raw_borrowed(&raw) }) else {
        return (0, 0);
    };
    let Ok(surface) = (unsafe { device.GetRenderTarget(0) }) else {
        return (0, 0);
    };
    let mut description = D3DSURFACE_DESC::default();
    if unsafe { surface.GetDesc(&mut description) }.is_err() {
        return (0, 0);
    }
    (description.Width, description.Height)
}

fn restore_frame_chara(scene: usize) {
    boundary(|| {
        // Restore even after feature disable/fault, before native update or teardown.
        if unsafe { GetCurrentThreadId() } == OWNER.load(Ordering::Acquire)
            && let Some(n) = NATIVE.get()
            && let Some(mut s) = state()
        {
            unsafe { s.frame_chara.restore(n, scene) };
            unsafe { s.qpro.restore(n, scene) };
            unsafe { s.backdrop.restore(n, scene) };
        }
    });
}

unsafe extern "system" fn select_cleanup(this: usize) -> usize {
    restore_frame_chara(this);
    boundary(|| {
        if let Some(mut s) = state() {
            s.leave_select(this);
        }
    });
    let f: Unary = unsafe { original("select_cleanup") };
    unsafe { f(this) }
}

unsafe extern "system" fn select_delete(this: usize, flags: u8) -> usize {
    restore_frame_chara(this);
    boundary(|| {
        if let Some(mut s) = state() {
            s.leave_select(this);
        }
    });
    let f: SceneDelete = unsafe { original("select_delete") };
    unsafe { f(this, flags) }
}

/// Run at the stock movie layer, before AFP draws the selection UI.
unsafe extern "system" fn movie_draw(this: usize) -> usize {
    let f: Unary = unsafe { original("movie_draw") };
    let mut drawn = false;
    boundary(|| {
        if !can_render() {
            return;
        }
        let Some(mut s) = state() else { return };
        let n = NATIVE.get().unwrap();
        unsafe {
            s.concentration
                .refresh(n, CONCENTRATION_ENABLED.load(Ordering::Acquire))
        };
        // The native sprite pool retires our premium sprite at scene changes
        // while the native nodes are session-global. Recreate it here so the
        // premium background also survives loading and gameplay.
        if s.premium_area.object != 0 {
            unsafe {
                s.premium_area
                    .refresh(n, snapshot(), PREMIUM_AREA_ENABLED.load(Ordering::Acquire))
            };
        }
        if s.scene == 0 {
            return;
        }
        if n.profile.surface_data.contains_key("select_background") {
            return;
        }
        if !s.select_attempted {
            s.select_attempted = true;
            if let Some(image) = s.select_choice.as_ref().and_then(|b| b.select.clone()) {
                let ok = if let Some(texture) = s.select_texture.as_mut() {
                    unsafe { texture.refresh(n, &image) }.is_ok()
                } else {
                    match unsafe { Texture::upload(n, &image) } {
                        Ok(texture) => {
                            s.select_texture = Some(texture);
                            true
                        }
                        Err(_) => false,
                    }
                };
                if ok {
                    event(Event::Surface("song selection"));
                } else {
                    s.select_texture = None;
                    event(Event::Fault(
                        "selection texture refresh failed; using stock",
                    ));
                }
            }
        }
        let mut alpha = 1.0;
        if let Some(texture) = &s.select_texture {
            alpha = if n.profile.surface_data["selection_fade"] != 0 {
                unsafe { ((this + 40) as *const f32).read() }
            } else {
                1.0
            };
            drawn = unsafe { draw_fullscreen_quad(n, texture, alpha) }.is_ok();
        }
        let tick = SELECT_DRAW_TICKS.fetch_add(1, Ordering::Relaxed);
        if tick.is_multiple_of(120) && readable(this) && readable(this.saturating_add(40)) {
            let (rt_width, rt_height) = unsafe { render_target_size(n) };
            let enable = unsafe {
                [
                    ((this + 16) as *const u8).read(),
                    ((this + 36) as *const u8).read(),
                ]
            };
            let rect = unsafe {
                [
                    ((this + 20) as *const i32).read(),
                    ((this + 24) as *const i32).read(),
                    ((this + 28) as *const i32).read(),
                    ((this + 32) as *const i32).read(),
                ]
            };
            let native_alpha = unsafe { ((this + 40) as *const f32).read() };
            event(Event::Diag(format!(
                "selection draw: scene={:#x} tex={} alpha={alpha:.3} drawn={drawn} rt={rt_width}x{rt_height} native_rect=({},{},{},{}) enable={}/{} native_alpha={native_alpha:.3}",
                s.scene,
                s.select_texture.is_some(),
                rect[0],
                rect[1],
                rect[2],
                rect[3],
                enable[0],
                enable[1],
            )));
        }
        // Video feasibility probe for the selection canvas: the real cost of a
        // full 1920x1080 per-frame refresh on the game device. Rewrites the
        // same image, so the visible frame is unchanged.
        if let (Some(texture), Some(image)) = (
            &s.select_texture,
            s.select_choice.as_ref().and_then(|b| b.select.as_ref()),
        ) {
            let probe_tick = SELECT_PROBE_TICKS.fetch_add(1, Ordering::Relaxed);
            if probe_tick.is_multiple_of(240)
                && let Some((lock, copy, unlock)) =
                    unsafe { Texture::probe_refresh(texture, image) }
            {
                event(Event::Diag(format!(
                    "video probe: selection refresh {}x{} lock={lock}us copy={copy}us unlock={unlock}us",
                    image.width, image.height
                )));
            }
        }
    });
    if drawn {
        0
    } else {
        unsafe { artwork::draw_movie(this, f) }
    }
}

#[repr(C)]
struct Vertex {
    x: f32,
    y: f32,
    z: f32,
    rhw: f32,
    color: u32,
    u: f32,
    v: f32,
}

pub(super) unsafe fn draw_fullscreen_quad(n: &Native, texture: &Texture, alpha: f32) -> Result<()> {
    let device: Device = unsafe { n.function("get_device") };
    let raw = unsafe { device() };
    let device = unsafe { IDirect3DDevice9::from_raw_borrowed(&raw) }
        .ok_or(Error::Native("selection device"))?;
    let raw_texture = texture.wrapper[1] as *mut c_void;
    let texture = unsafe { IDirect3DBaseTexture9::from_raw_borrowed(&raw_texture) }
        .ok_or(Error::Native("selection texture"))?;
    let saved = unsafe { device.CreateStateBlock(D3DSBT_ALL) }
        .map_err(|_| Error::Native("capture selection graphics state"))?;
    let color = (((alpha.clamp(0.0, 1.0) * 255.0) as u32) << 24) | 0xffffff;
    let vertices = [
        (0., 0., 0., 0.),
        (0., 1080., 0., 1.),
        (1920., 1080., 1., 1.),
        (1920., 0., 1., 0.),
    ]
    .map(|(x, y, u, v)| Vertex {
        x: x - 0.5,
        y: y - 0.5,
        z: 0.,
        rhw: 1.,
        color,
        u,
        v,
    });
    let draw = unsafe {
        (|| -> windows::core::Result<()> {
            device.SetVertexShader(None)?;
            device.SetPixelShader(None)?;
            device.SetFVF(0x144)?;
            device.SetTexture(0, texture)?;
            for (state, value) in [
                (D3DRS_ZENABLE, 0),
                (D3DRS_ZWRITEENABLE, 0),
                (D3DRS_CULLMODE, D3DCULL_NONE.0 as u32),
                (D3DRS_ALPHATESTENABLE, 0),
                (D3DRS_ALPHABLENDENABLE, 1),
                (D3DRS_SRCBLEND, D3DBLEND_SRCALPHA.0 as u32),
                (D3DRS_DESTBLEND, D3DBLEND_INVSRCALPHA.0 as u32),
                (D3DRS_BLENDOP, D3DBLENDOP_ADD.0 as u32),
                (D3DRS_SEPARATEALPHABLENDENABLE, 0),
                (D3DRS_SCISSORTESTENABLE, 0),
                (D3DRS_COLORWRITEENABLE, 15),
                (D3DRS_FOGENABLE, 0),
                (D3DRS_STENCILENABLE, 0),
            ] {
                device.SetRenderState(state, value)?;
            }
            for (state, value) in [
                (D3DTSS_COLOROP, D3DTOP_MODULATE.0 as u32),
                (D3DTSS_COLORARG1, D3DTA_TEXTURE),
                (D3DTSS_COLORARG2, D3DTA_DIFFUSE),
                (D3DTSS_ALPHAOP, D3DTOP_MODULATE.0 as u32),
                (D3DTSS_ALPHAARG1, D3DTA_TEXTURE),
                (D3DTSS_ALPHAARG2, D3DTA_DIFFUSE),
                (D3DTSS_TEXCOORDINDEX, 0),
                (D3DTSS_TEXTURETRANSFORMFLAGS, 0),
            ] {
                device.SetTextureStageState(0, state, value)?;
            }
            device.SetTextureStageState(1, D3DTSS_COLOROP, D3DTOP_DISABLE.0 as u32)?;
            for (state, value) in [
                (D3DSAMP_MINFILTER, D3DTEXF_LINEAR.0 as u32),
                (D3DSAMP_MAGFILTER, D3DTEXF_LINEAR.0 as u32),
                (D3DSAMP_MIPFILTER, D3DTEXF_NONE.0 as u32),
                (D3DSAMP_ADDRESSU, D3DTADDRESS_CLAMP.0 as u32),
                (D3DSAMP_ADDRESSV, D3DTADDRESS_CLAMP.0 as u32),
            ] {
                device.SetSamplerState(0, state, value)?;
            }
            device.DrawPrimitiveUP(D3DPT_TRIANGLEFAN, 2, vertices.as_ptr().cast(), 28)
        })()
    };
    let restored = unsafe { saved.Apply() };
    if restored.is_err() {
        ENABLED.store(false, Ordering::Release);
        event(Event::Fault("selection graphics state restoration failed"));
    }
    draw.and(restored)
        .map_err(|_| Error::Native("selection draw"))
}

unsafe extern "system" fn cover_build(pair: usize, archive: usize, id: i32, mirror: u8, flip: u8) {
    boundary(|| {
        if let Some(mut s) = state() {
            for c in &mut s.covers {
                if unsafe { field(c.object, 8) } == pair || unsafe { field(c.object, 16) } == pair {
                    c.upper = None;
                    c.lower = None;
                    c.attempted = false;
                }
            }
        }
    });
    let f: CoverBuild = unsafe { original("cover_build") };
    unsafe { f(pair, archive, id, mirror, flip) };
    boundary(|| {
        if let Some(mut s) = state() {
            s.pairs.retain(|p| p.pair != pair);
            if s.pairs.len() < 8 {
                s.pairs.push(PairInfo {
                    pair,
                    mirror: mirror != 0,
                    flip: flip != 0,
                });
            }
        }
    });
}

unsafe fn make_cover(n: &Native, image: &Asset, info: PairInfo) -> Result<Sprite> {
    let sprite = unsafe { Sprite::media(n, image, 108, 432., 723.) }?;
    let clip: Clip = unsafe { n.function("sprite_clip") };
    let origin: Position = unsafe { n.function("sprite_origin") };
    let scale: Scale = unsafe { n.function("sprite_scale") };
    unsafe {
        clip(sprite.node, [0, 0, 1920, 723].as_ptr());
        origin(sprite.node, 0, 0);
        let sx = 432. / image.width as f32;
        let sy = 723. / image.height as f32;
        scale(sprite.node, sx, sy);
        if info.mirror {
            scale(sprite.node, -sx, sy);
            origin(sprite.node, image.width as i32 / 2, 0);
        }
        if info.flip {
            scale(sprite.node, sx, -sy);
            origin(sprite.node, 0, image.height as i32);
        }
    }
    Ok(sprite)
}

/// Temporarily substitute only the art pair. The native routine still owns
/// visibility, SUDDEN/HIDDEN/LIFT heights, DP positioning and numeric overlays.
unsafe extern "system" fn cover_visual(this: usize) -> usize {
    let f: Unary = unsafe { original("cover_visual") };
    let mut result = None;
    boundary(|| {
        if !can_render() {
            return;
        }
        let Some(mut s) = state() else { return };
        let n = NATIVE.get().unwrap();
        if !s.covers.iter().any(|c| c.object == this) {
            if s.covers.len() >= 2 {
                return;
            }
            s.covers.push(Cover {
                object: this,
                choice: snapshot(),
                upper: None,
                lower: None,
                attempted: false,
            });
        }
        let upper = unsafe { field(this, 8) };
        let lower = unsafe { field(this, 16) };
        let infos = [upper, lower].map(|pair| s.pairs.iter().find(|p| p.pair == pair).copied());
        let c = s.covers.iter_mut().find(|c| c.object == this).unwrap();
        if !c.attempted {
            c.attempted = true;
            if let Some(b) = &c.choice {
                for (image, info, slot) in [
                    (&b.upper, infos[0], &mut c.upper),
                    (&b.lower, infos[1], &mut c.lower),
                ] {
                    if let Some(image) = image
                        && let Some(info) = info
                    {
                        *slot = unsafe { make_cover(n, image, info) }.ok();
                        if slot.is_none() {
                            event(Event::Fault("cover sprite creation failed; using stock"));
                        } else {
                            event(Event::Surface("lane cover"));
                        }
                    }
                }
            }
        }
        let mut restore = [None, None];
        for (index, (pair, sprite)) in [(upper, &c.upper), (lower, &c.lower)]
            .into_iter()
            .enumerate()
        {
            if pair != 0
                && let Some(sprite) = sprite
                && sprite.node != 0
                && unsafe { sprite.update() }
            {
                let old = unsafe { [field(pair, 0), field(pair, 8)] };
                restore[index] = Some(SavedPointers {
                    address: pair,
                    values: old,
                    count: 2,
                });
                unsafe {
                    visible(old[0], false);
                    visible(old[1], false);
                    (pair as *mut usize).write(sprite.node);
                    ((pair + 8) as *mut usize).write(0);
                }
            }
        }
        result = Some(unsafe { f(this) });
        drop(restore);
    });
    result.unwrap_or_else(|| unsafe { f(this) })
}

unsafe extern "system" fn cover_delete(this: usize) {
    retire_cover(this);
    let f: Release = unsafe { original("cover_delete") };
    unsafe { f(this) };
}

fn retire_cover(this: usize) {
    boundary(|| {
        if let Some(mut s) = state() {
            let pairs = unsafe { [field(this, 8), field(this, 16)] };
            s.pairs.retain(|p| !pairs.contains(&p.pair));
            let count = s.covers.len();
            s.covers.retain(|c| c.object != this);
            if count != s.covers.len() {
                request_images();
            }
        }
    });
}

unsafe extern "system" fn cover_init(this: usize, side: u32) -> usize {
    retire_cover(this);
    let f: unsafe extern "system" fn(usize, u32) -> usize = unsafe { original("cover_init") };
    unsafe { f(this, side) }
}

unsafe extern "system" fn graph_art(this: usize) {
    let f: Release = unsafe { original("graph_art") };
    let mut called = false;
    boundary(|| {
        if !can_render() {
            return;
        }
        let Some(mut s) = state() else { return };
        let n = NATIVE.get().unwrap();
        if !s.graphs.iter().any(|g| g.object == this) {
            if s.graphs.len() >= 2 {
                return;
            }
            s.graphs.push(Graph {
                object: this,
                choice: snapshot(),
                sprite: None,
                attempted: false,
                stock_hidden: false,
            });
        }
        let g = s.graphs.iter_mut().find(|g| g.object == this).unwrap();
        if !g.attempted {
            g.attempted = true;
            if let Some(image) = g.choice.as_ref().and_then(|b| b.graph.as_ref()) {
                g.sprite = unsafe { Sprite::media(n, image, 145, 432., 870.) }.ok();
                if let Some(sprite) = &g.sprite {
                    let alpha: Alpha = unsafe { n.function("sprite_alpha") };
                    unsafe { alpha(sprite.node, 1.0, 0.7, 0.7, 0.7) };
                    event(Event::Surface("target graph"));
                } else {
                    event(Event::Fault("graph sprite creation failed; using stock"));
                }
            }
        }
        let stock = unsafe { field(this, 64) };
        let marker: FindClip = unsafe { n.function("graph_marker") };
        let frame = unsafe { field(this, 8) };
        let has_marker =
            frame != 0 && unsafe { marker(frame, c"gr_line_max".as_ptr().cast()) } != 0;
        if let Some(sprite) = &g.sprite
            && sprite.node != 0
            && unsafe { sprite.update() }
            && has_marker
        {
            let restore = SavedPointers {
                address: this + 64,
                values: [stock, 0],
                count: 1,
            };
            unsafe {
                visible(stock, false);
                visible(sprite.node, true);
                ((this + 64) as *mut usize).write(sprite.node);
                f(this);
            }
            drop(restore);
            g.stock_hidden = stock != 0;
            called = true;
        } else if g.stock_hidden {
            unsafe { visible(stock, true) };
            g.stock_hidden = false;
        }
        if !has_marker && let Some(sprite) = &g.sprite {
            unsafe { visible(sprite.node, false) };
        }
    });
    if !called {
        unsafe { f(this) }
    }
}

unsafe extern "system" fn graph_reset(this: usize) -> usize {
    retire_graph(this);
    let f: Unary = unsafe { original("graph_reset") };
    unsafe { f(this) }
}

fn retire_graph(this: usize) {
    boundary(|| {
        if let Some(mut s) = state() {
            let count = s.graphs.len();
            s.graphs.retain(|g| g.object != this);
            if count != s.graphs.len() {
                request_images();
            }
        }
    });
}

unsafe extern "system" fn graph_parent_reset(this: usize) -> usize {
    retire_graph(this + 136);
    let f: Unary = unsafe { original("graph_parent_reset") };
    unsafe { f(this) }
}

unsafe extern "system" fn graph_storage_release(this: usize) {
    if this >= 80 {
        retire_graph(this - 80);
    }
    let f: Release = unsafe { original("graph_storage_release") };
    unsafe { f(this) };
}

/// Native scene/group/pool cleanup can retire a sprite independently of its
/// owner. Observe the common release function before its slot becomes reusable.
unsafe extern "system" fn sprite_release(this: usize) {
    artwork::released(this);
    let mut texture = None;
    boundary(|| {
        if let Some(mut s) = state() {
            texture = s.frame_chara.released(this);
            s.qpro.released(this);
            if s.backdrop.hidden.is_some_and(|saved| saved.layer == this) {
                s.backdrop.hidden = None;
            }
            if let Some(sprite) = &mut s.backdrop.sprite
                && sprite.node == this
            {
                sprite.node = 0;
                texture = sprite.texture.take();
                s.backdrop.attempted = false;
            }
            if let Some(n) = NATIVE.get()
                && let Some(released) = unsafe { s.premium_area.released(n, this) }
            {
                texture = Some(released);
            }
            for c in &mut s.covers {
                for sprite in [&mut c.upper, &mut c.lower].into_iter().flatten() {
                    if sprite.node == this {
                        sprite.node = 0;
                        texture = sprite.texture.take();
                        c.attempted = false;
                    }
                }
            }
            for g in &mut s.graphs {
                if let Some(sprite) = &mut g.sprite
                    && sprite.node == this
                {
                    sprite.node = 0;
                    texture = sprite.texture.take();
                    g.attempted = false;
                }
            }
        }
    });
    let f: Release = unsafe { original("sprite_release") };
    unsafe { f(this) };
    drop(texture);
}

/// Premium-area background constructor. Records the native base layer and the
/// optional customization sprite, then installs the replacement if one is ready.
unsafe extern "system" fn premium_area_ctor(
    this: usize,
    priority: i32,
    customize: u8,
    shared: *mut usize,
) -> usize {
    let f: unsafe extern "system" fn(usize, i32, u8, *mut usize) -> usize =
        unsafe { original("premium_area_ctor") };
    let result = unsafe { f(this, priority, customize, shared) };
    boundary(|| {
        let Some(n) = NATIVE.get() else { return };
        let Some(mut s) = state() else { return };
        unsafe { s.premium_area.observe(n, this, priority.max(0) as u32) };
        if can_render() {
            unsafe {
                s.premium_area
                    .refresh(n, snapshot(), PREMIUM_AREA_ENABLED.load(Ordering::Acquire))
            };
        }
    });
    result
}

/// Applied once per stage by the gameplay initializer. The clip lookup mirrors
/// the native customization binding at the same layer index.
unsafe extern "system" fn concentration_apply(window: usize, index: u32) {
    let f: unsafe extern "system" fn(usize, u32) = unsafe { original("concentration_apply") };
    unsafe { f(window, index) };
    boundary(|| {
        if !can_render() {
            return;
        }
        let Some(n) = NATIVE.get() else { return };
        let Some(mut s) = state() else { return };
        s.concentration.window = window;
        s.concentration.index = index;
        s.concentration.image = snapshot().and_then(|bundle| bundle.concentration.clone());
        // The native apply runs while the window may still be hidden; the clip
        // nodes are (re)created by the show path, so resolve there.
        if s.concentration.visible {
            unsafe { s.concentration.observe(n, window, true) };
            unsafe {
                s.concentration
                    .refresh(n, CONCENTRATION_ENABLED.load(Ordering::Acquire))
            };
        }
    });
}

/// Native concentration show/hide. The replacement follows this flag so it can
/// never outlive the gameplay overlay that owns the background.
unsafe extern "system" fn concentration_show(window: usize, show: u8) -> usize {
    let f: unsafe extern "system" fn(usize, u8) -> usize =
        unsafe { original("concentration_show") };
    let result = unsafe { f(window, show) };
    boundary(|| {
        if !can_render() {
            return;
        }
        let Some(n) = NATIVE.get() else { return };
        let Some(mut s) = state() else { return };
        if show != 0 {
            // Resolve after the native show so the clip entries point at the
            // freshly created AFP nodes; a repeated show with valid nodes reuses
            // the cached entries.
            unsafe { s.concentration.observe(n, window, false) };
            if s.concentration.image.is_none() {
                s.concentration.image = snapshot().and_then(|bundle| bundle.concentration.clone());
            }
        }
        s.concentration.visible = show != 0;
        unsafe { s.concentration.log_visibility(n, show) };
        unsafe {
            s.concentration
                .refresh(n, CONCENTRATION_ENABLED.load(Ordering::Acquire))
        };
    });
    result
}

/// Compare a game-provided NUL-terminated name with a fixed byte string. The
/// name pointer is validated before every read.
unsafe fn name_equals(name: *const u8, expected: &[u8]) -> bool {
    if name.is_null() {
        return false;
    }
    let address = name as usize;
    if !readable(address) || !readable(address.saturating_add(expected.len())) {
        return false;
    }
    for (index, byte) in expected.iter().enumerate() {
        if unsafe { name.add(index).read() } != *byte {
            return false;
        }
    }
    unsafe { name.add(expected.len()).read() == 0 }
}

/// Native archive/name texture setter.
///
/// It captures the exact results `illust` clip from the game's own
/// `set_archive(clip, archive, "result_bg")` call (the runtime binds the custom
/// descriptor to that object), and rebinds a concentration texture immediately
/// after a native customization restore.
/// Diagnostic: copy a bounded printable C string for logging.
unsafe extern "system" fn set_archive_hook(clip: usize, archive: usize, name: *const u8) -> u8 {
    let f: SetArchive = unsafe { original("set_archive") };
    let result = unsafe { f(clip, archive, name) };
    if clip != 0 && archive != 0 && unsafe { name_equals(name, b"result_bg") } {
        RESULTS_CLIP.store(clip, Ordering::Release);
        RESULTS_STOCK_REBINDS.fetch_add(1, Ordering::Relaxed);
    }
    boundary(|| {
        if !CONCENTRATION_ENABLED.load(Ordering::Acquire) || !can_render() {
            return;
        }
        let Some(n) = NATIVE.get() else { return };
        let Some(s) = state() else { return };
        unsafe { s.concentration.reassert_after_native(n, clip) };
    });
    result
}

pub(super) fn attach(n: &Native) -> Result<()> {
    let mut hooks: Vec<(&'static str, *const ())> = vec![
        (
            "select_update",
            if n.profile.surface_data["select_update_extended"] != 0 {
                select_update as *const ()
            } else {
                select_update_short as *const ()
            },
        ),
        ("select_cleanup", select_cleanup as *const ()),
        ("select_delete", select_delete as *const ()),
        ("movie_draw", movie_draw as *const ()),
        ("cover_build", cover_build as *const ()),
        ("cover_init", cover_init as *const ()),
        ("cover_visual", cover_visual as *const ()),
        ("cover_delete", cover_delete as *const ()),
        ("graph_art", graph_art as *const ()),
        ("graph_reset", graph_reset as *const ()),
        ("graph_parent_reset", graph_parent_reset as *const ()),
        ("graph_storage_release", graph_storage_release as *const ()),
        ("sprite_release", sprite_release as *const ()),
    ];
    // The premium-area and concentration backgrounds are implemented only for
    // builds whose analysis provides every required entry and layout offset.
    // Other exact-build profiles keep the previous surface set.
    if n.profile.rvas.contains_key("premium_area_ctor")
        && n.profile.surface_data.contains_key("premium_area_base")
        && n.profile.surface_data.contains_key("premium_area_custom")
    {
        hooks.push(("premium_area_ctor", premium_area_ctor as *const ()));
    }
    // Captures the results illustration clip from the game's own archive bind
    // (all supported profiles have this entry) and restores concentration art.
    if n.profile.rvas.contains_key("set_archive") {
        hooks.push(("set_archive", set_archive_hook as *const ()));
    }
    if n.profile.rvas.contains_key("concentration_apply")
        && n.profile.rvas.contains_key("concentration_show")
        && n.profile.rvas.contains_key("concentration_layer")
    {
        hooks.push(("concentration_apply", concentration_apply as *const ()));
        hooks.push(("concentration_show", concentration_show as *const ()));
    }
    // Only the analyzed hash has these entries and live-code guards. Do not
    // apply the layout workaround to other builds based on version labels.
    for (name, detour) in [
        ("box_fill_update", box_fill_update as *const ()),
        ("box_outline_update", box_outline_update as *const ()),
        ("line_update", line_update as *const ()),
    ] {
        if n.profile.rvas.contains_key(name) {
            hooks.push((name, detour));
        }
    }
    let mut created = Vec::new();
    artwork::hooks(n, &mut hooks);
    let mut trampolines = Vec::new();
    let mut targets: Vec<_> = hooks
        .into_iter()
        .map(|(name, detour)| (name, n.base + n.profile.rvas[name], detour))
        .collect();
    if let Some(hook) = artwork::library_hook(n) {
        targets.push(hook);
    }
    for (name, address, detour) in targets {
        let target = address as *mut c_void;
        match unsafe { MinHook::create_hook(target, detour.cast_mut().cast()) } {
            Ok(original) => {
                created.push(target);
                trampolines.push((name, original as usize));
            }
            Err(_) => {
                for target in created {
                    let _ = unsafe { MinHook::remove_hook(target) };
                }
                return Err(Error::Native("create surface hooks"));
            }
        }
    }
    HOOKS
        .set(trampolines)
        .map_err(|_| Error::Native("surface hooks already published"))?;
    let enable = || -> Result<()> {
        for target in &created {
            unsafe { MinHook::queue_enable_hook(*target) }
                .map_err(|_| Error::Native("queue surface hook"))?;
        }
        unsafe { MinHook::apply_queued() }.map_err(|_| Error::Native("enable surface hooks"))
    };
    if let Err(e) = enable() {
        // Keep published trampolines allocated even if an activation was partial.
        for target in created {
            let _ = unsafe { MinHook::disable_hook(target) };
        }
        return Err(e);
    }
    Ok(())
}

#[cfg(test)]
mod primitive_tests {
    use super::*;
    use std::cell::RefCell;

    thread_local! {
        static COPIES: RefCell<Vec<[u8; 56]>> = const { RefCell::new(Vec::new()) };
    }

    unsafe extern "system" fn capture(this: usize) -> usize {
        let object = unsafe { (this as *const [u8; 56]).read_unaligned() };
        COPIES.with(|copies| copies.borrow_mut().push(object));
        0x12345678
    }

    #[test]
    fn primitive_workaround_preserves_fields_and_forwards_once() {
        for handle in [0_u32, 123] {
            for padding in [0_u32, 0x10000, u32::MAX] {
                let mut object = [0xa5_u8; 56];
                object[8..12].copy_from_slice(&handle.to_le_bytes());
                object[44..48].copy_from_slice(&padding.to_le_bytes());
                let mut expected = object;
                if handle != 0 {
                    expected[44..48].fill(0);
                }
                COPIES.with(|copies| copies.borrow_mut().clear());
                let result = unsafe { update_primitive(object.as_mut_ptr() as usize, capture) };
                assert_eq!(result, 0x12345678);
                assert_eq!(object, expected);
                COPIES.with(|copies| assert_eq!(*copies.borrow(), vec![expected]));
            }
        }
    }

    #[test]
    fn primitive_workaround_has_exact_build_and_entry_guards() {
        for profile in crate::profile::supported() {
            for (name, rva) in [
                ("box_fill_update", 0x339db0),
                ("box_outline_update", 0x3397f0),
                ("line_update", 0x33bb10),
            ] {
                if profile.sha256
                    == "c98e183ec81ecb9e1cb13c237f9c9cb0b3f65b6fdaff6fd5e6a526008c546534"
                {
                    assert_eq!(profile.rvas.get(name), Some(&rva));
                    assert!(profile.guards.iter().any(|g| g.rva == rva));
                } else {
                    assert!(!profile.rvas.contains_key(name));
                }
            }
        }
    }
}

#[cfg(test)]
mod character_tests {
    use super::*;
    use std::cell::RefCell;

    #[derive(Default)]
    struct Mock {
        visible: u8,
        fail: bool,
        writes: Vec<(u32, u32, usize)>,
        geometry: Vec<Vec<f32>>,
    }
    thread_local! { static MOCK: RefCell<Mock> = RefCell::new(Mock::default()); }

    unsafe extern "system" fn get(_: u32, property: u32, out: *mut u8) -> i32 {
        MOCK.with(|m| {
            let m = m.borrow();
            if m.fail {
                return -1;
            }
            if property == 4103 {
                unsafe { out.write(m.visible) };
            } else {
                let values: &[f32] = match property {
                    4104 => &[100.0, 200.0],
                    4123 => &[960.0, 540.0],
                    4099 => &[1.5, 2.0],
                    4106 => &[0.7, 0.8, 0.9, 0.5],
                    _ => return -1,
                };
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        values.as_ptr().cast::<u8>(),
                        out,
                        values.len() * 4,
                    )
                };
            }
            0
        })
    }
    unsafe extern "system" fn set(clip: u32, property: u32, value: usize) -> i32 {
        MOCK.with(|m| {
            let mut m = m.borrow_mut();
            m.writes.push((clip, property, value));
            if property == 4103 {
                m.visible = value as u8;
            }
        });
        0
    }
    unsafe extern "system" fn dimensions(_: usize, w: *mut i32, h: *mut i32) {
        unsafe {
            w.write(1920);
            h.write(1080);
        }
    }
    unsafe extern "system" fn position(_: usize, x: i32, y: i32) {
        MOCK.with(|m| m.borrow_mut().geometry.push(vec![x as f32, y as f32]));
    }
    unsafe extern "system" fn scale(_: usize, x: f32, y: f32) {
        MOCK.with(|m| m.borrow_mut().geometry.push(vec![x, y]));
    }
    unsafe extern "system" fn color(_: usize, a: f32, r: f32, g: f32, b: f32) {
        MOCK.with(|m| m.borrow_mut().geometry.push(vec![a, r, g, b]));
    }

    #[test]
    fn character_mask_restores_native_state_and_rejects_retired_or_changed_owners() {
        for mut profile in profile::supported() {
            MOCK.with(|m| {
                *m.borrow_mut() = Mock {
                    visible: 1,
                    ..Default::default()
                }
            });
            let imports = [get as *const () as usize, set as *const () as usize];
            profile
                .surface_data
                .insert("afp_property_get".into(), imports.as_ptr() as usize);
            profile
                .surface_data
                .insert(
                    "afp_property_set".into(),
                    unsafe { imports.as_ptr().add(1) } as usize,
                );
            let n = Native { base: 0, profile };
            let mut scene = [0usize; 40];
            let mut layer = [0usize; 77];
            scene[0] = n.profile.surface_data["select_vtable"];
            layer[0] = n.profile.surface_data["layer_vtable"];
            layer[1] = 123;
            layer[66] = 42; // root clip at +528
            let index = n.profile.surface_data["select_frame_chara"] / 8;
            scene[index] = layer.as_ptr() as usize;
            let owner = scene.as_ptr() as usize;
            let mut mask = FrameChara::default();
            unsafe {
                mask.hide(&n, owner);
                mask.hide(&n, owner);
            }
            assert_eq!(MOCK.with(|m| m.borrow().visible), 0);
            unsafe { mask.restore(&n, owner) };
            assert_eq!(MOCK.with(|m| m.borrow().visible), 1);
            assert_eq!(
                MOCK.with(|m| m.borrow().writes.clone()),
                vec![(42, 4103, 0), (42, 4126, 1), (42, 4103, 1), (42, 4126, 1)]
            );
            // Native hide is never converted into a forced show.
            MOCK.with(|m| m.borrow_mut().visible = 0);
            unsafe {
                mask.hide(&n, owner);
                mask.restore(&n, owner);
            }
            assert_eq!(MOCK.with(|m| m.borrow().visible), 0);
            // Release observer clears the borrowed root before pool reuse.
            unsafe { mask.hide(&n, owner) };
            mask.released(layer.as_ptr() as usize);
            let before = MOCK.with(|m| m.borrow().writes.len());
            unsafe { mask.restore(&n, owner) };
            assert_eq!(MOCK.with(|m| m.borrow().writes.len()), before);
            unsafe { mask.hide(&n, owner) };
            layer[66] = 43;
            let before = MOCK.with(|m| m.borrow().writes.len());
            unsafe { mask.restore(&n, owner) };
            assert_eq!(MOCK.with(|m| m.borrow().writes.len()), before);
            // Wrong scene type and absent layer must not call AFP.
            scene[0] = 0;
            unsafe { mask.hide(&n, owner) };
            assert!(mask.hidden.is_none());
            scene[0] = n.profile.surface_data["select_vtable"];
            scene[index] = 0;
            unsafe { mask.hide(&n, owner) };
            assert!(mask.hidden.is_none());
            scene[index] = layer.as_ptr() as usize;
            std::hint::black_box(&scene);
            MOCK.with(|m| m.borrow_mut().fail = true);
            unsafe { mask.hide(&n, owner) };
            assert!(mask.hidden.is_none());
        }
    }

    #[test]
    fn character_sprite_uses_native_geometry_and_color_without_game_calls() {
        MOCK.with(|m| *m.borrow_mut() = Mock::default());
        let imports = [get as *const () as usize];
        let mut profile = profile::supported().into_iter().next().unwrap();
        profile
            .surface_data
            .insert("afp_property_get".into(), imports.as_ptr() as usize);
        for (name, ptr) in [
            ("layer_dimensions", dimensions as *const ()),
            ("sprite_position", position as *const ()),
            ("sprite_origin", position as *const ()),
            ("sprite_scale", scale as *const ()),
            ("sprite_alpha", color as *const ()),
        ] {
            profile.rvas.insert(name.into(), ptr as usize);
        }
        let n = Native { base: 0, profile };
        let mut chara = FrameChara::default();
        assert!(unsafe { chara.place_sprite(&n, 123, 42, 456) });
        assert_eq!(
            MOCK.with(|m| m.borrow().geometry.clone()),
            vec![
                vec![960.0, 540.0],
                vec![100.0, 200.0],
                vec![1.5, 2.0],
                vec![0.5, 0.7, 0.8, 0.9]
            ]
        );
        // Identical native geometry must not write the AFP setters again.
        let writes = MOCK.with(|m| m.borrow().geometry.len());
        assert!(unsafe { chara.place_sprite(&n, 123, 42, 456) });
        assert_eq!(MOCK.with(|m| m.borrow().geometry.len()), writes);
        MOCK.with(|m| m.borrow_mut().fail = true);
        assert!(!unsafe { chara.place_sprite(&n, 123, 42, 456) });
    }

    #[test]
    fn background_transform_is_read_from_native_properties_not_rebuilt() {
        MOCK.with(|m| *m.borrow_mut() = Mock::default());
        let imports = [get as *const () as usize];
        let mut profile = profile::supported().into_iter().next().unwrap();
        profile
            .surface_data
            .insert("afp_property_get".into(), imports.as_ptr() as usize);
        for (name, ptr) in [
            ("sprite_position", position as *const ()),
            ("sprite_origin", position as *const ()),
            ("sprite_scale", scale as *const ()),
            ("sprite_alpha", color as *const ()),
        ] {
            profile.rvas.insert(name.into(), ptr as usize);
        }
        let n = Native { base: 0, profile };
        let transform = unsafe { read_transform(&n, 42) }.expect("native transform");
        assert_eq!(transform.position, [100.0, 200.0]);
        assert_eq!(transform.origin, [960.0, 540.0]);
        assert_eq!(transform.scale, [1.5, 2.0]);
        assert_eq!(transform.color, [0.7, 0.8, 0.9, 0.5]);
        unsafe { apply_transform(&n, 7, &transform) };
        assert_eq!(
            MOCK.with(|m| m.borrow().geometry.clone()),
            vec![
                vec![960.0, 540.0],
                vec![100.0, 200.0],
                vec![1.5, 2.0],
                vec![0.5, 0.7, 0.8, 0.9]
            ]
        );
        // Non-finite native geometry must keep stock art instead of panicking.
        MOCK.with(|m| m.borrow_mut().fail = true);
        assert!(unsafe { read_transform(&n, 42) }.is_none());
    }
}

#[cfg(test)]
mod premium_lifetime_tests {
    use super::*;

    #[test]
    fn native_retirement_discards_both_saved_handles_and_blocks_rebuild() {
        let mut area = PremiumArea {
            object: 100,
            base: 200,
            custom: 300,
            hidden: [
                Some(NativeClip {
                    handle: 42,
                    visible: 1,
                }),
                None,
            ],
            pending: true,
            ..Default::default()
        };
        assert!(!area.retire_source(0));
        assert!(!area.retire_source(400));
        assert_eq!(area.base, 200);
        assert!(area.retire_source(200));
        assert_eq!((area.base, area.custom), (0, 0));
        assert!(area.hidden.iter().all(Option::is_none));
        assert!(!area.pending);
        // Keep the owner marker so the next graphics tick disposes our sprite.
        assert_eq!(area.object, 100);
        assert!(!area.retire_source(300));
    }
}
