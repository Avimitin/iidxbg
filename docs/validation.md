# Validation record — 2026-10-20

Scope: Windows x64 standalone plugin, known IIDX 33 / 20260812 and
IIDX 34 / 20260917 profiles. No running game was used.

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

Outstanding: real-game attachment, visual geometry/cropping, score UI,
scene lifecycle, device reset/shutdown, and frame-time measurements on both
versions. Unit tests and static verification cannot establish those properties.
