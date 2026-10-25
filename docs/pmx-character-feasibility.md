# Animated PMX character feasibility

Historical feasibility notes below predate implementation. See
[MMD implementation and validation](mmd-character.md) for current behavior.

Recorded 2026-10-25. Source review and local model-package inspection only.
No native code, game assets or game DLLs changed. No actual in-game testing,
rendering benchmark or motion playback was performed. This document records a
design assessment, not an implemented or validated feature.

## Inputs and identity

Model root:
`C:\Users\BeatMania IIDX LM\Downloads\学マスぬいセットv4_Japanese`.
Models below are in its `学マスぬいセット` subdirectory. SHA-256:

| Input | Hash |
|---|---|
| `ことぬい.pmx` | `5c7cfadc1e598bee9a01e8df3e0ae5cd51abdbc87c8c9c4e341a49460f6aa85c` |
| `さきぬい.pmx` | `ab473611c5b256340d68ed38c43038196c851a2cd33fc9ad2948e5bf8b68a09d` |
| `てまぬい.pmx` | `b363072898676272350e062b76e9766241e65b66e393388fd2d2337d4fe8d224` |
| `佑芽ぬい.pmx` | `50bca2ff9481d3d626baf89cfb1a779d3dc95c9c9065be4483257d189b45f441` |
| `千奈ぬい.pmx` | `f8c6c13cfa7403668b0b3d5da99d23a4b29aa8948d6884f85be453e6e73ed9f5` |
| `広ぬい.pmx` | `84aa9fd3a157bbcb93edbd96ff24c259fdcf58fa7e9d743f41daecd6f7568317` |
| `美鈴ぬい.pmx` | `46bf9e950e27edfc1e528ede2e13ace5a2f399a3cfa33c7808b702975275a801` |
| `viewer-source/model-report.json` | `940fcf74bf5ba372fe1b3649d925cfa793092f867f079264e5621cb5fcb6f6e7` |
| `学マスぬいセット/学マスぬいセット_りどみ.txt` | `358e8786ff41226af203dfbb168dfc3c9e10bb562ad935d690fbaac41c0a4ee1` |

Reviewed implementation SHA-256:

| Source | Hash |
|---|---|
| `src/character.rs` | `f6f53f2f482ca9dfc2cae3a4abcec779d46870171404b865d7dd1e03534fe23d` |
| `src/media.rs` | `31c53493e5fd59f8fbd9356cbc48bbe9f41d0de39058398e415b3e3ff98b0bd9` |
| `src/runtime/surfaces.rs` | `e272829345149c31db0c00895ade83190db65a02694afd07197b22335b7fb772` |
| `src/runtime/surfaces/artwork.rs` | `3272e75e03ef6476cfeb8332a7ce6efa825accdb62f7b54ee02dfc8cba7e5422` |

The existing model report lists:

| Model | Vertices | Triangles | Bones | Morphs | Rigid bodies / joints |
|---|---:|---:|---:|---:|---:|
| ことぬい | 7206 | 11133 | 72 | 63 | 0 / 0 |
| さきぬい | 6595 | 10057 | 72 | 58 | 0 / 0 |
| てまぬい | 5649 | 8531 | 84 | 54 | 7 / 6 |
| 佑芽ぬい | 6464 | 8975 | 73 | 48 | 2 / 1 |
| 千奈ぬい | 6022 | 8737 | 73 | 53 | 3 / 2 |
| 広ぬい | 5497 | 8255 | 76 | 59 | 5 / 4 |
| 美鈴ぬい | 6954 | 9619 | 71 | 53 | 0 / 0 |

These counts come from the supplied report, not a fresh parser run. Its companion
`check-models.mjs` uses Three.js's PMX parser. An attempted read-only reparse
could not start because `node` was unavailable on PATH. Report-to-file freshness
therefore remains unverified. Morph types include group, vertex, bone and UV;
some models also use material morphs. The source viewer pins Three.js 0.169.0,
loads PMX/TGA and provides expression controls; it does not establish dancing
inside IIDX. No VMD files were found in the supplied package outside dependencies.

The CP932 readme says most dance motions can be applied, recommends a 15-degree
camera field of view, and describes an optional eye/eyebrow effect. It also asks
users to refrain from use outside MMD, explicitly mentioning other 3D software,
games and VR. That author restriction matters for this particular model pack;
generic PMX support can be evaluated with assets permitting the intended use.

## Existing native boundary and lifetime evidence

No new RVAs, ABI or binary compatibility claims were discovered. Existing
exact-build evidence is reused as design context, not revalidated here:

- [IIDX 33 character evidence](iidx33-frame-chara.md), game SHA-256
  `c61b6dcb8894062e56d60da8ca90053b27f129e1a8e8da5e54457aa42602397d`:
  ordinary selection update RVA `0x8ec1f0`, character layer at scene +272.
- [IIDX 34 character evidence](iidx34-frame-chara.md), game SHA-256
  `3755587f44785effe5008f74cfe7dccbdf423ad09feead48e00a90f78a42fe83`:
  update RVA `0x8d8250`, character layer at scene +280.
- [Later IIDX 34 evidence](iidx34-010-0917-analysis.md), game SHA-256
  `c98e183ec81ecb9e1cb13c237f9c9cb0b3f65b6fdaff6fd5e6a526008c546534`:
  update RVA `0x8d8290`, movie dispatcher `0x9362b0`, layer at scene +280.

Existing documents record distinct Win64 update signatures: 33 receives
`(scene*, u8 tick, context*) -> u64`; inspected 34 receives `(scene*) -> u64`.
The older character adapter checks the current scene/layer identity and root ID,
restores native visibility before original updates, and uses a retained sprite.
Cleanup, release observation and device invalidation retire graphics resources.
Current code bypasses that replacement path for profiles with `artwork_dispatch`;
the artwork path substitutes character imagery within native animated geometry.

The dedicated character input is currently an image: `LocalImage::refresh`
decodes owned pixels, and artwork `acquire` wraps them in `Asset::Image`.
`Asset` currently has only image and video variants. PMX is therefore a new
source/runtime feature, not an extension accepted by existing media loading.

Artwork `compose` already creates D3D9 render-target textures and preserves
state, render targets including MRT1, depth surface and viewport. This is useful
integration infrastructure; it is not an existing 3D model renderer. MMD would
also require mesh buffers, skinning, appropriate materials and a depth buffer.

## Proposed scope

A feasible implementation would load a PMX model, textures and a separate VMD
motion, evaluate animation, render the model into a transparent texture, and
composite that texture through the character adapter. The model remains live
3D: camera, pose and animation can change at runtime. IIDX's UI consumes its
rendered appearance as a texture; this does not require prerecorded video or
native game support for PMX.

Keep file reads, parsing and texture decoding on workers, publishing owned
texture pixels and immutable model/pose data. All game-device objects and draws
stay on the graphics owner. Preserve original calls, exact-build guards, stock
fallback and reset/re-entry behavior. Bound animation work and render cadence;
reuse buffers/targets and avoid GPU readback in the native rendering design.
This would require a deliberate extension of the existing pixel-only media
interface. Model/motion selection can remain fixed while its pose animates.

A useful first version is one model, one looping VMD, fixed camera, transparent
background and a configurable 30/60 fps character update rate. Bone mapping,
skinning, animation interpolation and any IK required by that motion are core
work. Generic MMD support adds morph variants, inheritance/deformation ordering,
rigid-body physics, transparency/outline details and effect compatibility.
Full MME shader compatibility should be scoped separately. BPM matching or
reacting to the highlighted song requires independently verified song/timing
data; ordinary selection exit does not prove a song-confirmation event.

[Saba](https://github.com/benikabocha/saba) is a candidate to evaluate for PMX/VMD
loading and animation. Its documented examples use OpenGL, D3D11 and Vulkan;
it is not a drop-in D3D9 renderer. Its use would require dependency/ABI review
and a graphics adapter. Microsoft documents D3D9 texture rendering through
[SetRenderTarget](https://learn.microsoft.com/en-us/windows/win32/api/d3d9/nf-d3d9-idirect3ddevice9-setrendertarget).

Engineering assessment: a limited looping character is a moderate feature;
robust general MMD compatibility is a substantial subsystem. Mesh counts look
modest, but do not establish acceptable IIDX frame times. No schedule or
performance guarantee follows from this inspection.

## Reproduction and unknowns

1. Enumerate the package with `rg --files` excluding `**/node_modules/**` and
   the bundled HTML. Check for PMX, textures, VMD and viewer sources.
2. Hash the listed inputs with PowerShell `Get-FileHash -Algorithm SHA256`.
3. Read `model-report.json` using UTF-8 `ConvertFrom-Json`; inspect each model's
   metadata, morph types and material records. Inspect `check-models.mjs` to
   establish how the existing report was generated.
4. Decode the author readme using
   `[Text.Encoding]::GetEncoding(932).GetString([IO.File]::ReadAllBytes(path))`.
5. Review the source paths and linked native evidence above. A future fresh
   model parse should use the installed Three.js PMX parser in a read-only
   script and compare results with the recorded report.

Remaining unknowns: actual motion compatibility and bone behavior, material/
effect fidelity, alpha edges, framing during native transitions, target hardware
frame-time tails, graphics reset/re-entry stability, and intended-use permission
for this model pack. No tests, clippy or release build were run because the only
repository change is this assessment document; implementation would require all
three plus explicit in-game validation.

## Resource comparison with video (2026-10-25)

Additional inspected source hashes: `src/video/mf.rs` SHA-256
`ed4543b825031e7187a8b7c169f5bc28d92c30476ee2da0169875e27402a0c9c`;
`src/runtime.rs` SHA-256
`abb3658c7d81d62ff3b588c30c7b56fa0e5419ea45d9cf75bfd963f4478b8a2d`.
No new native RVAs or ABI assumptions. The ownership/lifetime boundary above
still applies. Reproduce by inspecting `Reader::open`, `Reader::read`,
`copy_scanlines` and `Texture::refresh` at those hashes.

The current video path requests RGB32, copies decoded samples into owned BGRA
frames and uploads each newly presented frame into a reusable D3D9 texture.
It explicitly makes alpha opaque. It enables advanced video processing, but
does not supply `MF_SOURCE_READER_D3D_MANAGER`. Advanced processing alone does
not prove hardware decoding. Microsoft's documentation distinguishes
[advanced processing](https://learn.microsoft.com/en-us/windows/win32/medfound/mf-source-reader-enable-advanced-video-processing)
from the [device manager enabling DXVA](https://learn.microsoft.com/en-us/windows/win32/medfound/mf-source-reader-d3d-manager).
The historical video research's hardware-decoding characterization is not
independently established by this source review; no decoder telemetry was taken.

At 1920x1080, four bytes per pixel and 60 unique presented frames per second,
the texture upload payload alone is 497,664,000 bytes/second (about 498 MB/s),
before other copies. This is arithmetic, not measured PCIe traffic, elapsed
upload time or encoded-video bitrate. Lower-resolution/rate videos reduce it.

A native PMX renderer could keep textures and meshes on the GPU, update pose
data (or deformed vertices for CPU skinning), and render directly into a reusable
transparent GPU target. This avoids full-frame CPU pixel transfer but adds
animation/IK/physics work and GPU mesh/material drawing. Whether it is cheaper
than video depends on hardware, resolution, cadence, materials and physics.
A browser/helper implementation that reads pixels back and uploads them into
IIDX would lose much of that transfer advantage.

For the reported modest model geometry, one character with simple materials,
limited physics and a character-sized target is a reasonable low-overhead
design candidate. This remains an estimate. Compare equal visible size/quality
and animation rate, measuring CPU/GPU time and game-frame spikes, particularly
against the 8.33 ms budget at 120 Hz. Rendering at 30/60 fps can reuse the target
on intervening game frames. Stop animation work outside selection. No new
benchmark or actual in-game test was run for this comparison.
