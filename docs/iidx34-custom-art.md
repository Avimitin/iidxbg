# IIDX34 native artwork replacement investigation — 2026-10-24

## Inputs and scope

Only game SHA-256 `c98e183ec81ecb9e1cb13c237f9c9cb0b3f65b6fdaff6fd5e6a526008c546534`
(profile `iidx34/010-20260917.rs`, preferred base `0x180000000`) has the new
native adapters. Other editions/dates are not inferred compatible.

Supporting library hashes:

- afp-core.dll: `baa6d62f4927ca39714a1ea9cfd8a7a8b3ab80873e67d85a749f8174edfc3321`
- afp-utils.dll: `b74a538ee13603d8d9d695a6c14b296ac532685528923b30e125d0197d706322`

Archives were copied from the game share into ignored
`analysis/custom-art-20261024/archives`, then extracted with mon/ifstools into
the sibling `extracted` directory (`super_disable=True`, `recurse=False`).
No extraction, replacement or temporary files were written into game assets.
`manifest.json` records source paths, byte counts and hashes. Relevant inputs:

| Relative graphic path | SHA-256 |
| --- | --- |
| `gmframe34.ifs` | `f474bb21e143cc355eb7a89384509bc4228fb21d2a2652a5b6564a7bf5beb9c1` |
| `notes00.ifs` | `03cb10fb8663f88ed3ad05ab1be0b05283f84b566aa95f469a010be9318e8e0f` |
| `notes00_long.ifs` | `ebf5e0657e34f0a21220e5ffbf509119941c49b5fcc348f9b64d66371e6ead2b` |
| `0/basic_frame_chara.ifs` | `ee81627f7cd79c0339d4826b8e26477ad411fffdbc886b1169ade5c25d429730` |
| `0/basic_frame.ifs` | `a4c27d80e53d21391c90db8bf80b8e89c50e5b14d2f81315ff1d70bb6e228c57` |
| `0/mselect.ifs` | `311b5732d9fa5241233a797378fa1784f66812fc14bcf76ebd478a76f865923f` |

The `1/` directory contains older artwork; it must not be mistaken for the
active `0/` archives. No extracted game images or binaries are distributed.
AFP timelines were inspected using the upstream bemaniutils parser in this
local analysis directory, alongside texturelist metadata and geometry strings.

## AFP draw adapter

All following addresses are game RVAs unless a library is named.

| RVA | ABI / evidence |
| --- | --- |
| `0x32da80` | CLayer factory `(manager, archive, name, priority:u32, auto:u32, group:u32) -> layer`; 616-byte pooled objects; archive handle at archive+1300 |
| `0x32c7d0` | Layer virtual release; stops AFP then calls common release `0x3363e0` |
| `0x3363e0` | Common release observer; remove mappings before the native pool can reuse a layer |
| `0x34fed0` | AFP dispatcher `(packed_vertices, count:i32, parameters, user_data) -> usize`; parameters+8 is texture ID; parameters+4 describes packed vertex fields |
| `0x338570` | Original texture/shader setup; called by dispatcher before vertex draw |
| `0x338900` | `(renderer, vertices, count:i32, flags:u8, primitive:i32, user_data) -> usize`; converts packed vertices, draws synchronously; renderer+98304 is D3D9 device |
| `0xc66da0` | IAT slot for afp-utils ordinal71, texture metadata lookup |

afp-utils ordinal71 (`+0x11450`, forwarding to `+0xd7e0`) writes a 56-byte
texture record. Ordinal77 (`+0x116e0`) proves its conversion into the existing
40-byte texture descriptor: source offsets 4/12/16 -> descriptor 0/4/8,
UV floats 28/32/36/40 -> 12/16/20/24, auxiliary QWORD48 ->32.
Record offsets20/24 are the interior image width/height, independently consumed
by note builder `0x341fb0`.

The adapter records only an explicit native layer/texture whitelist. It matches
both atlas ID and every vertex UV against the texture's interior rectangle;
mixed/unknown geometry falls back. Packed vertex flag8 means UV first; flag16
adds two fields, flag4 adds colour, flag2 adds XYZ, otherwise flag1 adds XY.
This order is visible in `0x338900`. No geometry or timeline is rewritten.

The original dispatcher still sets texture0, vertex transforms and additive
colour. The draw hook temporarily binds a small pixel shader and media texture1.
It samples the original alpha for shell/decision/transition masks, maps the
original tile coordinates to an aspect-preserving crop of the media, and keeps
vertex colour. Character artwork uses its own alpha and a contain transform.
Shader, texture1, sampler1 state and the three used constants are saved/restored
around the original synchronous draw. Native custom shader modes >1 are skipped.
The shader source and generated DWORDs are shipped together; rebuild with
`scripts/build-artwork-shader.ps1 -Fxc <Windows-SDK-fxc.exe>`.

Provider IO and decoding remain on workers. Draw hooks only consume owned
prepared pixels through the existing shared GPU media leases. The shader belongs
to the graphics thread; mapped media remains owned through each draw. Layer
release retires mappings before reuse and releases unused media; device reset
clears mappings, leases and shader, falling back until native layers are rebuilt.

## Located artwork

- Gameplay initializer `0x8ed420` creates `center_frame_sp` / `center_frame_2pp`
  / DP variants at priority173 and `1p_frame` / `2p_frame` / DP side frames at171.
  Judge/keys and live information are separate objects. Only
  `center_frame_sp`, `center_frame_2pp`, `unit_body_base` texture tiles are
  recoloured. Their original alpha retains the frame openings. The centre
  texture interior is 1920x1080; exported PNG adds a one-pixel border.
- Decision initializer `0x5f3160` creates `music_decide_1p` or `_1p2p` and a
  separate `music_decide_chara_01/02/03`. Ordinary selection calls it at
  `0x8d9330` with component scene+7000. `34decide_bg` is the background target.
- The moving `34decide_zinrai_01/01b/02/03/04/06/08/09/11/14` textures live
  inside that decision timeline. They are the separate transition-art target.
  `music_decide_flash` (created by `0x5f3590`) is a distinct mostly vector
  timeline; it is preserved, not incorrectly replaced with a fullscreen bitmap.
- `frame_chara_01/02/03` AFP roots have 1920x1080 canvases, but the actual image
  tiles are approximately842x1080 /998x1076. This explains why placing arbitrary
  cutouts on the old fullscreen canvas gave the wrong size/position. The new
  adapter retains each native child transform, removes transparent input margins
  on the worker, and contains the cutout inside the native tile. Main and matching
  noise textures are mapped. Small decorative glitch/petal elements stay stock.

## Note batch adapter

`0x8fcbe0(this, player:i32, type:u32, x:i32, y:i32)` accepts types0..3 and uses
`notes_white`, `notes_black`, `notes_turn`, `notes_auto_turn` from table
`0x103fe20`. `0x341fb0` appends a quad to the player batch. Normal stock interiors
are52x15,40x15,90x15,90x15 respectively; length variants are separate atlases.

`0x342730(batches, priority:i32)` synchronously flushes the two batches rooted at
`0xc38f720`, stride86096. Batch count is+0, configured priority+4, texture ID+28,
and vertices+80. Each note is six vertices of28 bytes (XYZ, RHW, colour, UV),
ordered TL/TR/BL/TR/BL/BR. Native priority109 maps to render pass101.
The adapter checks the exact batch base, priority and count1..512, retains the
native vertex positions/colour/clipping, substitutes the texture ID and crops
each quad's UV to its native aspect. The original flush resets the count after
DrawPrimitiveUP; the saved texture ID is restored afterward, and the media lease
is held through the draw. Charge-note bodies use other paths and stay stock.

## Reproduction and limits

1. Hash the exact game DLL and inspect the guarded entries in its Rust profile.
2. Copy only the listed archives into a local ignored analysis directory. Extract
   there; inspect texturelist interior UVs, AFP timelines and geometry references.
3. Enable `note_skin`, `gameplay_shell`, `decision`, `transition` in `[surfaces]`.
   Configure their `[sources.<name>]` sections using the same file/folder/provider
   controls as other modules. Old configs default all four to disabled.
4. For visual discrimination, use different conspicuous media for each target,
   plus a transparent character cutout with generous empty margins. Enter song
   selection, confirm a song, capture the moving decision pieces, then play.
5. Check note alignment, lane openings, native title/score visibility, repeat scene
   entry/exit, disable/fallback, video motion and GPU reset. Check both play sides
   and DP separately before claiming those variants visually verified.

At initial implementation, actual in-game tests are pending. Additional custom
gameplay frames, charge-note bodies, unusual shader effects, DP, second player,
and real device reset are not yet visually verified. Exact hash guards do not
turn those unknowns into compatibility claims. Missing mappings stay stock.

## Native resource-table adapter (supersedes the per-draw prototype)

Same game and supporting-library hashes as above. Additional exact guarded RVAs:

| RVA | ABI / evidence |
| --- | --- |
| `0x328b40` | `usize()`; AFP frame submission, called inside BeginScene by `0x9248f0`; plugin composites before forwarding |
| `0x343e10` | `IDirect3DTexture9*(u32 native_id)`; borrowed stock texture; completes pending staging upload |
| `0x343f30` | Same signature; looks up the stock pointer in the native replacement table and returns effective borrowed texture |
| `0x344950` | `IDirect3DTexture9*(u32 native_id, IDirect3DTexture9*)`; swaps/erases table entry, returns old pointer, does not AddRef/Release |
| `0x344100` | `usize(u32 native_id)`; releases staging, replacement and original COM references, then retires native ID |
| `0xc66d78` | IAT afp-utils ordinal67: AFP texture ID -> native texture ID |
| `0x33af10` | `u8(archive, index:i32, base_path, name, flags:i32)`; archive load/reference increment; +264 bounded full path, +1292 refcount, +1296 load state, +1300 AFP ID |
| `0x33b500` | `usize(archive, force:u8)`; archive reference release; state2 remains live, final release zeros ID/path/state |

Disassembly of `0x344100` verifies RCX receives the replacement pointer from an
out-parameter and calls vtable+16 (Release) at `0x34418c`; this is not the null
pointer shown by incomplete decompiler types. `0x344950` contains no COM ownership
calls. The plugin transfers a cloned COM reference into the table and retains its
own graphics-owner reference. Native free queues integer IDs only; graphics owner
retires its leases afterward. Explicit disable/reset detaches only a table entry
that still points to our texture, releases the returned reference, then drops its
own. An existing third-party override is declined, not overwritten.

A replacement atlas has the native dimensions. A GPU pass copies the stock atlas
and composites only configured tile rectangles; alpha masking is optional for
named assets and enabled for the shell. Unconfigured pixels, native geometry,
UVs, timeline, shader effects, draw ordering and clipping stay native. D3D state,
render targets0/1, depth surface and viewport are restored. Static atlases are
prepared once per resource/config revision. The unchanged static callback takes
an atomic revision fast path; it does no texture lookup/upload/draw. Videos reuse
the atlas and shared source texture, recompositing only for a new owned CPU frame.
This adds necessary image-to-video work; no zero-cost video claim is made.

The old AFP dispatcher/draw and per-note batch hooks are no longer installed.
`0x342730` calls `0x338570` before its native batch draw, which uses the same
replacement table. The four named note tiles can therefore be replaced in normal
and long-note archives without editing a single note's vertices.

Generic `[[artwork]]` entries identify a relative graphic archive and texture
name. Archive load/release observers permit any named texture in that archive,
not only the preset layers. Provider preparation remains worker-only. Explicit
named entries take precedence over preset mappings of the same tile.

## Full installation inventory

`scripts/inventory-artwork.py` copies each IFS to a local analysis tree before
opening it with ifstools. It records SHA-256, size, file types, texture names,
atlas format/dimensions and interior pixel rectangles. UV rectangle integers in
the inspected metadata are half-pixel units (divide by2). Local inventory:
`analysis/asset-inventory-20261024/inventory.json`; no game files committed.

The scan found4520 `.ifs` paths and106370 named texture entries. 21 paths contain
identical256-byte non-IFS data and are explicitly reported as unresolved inputs;
they are not silently counted as decoded archives. The inventory additionally
contains AFP/geometry and other non-texture files. A texture catalog is not proof
that non-textured vectors, generated text, external movies or every gameplay
mode are replaceable or visually verified. Those paths need separate evidence.

## Live evidence so far

- Atlas build SHA-256 `a29af5b73a4a5f8ace02bd74bfe60c3fd0a693a0cc9e9697175186bcd5e5a4dc`
  attached on exact game hash above after a mouse-controlled launch/UAC click.
- Demonstration: orange centre shell and green notes visible, both player lanes,
  native labels and movie unobstructed. `verification/atlas-shell-notes-demo.jpg`.
  Shell atlas preparation CPU times1602us and2422us (IDs1142/1143). This is CPU
  submission time, not a GPU/frame-time benchmark. Notes still used the older
  batch adapter in this particular screenshot; next build removes it.
- Selection: transparent test cutout follows native child placement, but the top
  extends under the HUD. `verification/character-before-inset.jpg`. A subsequent
  contain inset is being tested; do not call that correction verified yet.
- Initial fixture paths failed with OS error3; UNC self-access failed1326. Tests
  now use owned fixture files under `D:\IIDX34\iidxbg-artwork-test-20261024`, outside
  modules/assets. Original remote TOML is backed up and restored only if unchanged
  from the test copy. These failures are not renderer-success evidence.
- Generic entries, video atlas composition, disable/reset, performance tails and
  broader modes are pending the next verification loop.

## Direct movie resource path (2026-10-24)

Input is the same exact game SHA-256
`c98e183ec81ecb9e1cb13c237f9c9cb0b3f65b6fdaff6fd5e6a526008c546534`.
`0x9374c0` returns the singleton movie wrapper at RVA `0x1041e50`.
Both `0x9248f0` (normal frame) and `0x8e46e0` (alternate scene rendering)
call `0x9362b0(wrapper)`. It checks enable bytes +16/+36, then calls
`0x936300(wrapper,x,y,w,h,filter,alpha)` using fields +20/+24/+28/+32/+37/+40.
The latter retains native screen geometry and draws a triangle fan directly,
bypassing AFP's texture-ID table. Its movie interface is a shared_ptr at +0/+8;
virtual +96 returns a shared_ptr to AfpTextureEx. At `0x93658b` it calls
`0xa4fd50(wrapper,out_com)` to obtain the D3D texture.

`0xa4fd50` has ABI `void**(usize wrapper, void** output)`. It copies wrapper+8
into output, calls COM vtable+8 (AddRef), and returns output. The caller releases
that reference. Entry guard: `40534883ec3048c7442420feffffff48`. The adapter always
calls this getter, then replaces its owned reference with one cloned owned
reference only inside the original movie draw's thread-local scope. The source
GPU lease survives the entire call. Other getter users (`0x7502b0`, `0x98ecd0`)
remain unchanged outside that scope. Native movie enable/alpha/geometry and
original drawing are preserved. No movie shared_ptr layout is synthesized.

Catalog virtual resource `@movie` / `main` targets this singleton movie layer.
Its media choice freezes until the native movie identity changes; reset drops
GPU resources while retaining the CPU choice. This covers the main movie layer,
not an assertion that every external video player uses this class. Live
verification of the new getter adapter is pending. Reproduce with a named
artwork entry for that virtual resource, fixed local video, and compare title,
song gameplay and scene exit captures. Unknowns: native movie pointer reuse
without an intervening null/update; independently instantiated movie classes;
non-textured AFP vector and dynamic font paths remain separate analysis work.

## Verification and performance follow-up

Live build `04c06f09882ce076632fa44318b2ba6f45f2158b73e37a38a146aab18f5e8497`
was tested on the exact game above. Mouse launch and UAC confirmation succeeded.
Evidence is local under `analysis/custom-art-20261024/verification/`:

- `movie-and-lane-video-a.jpg` / `movie-and-lane-video-b.jpg`: distinct moving
  frames on the native main movie and lane atlas simultaneously, both demo lanes,
  with native notes, UI, clipping and right-lane mirroring preserved.
- `decision-and-moving-pieces.jpg` (preceding b0d91c build): blue decision canvas,
  magenta native animated transition pieces, stock song title/difficulty/mascot.
- `character-inset.jpg`: entire arbitrary transparent cutout fits between HUD
  boundaries. Additional native glitch fragments still showed the old character;
  catalog inspection identified five more `frame_chara_01_eff_noise*` textures.
  Their subsequent mapping change requires another visual check.
- `disabled-stock-fallback.jpg`: after a valid disabled snapshot and the next
  scene, native shell, lane and movie returned. No replacement leaked over
  ordinary result scenes. This does not yet prove an actual D3D device reset.
- `catalog-thumbnails.png`: 106370 entries loaded with native-aspect previews,
  irregular adjacent mosaic, hover details and pagination; catalog reopened
  from the saved local catalog-path preference.

Opt-in `profile_artwork=true` measures CPU callback time excluding the original
AFP frame and excluding the separate movie getter adapter. Settled static
samples had mean~0.4us, p95~0.7us, p99~0.9us. First GPU preparation/upload produced
one-time tails of~18-24ms. A 1920x1080 video decoded for a 440x728 lane measured
mean0.577ms, p953.136ms, p993.719ms (sample included first preparation). Native-size
440x728 preparation measured settled mean0.153-0.156ms, p950.744-0.747ms,
p990.893-0.898ms, max1.11-1.23ms. These are submission/callback measurements, not
GPU timestamps or a controlled total game frame-time benchmark. Source texture
leases and copied native atlases consume additional memory; unchanged static
rendering itself uses the original native binding/draw path with no extra draw.

## Geometry inventory

`inventory-geometry.py` reads only copied local IFS files. It hashes each GE2D
input and records draw flags, region names and texture-less draw counts. Header
interpretation was cross-checked against DragonMinded/bemaniutils commit
`1e66fcd780afb3088ce8e78c88615a143db492ee`, `bemani/format/afp/geo.py`.
It parsed273660 shapes without errors;40201 contain at least one untextured draw.
The inventory also contains60342 AFP files. These counts describe resources,
not unique visible objects or proof of runtime coverage. The21 non-IFS placeholder
inputs have SHA-256`8628dd304f13be8cdcc7b6a5f1946c8b7ba34818cb50dd5d94c8d122e8981e42`.
Texture-less geometry cannot be made replaceable just by enumerating atlas names;
its material/UV and owner lifetime require a separate adapter. Generated font
content also remains outside named-raster coverage. Analysis is ongoing.

### Shape resource ABI investigation

Input: afp-utils.dll SHA-256
`b74a538ee13603d8d9d695a6c14b296ac532685528923b30e125d0197d706322`;
all addresses here are RVAs in that library, not in bm2dx.dll.
Ordinal89 (`0x119f0`) forwards to `0x10550(out,id)`, which resolves a shape
with `0x13960` and fills a 32-byte borrowed view through `0xdb90`.
The shape work record is 48 bytes: ID +0, parent AFP ID +4, flags +8,
shape number +10, region count +12, geometry pointer +16, region bindings +24,
cached name +32. Resolver validates `(id & 0x78000000) == 0x18000000`,
package shape count at +26, and the record's stored ID before returning it.
The output view contains ID +0, geometry pointer +8, bindings +16, name +24.
Names are lazily allocated by `0xfcd0` from the parent name and shape number.

`0x137c0` lazily loads and checks UVs. `0xcc40` modifies geometry UVs in place:
for draw flag 0x42 it maps normalized coordinates through region bounds and
clears bit0x40. Region count one stores a direct region pointer; larger counts
use an array. `0x13800` checks missing bitmap bindings and sets ready bit4.
These are borrowed native records, not safely replaceable owned buffers.

The important correction is that the getter is not the render boundary:
`0x15420(shape_id,multiply,add,params)` and queued command handler `0x1d960`
resolve the work record directly, then call `0x1a420(work,multiply,add,params)`
for GE2D or `0x1b370` for the distinct vector format (work flag0x20).
`0x1a420` transforms vertices and calls `0x16ee0` per primitive. The latter
reads region+16 -> texture record -> AFP texture ID, builds a render packet,
and copies vertices into the native queue. Mask rendering can intentionally
omit texturing. Merely hooking ordinal89 would therefore miss actual drawing.

Reproduction: open the exact copied DLL in IDA, follow ordinal89, then string
xrefs for `draw_shape`, `call_draw_shape`, `afpu_render_draw_geo` and
`draw_geo_primitive`; compare GE2D offsets with the local geometry inventory.
At this intermediate analysis stage no shape hook had been installed. Unknowns then
include package-retirement synchronization, the separate vector format,
generated glyph ownership and a supported way to register a synthetic native
texture binding. A future adapter must preserve masking, batching and native
queue lifetimes instead of assuming a borrowed getter controls all consumers.

### Temporary GE2D draw adapter

Further tracing found `afp-utils+0xfa30` frees geometry +16, name +32,
auxiliary +40 and multi-region bindings through its own allocator at +0x1d040.
The implementation therefore never replaces fields in a native work record.
It clones an eligible GE2D resource once, appends planar UVs, preserves polygon
indices and alpha, and passes a temporary 48-byte work view to the original
`0x1a420`. That routine's `0x16ee0` copies transformed vertices and material IDs
into the native queue; it does not retain the temporary geometry/work pointers.
Mask handling, native draw order, transforms and blend/add colors remain native.

The synthetic binding uses the existing AfpTextureEx constructor in bm2dx
`0xa4f8e0`, which calls `0x344b60` and stores a direct native handle at +16.
Binder `0x338570` first looks up this direct handle through `0x343830`; only if
absent does it use afp-utils ordinal67. Thus no fake AFP package/texture ID is
registered. GPU leases remain graphics-owner objects across queue consumption.

`@shape/<native-shape-name>` entries select these resources by archive. Only
fully untextured flags9 GE2D triangle resources are accepted; malformed,
mixed-texture, degenerate or unknown geometry falls back to stock. Source data
is bounded and indexed geometry is validated. The separately hashed afp-utils
profile checks image size0x42000 and live entries for +0x119b0/+0x10550/+0x1a420.
An unknown optional library disables this adapter, preserving raster support.
This adds a cheap inactive draw-hook branch; active shapes use a cached geometry
copy and shared texture lease. It is not claimed to have zero CPU overhead.
Pure conversion/fallback tests pass. Live queue and scene checks subsequently passed as recorded below; actual device-reset verification remains outstanding.

The expanded geometry audit found933 distinct untextured input hashes.29110
records have a16x16 magenta rectangle,5009 white,2285 black and1266 red fills.
These are resource observations, not proof each is visible: masks and timeline
placeholders also use shapes. They must not be counted as40201 unique artworks.

Font analysis: afp-utils `0x55e0(font_id,code,out)` resolves font records and
uses `0x2ca0`/`0x2d40` to copy glyph bind ID, atlas UVs, offsets, dimensions,
advance, ascent/descent/leading into a68-byte output. Missing bind IDs can use
outline data at output+48. This is separate from named texture regions and is
not covered by the GE2D adapter. No font hook has been installed.

Live build4cd755 exposed a catalog matching bug: the native archive-load observer
reported `/0/common.ifs`, `/0/gameparts.ifs`, `/0/graph.ifs`, etc., whereas the
inventory keys omit the leading slash. Root `gmframe34.ifs` happened to match,
so the earlier lane test did not exercise this case. `archive_key` now removes
the known graphic prefix, normalizes separators and empty path components.
Regression cases cover `/data/graphic//0/basic_frame.ifs`, relative forms and
Windows separators. The native loader at bm2dx+0x33af10 loads synchronously and
sets state+1296=2 before successful return, so this was a path mismatch rather
than an assumed asynchronous ID-readiness problem.

### Alternate archive paths and cache identity (2026-10-24)

Input remains game SHA-256 c98e183ec81ecb9e1cb13c237f9c9cb0b3f65b6fdaff6fd5e6a526008c546534.
RVA 0x33ac50 is the six-argument normal archive opener (object, index, base,
name, flags, RAM byte), returning a success byte. The RAM branch calls 0x33af10;
both publish state+1296=2, package ID+1300, full path+264 before successful return.
The new observer forwards first and deduplicates object/package/path: native
retain calls must not invalidate an unchanged static GPU cache.

The already-mounted opener 0x33b2f0 instead stores its logical name at +8 and
mount path at +520, without filling +264. Manager 0x33c840 calls this from
resource job 0xa46750, whose internal branch obtains a relative name from
0x818bc0. Its external branch strips the full filename to a basename. Layer
creation 0x32da80 also observes its retained archive, covering mounted jobs.
No ownership transfer occurs; retirement remains synchronized with 0x33b500.

Live build 5e549a9825057939be9d7b3879b3cf655ed2ac3425d58482bf0d5d875027896d
reported mounted logical names 0/sub_card.ifs, 0/card_j.ifs and 0/card.ifs on the
card-entry scene. These contain the catalog-relative path. The observer now
accepts such relative paths; basename-only external names remain @mounted/name,
rather than guessing which same-named archive they came from. Reproduce by
starting a guest credit and inspecting profile_artwork archive diagnostics.
Unknown: external basename-to-original-file mapping is not reconstructed.

### Preset coverage correction

The complete texture inventory contains 47 region names across root notes*.ifs
archives: ordinary notes plus CN/HCN, BSS/HBSS/automatic scratch and MSS
start/body/end/joint variants. The note preset now resolves this complete name
set through the same verified ordinal71 ABI; missing names are skipped. This
requires no new native hook or geometry changes. All retain their native alpha.
Reproduce by grouping inventory.json texture names by the notes archive prefix.

The five numbered frame_chara_01_eff_noise01..04/eff_noise_line resources are
line/glitch decorations, not character silhouettes. They remain independently
replaceable in the catalog but are no longer automatically filled with the
full character cutout by the character preset. The three principal character
textures and their corresponding unnumbered silhouette effects remain mapped.


### Corpus conversion and coverage audit (2026-10-24)

The user scoped this work to visual artwork, keeping generated text/fonts and
3D models functional. No font or model-geometry hooks are installed.
The local inventory covers 4,520 IFS inputs, 106,370 named texture regions and
273,660 GE2D resources. All parsed GE2D primitives are flags3 (216,920), flags67
(16,539), or fully untextured flags9 (40,201); there were no geometry parse
errors. The untextured inputs have 933 distinct hashes, not 40,201 unique images.
The actual Rust converter passed all 933 at landscape, portrait and square
source ratios using the ignored `all_audited_untextured_shapes_convert` test.

Raw big-endian GE2D must first undergo the native endian conversion. In the
above afp-utils hash, RVA 0x14bf0 (`afpuloc_bin_geo_swap_data`) swaps header
DWORDs 0/4/8/12/16/32/36/40/44/48, WORDs20..30, optional flags4 bounds52..64,
vertex/UV pairs, label pointers, each draw's WORD+4/DWORD+12 and index WORDs.
RGBA bytes remain unchanged. `export-shape-corpus.py` implements this conversion
on local copies, recording source and normalized hashes. Its first test run
incorrectly supplied raw big-endian data and failed; normalization corrected
that harness error. No game data is converted on disk.

Reproduction: run inventory-artwork.py and inventory-geometry.py into an ignored
local analysis directory; export-shape-corpus.py on that directory; set
IIDXBG_SHAPE_CORPUS to its shape-corpus directory and run the ignored test.
Each input archive is hash-checked against the inventory before export.
Run audit-artwork-coverage.py to reproduce the reference audit and its input
JSON hashes. Of 70,082 archive-local region-name references, 65,221 have a
same-archive texture, 4,700 have candidates in other archives, and 161 references
(98 distinct names) lack an IFS texture candidate. Cross-archive names alone do
not prove the native binding; some missing names are loose images, as below.
Twenty-one 256-byte IFS inputs all have SHA-256
8628dd304f13be8cdcc7b6a5f1946c8b7ba34818cb50dd5d94c8d122e8981e42 and are not
parseable IFS. Their contents are unknown; do not claim complete coverage of
those inputs or that every catalog item was individually displayed in-game.

### Loose PNG/JPEG path

Same exact game hash as above. The graphic tree additionally contains 374 PNGs
and 822 JPEGs, outside IFS, plus three Windows thumbnail databases. The latter
are not artwork. `inventory-loose-artwork.py` copies images to the local analysis
folder before decoding, hashes every source/copy, and appends @file entries.
No image or thumbnail is committed to the repository or written to game assets.

RVA 0x6d40b0 formats assistant_chara/assistant_chara%03d.png; 0x8815b0 formats
movie_thumbnail/%5d_thum.png. ThreadedTextureLoader entry0x6ac140 starts worker
0x6ac8e0, which calls 0x6abd90(this, shared_ptr_out, const char *path). The latter
reads via AVS, calls decoder0x6abed0 (D3DXCreateTextureFromFileInMemoryEx), then
0xa4fb60 constructs a shared AfpTextureEx. The observer forwards first, reads
only a successful owned output, and records path, handle+16, COM identity+8,
and descriptor dimensions+28/+32. No provider IO or GPU work runs in this hook.

AfpTextureEx constructor0xa4f8e0 registers the COM texture through0x344b60.
Destructor0xa4fa70 calls unregister0x344c30 BEFORE releasing wrapper+8. The
unregister observer removes integer metadata before forwarding; on the graphics
owner, acquisition of a temporary owned original COM reference uses the same
metadata lock. Native pointers are never retained without an owned reference
through composition. GPU state belongs exclusively to graphics-owner update,
bind and invalidate. A serial number rejects reused native handles.

Direct handles bypass the atlas override map: binder0x338570 first calls direct
lookup0x343830 and sets texture0 directly. The adapter forwards this binder,
then substitutes a prepared texture only for a live configured loose handle.
The already-guarded COM getter0xa4fd50 also supports configured loose handles,
retaining its AddRef/Release return contract. Geometry, text, animation, masks
and native shader setup still run. Composition preserves source aspect and can
retain native alpha. Static pixels compose once; videos compose on new frames.
This direct binding path has an extra texture bind for matched resources; it
must not be described as identical performance to a native static image.

Reproduction: configure archive='@file' with texture equal to the path relative
to data/graphic, e.g. assistant_chara/assistant_chara000.png. Open the native
assistant menu and inspect both screens and profile_artwork loader diagnostics.
Entry bytes for load0x6abd90, unregister0x344c30 and bind0x338570 are included
in the exact-build profile. Live verification is recorded separately below.


## Live verification and performance outcome

Actual in-game testing was performed on the exact game hash above, through
Spice API input and main/subscreen captures. Moonlight mouse input also confirmed
launch/UAC successfully. No game DLL or artwork file was edited. The game was
stopped after testing and its original TOML restored byte-for-byte after checking
that it still matched our temporary test copy. The final native test plugin is
SHA-256 83f1396c458d25f59e9b41c525ee9e0249225f51bc56e8464fc25586ccdfa863.
The previous atlas/shape run was b7d9c6ac4e57cbd1b7b9a0d66cba974c824078325241ef4c610c1a19652909bb.

Private captures/logs are under analysis/custom-art-20261024/verification:

| Check | Evidence / result |
| --- | --- |
| Notes and shell, lane video, native movie | Earlier gameplay captures; movie-and-lane-video-a/b show motion on both native paths |
| Decision and moving transition pieces | decision-and-moving-pieces.jpg; native foreground and text retained |
| Transparent selection character | character-and-shared-shapes-b7d9c6.jpg; principal artwork placed within native character region; decorative glitch lines retained |
| Untextured GE2D static/video | shape-mode-static-8f98b5.jpg and shared-shape-motion-a/b.jpg; video behind native selection controls |
| Catalog subscreen region and native alpha | catalog-subscreen-native-mask-b7d9c6.jpg and catalog-subscreen-result-b7d9c6.jpg |
| Loose PNG static/video | loose-assistant-static-1bd475.jpg, loose-assistant-video-a/b-1bd475.jpg and loose-final-video-83f139.jpg; portrait plus header icons, native speech bubble/menu unaffected |
| Disable and scene change | disabled-stock-main/sub-b7d9c6.jpg and loose-disabled-stock-83f139.jpg; stock art restored |
| Catalog UI | catalog-loose-147762.png; all 147,762 entries loaded, loose assistant search and source editor exercised |

The loose loader observed assistant_chara000.png as128x150, direct ID2053 in
the tested visit. 1190 of1196 loose images decoded; six movie thumbnails are
unreadable. Five have the same8628dd30... hash as the invalid IFS inputs; the
remaining252-byte34083_thum.png has hash
3e30387c43f8aa8fcc9c420e4f9f3c1b40ba49934869c565cdc59bdab79f99b8.
The catalog total is106370 atlas regions +40201 shape records +1190 loose
images +1 native movie entry. Catalog count is not a count of unique artwork.

The following CPU timings use opt-in profile_artwork,1200 samples per batch.
They measure plugin callback preparation or matched binding, excluding forwarded
native draw work, GPU completion and background decoder CPU. They are not an
end-to-end FPS benchmark or a guarantee for other machines.

| Workload | Mean | p95 | p99 |
| --- | --- | --- | --- |
| Settled static atlas callback | 0.4?0.6 us | 0.7?1 us | 0.9?1.5 us |
| Lane video decoded at1920x1080 | 0.577 ms | 3.136 ms | 3.719 ms |
| Same lane at native440x728 | 0.153?0.156 ms | 0.744?0.747 ms | 0.893?0.898 ms |
| Three independently decoded shape videos | about1.85 ms | 7.55 ms | 9.49 ms |
| Three shapes bound to one decision video | 0.65?0.75 ms | 3.15?3.69 ms | 3.43?3.98 ms |
| GE2D dispatch lookup (mixed matched/unmatched) | 40?70 ns | 100 ns | 100 ns |
| Main-movie preparation, active video sample | 0.141 ms | 1.461 ms | 1.704 ms |
| Loose-image matched texture bind, settled83f139 batches | 0.164?0.169 us | 0.4 us | 0.7?1.0 us |

The final mixed tutorial workload (lane video + shared shape/assistant video)
measured about0.88?0.99ms mean in the artwork preparation callback. Initial
scene/upload allocation can still spike18?36ms in earlier workloads; static
steady-state timing must not hide those first-use costs. Native-size decode
limits and explicit shared bindings are implemented, not merely suggestions.
Static atlas regions do no recurring upload/composition after their revision is
applied. GE2D and loose files retain small dispatch/bind costs. Originals plus
replacement/source textures consume extra VRAM. Existing native movie decoding
continues for fallback, so video-to-video replacement does not have identical
whole-process CPU cost to the original game. Video-enabled entries are prepared
at scene events even if their archive is unloaded; visibility-demand preparation
is not implemented. No claim of zero-overhead or exact memory parity is made.

Coverage boundary: all readable resource classes found by this graphic-tree
inventory now have adapters, but not all147762 entries were individually visited
in-game. Online/event-specific timelines, every CN/HCN/BSS variant in actual
play, external mounted basename resolution and a real D3D9 Reset/ResetEx cycle
remain unverified. Shutdown/invalidate is not evidence of a real device reset.
Unknown vector formats retain stock rendering; none were present among the
273660 audited GE2D resources. The unresolved-name report remains an evidence
gap, not a claim that absent or obsolete textures are valid replaceable artwork.

Final checks:58 core tests passed (five environment/corpus tests ignored in the
normal suite); the private933-input shape test passed separately;15 UI tests
passed. Core/UI Clippy with warnings denied, formatting, release builds,
verify-native.py against the exact game binary and smoke-dll.py passed. The UI
preview regression test checks independent resource paths/native dimensions and
explicit bindings, preventing previewing the unrelated transition preset.


The full147762-entry catalog also passed the ignored
`every_catalog_entry_is_configurable` test with IIDXBG_ARTWORK_CATALOG pointing
to the generated JSON. This found and fixed a validation mismatch: resource
canvases can be smaller than the64-pixel minimum used for ordinary backgrounds.
Only derived resource configurations now allow a2-pixel decoder floor; normal
background validation is unchanged. One-pixel/16-pixel artwork has a regression
test. This final configuration-only change followed native83f139 testing; it
introduces no hook or ABI change. The catalog preview was separately exercised
with a local blue-grid image (catalog-source-preview.png); it uses that resource's
source/native decode limit and explicit binding rather than the transition
preset. Stale preview outcomes are rejected after edits or changing entries.


Final built/deployed artifacts after the configuration validation and preview
corrections: iidxbg.dll SHA-256
 afd437ae3f4c5310fafa790e988996de28d351ce48e109131c3f4b9953eb0266;
iidxbg-config.exe SHA-256
 d7d3abf3593f0ad3d0606765b05253abf2b2f6c5f183b11693712380c1af1424.
The final native rendering implementation was live-tested in83f139; the later
resource-size validation change passed the full catalog and core tests without
another game launch. Previous executables are backed up locally. Temporary test
media was removed from the remote test directories only after comparing local
fixture hashes. Original game configuration is restored and the game remains
stopped. No commit/tag/push was made for this implementation pass.
