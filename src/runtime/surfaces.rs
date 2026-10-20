//! Exact-build surface hooks. See docs/*-background-surfaces.md for ABI evidence.
use super::*;
use minhook::MinHook;

#[derive(Default)]
pub(super) struct Bundle {
    select: Option<Arc<Prepared>>,
    upper: Option<Arc<Prepared>>,
    lower: Option<Arc<Prepared>>,
    graph: Option<Arc<Prepared>>,
}

static LATEST: Mutex<Option<Arc<Bundle>>> = Mutex::new(None);
static CHARACTER_IMAGE: Mutex<Option<Arc<Prepared>>> = Mutex::new(None);
static STATE: Mutex<Surfaces> = Mutex::new(Surfaces::new());
static HOOKS: OnceLock<Vec<(&'static str, usize)>> = OnceLock::new();

/// Worker publishes already decoded, independently sourced surface canvases.
pub(super) fn publish(images: &[Option<Arc<Prepared>>; 5]) {
    *LATEST.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(Bundle {
        select: images[1].clone(),
        upper: images[2].clone(),
        lower: images[3].clone(),
        graph: images[4].clone(),
    }));
}

pub(super) fn publish_character(image: Option<Arc<Prepared>>) {
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

/// Only the character's root clip is masked. Never cache an owning native pointer.
#[derive(Default)]
struct FrameChara {
    hidden: Option<HiddenFrameChara>,
    image: Option<Arc<Prepared>>,
    sprite: Option<Sprite>,
    layer: usize,
    attempted: bool,
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

    fn released(&mut self, layer: usize) -> Option<Texture> {
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
            self.attempted = false;
        }
        if hide {
            unsafe { self.hide(n, scene) };
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
                    event(Event::Surface("frame character"));
                }
                Err(_) => event(Event::Fault(
                    "Frame character texture upload failed; using stock",
                )),
            }
        }
        if let Some(sprite) = &self.sprite {
            if !unsafe { Self::place_sprite(n, layer, clip, sprite.node) } {
                return;
            }
            unsafe { self.hide(n, scene) };
            if self.hidden.is_some_and(|saved| saved.visible != 0) {
                unsafe { visible(self.sprite.as_ref().unwrap().node, true) };
            }
        }
    }

    unsafe fn place_sprite(n: &Native, layer: usize, clip: u32, sprite: usize) -> bool {
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
    texture: Option<Texture>,
}

impl Sprite {
    unsafe fn create(n: &Native, image: &Prepared, priority: u32) -> Result<Self> {
        let texture = unsafe { Texture::upload(n, image) }?;
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

struct Surfaces {
    scene: usize,
    select_choice: Option<Arc<Bundle>>,
    select_texture: Option<Texture>,
    select_attempted: bool,
    pairs: Vec<PairInfo>,
    covers: Vec<Cover>,
    graphs: Vec<Graph>,
    frame_chara: FrameChara,
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
            frame_chara: FrameChara {
                hidden: None,
                image: None,
                sprite: None,
                layer: 0,
                attempted: false,
            },
        }
    }

    fn leave_select(&mut self, scene: usize) {
        if self.scene == scene {
            self.frame_chara = FrameChara::default();
            self.scene = 0;
            self.select_texture = None;
            self.select_choice = None;
            self.select_attempted = false;
            request_images();
        }
    }

    unsafe fn invalidate(&mut self) {
        if let Some(n) = NATIVE.get() {
            unsafe { self.frame_chara.restore(n, self.scene) };
        }
        self.frame_chara.sprite = None;
        self.frame_chara.attempted = false;
        self.select_texture = None;
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
    if let Some(mut s) = state() {
        unsafe { s.invalidate() };
    } else {
        ENABLED.store(false, Ordering::Release);
        event(Event::Fault("surface invalidation reentry"));
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

fn observe_select(this: usize) {
    boundary(|| {
        let n = NATIVE.get().unwrap();
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
            }
            unsafe {
                s.frame_chara
                    .update(n, this, HIDE_FRAME_CHARA.load(Ordering::Acquire))
            };
        }
    });
}

fn restore_frame_chara(scene: usize) {
    boundary(|| {
        // Restore even after feature disable/fault, before native update or teardown.
        if unsafe { GetCurrentThreadId() } == OWNER.load(Ordering::Acquire)
            && let Some(n) = NATIVE.get()
            && let Some(mut s) = state()
        {
            unsafe { s.frame_chara.restore(n, scene) };
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
        if s.scene == 0 {
            return;
        }
        let n = NATIVE.get().unwrap();
        if !s.select_attempted {
            s.select_attempted = true;
            if let Some(image) = s.select_choice.as_ref().and_then(|b| b.select.as_ref()) {
                s.select_texture = unsafe { Texture::upload(n, image) }.ok();
                if s.select_texture.is_none() {
                    event(Event::Fault("selection texture upload failed; using stock"));
                } else {
                    event(Event::Surface("song selection"));
                }
            }
        }
        if let Some(texture) = &s.select_texture {
            let alpha = if n.profile.surface_data["selection_fade"] != 0 {
                unsafe { ((this + 40) as *const f32).read() }
            } else {
                1.0
            };
            drawn = unsafe { draw_selection(n, texture, alpha) }.is_ok();
        }
    });
    if drawn { 0 } else { unsafe { f(this) } }
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

unsafe fn draw_selection(n: &Native, texture: &Texture, alpha: f32) -> Result<()> {
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

unsafe fn make_cover(n: &Native, image: &Prepared, info: PairInfo) -> Result<Sprite> {
    let sprite = unsafe { Sprite::create(n, image, 108) }?;
    let clip: Clip = unsafe { n.function("sprite_clip") };
    let origin: Position = unsafe { n.function("sprite_origin") };
    let scale: Scale = unsafe { n.function("sprite_scale") };
    unsafe {
        clip(sprite.node, [0, 0, 1920, 723].as_ptr());
        origin(sprite.node, 0, 0);
        scale(sprite.node, 1.0, 1.0);
        if info.mirror {
            scale(sprite.node, -1.0, 1.0);
            origin(sprite.node, 216, 0);
        }
        if info.flip {
            scale(sprite.node, 1.0, -1.0);
            origin(sprite.node, 0, 723);
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
                g.sprite = unsafe { Sprite::create(n, image, 145) }.ok();
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
    let mut texture = None;
    boundary(|| {
        if let Some(mut s) = state() {
            texture = s.frame_chara.released(this);
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

pub(super) fn attach(n: &Native) -> Result<()> {
    let hooks = [
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
    let mut created = Vec::new();
    let mut trampolines = Vec::new();
    for (name, detour) in hooks {
        let target = (n.base + n.profile.rvas[name]) as *mut c_void;
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
        assert!(unsafe { FrameChara::place_sprite(&n, 123, 42, 456) });
        assert_eq!(
            MOCK.with(|m| m.borrow().geometry.clone()),
            vec![
                vec![960.0, 540.0],
                vec![100.0, 200.0],
                vec![1.5, 2.0],
                vec![0.5, 0.7, 0.8, 0.9]
            ]
        );
        MOCK.with(|m| m.borrow_mut().fail = true);
        assert!(!unsafe { FrameChara::place_sprite(&n, 123, 42, 456) });
    }
}
