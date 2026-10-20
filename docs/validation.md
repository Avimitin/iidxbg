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
