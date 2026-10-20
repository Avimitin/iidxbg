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
target/release/iidxbg-tool.exe test-image images/test-card.png
target/release/iidxbg-tool.exe prepare iidxbg.toml preview.png
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/package.ps1
```

`prepare` exercises the same provider, body limits, decoder, resize and download
save path used by the plugin. It writes a preview without calling game functions.
Run with a separate configuration to test Konachan without changing installation
settings. `test-image` generates original diagnostic artwork; no game or provider
image is redistributed in the package.

Unit tests cover commented TOML, literal Windows/Unicode paths, strict settings
and types, duplicate keys, relative path resolution, malformed/wrong DLL rejection, image
limits/channel conversion/crop, URL boundaries, saved-file preservation, and
visit freezing/fallback. These cannot validate native callbacks or GPU reset
inside IIDX. See the [architecture](architecture.md) and native evidence for
[IIDX 33](iidx33-analysis.md) / [IIDX 34](iidx34-analysis.md).

`scripts/package.ps1` copies the built DLL, repository-default commented TOML, licenses,
and diagnostic image into `dist/iidxbg`, then produces a ZIP. It preserves an
existing dist configuration. Delete or move dist yourself if you want a pristine
package after locally editing that configuration.

