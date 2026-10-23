# IIDX 33: selection, covers and target graph

Recorded 2026-10-21 using IDA/Hex-Rays, PE vtables, and Capstone disassembly.
Input: `D:\Share\chart-requester\analysis\bm2dx.dll`.
Build label: **33 / 010-20260819**; file size **18,467,992**.
SHA-256: `c61b6dcb8894062e56d60da8ca90053b27f129e1a8e8da5e54457aa42602397d`.
Preferred base `0x180000000`; image size `0xbb95000`.
All code/data addresses below are **RVAs**, object offsets are decimal,
and vtable byte offsets are explicitly marked.

These are analysis anchors, not production hook profiles.
See [design and limitations](background-surfaces.md).

## Selection background

| RVA | Evidence |
|---|---|
| `0xd84788` | `CMusicSelectScene` vtable: init slot 13 `0x8eb820`, cleanup 14 `0x8ebeb0`, update 15 `0x8ec1f0` |
| `0x8eb050` | Scene setup initializes selection widget at scene +408 through `0x607130` |
| `0x607130` | Initializes background helper at widget +952 through `0x600cc0` |
| `0x600cc0` | Initializes 1920x1080 rectangle, path buffer, fade state; calls movie-selection helper |
| `0x600f40` | Chooses default/selected-song movie; stops and reloads manager on filename change |
| `0x82c6d0`, `0xcf1120` | Movie filename getter and pointer table |
| `0xcf1180` | `/1/music_sel_bg.mp4`; nearby `/1/onemore_sel_bg.mp4` |
| `0x600e40` | Fade/reload update; writes manager +40 alpha |
| `0x950510`, `0x1080bc8` | Movie manager getter and global object |
| `0x94fc00` | Movie loading under `/data/movie/`, decoder replacement and shared ownership |
| `0x94f7c0` | Movie render dispatcher: requires flags +16/+36, then calls `0x94f810` |
| `0x94f810` | Textured D3D9 quad, native rectangle, alpha and filter flag |
| `0x93d930` | Main rendering path calls dispatcher at `0x93da48` before subsequent scene/AFP rendering |
| `0x8f81e0` | Gameplay path also calls dispatcher (`0x8f8219`): not selection-exclusive |

Helper layout: filename +0 (256 bytes), counter +256, thresholds +260/+264,
music ID +268, mode flags +272..274, rectangle +276/+280/+284/+288.
Movie manager rectangle is +20/+24/+28/+32; filter flag +37; alpha float +40.

The dispatcher disassembly establishes the Microsoft x64 draw ABI:
RCX manager, EDX x, R8D y, R9D width; caller stack +0x20 height,
+0x28 byte filter flag, +0x30 float alpha. Its normal return is EAX zero.
The quad uses D3D9 SetTexture (vtable byte +520), SetFVF (+712),
DrawPrimitiveUP (+664), triangle fan, two primitives, 28-byte vertex stride.
Draw behavior and state restoration still require an in-game prototype.

## SUDDEN+ and lower/LIFT cover

| RVA | Evidence |
|---|---|
| `0x9020a0` | Gameplay graphics initialization; cover objects at owner +4064, two players, **56-byte stride** |
| `0x90a100`, `0x90a170` | Cover construction and destruction; destructor frees pair holders |
| `0x90adf0` | Cover initialization `(cover*, u32 side)`, including both artwork pairs and adjustment overlays |
| `0x8742d0`, `0x874330` | Independent upper/lower customization setting reads |
| `0xd88530` | `laneNNN` records, **259-byte stride** |
| `0x90acd0` | Name getter checks range using bound 366 |
| `0x90b5e0` | Build pair `(pair*, archive*, i32 id, byte mirror, byte flip)` |
| `0x34aa90`, `0x344080` | Static sprite / animated layer factories; artwork priority 108 |
| `0xd88520` | Four signed integers: **0, 0, 1920, 723** |
| `0x90b8f0` | Tick calls toggle `0x90a1e0`, adjustment `0x90bc70`, then visual update `0x90a430` |
| `0x90a430`, `0x90bc00` | Compute cover geometry; show pair and set position |
| `0x90a840` | Adjustment UI, including `lift_line` and numeric values |
| `0x3539c0` | Static sprite initialization creates AFP bitmap node and root clip at **sprite +16** |
| `0x340030` | Root clip setup through `afp_core_103(..., "/")`; clip node ID at clip +512 |
| `0x340440` | Existing dynamic texture setter reads clip +512, invokes `afp_core_137`, returns status in AL |
| `0xe6c810` | `BM2D::CSprite` vtable |

Cover layout: side +0; upper pair pointer +8; lower pair pointer +16;
`lane_sud` overlay +24; `lane_lift_sud` overlay +32; enabled byte +40;
adjustment state +44/+48; frame counter +52.
Each heap pair is 16 bytes: static `CSprite*` at +0, animated `CLayer*` at +8.

The upper pair uses the upper selection. The lower pair uses a separate
selection when the native lower-mode predicate is enabled; otherwise it uses
the upper archive and its flip flags. Table byte +256 controls the side-2
mirror branch; +257 is consulted for the lower flip branch. Their effects
include native origin values (216,0) or (0,723).

Virtual byte offsets common to both artwork classes: +40 visibility,
+72 position, +104 origin, +120 clip rectangle, +160 alpha, +208 transform.
The static sprite's corresponding entries are `0x34c240`, `0x34c300`,
`0x34c450`, `0x34c6a0`, `0x34c8a0`, `0x34c990`. The clip wrapper delegates to
`afp_core_48`. Float arguments obscured by Hex-Rays must be recovered from
register-level disassembly before writing callable signatures.

The visual update derives upper y from SUDDEN height minus 723 and lower y
from 723 minus the native lower heights. Side/DP placement comes from table
`0x107e7e8`. Rebinding art must not bypass the tick or adjustment functions.
Pair holders are freed by the cover destructor; graphics objects come from
native managers. Do not free their sprite pointers as plugin allocations.

## Gameplay target-score graph

| RVA | Evidence |
|---|---|
| `0xa3d650` | Graph initialization; native enable/mode guards, selected customization lookup, archive load |
| `0x874000` | Graph artwork setting getter |
| `0xa3c370` | Separate graph bars/labels: `mybest`, `target`, `lv_pass`, `achieve_pass`, `sabun_*` |
| `0xa3c850` | Artwork layout/draw helper, reached from update `0xa3d980` at `0xa3da56` |
| `0xa3d870` | Resets graph object and releases its shared frame owner; clears artwork +64 |
| `0x34ac50` | Transient sprite draw/allocate helper; static graph call at `0xa3ca34` |
| `0x353930` | Named transient bitmap initialization; also constructs root clip at sprite +16 |
| `0xdc6e30` | `gr_line_max` marker used to derive graph clipping |

Initialization is reached from gameplay graphics setup through `0xa44d40`
(graph initializer call `0xa44e6b`). Nonzero valid graph-artwork IDs resolve
through **the same `0x90acd0` lane-name getter**. Animated archives create a
retained layer at graph +64, priority 145. Zero/default leaves archive +0 null.

Artwork helper receives RCX = graph object. It uses the frame at +8 and
horizontal offset at +136. Let `top` be the native marker's y plus height.
Clip = `(offset+1503, top, 414, bottom-top)`, where bottom = 870 or 880
depending on byte +156. Artwork position = `(offset+1494, top-34)`.
It updates retained animated artwork, or obtains a transient static sprite
at priority 145, then applies native alpha and clipping.

Transient pool details: 608-byte sprite stride, pool base manager +2490376,
counter +4980744, limit 4096. Existing nodes are rebound with `afp_core_36`.
Therefore a graph background texture replacement must be applied to the
current returned sprite, not cached as a persistent sprite pointer.

The similarly named `graph_bg` used by `0x91e620` belongs to a different
results graph path and is not the gameplay hook found above.

## Reproduction and remaining unknowns

Run `python scripts/verify-surfaces.py <this DLL>`: PASS on 2026-10-21.
It checks SHA-256, sizes, scene/sprite vtables, lane records, clipping constants,
strings, and direct call/tail-jump evidence using Capstone. In IDA, decompile
the functions in the tables and follow the listed callsites; add the preferred
base to each RVA. The script's code windows are not inferred function sizes.

Static sprite descriptor rebinding is a supported ABI candidate, not yet
tested on these new surfaces. Remaining work includes actual artwork bounds,
animated child ownership, graph default backdrop, manager pool teardown/reset,
scene-hook coexistence, fade/ordering, and SP/DP/side/arena variants. No
in-game validation of these four replacements was performed.

## Implementation evidence ? 2026-10-21

This extends the earlier feasibility record; the runtime now implements all four
surfaces. Input SHA-256 remains `c61b6dcb8894062e56d60da8ca90053b27f129e1a8e8da5e54457aa42602397d`.
All addresses below are RVAs under preferred base `0x180000000`.

| Native entry | RVA | Microsoft x64 ABI / purpose |
|---|---|---|
| `select_update` | `0x8ec1f0` | (scene, u8 tick, context) -> u64; ordinary-scene vtable check |
| `select_cleanup` | `0x8ebeb0` | (scene) -> u64; retire before original |
| `select_delete` | `0x8eabb0` | (scene, u8 flags) -> scene; vtable slot 7 deleting destructor |
| `movie_draw` | `0x94f7c0` | (manager) -> u64; outer dispatch before enabled-movie check |
| `cover_init` | `0x90adf0` | (cover, u32 side) -> u64; starts a new chart ownership interval |
| `cover_build` | `0x90b5e0` | (pair, archive, i32 id, u8 mirror, u8 flip) -> void; fifth arg on stack |
| `cover_visual` | `0x90a430` | (cover) -> u64; native visibility/alpha/position update |
| `cover_delete` | `0x90a170` | (cover) -> void; frees both pair holders |
| `graph_art` | `0xa3c850` | (graph) -> void; marker-derived position and clip |
| `graph_reset` | `0xa3d870` | (graph) -> u64; clears artwork/frame owner |
| `graph_parent_reset` | `0xa45000` | (parent) -> u64; graph at parent +136 overwritten by fresh object |
| `graph_storage_release` | `0xa3bdc0` | (map storage) -> void; graph map storage at +80, destructor path |
| `sprite_release` | `0x34c160` | (sprite) -> void; destroys AFP node and zeros sprite +8 |
| `sprite_manager` | `0x34a610` | () -> manager pointer |
| `sprite_create` | `0x34ab60` | (manager, 40-byte descriptor, u32 priority) -> sprite pointer |
| `sprite_origin` | `0x34c450` | (sprite, i32 x, i32 y) |
| `sprite_scale` | `0x34c990` | (sprite, f32 x, f32 y); XMM1/XMM2 |
| `sprite_clip` | `0x34c6a0` | (sprite, int[4] rectangle pointer) |
| `sprite_position` | `0x34c300` | (sprite, i32 x, i32 y) |
| `sprite_alpha` | `0x34c8a0` | (sprite, f32 alpha, f32 red, f32 green, f32 blue); blue at caller stack +32 |
| `graph_marker` | `0x8322c0` | (frame, NUL-terminated clip name) -> clip pointer |

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

### Read-only stock artwork dimension check

`data/graphic/lane000.ifs` SHA-256 `7902f1ce8c445a5f65f2b038c2c142b39a3bc97c02b38c69761dc7c014ee9568`, 158592 bytes. Its texturelist metadata has texture size 512x1024,
uvrect `2 866 2 1450` and imgrect `0 868 0 1452` (half-pixel units).
The extracted padded PNG is 434x726; the interior UV span is 432x724.
The prototype prepares a 432x723 canvas matching native clipping height and
width, without embedding archive padding. This still needs visual crop checks.
Only metadata is documented; the temporary extraction stays ignored under
`analysis/lane000` and is not distributed. The 34 implementation uses this
canvas with its independently matching clip/transform constants; its own
stock asset dimensions were not measured.
