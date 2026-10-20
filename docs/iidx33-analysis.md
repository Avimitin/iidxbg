# IIDX 33 results background evidence

Recorded 2026-10-20 using IDA/Hex-Rays, MSVC RTTI, PE exception boundaries and
Capstone. Input `D:\Share\chart-requester\analysis\bm2dx.dll`, build label
IIDX 33 / 20260812, 18,467,992 bytes.
SHA-256 `c61b6dcb8894062e56d60da8ca90053b27f129e1a8e8da5e54457aa42602397d`.
Image base `0x180000000`, image size `0xbb95000`.
All addresses below are RVAs; object offsets are decimal.

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
`src/profiles/iidx33-20260812.json` records all called native entries and the
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

No real-game test has been performed. First test with the diagnostic image:
check crop, origin, score readability, clear/failed results, player sides and
Premium Free. Check repeated visits, cleanup, early exit, reset and shutdown.
Measure frame-time tails during decoding and first-result upload.
Specialized result scene classes are excluded by ordinary-scene vtable checks.
This evidence establishes a native path, not successful live rendering.
