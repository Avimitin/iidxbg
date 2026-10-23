# Development and validation

Windows x64, Rust stable and Visual Studio C++ Build Tools with a Windows SDK.
This repository has no build-time dependency on bilimani/chart-requester.

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/build.ps1
powershell -NoProfile -ExecutionPolicy Bypass -Command "& ./scripts/build.ps1 -CargoArgs @('test','--locked')"
powershell -NoProfile -ExecutionPolicy Bypass -Command "& ./scripts/build.ps1 -CargoArgs @('clippy','--all-targets','--locked','--','-D','warnings')"
cargo fmt --all --check
```

`-SdkRoot` optionally points to a standalone Microsoft SDK NuGet layout;
normally the SDK installed with Visual Studio is used.

Outputs:

- `target/release/iidxbg.dll`: game plugin.
- `target/release/iidxbg-tool.exe`: offline verification/image preparation tool;
  it does not load the game DLL.

```powershell
target/release/iidxbg-tool.exe verify 'C:\path\bm2dx.dll'
target/release/iidxbg-tool.exe test-image iidxbg/test-card.png
target/release/iidxbg-tool.exe prepare iidxbg.toml preview.png
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/package.ps1
```

`prepare` exercises the same provider, body limits, decoder, resize and download
save path used by the plugin. It writes a preview without calling game functions.
Run with a separate configuration to test Konachan or Yande.re without changing installation
settings. `test-image` generates original diagnostic artwork; no game or provider
image is redistributed in the package.

Unit tests cover commented TOML, literal Windows/Unicode paths, strict settings
and types, duplicate keys, relative path resolution, malformed/wrong DLL rejection, image
limits/channel conversion/crop, URL boundaries, saved-file preservation, and
visit freezing/fallback. These cannot validate native callbacks or GPU reset
inside IIDX. See the [architecture](architecture.md) and native evidence for
[IIDX 33](iidx33-analysis.md), [IIDX 34 / 20260917](iidx34-analysis.md) and
[IIDX 34 / 010-0917](iidx34-010-0917-analysis.md).

With a supported game DLL available locally, run `scripts/smoke-attach.py` in
`native`, `chain` and `bad-tail` modes as documented in
[hook compatibility](iidx33-hook-compatibility.md). This maps the DLL privately
without executing game code and verifies vtable installation/rejection.
CI does not include game binaries; these checks run locally.

`python scripts/verify-origin.py path/to/bm2dx.dll` additionally emulates the
guarded pixel-origin helper with mocked AFP imports (`pefile`/`unicorn` required).
It verifies float argument passing, linked clip traversal and property writes.

The additional [background-surface investigation](background-surfaces.md)
covers song selection, SUDDEN+, LIFT, the gameplay target graph, the premium
area and concentrate mode. `python scripts/verify-surfaces.py path/to/bm2dx.dll`
reproduces its static anchors using `pefile` and `capstone`. These checks now cover the optional runtime surface hooks. Build-specific evidence is recorded for
[33 / 20260812](iidx33-background-surfaces.md),
[34 / 20260917](iidx34-background-surfaces.md) and
[34 / 010-0917](iidx34-010-0917-analysis.md). The premium-area and concentrate
hooks exist only in the last profile.

The [video playback investigation](video-surfaces.md) measures the D3D9 texture
refresh cost and the Media Foundation decode options for playing custom video in
the existing surface positions. `tools/video-bench` reproduces those numbers
(D3D9Ex upload benchmark and a Source Reader decode benchmark); it is a
development tool and is not part of the plugin or the release package.

`scripts/package.ps1` recreates `dist/iidxbg` with the built DLL, configuration
GUI and repository TOML, then produces a ZIP containing those three files.
Use the repository-default TOML when packaging; do not ship personal media paths
or an installation's edited configuration. No video or FFmpeg binary is bundled.

## GitHub builds

`.github/workflows/release.yml` follows bilimani's release workflow: any pushed
tag builds on `windows-2022`, checks formatting, tests and Clippy, builds the
release DLL, runs the Windows load/SDK smoke test, generates license notices and
uploads the ZIP as `iidxbg-windows-x64` (retained for 30 days). A following job
publishes that ZIP on the tag's GitHub Release. Manual runs build the artifact
without publishing a release.

ZIP names use the package version from `Cargo.toml`. Update the package version
and lockfile before tagging a new version. CI uses the repository's default TOML
and generated diagnostic image; no game binaries, local downloads or tokens
are required. Build permissions are read-only; only the release job can write
repository contents. Actions use the same pinned commits as bilimani.


`python scripts/verify-frame-chara.py path/to/bm2dx.dll` checks exact identity,
scene/frame/root offsets, character construction, AFP property imports and the
guarded dimension helper. Unicorn runs the native visibility getter/setter and
dimension wrapper with mocked AFP imports. Requires pefile, capstone and unicorn;
this does not execute the game or validate rendering. Rust tests additionally
exercise owner retirement/restoration, geometry arguments, transparent image
reload and scene-request config acceptance without HTTP/game calls.
