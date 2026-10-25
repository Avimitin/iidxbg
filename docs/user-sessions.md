# Users and login sessions

Implemented 2026-10-25. A stable user key owns settings; card IDs are a list of
bindings, never the user key. New users start with every replacement off. An
unbound card, guest, incomplete login or failed memory read gets stock user
modules. With two joined players, both must resolve to the same user; mixed
users, an unbound card or a guest make user modules stock. Shared game asset
replacements remain active regardless of login.

## Configuration and UI

The bottom-left user icon opens a compact scrollable menu. Its first row creates
a user by name with the plus button (or Enter); the search below matches names,
stable IDs and card numbers. The current user is green and sorted first. Clicking
a row switches users while retaining the current page; the row's settings button
opens that user's **用户与卡片** page, containing rename/delete, card bindings,
live P1/P2 capture and Bilibili login. The old import action is removed. Original
top-level settings and comments remain intact; new users start disabled.

The selected user is marked in the switcher and identified in settings editors.
The module page is a grid without a user header. Startup selects the
first user by name, or shows a create-user empty state. Reload retains a valid
selection; deleting the selection or externally removing it chooses the next
available user, or returns to the empty state. User selection in the editor does
not select the player in the game; bound cards determine runtime settings.
Card IDs accept 16 hexadecimal digits, normalize to uppercase on binding and
must be unique across all users. Zero and all-ones reader sentinels are rejected.

The top-bar **游戏素材** button opens the shared artwork page with one click.
Title and login backgrounds use the same preview cards and editor as display
modules, pinned above the catalog. The page has no user header or management
shortcut and works without any users. Closing an editor, reloading or switching
users keeps this page in the shared scope. All buttons have clear backgrounds;
text color and borders communicate hover and selection.
Display components have small, light white labels with black outlines and shadows.
Module hover popups are removed; the sidebar uses green icons without background blocks.
Module editors start on **设置**; **媒体来源** appears only for **单独设置**.
Relevant API/download/model controls are part of **设置**, with help in tooltips.
Premium Area, concentration, song selection, Qpro, large character, SUDDEN+,
LIFT, graph, results, mode selection and gameplay/transition/logout modules
belong to the selected user. Direct catalog replacements belong to the
installation, stored in top-level `[[artwork]]`. MMD placements share that user's
model settings. Each user binds their own Bilibili account. Login, logout,
favourites, previews, video and live sources follow that user; switching cancels
pending QR login and clears the previous account's visible data. Deleting a user
also removes their local binding. Sessions are DPAPI-encrypted and keyed by the
canonical configuration path plus stable user ID, outside TOML. Renaming a user
or changing cards preserves the binding; moving the config requires another scan.
Shared pre-login modules and direct replacements are anonymous. Legacy machine-wide Bilibili sessions are
never inherited; existing users must bind again. Video caches are separated too.

```toml
[pre_login.title]
enabled = true
[pre_login.title.source]
source = "file"
media_path = 'iidxbg/title.png'

[pre_login.login]
enabled = false

[users.player_one]
name = "Player one"
cards = ["E004010000000001", "E004010000000002"]

[users.player_one.settings.surfaces]
results = true
song_select = true

[users.player_one.settings.sources.results]
source = "local"
local_directory = 'iidxbg/player-one/results'

[users.player_one.settings.selection]
frame_chara = true
frame_chara_source = "stock"
```

Global provider/decode defaults remain shared, as does the legacy master switch.
Direct replacements resolve sources from a retained global configuration snapshot,
including global bindings; they never inherit a user's source or Bilibili identity.
Generated display-module presets still follow the selected user. Legacy artwork
under user settings is promoted to top-level artwork on load and written on the
next UI edit. Identical entries are deduplicated; conflicting definitions of the
same archive/texture reject the configuration instead of choosing a user silently.
Global module switches, character and MMD settings do **not** activate user
replacements automatically. Legacy title/login settings are used only when the
whole `pre_login` section is absent. User/pre-login bindings cannot cross scopes.
Omitted user switches, including results in a partial surfaces table, are off.
All relative paths resolve beside the TOML. GUI previews use an owned snapshot
of the selected user's settings; changing users invalidates queued previews.

After logging into the game, open or switch to the config EXE. Both player slots
appear in **用户与卡片 → 当前游戏登录**. Choose a user and use **绑定到当前用户**, or enter a
card manually. Binding persists in TOML; the EXE need not be open during login.
The plugin atomically writes `iidxbg.session.json` beside its configuration.
This local file contains card IDs, joined flags, process ID and heartbeat time;
it contains no PIN or refid. The UI rejects a heartbeat older than five seconds.
The file is ignored by Git and is not a distributable configuration file.

## Runtime and ownership

A separate observer reads bounded static globals with `ReadProcessMemory` every
100 ms, taking two matching samples. It writes the status file every second
and on changes, and checks config modification time once a second. All this IO
is off game/graphics callbacks and independent of slow image downloads.

An identity change increments a context generation and wakes the image worker.
Rendering stays gated until the graphics owner has restored stock and retired
old textures/sprites, and the worker has cleared old pending pixels, provider
history, character/model publication and frozen choices. Rebinding a current
card to a different user also changes context. Late old requests cannot become
the next user's fallback. Invalid config reloads retain the last accepted
document, but resolve it against the current login rather than retaining the
previous user's effective settings. Provider setup failure on a context change
fails closed. Native shutdown/fault latches remain separate.

Ordinary edits for the same user do not reset a result visit: it retains its
one frozen choice. Results and song-selection background changes await a new
visit after preparation; character edits retain their existing live behavior.
The observer never calls a game function or retains a native object pointer.
Native original calls, exact SHA-256/live entry guards and device resets remain.

## Native evidence

Inputs inspected read-only; image base `0x180000000`. All addresses below are
RVAs. The user's `D:\Share\tmp\010-20260916.dll` is the **c98e…6534** binary;
the filename does not select compatibility.

| Profile | SHA-256 | Input | Image size |
|---|---|---|---|
| 33 / 010-20260819 | `c61b6dcb8894062e56d60da8ca90053b27f129e1a8e8da5e54457aa42602397d` | `D:\Share\chart-requester\analysis\bm2dx.dll` | `0xbb95000` |
| 34 / 010-20260916 | `3755587f44785effe5008f74cfe7dccbdf423ad09feead48e00a90f78a42fe83` | `D:\Share\2dx-gsm\analysis\bm2dx-2026091700-34-010.dll` | `0xc5ff000` |
| 34 / 010-20260917 | `c98e183ec81ecb9e1cb13c237f9c9cb0b3f65b6fdaff6fd5e6a526008c546534` | User-supplied path above | `0xc5ff000` |

| Item | 33 | 34 / 3755… | 34 / c98e… |
|---|---|---|---|
| initialized flags (two u32) | `0x10b90e8` | `0x107c0e8` | `0x107c0e8` |
| joined flags (two u32) | `0xacd79b0` | `0xb60b3b0` | `0xb60b3b0` |
| side-zero UID/session block | `0x6c11808` | `0x70f2e00` | `0x70f2e00` |
| player ready byte | `0x312771e` | `0x32b34d8` | `0x32b34d8` |
| side stride | `0x3b103a0` | `0x3e66db8` | `0x3e66db8` |
| initialization validator | `0x5c4480` | `0x5b6770` | `0x5b6790` |
| reader UID setter | `0x5ad900` | `0x59ec80` | `0x59ec80` |
| UID formatter wrapper | `0x5a0210` | `0x591020` | `0x591020` |
| numeric hex formatter | `0xab7060` | `0xaa1e20` | `0xaa1e60` |
| profile clear | `0x5abed0` | `0x59d220` | `0x59d220` |
| ready/type consumer | `0x5ad8a0` | `0x59ebc0` | `0x59ebc0` |
| participation getter / clear | `0x9493e0` / `0x949680` | `0x92fcc0` / `0x92ff60` | `0x92fd00` / `0x92ffa0` |

IDA/Hex-Rays on the 33 and supplied c98e inputs and Capstone on all three
establish the independent layouts. The UID setter takes side in ECX, reader
bytes in RDX, play kind in R8D and auxiliary text in R9. It reverses eight
reader bytes into the static u64. Formatter wrappers take side in ECX and
caller-owned output in RDX; the formatter emits 16 uppercase hex digits and a
NUL from the most significant nibble. Runtime reproduces that formatting on
an owned Rust u64; none of these functions are invoked or detoured.

The type at identity +24 is set by the UID setter; kind 1 clears the UID for
guest play. Ready consumers reject guest kind and a zero profile-ready byte.
The runtime conservatively accepts only kind 0 and ready ==1. The player clear
zeroes a whole per-side profile block including the ready byte. Participation
getters check side bounds and the joined u32; their clear functions write zero.
On c98e, card-entry state `0x8a78e0` obtains reader output and calls the setter
at `0x8a7f6e`; cancellation state `0x8a7470` calls participation clear and
resets card state. These globals have module lifetime; there is no borrowed
card-reader buffer across callbacks. Static reads are checked and copied twice.

## Reproduction and limits

Build the tool, then for each exact input run:

```powershell
py scripts/verify-native.py path/to/bm2dx.dll
py scripts/verify-sessions.py path/to/bm2dx.dll
py scripts/smoke-attach.py target/release/iidxbg.dll path/to/bm2dx.dll --session
```

The mapped smoke-test input must be named `bm2dx.dll`, matching the game's
module lookup. For a differently named input, use a temporary hardlink named
`bm2dx.dll` outside the repository; remove the link after the test. This does
not change the original file's bytes.

The session verifier checks instruction operands, globals, stride, UID copy
order, ready/type gates, clear bounds and new guards. The smoke test privately
maps the DLL without running game code, writes synthetic session globals and
checks the plugin's actual observer/status output through card changes, guest,
P2 and logout. Unit tests cover malformed/torn reads, binding uniqueness,
unconfigured fallback, two-player conflicts, path resolution, scoped saves and
legacy migration. Existing visit-freeze tests remain applicable.

Validation on 2026-10-25: core tests 82 passed / 7 ignored; UI tests 29 passed /
1 ignored. Both crates passed clippy with warnings denied and release builds.
Native guards and synthetic login transitions passed for all three hashes.
The IIDX 33 preserved-hook-chain and damaged-tail rejection smoke checks passed.
The UI was checked with a temporary configuration: create a user, bind the
captured card, manually add a second card, switch between user/pre-login pages,
and reject an expired heartbeat. Synthetic cards were used; the existing
local `iidxbg.toml` was left unchanged.

Shared-assets follow-up: core 87 passed / 7 ignored, UI 36 passed / 1 ignored;
Clippy with warnings denied and release builds passed. Added checks cover global
sources for users/guests, disabled bound sources, isolation from personal prepared
media, pre-login bindings, migration conflicts, empty/inline artwork arrays and
format-preserving global saves. Desktop checks covered single-click assets
navigation, the restored title/login cards, scope retention across user switches
and editor dismissal, global mask/binding saves and transparent button states.

On 2026-10-26 the user reported that the deployed build tested OK; the exact
scenarios were not specified. No additional agent-run in-game testing was performed. Native
login/cancel/logout timing, simultaneous physical card reads, session changes
while a slow download is running and GPU reset during such changes still need
live validation. Static/synthetic checks do not establish those behaviors.
