# 0.5.0

Adds the All Assets catalog with an opt-in, built-in IFS scanner, cache location
and open-folder control. Supports named artwork, geometry fills, loose images
and the main gameplay movie on the inspected IIDX34 010-0917 build. Adds note,
shell, decision, transition, title, login, mode selection, bottom frame and
logout presets. The configuration gallery keeps an adjacent, unequal-size
scrolling mosaic. Text and 3D model behavior remains native.

Fixes Premium Area source-lifetime tracking: retiring a native source invalidates
saved handles and prevents rebuilding from recycled pool objects. No game assets
or DLLs are modified. Native adapters remain exact-hash guarded.

Actual in-game testing covered the artwork work and five new scene presets,
including mode-selection video and logout; see the dated analysis documents.
The idle-demo overlay was reproduced, and its fix passed tests, Clippy and a
release build. In-game verification of that fix was interrupted and remains
outstanding. DP/P2, card-summary variants and induced device-loss checks remain
incomplete. Catalog coverage is not proof of every asset in every game mode.
