# IIDX 34 / 010-0917 results hook: evidence

Recorded 2026-10-22 using local IDA Professional 9.4 (Hex-Rays), PE exception
tables, MSVC RTTI, Capstone and Unicorn. This is a second inspected IIDX 34
executable, not automatic support inferred from the version number.

## Input identity

- Supplied file: `C:\Users\BeatMania IIDX LM\Downloads\010-0917bm2dx.dll`
  (same bytes as `010-0917bm2dx (1).dll` in that folder).
- Size **18,200,216** bytes; SHA-256
  `c98e183ec81ecb9e1cb13c237f9c9cb0b3f65b6fdaff6fd5e6a526008c546534`.
- PE timestamp **2026-09-17 02:34:47 UTC**; image base `0x180000000`,
  SizeOfImage `0xc5ff000`, entry RVA `0xb02b7c`.
- Label **IIDX 34 / 010-0917**; patch identity `LDJ-6aab51c7_b02b7c`,
  sp2xpatcher version **2026-09-17 (010)**; contract file
  `src/profiles/iidx34-010-0917.json`.

The previously supported IIDX 34 hash `3755...fe83` has patch identity
`LDJ-6aa23d57_b02b3c`, sp2xpatcher version **2026-09-16 (010)**, a 2026-09-10 PE
timestamp and entry RVA `0xb02b3c`. Both PEs have the same image size and
57,024 exception-table regions. Their data sections have identical virtual
addresses; after `0x5afa9b` most functions are shifted by `+0x20`, `+0x30` or
`+0x40` because five regions outside the plugin's hook set changed size. Every
function this plugin calls or hooks matched the earlier profile after masking
rel32 and RIP-relative displacements. IDA names the changed regions as:

| Old RVA | New RVA | IDA evidence |
|---|---|---|
| `0x5afa9b` | `0x5afa9b` | cold block of `sub_1805af8e0`, a boot/state machine using `timeGetTime` and `avs2_core_224` |
| `0x5c9f00` | `0x5c9f20` | `sub_1805c9f20`, closes the `dlbg` debug layer (`close_loop` / `close_in`) |
| `0x5c9f80` | `0x5c9fa0` | `sub_1805c9fa0`, creates the `dlbg` debug layer |
| `0x5ca0c0` | `0x5ca0f0` | `sub_1805ca0f0`, opens the debug layer (`open_in`) |
| `0x94fb10` | `0x94fb50` | `sub_18094fb50`, common scene helper called from selection and result initialization; exact gameplay role not proven |

None of those entries is in the profile's call or vtable set. That is static
equivalence evidence, not a claim that the changes are unrelated to gameplay.
The common scene helper is called by `sub_1808d7bb0` (selection init) and
`sub_1809140c0` (result init), so its changed body is the main remaining
behavioral unknown.

## Results evidence

| RVA | Evidence |
|---|---|
| `0xd72950` | RTTI `.?AVCStageResultScene@@`; slots 16/17/18 = init `0x9140c0`, cleanup `0x9155c0`, update `0x915b50`; slot 27 = `0x915a50` |
| `0x911ea0` | Results constructor; `0x911f0e` loads `scene +0x108` (264), `0x911f15` calls background constructor `0x906b80` |
| `0xd70598` | RTTI `.?AVStageResultDrawBg@@`; slots 0/1/2 = `0x906d70` / `0x906da0` / `0x906eb0` |
| `0x906b80` | Background constructor; archive pointer at background +272, difficulty at +280 |
| `0x906eb0` | Background callback; movie clip at background +296, finds `illust`, binds archive `result_bg` or hides the clip |
| `0x81ea30` | Null-checked named clip lookup; tail-jumps to unchanged `0x32d1a0` |
| `0x329f30` | Archive/name texture setter through `afp_core_137` |
| `0x329e50` | Dynamic-descriptor setter through the same import |
| `0x32a1c0` | Visibility helper through AFP property 4103, dirty 4126 |
| `0x32a460` | Native archive-center helper `(clip, 3, 3)` |
| `0x32a3e0` | Pixel-origin helper `(clip, f32 x, f32 y)` through property 4123 |
| `0xa4f8e0` | `AfpTextureEx` constructor wrapping a D3D texture; AddRef, register, descriptor |
| `0xa4fa70` | Non-deleting destructor; unregisters `+16`, releases COM texture `+8` |
| `0xa4fb20` | Deleting destructor in `AfpTextureEx` vtable `0xd9b358` |
| `0xa4fd80` | Descriptor getter: `lea rax, [rcx + 24]; ret` |
| `0x344b60` / `0x344ea0` | Texture registration / 40-byte descriptor construction (unchanged) |
| `0x344c30` | Texture unregister (unchanged) |
| `0x922a90` | D3D device getter from root global; returns device +224 |
| `0x912be0` | Result deleting destructor; calls `0x912500`, frees on flag bit 0 |
| `0x63da20` | Native assistant example: descriptor getter then dynamic setter |

IDA pseudocode for `0x906eb0` confirms the branch:

```text
clip = find_named_clip(background +296, "illust")
if clip:
    if background +272:
        set_archive(clip, archive, "result_bg")
        set_origin(clip, 3, 3)
    else:
        set_visible(clip, false)
```

The `illust` string is at RVA `0xd70620`, `result_bg` at `0xca56f0`, unchanged.
The callback also updates title, artist, difficulty, stage and timers; it is
still called through its original entry.

## ABI, layout and ownership

Microsoft x64 conventions are unchanged: background `(this, u32 timing)`,
cleanup `(scene)`, scene delete `(scene, u8 flags)`, setters as documented for
the earlier 34 build. Scene/background layout is also unchanged:

| Offset | Meaning |
|---|---|
| scene +264 | `StageResultDrawBg` |
| background +272 | result archive pointer |
| background +280 | difficulty |
| background +296 | result movie clip |
| wrapper +0 / +8 / +16 / +24 | vtable / owned D3D texture / registered u32 / 40-byte descriptor |

The wrapper constructor initializes all 64 bytes. The destructor unregisters
handle `+16` through `0x344c30` before releasing the COM texture and clearing
the descriptor. Runtime uses stable 8-byte-aligned storage and calls these
native entries; it never treats the wrapper/descriptor as a D3D texture.

## Additional surfaces and frame character

| RVA / data | Evidence |
|---|---|
| `0xd52508` | `.?AVCMusicSelectScene@@` vtable; slots 16/17/18 = `0x8d7bb0` / `0x8d7fa0` / `0x8d8290` |
| `0x9362b0` | Movie draw dispatcher |
| `0x8f4e20` / `0x8f5b20` / `0x8f5120` / `0x8f6390` | Cover delete / init / visual / pair builder |
| `0xa1fd30` / `0xa20d50` / `0xa284e0` / `0xa1f2a0` | Target graph artwork / reset / parent reset / storage release |
| `0x3363e0`, `0x334890`, `0x334de0` | Sprite release observer, manager getter, sprite creation |
| `0x3366d0` / `0x336c10` / `0x336920` / `0x336580` / `0x336b20` | Sprite origin / scale / clip / position / alpha adapters |
| `0x32d350` | Layer dimension helper; `CLayer` vtable `0xd968b0` slot +312 |
| `0xd968b0` | `.?AVCLayer@BM2D@@`; layer +8 handle, root clip ID at layer +528 |
| `0xc66bf0` / `0xc66bf8` | AFP group setter/getter IAT (ordinals 20/21) |
| `0xc66c10` / `0xc66c28` | AFP property setter/getter IAT (ordinals 116/115) |
| `0x8da400`, call `0x8da46e` | Selection initialization; frame at scene +160, builder `0x5cf1f0` |
| `0x5cf4ea` | Frame builder stores character layer at frame +120 (scene +280) |
| `0x5d0a30` | Frame update |
| `0x329bb0` / `0x329c0a` | Root clip creation / root ID store at clip +512 |
| `0x32da80` / `0x32acd0` | Static layer factory / native visibility getter |
| `0xc76290` | `frame_chara_01` string |

`surface_data` keeps the earlier 34 values for `select_frame_chara` (280),
`select_frame_priority` (188), `select_vtable`, `layer_vtable`, AFP imports and
the two zero-valued optional flags. Those offsets were checked in the new
function bodies.

## Premium-area and concentration backgrounds (2026-10-22)

Two more replacement targets were analysed on this same input. Both are
user-customizable in the game itself (`txt_cus_premium_bg`,
`txt_cus_concentration`, `sub_customize_bg%03d`), and both native displays are
**1280x720** canvases inside the 1920x1080 UI. The plugin therefore prepares
1280x720 pixels and copies the native transform instead of rebuilding a screen
rectangle.

| RVA / data | Evidence |
|---|---|
| `0x9af8c0` | Lazy `CPremiumArea` accessor; the object is session-global and starts `sub_1809ab950` once |
| `0x9ab200` | `CPremiumArea` constructor; `PremiumAreaMainControl` shared_ptr at +56, concentration window at +72 after init |
| `0x994650` | `PremiumAreaMainControl` constructor; creates `PremiumAreaBG` at priority **58** through `0x6447a0`, stores it at control +240 |
| `0x6447a0` | `PremiumAreaBG(bg, i32 priority, u8 customize, shared_ptr*)`: base UI layer at **+16**, customization sprite at **+24** |
| `0x9c1ee0` / `0x9c1e30` | Premium UI layer creation: position `(0,1080)`, clip `{0,1080,1280,720}`, anchor flag 1, 48-byte transform `0x80000001` |
| `0xd8bbf0` | Clip constant `{0, 1080, 1280, 720}` (x, y, width, height) |
| `0x334d10` | Customization sprite factory (archive/name, priority, visible); the premium path uses position `(0,1080)` |
| `0x6375c0` | Concentration apply `(window, u32 customize_index)`; called from `CStageMain::Init` at `0x8f952e` with `window = CPremiumArea +72`, index from `0x860680` |
| `0x638420` | Window layer lookup `(window + 8, &index)`; returns the shared_ptr slot; the native caller uses index **1** |
| `0x81ea30` | Existing named-clip lookup; resolves `concentration_bg_set/bg` in that layer |
| `0x6377f0` | Concentration show/hide `(window, u8 show)`; reached through `0x9acf00` from `0x9aeef0` and `CStageMain::Init` at `0x8f9570` |
| `0x860680` | Customization index getter used by the concentration apply call |
| `0xc8df90`.. | `concentration`, `concentration_uioff`, `concentration_bg_set/bg`, `concentration_bg_set/black_cover`, `sub_customize_bg%03d`, `sub_monitor_base_bg` |

The premium background is created once per game session by the lazy
`CPremiumArea`, so its replacement is frozen to the bundle that created it and
rebuilt when the worker publishes a new bundle, when the visible node switches
between the base layer and the customization sprite, or after device loss.
The native base/custom visibility is saved and restored; the plugin never
rebinds a native clip texture, because the layer's root clip keeps the name
`/` and no archive pointer that would allow restoring it.

The concentration clip lives in the session-global window and is applied per
stage. The replacement is frozen per stage and follows the native show flag
(`0x6377f0`), so it cannot outlive the gameplay overlay. The native
`concentration_bg_set/black_cover` and its brightness handling remain
authoritative.

Implementation evidence: the runtime hooks
`premium_area_ctor` (`0x6447a0`), `concentration_apply` (`0x6375c0`) and
`concentration_show` (`0x6377f0`), plus the layer getter `0x638420`. Exact entry
bytes are embedded in `src/profiles/iidx34-010-0917.json`.

The premium replacement is an owned sprite created at the native priority and
kept across scenes. Both the base layer and the customization sprite are hidden
while it is active, and their saved visibility is restored when the sprite is
actually released by the native pool. Its position/origin/scale/color are copied
from the visible node through AFP properties 4104/4123/4099/4106. The first
in-game test showed that without the release-path restore the premium area
stayed black on the results screen; the second test showed that restoring on
scene exit instead made gameplay fall back to stock, so the replacement now
survives scene changes and only a real pool release reverts it.

The concentration replacement does **not** create a sprite. Both window
elements (the default `concentration` view and the customized
`concentration_uioff` view) are examined. The `concentration_bg_set/bg` clip is
rebound to our texture with the native dynamic-descriptor setter `0x329e50`;
when an element has no such clip, its layer root clip (`layer +16`) is rebound
instead, because the default view may keep its art on the layer itself. The
clip keeps its own transform, parent layer and draw order. The native archive
pointer (`clip +520`) and 256-byte name (`clip +0`) are saved and rebound
through `0x329f30` when available. The first in-game test used an owned sprite
with copied properties instead; it appeared far to the right because `position`
is relative to the window layer, not the screen, so that approach was
discarded. The second test showed that element 0 has no named background clip
while element 1 does, so the runtime now logs the customize index and both
element visibilities on every native show.

Remaining unknowns: which window element is visible in the user's default
concentration view (layer visibility reports 1 for both, so the log now records
the customize index and the per-bind node state), and whether the default
element's art lives on its layer root or on the `subbg0_concentration` clip that
the customize preview uses (`data/graphic/customize/subbg0_concentration.jpg` is
its 120x120 thumbnail). The AFP group and exact z-order of the premium sprite
still need confirmation. Until those checks are done, keep the 1280x720 canvas.

## Live binding and selection draw order (2026-10-23)

The background callback `0x906eb0` runs on every component update and
unconditionally calls `set_archive`/`set_origin` when the result archive exists,
or hides the `illust` clip when it does not. A custom descriptor bound only once
is therefore overwritten on the next frame. The runtime now caches the clip but
reasserts the custom descriptor, pixel origin and visibility after **every**
original call; the upload stays once per visit. See the 2026-10-23 entry in
[validation](validation.md).

The selection `movie_draw` dispatcher (`0x9362b0` → `0x936300`) draws a textured
quad from an AFP descriptor. It is called from the stage draw dispatcher
(`0x8e46e0`, PRERENDER) and the render compositor (`0x9248f0`), before the AFP
2D scene render (`0x328b40`). A replacement drawn there is covered by the
selection scene's AFP background and UI. This build family's selection movie
loader is stubbed, so the stock selection background is an AFP element; a
selection replacement needs an AFP-level rebind or an owned sprite below the UI,
not the movie quad. No such implementation has been validated in-game.

## Reproduction

```powershell
python -m pip install pefile capstone unicorn
python scripts/verify-native.py 'C:\path\to\010-0917bm2dx.dll'
python scripts/verify-surfaces.py 'C:\path\to\010-0917bm2dx.dll'
python scripts/verify-frame-chara.py 'C:\path\to\010-0917bm2dx.dll'
python scripts/verify-origin.py 'C:\path\to\010-0917bm2dx.dll'
target/release/iidxbg-tool.exe verify 'C:\path\to\010-0917bm2dx.dll'
```

The native verifier checks exact SHA-256/image size, every embedded entry guard,
the three modified vtable slots, RTTI, scene ownership, the `illust` binding
branch/string references, texture registration/destruction and AFP ordinal 137.
The surface verifier checks selection/cover/graph anchors and AFP group imports.
The frame-character verifier checks construction, layer/root stores, dimension
vtable and emulates visibility/dimension wrappers. The origin verifier emulates
the guarded pixel-origin helper with two linked clips.

## Validation status and unknowns

Offline evidence only. This executable has **not** been run in-game by this
analysis. Check normal clear/failed/full-combo, SP/DP and Premium Free results,
score/title/graph visibility, repeated visits, early exit, device reset and
shutdown, and frame-time tails. The changed `sub_18094fb50` common scene helper
is called from selection and result initialization; its exact behavioral effect
is not established. No in-game claim is made from the earlier 34 offline
validation. This describes the original offline investigation; see the live
follow-up below for subsequent tests and remaining limitations.

## Results follow-up, 2026-10-23

Input rechecked from `\\2dx.lan.sh1mar.in\IIDX34\modules\bm2dx.dll`:
18,200,216 bytes, SHA-256
`c98e183ec81ecb9e1cb13c237f9c9cb0b3f65b6fdaff6fd5e6a526008c546534`.
Preferred/observed base `0x180000000`. Addresses below are RVAs, offsets decimal.

### Delayed setup and temporary wrappers

- `0x906b80` initializes the background's +288/+296/+304 layer pointers to zero.
  `0x9178a0`, loop at `0x9179cc`, later dispatches component slot 1 at
  `0x9179ea`, passing the result archive from scene +104. The first update
  hook can precede this setup. A one-time probe there cannot describe the
  visible result screen.
- Setup `0x906da0` creates header (+288, priority 90), `bg1_clear` or
  `bg1_failed` (+296, priority 154), and difficulty/music (+304, priority 126).
  Preserve the original update `0x906eb0`; it also maintains timers and text.
- The actual `illust` lookup is `0x907014`; archive rebind `0x907037`; the
  no-archive hide is `0x90704e`. A failed SP result was observed with archive
  zero, movie `0x18bcf49f8`, root handle `0x5ba0002f`, and a **valid** `illust`
  handle `0x5c28013d`. Thus archive zero does not mean no usable illustration.
- `find_clip` `0x81ea30` delegates to `0x32d1a0`. The latter returns a
  536-byte temporary wrapper from pool RVA `0xc3b9820`, incrementing the
  counter at `0xc500a80`, capacity 2500. Initializer `0x329bb0` looks up the
  AFP node and writes the handle at wrapper +512, name at +256 and owner at
  +528. `0x328860`, `0x328d80`, and `0x328e80` reset the pool and invalidate
  handles to -18; `0x328860` is called at `0x924480` in the main update path.
  A wrapper address is not a scene-lifetime object. The runtime now obtains
  the current update's wrapper, using the native `result_bg` bind capture
  when available or a fresh `illust` lookup otherwise.
- `set_texture` `0x329e50` can return success even for an invalid handle:
  its success byte starts at 1 and the loop is skipped for a negative +512.
  Check the handle before interpreting its return value. For valid handles,
  the Microsoft x64 ABI is `(wrapper in RCX, descriptor in RDX) -> AL`.
  The function applies AFP ordinal 137 and follows linked handles via 106.
- Scene +16264 was an incorrect background guess: `0x914685` initializes
  scene +16256 via `0x5b9af0`, then `0x5b9b90`/`0x5b9320` create `point_1p`
  or `point_2p`, storing the layer at +8 (priority 101). The speculative
  +16264/+16360 suppression and priority-1000 overlay were removed.

The first live diagnostic build reached a failed result but a cached wrapper
produced a misplaced image overlapping UI. That build is not a successful
replacement test. It motivated the current-update lifetime correction.

Read-only asset inspection corroborates the names: `data/graphic/0/result.ifs`
SHA-256 `d6383a173e1aa84d6ad47b58311b11f13456f13396add403e093c7f7c2e717ff`,
51,278,960 bytes. Embedded `afp/bg1_clear` SHA-256
`0ea96326c22dacc0559392387d34191a3ca2b43d5c3ae0337155994b42e60f7d`
and `afp/bg1_failed` SHA-256
`dc1e3ac32e7009b0465e449135b56779783db71907fc31009379f813a8120505`
both have `illust`, `stage`, and timer names in their decoded string tables.
Names alone do not establish geometry or visibility. No archive was edited
or copied into the repository.

Reproduce with the exact DLL in IDA: decompile the functions above, follow
the pool counter's write xrefs, then launch with `iidxbg.dll` and record a
failed result's `results initialized` and `results ready probe` log lines
after setup. Compare screenshots over several frames, not only bind success.
See `docs/validation.md` for the final live verification status.

### Test-launch monitor shortcut

The earlier investigation at
`D:\Share\chart-requester\docs\iidx34-monitor-check-patch-analysis.md`
describes a different hash; its addresses were not copied blindly.
For the exact hash above, decompilation and instruction checks establish:

| RVA | Original | Test replacement | Effect |
|---|---|---|---|
| `0x91b0a8` | `0f8540010000` | `90e940010000` | Always take native completion at `0x91b1ee`, skipping TDJ validation/retries |
| `0x91b37b` | `8d1480c1e202` | `8d1440909090` | Reset `0x91b350` uses 3 * selected rate instead of 20 * rate |

`0x91af80` still calculates FPS with `0x92bc70(...,359)`. At 120 Hz,
the shortened interval is 360 updates. The averaging branch starts at 460,
so this shortcut uses the rolling measurement, not the longer average.
Completion still stores measured FPS at configuration +28 (`0x91b21c`)
and performs native monitor/scene bookkeeping. No timing constant is forged.
The changed LEA produces EDX; subsequent MOVD/conversion/store consumes EDX,
without consuming the original SHL's flags. This is a test-launch shortcut,
not evidence of equivalent calibration precision or non-TDJ behavior.

`scripts/spice-monitor-bypass.py` checks disk SHA-256, both complete original
instructions, and both live instructions before writing through the Spice
memory API. It reads back both patches and supports `--restore`. Disk bytes
remain unchanged; it must be applied after API startup on each new process.
Both live readbacks passed and the subsequent launch reached attract mode.


## 2026-10-23 selection background and shared media follow-up

Input remains 18,200,216-byte bm2dx.dll, SHA-256
`c98e183ec81ecb9e1cb13c237f9c9cb0b3f65b6fdaff6fd5e6a526008c546534`,
base `0x180000000`, image size `0xc5ff000`. No disk game modifications.
All following addresses are RVAs and offsets are decimal.

- `0x8d8290` selection update is `(this, u8 tick, context)` in the Microsoft x64
  ABI. It updates `this +160` with `0x5d0a30`, then forwards all three incoming
  arguments to `0x94fcf0` at `0x8d849b`. The exact profile now selects the extended
  trampoline rather than dropping RDX/R8. Other build profiles are unchanged.
- `0x5cf1f0` builds the frame. `frame_layer1` is frame +80 (scene +240),
  `frame_layer2` is frame +88 (scene +248), and `frame_layer3` is frame +96
  (scene +256). The layer2 factory call at `0x5cf3d1` uses frame +16 (scene +176)
  for priority. Character is frame +120 (scene +280), with priority frame +28.
  The frame builder now has its own 16-byte live-code guard.
- The live selection scene had scene +176 =157; a custom priority-158 AFP sprite
  appears behind the song list, radar and native character. The previous D3D
  movie-hook quad was obscured by later AFP drawing. The native movie call now
  still runs on this profile; only layer2's root is masked for replacement.
- Layer2 is validated against selection vtable `0xd52508`, layer vtable
  `0xd968b0`, nonzero layer +8, and nonnegative AFP root at layer +528.
  Property 4103 visibility is saved, restored before original scene update and
  teardown, and hidden only after valid sprite creation. Release observations
  clear owner identity before the native pool can reuse it. Custom sprite nodes
  are released before their Arc-owned texture wrapper; reset releases all strong
  GPU leases while retaining the selected CPU asset.
- Existing graph adapter `0xa1fd30` positions/clips the substituted node at graph
  +64, without changing its scale. This permits a shared cover texture to be
  scaled to 432x870 while cover sprites use 432x723. Covers still temporarily
  substitute the original art pair; native adjustment, visibility and text run.

Reproduce: verify the exact input with scripts/verify-native.py; decompile
0x8d8290 and 0x5cf1f0; configure song_select as a fixed local MP4; restart the
plugin, enter selection, and compare captures separated by several seconds.
For descriptor sharing, set sources.sudden_plus.bind and sources.target_graph.bind
both to lift, with sources.lift selecting the fixed clip. Enable the native
covers/graph during gameplay and compare `media sprite` descriptor addresses.
Unit tests also establish shared session clock/frame identity and last-lease stop.

Live reproduction succeeded on guest SP Caramel Pain ANOTHER 8 with native
LIFT&SUDDEN+ and pacemaker enabled. Both cover sprites (priority 108) and graph
(priority 145) reported descriptor `0xb666f5e8`, canvas 432x723. LIFT's diagnostic
timecode advanced from 0.100/frame 3 to 4.367/frame 131 while the graph moved.
Capture pairs and test coverage are indexed in docs/validation.md. Selection
and graph also played the user's H.264 clip; the final config restores that clip
after the diagnostic cover-pattern test.

Unknowns: no live tests on IIDX 33 or the other IIDX 34 hash, no DP/player-2 test,
no GPU loss induced under simultaneous playback, and no 1080p60 frame-time-tail
benchmark. The new selection mask is embedded only in this exact build. The
user previously confirmed static high-grade results; new high-grade video needs
an actual A/AA/AAA visit to establish live coverage separately from failed/F.


## Subscreen video and live streams (2026-10-23)

Input remains the exact SHA-256
`c98e183ec81ecb9e1cb13c237f9c9cb0b3f65b6fdaff6fd5e6a526008c546534`.
No native entry, ABI, guard or RVA changed. Premium constructor `0x6447a0`
observed a nonzero base and null custom sprite (previously unconfirmed base-only
path), priority 58. Its 1280x720 media sprite copies the same AFP transform.
`movie_draw` at `0x9362b0` and the existing results tick update owned GPU pixels.
Worker IO/decoding never runs inside these callbacks.

Concentration apply/show/layer remain `0x6375c0` / `0x6377f0` / `0x638420` with
the ABIs recorded above. On native show, customize index 0 resolved element0 to
its root clip (fallback, no name) and element1 to `concentration_bg_set/bg`.
Both reported a live AFP node, visibility 1, and no restorable archive/name.
Binding still uses `set_texture` at `0x329e50`; the original archive setter at
`0x329f30` remains the restoration path where a complete binding exists.
Each bound clip now retains an Arc texture and shared playback lease; only
ready owned frames update the dynamic texture. Hiding releases the playback
lease while keeping an unrestorable descriptor alive for the native pointer.

The first live transition exposed a pre-existing visibility hole: native
show(0) left a dynamic texture bound to the default root visible over results.
The decoder correctly stopped, so the stale frame persisted. Unrestorable
bindings now explicitly mask AFP visibility (4103, then dirty 4126), saving its
old value once. Rebinding on the next show restores the saved value only if
the clip still contains the same AFP node ID at +512. This avoids applying a
saved visibility to a recycled node. Archive/name restoration data is retained
across source changes rather than replaced by our own dynamic binding.

Reproduce on guest Standard SP: use live room 24065 for Premium and room 6 for
concentration; enter selection, PgUp to open the Spice subscreen, and compare
Spice capture screen 1 over time. During gameplay tap the top-left assistant
icon, then the CONCENTRATION MODE switch (about 100,196 on the 1280x720
subscreen). Wait for the native transition, compare frames, and let the song
retire. Confirm concentration is hidden and Premium returns on results; repeat
the show/hide cycle. Evidence and exact tested plugin hashes are in validation.md.

Unknowns: customized (nonzero-index) concentration window, alternative native
layouts, device loss during these streams, and other game hashes remain outside
this live test. Public live room content and availability can change.
