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
concentration view (the log now records it), and whether the default element's
art lives on its layer root or on an unnamed child clip. The AFP group and exact
z-order of the premium sprite still need confirmation. Until those checks are
done, keep the 1280x720 canvas.

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
validation.
