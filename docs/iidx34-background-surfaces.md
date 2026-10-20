# IIDX 34: selection, covers and target graph

Recorded 2026-10-21 using IDA/Hex-Rays, PE vtables and Capstone.
Input: `C:\Users\BeatMania IIDX LM\Downloads\bm2dx.dll`.
User build label **34 / 20260917**; size **18,200,216**.
SHA-256: `3755587f44785effe5008f74cfe7dccbdf423ad09feead48e00a90f78a42fe83`.
Preferred base `0x180000000`; image size `0xc5ff000`.
All addresses below are **RVAs**; object offsets are decimal.
These are investigation anchors, not additional runtime hook profiles.

See [design and limitations](background-surfaces.md) and
[33 comparison](iidx33-background-surfaces.md).

## Selection background and return-only helpers

| RVA | Evidence |
|---|---|
| `0xd52508` | `CMusicSelectScene` vtable: init 16 `0x8d7b70`, cleanup 17 `0x8d7f60`, update 18 `0x8d8250` |
| `0x5fcde0` | Wrapper initializes background helper at widget +936 via `0x5f2f50`, then tail-jumps to `0x5f30c0` |
| `0x5f2f50` | Initializes filename, flags, 1920x1080 rectangle and fade thresholds; **does not load a movie** |
| `0x5f3070` | `32 c0 c3`: returns false |
| `0x5f3080`..`0x5f30b0`, step 16 | `c2 00 00`: return-only helpers |
| `0x5f30c0` | `b0 01 c3`: returns true |
| `0x5f30d0` | `c2 00 00`: return-only helper |
| `0xcc5ea0`, `0xcc5f00` | Filename table; `/0/music_sel_bg.mp4` still exists |
| `0x937480`, `0x1041e50` | Shared movie manager getter/object |
| `0x936770`, `0x936dd0` | Sync/async movie loading functions still exist |
| `0x9374a0` | Async completion/decoder update, called by selection widget update `0x5fa9e0` |
| `0x936270`, `0x9362c0` | Movie render dispatcher and textured D3D9 quad |
| `0x9248b0` | Main renderer calls dispatcher at `0x9249d9`, before subsequent scene/AFP rendering |
| `0x8e46a0` | Gameplay rendering also calls dispatcher (`0x8e46d9`) |

The return-only helpers have real selection callers. For example `0x5fa4d0`
calls `0x5f3090`; `0x5fb610`, `0x5fbfa0` and `0x5fc6f0` call `0x5f30b0`.
`0x5fa0a0` and `0x5fce10` call `0x5f30d0`. This establishes behavior of
**this supplied binary**, not the cause of the stubs or what an unmodified
34 release should do. Do not silently restore presumed original code.

The movie dispatcher still checks enable bytes +16/+36, then passes manager
rectangle +20/+24/+28/+32, filter byte +37 and float alpha +40 to the quad.
Disassembly verifies RCX manager, EDX x, R8D y, R9D width, caller stack
+0x20 height, +0x28 filter byte, +0x30 float alpha. It returns EAX zero.
The D3D9 virtual byte offsets are +520 SetTexture, +712 SetFVF and +664
DrawPrimitiveUP, with 28-byte vertices.

Consequently a replacement that only hooks the enabled quad is insufficient.
The outer dispatcher is a candidate insertion point even without a loaded
movie, provided a verified selection-scene lifetime controls it. It also needs
explicit geometry/fade ownership when native selection helpers do nothing.
Reusing the 33 helper layout or assuming a filename string proves active movie
loading would give an incorrect implementation.

## SUDDEN+ and LIFT/lower cover

| RVA | Evidence |
|---|---|
| `0x8ecd40` | Gameplay graphics init: cover base +4064, **72-byte stride**; initializer call `0x8ed07e` |
| `0x8f4d70`, `0x8f4de0` | Cover construction/destruction; destructor frees pair holders |
| `0x8f5ae0` | `(cover*, u32 side)` initializes upper/lower art and adjustment overlays |
| `0x860bf0`, `0x860c50` | Separate upper/lower customization setting getters |
| `0xd56270` | Lane-name table, stride 259 |
| `0x8f59a0` | Lane-name getter with bound 373 |
| `0x8f6350` | Pair builder `(pair*, archive*, i32 id, byte mirror, byte flip)` |
| `0x334d10`, `0x32da80` | Static sprite / animated layer factories; priority 108 |
| `0xd56260` | Clip rectangle **(0, 0, 1920, 723)** |
| `0x8f66e0` | Tick: toggle `0x8f4e50`, adjustment `0x8f6a60`, visual `0x8f50e0` |
| `0x8f50e0`, `0x8f69f0` | Visual geometry and pair show/position |
| `0x8f5510` | Native adjustment overlay update |
| `0x343050`, `0x329bb0` | Static sprite creation and root clip setup at **sprite +16** |
| `0x329e50` | Existing dynamic descriptor setter |
| `0xe368c8` | `BM2D::CSprite` vtable |

Upper/lower pair pointers remain +8/+16; adjustment layers +24/+32; enable
byte +40. Each pair is `{CSprite*, CLayer*}`. The same separate-selection vs
reuse/flip branches exist as in 33.

**Do not copy 33's 56-byte cover stride.** This build adds state at +56/+60/+64.
The visual routine reads cached upper height at +64, producing y = height-723.
It reads lower height at +60 and an additional native lift amount for lower y.
Side/DP placement comes from table `0x103fad8`.

Static sprite vtable byte offsets +40/+72/+104/+120/+160/+208 resolve to
`0x3364c0`, `0x336580`, `0x3366d0`, `0x336920`, `0x336b20`, `0x336c10`.
They control visibility, position, origin, clip, alpha and transform. Preserve
native behavior; texture changes must not replace input/height adjustment.
The 216 origin constant is not proof of a universal bitmap width.

## Gameplay target-score graph

| RVA | Evidence |
|---|---|
| `0xa20af0` | Graph init: native mode guards, artwork setting, archive and optional animated layer |
| `0x860920` | Graph artwork setting getter |
| `0xa1fcf0` | Artwork layout/draw; called by `0xa20e20` at `0xa20ef6` |
| `0xa20d10` | Graph reset/shared-owner release, clears archive/frame/artwork |
| `0xa1f810` | Separate score bar/label setup, called by graph init |
| `0x334ed0` | Transient static sprite allocator/draw; called at `0xa1fed4` |
| `0x342fc0` | Named transient bitmap initialization |
| `0xd95000` | `gr_line_max` marker |

Graph init uses `0x8f59a0`, the same lane-name resolver as cover customization,
then archive loader `0x818ee0`. A valid nonzero ID can create a retained animated
layer at +64 with priority 145. Default ID zero bypasses this archive path.

The artwork helper independently verifies the same geometry as 33:
graph frame +8, horizontal offset +136, mode flag +156; marker-derived `top`;
clip `(offset+1503, top, 414, (mode ? 880 : 870)-top)` and artwork position
`(offset+1494, top-34)`. The static branch creates a transient sprite,
then applies alpha and clip.

Transient sprite stride is 608, pool base manager +2490376, counter +4980744,
limit 4096. A reused bitmap is rebound by `afp_core_36`. Bind a custom descriptor
to the current root clip, not a sprite pointer retained between frames.
The animated layer is a separate path and cannot be treated as an identical
bitmap. Native mode/enable predicates still control whether this graph exists.

## Reproduction and limits

`python scripts/verify-surfaces.py <this DLL>` passed on 2026-10-21. It checks
identity, vtable targets, clip/lane constants, strings, direct call/tail-jump
anchors and the exact stub bytes. It does not patch the file. In IDA, add
the preferred base to the listed RVAs, inspect the callers and decompile
each relevant function. Script code windows are not function-size assertions.

Unresolved before implementation: selection lifecycle/fades with these stubs,
safe coexistence with other hooks, native graphics pool teardown/reset,
actual bitmap bounds, animated artwork child paths, default graph backdrop
and special game modes. These new surfaces have **no in-game validation**;
34 results-background support also remains offline-validated only.

## Implementation evidence ? 2026-10-21

This extends the earlier feasibility record; the runtime now implements all four
surfaces. Input SHA-256 remains `3755587f44785effe5008f74cfe7dccbdf423ad09feead48e00a90f78a42fe83`.
All addresses below are RVAs under preferred base `0x180000000`.

| Native entry | RVA | Microsoft x64 ABI / purpose |
|---|---|---|
| `select_update` | `0x8d8250` | (scene) -> u64; ordinary-scene vtable check; separate detour from 33 |
| `select_cleanup` | `0x8d7f60` | (scene) -> u64; retire before original |
| `select_delete` | `0x8d7340` | (scene, u8 flags) -> scene; vtable slot 7 deleting destructor |
| `movie_draw` | `0x936270` | (manager) -> u64; outer dispatch before enabled-movie check |
| `cover_init` | `0x8f5ae0` | (cover, u32 side) -> u64; starts a new chart ownership interval |
| `cover_build` | `0x8f6350` | (pair, archive, i32 id, u8 mirror, u8 flip) -> void; fifth arg on stack |
| `cover_visual` | `0x8f50e0` | (cover) -> u64; native visibility/alpha/position update |
| `cover_delete` | `0x8f4de0` | (cover) -> void; frees both pair holders |
| `graph_art` | `0xa1fcf0` | (graph) -> void; marker-derived position and clip |
| `graph_reset` | `0xa20d10` | (graph) -> u64; clears artwork/frame owner |
| `graph_parent_reset` | `0xa284a0` | (parent) -> u64; graph at parent +136 overwritten by fresh object |
| `graph_storage_release` | `0xa1f260` | (map storage) -> void; graph map storage at +80, destructor path |
| `sprite_release` | `0x3363e0` | (sprite) -> void; destroys AFP node and zeros sprite +8 |
| `sprite_manager` | `0x334890` | () -> manager pointer |
| `sprite_create` | `0x334de0` | (manager, 40-byte descriptor, u32 priority) -> sprite pointer |
| `sprite_origin` | `0x3366d0` | (sprite, i32 x, i32 y) |
| `sprite_scale` | `0x336c10` | (sprite, f32 x, f32 y); XMM1/XMM2 |
| `sprite_clip` | `0x336920` | (sprite, int[4] rectangle pointer) |
| `sprite_position` | `0x336580` | (sprite, i32 x, i32 y) |
| `sprite_alpha` | `0x336b20` | (sprite, f32 alpha, f32 red, f32 green, f32 blue); blue at caller stack +32 |
| `graph_marker` | `0x81e9f0` | (frame, NUL-terminated clip name) -> clip pointer |

The descriptor factory creates an AFP node through ordinal 35, initializes its
root at sprite +16, and applies native priority. Retained pool entries are 608
bytes; a zero handle at +8 marks a free slot. The common release destroys it via
ordinal 33 and zeros the handle. Sprite virtual +8/+16 delegate to release,
and the deleting destructor also calls that release entry. The release hook
observes external native pool/group retirement, so the plugin never releases
a recycled sprite slot. Own cleanup calls the trampoline directly, then drops
the texture wrapper. AFP group getter/setter are ordinal 21/20; the runtime
profile records their IAT slots and the verifier checks those import ordinals.

Cover builder disassembly loads floats +1 and -1: mirror applies scale(-1,1),
origin(216,0); flip then applies scale(1,-1), origin(0,723). These operations
are sequential, not combined into an inferred two-axis transform. Clip remains
(0,0,1920,723). A dedicated bitmap is temporarily placed in pair[0], pair[1]
is zeroed, the original visual routine runs, and both original pointers return.
This preserves native alpha and adjustment instead of reconstructing gameplay
settings. Original static/animated artwork is hidden before substitution.

The graph art helper only uses the common sprite position/clip virtual methods
on graph +64. Substitution is scoped to that call; no CLayer-specific animation
method receives the replacement CSprite. The native static branch's color call
passes alpha=1 and RGB=0.7; it is a five-argument function including `this`,
not a single-float alpha setter. Score bars/labels are not rebound.

Graph initialization calls reset, but reset alone is NOT its destructor.
The parent reset overwrites the graph from a fresh temporary. Map-storage
release (graph +80) is shared by the graph destructor's cleanup path; the hook
matches tracked graph addresses before retiring their GPU state. These extra
hooks cover normal, reinitialization and unwind paths without dereferencing
arbitrary map owners.

Reproduce with `python scripts/verify-surfaces.py <exact DLL>` and
`python scripts/smoke-attach.py target/release/iidxbg.dll <exact DLL>`.
Decompile the entries above and inspect float-call setup in the cover builder
and graph art helper. Local raw evidence is in ignored
`analysis/surfaces-implementation-evidence.json`,
`analysis/surfaces-lifetime-evidence.json` and
`analysis/surfaces-34-implementation-evidence.json`; the earlier 53-function
snapshot is `analysis/surfaces-evidence-20261021.json`. Both are reference aids,
not shipped game material. Exact entry bytes live in the embedded profile.
The implementation has not been exercised in a running game; visual ordering,
cropping, all mode variants and real device-reset behavior remain unverified.
