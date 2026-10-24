//! GE2D substitution keeps the original work record and allocator untouched.
//! See docs/iidx34-custom-art.md for the exact library and queue-copy evidence.
use super::*;
use std::collections::BTreeMap;

struct Library {
    base: usize,
    id: usize,
    info: usize,
    draw: usize,
}
static LIBRARY: OnceLock<Library> = OnceLock::new();
static ACTIVE: AtomicBool = AtomicBool::new(false);
struct Replacement {
    original_geometry: usize,
    geometry: Vec<u8>,
    region: Box<[usize; 3]>,
    _binding: Box<[u32; 4]>,
    media: Arc<GpuMedia>,
}
struct Cache {
    revision: u64,
    items: BTreeMap<u32, Arc<Replacement>>,
}
static CACHE: Mutex<Cache> = Mutex::new(Cache {
    revision: 0,
    items: BTreeMap::new(),
});

/// Called at attachment, never from a graphics callback. A mismatched optional
/// library disables this adapter without weakening the main game's guards.
pub(super) fn hook(n: &Native) -> Option<(&'static str, usize, *const ())> {
    use sha2::{Digest, Sha256};
    let p = n.profile.artwork_library.as_ref()?;
    let name: Vec<u16> = p.name.encode_utf16().chain(Some(0)).collect();
    let base = unsafe { GetModuleHandleW(name.as_ptr()) } as usize;
    let verify = || -> Result<()> {
        if base == 0 {
            return Err(Error::Build("artwork library not loaded"));
        }
        let data = std::fs::read(module_path(base)?)?;
        if hex::encode(Sha256::digest(&data)) != p.sha256
            || profile::image_size(&data)? != p.image_size
        {
            return Err(Error::Build("unsupported artwork library"));
        }
        for guard in &p.guards {
            let live = unsafe {
                std::slice::from_raw_parts((base + guard.rva) as *const u8, guard.hex.len() / 2)
            };
            guard.check("artwork library", live)?;
        }
        Ok(())
    };
    if let Err(e) = verify() {
        event(Event::Diag(format!("shape replacement unavailable: {e}")));
        return None;
    }
    let library = Library {
        base,
        id: p.rvas["shape_id"],
        info: p.rvas["shape_info"],
        draw: p.rvas["artwork_shape_draw"],
    };
    let address = base + library.draw;
    LIBRARY.set(library).ok()?;
    Some(("artwork_shape_draw", address, draw as *const ()))
}

pub(super) fn invalidate() {
    // Only called by the established graphics owner, before device reset.
    ACTIVE.store(false, Ordering::Release);
    if let Ok(mut cache) = CACHE.try_lock() {
        cache.items.clear();
        cache.revision = 0;
    }
}

pub(super) unsafe fn update(n: &Native, assets: &GenericAssets, revision: u64) -> Option<bool> {
    let Some(library) = LIBRARY.get() else {
        return Some(false);
    };
    let mut cache = CACHE.try_lock().ok()?;
    if cache.revision != revision {
        let archives = ARCHIVES.try_lock().ok()?;
        let lookup: unsafe extern "system" fn(u32, *const u8) -> u32 =
            unsafe { std::mem::transmute(library.base + library.id) };
        let info: unsafe extern "system" fn(*mut u64, u32) -> i32 =
            unsafe { std::mem::transmute(library.base + library.info) };
        let mut items = BTreeMap::new();
        for (art, asset) in assets {
            if !scenes::active(art) {
                continue;
            }
            let Some(name) = art.texture.strip_prefix("@shape/").filter(|_| art.enabled) else {
                continue;
            };
            let Some(asset) = asset else {
                continue;
            };
            let name: Vec<u8> = name.bytes().chain(Some(0)).collect();
            for archive in archives.iter().filter(|a| a.path == art.archive) {
                let id = unsafe { lookup(archive.id, name.as_ptr()) };
                if PROFILE.load(Ordering::Relaxed) {
                    event(Event::Diag(format!(
                        "shape lookup {} {} -> {id:#x}",
                        art.archive, art.texture
                    )));
                }
                if id & 0x7800_0000 != 0x1800_0000 {
                    continue;
                }
                let mut view = [0u64; 4];
                if unsafe { info(view.as_mut_ptr(), id) } != 0 || view[1] == 0 {
                    continue;
                }
                let pointer = view[1] as usize;
                if let Some(previous) = cache.items.get(&id)
                    && previous.original_geometry == pointer
                    && Arc::ptr_eq(&previous.media.identity, &asset.first())
                {
                    items.insert(id, previous.clone());
                    continue;
                }
                let Some(header) = read_live::<52>(pointer) else {
                    continue;
                };
                let length = u32::from_le_bytes(header[12..16].try_into().unwrap()) as usize;
                if !(52..=8 * 1024 * 1024).contains(&length) {
                    continue;
                }
                // Native shape info has completed native loading/UV conversion.
                // The archive metadata lock prevents concurrent package retirement.
                let bytes = unsafe { std::slice::from_raw_parts(pointer as *const u8, length) };
                let pixels = asset.first();
                let Some(geometry) =
                    textured_geometry(bytes, [pixels.width as f32, pixels.height as f32])
                else {
                    if PROFILE.load(Ordering::Relaxed) {
                        event(Event::Diag(format!(
                            "shape geometry rejected id={id:#x} header={}",
                            hex::encode(header)
                        )));
                    }
                    continue;
                };
                let Ok(media) = (unsafe { GpuMedia::acquire(n, asset) }) else {
                    continue;
                };
                // bm2dx's binder first checks its direct registered texture table;
                // AfpTextureEx +16 is such a handle, before the AFP-ID fallback.
                let binding = Box::new([
                    media.texture.wrapper[2] as u32,
                    0,
                    media.texture.width,
                    media.texture.height,
                ]);
                let region = Box::new([0, 0, binding.as_ptr() as usize]);
                items.insert(
                    id,
                    Arc::new(Replacement {
                        original_geometry: pointer,
                        geometry,
                        region,
                        _binding: binding,
                        media,
                    }),
                );
                event(Event::Diag(format!(
                    "artwork shape archive={} name={} id={id:#x}",
                    art.archive, art.texture
                )));
            }
        }
        cache.items = items;
        cache.revision = revision;
        ACTIVE.store(!cache.items.is_empty(), Ordering::Release);
    }
    let mut dynamic = false;
    for item in cache.items.values() {
        dynamic |= item.media.dynamic;
        let _ = unsafe { item.media.update() };
    }
    Some(dynamic)
}

type Draw = unsafe extern "system" fn(usize, usize, usize, usize) -> usize;
unsafe extern "system" fn draw(work: usize, multiply: usize, add: usize, params: usize) -> usize {
    let original: Draw = unsafe { original("artwork_shape_draw") };
    if !ACTIVE.load(Ordering::Acquire) || !can_render() || work == 0 {
        return unsafe { original(work, multiply, add, params) };
    }
    let started = PROFILE.load(Ordering::Relaxed).then(Instant::now);
    let id = unsafe { (work as *const u32).read_unaligned() };
    let item = CACHE
        .try_lock()
        .ok()
        .and_then(|c| c.items.get(&id).cloned());
    let Some(item) = item.filter(|item| unsafe { field(work, 16) } == item.original_geometry && !item.media.failed.load(Ordering::Acquire)) else {
        if let Some(started) = started { profile_extra(1, started); }
        return unsafe { original(work, multiply, add, params) };
    };
    // Native renderer consumes this temporary record synchronously. Its queue
    // copies packed vertices/material IDs; it does not retain these pointers.
    // Native masks, transforms, blend/add colors and primitive ordering remain.
    let mut local = unsafe { (work as *const [u64; 6]).read_unaligned() };
    let bytes = local.as_mut_ptr().cast::<u8>();
    unsafe {
        bytes.add(12).cast::<u16>().write_unaligned(1);
    }
    local[2] = item.geometry.as_ptr() as u64;
    local[3] = item.region.as_ptr() as u64;
    if let Some(started) = started {
        profile_extra(1, started);
    }
    unsafe { original(local.as_ptr() as usize, multiply, add, params) }
}

/// Preserve the polygon/index topology and append planar UVs. This adapter is
/// intentionally limited to fully untextured GE2D resources; existing raster
/// geometry continues through the lower-overhead atlas substitution.
fn textured_geometry(input: &[u8], source: [f32; 2]) -> Option<Vec<u8>> {
    fn word(b: &[u8], at: usize) -> Option<usize> {
        Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?) as usize)
    }
    fn dword(b: &[u8], at: usize) -> Option<usize> {
        Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?) as usize)
    }
    let vertices = word(input, 20)?;
    let draws = word(input, 28)?;
    let positions = dword(input, 32)?;
    let records = dword(input, 48)?;
    if vertices == 0 || draws == 0 || word(input, 22)? != 0 || dword(input, 12)? != input.len() {
        return None;
    }
    let points = input.get(positions..positions.checked_add(vertices * 8)?)?;
    let records_bytes = input.get(records..records.checked_add(draws * 16)?)?;
    let mut bounds = [
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    ];
    let mut xy = Vec::with_capacity(vertices);
    for p in points.as_chunks::<8>().0 {
        let x = f32::from_le_bytes(p[..4].try_into().ok()?);
        let y = f32::from_le_bytes(p[4..].try_into().ok()?);
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        bounds[0] = bounds[0].min(x);
        bounds[1] = bounds[1].min(y);
        bounds[2] = bounds[2].max(x);
        bounds[3] = bounds[3].max(y);
        xy.push([x, y]);
    }
    if bounds[2] <= bounds[0] || bounds[3] <= bounds[1] {
        return None;
    }
    for record in records_bytes.as_chunks::<16>().0 {
        // Current inventory's untextured resources all have flags9. Reject
        // undocumented vertex-color/dual-texture formats rather than guessing.
        if record[0] != 4 || record[1] != 9 {
            return None;
        }
        let count = word(record, 4)?;
        let indices = dword(record, 12)?;
        for index in input
            .get(indices..indices.checked_add(count * 2)?)?
            .as_chunks::<2>()
            .0
        {
            if usize::from(u16::from_le_bytes(*index)) >= vertices {
                return None;
            }
        }
    }
    let mut result = input.to_vec();
    let uv_offset = result.len();
    let [sx, sy, ox, oy] = crop(
        source,
        [bounds[2] - bounds[0], bounds[3] - bounds[1]],
        false,
    );
    for [x, y] in xy {
        result.extend_from_slice(
            &(ox + sx * (x - bounds[0]) / (bounds[2] - bounds[0])).to_le_bytes(),
        );
        result.extend_from_slice(
            &(oy + sy * (y - bounds[1]) / (bounds[3] - bounds[1])).to_le_bytes(),
        );
    }
    let length = u32::try_from(result.len()).ok()?;
    result[12..16].copy_from_slice(&length.to_le_bytes());
    result[22..24].copy_from_slice(&(vertices as u16).to_le_bytes());
    result[36..40].copy_from_slice(&(uv_offset as u32).to_le_bytes());
    for n in 0..draws {
        let at = records + n * 16;
        result[at + 1] = 11; // geometry + texture + constant alpha
        result[at + 2] = 0;
        result[at + 3] = 0;
        result[at + 8..at + 11].fill(255); // image supplies RGB, native fill keeps alpha
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires a private local GE2D corpus; set IIDXBG_SHAPE_CORPUS"]
    fn all_audited_untextured_shapes_convert() {
        let root = std::env::var_os("IIDXBG_SHAPE_CORPUS").expect("IIDXBG_SHAPE_CORPUS");
        let mut count = 0;
        for entry in std::fs::read_dir(root).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|ext| ext != "geo") {
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            for aspect in [[1920., 1080.], [1080., 1920.], [1., 1.]] {
                assert!(
                    textured_geometry(&bytes, aspect).is_some(),
                    "{}",
                    path.display()
                );
            }
            count += 1;
        }
        assert!(count > 0, "empty shape corpus");
        eprintln!("Validated {count} distinct GE2D resources at three source aspects");
    }
    fn rectangle() -> Vec<u8> {
        let mut b = vec![0u8; 112];
        b[..4].copy_from_slice(b"D2EG");
        b[12..16].copy_from_slice(&112u32.to_le_bytes());
        b[20..22].copy_from_slice(&4u16.to_le_bytes());
        b[28..30].copy_from_slice(&1u16.to_le_bytes());
        b[32..36].copy_from_slice(&52u32.to_le_bytes());
        b[48..52].copy_from_slice(&84u32.to_le_bytes());
        for (i, v) in [10f32, 20., 110., 20., 10., 220., 110., 220.]
            .into_iter()
            .enumerate()
        {
            b[52 + i * 4..56 + i * 4].copy_from_slice(&v.to_le_bytes());
        }
        b[84] = 4;
        b[85] = 9;
        b[88..90].copy_from_slice(&6u16.to_le_bytes());
        b[95] = 128;
        b[96..100].copy_from_slice(&100u32.to_le_bytes());
        for (i, v) in [0u16, 1, 2, 1, 2, 3].into_iter().enumerate() {
            b[100 + i * 2..102 + i * 2].copy_from_slice(&v.to_le_bytes());
        }
        b
    }
    #[test]
    fn geometry_keeps_indices_and_alpha_and_generates_local_uv() {
        let b = rectangle();
        let r = textured_geometry(&b, [100., 200.]).unwrap();
        assert_eq!(&r[100..112], &b[100..112]);
        assert_eq!(&r[92..96], &[255, 255, 255, 128]);
        let uv: Vec<_> = r[112..]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| f32::from_le_bytes(*p))
            .collect();
        assert_eq!(uv, vec![0., 0., 1., 0., 0., 1., 1., 1.]);
        assert_eq!(b[85], 9);
    }
    #[test]
    fn malformed_or_unsupported_geometry_falls_back() {
        for size in 0..112 {
            assert!(textured_geometry(&rectangle()[..size], [100., 200.]).is_none());
        }
        let mut b = rectangle();
        b[100..102].copy_from_slice(&4u16.to_le_bytes());
        assert!(textured_geometry(&b, [100., 200.]).is_none());
        let mut b = rectangle();
        b[85] = 11;
        assert!(textured_geometry(&b, [100., 200.]).is_none());
        let mut b = rectangle();
        b[52..56].copy_from_slice(&f32::NAN.to_le_bytes());
        assert!(textured_geometry(&b, [100., 200.]).is_none());
    }
}
