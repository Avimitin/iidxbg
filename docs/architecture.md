# Version support

Ship one `iidxbg.dll` for IIDX, with an independently verified profile for each
supported build. This is explicit support for known binaries, not automatic
compatibility with older or newer versions.

Configuration, local selection, Konachan/Yande.re, downloading, decoding, pending pixels
and result-visit selection are shared. The native boundary selects a profile by
the game file's exact SHA-256, checks the PE image size, then checks live native
entry bytes and original vtable targets before installing hooks. A changed entry
is accepted only for the constrained MinHook chain described in
[the IIDX 33 compatibility investigation](iidx33-hook-compatibility.md).
Calls continue through the original game entry so the existing hook stays active.

The remote provider shares listing, downloading, decoding and save logic across
the three supported domains. A typed domain selects the API endpoint, permitted
image/redirect hosts and provider namespace. Konachan's two domains share a
namespace; Yande.re has a separate one for labels and saved filenames.
`[moebooru]` is the only API section name. Source modes are local, remote and
mixed; there are no legacy aliases.

Scene entry/exit and gameplay-owner retirement wake the worker to reload and
validate TOML before preparing images. Deferred requests and retries also reload. It swaps the complete provider configuration and HTTP client only
after successful construction, retaining the previous image label. Invalid
reloads keep the last valid snapshot. One snapshot governs the entire query.
No file reads, HTTP work or configuration parsing occur in graphics callbacks.
The worker publishes either owned pixels or a disabled decision for the next
visit. The graphics owner freezes that decision at entry; disabling clears its
previous image, while an ordinary preparation failure retains the previous
selection. Lifecycle hooks remain installed while configured off so future
visits can request a reload and re-enable. Configuration never resets the
separate native fault/shutdown latch.

Each profile owns its addresses, scene layout and hook slots. IIDX 33 uses
scene +256 and cleanup slot 14; both inspected IIDX 34 builds use +264 and slot
17. Runtime code does not infer these from a version number or scatter version
conditionals through the image pipeline. Unknown builds fail attachment and
leave the game untouched. Profiles are embedded in the DLL; they are not
user-editable runtime JSON.

The inspected builds share the Microsoft x64 callback signatures and the
64-byte AFP texture wrapper layout, so they use the same native adapter.
A future build with different signatures, ownership or rendering needs a
separate adapter with its own evidence, still behind the same image pipeline.
Do not try to express incompatible behavior through offsets alone.

Separate version DLLs become justified if versions require incompatible
architectures/dependencies or independent release lifecycles. Neither is true
of these three inspected x64 builds. Separate artifacts now would duplicate
packaging and invite loading the wrong plugin without improving isolation.

See [IIDX 33 evidence](iidx33-analysis.md),
[IIDX 34 / 20260917 evidence](iidx34-analysis.md) and
[IIDX 34 / 010-0917 evidence](iidx34-010-0917-analysis.md). Static compatibility
does not establish in-game stability. The user confirmed IIDX 33 image display
and corrected placement; broader lifecycle testing remains. Both IIDX 34 builds
have only offline validation.

## Additional surface adapters

`runtime/surfaces.rs` uses the same exact-build profile and CPU image worker.
The worker publishes an immutable bundle of optional selection/cover/graph crops
only when its request generation is still current. Each scene/component freezes
its bundle; GPU resets keep those CPU pixels. Five worker-owned providers have
independent settings and repeat histories, inheriting global defaults through
`Config::for_surface`. At each request, a complete validated config and replacement clients are built
before swapping; one snapshot governs the whole surface preparation batch. Each completed query publishes
its image with the other cached images; a failure retains only its own previous
image. SUDDEN+ and LIFT no longer share a bitmap. New switches default off.

Cleanup is a separate worker effect before/after each enabled query. It prunes
only downloader-named regular files in the effective Moebooru save directory,
oldest modification time first, with zero meaning unlimited. It skips links,
reparse entries and subdirectories, rechecks candidates before deleting, and
logs errors without converting a successful image preparation to failure.
Contain resizing builds a low-resolution blurred backdrop and overlays a sharp
centered foreground; no native ABI or hook address changes are needed.

Selection draws directly at the native movie dispatch layer with D3D state
restoration. Cover/graph adapters allocate native retained CSprites from custom
texture descriptors and temporarily substitute artwork pointers only during
native layout calls. Explicit owner lifecycle and a common sprite-release
observer prevent stale pooled pointers. MinHook trampolines are published before
queued activation and remain allocated for the pinned plugin's lifetime.
The original calls and supported pre-existing hook chains are preserved.
See the build-specific surface documents for ABI and teardown evidence.


## Live selection character

Selection/result entry and exit, plus cover/graph retirement, increment a
request generation and unpark the single worker. Startup also prepares once.
There is no config watcher and no periodic file/metadata polling. The worker
reloads the complete config, prepares optional character pixels, then prepares
backgrounds. Character refresh is done even when background queries are still
in the existing 30-second success / 60-second failure backoff. An in-flight
HTTP request may delay handling a new scene request; no IO runs on game threads.
Only pending deferred/retry work uses a timeout; otherwise the worker parks
until a hook/log/shutdown notification. Duplicate requests coalesce through the
generation counter. Selection exit is a lifecycle trigger, not a verified song-
confirmation event. Result/selection entry is detected once per owner visit.

Failed reloads retain the last accepted settings/image. Character image
fingerprints include path, modification time, length and decode limits and
are checked only at request boundaries. Arbitrary input dimensions are accepted;
the transparent 1920x1080 output is the movie canvas convention, not a required
source image size or a reconstruction of the stock 700x800 child-art transform.
Background visit freezing stays unchanged. Character updates can take effect
in the current selection visit when the queued event finishes processing.

The existing ordinary-selection update hooks restore the previous native root
visibility before calling the original update, then mask only the character
root afterward. Native animation/update continues. A replacement uses an owned
retained sprite with native root geometry/color and draw priority. Scene cleanup,
release observation and device invalidation retire masks/sprites before owner
reuse; restoration checks the current scene, exact layer vtable, live handle
and root ID. No stale scene is dereferenced. A failed geometry read/upload keeps
stock art. There are no additional detours; the dimension helper gains an exact
entry guard. Runtime uses imported AFP property functions, not AFP internal RVAs.
See [33 evidence](iidx33-frame-chara.md) and [34 evidence](iidx34-frame-chara.md).
