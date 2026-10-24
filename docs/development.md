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
target/release/iidxbg-tool.exe profiles
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
[33 / 010-20260819](iidx33-background-surfaces.md),
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

## Maintaining native build profiles

Edit one file under `src/profiles/iidx<major>/<edition>-<YYYYMMDD>.rs` for each
verified DLL. Use the release date and edition (`010` / 120 Hz arcade,
`012` / 60 Hz arcade); do not substitute the PE timestamp for a release date.
Use `unknown` when an edition has not been established. Currently:

- `iidx33/010-20260819.rs`: verified 010/TDJ edition, using the release identity
  associated with this DLL's PE identifier.
- `iidx34/010-20260916.rs`: hash `3755...fe83`; historical display label
  `IIDX 34 / 20260917` is retained for existing diagnostics.
- `iidx34/010-20260917.rs`: hash `c98e...6534`, including the primitive-padding
  workaround. No 012 binary has been added by this reorganization.

Each `native_entries` row puts the function name, hexadecimal RVA and 16-byte
entry guard together. The same file specifies the complete layout, vtable
hooks and surface data. These are Rust definitions, not strings containing
JSON. Rust `#[path]` declarations in `profile.rs` allow the requested hyphenated
filenames while keeping ordinary Rust module identifiers.

For a new build, copy the closest profile as a starting point, analyze the
actual DLL, record its full hash and ABI/lifetime evidence, then update all
relevant addresses/guards/layouts and register its module in `supported()`.
An unchanged date-family ABI is a hypothesis to check, not a compatibility
rule. Shared native adapters remain reusable across verified builds; a changed
calling convention or lifetime requires a corresponding adapter change.

Build `iidxbg-tool` before running the Python native verification scripts.
They obtain JSON from the tool's `profiles` command, which exports the same
Rust definitions embedded in the plugin. `scripts/native_profiles.py` selects
the newest local debug/release tool and rejects a tool older than the profile
sources. Set `IIDXBG_TOOL` to choose an explicit tool path. No game DLL is loaded
by the export command. Exported JSON is diagnostic output, not a second source
of offsets to edit or commit.

Migration verification (2026-10-24): both debug and release `profiles` exports
exactly matched every field of all three pre-migration JSON profiles, including
the pending primitive-padding workaround. Core tests passed (49 passed, 3
environment-dependent tests ignored), as did formatting, Clippy with warnings
denied, the release build and DLL smoke check. Native, surface, origin and
frame-character verification passed against each of the three exact binaries;
private-mapping attachment passed on 010-20260917. No new RVAs or ABI assumptions
were introduced. Actual in-game testing was not performed for this refactor;
the build was not deployed.

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

## Named artwork catalog

The exact IIDX34 010-20260917 adapter can replace named atlas regions and
untextured GE2D shape fills without editing game archives. Use the configuration
UI's third sidebar page (artwork catalog), click **Scan game assets**, search by
archive/region name and choose media. Entries start disabled. Native shapes,
animation, draw order and masks still belong to the game. A resource used in
several places changes everywhere it is used. The @movie/main entry targets the
main gameplay movie. Fonts and model geometry are not image resources.

Scanning is opt-in: opening the page or starting the application never scans.
The EXE detects `data/graphic` beside the configuration's `modules` directory;
otherwise a folder picker requests the installation or `graphic` directory.
An in-process Rust worker reads IFS/binary XML, maps hashed texture/AFP geometry
names, crops texture regions, draws GE2D fill previews, and indexes loose images.
No Python, ifstools installation, or game launch is needed. Progress is shown;
click the same button to cancel. The previous catalog remains until completion.

The generated catalog loads automatically and is remembered for the next launch.
Caches live under `%LOCALAPPDATA%/iidxbg/asset-cache/<installation-hash>/scanner-v2/`.
The All Assets page always displays the expanded cache root and an **Open cache
folder** button. Users can clear files there and scan again to regenerate them.
Each source is SHA-256 checked on a requested scan. Unchanged inputs reuse catalog
records/previews; duplicate texture and shape content shares thumbnail files.
Only catalog records, previews and a `scan-report.json` are saved; source archives
are not extracted or copied to the game folder. Reports include input hashes and
unreadable/unsupported members. External `_super_` references are reported and
skipped, matching the earlier inventory's `super_disable=True` behavior.
Scan results identify resources, not DLL/ABI compatibility; existing exact-build
runtime guards still apply. Fonts and 3D model geometry are not interpreted.
Known note, shell, decision, transition and character names have bilingual search
hints. Shape previews are approximate rasterizations, not native AFP animation.
See [artwork-scanner.md](artwork-scanner.md) for format evidence, corpus parity
results, input/catalog hashes, limits and reproduction commands.

**Import catalog** remains available for existing catalogs. The independent Python
tools below are also retained for analysis and reader parity checks:

```powershell
python scripts/inventory-artwork.py C:/Games/IIDX/data/graphic D:/Analysis/iidx-art
python scripts/inventory-geometry.py D:/Analysis/iidx-art
python scripts/build-artwork-catalog.py D:/Analysis/iidx-art
python scripts/inventory-loose-artwork.py C:/Games/IIDX/data/graphic D:/Analysis/iidx-art
python scripts/audit-artwork-coverage.py D:/Analysis/iidx-art
```

Import `D:/Analysis/iidx-art/catalog.json`; keep its thumbnails directory beside
it. The first command copies archives to the local analysis directory and checks
SHA-256. It refuses an output inside the source directory or on a UNC share.
No archives/previews belong in the repository or release package.

`native_size` limits decoding to the selected region's native dimensions (with
an overall 1920x1080 ceiling), avoiding a full-HD upload for a tiny sprite.
Transparent images retain their alpha. `native_mask` additionally applies the
original texture alpha; GE2D always retains its native polygon/mask behavior.
Bindings reuse an existing module's prepared asset, decoder and GPU source
texture. Independent selections use independent playback even for the same URL.
Static raster regions are composed once and then use native draws; video tiles
are recomposed only for a newly published frame. See
[iidx34-custom-art.md](iidx34-custom-art.md) for measured costs and limitations.

Loose PNG/JPEG resources appear as `@file` entries, with a texture name relative
to `data/graphic`. They use the same media editor and source providers. This
covers assistant portraits, customization previews and movie thumbnails that
are outside IFS. Native direct-handle drawing requires a matched texture bind;
static images still upload/compose only when changed. The catalog lists readable
resources, not proof that every resource appears in an accessible game mode.
Unreadable inputs are recorded in the inventory with hashes and errors.
