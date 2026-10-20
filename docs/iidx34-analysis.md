# IIDX 34 results hook: evidence and implementation

Recorded 2026-10-20. Analysis input:
`C:\Users\BeatMania IIDX LM\Downloads\bm2dx.dll`, 18,200,216 bytes.
SHA-256 `3755587f44785effe5008f74cfe7dccbdf423ad09feead48e00a90f78a42fe83`.
Image base `0x180000000`, image size `0xc5ff000`. All addresses below are RVAs.
Object offsets are decimal. The repository uses the user's 20260917 build label;
sp2xpatcher identifies the PE as `LDJ-6aa23d57_b02b3c`, 2026-09-16 (010).

## Evidence

| RVA | Verified role |
|---|---|
| `0xd72950` | `CStageResultScene` vtable; slots 16/17/18 = init `0x914080`, cleanup `0x915580`, update `0x915b10`; slot 27 = `0x915a10` |
| `0x911e60` | Scene constructor constructs `StageResultDrawBg` at +264; callsite `0x911ed5` |
| `0x912ba0` | Result vtable slot 7: deleting destructor `(scene, u8 flags)`, calls `0x9124c0` then frees if flags bit 0 is set |
| `0xd70598` | Background vtable; slot 1 setup `0x906d60`, slot 2 callback `0x906e70` |
| `0x913270` | Component index 0 is scene +264; component pointer array starts at scene +16088 |
| `0x915a10` | Dispatches slot 2 of 14 components, passing timing in EDX |
| `0x906b40` | Chooses/loads background archive; archive pointer at background +272 |
| `0x620ad0` | Result filename maker, `.ifs`, fallback stem `result_get_clear` |
| `0x818ee0` | Resource loading below `/data/graphic/` |
| `0x906d60` | Creates header at +288, `bg1_clear`/`bg1_failed` clip at +296, difficulty at +304 |
| `0x906e70` | Finds `illust` in +296; binds archive `result_bg`, or hides it if no archive |
| `0x81e9f0` | Null-checked clip lookup; delegates to `0x32d1a0` |
| `0x329f30` | Archive/name texture setter through `afp_utils_71`, `_77`, then `afp_core_137` |
| `0x329e50` | Dynamic-descriptor setter through the same `afp_core_137` |
| `0x32a1c0` | Sets clip visibility through AFP property 4103; marks dirty with 4126 |
| `0x32a460` | Native origin helper; `(clip, 3, 3)` consults archive dimensions and sets center |
| `0xa4f8a0` | Wraps an existing `IDirect3DTexture9*` into `AfpTextureEx`, AddRefs and registers it |
| `0x344b60` / `0x344ea0` | Texture registration / descriptor construction |
| `0xa4fd40` | Returns wrapper +24 |
| `0xa4fa30` | Wrapper destructor unregisters handle via `0x344c30` and releases COM texture |
| `0x63d9e0` | Native assistant UI obtains descriptor through `0xa4fd40`, then binds through `0x329e50` |
| `0x922a50` | Returns D3D device from root global `0xb4e3360`, root +224 |

The relevant branch:

```text
clip = find(background.movie_at_296, "illust")       // 0x906fd4
if clip:
    if background.archive_at_272:
        bind_archive(clip, archive, "result_bg")     // 0x906ff7
        origin(clip, 3, 3)                          // 0x907007
    else:
        visible(clip, false)                       // 0x90700e
```

The component also updates stage label, Premium Free timer, title, artist and
difficulty. Replacing/skipping the whole callback would remove unrelated UI.

## Live insertion, not archive filename replacement

Archive manager `0x33c450` first queries `0x33d840` by filename/options.
For a cache hit it calls `0x33af10` on the existing entry. This reads/mounts the
archive only if reference count at entry +1292 is <=0; otherwise it increments
the count without rereading. Overwriting a referenced archive at the same path
therefore does not refresh it through that path. New paths still require valid
AFP/IFS archives and explicit rebinding.

`iidxbg` instead owns HTTP/file reads and PNG/JPEG decoding. Each prepared image
becomes a new D3D texture. The native wrapper registers it, and the dynamic setter
binds its descriptor to the existing `illust`. No game archive is changed.

## ABI and ownership

Microsoft x64 ABI: RCX is `this`, RDX the second pointer. Hook signatures retain
the background callback's unused timing argument in EDX. Original return values
are forwarded. Ordinary result cleanup is `(scene) -> machine word`.

`AfpTextureEx` occupies 64 bytes in the inspected constructor:

| Offset | Field |
|---|---|
| +0 | vtable |
| +8 | AddRef-owned D3D texture pointer |
| +16 | registered u32 handle |
| +24 | 40-byte descriptor |

The descriptor begins with handle, width, height; the auxiliary pointer is +32
within the descriptor. It is not interchangeable with a D3D texture pointer
or an archive object. `iidxbg` uses stable aligned Box storage, calls the native
constructor, and calls the non-deleting destructor before freeing storage.

Runtime guards in `src/profiles/iidx34-20260917.json` cover every directly called native function
and all three modified vtable slots. File SHA-256 must match before memory hooks are
installed. Loaded entry-byte mismatches and already replaced slots are rejected.
Unrelated in-memory patches may coexist if they do not alter those guards.
The DLL file is never patched.

## Implemented lifecycle

- Spice SDK initialization starts the worker outside the loader lock.
- Exact-build checks precede atomic replacement of background slot 2 and
  ordinary result cleanup slot 17 and deleting-destructor slot 7. The destructor
  also retires tracked graphics when normal cleanup was skipped. The plugin is
  pinned until process exit.
- The background hook calls original code first. It verifies that
  `background -264` has the ordinary result vtable, and that the callback thread
  matches the SDK presentation thread.
- On first callback per visit, it freezes the ready CPU image or retains the
  previous selection. A try-lock miss also freezes the previous selection.
- GPU creation/upload occurs once at results entry, never on the download worker.
  Texture dimensions are configured, default 1920x1080. Native origin/layout is
  preserved. The descriptor is rebound after each original callback.
- Cleanup restores the stock binding while clips are valid, destroys the custom
  texture, ends the visit and requests the next image. Preparation begins ahead
  of the next results, usually during selection/gameplay.
- SDK INVALIDATE restores stock binding and destroys default-pool resources.
  READY allows recreation from the same frozen CPU image after reset.
- SDK DESTROY releases graphics resources. SDK shutdown disables custom work;
  pinned passthrough vtable callbacks retain valid code until process exit.

The image worker has below-normal priority, bounded downloads/decodes, a single
pending image and generation checks before publishing. Graphics hooks never do
network IO, decoding, file logging or waiting for the worker. Diagnostics use a
bounded non-blocking event queue. Uploads are timed in microseconds in the log.
Background work can still consume CPU/memory bandwidth; no zero-impact claim.

## Sources and reproduction

Original static investigation used IDA/Hex-Rays, MSVC RTTI, cross-references,
Capstone and PE exception entries. That investigation and its verification script
are preserved in this repository under `docs/feasibility.md` and
`scripts/verify-native.py` so future work does not depend on another checkout.

```powershell
python -m pip install pefile capstone
python scripts/verify-native.py 'C:\path\to\bm2dx.dll'
target/release/iidxbg-tool.exe verify 'C:\path\to\bm2dx.dll'
```

Spice SDK ABI reviewed from
[`spicesdk.h`](https://github.com/spice2x/spice2x/blob/7aba7da3b67f3e4c8b0e950f306bd044d69fa494/src/spice2x/sdk/include/spicesdk.h)
and its D3D9 lifecycle source. Native hook logic is independent code; no Spice
implementation is bundled. The image provider uses the
[official Konachan API](https://konachan.net/help/api).
Image decoding follows [image's bounded reader API](https://docs.rs/image/0.25.10/image/struct.ImageReader.html).

## Remaining game validation

This is an experimental implementation, not a validated game release.
Static evidence proves the path exists; it does not prove every lifetime/visual
assumption during the running game.

Check the diagnostic border/cross in normal clear/failed/full-combo results,
SP on both sides, DP/2P and Premium Free. Confirm score text, graphs, title and
timers remain visible. Separate Dan, Arena, StepUp and overall result scene
classes are deliberately not hooked. Inspect transitions, repeated visits,
early exits, device resets and shutdown. Measure frame-time tails with local
and remote sources enabled, including decode load and first-results GPU upload.
Native access violations are not caught by Rust panic containment.
