# iidxbg

An experimental `iidxbg.dll` plugin for spice2x that replaces the **per-song
results illustration** with a random local image or an image from Konachan.
It prepares images in the background and keeps one image fixed for each results
visit. Konachan images are saved in your configured folder.

One DLL supports the inspected **IIDX 33 / 20260812** and
**IIDX 34 / 20260917** x64 builds, selected automatically by exact SHA-256.
Other builds are refused. Fingerprints and native evidence:
[IIDX 33](docs/iidx33-analysis.md), [IIDX 34](docs/iidx34-analysis.md).
Requires spice2x with **Spice SDK v0.4 D3D9 callbacks**.
The user has confirmed image display and corrected placement in **IIDX 33**.
**IIDX 34 has only offline validation**; broader stability testing remains.
It targets the native `illust` area, which may be
masked or cropped by the game.

## Install and try

1. Download the `iidxbg-<version>-windows-x64.zip` asset from
   [GitHub Releases](https://github.com/Avimitin/iidxbg/releases) and extract it.
   Keep `iidxbg.dll`, `iidxbg.toml` and the `images` directory together.
2. Add `-k "D:\your\folder\iidxbg.dll"` to your existing spice64 launch command.
   Keep any other plugin arguments you already use.
3. The default configuration uses local images. The package includes a generated
   diagnostic image so the first test needs no network. Add your own `.jpg`,
   `.jpeg` or `.png` files to `images`; remove the diagnostic image when done.
4. Play a song and inspect the ordinary results screen. Check `iidxbg.log`
   alongside the DLL for attachment, image preparation and upload messages.

The next image is prepared at startup and after each results exit, ahead of the
following song's results. Late/failed downloads keep the previous image or stock
background. No image should change halfway through a results visit.

To use Konachan, change `source = "local"` to `source = "konachan"` in
`iidxbg.toml`. `"mixed"` randomly chooses local or Konachan and falls back to the
other source on failure. Set `konachan.save_directory` to your download folder,
for example `"D:/Pictures/IIDX/Konachan"`. Relative paths are relative to the DLL,
not the launcher's working directory. Restart the game after changing TOML.

[Configuration](docs/configuration.md) ·
[Build and test](docs/development.md) ·
[Version design](docs/architecture.md)

