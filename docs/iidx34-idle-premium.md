# Idle demo Premium Area lifetime regression (2026-10-24)

Input: IIDX34 `010-0917bm2dx.dll`, SHA-256
`c98e183ec81ecb9e1cb13c237f9c9cb0b3f65b6fdaff6fd5e6a526008c546534`.
All addresses below are RVAs. No game DLL/archive writes.

## Reproduction

With Premium Area replacement enabled, leave the machine without logging in.
After attract-mode transitions into eco demonstration gameplay, a 1280x720
replacement can appear at main-screen (0,0), covering the left playfield.
Reproduced on the deployed `616952978ac221c1ccc60c9da393ee62d56ff50b2cbcc96a1c3e2b5f94ebb50c`
plugin; private capture `analysis/idle-premium-20261024/before-demo.jpg`.
The first demo frame need not contain the overlay; it can appear later when
Premium Area refresh rebuilds the sprite.

## Native lifetime evidence

- PremiumAreaBG constructor `0x6447a0`: Win64 `(this, i32 priority,
  u8 customize, shared_ptr*) -> this`; creates base at +16 and custom at +24.
- PremiumAreaBG teardown `0x644920`: unary `(this) -> machine word`, invokes
  each nonnull node's vtable +32, then zeros +24/+16. Called by `0x99a2c0`.
- Layer vtable `0xd968b0`, slot +32 -> `0x32c7d0`. This releases the AFP layer,
  then calls common sprite release `0x3363e0` before resetting layer metadata.
- Existing hooked `0x3363e0`: unary void `(this)`; removes native node,
  zeros +8, invalidates clip at +528 to -18, and clears state at +552..+599.
  Thus a retained address is not a retained lifetime; pool reuse can produce a
  valid but unrelated transform at the same address.
- Subscreen layer builder `0x9c1e30` establishes (0,1080), clipping rectangle
  {0,1080,1280,720}, and transform flag 0x80000001. The unintended (0,0) sprite
  is inconsistent with that native construction.

The old observer handled retirement of the plugin sprite but not its native
base/custom sources. It kept their addresses and saved clip handles, allowing
refresh to rebuild from recycled state and potentially restore visibility into
an unrelated clip.

## Change

The existing release observer now invalidates both source references and saved
visibility handles when either source retires. It performs metadata work only.
The next graphics-owner refresh disposes replacement GPU state and refuses to
rebuild until a fresh PremiumAreaBG constructor is observed. Constructor events
establish a new lifetime even when the owner address repeats. Retirement of only
the plugin sprite retains the existing rebuild behavior for a live source.
No new offsets, hooks, ABI assumptions, provider IO or decoding callbacks.

A regression test checks unrelated releases, source retirement, invalidation of
saved handles and retaining the owner marker needed for graphics cleanup.

## Verification

Core tests: 60 passed, 5 ignored. Clippy and release build passed.
Fix build `b5561cab850b0971f13624461467e5bcd0cbb4edad6878d33d34ef2d24847db0`
was copied to the game machine. In-game regression verification was interrupted
before the new build was observed running; the old-build reproduction is verified,
but the fix's visual outcome remains unverified. Release 0.5.0 includes this
limitation. No remote configuration was changed for this regression. Custom-native-background mode, induced GPU
reset, and other exact game hashes remain unverified in this regression run.
