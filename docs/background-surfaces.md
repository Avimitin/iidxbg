# Additional background surfaces

Implemented in **0.2.0**, for the exact IIDX 33 and 34 fingerprints in the
build-specific records. In-game validation of
these new surfaces is pending on both versions.

Enable individual options in `[surfaces]` in `iidxbg.toml`:

```toml
[surfaces]
song_select = true
sudden_plus = true
lift = true
target_graph = true
```

All four default to false. Results replacement continues to use `enabled`.
Each enabled surface independently selects an image. `[sources.<surface>]`
overrides its local directory, source mode, Moebooru site/settings and fit;
omitted values inherit the main config. The worker decodes each original image
directly to the required canvas: selection 1920x1080, covers 432x723, and graph
432x870. Results keep `[image]` width/height. These canvases do not change game
resolution or cover height. SUDDEN+ and LIFT have separate CPU buffers.
`contain` uses a blurred, stretched backdrop around its sharp foreground.
See [configuration](configuration.md) for examples and download count limits.

| Surface | Integration |
|---|---|
| Song selection | D3D9 quad at the outer movie dispatcher, gated by the ordinary selection scene update/cleanup/destruction. It draws before the AFP UI, preserving and restoring D3D state. IIDX 33 uses the movie fade; the supplied 34's stubbed movie helper uses full opacity. |
| SUDDEN+ | Dedicated native bitmap sprite at priority 108, temporarily substituted for the upper artwork pair during native cover visual update. |
| LIFT/lower cover | Independent bitmap for the lower pair at priority 108; native HIDDEN/LIFT selection, movement, clipping, alpha and visibility still apply. |
| Target graph | Dedicated bitmap at priority 145, temporarily substituted at graph +64 during the native artwork helper, including default and animated selections. Native marker-derived clipping and score UI remain active. |

Both static and animated cover choices use a dedicated replacement bitmap;
the original animation is hidden while replacement is active. Cover mirror
and flip operations are copied in native order. Numeric adjustment layers
remain native. The graph uses the stock background tint (alpha 1, RGB 0.7).
If its frame/marker is absent, the custom sprite stays hidden. Native graph
mode guards still decide whether that component runs.

## Ownership and reload

One image/config decision is frozen on first selection update or first cover/
graph visual callback for that object's visit. Late images wait for another
visit. Initialization, destruction and graph reset end ownership; their query
requests are coalesced by the worker's existing 30-second success / 60-second
failure intervals. Editing TOML does not change an image already on screen.
The worker rereads the file before the next query. Invalid edits preserve the
last valid provider/config snapshot. Disabled decisions take effect at a new
visit after the worker publishes them. No network, file reads or image resize
occurs in game/graphics callbacks.

The native descriptor sprite factory allocates from the retained sprite pool.
AFP group 0 is selected temporarily and the caller's group restored. Each
sprite is released before unregistering its owned texture. A hook on the common
native sprite release observes scene/group/pool retirement before its slot can
be reused. Cover init/destruction and graph reset, parent reset and map storage
destruction retire tracked owners. Device invalidation drops GPU resources but
retains frozen pixels for recreation. Original art pair/graph pointers are
restored before returning from the native visual callback.

New inline hooks use bundled MinHook, publish all trampolines before activation,
and retain them for the pinned DLL lifetime. Native entries are fingerprinted
and guarded; supported existing entry chains are preserved. Selection vtables
are not overwritten, allowing bilimani's wrappers to keep calling native entries.
The original results vtable hooks continue independently.

## Validation and remaining unknowns

Read the independent [IIDX 33](iidx33-background-surfaces.md),
[IIDX 34 / 20260917](iidx34-background-surfaces.md) and
[IIDX 34 / 010-0917](iidx34-010-0917-analysis.md) records for hashes, ABI and RVAs.
`verify-surfaces.py` checks static paths, every runtime entry guard and AFP
imports. `smoke-attach.py` installs all hooks in a private mapped image without
executing the game, and checks rejection of modified code.

Still requires real-game checks: selection fading and render order, lane art
crop/origin for 1P/2P/DP and mirror modes, cover adjustment, static/animated
customization, default/custom graph, device reset, repeated songs and coexistence
with bilimani/spice hooks. Arena, battle and alternate layouts are not claimed
as validated. Only the pre-existing results artwork was confirmed by the user
on IIDX 33. No game DLL or game asset has been modified or committed.


Selection frame-character controls are a separate live exception to background
visit freezing. `[selection] frame_chara` hides/shows native or custom character
art; `frame_chara_image` optionally selects a local image. See
[configuration](configuration.md#live-selection-frame-character) and the
build-specific [33](iidx33-frame-chara.md) / [34](iidx34-frame-chara.md) evidence.
