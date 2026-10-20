# IIDX 33 / 20260812: selection frame character

Analysis date: 2026-10-21. Offline disassembly/decompilation, emulation and
mock-native tests; **no in-game validation of these controls**.

## Input identity

- `bm2dx.dll`: 18,467,992 bytes, SHA-256 `c61b6dcb8894062e56d60da8ca90053b27f129e1a8e8da5e54457aa42602397d`.
- PE image base `0x180000000`, SizeOfImage `0xbb95000`.
- Addresses below are RVAs. Exact profile matching is required; this is not
  support inferred from the IIDX version number.

## Construction and ownership evidence

| Anchor | RVA / layout |
|---|---|
| Ordinary selection update | `0x8ec1f0` |
| Selection initialization helper / frame-build call | `0x8eb050` / `0x8eb74d` |
| Frame builder / updater | `0x5dd110` / `0x5de940` |
| Embedded frame | scene +152 |
| Character-name selector | `0x5de1b0` |
| `frame_chara_01` / `frame_chara_usr` strings | `0xca15a8` / `0xca1158` |
| Character layer pointer store | `0x5dd40e`, frame +120 = scene +272 |
| Character priority | frame +28 = scene +180 |
| CLayer factory / vtable | `0x344080` / `0xdc86e0` |
| Root clip initialization / ID store | `0x340030` / `0x3400f5` |
| Native visibility getter / setter | `0x341330` / `0x3407b0` |
| Layer dimension helper | `0x343950`, vtable +312 |
| AFP property get / set IAT entries | `0xc91bb0` / `0xc91bc8` |

The frame builder obtains archive resource 34, chooses one of
`frame_chara_01/02/03`, creates a separate CLayer and stores it at frame +120.
It looks up `frame_chara_usr` in the frame layout, sets a centered origin through
virtual +88 with `(3,3)`, and positions the character through virtual +72.
The original frame update is always preserved by the plugin.

CLayer has a live handle at +8. The factory initializes its root clip at
layer +16 with the path `/`; clip initialization stores the resulting root ID
at clip +512, hence layer +528. Runtime checks exact CLayer vtable and live
handle before reading this ID. The existing common-release hook forgets
retired owners before pool reuse. Selection cleanup/destruction and device
invalidation restore the current root mask and release the custom GPU sprite.
Restoration compares the current scene, layer and root ID; it never follows a
saved scene pointer. See the existing background-surface record for shared
sprite factory, release observer and selection teardown ABIs.

## ABI and rendering evidence

Microsoft x64 calling convention:

- AFP ordinal 115: `(u32 clip, u32 property, void* output) -> i32`, negative
  indicates failure. Property 4103 writes a visibility byte.
- AFP ordinal 116: `(u32 clip, u32 property, uintptr_t value) -> i32`. Native
  visibility writes scalar 4103, then 4126 = 1 to mark dirty. Runtime writes
  only the root, without the native helper's linked-clip traversal.
- Property 4104 is position (two floats), 4123 origin (two floats), 4099 scale
  (two floats), and 4106 color (four floats in RGBA order).
- Layer dimensions: `(layer*, i32* width, i32* height)`. Calls AFP ordinal 71
  with the layer handle and a temporary buffer; sign-extends the shorts at
  buffer +16/+18 into the outputs. Its entry has a profile byte guard.
- Existing sprite position/origin setters take integer pixel coordinates;
  scale takes two floats. Color setter takes `(sprite, alpha, red, green, blue)`,
  with the fifth argument passed on the Win64 stack.

Before the original selection update, runtime restores saved native root
visibility and hides the custom sprite. After the update, it captures fresh
native state and optionally masks the root. Native character logic continues.
A static custom sprite uses the native priority and root position/origin/scale/
RGBA. Its transparent 1920x1080 canvas is scaled to the dimensions returned by
the native layer. This does not reproduce internal child animation.
Failed property reads, invalid dimensions or failed upload leave stock art.
No additional inline hooks are needed.

## Reproducible checks

```powershell
python scripts/verify-frame-chara.py 'path/to/bm2dx.dll'
```

Requires pefile, capstone and unicorn. The script selects the exact SHA-256
profile; checks frame offsets, constructor/update calls, layer pointer/root-ID
stores, AFP imports, dimension vtable target and entry bytes; then emulates the
native visibility wrappers and dimension helper with mocked AFP imports.
It covers visibility 0/1, getter failure and 1920x1080 dimension outputs.
Both supported builds passed. Rust tests additionally cover mask restoration,
retired/changed owners, missing layers, geometry/color arguments, transparent
local image loading/reloading/clearing and scene-request config reloading with invalid
edit retention. Private-process attachment does not execute the game.

## Remaining unknowns / real-game checklist

Verify hiding/re-enabling during a selection visit; native/custom/path-clear
transitions; image edits while downloading; selection exit/re-entry; device
reset; repeated scene/pool reuse; 1P/2P/DP and alternate layouts. Verify draw
order, origin/crop, transparency, fades and coexistence with other plugins.
Root properties do not include separate AFP layer-level visibility flags or
internal child animation/rotation; those transitions need visual checking.
The option is restricted to ordinary selection and does not force creation of
an absent native character. Unsupported builds remain refused.

## Read-only archive inspection

`data/graphic/1/basic_frame_chara.ifs`, 4,285,376 bytes, SHA-256
`dfac13df2dc4afdaf83dc50e44f39b0077ec1f0540fc49184cbcc5006c8c1dd0`.
Extracted locally using `ifstools.IFS(path).extract(progress=False,
path=output, extract_manifest=True)`; no extracted game assets are shipped.
The `frame_chara_01/02/03` AFP headers contain big-endian width 1920 and height
1080 at +0x10/+0x14. Texture manifests describe 700x800 artwork interiors
(half-pixel UV rectangle `2 1402 2 1602`), with padded PNGs 702x802.
The movie canvas is therefore larger than its artwork; custom placement uses
the full movie canvas instead of treating the artwork rectangle as the layer.
The original child-art transform has not been reproduced.

Additional local AFP inspection used `afp-core.dll`, 986,776 bytes, SHA-256
`8fdf2f8a2d420790e44d0a89da5a7f7949c430b72fb117166afbcc8a642dc192`.
Ordinal 56 (RVA 0xc5b0, helper 0xa770) masks/sets layer flags; ordinal 57
sets global flags and is not a visibility getter. Runtime does not call these
internal addresses; the distinction is why separate layer flags remain a
visual-validation caveat instead of being guessed from ordinal 57.


## Event-driven revision (2026-10-21)

Removed the 500 ms config/image watcher. Existing selection/result entry/exit
and gameplay-owner retirement hooks now wake the single preparation worker.
Selection entry is the first observed update of a new tracked scene; result
entry is the first ordinary result-background update of a new visit. Neither
is called once per frame for request purposes. Selection exit is not evidence
of a distinct song-confirmation callback. This wiring adds no native addresses.
The input image can have arbitrary valid dimensions. 1920x1080 is the internal
movie canvas convention, not the stock child-art size. Child-art placement
remains an explicit limitation of the whole-layer replacement.
