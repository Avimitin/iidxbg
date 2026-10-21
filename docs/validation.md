# Validation record — 2026-10-20

Scope: Windows x64 standalone plugin, known IIDX 33 / 20260812 and
IIDX 34 / 20260917 profiles. No running game was used for this initial validation;
later user-confirmed IIDX 33 results are recorded below.

- Eight Rust unit tests passed: commented TOML configuration, literal Windows
  and Unicode paths, duplicate/unknown keys, types, relative path resolution,
  profile selection and guard
  completeness, invalid PE rejection, image limits, provider URL boundaries,
  preservation of saved files, and image freezing across result visits.
- Clippy passed for all targets with warnings denied.
- Native offline verifier passed for both documented DLL hashes. Checks cover
  live-hook guard source bytes, vtable targets and RTTI, background ownership,
  AFP binding and texture lifetime evidence. It does not execute game code.
- The provider pipeline prepared a local generated 1920x1080 diagnostic image.
- A Konachan request using `rating:safe id:409820`, page 1, downloaded a sample,
  saved it to the configured test directory, decoded and resized it successfully.
  Saved JPEG was 552,532 bytes, SHA-256 prefix `be35e9ddc14a38fa86a5`.
  The downloaded image is ignored and is not redistributed.

The provider checks preceded the version-profile refactor; provider code did not
change during that refactor. Generated test images and downloads stay under
ignored `images`/`target` paths.

The release build, CLI fingerprint checks for both game DLLs and Windows
load/export smoke test passed. The smoke test supplies an SDK without graphics
registration and confirmed clean rejection before native hooks; it does not
test successful game initialization.

Packaged `dist/iidxbg/iidxbg.dll` SHA-256:
`5f77e848231f801ae39027b85e232a08b3c9eb4e5374dafb016ba131e02212f8`.
ZIP: `dist/iidxbg-0.1.0-windows-x64.zip`.

Outstanding at that stage: real-game attachment, visual geometry/cropping, score UI,
scene lifecycle, device reset/shutdown, and frame-time measurements on both
versions. Unit tests and static verification cannot establish those properties.

## Hook compatibility update — 2026-10-21

- Eleven unit tests passed, including constrained MinHook-chain recognition
  against every native guard in both profiles and invalid-chain rejection.
- Clippy with warnings denied, formatting and the release build passed.
- All six private-mapping attachment tests passed: IIDX 33 and 34, each with
  native entries, a synthetic preserved MinHook chain and a modified tail.
  Rejected entries left every vtable slot unchanged.
- Windows LoadLibrary/SDK-refusal smoke test passed.
- Updated DLL SHA-256:
  `7f079cf3b582acffa04ef82a775f989b1f6169e9c9dfcc6c6a91caaec89a62db`.

The reported installation's original disk hash matches the known IIDX 33 build.
Its old log lacked the changed function/bytes, so actual compatibility was
pending an updated run. No successful rendering had been observed at that stage.
Automatic approval review blocked replacement on the network share; the
installed plugin was not changed. The local build is available for replacement.

## Origin correction - 2026-10-21

The user confirmed image display on IIDX 33, with its top-left quarter in the screen bottom-right quarter. The live log identified the preserved cleanup chain as 2dx-gsm.dll.

The new build explicitly sets the uploaded texture origin after binding. Eleven unit tests, Clippy, release build, both native verifiers, six origin-helper emulation cases, six private attachment cases and the DLL SDK-refusal smoke test passed. The ZIP was verified against the built DLL and repository-default TOML.

DLL SHA-256: `3e8ac69538abd559320ae1e6ef93922ad1c0cc718a22a5ea9a6575a53a82ac76`.

The user subsequently confirmed that this build works in-game on IIDX 33,
including corrected image placement. This is user-reported live validation;
the automated checks above were performed locally.

IIDX 34 remains offline-validated only. Broader result-mode coverage, repeated
scene lifecycle and device-reset testing, and frame-time measurements remain
outstanding.

## Konachan domain selection - 2026-10-21

Added optional `konachan.domain`: `konachan.net` (also the default when omitted)
or `konachan.com`. Both sites document the same listing API at
<https://konachan.net/help/api> and <https://konachan.com/help/api>.
The existing post parser and download pipeline are shared.

- Twelve unit tests passed, including domain selection, omitted-field
  compatibility, invalid-domain rejection and both image-host boundaries.
- Formatting, Clippy with warnings denied, release build and Windows DLL
  load/SDK-refusal smoke test passed.
- The release CLI prepared a 1920x1080 image through the `.com` API with
  `rating:safe id:409820`, page 1. Downloads and preview remain under ignored
  `target/konachan-com-check/`; no installation configuration was changed.
- No additional in-game test was performed for this provider-only change.

Related provider check: Yande.re's `/help/api` and
`/post.json?limit=1&tags=rating%3Asafe` returned HTTP 200. The listing contains
`id`, `sample_url`, `jpeg_url` and `file_url`, matching the fields used here.
This initially established listing compatibility only; the implementation and
download check follow below.

## Yande.re provider - 2026-10-21

The shared remote provider now supports `konachan.net`, `konachan.com` and
`yande.re`. The default configuration lists these candidates in comments under
`[remote]`. Existing `[konachan]` and `source = "konachan"` configurations remain
accepted; omitted domains still select `konachan.net`.

The domain selects both API endpoint and permitted image/redirect hosts.
Yande.re permits `yande.re` and its subdomains, including the observed
`files.yande.re`; Konachan permits its two domain families. Labels and saved
filenames distinguish `konachan` and `yandere` post IDs.

- Thirteen unit tests passed, including all three domains, existing config
  aliases, duplicate-section rejection, cross-provider host rejection and
  separate post labels. Formatting and Clippy with warnings denied passed.
- Release build and Windows DLL load/SDK-refusal smoke test passed.
- Live Yande.re listing with `rating:safe landscape` returned post `1270356`
  with a sample on `files.yande.re`. A separate test configuration using
  `rating:safe id:1270356`, page 1, successfully prepared a 1920x1080 preview
  through the release CLI. The saved JPEG is 175,509 bytes, with SHA-256
  prefix `7261785b544396268729`. Test files stay in ignored
  `target/yandere-check/` and are not redistributed.
- The same release CLI also prepared Konachan `.com` post `409820` using the
  previous `[konachan]` configuration, exercising compatibility through actual
  listing, download, decode and save.

Reproduce with `target/release/iidxbg-tool.exe prepare <test.toml> <preview.png>`,
using a separate copy of the shipped TOML with `source = "remote"`, the selected
domain, tags and page limit above. Remote availability may change.

DLL SHA-256:
`d3afb78a291eedbd9a59cecf94fd00ad298d9b26a8de3ad7db58da9c643120ae`.
No additional in-game validation was performed; native rendering code did not
change.

## Release 0.1.1 - 2026-10-21

Package and lockfile version are `0.1.1`; the HTTP User-Agent now derives its
version from package metadata. Thirteen tests, Clippy with warnings denied,
formatting, release build and DLL load/SDK-refusal smoke test passed.
`dist/iidxbg-0.1.1-windows-x64.zip` was verified byte-for-byte against the built
DLL and repository-default configuration, with licenses and diagnostic image.
Local DLL SHA-256:
`3edf8f866864a4508c6da501575721f9a66fd99aa9a9dfd9652098995729fed8`.

Master and annotated tag `0.1.1` were pushed at commit
`a263204fb241fe28158c93e5e01ae8b73d76d554`.
[GitHub workflow 39463334770](https://github.com/Avimitin/iidxbg/actions/runs/39463334770)
completed successfully and published
[`iidxbg-0.1.1-windows-x64.zip`](https://github.com/Avimitin/iidxbg/releases/tag/0.1.1).
The downloaded GitHub asset is 2,066,044 bytes, SHA-256
`d9903d0411d8dbe22f019b025d1a9bb5e0944567dedee1f95ef5df30175297ae`.
Its configuration matches the tagged repository default byte-for-byte and includes
the Yande.re domain. The package contains the DLL, TOML, diagnostic image and
two license/notice files.

## Additional surfaces investigation - 2026-10-21

The read-only `scripts/verify-surfaces.py` passed against both exact DLLs.
It reproduces hash/PE identity, native scene/sprite vtables, lane table stride,
cover clipping, call/tail-jump paths and the supplied 34 selection helper stubs.
Evidence and unresolved runtime questions are in
[background surfaces](background-surfaces.md) and its two build-specific records.
Raw local decompilation snapshots are retained in ignored
`analysis/surfaces-evidence-20261021.json`; no game binary/assets are committed.

This investigation changes documentation and an offline verifier only.
No new hooks were added to the runtime or the 0.1.1 tag, and no in-game testing
of selection, SUDDEN+, LIFT or target-graph replacements was performed.

## Live configuration reload - 2026-10-21 (unreleased)

The worker now rereads TOML before every preparation attempt, including retries,
and atomically replaces the provider snapshot/client after validation. Read,
parse, validation and client-construction errors retain the previous snapshot.
Disabled decisions are delivered through the same pending/visit boundary as
images; lifecycle hooks remain installed for later re-enabling. Native fault
and shutdown flags remain separate from configuration.

- All 15 tests passed. The new provider test edits an actual temporary TOML and
  switches between two local PNG folders and output sizes without recreating
  the provider. It checks updated domain, timeout, tags and save path; retained
  repeat history; malformed/invalid/missing config fallback; disable/re-enable.
- Visit-state tests verify disabled decisions cannot change an active image,
  clear the next visit's prior image, and allow later re-enabling.
- Formatting, Clippy with warnings denied, optimized build and the Windows
  DLL load/SDK-refusal smoke test passed. No in-game test was performed.

Local build: `target/release/iidxbg.dll`, SHA-256
`e114a79b758cb106701cafd2cfaebbcb5fc232073429e229803357b5bfe67bae`.
Local package: `dist/iidxbg-live-config-windows-x64.zip`, SHA-256
`3d80c031a78434d2315a48a475a4d51c01d9fba5cf62eb6722367b2ee75c59a8`.
Every ZIP entry was compared against the built DLL, repository default TOML,
fresh diagnostic image and license/notice files. The published 0.1.1 tag and
asset were not changed; this local package contains subsequent development work.

## Four optional surfaces ? 2026-10-21 (unreleased)

Implemented selection, SUDDEN+, lower HIDDEN/LIFT and target-score graph
adapters for both fingerprinted builds. Options default off in `[surfaces]`.
The existing results and live-reload changes are retained. The provider worker
prepares all crops; each active component freezes its image/config decision.

- All 17 Rust tests passed, including existing-configuration compatibility,
  independent surface opt-ins, invalid-option rejection and BGRA crop/contain
  derivatives without source mutation.
- Formatting and Clippy with warnings denied passed; optimized Windows x64
  DLL and tool built successfully.
- Static surface verifier passed for both game files, including all 33 runtime
  entry guards, scene/sprite vtables, call anchors and AFP group IAT ordinals.
- DLL export/SDK refusal smoke test passed.
- Six private-mapping attachment checks passed: native, supported existing
  MinHook chain and altered-tail rejection for each of IIDX 33 and 34. All
  13 new inline hooks install in successful cases; rejected cases leave every
  surface entry and results vtable unchanged. These checks use
  DONT_RESOLVE_DLL_REFERENCES and execute no game/graphics callbacks.
- Package entries were compared byte-for-byte with the built DLL, repository
  default TOML, diagnostic image and license notices. MinHook's bundled C
  library license is included alongside its Rust wrapper license.

No real-game validation of these four surfaces has been performed. In-game
crop/origin, fades, repeated transitions, device reset, static/animated choices,
1P/2P/DP/arena/battle layouts and actual plugin coexistence remain to be tested.
The prior IIDX 33 results-screen user confirmation does not cover these changes.

Outputs (no release tag changed):

- `dist/iidxbg.dll`: SHA-256 `22c526dff4fbf9998cb45f3de4d3f575ee00ec40127a0daca1be12eb28e40878`
- `dist/iidxbg.toml`: repository default with all four commented switches
- `dist/iidxbg-surfaces-dev-windows-x64.zip`: SHA-256 `522a500dc6349469fb64ba93c98235532d469e0219c2e85048608cd2a2219a24`

## Independent sources, retention and blurred contain ? 2026-10-21 (unreleased)

The canonical section is now `[moebooru]`, including nested per-surface overrides.
Legacy API-section names and the legacy source-mode alias are rejected, as an
explicitly requested breaking configuration change. Five worker-owned providers
have independent sources, repeat histories and output canvases. Configuration
reloads before each surface query; invalid reloads retain the last valid bank.
Query completions publish immediately and failures retain that surface's pixels.

Download retention is opt-in (`max_images = 0` by default). It removes oldest
managed regular files by modification time in the configured save directory,
nonrecursively, and leaves unrelated files alone. No user image directory was
pruned during this work; deletion tests used isolated generated temporary files.
The contain renderer produces a sharp centered foreground over a blurred,
stretched backdrop, computed entirely on the worker.

Validation:

- All 21 tests passed. New coverage includes independent folders and pixel
  identities for all five surfaces, atomic config rejection, failure isolation,
  independent overrides/inheritance, removal of old section/source names,
  zero/exact/over-limit cleanup and oldest-first ordering across both cache
  namespaces while preserving unrelated files and nested data.
- Contain tests verify sharp foreground pixels, softened nonblack margins,
  channel order and source immutability. An optimized CLI preview was visually
  inspected at `target/contain-preview/preview.png` (ignored development output).
- Formatting, Clippy with warnings denied, optimized DLL/tool build and SDK/load
  smoke test passed. Hook attachment passed on private mappings of both exact
  IIDX builds; no game callbacks were executed by those tests.
- Every development ZIP entry matches its source file. The bundled TOML has
  `[moebooru]`, all commented per-surface examples and `max_images = 0`.

No native addresses/ABIs changed. Actual in-game rendering and device-reset
validation remain pending; no new real-game test or remote download was claimed.

Current outputs:

- `dist/iidxbg.dll`: SHA-256 `d06773ae2f3fc4271b0acc5475706a003dd48a8e89118dffe34b4b548549a809`
- `dist/iidxbg.toml`: updated canonical config and all option comments
- `dist/iidxbg-surfaces-dev-windows-x64.zip`: SHA-256 `e9c2493365edc70cb4d8f773beacd870de8d5be36ac76ab7729dd4ad4403e513`

## Default image directories (2026-10-21)

Changed the shipped local folder to `iidxbg` and download folder to
`iidxbg/downloads`, relative to the TOML file. Updated per-surface examples,
installation instructions, path tests and package image layout. Existing
custom paths remain configurable; default recursion remains disabled.

All 21 tests, Clippy with warnings denied and the optimized release build passed.
The development ZIP was checked entry-by-entry against its source files and
contains `iidxbg/test-card.png` plus the updated config. No in-game testing was
performed for this configuration/package change.

- `dist/iidxbg.dll`: SHA-256 `eccba9881753db74ebdf3dded2aebc37b24864fb0c2b54ab2a55bf895e9a4e23`
- `dist/iidxbg-surfaces-dev-windows-x64.zip`: SHA-256 `09d3344331bde8395f1453b11f03b482c5aaac184adcaaad99eec245fe68b93c`

## Active default surface overrides (2026-10-21)

The shipped TOML now enables all five source override sections, each using
local mode, cover fit and separate local/download folders under `iidxbg` and
`iidxbg/downloads`. API domain/tags/retention inherit global settings unless
overridden. Additional surface enable switches remain separate and opt-in.
The package supplies a generated test card in each configured local folder.

All 21 tests, Clippy with warnings denied and the release build passed.
Existing config tests now check every resolved default folder. The ZIP was
verified against source files and all ten source/API sections were checked.
No native code changed and no in-game testing was performed.

- `dist/iidxbg.dll`: SHA-256 `f3d8b2d771f837d528e774b51f653b6afa9789d53321efada92d53bbd244ca7e`
- `dist/iidxbg-surfaces-dev-windows-x64.zip`: SHA-256 `1f2c920a7d2ae19b76cac54204159d875d6c721e4b6e5ba9c8c532b29f06f24b`

## Default contain fit (2026-10-21)

The global image fit and all five active surface overrides now default to
`contain`, preserving the sharp foreground with blurred background fill.
All 21 tests, Clippy with warnings denied and release build passed. Updated
`dist/iidxbg.toml` and the development ZIP; verified all six fit settings and
archive entries against their source files. The release DLL is unchanged.
No in-game testing was performed.

- Development ZIP SHA-256: `e07ed272931d5cd6a77decbb347544797ea6d9e64a0576fc0f43bc30a4f9d812`

## Center-crop defaults for narrow surfaces (2026-10-21)

SUDDEN+, LIFT and target graph now default to `cover`, using the existing
aspect-preserving resize-to-fill and center-crop renderer. Global, results and
song-selection defaults remain `contain`. Every surface still accepts either
mode, including contain for manually cropped local images. No renderer or
native code changed. Updated config comments and configuration documentation.

All 21 tests, Clippy with warnings denied and release build passed. Refreshed
the dist config and development ZIP; verified the fit settings per surface and
archive bytes against source files. The release DLL is unchanged. No in-game
testing was performed.

- Development ZIP SHA-256: `647e388df95f117675e082f39c5389c3592a4d6054f346df249bd8621e8d3cfd`

## Installed IIDX33 check (2026-10-21)

Read-only inspection of `\\2dx.lan.sh1mar.in\IIDX33\modules`:

- Installed DLL SHA-256: `d06773ae2f3fc4271b0acc5475706a003dd48a8e89118dffe34b4b548549a809`, matching the recorded independent-source/blur/retention build. Current local rebuild: `f3d8b2d771f837d528e774b51f653b6afa9789d53321efada92d53bbd244ca7e`. Subsequent recorded changes concern defaults, packaging, docs and tests; hashes are not identical.
- Installed TOML SHA-256: `cb1d7c1f1d0813cddae14cf212803a2e6c7da3a8c505be29618e14ed96b5a5bc`. Parsed using `pip._vendor.tomli`; manually checked against the current config fields.
- All five surfaces enabled, remote mode inherited from the root, domain `konachan.net`, tags `rating:safe`, shared `iidxbg/downloads`, retention disabled (`max_images = 0`). Results/selection inherit contain; SUDDEN+/LIFT/graph explicitly select cover. Local directory `iidxbg` and download directory exist; 40 download files present at inspection.
- Existing game log records loading and SDK registration of iidxbg; plugin log records exact IIDX33 attachment, per-surface preparation and result/selection/cover/graph upload readiness. Earlier duplicate-key reload rejections are followed by a successful configuration reload.
- Config last-write time 02:45:29 is later than plugin log last-write time 02:43:54 on October 21. These logs do not prove that the latest saved config has been consumed. Recheck for a subsequent successful reload/preparation after the next query.

No installed files were modified. Inspection of existing logs is not direct visual validation of rendering or evidence that the latest config was applied.


## Live selection frame character (2026-10-21)

Implemented live native visibility and optional local static image replacement
for the exact IIDX33/34 profiles. A separate 500 ms worker watches config and
image metadata independently of HTTP preparation. Transparent PNG alpha is
preserved on a 1920x1080 canvas. New build-specific evidence is in
[iidx33-frame-chara.md](iidx33-frame-chara.md) and
[iidx34-frame-chara.md](iidx34-frame-chara.md).

All 26 Rust tests passed, including live config edits/invalid-edit retention,
image reload/clear/alpha, mask restoration/retired-owner rejection and mocked
native geometry/color calls. Formatting, Clippy with warnings denied and the
optimized release build passed. `verify-frame-chara.py` passed both exact DLLs,
including native visibility/dimension emulation with mocked AFP imports.
The built DLL passed LoadLibrary/export/SDK smoke checks and private-process
native attachment for both builds. These do not execute the game; no in-game
visual, transition or reset validation was performed for this feature.

Cleaned dist by moving its old contents to `target/dist-before-frame-chara`
and publishing fresh DLL, repository-default TOML, licenses and five generated
test cards. The development ZIP contains exactly those nine files and each
entry was compared byte-for-byte with the clean dist source. Installed game
files were not modified.

- `dist/iidxbg.dll`: SHA-256 `cc250ff76331d55c8a47b1db00254c4742ee6210a31b4b7e4ef536c5db6bef8e`.

- `dist/iidxbg-surfaces-dev-windows-x64.zip`: SHA-256 `2b94f41f7b17a993fe501005bad3ae358a4abfd3b8b49913bb0841702faab530`.


## Scene-driven reload revision (2026-10-21)

Removed the separate 500 ms file watcher. Selection/result entry and exit and
cover/graph retirement increment the shared request generation and wake the
single image worker. Config reload and character file checks happen when it
handles that request, or an existing deferred/retry request. One accepted
snapshot governs a preparation batch. There is no idle filesystem polling;
the worker parks between notifications, with timeout only for pending work.
Background success/failure backoff and active-visit freezing remain intact.
Selection exit is used as a lifecycle trigger, not claimed to be a verified
song-confirmation hook. Original native callbacks still run and no new native
address or detour was introduced.

Corrected image guidance: 1920x1080 is the internal AFP movie canvas, while the
inspected IIDX33 character artwork is 700x800 (702x802 padded texture). Input
files of other valid dimensions were already accepted. Whole-layer replacement
still does not reconstruct the internal stock child-art placement.

All 26 tests passed, including request-driven config acceptance/invalid-edit
retention and file reload checks. Fixed the omitted-section test to remove the
parsed TOML table instead of splitting at a token that can occur in comments.
Clippy with warnings denied passed. No game execution or visual validation.

Optimized release build, formatting, DLL loading/SDK smoke and private native
attachment for both exact builds passed. Refreshed dist DLL/TOML and verified
all nine ZIP entries byte-for-byte. No installed game files were changed.

- `dist/iidxbg.dll` SHA-256: `09d5e9165c10937b3c2598907eea682be1a817b26c809994d771e700ecb3c651`.

- `dist/iidxbg-surfaces-dev-windows-x64.zip` SHA-256: `90758d1251ef63b2dc3878e0a47638b1fc467713fc5dfc5f6f409d7455fe748b`.


## Version 0.2.0 release preparation (2026-10-21)

Bumped Cargo manifest/lockfile to 0.2.0 and updated current user documentation
for this release. Included independent surface sources, Moebooru configuration,
download retention, contain blur/crop defaults, selection/cover/graph adapters,
and scene-driven frame-character controls with build-specific evidence.
All 26 tests and Clippy with warnings denied passed. Binary analysis inputs,
tokens, local images and dist artifacts remain ignored and are not committed.
The tag workflow builds the release ZIP from repository defaults. New native
surfaces/character behavior still lack real-game validation on either build.

Release build, formatting, DLL loading and private-process attachment checks
for both exact builds passed. Refreshed dist DLL/default TOML and generated
`iidxbg-0.2.0-windows-x64.zip`; verified all nine entries byte-for-byte.

- `dist/iidxbg.dll` SHA-256: `6e4fb3f571267c2167565ad291cbe91216792b978c1d440ac9061359103c3050`.

- `dist/iidxbg-0.2.0-windows-x64.zip` SHA-256: `034813bc1f769fa2f64d057db6248ceff962de954b8243174979715392b60d84`.

## IIDX 34 / 010-0917 support (2026-10-22)

Added a third exact-build profile for SHA-256
`c98e183ec81ecb9e1cb13c237f9c9cb0b3f65b6fdaff6fd5e6a526008c546534`
(18,200,216 bytes; PE timestamp 2026-09-17). Input analysis used local IDA
Professional 9.4/Hex-Rays, PE exception entries, RTTI, Capstone and Unicorn;
see [the evidence record](iidx34-010-0917-analysis.md).

- All 26 Rust tests passed, including the new profile in guard completeness,
  hook-chain recognition and surface/frame-character tests.
- Clippy for all targets with warnings denied and `cargo fmt --all --check` passed.
- Optimized release build succeeded. `iidxbg-tool verify` accepted all three
  profiles and rejected unknown hashes.
- `verify-native.py`, `verify-surfaces.py`, `verify-frame-chara.py` and
  `verify-origin.py` passed on the new DLL. The native verifier checked every
  embedded guard, all three hook slots, RTTI, scene/background layout, binding
  strings, texture lifecycle and AFP ordinal 137; the origin emulator covered
  1920x1080, 1280x720 and 1921x1081 centers.
- Private-process attachment passed in `native`, `chain` and `bad-tail` modes for
  the new profile; the earlier IIDX 34 `native` mode still passed.
- No game process was started and no in-game rendering was validated.

Local `target/release/iidxbg.dll` SHA-256:
`4020f55a80896c57a50b732ef6ed58a570be80d73f8dc3dafefa7e102c799eea`.
The `dist` release artifacts were not refreshed by this change.

## Premium-area and concentration backgrounds (2026-10-22)

Added two opt-in replacement targets for the IIDX 34 / 010-0917 profile:
`surfaces.premium_area` and `surfaces.concentration`. Both use a 1280x720
canvas and copy the native AFP transform; concentration reuses the prepared
premium-area image while `[sources.concentration]` stays empty. Analysis is
recorded in [the 010-0917 evidence record](iidx34-010-0917-analysis.md#premium-area-and-concentration-backgrounds-2026-10-22).

- 27 Rust tests passed, including a new native-transform copy test and the
  shipped-TOML sharing assertions. Clippy for all targets with warnings denied
  and `cargo fmt --all --check` passed.
- Optimized release build succeeded. `iidxbg-tool verify` accepted both
  IIDX 34 profiles.
- `verify-native.py` passed and now checks 38 embedded guards, the
  `CPremiumArea +72` window, `CStageMain::Init` apply/show call sites, the
  premium background construction path, the concentration layer lookup, the
  native `{0,1080,1280,720}` clip and the three background strings.
- `verify-surfaces.py` passed with the new premium/concentration anchors and
  clip; `verify-frame-chara.py` and `verify-origin.py` still passed on the same
  DLL.
- Private-process attachment passed in `native`, `chain` and `bad-tail` modes
  for the new profile, and in `native` mode for IIDX 34 / 20260917, which does
  not install the two new hooks.
- No game process was started and no in-game rendering was validated. The AFP
  group, draw priority and the no-customization premium background path remain
  unconfirmed.

Release `target/release/iidxbg.dll` SHA-256:
`46c1926494136e5808c5473f7313170222ab25c56785b9e0956d8d0241e9ab76`.
The `dist` release artifacts were not refreshed by this change.

### In-game feedback round 1 (2026-10-22)

The user ran the plugin in the exact IIDX 34 / 010-0917 build. The premium-area
image displayed correctly. Two defects appeared:

- Concentration still showed stock art, and an owned sprite appeared far to the
  right. The copied `position` is relative to the concentration window layer,
  not the screen, so an owned sprite cannot be placed without the parent
  transform.
- The session-global premium nodes stayed hidden after the scene sprite pool
  retired the replacement sprite, so the premium area was black on the results
  screen.

Fixes:

- Concentration now rebinds the native `concentration_bg_set/bg` clip in both
  window elements to the prepared texture, using the same dynamic-descriptor
  setter as the game. The clip's transform/layer/draw order stay native. The
  saved archive/name are restored through the native archive setter on hide,
  disable and device invalidation.
- Premium restores native visibility in the common sprite-release observer and
  on selection exit, before the session-global nodes can be seen without their
  replacement.
- Diagnostic log lines record the premium node handles and both concentration
  element lookups for the next round. Startup also logs the loaded DLL's own
  SHA-256 (`iidxbg build SHA-256 ...`), so a stale deployment is immediately
  visible in the next report.

Re-verified offline after the fixes: 27 tests, clippy, `cargo fmt --check`,
release build, all four verifiers and private-process attachment in `native`,
`chain` and `bad-tail` modes. No automated in-game run is possible here; the
next visual check is with the user.

Release `target/release/iidxbg.dll` SHA-256:
`cf26dbaf86e35570d294cdad4d7f7060e8cd2f8efbb76db0e79d38ae1f4d708c`.
The `dist` release artifacts were not refreshed by this change.

### In-game feedback round 2 (2026-10-22)

The user deployed the build above (confirmed by its startup hash line). Premium
selection displayed the custom image; gameplay fell back to stock, and
concentration still showed stock. The log added these facts:

- The premium sprite was created, released once by the native sprite pool, then
  recreated eight times inside one preparation batch. The release observer
  restored native visibility (no black area), and the repeated rebuilds came
  from comparing the whole `Bundle` instead of the prepared image.
- `concentration background clips: element0=0x0 element1=0x18c3de690`: the
  default window element has no `concentration_bg_set/bg`; only element 1 does.
  That clip has no restorable archive/name.
- Leaving selection restored the premium native nodes, which is why gameplay
  showed stock art again.

Changes:

- Premium keeps its replacement across scenes. Only an actual sprite-pool
  release restores native visibility. Both the base layer and the customization
  sprite are hidden while our sprite is active, so a later native show cannot
  replace it.
- Concentration resolves both window elements and falls back to the element
  layer root clip when the named background clip is missing. On every native
  show it logs the customize index plus both element visibilities, so the next
  report identifies the visible element.
- Image-identity comparison replaces whole-bundle comparison, so a publish
  batch no longer recreates the premium sprite seven times.

Offline re-verification: 27 tests, clippy, `cargo fmt --check` and the release
build passed.

Release `target/release/iidxbg.dll` SHA-256:
`7567615abebc15ef89bc0388321d021a89f07b1d351424a51c78b4de4d534f1d`.
The `dist` release artifacts were not refreshed by this change.

### In-game feedback round 3 (2026-10-22)

The round-2 build was confirmed by its startup hash. New observations:

- Premium still reverted in gameplay: the startup log shows the sprite being
  released by the native pool at scene transitions, and the old code restored
  the native nodes on selection exit. The replacement is now kept across
  scenes; only a real pool release restores native visibility, and both native
  nodes are hidden while our sprite is active.
- `concentration background: index=112 ... name=...`: the customize index is
  nonzero, element 0 has no `concentration_bg_set/bg`, element 1 does, and both
  element layers report visibility 1, so visibility alone cannot identify the
  displayed element. Binding both named clips had no visible effect.
- The customize preview code builds `/data/graphic/customize/%s.jpg` from
  `subbg0_concentration`, and `data/graphic/customize/subbg0_concentration.jpg`
  exists in the game data. The background probe now also tries the
  `subbg0_concentration` clip name in both elements before falling back to the
  layer root.
- Each bind now logs the clip handle, its AFP node id, node visibility and
  whether a native archive/name could be saved.

Offline re-verification: 27 tests, clippy, `cargo fmt --check` and the release
build passed.

Release `target/release/iidxbg.dll` SHA-256:
`736af90aca92b8430e32a0c610b1745c9e794250ab7dc62aff4fe4c204ec7666`.
The `dist` release artifacts were not refreshed by this change.

### In-game feedback round 4 (2026-10-22)

The round-3 build was confirmed by its startup hash; the visible behaviour did
not change. The bind diagnostics added in round 3 gave the decisive data:

- `concentration bind clip=0x18c3de690 node=0xffffffee` - the named
  `concentration_bg_set/bg` clip has **no AFP node**, so the game never bound a
  background there and our setters silently do nothing.
- `element0 ... name=0 fallback=true` - neither `concentration_bg_set/bg` nor
  `subbg0_concentration` exists in the default window element.
- The premium sprite is still released by the native pool at scene changes
  (three release lines), so it reverted after song selection.

Changes:

- Concentration now re-resolves the window elements **after** the native show
  callback, when the clip nodes exist, instead of caching entries found while
  the window was hidden. A clip without an AFP node is rejected instead of
  bound silently.
- The premium replacement is now also ensured from the movie dispatcher, so
  the session-global premium background is recreated after the pool release
  during loading/gameplay.

Offline re-verification: 27 tests, clippy, `cargo fmt --check` and the release
build passed.

Release `target/release/iidxbg.dll` SHA-256:
`14c1c8098fddd74a63c07c5fc2b16d6346304bec164d7993b84f55705c88fea6`.
The `dist` release artifacts were not refreshed by this change.

### In-game feedback round 5 (2026-10-22)

The user described the actual concentration view: a **full-screen static image
on the secondary display** plus a small "CONCENTRATION MODE" line. That is the
sub-monitor presentation, like the premium background, not a texture inside the
named customization clip. The bind diagnostics agree: the named clip has no AFP
node and element 0 has no background clip at all.

The concentration replacement now uses the same mechanism as premium: create an
owned sprite at the window priority (6, the id passed to the layer factory),
copy the position/origin/scale/color of the window element layer, hide both
native elements and show the sprite. It is recreated on every native show and
removed on hide, disable or device loss.

Known trade-off: if the "CONCENTRATION MODE" line is part of the hidden element
layer it disappears with the native image; the next in-game check must confirm
whether the line is a separate node. The premium replacement is also recreated
from the movie dispatcher after the pool release, so it should survive song
loading/gameplay.

Offline re-verification: 27 tests, clippy, `cargo fmt --check` and the release
build passed.

Release `target/release/iidxbg.dll` SHA-256:
`3c2d37572023387e7394b2938693b0e755c2bd5f95844339821a00d2e7a20dd7`.
The `dist` release artifacts were not refreshed by this change.
