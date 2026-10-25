//! Selection-only Qpro replacement. Exact-build evidence: docs/iidx34-mmd-qpro.md.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Owner {
    layer: usize,
    handle: u32,
    root: u32,
}
#[derive(Clone, Copy)]
struct Mask {
    owner: Owner,
    visible: u8,
}

pub(super) struct Replacement {
    sprites: [Option<Sprite>; 2],
    hidden: [Option<Mask>; 2],
    identity: Option<Arc<Prepared>>,
    logged: [bool; 2],
    failed: bool,
}
impl Replacement {
    pub(super) const fn new() -> Self {
        Self {
            sprites: [None, None],
            hidden: [None, None],
            identity: None,
            logged: [false; 2],
            failed: false,
        }
    }
    pub(super) fn released(&mut self, layer: usize) {
        for side in 0..2 {
            if self.hidden[side].is_some_and(|m| m.owner.layer == layer) {
                self.hidden[side] = None;
            }
            if let Some(sprite) = &mut self.sprites[side]
                && sprite.node == layer
            {
                // Metadata only. GPU leases are retired on the next graphics callback.
                sprite.node = 0;
            }
        }
    }
    pub(super) unsafe fn restore(&mut self, n: &Native, scene: usize) {
        for side in 0..2 {
            if let Some(sprite) = &self.sprites[side]
                && sprite.node != 0
            {
                unsafe { visible(sprite.node, false) };
            }
            if let Some(mask) = self.hidden[side].take()
                && unsafe { current(n, scene, side as u32) } == Some(mask.owner)
            {
                unsafe { set_clip_visible(n, mask.owner.root, mask.visible) };
            }
        }
    }
    pub(super) unsafe fn update(&mut self, n: &Native, scene: usize) {
        // Also handles a model becoming disabled between native update and draw.
        unsafe { self.restore(n, scene) };
        let Some(component) = component(n, scene) else {
            return;
        };
        let Some(model) = crate::mmd::snapshot().filter(|_| MMD_QPRO.load(Ordering::Acquire))
        else {
            self.sprites = [None, None];
            self.identity = None;
            self.failed = false;
            return;
        };
        if !self
            .identity
            .as_ref()
            .is_some_and(|p| Arc::ptr_eq(p, &model.first))
        {
            self.sprites = [None, None];
            self.identity = Some(model.first.clone());
            self.failed = false;
        }
        if self.failed
            || !crate::mmd::ACTIVE.load(Ordering::Acquire)
            || byte(component + 16) != Some(1)
            || byte(component + 17) != Some(1)
        {
            return;
        }
        let Some(layout) = pointer_field(component, 8).filter(|l| layer(n, *l).is_some()) else {
            return;
        };
        let count: unsafe extern "system" fn() -> u32 = unsafe { n.function("qpro_player_count") };
        let count = unsafe { count() };
        if !(1..=2).contains(&count) {
            return;
        }
        let joined: unsafe extern "system" fn(u32) -> u8 = unsafe { n.function("reaction_joined") };
        let find: FindClip = unsafe { n.function("find_clip") };
        // One GPU render/straight-alpha conversion is shared by both placements.
        let Ok(media) = (unsafe { GpuMedia::acquire_model(n, model.clone()) }) else {
            return;
        };
        if unsafe { media.update() }.is_err() {
            self.failed = true;
            return;
        }
        let Ok(texture) = (unsafe { media.straight_model_texture(n) }) else {
            self.failed = true;
            return;
        };
        for side in 0..2 {
            if unsafe { joined(side as u32) } == 0 {
                continue;
            }
            let Some(owner) = (unsafe { current(n, scene, side as u32) }) else {
                continue;
            };
            let name: &[u8] = if count == 1 {
                b"qpro\0"
            } else if side == 0 {
                b"qpro_01p\0"
            } else {
                b"qpro_02p\0"
            };
            let anchor = unsafe { find(layout, name.as_ptr()) };
            if anchor == 0 || !readable(anchor + 515) {
                continue;
            }
            let anchor_id = unsafe { ((anchor + 512) as *const u32).read_unaligned() };
            let Some(mut transform) = (unsafe { anchor_transform(n, anchor_id) }) else {
                continue;
            };
            let dimensions: unsafe extern "system" fn(usize, *mut i32, *mut i32) =
                unsafe { n.function("layer_dimensions") };
            let (mut width, mut height) = (0, 0);
            unsafe { dimensions(owner.layer, &mut width, &mut height) };
            if !(1..=16384).contains(&width) || !(1..=16384).contains(&height) {
                continue;
            }
            let source = [model.first.width as f32, model.first.height as f32];
            place(
                &mut transform,
                [width as f32, height as f32],
                source,
                model.settings.qpro_height,
            );
            if self.sprites[side].as_ref().is_some_and(|s| s.node == 0) {
                self.sprites[side] = None;
            }
            if self.sprites[side].is_none() {
                let priority = n.profile.surface_data["qpro_priority"] as u32
                    + if count == 2 { side as u32 } else { 0 };
                self.sprites[side] = unsafe {
                    Sprite::from_texture(n, texture.clone(), priority, Some(media.clone()))
                }
                .ok();
            }
            let Some(sprite) = &self.sprites[side] else {
                continue;
            };
            unsafe { apply_transform(n, sprite.node, &transform) };
            let Some(saved) = (unsafe { clip_visible(n, owner.root) }) else {
                continue;
            };
            // Publish replacement only after all reads, upload and sprite allocation succeeded.
            if unsafe { set_clip_visible(n, owner.root, 0) } {
                self.hidden[side] = Some(Mask {
                    owner,
                    visible: saved,
                });
                unsafe { visible(sprite.node, saved != 0) };
                if !self.logged[side] {
                    self.logged[side] = true;
                    event(Event::Diag(format!(
                        "MMD Qpro P{}: native={}x{} position={:?} origin={:?} scale={:?}",
                        side + 1,
                        width,
                        height,
                        transform.position,
                        transform.origin,
                        transform.scale
                    )));
                }
            }
        }
    }
}

fn byte(address: usize) -> Option<u8> {
    readable(address).then(|| unsafe { (address as *const u8).read() })
}
fn component(n: &Native, scene: usize) -> Option<usize> {
    let offset = *n.profile.surface_data.get("qpro_component")?;
    if scene == 0 || pointer_field(scene, 0)? != n.base + n.profile.surface_data["select_vtable"] {
        return None;
    }
    let component = scene.checked_add(offset)?;
    (pointer_field(component, 0)? == n.base + n.profile.surface_data["qpro_vtable"])
        .then_some(component)
}
fn layer(n: &Native, object: usize) -> Option<Owner> {
    if object == 0
        || pointer_field(object, 0)? != n.base + n.profile.surface_data["layer_vtable"]
        || !readable(object.checked_add(531)?)
    {
        return None;
    }
    let handle = unsafe { ((object + 8) as *const u32).read() };
    let root = unsafe { ((object + 528) as *const u32).read() };
    (handle != 0 && (root as i32) >= 0).then_some(Owner {
        layer: object,
        handle,
        root,
    })
}
unsafe fn current(n: &Native, scene: usize, side: u32) -> Option<Owner> {
    component(n, scene)?;
    let manager: unsafe extern "system" fn() -> usize = unsafe { n.function("qpro_manager") };
    let player: unsafe extern "system" fn(usize, u32) -> usize =
        unsafe { n.function("qpro_player") };
    let character: unsafe extern "system" fn(usize, u32) -> usize =
        unsafe { n.function("qpro_layer") };
    let manager = unsafe { manager() };
    if manager == 0 {
        return None;
    }
    let player = unsafe { player(manager, side) };
    if player == 0 {
        return None;
    }
    layer(n, unsafe { character(player, 0) })
}
unsafe fn anchor_transform(n: &Native, id: u32) -> Option<Transform> {
    if (id as i32) < 0 {
        return None;
    }
    let get: PropertyGet = unsafe {
        std::mem::transmute(field(
            n.base + n.profile.surface_data["afp_property_get"],
            0,
        ))
    };
    let mut t = Transform {
        position: [0.; 2],
        origin: [0.; 2],
        scale: [1.; 2],
        color: [1.; 4],
    };
    // Same properties as the native Qpro transfer, including 4109 scale.
    for (key, values) in [
        (4104, t.position.as_mut_slice()),
        (4123, t.origin.as_mut_slice()),
        (4109, t.scale.as_mut_slice()),
        (4106, t.color.as_mut_slice()),
    ] {
        if unsafe { get(id, key, values.as_mut_ptr().cast()) } < 0
            || !values.iter().all(|v| v.is_finite() && v.abs() <= 100000.)
        {
            return None;
        }
    }
    Some(t)
}
fn place(t: &mut Transform, native: [f32; 2], source: [f32; 2], height: f32) {
    // Preserve the native anchor's relative origin, fitting the model into a
    // square of the requested height. This keeps differing PMX aspects intact.
    let ratio = height / source[0].max(source[1]);
    for i in 0..2 {
        t.origin[i] = t.origin[i] / native[i] * source[i];
        t.scale[i] *= ratio;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    thread_local! {
        static CURRENT: Cell<usize> = const { Cell::new(0) };
        static WRITES: RefCell<Vec<(u32,u32,usize)>> = const { RefCell::new(Vec::new()) };
    }
    unsafe extern "system" fn manager() -> usize {
        1
    }
    unsafe extern "system" fn player(_: usize, _: u32) -> usize {
        1
    }
    unsafe extern "system" fn character(_: usize, _: u32) -> usize {
        CURRENT.with(Cell::get)
    }
    unsafe extern "system" fn set(root: u32, property: u32, value: usize) -> i32 {
        WRITES.with(|v| v.borrow_mut().push((root, property, value)));
        0
    }
    #[test]
    fn qpro_restore_rejects_recycled_layer_scene_and_root() {
        let mut profile = profile::supported()
            .into_iter()
            .find(|p| p.surface_data.contains_key("qpro_component"))
            .unwrap();
        let imports = [set as *const () as usize];
        profile
            .surface_data
            .insert("afp_property_set".into(), imports.as_ptr() as usize);
        for (name, function) in [
            ("qpro_manager", manager as *const ()),
            ("qpro_player", player as *const ()),
            ("qpro_layer", character as *const ()),
        ] {
            profile.rvas.insert(name.into(), function as usize);
        }
        let n = Native { base: 0, profile };
        let mut scene = vec![0usize; 1100];
        let mut native = [0usize; 77];
        scene[0] = n.profile.surface_data["select_vtable"];
        let component_index = n.profile.surface_data["qpro_component"] / 8;
        scene[component_index] = n.profile.surface_data["qpro_vtable"];
        native[0] = n.profile.surface_data["layer_vtable"];
        native[1] = 123;
        native[66] = 42;
        let address = scene.as_ptr() as usize;
        let layer_address = native.as_ptr() as usize;
        CURRENT.with(|c| c.set(layer_address));
        let owner = unsafe { current(&n, address, 0) }.unwrap();
        let mut state = Replacement::new();
        for mutation in 0..5 {
            native[1] = 123;
            native[66] = 42;
            scene[component_index] = n.profile.surface_data["qpro_vtable"];
            WRITES.with(|v| v.borrow_mut().clear());
            state.hidden[0] = Some(Mask { owner, visible: 1 });
            match mutation {
                1 => native[1] = 124,
                2 => native[66] = 43,
                3 => scene[component_index] = 0,
                4 => state.released(layer_address),
                _ => {}
            }
            unsafe { state.restore(&n, address) };
            let writes = WRITES.with(|v| v.borrow().clone());
            if mutation == 0 {
                assert_eq!(writes, [(42, 4103, 1), (42, 4126, 1)]);
            } else {
                assert!(writes.is_empty());
            }
            assert!(state.hidden[0].is_none());
        }
    }
    #[test]
    fn qpro_fitting_keeps_anchor_and_aspect_independent_of_large_placement() {
        let mut t = Transform {
            position: [1000., 650.],
            origin: [128., 256.],
            scale: [-0.8, 0.8],
            color: [1.; 4],
        };
        place(&mut t, [256., 256.], [512., 768.], 360.);
        assert_eq!(t.position, [1000., 650.]);
        assert_eq!(t.origin, [256., 768.]);
        assert_eq!(t.scale, [-0.375, 0.375]);
    }
}
