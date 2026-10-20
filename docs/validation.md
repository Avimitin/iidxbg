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
