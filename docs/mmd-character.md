# User-imported MMD character

The selection character can render an imported PMX model in real time with
VMD motion. No model, motion, texture or audio is bundled. This initial renderer
is enabled only on exact game hash
`c98e183ec81ecb9e1cb13c237f9c9cb0b3f65b6fdaff6fd5e6a526008c546534`
(the existing IIDX 34 / 010-0917 profile).

## Configuration

In `iidxbg-config.exe`, open song selection, enable the character and MMD,
choose an extracted PMX, then choose the normal VMD. Keep the model's texture
files alongside it in their original relative directories. Texture references
must stay inside the PMX's directory (parent/absolute references are rejected).
Configured PMX and VMD paths can be
absolute or relative to `iidxbg.toml`. Nothing is copied into game archives.

Each condition has a separate optional VMD file picker:

| Condition | Playback |
| --- | --- |
| Normal | Silent looping dance; empty means rest pose with optional blinking. |
| Scratch up / down | Separate VMDs (for example look up / look down); plays once, repeated ticks in the same direction do not restart it. |
| A / AA / AAA | Plays once on returning from ordinary results to song selection. |
| B through F | Plays once on returning from ordinary results to song selection. |

A result motion takes priority over scroll motions until it finishes. After a
condition motion finishes, normal dancing resumes on its continuing timeline.
An empty condition path keeps the normal motion. Input uses spice2x's direct
plugin SDK turntable state, with circular wrap handling and jitter suppression;
no network API/password is needed. Both players' platters and directional TT
buttons are supported. Reverse the direction in the GUI if needed. This detects
scratch gestures during selection, not exact song-row movement. Custom
condition VMDs take precedence over procedural head tracking.
Initial clip changes are cuts,
without pose crossfading. Prefer short gestures for condition motions.

Optional **idle BPM matching** uses the selected chart's maximum BPM divided
by the original BPM entered for the normal VMD. It defaults off. VMD files do
not contain BPM; the default 120 is only a placeholder. Changing selections
preserves motion phase, with speed limited to 0.25–4x. Unknown BPM uses 1x.
Scratch/result motions keep their original speed. This matches tempo, not beat
phase or preview audio. The character still disappears during gameplay.
See [native BPM evidence and limits](iidx34-mmd-tempo.md).

The configuration app also offers an **upper-left preset** and display size,
horizontal center and vertical center controls. These are percentages of the
native character area. The entire model canvas stays inside reserved top,
bottom and side margins; large sizes and edge positions are constrained.
The preset uses 42% size, X 20%, Y 32%, reducing song-detail overlap while
retaining native draw order. Long titles can still overlap. Preview shows the
model pose, not its placement within the game's complete UI.

The **original size and position** button restores the earlier large-character
mapping (82%, centered) and disables the HUD size constraint. That constraint
also has its own switch. An independent **replace selection Qpro** switch puts
a smaller copy of the same model at each joined player's native Qpro position.
The small model's size is independent of the large model; both share the same
animation and GPU pose. Disable the existing large-character switch to show
only the Qpro replacement. Native Qpro returns when replacement is disabled or
unavailable. Other Qpro screens and optional background effects remain native.
See [the exact-build Qpro analysis](iidx34-mmd-qpro.md).

The reaction switch controls these condition motions and procedural head/smile
overlays. The configurable head bone looks toward the right-hand list while
the platter moves, with a small up/down tilt. The default target is 45 degrees,
held for 1.5 seconds after scrolling stops. Its rotation is blended in world
space so the VMD's head turn does not cancel the reaction; local neck rotation
is limited to 80 degrees. The angle and hold time are available in the GUI. The
happy morph fades after six seconds following A-or-better. Names must match a
PMX Japanese or English bone/morph name. Empty names disable those overlays.
For the supplied plush set, use `head` (English) or `頭-T` for the look bone:
the visible head inherits only 30% of the usual `頭` controller's rotation.
Tracking also compensates the selected bone's inherited rotation grant.
Blinking has its own morph name and preserves VMD expressions between blinks.
The grade condition is met by either joined side; it does not require a clear.

Example (edit existing sections instead of duplicating them):

```toml
[selection]
frame_chara = true

[mmd]
enabled = true
model_path = 'characters/model/model.pmx'
motion_path = 'motions/dance.vmd'
bpm_sync = false
motion_bpm = 120.0 # replace with the known motion tempo before enabling
display_scale = 42.0
position_x = 20.0
position_y = 32.0
keep_hud_clear = true
replace_qpro = false
qpro_height = 360.0
scroll_up_motion_path = ''
scroll_down_motion_path = ''
happy_motion_path = 'motions/celebrate.vmd'
other_result_motion_path = ''
fps = 30
width = 512
height = 768
scale = 0.85
yaw = 0.0
reactions = true
look_bone = '頭'
look_yaw = 45.0
look_hold_seconds = 1.5
scratch_reverse = false
happy_morph = '笑い'
blink_morph = 'まばたき'
```

The GUI offers 30/60 fps. Advanced TOML framing supports width/height 128–1024,
scale 0.1–2.0 and yaw -180–180 degrees. Camera framing is fixed from model rest
bounds; motions with large travel may move outside the character canvas.
The GUI preview is a still rendered pose. File/config changes are prepared at
scene request boundaries; a failed import keeps the last valid imported model,
or the existing image/stock character when none has loaded.

## Supported subset and cost

PMX geometry, PNG/JPEG/TGA/BMP textures, BDEF/SDEF/QDEF skinning, VMD bone and
morph animation, IK, group/bone/vertex/basic UV and diffuse material morphs are
supported. Lighting is simple directional shading. Physics, soft bodies,
MME, sphere/custom toon maps, outlines, advanced material factors, additional
UV sets, VMD cameras/lights and WAV audio are not rendered. Rig compatibility
still matters; a matching name does not guarantee a motion suits a model's
proportions. Transparent geometry uses model material order, not per-triangle
depth sorting.

Default rendering is 512×768 at 30 fps. PMX/VMD/texture reads, decoding, pose
evaluation, IK and skinning run on workers. The game thread uploads completed
vertices and renders into a transparent D3D9 target, then reuses the existing
native character atlas path. No model framebuffer is copied back through the
CPU in the game. GPU objects and reset handling belong to the graphics owner.
The animation worker parks outside ordinary selection; last GPU-lease release
signals termination without joining in a game callback.

Compared with hardware-decoded video, this adds CPU animation/skinning and GPU
draw calls. The cost depends on mesh complexity, material count, texture sizes
and refresh rate. Offline CPU timings exclude atlas composition, game load and
GPU time; they cannot establish live frame pacing. Import limits: 100k vertices,
200k triangles, 512 bones/morphs, 256 materials, 64 IK links/iterations, 64 MiB
PMX, 32 MiB per VMD, 4096-pixel texture edges and 256 MiB decoded textures.

## Tools and validation

`iidxbg-tool mmd-inspect <model.pmx> [motion.vmd]` reports bone/morph names and
unsupported features. `mmd-preview <config.toml> <output.png> [seconds]` renders
a still with a separate hidden-window D3D9 device; only this offline preview
uses GPU readback.

Tests use an original procedural triangle/rig/morph and generated VMD, with no
third-party assets committed. They cover import failures, texture containment,
skinning, looping, morph/head overlays, condition
priority and fallback, and worker-lease ownership. The optional
`standalone_gpu_renders_animation_and_transparency` test renders to a real D3D9
device. `supplied_model_motion_and_gpu_preview` additionally accepts local
`IIDXBG_TEST_PMX` and `IIDXBG_TEST_VMD`; diagnostic images go under ignored
`target/mmd-validation`. Native signal evidence is recorded in
[the exact-build analysis](iidx34-mmd-reactions.md).

### Validation on 2026-10-25

All seven local plush PMX models imported with the supplied `IRIS OUT.vmd`;
the import report is under `target/mmd-validation/imports.json`. Kotone
(`ことぬい.pmx`, 7,206 vertices) was rendered with real D3D9 at 0, 1 and 3
seconds and with the happy/head overlay. Visual inspection caught and corrected
back-face culling; the resulting poses, face and hair textures were inspected.
The procedural GPU test also passed transparency and changed-frame assertions.
These previews are local test outputs, not distributed assets.

Motion SHA-256:
`06d89dd98d2a3cb18a8802a70425906c1d47b683354fb1358cba08271eddd2fd`.
Model SHA-256:
`5c7cfadc1e598bee9a01e8df3e0ae5cd51abdbc87c8c9c4e341a49460f6aa85c`.
On an i5-12490F / RTX 2070 SUPER workstation, 120 release-build pose/skinning
samples averaged 0.866 ms per frame. This measures CPU preparation only;
it is not a comparison against video or an in-game frame-time measurement.

**Actual in-game rendering has been tested** on the exact 010-0917 hash.
The first build (`8d4b2e7412f74cc5bb9305d9a6c53ddaca8e0dd9506483615c631840bb23e51d`)
loaded the private model on the game host. After a manual launch, the model
appeared in ordinary SP selection; the user confirmed playback. Selection to
gameplay transition was captured with no model left over the playfield, while
existing backgrounds/covers/graph media continued. Evidence is under
`target/mmd-validation/menu-main.jpg` and `dance-a-main.jpg` / `dance-b-main.jpg`
(the latter pair captures gameplay, despite their original filenames).

The user found the original brief additive head turn insufficient. Inspection
of the same hashed PMX showed bone 14 (`頭`) drives bone 15 (`頭-T`, English
`head`) through a rotation grant of 0.3; bone 15 has 6,078 weighted vertex
influences. The private test configuration now selects `head`. The stronger
world-space look compensates parent and inherited rotations. An offline D3D9
render at 3 seconds / 45 degrees shows the visible head facing right; see
`target/mmd-validation/iris-look-right.png`. Reproduce with the supplied-model
ignored test and the environment paths described above. Its live cursor signal
and appearance were not confirmed in-game: the outer list index remained zero
as the visible song list moved. Input was subsequently changed to the direct
Spice SDK analog/directional-button reader, sampled on the animation worker.
Separate short look-up/down private test VMDs were generated from the supplied
IRIS first pose, with an added visible-head track; source/output hashes and
the generator are under `target/mmd-validation/gestures.json` and
`make-look-motions.py`. They are not distributed assets.
A/AA/AAA reactions, P2/DP and induced
device loss remain unverified. Native game DLLs/assets remain unchanged. Plugin,
GUI and private test configuration have backups under the ignored deployment
directory recorded in `target/mmd-validation/deployment.json`.

Automated checks for the earlier stronger look: 68 root tests passed (7 optional
tests skipped), 23 UI tests passed (1 private inventory test skipped), both
Clippy runs passed with warnings denied, and the two optional real-D3D9 tests
passed. Exact-build native guards passed. Release builds of the plugin/tool and
configuration app passed. Updated plugin SHA-256:
`e0aac564091820515a152735a97a469133e9550d8b530d0231ff8801814c7064`;
configuration app:
`c8cd5578acb2ed1911d385272a88fe76a0b8fab380457e594f2db38a09a471f5`.
Both were staged on the test host, preserving the previous binaries and
configuration. The running game was not restarted automatically; this update
requires the user's manual restart. Deployment evidence and backups are
referenced by `target/mmd-validation/look-update.json`.

Scratch-input revision: 70 root tests passed (7 optional skipped), 23 UI tests
passed (1 optional skipped), both Clippy runs and both release builds passed.
Native guards passed after removing the obsolete cursor guards. Real D3D9
previews of both private look VMDs were inspected at 0.5 seconds. Read-only
Spice API inspection confirmed a live normalized P1 turntable value; this is
not yet evidence of the new in-process SDK trigger.

Staged plugin SHA-256:
`6f6305a2c8fa52b2de4db839c9c184b2dc0b419dbe1585020c752bbe1995b3d7`;
configuration app:
`a0fbd26203880bb697cd2f3e00efcafe395f41340eb3941a139b41cc307d14c0`.
The private test configuration selects separate `look-up-test.vmd` and
`look-down-test.vmd`; normal motion remains IRIS. Backups and hashes are in
`target/mmd-validation/scratch-update.json`. The user performs the restart;
actual in-game scratch-trigger validation subsequently succeeded: the live
log recorded in-process P1 triggers, and the user confirmed the separate
look-up/down motions were working. New tempo/placement validation is tracked
in [the follow-up analysis](iidx34-mmd-tempo.md).
