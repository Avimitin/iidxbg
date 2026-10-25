# Selection Qpro replacement with a shared MMD model

Analysis date: 2026-10-25. Input `D:\Share\tmp\010-20260916.dll`,
18,200,216 bytes, SHA-256
`c98e183ec81ecb9e1cb13c237f9c9cb0b3f65b6fdaff6fd5e6a526008c546534`.
AMD64, image base `0x180000000`, image size `0xc5ff000`. This is the existing
010-0917 profile by file hash. Addresses below are RVAs. No game file is changed.

## Native component and ownership

IDA/Hex-Rays and instruction-level inspection established:

| RVA | Evidence |
| --- | --- |
| `0x8d63c0` | Ordinary selection constructor calls the Qpro constructor with scene +8464 at `0x8d64f7`/`0x8d64fe`. |
| `0x98f5e0` | Constructs `MusicSelectQpro`: vtable at +0, layout layer at +8, active/visible bytes +16/+17, initially zero. |
| `0xd86e58` | `MusicSelectQpro` vtable: destructor `0x98f630`, setup `0x98f670`, transform update `0x98f850`, visibility update `0x98f7e0`. RTTI resolves to `MusicSelectQpro`. |
| `0x8da4e0` | Ordinary selection setup passes archive index 34 and priority **119** to component setup. Priority immediate at `0x8da53d`. |
| `0x98f670` | Setup obtains joined players' Qpro layers, sets priority, creates `qpro_music_1player` or `qpro_music_2players`, writes active/visible bytes to 1. For two players, P2 priority is 120. |
| `0x98f850` | Looks up layout anchor `qpro` or `qpro_01p`/`qpro_02p`, then transfers its geometry/color to each native Qpro layer through `0x81c4e0`. |
| `0x98f7e0` | Transfers component visible byte +17 to joined players' Qpro layers through `0x800f70`. |
| `0x8d8290` | Existing selection update calls component vtable +16 (transform update) at `0x8d848e` before forwarding to base-scene update. |
| `0x8d8260` | Selection rendering/update phase invokes component vtable +24 (visibility update), then updates the Qpro manager. |
| `0x98f7b0` | Releases layout layer via virtual +32, clears +8 and +16/+17. Called by selection cleanup at `0x8d801b` and the component destructor. |
| `0x806020` | `manager*()` returns static manager at `0xb120300`. |
| `0x805070` | `player*(manager: RCX, side: EDX)` validates side 0–1 and returns manager +160*side. |
| `0x800670` | `CLayer*(player: RCX, instance: EDX)` validates instance 0–1 and returns pointer at player +24*instance. Ordinary selection uses instance 0. |
| `0x8007b0` | Constructs native `qp_motion` layers through the existing layer factory. Appearance parts bind within this layer; optional `qp_bg` is a separate layer. |
| `0x92fda0` | `u32()` player/layout count, 1 or 2; same getter used by Qpro setup and transform update. |

All new callable entries and layout anchors have 16-byte guards in
`src/profiles/iidx34/010-20260917.rs`, alongside the full-file hash. Existing
ordinary selection vtable and `BM2D::CLayer` vtable (`0xd968b0`) are checked
before accessing objects. Other profiles do not enable this replacement.

## Geometry and visibility ABI

`0x81c4e0` borrows a `CLayer*` and temporary `CMovieClip*` anchor. Disassembly
confirms Win64 virtual setters: position +72 takes integer X/Y in EDX/R8D,
origin +104 takes integer X/Y, scale +208 takes floats in XMM1/XMM2, and color
+168 takes a pointer. Anchor getters use AFP import `afp_core_115` at
`0xc66c28`: property 4104 is a float position pair, 4123 is origin, **4109**
is scale, and 4106 is RGBA. In particular `0x32b330` reads the first scale
float, `0x32b360` the second. These are the native Qpro transfer properties,
not inferred from other image adapters.

The existing guarded layer-dimensions helper (`0x32d350`) obtains native canvas
width/height through `afp_core_71`. The replacement keeps the anchor position
and relative origin, fits the model into a square of the requested size and
multiplies by the anchor scale. Color is copied. This also preserves negative
scale/mirroring. Default square size is 360 native layout units; PMX transparent
padding affects the visible character height. A bounded diagnostic reports
actual native canvas, position, resulting origin and scale on first use per side.

The native `CLayer` is 616-byte pooled storage (factory `0x32da80`), with layer
handle at +8 and root clip ID at +528. Visibility setters act on layer flags,
so the plugin instead masks only root property 4103 through the existing AFP
property setter (`0xc66c10`), marking 4126 dirty. The original component's
visibility updates continue. The replacement additionally follows component
active/visible flags and joined players, and copies root visibility. Optional
`qp_bg` effects remain native; no appearance part, user equipment or save data
is edited.

Only identity integers (layer address, layer handle, root ID) and saved root
visibility persist. The next native selection callback restores the root before
calling original code. Restoration first obtains the current layer through the
validated scene/component and native getters and compares all three identity
values. A pooled address with a new layer/root handle is rejected. Cleanup,
destruction and device invalidation restore while owners are valid and retire
replacement sprites. The common native release observer clears metadata; GPU
leases retire on the graphics owner. Temporary anchor wrappers never persist
across callbacks.

## Rendering and configuration

No new detour is installed. The existing graphics-owner artwork callback runs
Qpro replacement after selection update. It preserves original calls and native
priority, and hides stock only after a prepared model, usable anchor, owned GPU
texture and replacement sprite all exist. Failure retains stock Qpro. Ordinary
selection cleanup drops the sprites; gameplay, results and other Qpro screens
are not replaced.

Large and small characters use the same imported model, animation session and
rendered pose. Native sprites need straight-alpha pixels, while the model render
target is premultiplied. A lazy GPU-only conversion reuses the existing compiled
atlas shader, producing one cached straight-alpha texture per pose revision.
Both small sprites share it. There is no game framebuffer readback and no second
model animation/render pass. Targets, depth, viewport and D3D state are saved and
restored; both textures are released/recreated with the graphics owner on reset.
Only the offline preview reads GPU pixels back to test/save an image.

`[mmd] replace_qpro` defaults false; `qpro_height` defaults 360 (100–800).
The existing selection character switch controls the large character and can
be disabled independently. The MMD worker remains available when Qpro alone is
enabled. Both placements share condition motions and optional BPM behavior.

The large-character original preset is display scale 82, X/Y 50, with
`keep_hud_clear = false`; this reproduces the pre-placement atlas mapping.
The upper-left preset keeps HUD margins enabled. Disabling margins restores
original sizing rather than silently limiting it to the previous 77% safe area.

## Reproduction and unknowns

1. Hash the supplied binary, open it in IDA and follow the above constructor,
   setup, update and cleanup paths. Inspect float register arguments in
   `0x81c4e0`, not just inferred decompiler types.
2. Run `scripts/verify-native.py` against this exact file after a fresh release
   build; it checks all exported guards and hook targets. No game assets are
   needed for automated geometry/lifetime tests.
3. Run Qpro anchor/ownership tests. The optional real-D3D9 MMD test exercises the
   straight-alpha copy, including translucent pixels whose RGB must exceed alpha.
4. Enable MMD and Qpro replacement, enter ordinary selection, compare the small
   character with the native anchor, then leave for gameplay and return. Disable
   replacement to confirm stock restoration; test missing models, P2/DP,
   two-player entry, mode changes and induced device reset before claiming them
   validated. Inspect the bounded `MMD Qpro P1/P2` log.

Static evidence establishes the native path. Actual placement still needs live
inspection; authored Qpro canvases and unusual frame layouts may need a different
small-character size. Other Qpro uses (results, mode select, customization,
attendants) are outside this scope. User PMX/VMD assets are not distributed.

## Automated validation on 2026-10-25

74 root tests passed (7 optional tests skipped), including Qpro anchor fitting
and root-mask restoration against recycled layers, roots and components. The
optional real-D3D9 test also passed: it renders animation/transparency and checks
the GPU straight-alpha copy with partially transparent pixels. The configuration
app passed 23 tests (1 optional test skipped), including persistence of the new
switches and size field. Both Clippy checks passed with warnings denied.

These checks do not establish actual Qpro placement or lifecycle behavior in the
game. The new replacement has not yet been tested in game; earlier large-model
playback and scratch reactions were verified separately.

Both release builds passed. `scripts/verify-native.py` with the fresh release
tool passed all native-entry/hook-slot guards and existing ownership/lifecycle
checks against the input hash above. The release tool loaded the private current
Temari PMX and normal VMD and rendered a real-D3D9 pose at one second through
the new straight-alpha path. The resulting image was visually inspected:
`target/mmd-validation/qpro-temari-preview.png` (not a game capture).

Plugin SHA-256:
`48aaaabb93246e9992c920ab8b301cd2934cddc61b2b0a4c5ec447c79ce8ae9a`.
Configuration app SHA-256:
`9ace8f727253160b8a049b88822222358cdf328ad1f27ea6ec8b765a0c3a9ca0`.
Both were installed on the test host with the new settings: original large
placement (82%, center 50/50, HUD margins off), Qpro replacement on, size 360.
The user's Temari paths and condition motions are preserved; BPM matching
remains off. Before/after hashes and backups are recorded in
`target/mmd-validation/qpro-update.json`. No restart command was sent; a manual
restart is required to load this DLL. The passive `qpro-before-main.jpg` capture
shows gameplay with the previous DLL and is not evidence of this replacement.

After restarting, the user confirmed that the new Qpro replacement works in
game. This is user-reported live validation of the installed build and current
selection layout; it does not cover the outstanding P2/DP, reset, or mode-change
cases listed above.
