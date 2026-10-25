# iidxbg configuration UI

A Windows desktop editor for `iidxbg.toml`, built with **Rust**, **GPUI**, and
[**GPUI Kit**](https://github.com/longbridge/gpui-kit). It is a separate optional
binary; the plugin still ships `iidxbg.dll` and `iidxbg.toml` on its own.

## What it does

- Bottom-left user menu: compact scrollable rows, name/card keyword search,
  current user pinned first in green, one-click switching, inline creation with
  a plus button or Enter, and a settings shortcut on each row. No legacy import.
- The selected user is independent of the page. **用户与卡片** groups rename/delete,
  multiple card bindings, live P1/P2 capture and that user's Bilibili binding.
  Switching users switches credentials and favourites and cancels pending QR login.
  Shared pre-login modules never use a user's Bilibili account.
- Compact regular-weight title bar with a single-click **游戏素材** button.
  Title/login preview cards are pinned above the shared artwork catalog.
  The page works without users and has no user header or management shortcut.
  Closing an editor or switching users keeps its shared scope. Direct asset
  replacements apply to everyone; unbound cards and guests keep stock user modules.
  New user modules start disabled. See [user sessions](../docs/user-sessions.md).
- All buttons have transparent backgrounds, with color and border feedback.
  Sidebar icons use color alone for selection, without background blocks.
  The account page omits its redundant management shortcut. **重新读取配置**
  reloads the configuration file and previews after external edits; UI edits auto-save.
- Display components retain the scrollable adjacent mosaic, with permanent white
  names in small, light text, outlined and shadowed in black. Module hover popups
  are removed; preview errors remain visible in the editor. Help is in tooltips beside section titles
  and controls; errors and active states remain visible.
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
- Module settings open with **设置**, with **媒体来源** visible only for
  **单独设置**. Shared modules hide that page and its search destinations.
  Download, API and model tuning controls are integrated into **设置**; there
  is no separate advanced page. Settings use category navigation, search and a fixed
  save/error footer. On narrow windows, categories wrap above the form. Ctrl+F
  focuses search; Escape clears search first, then closes the editor.
- Song selection also has pages for the large character, Qpro, MMD model/dance
  and reactions. Ordinary modules show settings and, when independent, media.
  Provider controls follow the selected source; BPM and reaction details appear when enabled.
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
a relevant category, provider controls follow the selected source, and model/download options
share the settings page. Existing automatic saving remains.

## Source layout

| Path | Role |
|---|---|
| `src/app.rs` | model, window/theme setup, custom title bar, preview scheduling and copy rules |
| `src/gallery.rs` | adjacent mosaic, permanent outlined labels and preview modules |
| `src/editor.rs` | editor input state, validation, media pickers and bindings |
| `src/editor/pages.rs` | category layout, search destinations and contextual controls |
| `src/store.rs` | comment-preserving TOML load/edit/validate/atomic write |
| `src/preview.rs` | provider worker, fingerprints, BGRA→GPUI images, placeholders |
| `src/theme.rs` | type scale, colors, responsive layout metrics |
| `src/widgets.rs` | small shared GPUI Kit-based building blocks |
| `assets/iidxbg.ico`, `assets/app.rc`, `build.rs` | bundled multi-resolution Windows icon (resource 1, also used by GPUI) |
| `scripts/generate-icon.ps1` | original vector geometry and reproducible 16–256 px ICO generation |

## Multi-user UI verification (2026-10-25)

Core: 87 tests passed / 7 optional tests ignored. UI: 36 passed / 1 optional
scan test ignored. Both crates passed Clippy with warnings denied, formatting
checks and release builds. The refactor includes per-user Bilibili credential
routing in providers and live transport; it introduces no native hooks, RVAs or
ABI changes. Session tests cover separate users/config files, ignoring legacy
credentials, DPAPI rejection of copied ciphertext, logout isolation, anonymous
pre-login sources and rejecting stale QR/account UI responses.

Windows UI checks used isolated configurations under ignored
`target/multi-user-ui`, including an empty configuration and 24 synthetic users.
At 200% scaling (2100×1450 and 1650×1200 windows), checks covered creation with
the plus button, name search and Enter selection, current-user ordering,
scrolling without moving the underlying gallery, per-row settings, card binding,
deleting the final user and shared pre-login edits. Follow-up checks covered
single physical clicks on the title-bar assets button, title/login preview cards,
remaining in the shared page after closing editors, user switching and reload,
and saving artwork bindings/masks globally without touching either user record.
Normal, hovered, selected and disabled buttons have transparent backgrounds;
dark mode is loaded before applying the custom palette so it cannot replace it.
Saved TOML confirmed that unrelated users were unchanged. The final editor was
checked for Settings-first navigation, hiding/restoring Media when binding and
unbinding, Escape dismissal, smaller light module labels, removed module-page
headers and the concise top-bar assets entry. The built executable includes nine
icon sizes in resource group 1; the Windows window class loads the icon. A two-user
fixture with a deliberately corrupt session verified account switching and local
unbind without affecting the other user's state. No real Bilibili login was changed;
credential-storage tests use temporary fixture sessions. Actual in-game testing
was **not** performed for this refactor.

Final Windows checks also confirmed that module hover keeps only the border
feedback, sidebar icons have no selected/hover background, the account page
omits its own management shortcut, and the reload button reads **重新读取配置**.
