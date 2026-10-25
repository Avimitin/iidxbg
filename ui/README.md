# iidxbg configuration UI

A Windows desktop editor for `iidxbg.toml`, built with **Rust**, **GPUI**, and
[**GPUI Kit**](https://github.com/longbridge/gpui-kit). It is a separate optional
binary; the plugin still ships `iidxbg.dll` and `iidxbg.toml` on its own.

## What it does

- Icon sidebar for display components, Bilibili login and all artwork.
- Scrollable adjacent mosaic with sixteen independent components, including
  title, login, mode selection, gameplay bottom frame and logout summary.
- All artwork has an on-demand installation scan, a visible cache path and an
  Open Cache Directory button. No scan runs merely from opening the page.
- Every surface, including results, has an independent enable switch.
- Choose a fixed image/video, a random folder, a compatible Moebooru API, or a
  local/API mix. Native file/folder pickers write paths into the selected surface.
- Compatible surfaces can bind to another surface. SUDDEN+, LIFT and target graph
  share one selected file, decoder session and GPU descriptor when bound.
  Compatible wide backgrounds form another group; premium/concentration can bind.
  Cycles are rejected before saving. Unbinding restores independent settings.
- Built-in Konachan/Yande.re endpoints and custom post.json URLs are supported.
  Custom APIs may use explicitly listed CDN hostnames. API downloads remain images.
- Module settings use a persistent category sidebar, settings search and a fixed
  save/error footer. On narrow windows, categories wrap above the form. Ctrl+F
  focuses search; Escape clears search first, then closes the editor.
- Song selection has separate pages for media, display/sharing, the large
  character, Qpro, MMD model/dance, reactions and advanced options. Ordinary
  modules show only media, display/sharing and advanced pages. Provider controls
  follow the selected source; BPM and reaction details appear when enabled.
- Large-character source is independent of Qpro: stock or a custom image can
  remain on the left while Qpro uses PMX. Both can also use MMD. MMD consumers
  share the imported model/motions, with separate placement sizes. Source changes
  preserve saved image, PMX and VMD paths. Old configurations retain their prior
  selection until a source is explicitly chosen.
- Valid edits save atomically while preserving TOML comments. Invalid edits retain
  the last valid on-disk file and display an error. Reload picks up external edits.
- Previews run off the UI thread and share one prepared preview for bound surfaces.
  A video preview shows its first frame. Video playback is silent, loops, and
  stretches to the native canvas; the fit controls currently apply to images.
- Legacy top-level defaults/results_video remain readable for old installations;
  there is no master switch in the UI. Enabling a component in an old master-off
  file migrates the switches without silently enabling other components.

## Build

Requirements: Rust stable (edition 2024), Visual Studio C++ Build Tools, and a
Windows SDK. GPUI Kit is pinned to `0.7.1` (GPUI `0.3.8` snapshots).

From the repository root:

```powershell
# Standard Visual Studio installation with the Windows SDK:
powershell -NoProfile -ExecutionPolicy Bypass -Command "& ./ui/scripts/build.ps1"

# If your SDK is provided by a standalone NuGet layout:
powershell -NoProfile -ExecutionPolicy Bypass -Command `
  "& ./ui/scripts/build.ps1 -CargoArgs @('build','--release','--locked') -SdkRoot 'C:\path\to\windows-sdk'"
```

`IIDXBG_SDK_ROOT` can be set instead of `-SdkRoot`. Without the SDK argument the
script runs `vcvars64.bat` and uses the SDK installed with Visual Studio.

Output: `ui/target/release/iidxbg-config.exe`.

## Run

The release ZIP places `iidxbg-config.exe` beside `iidxbg.toml`. Extract the ZIP
contents into the game directory, then double-click `iidxbg-config.exe`; no
arguments are needed. For a manual build, keep the executable beside
`iidxbg.toml`, or pass the file explicitly:

```powershell
.\iidxbg-config.exe --config ..\iidxbg.toml
```

With no argument it checks, in order: the executable's folder, the current
folder, `IIDXBG_CONFIG`, and the crate's parent folder. Relative paths inside the
TOML are still resolved by the plugin from the TOML's own folder.

## Behaviour notes

- **Previews really call the configured provider.** With `source = "remote"` or
  `"mixed"`, enabling a surface can download an image and save it to the
  configured `save_directory`. A missing local folder is reported in the card's
  error strip instead of a raw OS error.
- **Independent switches.** Turning results off only disables results.
- **No file watcher.** Reload in the title bar picks up external changes.
- **Binding takes precedence.** Bound consumers use their target's source. Their
  independent settings stay in the file and become active when unbound.
- **Config comments survive.** Tests cover comment preservation, invalid-write
  rollback, override removal, concentration sharing, Windows path quoting, and
  per-surface output sizes.

## Verify

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -Command `
  "& ./ui/scripts/build.ps1 -CargoArgs @('test','--offline') -SdkRoot 'C:\path\to\windows-sdk'"
powershell -NoProfile -ExecutionPolicy Bypass -Command `
  "& ./ui/scripts/build.ps1 -CargoArgs @('clippy','--offline','--all-targets','--','-D','warnings') -SdkRoot 'C:\path\to\windows-sdk'"
cargo fmt --manifest-path ui/Cargo.toml --check
```

The GUI was exercised locally at 200% Windows scaling: gallery and
card editor, status overlay, immediate TOML write, and an actual prepared image.
This does not replace in-game validation of the plugin. When redistributing the
GUI executable, include the GPUI Kit (Apache-2.0) and Lucide icon (ISC) notices.

The reorganized editor was additionally exercised on 2026-10-25 at 2500×1650
and 1650×1200 physical window sizes with 200% scaling. Checks covered category
navigation, Ctrl+F/BPM search, Escape, compact navigation, scrolling, independent
character/Qpro choices and their saved TOML, invalid-size rejection without a
disk change, and retaining an unfinished value across page changes. Ordinary
modules showed only their three relevant categories. Screenshots and private
fixtures are under ignored `target/ui-settings-*`; no user assets are shipped.
75 core tests and 25 UI tests passed (7/1 optional tests skipped), along with
Clippy, release builds and exact-build native guards. The new source routing has
not yet been tested in game; earlier MMD/Qpro playback was user-confirmed.

The design borrows category navigation and search destinations from
[VS Code settings](https://code.visualstudio.com/docs/configure/settings), and
separates routine controls from specialized options as in
[OBS settings](https://obsproject.com/kb/obs-studio-overview). Here, searches open
a relevant category, provider controls follow the selected source, and advanced
model/download options have their own page. Existing automatic saving remains.

## Source layout

| Path | Role |
|---|---|
| `src/app.rs` | model, window/theme setup, custom title bar, preview scheduling and copy rules |
| `src/gallery.rs` | full-bleed Metro grid, hover details, preview modules |
| `src/editor.rs` | editor input state, validation, media pickers and bindings |
| `src/editor/pages.rs` | category layout, search destinations and contextual controls |
| `src/store.rs` | comment-preserving TOML load/edit/validate/atomic write |
| `src/preview.rs` | provider worker, fingerprints, BGRA→GPUI images, placeholders |
| `src/theme.rs` | type scale, colors, responsive layout metrics |
| `src/widgets.rs` | small shared GPUI Kit-based building blocks |
