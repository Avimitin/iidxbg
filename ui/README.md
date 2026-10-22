# iidxbg configuration UI

A Windows desktop editor for `iidxbg.toml`, built with **Rust**, **GPUI**, and
[**GPUI Kit**](https://github.com/longbridge/gpui-kit). It is a separate optional
binary; the plugin still ships `iidxbg.dll` and `iidxbg.toml` on its own.

## What it does

- **Metro gallery** — the board is full-bleed, with no page padding or hero.
  Four 16:9 surface tiles form the top band, which now reserves at least 30% of
  the content height; four shorter portrait columns below continue the same
  hairline grid to the window edges. The last lower column is a compact
  global-settings shortcut.
- **Grid / image separation** — the grid owns only the region geometry and the
  dividing lines. Each image module is a separate layer that centres one surface
  inside its region with `ObjectFit::Contain`; it never paints a tile fill.
- **Hover details** — the regions carry no permanent footer. Hovering one lifts a
  compact floating panel with its name, status, source/fit, and an
  “打开配置” action; clicking anywhere in the region opens the editor.
- **Disabled placeholder** — an unenabled region shows a thin neutral outline
  plus a surface glyph and eye-off badge. There is no added panel fill; the
  page colour remains the only background. Enabled regions fetch the next image
  through the plugin's own local/Moobooru provider and centre it in the region.
- **Black-only surfaces** — the page, sidebar, panels, inputs, pills, and hover
  card are black. Separation comes from one-pixel outlines; green is limited to
  icons, selected labels/borders, switches, and status dots rather than a tinted
  secondary fill.
- **Centered editor** — clicking a region opens a centered panel. Source overrides,
  fit, recursion, and Moebooru fields can each inherit the global value or be
  overridden. The results surface follows the global master switch because the
  TOML format has no independent `surfaces.results` key.
- **Global options page** — master switch, source, local directory, Moebooru
  account-free API settings, output size/fit/limits, and the selection-character
  options.
- **Native folder picker** — every local/download directory field has a folder
  button that opens the Windows folder picker. Choosing a folder fills the input
  and writes the selected path immediately; cancel leaves the current value
  untouched.
- **Immediate, comment-preserving writes** — there is no Save button. Valid
  changes are written atomically to `iidxbg.toml` on every edit, using
  `toml_edit` so the shipped explanatory comments and unknown whitespace stay
  intact. Invalid numeric edits are not written; the last valid file remains and
  the UI shows the error. External edits can be picked up with **重新载入**.
- **Inheritance is explicit** — an override key is only created when the user
  presses **自定义** or picks a non-inherit segment. Empty sections continue to
  inherit, and concentration mode keeps reusing the prepared premium-area image
  until it gets an override.

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
- **The results switch is global.** The TOML schema only has the top-level
  `enabled` switch for results; the six extra surfaces have their own
  `[surfaces]` switches. Turning results off turns every surface off.
- **No file watcher.** The UI writes immediately, but it does not watch the file
  for outside changes. Use **重新载入** on the global page.
- **Explicit `[sources.*]` values win.** The global page reports how many
  surfaces still inherit the global source; use each card editor to add or remove
  an override.
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

The GUI was exercised locally at 200% Windows scaling: gallery, global page,
card editor, status overlay, immediate TOML write, and an actual prepared image.
This does not replace in-game validation of the plugin. When redistributing the
GUI executable, include the GPUI Kit (Apache-2.0) and Lucide icon (ISC) notices.

## Source layout

| Path | Role |
|---|---|
| `src/app.rs` | model, window/theme setup, custom title bar, icon rail, preview scheduling and copy rules |
| `src/gallery.rs` | full-bleed Metro grid, hover details, preview modules |
| `src/editor.rs` | centered per-surface editor and override controls |
| `src/global.rs` | global options page |
| `src/store.rs` | comment-preserving TOML load/edit/validate/atomic write |
| `src/preview.rs` | provider worker, fingerprints, BGRA→GPUI images, placeholders |
| `src/theme.rs` | type scale, colors, responsive layout metrics |
| `src/widgets.rs` | small shared GPUI Kit-based building blocks |
