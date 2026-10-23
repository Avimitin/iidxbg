# IIDX 33 results background evidence

Recorded 2026-10-20 using IDA/Hex-Rays, MSVC RTTI, PE exception boundaries and
Capstone. Input `D:\Share\chart-requester\analysis\bm2dx.dll`, build label
IIDX 33 / 010-20260819, 18,467,992 bytes.
SHA-256 `c61b6dcb8894062e56d60da8ca90053b27f129e1a8e8da5e54457aa42602397d`.
Image base `0x180000000`, image size `0xbb95000`.
All addresses below are RVAs; object offsets are decimal.

## Edition identification (2026-10-24)

The input above is the **010 / TDJ / 120 Hz** edition. Its profile is now
`src/profiles/iidx33/010-20260819.rs`. This identifies the edition of the
already-supported hash; it introduces no new offsets, ABI or lifetime assumptions.

Read-only PE inspection found:

- COFF timestamp `0x6A713632`, decoded as **2026-08-04 00:45:38 UTC**;
  entry-point RVA `0xB1CCCC`. Together these form patch identity
  `LDJ-6a713632_b1cccc`. A compilation timestamp is not a release date.
- The PE debug directory's CodeView/RSDS record is at RVA `0xECDEAC`
  (file offset `0xECCAAC`, size 97). Its embedded PDB path starts at RVA
  `0xECDEC4` and reads
  `C:\work\33\ReleaseTDJ\build\ReleaseTDJ\bm2dx_x64ReleaseTDJ\out\bm2dx.pdb`.
- The DLL has a manifest resource (type 24), but no VERSIONINFO resource
  (type 16). No literal `20260812` string was found. The ordinary PE headers
  do not directly contain an edition/release-date tuple.
- The local sp2xpatcher reference at commit
  `d407a55bc36d43b66aaf46aff8d93fad800ff1bb`, file
  `patches/LDJ-6a713632_b1cccc.json`, identifies the edition as **010** and
  describes TDJ as 010/120 Hz versus LDJ as 012/60 Hz. Its mode-selection
  original byte `03` at file offset `0xA59227` matches this DLL. This is the
  immediate at RVA `0xA59E27` in instruction `mov dword ptr [r14], 3`
  at `0xA59E24`, inside function `0xA59D90..0xA5A01E`. This corroborates
  the embedded TDJ build name; the instruction was inspected, not patched.

**Corrected release identity:** the canonical profile and display label now use
**010-20260819**, matching the database entry for this DLL's PE identity. This
supersedes the project's former `20260812` label at the user's request. The
release date is obtained from that identity mapping, not a literal release-date
field in the PE header. Selection still requires the full SHA-256 above, not
the date label or the PE identifier. The external chart-requester source path
quoted elsewhere retains its historical `b20260812` directory name.

To reproduce, parse the exact input with `pefile.PE`, inspect
`FILE_HEADER.TimeDateStamp`, `OPTIONAL_HEADER.AddressOfEntryPoint`, resource
type IDs and each `DIRECTORY_ENTRY_DEBUG` entry whose `Type == 2`; decode its
RSDS payload after the 24-byte signature/GUID/age prefix. Compare the mode
byte at the recorded file offset and disassemble from the recorded function
start with Capstone. The metadata file SHA-256 at inspection was
`ec581d667f8d04d2a74e24ee4b122122a606e6d127f97d5b884ddaee663c543b`.
No game execution, game DLL modification or in-game testing was performed.

## Results hook evidence

| RVA | Evidence |
|---|---|
| `0xda4800` | `CStageResultScene` RTTI/vtable: slot 7 deleting destructor, 13 init, 14 cleanup, 15 update, 24 component dispatch |
| `0x929a60` | Scene constructor calls background constructor `0x91bd90` with scene +256 at callsite `0x929add` |
| `0xda2188` | `StageResultDrawBg` RTTI/vtable: slot 1 setup `0x91bfb0`, slot 2 update `0x91c120` |
| `0x92de20` | Dispatches slot 2 of 14 components from scene +16096; timing argument in EDX |
| `0x91c120` | Background callback uses archive +272, movie +296; finds `illust` and binds `result_bg` or hides it |
| `0x8322c0` | Null-checked movie clip lookup |
| `0x340520` | Archive/name setter uses AFP archive lookup and `afp_core_137` |
| `0x340440` | Dynamic descriptor setter uses the same `afp_core_137`; status in AL |
| `0x3407b0` | Visibility helper sets AFP property 4103 and dirty property 4126 |
| `0x340a50` | Origin helper; `(clip, 3, 3)` sets center using archive dimensions |
| `0x92d970` | Ordinary scene cleanup; retire custom texture before forwarding |
| `0x92add0` | Deleting destructor calls `0x92a280`, then frees if flags bit 0 is set |
| `0xdcba00` | `AfpTextureEx` RTTI/vtable |
| `0xa61ca0` | Wrap existing D3D texture; AddRef, register, construct descriptor |
| `0x355650` / `0x355990` | Texture registration / descriptor construction |
| `0xa62140` | Leaf returns wrapper +24 |
| `0xa61e30` | Non-deleting wrapper destructor unregisters via `0x355720`, releases COM texture and clears descriptor |
| `0x93bc30` | D3D device getter returns root global `0xabaf550`, root +224 |

The `illust` lookup is at `0x91c284`; archive binding at `0x91c2a7`;
origin at `0x91c2b7`; no-archive hiding at `0x91c2be`. The original callback
also maintains title, artist, difficulty, stage and timer. It must still run.

## Shared ABI, distinct layouts

The inspected Microsoft x64 signatures match IIDX 34: background receives
`(this, u32 timing)`, cleanup receives `(scene)`, deleting destructor receives
`(scene, u8 flags)`. Original return values are forwarded.

The wrapper constructor initializes 64 bytes: vtable +0, owned texture +8,
u32 registration handle +16, 40-byte descriptor +24. Its destructor unregisters
the handle before releasing the texture. The manager fallback texture is at
manager +99536 in 33, versus +99520 in 34; the plugin uses each build's native
constructor/destructor so it does not access that manager field itself.

Scene background offset and cleanup slot differ from 34 (+256/14 versus
+264/17). Reusing 34's addresses or layout would be invalid. The independent
`src/profiles/iidx33/010-20260819.rs` records all called native entries and the
three modified vtable slots. File identity and live guards are required.

The shared lifecycle is documented in [IIDX 34](iidx34-analysis.md): original
background callback first, bind owned descriptor, freeze image for the visit,
restore stock before cleanup/destruction, release GPU resources on reset and
recreate from the same frozen CPU image. Network and decoding stay on the worker.

## Reproduction and limits

```powershell
python -m pip install pefile capstone
python scripts/verify-native.py 'C:\path\to\IIDX33\bm2dx.dll'
target/release/iidxbg-tool.exe verify 'C:\path\to\IIDX33\bm2dx.dll'
```

The verifier checks exact identity, every runtime guard and vtable hook target,
RTTI, scene constructor offset, native background binding calls/strings,
shared AFP binding import and wrapper registration/destruction/descriptor.
To extend support, repeat decompilation and lifetime analysis; similar bytes
alone do not prove an ABI.

The user has confirmed image display and corrected placement on IIDX 33; see the placement
investigation below. Full validation still requires the diagnostic image:
check crop, origin, score readability, clear/failed results, player sides and
Premium Free. Check repeated visits, cleanup, early exit, reset and shutdown.
Measure frame-time tails during decoding and first-result upload.
Specialized result scene classes are excluded by ordinary-scene vtable checks.
Static evidence establishes the native path; the user's report confirms basic
live rendering on this installation.

Update 2026-10-21: the supplied installation reached plugin initialization but
failed a live entry guard. Disk identity matched. See the separate
[hook compatibility record](iidx33-hook-compatibility.md), including the
chart-requester precedent and remaining live validation.

## Custom image origin — 2026-10-21

Same IIDX 33 input/hash as above. The updated installation log confirms a
preserved `result_cleanup` chain through `2dx-gsm.dll`, successful attachment,
and a 1920x1080 upload (9584 microseconds). The user reports that only the image's
top-left quarter appears in the screen's bottom-right quarter.

The old custom path called `set_texture` and `set_visible` without establishing
the uploaded texture's origin. The native result callback calls the archive
center helper only when an archive exists; the no-archive branch hides the clip
without centering it. Thus preserving an assumed native center is insufficient.
The visual report is consistent with a zero origin at a screen-centered clip,
but no live property readback was captured.

Additional verified function: **RVA `0x3409d0`**, Win64
`void(clip: RCX, x: XMM1/f32, y: XMM2/f32)`. It reads the AFP clip ID from
object +512, writes property **4123** using the supplied pair of floats, marks
property **4126** dirty with 1, and traverses linked clips using `afp_core_106(id, 6)`.
Unlike `0x340a50(clip, 3, 3)`, it does not query dimensions from the original
archive/name. Entry guard: `40534883ec408b99000200000f297424`.

The custom path now binds the descriptor, sets this pixel origin to
`(uploaded_width / 2, uploaded_height / 2)`, then shows the clip. For 1920x1080
the origin is `(960, 540)`. It keeps native screen position, scale and animation.
The operation repeats after each original callback because the original can
rebind the stock texture. Stock restoration retains the archive-center helper.
No manual X/Y settings are needed.

The installed `afp-core.dll` was also inspected read-only:
986,776 bytes, SHA-256
`8fdf2f8a2d420790e44d0a89da5a7f7949c430b72fb117166afbcc8a642dc192`,
preferred image base `0x180000000`. Relevant AFP RVAs:

| RVA | Evidence |
|---|---|
| `0x1ea70` | Export ordinal 137; texture binding delegates to `0x1e850` / `0x1e710` |
| `0x1bf60` | Converts descriptor dimensions and UV bounds to the internal texture description |
| `0x74e90` / `0x74ad0` | Non-attach texture geometry; new bounds begin at zero and extend to texture extent |
| `0x22d60` | Export ordinal 116, property setter dispatch |
| `0x22b20` / `0x21910` | Property 4123 writes float pair at work +192/+196 and marks transform state dirty |

These AFP internals explain the native helper; the plugin does not call internal
AFP RVAs or access those fields directly. Inspection did not establish that
every texture rebind resets the origin; the fix does not depend on that claim.

Reproduce with `python scripts/verify-origin.py path/to/bm2dx.dll`
(`pefile` and `unicorn` required). It emulates the actual helper with mocked AFP
imports for two linked clips, verifying property IDs, dirty marking and Win64
float arguments at centers `(960,540)`, `(640,360)` and `(960.5,540.5)`.
All three cases passed.

Follow-up 2026-10-21: the user confirmed that the corrected build works in-game.
This confirms image display and placement on the reported IIDX 33 installation.
The tested DLL SHA-256 is
`3e8ac69538abd559320ae1e6ef93922ad1c0cc718a22a5ea9a6575a53a82ac76`.
It does not establish all result modes, device-reset behavior or frame-time tails.
