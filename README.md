# iidxbg

An experimental `iidxbg.dll` plugin for spice2x that replaces the **per-song
results illustration** with a random local image or an image from Konachan or Yande.re.
It prepares images in the background and keeps one image fixed for each results
visit. Downloaded images are saved in your configured folder.

One DLL supports the inspected **IIDX 33 / 20260812**, **IIDX 34 / 20260917**
and **IIDX 34 / 010-0917** x64 builds, selected automatically by exact SHA-256.
Other builds are refused. Fingerprints and native evidence:
[IIDX 33](docs/iidx33-analysis.md), [IIDX 34 / 20260917](docs/iidx34-analysis.md),
[IIDX 34 / 010-0917](docs/iidx34-010-0917-analysis.md).
Requires spice2x with **Spice SDK v0.4 D3D9 callbacks**.
The user has confirmed image display and corrected placement in **IIDX 33**.
**Both IIDX 34 builds have only offline validation**; broader stability testing remains.
It targets the native `illust` area, which may be
masked or cropped by the game.

## Install and try

1. Download the `iidxbg-<version>-windows-x64.zip` asset from
   [GitHub Releases](https://github.com/Avimitin/iidxbg/releases) and extract it.
   Keep `iidxbg.dll`, `iidxbg.toml` and the `iidxbg` directory together.
2. Add `-k "D:\your\folder\iidxbg.dll"` to your existing spice64 launch command.
   Keep any other plugin arguments you already use.
3. The default configuration uses local images. The package includes a generated
   diagnostic image so the first test needs no network. Add your own `.jpg`,
   `.jpeg` or `.png` files to the matching subfolder in `iidxbg` (for example, `iidxbg/results`);
   remove its diagnostic image when done.
4. Play a song and inspect the ordinary results screen. Check `iidxbg.log`
   alongside the DLL for attachment, image preparation and upload messages.

Scene entry/exit hooks request preparation on the background worker, including
prefetching after selection exit ahead of the song's results. Late/failed downloads keep the previous image or stock
background. No image should change halfway through a results visit.

To download images, change `source = "local"` to `source = "remote"` in the desired
`[sources.<surface>]` section of `iidxbg.toml`. In `[moebooru]`, set `domain` to `"konachan.net"` (default),
`"konachan.com"`, or `"yande.re"`. `"mixed"` randomly chooses local or the
selected remote site and falls back to the other source on failure.
Set `save_directory` in that surface's `[sources.<surface>.moebooru]` section to your download folder,
for example `"D:/Pictures/IIDX/Remote"`. Relative paths are relative to the DLL,
not the launcher's working directory. The TOML is reread
when scene events request image preparation, so saved edits apply without restarting. Invalid edits
keep the last valid settings, and the current results image stays fixed.

[Configuration](docs/configuration.md) ·
[Build and test](docs/development.md) ·
[Version design](docs/architecture.md)


## Additional backgrounds

Version 0.2.0 also supports song selection, SUDDEN+, LIFT/lower cover and
target-score graph artwork. Enable their individual switches in `[surfaces]`
in the commented `iidxbg.toml`; they default to false. These new hooks still
need in-game testing on all supported builds. See [surface behavior and testing](docs/background-surfaces.md).

Each background can use its own folder or supported Moebooru site through
`[sources.<surface>]` overrides. `moebooru.max_images` optionally removes oldest
managed downloads, and `fit = "contain"` fills borders with a heavily blurred
copy of the image. The shipped config enables overrides for all five surfaces, using separate
local folders under `iidxbg` and download folders under `iidxbg/downloads`.


Version 0.2.0 also supports selection-character visibility and an
optional local replacement image through `[selection]`. See the commented
config and [character options](docs/configuration.md#live-selection-frame-character).
These controls reload through scene events, with no periodic file watcher.
Any valid image dimensions are accepted. In-game validation remains pending.
