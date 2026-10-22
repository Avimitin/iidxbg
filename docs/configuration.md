# Configuration

This document describes version 0.2.2, including per-surface sources, download
retention, blurred contain fill and scene-driven reload. The canonical API section is
`[moebooru]`; unknown sections/source modes are rejected.

`iidxbg.toml` is read at startup and when the worker handles scene requests,
including deferred preparations and retries. Save edits to change the next query without restarting.
Reload validates the entire file and replaces settings together, including the
HTTP client's domain/redirect policy and timeout. A missing, invalid or unknown
setting during reload is logged in `iidxbg.log`; the last valid configuration
continues to be used. At startup there is no previous configuration, so an
invalid file still prevents startup. The shipped file is a complete starting
configuration.

An optional GUI in `ui/` edits this file without dropping comments. It writes
valid changes immediately, exposes per-surface inheritance and reports the image
it would prepare next. The results surface still follows the top-level `enabled`
switch; see [the UI README](../ui/README.md).

| Setting | Meaning |
|---|---|
| `enabled` | False skips image preparation and selects stock artwork for the next visit; lifecycle hooks remain active so true can re-enable without restarting |
| `source` | `local`, `remote`, or `mixed` |
| `local_directory` | Folder of PNG/JPEG images; randomly selected, avoiding immediate repeats when another usable image is found |
| `recursive` | Search subfolders, at most eight levels and 10,000 candidates; symlinks/junctions are skipped |
| `moebooru.domain` | API domain: `konachan.net` (default when omitted), `konachan.com`, or `yande.re` |
| `moebooru.tags` | Search tags; default `rating:safe` |
| `moebooru.save_directory` | Where successfully decoded downloads are saved |
| `moebooru.page_max` | Randomly select one listing page from 1 through this number, then shuffle its posts; default 5, range 1–100 |
| `moebooru.max_images` | Maximum managed downloads in the save folder; 0 disables cleanup, maximum 1,000,000 |
| `moebooru.timeout_seconds` | Per-request timeout, 3–60 seconds; default 20 |
| `image.width`, `image.height` | Output texture canvas; default 1920×1080; ranges 64–3840 and 64–2160 |
| `image.fit` | `cover` fills/crops the canvas; `contain` keeps a sharp centered image over a heavily blurred, stretched copy |
| `image.max_download_mb` | File-body limit for local and remote images, 1–64 MiB |
| `image.max_source_pixels` | Reject images exceeding this decoded pixel count; at most 64 million |

The default local folder is `iidxbg`, and downloads are saved in
`iidxbg/downloads`, both relative to `iidxbg.toml`. With the default
`recursive = false`, downloaded images are not included in local selection.

Example source settings:

```toml
source = "mixed"
local_directory = 'D:\Pictures\IIDX\Local'
recursive = true

[moebooru]
domain = "yande.re"
tags = "rating:safe landscape"
save_directory = 'D:\Pictures\IIDX\Remote'
page_max = 5
timeout_seconds = 20
```

This is an excerpt; keep the other fields from the shipped TOML. Every option
in that file has explanatory comments. Keep top-level settings before the
`[moebooru]` and `[image]` sections. Single-quoted paths preserve backslashes
literally; forward slashes work too. See the [TOML specification](https://toml.io/en/v1.0.0)
for comments and string syntax.

Local folders are rescanned on each preparation; new local images and
configuration/path edits can be used without restarting. Saved remote files
are not automatically part of `local_directory`;
point the local folder there if you want offline random playback of downloads.

Selection/result entry and exit and gameplay-owner retirement request reloads.
No config or image-file watcher runs while sitting idle on a screen.
The worker requests images at startup, on results/selection entry and exit,
and when retiring a cover/graph owner,
subject to the 30-second success interval or 60-second failure retry below.
One configuration snapshot covers the whole query (listing, candidate retries,
download, save and decode). Edits during a query apply to the next query.
An already prepared image is not retroactively replaced, and the current
results visit stays fixed, including when changing `enabled`. Disabling clears
the previous custom selection at the next visit that receives the disabled
decision; re-enabling takes effect after a subsequent query prepares an image.

The plugin requests public metadata from `/post.json` on the selected domain.
[konachan.net](https://konachan.net/help/api),
[konachan.com](https://konachan.com/help/api) and
[Yande.re](https://yande.re/help/api) use the shared Moebooru listing API.
The downloader prefers
`sample_url`, followed by `jpeg_url` and `file_url` when a field is absent.
It makes no account changes and requires no credentials. It tries up to three
shuffled candidates per preparation; an empty random page falls back to page 1.
This is randomness within the selected page, not a uniform sample of the entire
site. Successful preparation has a minimum 30-second interval; failures retry
after 60 seconds. HTTP errors, including provider throttling, use that backoff.

Only HTTPS image hosts belonging to the selected provider are accepted, including
redirects: `konachan.net`/`konachan.com` and their subdomains for Konachan,
or `yande.re` and its subdomains (including `files.yande.re`) for Yande.re.
Download body, pixel and decoder allocation limits are independent.
Saved files use `provider-post-id-content-hash.jpg/png` (`konachan` or `yandere`),
so the two sites' post IDs remain distinct. Files are validated before saving,
and are published from a temporary file. Existing named files are preserved.
Set `moebooru.max_images` to enable automatic retention. Before and after each
enabled surface query, the worker counts managed files in that surface's
`save_directory` and deletes excess files oldest modification-time first
(filename breaks ties). It only counts regular files matching downloader names:
`konachan-<id>-<20 hex chars>.jpg/png` or `yandere-<id>-<20 hex chars>.jpg/png`.
Unrelated/local artwork, partial files, subfolders, symlinks and junctions are
excluded. It never recursively deletes a directory. A missing cache directory
is harmless; cleanup errors are logged without failing a prepared image.
A shared folder shares the count across both providers: use matching limits
for surfaces sharing a folder, or give each surface its own save directory.
The lowest active limit will constrain a shared folder. `0` means unlimited.
Displayed images are owned pixels, so removing their cached file does not alter
what is already on screen. Preview-tool commands do not prune directories.

The game never receives a URL or an updated archive filename. The plugin decodes
new bytes, creates a new texture and rebinds the native illustration. It preserves
the game's illustration position/animation and other UI, and centers the custom
texture using half its own width/height as the origin. The correct texture canvas
for every result layout still needs in-game verification; use the diagnostic
image's border and central cross to check the crop.


## Additional surfaces

Set any of `surfaces.song_select`, `surfaces.sudden_plus`, `surfaces.lift`,
`surfaces.target_graph`, `surfaces.premium_area`, or `surfaces.concentration` to
true. They default to false, including in older configurations that omit
`[surfaces]`. Each option has comments in the shipped TOML. Each enabled surface
chooses its image independently and has its own repeat history, source
directory, Moebooru client and fit policy. `enabled = false` disables all
surfaces at the next prepared decision/visit. See
[background surfaces](background-surfaces.md) for behavior and the pending
in-game checks.

`premium_area` and `concentration` are implemented only for the inspected
**IIDX 34 / 010-0917** build; other exact-build profiles install their previous
surface set and ignore the two switches. Their native display canvas is
**1280x720**, so the worker decodes them directly to that canvas and leaves the
game's own placement and upscaling in charge. `concentration` shares the
prepared premium-area image while `[sources.concentration]` is empty; adding any
field there gives concentrate mode an independent source.

A selection-screen edit becomes visible on a **new selection visit after the
next query has reloaded TOML and prepared the image**. It does not repaint the
current visit. Leaving selection, results, or a tracked gameplay owner requests
another query, subject to the existing backoff. `[image]` width/height remain
the result canvas; each other surface decodes directly to its own shape from
its original selected image: selection 1920x1080, covers 432x723, graph 432x870,
premium area 1280x720, concentration 1280x720.

## Independent source overrides

The shipped config has active overrides for six surfaces. Each starts in
local mode with its own `iidxbg/<surface-folder>` and
`iidxbg/downloads/<surface-folder>`. Folder names are `results`, `song-select`,
`sudden-plus`, `lift`, `target-graph`, and `premium-area`. Edit the
corresponding override to change its mode or path; explicit overrides take
precedence over global values. Results and song selection default to `contain`;
SUDDEN+, LIFT and target graph default to `cover` for their narrow canvases. Any
surface can use either mode. The additional background enable switches remain
separate in `[surfaces]`. `[sources.concentration]` is intentionally empty, so
concentrate mode reuses the premium-area image instead of querying twice.
The following example illustrates alternative settings, not the shipped defaults.

Use `[sources.results]`, `[sources.song_select]`, `[sources.sudden_plus]`,
`[sources.lift]`, `[sources.target_graph]`, `[sources.premium_area]`, and
`[sources.concentration]`. Each accepts optional `source`,
`local_directory`, `recursive`, `fit`, and a nested `moebooru` section.
Nested fields are `domain`, `tags`, `save_directory`, `page_max`,
`timeout_seconds`, and `max_images`. Omitted values inherit the main config;
all paths are relative to the TOML file. Surface enable switches remain in
`[surfaces]`, and the main `enabled` switch turns everything off.

```toml
[sources.results]
source = "local"
local_directory = 'iidxbg/results'
fit = "contain"

[sources.song_select]
source = "remote"
fit = "contain"
[sources.song_select.moebooru]
domain = "yande.re"
save_directory = 'iidxbg/downloads/song-select'
max_images = 200

[sources.sudden_plus]
source = "local"
local_directory = 'iidxbg/upper'

[sources.lift]
source = "local"
local_directory = 'iidxbg/lower'

[sources.target_graph]
source = "remote"
[sources.target_graph.moebooru]
domain = "konachan.com"
save_directory = 'iidxbg/downloads/graph'
max_images = 100

[sources.premium_area]
source = "local"
local_directory = 'iidxbg/premium-area'
fit = "contain"

# Empty: concentrate mode reuses the prepared premium-area image.
[sources.concentration]
```

The worker rereads and validates all settings for each scene request or retry.
One snapshot covers its full preparation batch. All replacement clients are constructed
before accepting a config edit; failure retains the whole last valid config.
Queries run sequentially on the worker; each completion publishes immediately,
so a prepared result need not wait behind another surface's download. Failed
surfaces keep their own previous image, or stock if none exists. Independent
random choices may still pick the same image when sharing a folder/provider.

`source = "remote"` means downloading through the supported Moebooru API.
`moebooru.domain` explicitly selects the endpoint, permitted image hosts and
cache filename namespace. It does not infer an API for arbitrary sites.

## Contain fill

`fit = "contain"` is the global, results and song-selection default. It preserves the whole sharp foreground image at its aspect ratio.
A stretched copy, heavily blurred on the worker, fills the unused canvas.
The blur is computed on a small image before upscaling to keep its cost bounded.
`cover` preserves aspect ratio, scales to fill the canvas, then center-crops the
excess without a blurred backdrop. It is the default for SUDDEN+, LIFT and target
graph. For manually cropped local artwork, set that surface to `contain` to keep
the whole image; matching the canvas aspect ratio avoids unused space. A per-surface `fit` overrides
`image.fit`; active visits retain their existing pixels until the next visit.

## Live selection frame character

Version 0.2.2 accepts:

```toml
[selection]
frame_chara = true
frame_chara_image = 'iidxbg/frame-chara.png'
```

`frame_chara` defaults to true, preserving native visibility. False hides the
character during ordinary song selection. True does not create a character
where the game has none. This works independently of `surfaces.song_select`.
The optional `frame_chara_image` defaults to an empty string (native artwork).
Set it to a local PNG/JPEG path, relative to the TOML file or absolute, to use
a static replacement. Clear the path to return to native artwork. The visibility
switch controls both native and custom character artwork.

Scene hooks request **reload config -> prepare character/background images ->
publish** on the single worker. Startup, selection/result entry and exit, and
cover/graph retirement are triggers. The selection exit trigger also covers
leaving selection for gameplay; it is not an exact song-confirmation hook.
There is no 500 ms watcher. Saving an edit alone does not trigger work. On the
next request, changed paths and image size/modification time are checked.
Invalid config edits retain the last accepted settings; invalid images retain
the previous prepared character, or stock if none exists. Clearing the path
returns to native art. Top-level `enabled = false` restores character behavior
when the worker processes the next request. Ongoing downloads may delay this;
they never block the game thread. Background rate limits and visit freezing
remain unchanged. Character changes can apply to the current selection visit
after the worker handles its entry request.

**The source image does not need to be 1920x1080.** Any dimensions within the
configured decode limits are accepted. Transparent PNG is recommended; JPEG
is opaque. The inspected stock IIDX33 artwork is 700x800 inside a padded
702x802 texture, while its AFP animation canvas is 1920x1080. The current hook
replaces that whole animation layer. It fits input into the animation canvas
with transparent padding; a full-canvas PNG is only an optional way to control
placement manually. This does not reconstruct the stock child artwork's
transform. It follows native root position/origin/scale/color/priority, without
internal character animation. Background fit/blur and Moebooru settings do not
apply to this local image; file/pixel limits still apply.

Exact placement, transitions and reset handling need in-game testing on both
builds. Evidence: [IIDX 33](iidx33-frame-chara.md),
[IIDX 34](iidx34-frame-chara.md).
