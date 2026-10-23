> 2026-10-23 update: all listed surfaces now accept video/live media. Premium
> uses the shared media sprite and concentration updates its bound texture from
> the same graphics-owner callbacks. See the latest [validation](validation.md)
> and [exact-build evidence](iidx34-010-0917-analysis.md#subscreen-video-and-live-streams-2026-10-23).
> The original implementation notes below describe the image-only stage.

# Additional background surfaces

Implemented in **0.2.1**, for the exact IIDX 33 and 34 fingerprints in the
build-specific records. In-game validation of
these new surfaces is pending on both versions.

Enable individual options in `[surfaces]` in `iidxbg.toml`:

```toml
[surfaces]
song_select = true
sudden_plus = true
lift = true
target_graph = true
premium_area = true
concentration = true
```

All six default to false. Results replacement continues to use `enabled`.
Each enabled surface independently selects an image. `[sources.<surface>]`
overrides its local directory, source mode, Moebooru site/settings and fit;
omitted values inherit the main config. The worker decodes each original image
directly to the required canvas: selection 1920x1080, covers 432x723, graph
432x870, premium area and concentration 1280x720. Results keep `[image]`
width/height. These canvases do not change game resolution or cover height.
SUDDEN+ and LIFT have separate CPU buffers. `contain` uses a blurred, stretched
backdrop around its sharp foreground.
See [configuration](configuration.md) for examples and download count limits.

| Surface | Integration |
|---|---|
| Song selection | D3D9 quad at the outer movie dispatcher, gated by the ordinary selection scene update/cleanup/destruction. It draws before the AFP UI, preserving and restoring D3D state. IIDX 33 uses the movie fade; the supplied 34's stubbed movie helper uses full opacity. |
| SUDDEN+ | Dedicated native bitmap sprite at priority 108, temporarily substituted for the upper artwork pair during native cover visual update. |
| LIFT/lower cover | Independent bitmap for the lower pair at priority 108; native HIDDEN/LIFT selection, movement, clipping, alpha and visibility still apply. |
| Target graph | Dedicated bitmap at priority 145, temporarily substituted at graph +64 during the native artwork helper, including default and animated selections. Native marker-derived clipping and score UI remain active. |
| Premium area | **IIDX 34 / 010-0917 only.** Both native `PremiumAreaBG` nodes (base layer and customization sprite) are hidden and an owned sprite with the visible node's AFP position/origin/scale/color is created at its priority. The replacement is kept across scenes; only a native sprite-pool release restores the saved visibility. |
| Concentration | **IIDX 34 / 010-0917 only.** Both concentration window elements are examined. The `concentration_bg_set/bg` clip is rebound to the prepared texture with the native dynamic-descriptor setter; if that clip is missing, the default `subbg0_concentration` clip is tried, then the layer root. The clip keeps its own transform, parent layer and draw order, and stock art is restored from the saved archive/name where available. |

Both static and animated cover choices use a dedicated replacement bitmap;
the original animation is hidden while replacement is active. Cover mirror
and flip operations are copied in native order. Numeric adjustment layers
remain native. The graph uses the stock background tint (alpha 1, RGB 0.7).
If its frame/marker is absent, the custom sprite stays hidden. Native graph
mode guards still decide whether that component runs.

The premium-area and concentration images are prepared on a 1280x720 canvas,
the native size of both displays, so the game's own placement and upscaling
remain in charge. `concentration` reuses the prepared premium-area image while
`[sources.concentration]` stays empty; adding any field there gives it an
independent source. No other source/settings keys are required.

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

The premium-area replacement is frozen to the bundle that created it and is
rebuilt when a new prepared bundle is published for the area. It saves and
restores the native visibility; the common sprite-release observer restores it
immediately when the scene sprite pool retires our sprite, because the native
premium nodes are session-global and would otherwise stay black on the next
screen. The concentration replacement rebinds both candidate window clips per
stage, freezes that decision for the stage and follows the native window
show/hide flag. It saves each clip's native archive/name and rebinds it through
the native archive setter on hide, disable or device invalidation.

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

The two new surfaces are being validated in-game. The first round confirmed the
premium-area image and exposed two defects that are now addressed: an owned
concentration sprite used a layer-relative position and appeared far to the
right, so the implementation now rebinds the native clip instead; and the
premium sprite pool retired with its scene while the native nodes stayed hidden,
leaving the session-global premium area black on the results screen, so the
release observer now restores native visibility. Remaining checks: the default
concentration view may not contain the same named background clip (the log
records both element lookups), and the premium base-layer path without a
customization sprite is still unconfirmed. Keep the shipped 1280x720 canvas.


Selection frame-character controls are a separate live exception to background
visit freezing. `[selection] frame_chara` hides/shows native or custom character
art; `frame_chara_image` optionally selects a local image. See
[configuration](configuration.md#live-selection-frame-character) and the
build-specific [33](iidx33-frame-chara.md) / [34](iidx34-frame-chara.md) evidence.
