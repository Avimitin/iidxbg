# Title, login, mode, lower frame and logout presets

Analysis recorded 2026-10-24. Game input: `010-0917bm2dx.dll`, SHA-256
`c98e183ec81ecb9e1cb13c237f9c9cb0b3f65b6fdaff6fd5e6a526008c546534`.
Addresses are RVAs relative to image base `0x180000000`.

Private input archives were copied previously to the local analysis directory;
the game directory was not unpacked or modified:

| Archive | SHA-256 |
| --- | --- |
| 0/basic_frame.ifs | a4c27d80e53d21391c90db8bf80b8e89c50e5b14d2f81315ff1d70bb6e228c57 |
| 0/card.ifs | 30750bd21a868a2a40589b0113fa85c1ff67bf6fb9315cbe47e741f5c119583f |
| 0/title.ifs | 2a48640726711f394cd606018050d6a0992254f0085d578e98b8183ff9cef235 |
| gmframe34.ifs | f474bb21e143cc355eb7a89384509bc4228fb21d2a2652a5b6564a7bf5beb9c1 |

## Scene distinction and lifetime

Login setup `0x8a06a0` creates `card_bg` at priority134 and `card_bg2` at136,
then enters CardEntryStatePlayerEntry or CardEntryStateNoCardPlay. Logout setup
`0x8a1fd0` creates those same named layers at the same priorities, then enters
CardOutStatePlayerOut and emits THANK YOU FOR PLAYING. Both use Win64
`usize(this: RCX, argument: RDX)` and forward the original return value.
The setup hooks only establish a thread-local creation scope around the original
call; they do not access scene fields or retain scene pointers.

Existing layer-create `0x32da80` returns a layer owner. The observer tags only
`card_bg2` with login/logout from that scope; `card_bg` also owns the native
heading/panel framing and must stay visible. Basic-frame build
`0x5cf1f0` uses component+44 == 1 for mode selection (the same switch creates
`name_mode`); a scoped wrapper tags its `frame_layer2` only in that case.
The build wrapper preserves its Win64 unary signature and original return.

The existing layer vtable and root-clip field +528 guard each observation.
The sprite-release observer retires owners before forwarding destruction.
A monotonically increasing creation serial prevents pooled address/clip reuse
from selecting an earlier visit's image or restoring its visibility. Metadata
is synchronized with release; callbacks never retain an unowned native texture.

On the graphics owner, the replacement creates one existing `Sprite::media`
background at native priority+1, freezes the prepared media, and hides only the
tagged background layers after the replacement is ready. UI components stay
native. The same descriptor, sprite factory, AFP group0, upload/decoder cache
and release ABI as the existing selection backdrop are reused. Failed allocation
or playback restores stock and is not retried every frame. The owned sprite is
retired on scene change; native pool retirement is observed to prevent double
release. Device invalidation releases GPU state and preserves the chosen CPU
media for reconstruction. Late initial media may be adopted on these backgrounds;
the separate song-result freeze policy is unchanged.

## Resource mappings

`src/presets.rs` expands title and bottom-frame switches into internal artwork entries.
Explicit catalog entries, including disabled entries, override preset mappings.
The preset provenance and bottom crop are internal metadata and are never serialized.

- Title: `0/title.ifs/34title_bg`, 1920x1080.
- Login/logout: independently owned native `card_bg2` background layers.
- Mode: native basic-frame background layer, scoped to mode selection.
- Gameplay bottom: lower300 pixels of unit_main_sp_1p/unit_main_dp_1p, plus
  unit_sp_01, unit_score, unit_spd, unit_groove_gauge and unit_dp_bottom/2/3/4.
  The atlas replacement rectangle is cropped before composition; upper rails
  and the lane opening retain native pixels. Separate numeric/gauge-fill assets
  are not mapped. Native alpha masks remain enabled.

The adapters reuse archive observation `0x33ac50`/`0x33af10`, release `0x33b500`,
frame `0x328b40`, AFP texture information at IAT `0xc66da0` and existing exact
AFP-library shape guards. See iidx34-custom-art.md for wrapper ABI, owned COM
references, queue-copy lifetime and reset behavior. No new decoding or provider
IO occurs on game callbacks. Bound providers prepare once and share owned media;
static atlas composition remains revision-driven, video updates remain frame-driven.
This does not establish zero overhead or a new measured performance result.

## Reproduction and remaining verification

Enable each of title/login/mode_select/gameplay_bottom/logout in `[surfaces]`,
choose fixed media under `[sources.<name>]`, then traverse title, card entry,
mode selection, SP play and card-out. Use different media for login/logout to
check separation. Verify stock fallback by disabling, and verify native labels,
gauge fills and notes remain intact. Repeat with video and capture frame pairs.

First live build `32892165bff4e69615541ca6dc2e40baf715e85ccff1ced9d1ec066b43e26e25`
confirmed bottom-frame replacement with intact keys, gauge fills and score text
(`analysis/scene-presets-20261024/bottom-tutorial.jpg`). Title pattern, card
solid-fill and shared mode-shape mappings did not replace the visible background
in this run, despite successful preparation/lookup; those presets were revised
to the mappings/layer strategy above. Full DP, alternate skins,
device-reset behavior and other exact builds remain unverified. The earlier
guest round reached an early F result (note50/measure9), not a full song; its
screenshots establish previous cover/graph/shell/result behavior only.

Second live build `0f605816f3245fb58644d0b6176bfdab910bc92beaf8a22f17b7dded4eb90b87`
confirmed title artwork behind the logo/character and the mode background behind
native cards/tutorials. Hiding both card roots removed the heading; the final
adapter is narrowed to card_bg2. These observations are in title.jpg, mode.jpg
and login-first-layer-pass.jpg under the private verification directory.


Final live build `616952978ac221c1ccc60c9da393ee62d56ff50b2cbcc96a1c3e2b5f94ebb50c`
confirmed the login image with the native heading, instructions, QR and character
preserved (`login.jpg`). Mode selection video moves in captures three seconds
apart (`mode-video-a.jpg`, `mode-video-b.jpg`), with native cards/header intact.
The lower frame also appears in SP BEGINNER gameplay (`bottom-play.jpg`);
upper rails, native keys, gauge fill and score remain visible. Screenshots are
private under `analysis/scene-presets-20261024`, not repository assets.

The first page now has 16 independent tiles, retaining the adjacent unequal-size
mosaic layout. The new five are opt-in and use existing provider/path/binding
controls. The matching configuration executable was deployed with SHA-256
`fb7010ff877db0e0041109046d1624381cb3c7c750dbfa73d9a8451031f6201e`.

Static title/bottom atlas composition is performed when its source revision
changes; ordinary frames retain the composed texture. The three layer-based
backgrounds allocate an owned native sprite/texture per visit, then reuse it.
They still incur metadata checks and visibility calls, so native-equivalent
frame cost is not established. Video additionally uploads decoded frames through
the existing shared media cache; provider IO/decoding remains on workers. This
run is functional verification, not a controlled CPU/GPU benchmark.


The guest session continued through Caramel Pain BEGINNER and into stage 2.
Repeated test key inputs advanced past the first result before it was captured;
therefore that result's final note count/score is not established. Stage 2 used
?? HYPER 8 and intentionally retired at note50/measure10 to reach card-out.
`stage2-results.jpg` confirms the existing live-video result background still
works. `logout.jpg` confirms the distinct logout image behind the native game-ended
heading, central character and side panels. No stale login/mode image remained.
The original remote TOML was restored byte-for-byte after checking the active
file still matched the temporary test file. Test inputs were released.

Validation: core tests 59 passed/5 ignored; UI tests 22 passed/1 ignored;
core/UI Clippy and release builds passed; both formatting checks and diff check
passed. Actual in-game testing was performed on the exact hash above. DP/P2,
card-account summary variants, alternate skins, induced device loss and other
builds remain unverified. Mode video was checked with moving-frame pairs;
login/logout were checked with static images, not separate video runs.

After restoring settings, title and a fresh login visit showed native backgrounds
again (`stock-title.jpg`, `stock-login.jpg`). This verifies disable fallback for
both the atlas and layer approaches without restarting. The memory-only monitor
shortcut was restored to original instructions. Game DLL/assets were not written.
