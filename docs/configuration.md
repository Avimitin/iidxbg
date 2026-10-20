# Configuration

`iidxbg.toml` is read once when the Spice SDK initializes the plugin. A missing,
invalid or unknown setting disables startup and is reported in `iidxbg.log`.
The shipped file is a complete starting configuration.
`source = "konachan"` and `[konachan]` are accepted as aliases for `remote`.
Use only one remote settings section; specifying both names is an error.

| Setting | Meaning |
|---|---|
| `enabled` | Set false to skip image preparation and native hooks |
| `source` | `local`, `remote`, or `mixed` |
| `local_directory` | Folder of PNG/JPEG images; randomly selected, avoiding immediate repeats when another usable image is found |
| `recursive` | Search subfolders, at most eight levels and 10,000 candidates; symlinks/junctions are skipped |
| `remote.domain` | API domain: `konachan.net` (default when omitted), `konachan.com`, or `yande.re` |
| `remote.tags` | Search tags; default `rating:safe` |
| `remote.save_directory` | Where successfully decoded downloads are saved |
| `remote.page_max` | Randomly select one listing page from 1 through this number, then shuffle its posts; default 5, range 1–100 |
| `remote.timeout_seconds` | Per-request timeout, 3–60 seconds; default 20 |
| `image.width`, `image.height` | Output texture canvas; default 1920×1080; ranges 64–3840 and 64–2160 |
| `image.fit` | `cover` fills/crops the canvas; `contain` keeps the whole image with black margins |
| `image.max_download_mb` | File-body limit for local and remote images, 1–64 MiB |
| `image.max_source_pixels` | Reject images exceeding this decoded pixel count; at most 64 million |

Example source settings:

```toml
source = "mixed"
local_directory = 'D:\Pictures\IIDX\Local'
recursive = true

[remote]
domain = "yande.re"
tags = "rating:safe landscape"
save_directory = 'D:\Pictures\IIDX\Remote'
page_max = 5
timeout_seconds = 20
```

This is an excerpt; keep the other fields from the shipped TOML. Every option
in that file has explanatory comments. Keep top-level settings before the
`[remote]` and `[image]` sections. Single-quoted paths preserve backslashes
literally; forward slashes work too. See the [TOML specification](https://toml.io/en/v1.0.0)
for comments and string syntax.

Local folders are rescanned on each preparation;
new local images can be used without restarting. Configuration/path edits need
a restart. Saved remote files are not automatically part of `local_directory`;
point the local folder there if you want offline random playback of downloads.

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
Downloads are retained until you delete them; there is no automatic disk eviction.

The game never receives a URL or an updated archive filename. The plugin decodes
new bytes, creates a new texture and rebinds the native illustration. It preserves
the game's illustration position/animation and other UI, and centers the custom
texture using half its own width/height as the origin. The correct texture canvas
for every result layout still needs in-game verification; use the diagnostic
image's border and central cross to check the crop.

