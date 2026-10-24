# Built-in artwork scanner — 2026-10-24

The All Assets page's **扫描游戏素材** button runs the read-only Rust scanner in
`ui/src/scanner`. Nothing scans on application startup or page navigation.
The GUI detects `data/graphic` above its config directory or prompts for an
installation. Completion imports the generated catalog; cancellation/failure
keeps the previous one. **查看报告** reveals the report in Explorer.

## Format and mapping evidence

The independent references are mon/ifstools 3.1, mon/kbinxml 2.1, and our
`inventory-artwork.py`, `inventory-geometry.py`, `inventory-loose-artwork.py`
and `build-artwork-catalog.py` output. MIT notices are retained in
`licenses/ifs-reader.txt`, the generated notices, and release packages.
No Python runtime or external executable is launched by the scanner.

IFS signature is big-endian `0x6cad8f89`; the manifest/data boundary is at file
offset 16, and the optional digest changes the manifest start from 20 to 36.
Binary XML has separately aligned byte/word/dword data lanes and six-bit names.
Texture names are recovered through their encoded MD5, and `imgrect`/`uvrect`
coordinates are divided by two before cropping. AVSLZ output is bounded by the
declared length. IFS DXT1/DXT5 requires a per-u16 byte swap before standard BC
decoding. BGRA8888 and ARGB4444 are also decoded. Some metadata incorrectly
declares Shift-JIS for UTF-8; recovery also changes the MD5 name encoding.

AFP geometry lists restore `name_shapeN` identifiers. GE2D/D2EG counts and offsets
match `inventory-geometry.py`; only draws with flags 9 become `@shape/` fill
entries. Atlas regions keep their archive/name keys, loose images use `@file`
and a path relative to `data/graphic`, and the main movie is `@movie/main`.
Known preset names receive bilingual search hints. No arbitrary file names are
used as cache output paths: previews/records are keyed by SHA-256.

**RVAs / ABI / lifetime:** no new native hooks, offsets or ABI assumptions.
All input bytes, decoded images and catalog records are worker-owned Rust data.
Only completion/progress crosses to the GUI. Nothing runs on game/graphics
callbacks. Scanning another build does not imply that the plugin supports it;
the existing exact-DLL guards remain the authority for rendering replacement.

## Verification and reproduction

Private input copies: `analysis/asset-inventory-20261024/{archives,loose}`;
the test installation `analysis/scanner-fixture/data/graphic` contains local
hard links to these copies, not links to the running game's files.
Input SHA-256 values for all 5,716 files are recorded in the generated report:

- Report SHA-256: `7f62db4315c4206c654ab60e901a5f0199256b457715064a89b74788c011a6c9`.
- Reference catalog SHA-256: `9b9a4f81d068d96928774018310ddc8ac80b7d4431192b81d21f9d4b65860699`.
- Generated catalog SHA-256: `2ffb084ce2deaf7752868716778e87b1f9797dd2afcdc3db749d226a516a43b6`.
- Config EXE SHA-256: `d839c2c288b4dba073bd6675ca584561ea8cee3f78ccd34ccc25a499d23cd9bb`.

The 4,520 archives and 1,196 loose inputs produce **147,761 unique entries**.
Every identifier and dimension matches the reference. Its 147,762 rows contain
one duplicate: `19033.ifs / stn_wing_r`. A sample spread across the complete
catalog compares 1,980 unscaled atlas previews pixel-for-pixel with zero
differences. A separate 10-input GUI scan also matched all 784 entries and 441
native-size atlas previews. Exactly 27 known unreadable inputs remain: the
21 archive placeholders and six loose thumbnails documented in the artwork
analysis. There are no additional reader errors in this corpus.

To reproduce with private local copies:

```powershell
$env:IIDXBG_SCAN_CORPUS = 'D:/Analysis/test-installation'
$env:IIDXBG_SCAN_OUTPUT = 'D:/Analysis/scanner-cache'
cargo test --manifest-path ui/Cargo.toml --release scanner::tests::private_inventory_scan -- --ignored --nocapture
py scripts/verify-artwork-scanner.py reference/catalog.json generated/catalog.json --report parity.json
```

Use the SDK build wrapper documented in development.md when needed. The parity
script checks all names/dimensions and evenly samples up to 2,048 native-size
atlas previews; `--pixel-limit 0` compares all eligible previews.

22 UI tests and 58 core tests pass; UI/core Clippy with warnings denied,
format checks and both release builds pass. Synthetic tests cover alignment,
wrong encoding labels, span truncation, LZ77 overflow/overlap, DXT word order,
GE2D endianness/indices/alpha, cancellation and cache placement. Actual desktop
UI testing exercised scan, progress, automatic catalog loading and note search.
**No new in-game testing was performed for this GUI-only change.**

## Cache and limits

Cache root: `%LOCALAPPDATA%/iidxbg/asset-cache/<source-path-hash>/scanner-v2`.
Requested rescans rehash inputs and reuse unchanged records. Content-identical
texture/shape thumbnails are shared. No archive is extracted or modified.
Catalog/record JSON publication uses temporary files and replacement; cancelling
can leave useful completed previews/records, but does not publish a partial
catalog. Reparse-point directories are not followed. Input/archive size is
bounded at 256 MiB, image dimensions at 16,384, and catalogs at 200,000 entries.

The first full pass took 379 seconds; the corrective pass reusing cached inputs
took 139 seconds, including four recovered archives. These are local wall times
under concurrent build/verification load, not a game performance measurement.
Requested rescans still read every file to validate its content hash.

Unknowns/limits: external `_super_` references are explicitly reported and
skipped, matching the previous Python inventory mode; they were not needed by
this corpus. Unsupported/corrupt files are reported rather than repaired.
Resized thumbnail filters and vector rasterization differ from Pillow/native
rendering; previews are not AFP animation playback. The reader does not identify
new executable ABIs, replace generated text/fonts, or interpret 3D model geometry.
