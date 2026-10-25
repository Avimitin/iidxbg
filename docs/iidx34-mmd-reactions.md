# Song-selection MMD reactions: exact-build evidence

Analysis date: 2026-10-25. Input supplied as `010-20260916.dll`, 18,200,216
bytes, SHA-256
`c98e183ec81ecb9e1cb13c237f9c9cb0b3f65b6fdaff6fd5e6a526008c546534`.
This is the existing `010-20260917.rs` / 010-0917 profile by **hash**. The
filename does not establish a new supported build. PE image base `0x180000000`,
image size `0xc5ff000`, AMD64. Addresses below are RVAs.

## Current implementation: Spice SDK scratch input

The user requested scratch movement as the trigger on 2026-10-25. The plugin
now reads the SDK's normalized turntable state on the animation worker at the
configured 30/60 fps. No network API or password is needed. Native selection
callbacks publish only the active/interactive gates (scene +88 == 3); leaving
selection/reset clears them. No input API call runs in a game/graphics callback.

Local reference source: `D:\Share\chart-requester\reference\spice2x\src\spice2x`,
Git HEAD `7aba7da3b67f3e4c8b0e950f306bd044d69fa494`. File SHA-256:

| Input | SHA-256 |
| --- | --- |
| `sdk/include/spicesdk.h` | `1e292d9ece962f91d1020ff79381eada06ccd5736a9f0f69496f25dfab2d2109` |
| `sdk/include/spicesdk_io.h` | `b38bd7f750f65ced1f104976cce4f6587a485e96103ca0d9de0c7fdae6168c7e` |
| `sdk/sdk.cpp` | `d3e8917f8aef81ac3c6b389fdb37bc4c6d1097c928f58b94ae0163f5067069c6` |

ABI: `SPICE_SDK_V0` has a u32 size, x64 padding, then function pointers.
`get_analog` is pointer slot 5 (byte offset 48): cdecl `i32(u32, float*)`;
`get_button` is slot 3 (byte offset 32): cdecl
`i32(u32, bool*, float*)`, with nullable velocity. Status 0 is success. These
are v0.1 fields of the already negotiated SDK; no executable RVAs are used.
Analog IDs 0/1 are IIDX TT P1/P2. Button IDs 10/11 and 22/23 are directional
TT+/TT- controls. Combined alternating TT+/- buttons are not interpreted.

`sdk_get_analog` checks initialization under the SDK shared mutex and calls
`GameAPI::Analogs::getState`; `games/iidx/iidx.cpp::get_tt` uses the same state
for game input. SDK shutdown makes subsequent calls return TOO_LATE. Plugin
shutdown also clears its published input function addresses and active gates;
the plugin is pinned and the host code remains loaded until process exit.
The worker retains only SDK function addresses, never input-device pointers.

Circular deltas handle 0/1 wraparound. A 0.004 dead zone suppresses jitter;
changes above 0.25 turns/sample rebaseline instead of firing. Baselines reset
outside interactive selection. Either turntable can trigger; simultaneous
opposite inputs give P2 priority. Positive movement maps to down by default,
with a GUI/TOML reversal switch. This is an approximate gesture trigger, not
proof that the game moved a song row. Controller sensitivity/inversion,
extremely fast movement and wheel use in selection option menus are limits.

Reproduce: inspect the hashed SDK declarations/implementation; run scratch
wrap/jitter/reversal tests; enable MMD on the exact game hash; turn the platter
both ways on selection. The worker logs at most five `MMD Spice scratch` events
per interactive visit. Verify separate configured up/down VMDs and return to
normal; leave selection and confirm input no longer triggers character work.

## Superseded selection-cursor investigation

The first implementation read list +124. Actual SP testing with plugin hash
`e0aac564091820515a152735a97a469133e9550d8b530d0231ff8801814c7064`
showed this field remaining zero while visible songs changed; it is not the
song-row cursor. The native list reads and their extra guards were removed.
The following evidence is retained as analysis history, not a working trigger.

| RVA / layout | Evidence |
| --- | --- |
| `0x8d8290`, vtable `0xd52508` | Existing ordinary selection update adapter; original update runs first. |
| scene `+88` | Existing selection state; observe cursor only in ordinary interactive state 3. |
| `0x86f8c0` | No-argument Win64 getter; returns singleton pointer from `0xb3e9f88` in RAX. |
| owner `+94032`, `+94040` | Current list object and shared-pointer control block, respectively. |
| `0x5f6b90` | Reads this shared pointer and invokes `0x5f6dd0`; increments/decrements control-block reference counts. |
| `0x5f6dd0` | Invokes `0x5f6c40`, then current entry virtual slot `+24` with list column `+120`. |
| `0x5f6c40` | Bounds column at list `+120` to 0/1; uses signed index at `+124` to select an entry from vector at `+72 + column*24` / end `+80 + column*24`. |
| `0x5fa3b0` | Selected song getter calls `0x5f6b90`, resolves song data, or returns the default song when unavailable. |

The former observer ran synchronously after the original selection update on the
existing game/graphics owner. It uses the current singleton/list only within
that callback, checks readable fields and column/index bounds, and publishes
only a signed direction and serial number. It saves object *identity* for
comparison but never dereferences a saved pointer. A new list or column resets
the baseline, so changing folders does not deliberately trigger a scroll.
No native object crosses to the animation worker. The lifetime assumption is
the same owner-thread list stability as the native getter; cross-thread list
replacement is not established by this static analysis.

Follow-up decompilation of the same hashed input: base scene update `0x94fcf0`
switches on `scene+88`, invokes virtual `+200` in state 3 and commits next state
from `+92`. Selection `0x8d8290` calls controller updater `0x8da130` and finishes
through `0x94fcf0`; controller `+8` is the active member target and `+84` its
frame counter. `0x8da0a0` is the normal interactive member. This confirms the
state gate independently; it does not replace live cursor validation. The
observer logs state changes, the initial cursor and at most five scroll events
per visit when an imported model is present, through the existing worker log
queue (no callback file IO).

Follow-up: `0x5f6dd0(list)` resolves the entry and reads an inner index through
virtual +24; `0x5f88e0(list,index)` writes that inner index, and `0x5f7f70`
uses it for visible rows. The outer +124 index is insufficient. This alternative
was not implemented after switching to the user's requested SDK input.

## Last-play grade

| RVA | ABI / evidence |
| --- | --- |
| `0x906b80` | Result background constructor calls `0x9075a0`; grade-dependent background chosen after play. |
| `0x9075a0`, `0x9073e0` | Clear-result selection calls joined-player and grade getters, combines the joined sides. |
| `0x9078b0` | DJ-level UI construction creates `level_p01` / `level_p02`, calls `0x919540(side,0)` then `0x907a20`. |
| `0x907a20` | Grade 0 -> `a3_in`, 1 -> `a2_in`, 2 -> `a_in`, 3..6 -> B..E, otherwise F. |
| `0x92fd00` | `bool(u32 side)` (ECX -> AL); side below 2, active flag at `0xb60b3b0 + side*4`. |
| `0x919540` | `u32(u32 side,u8 clamp)` (ECX, DL -> EAX); obtains notes via `0x871fb0` + `0x88cd50`, EX score via `0x919610`, calls `0x8195b0`. |
| `0x8195b0` | For clamp=0 and notes>0: clamp `8 - floor(9*EX/(2*notes))` into 0..7. Zero notes returns 7. |
| `0x919610` | Gets current play judgment counts via `0x95b8c0`, delegates EX computation to `0x819880`. |

The plugin calls these same native getters once, after the original first
ordinary-result background update. It publishes A-or-better when **any joined
side** has grade 0, 1 or 2. This is current-play DJ grade, not the clear lamp or
historical best. The next ordinary selection activation consumes the event.
No score memory is written and no additional detour is installed for reactions.

## Guards and reproduction

The existing exact-file hash, PE size and live hook checks still run. Added
profile guards cover the grade getters/formula. Removed cursor guards are
listed explicitly for reproducibility of the superseded investigation:

| Key | RVA | Bytes |
| --- | --- | --- |
| `reaction_grade` | `0x919540` | `48895c24084889742410574883ec200f` |
| `reaction_joined` | `0x92fd00` | `85c9781783f90273124863c1488d0d9d` |
| removed: `reaction_selection_owner` | `0x86f8c0` | `488b05c1a6b70ac3cccccccccccccccc` |
| removed: `reaction_list_pointer` | `0x5f6bb6` | `488b98586f01004885db740bf0ff4308` |
| removed: `reaction_cursor_layout` | `0x5f6c60` | `486349784883f9020f834b0100004863` |
| `reaction_grade_formula` | `0x8195b0` | `85d27506b807000000c38d04c98d0c12` |

The removed layout anchors were instruction guards inside functions. Scene
state and retained grade offsets remain limited to this exact profile.

Reproduce: hash the input; build `iidxbg-tool`; run
`py scripts/verify-native.py <input.dll>`; open the same input in IDA;
decompile the RVAs above at image base + RVA and inspect call sites and field
accesses. The offline verifier passed all entry, slot and texture-lifetime
guards including these new anchors. It does not prove live reactions.

Rendering reuses the verified AFP frame callback `0x328b40`, inside the game's
BeginScene/EndScene; see [existing atlas evidence](iidx34-custom-art.md).
Original callbacks still run. Device reset detaches replacement atlases and
drops model GPU resources on the graphics owner; immutable imported assets
remain available for recreation.

## Remaining validation boundaries

Static evidence does not establish live A/AA/AAA triggering, DP/P2 behavior,
all list modes, long sessions or induced device loss. This implementation
does not support reaction offsets on other hashes. See
[MMD usage and validation](mmd-character.md) for the actual tests performed.
