# Selected-song tempo for MMD idle motion

Analysis date: 2026-10-25. Input `D:\Share\tmp\010-20260916.dll`,
18,200,216 bytes, SHA-256
`c98e183ec81ecb9e1cb13c237f9c9cb0b3f65b6fdaff6fd5e6a526008c546534`.
AMD64, image base `0x180000000`, image size `0xc5ff000`. This matches the
existing 010-0917 profile by hash, regardless of the supplied filename.
All addresses below are RVAs. No game file is modified.

## Native evidence

The existing selected-chart decoder in the local chart-requester checkout
(`crates/bilimani-iidx/src/versions/v34/b010_0917/native.rs`,
`sample_selected_song`) supplied starting points. IDA/Hex-Rays independently
confirmed the following against the input hash above. Earlier build addresses
in that project's general IIDX34 notes are different and were not reused.

| RVA | Observed ABI and purpose |
| --- | --- |
| `0x5fa240` | `u32(widget: RCX)`; selected entry type, 1 means song. |
| `0x5fa3b0` | `record*(widget: RCX)`; selected song record. Has fallback ID 1000, so check entry type first. |
| `0x5fa410` | `u32(widget: RCX, side: EDX)`; selected difficulty 0–4. |
| `0x92fb50` | `u8()` in AL; active P1/P2 side. |
| `0x92fcc0` | `u32()` in EAX; actual play-style flag at global `0xb60b3a4`. |
| `0x92fe50` | `u8()` in AL; DP layout, accounting for battle and two-player layout. |
| `0x81a600` | Native difficulty/layout-to-chart conversion: SP 0–4, DP 5–9, invalid 11. |
| `0x939420` | Record/index validator; level byte at record +`0x3ec`+index must be 1–12. |
| `0x6281c0` | `void(widget: RCX, record: RDX, difficulty: R8D)`; copies chart BPM to widget +28/+32. |
| `0x628010` | Display consumer of those widget BPM fields. |
| `0x6259e0` | Caller resolves the `bpm` clip and passes its song record to the copy helper. |

The selection widget starts at scene +416. The existing post-original
selection callback owns this scene. Scene +88 == 3 identifies interactive
selection, as recorded in [reaction analysis](iidx34-mmd-reactions.md).
After checking side 0–1 and difficulty 0–4, add five only when both the
actual style and DP layout are nonzero: two-player SP is not DP, and DP
battle uses SP charts.

Record +`0x67c` contains the music ID. At `0x6281f7`, the native helper
loads maximum BPM from record +`0x3fc`+8*index; at `0x628201` it loads minimum
BPM from record +`0x400`+8*index. The helper copies those values to widget
+28/+32 and treats nonpositive minimum as a single BPM. The plugin accepts
ID 1–99999, level 1–12, maximum 1–100000, and minimum 0–maximum. Invalid or
unavailable records publish zero (unknown). The maximum is used for a range.

All six directly called getters have exact 16-byte entry guards in
`src/profiles/iidx34/010-20260917.rs`. Layout anchors at `0x6281f7` and
`0x93942d` guard the independently observed data reads. No new detour is
installed. The profile still requires the complete file hash and existing
scene/graphics guards. The other profiles do not enable this path.

## Ownership and timing

Sampling runs at most four times per second after the original selection
update, only in state 3, with a loaded model whose BPM option is enabled.
The native getters are not called when the option is off. Record pointers
are borrowed only within this callback; no reference crosses a thread or
survives a visit. Readability and bounded chart offsets are checked before
field reads. An owned atomic integer is published to the animation worker.
Scene departure, reset and shutdown clear it.

The worker integrates an independent idle-motion clock using
`selected_max_BPM / motion_original_BPM`, bounded to 0.25–4.0. Changing songs
changes speed without restarting the pose timeline. Unknown BPM uses 1x.
Condition VMDs retain wall-clock playback duration. Work parks outside ordinary
selection, and the existing atlas/scene lifetime removes the character for
gameplay. There is no file IO, decoding, pose calculation or retained game
pointer in the selection callback. Diagnostics are bounded to five changes
per interactive visit and use the existing asynchronous event queue.

## Reproduction and limits

1. Hash the supplied input and open that exact file in IDA. Inspect the above
   functions, callers and field accesses; add image base to each RVA.
2. Build release and run `target/release/iidxbg-tool verify <input.dll>` and
   `py scripts/verify-native.py <input.dll>`. Check exported profile guards
   against file bytes; they include the two interior layout anchors.
3. Run the idle-tempo unit test: opt-in behavior, unknown values, ratio limits,
   phase continuity, and unchanged condition-motion duration.
4. For a future live test, enter the known original motion BPM, enable matching,
   and compare slow/fast selected charts and folders. Inspect `MMD selected
   BPM=` logs; confirm leaving selection removes the model. Include P2, DP,
   battle and BPM ranges before claiming those modes verified.

The user explicitly requested leaving matching off. The new native BPM
sampling has therefore **not been tested in game**. Static evidence and tests
do not establish live correctness across all modes. This controls playback
speed, not musical beat phase or audible song-preview synchronization. VMD
does not store original BPM; 120 is an editable placeholder, not a measured
tempo of the user's motion. Variable-tempo songs use their maximum; stops,
ramps and exact preview beat positions are not followed.

## Character placement in the same update

No new native address or ABI is used for positioning. The existing native
character tiles and draw order are retained (see
[atlas evidence](iidx34-custom-art.md)). Model-only composition now fits the
complete transparent render target into a configurable rectangle within the
tile, keeping its aspect ratio. Size and center use percentages. Bounds reserve
the top 13%, bottom 10% and side 2% of the native tile; oversized requests are
reduced and centers constrained before shader mapping. The upper-left preset
is size 42%, center X 20%, center Y 32%. Animation remains clipped to its owned
render target, so movement cannot escape this rectangle.

These are tile-relative margins, not a new screen overlay or HUD detector.
Native geometry/animation can still transform the complete tile. Song detail
text retains its existing foreground priority; resizing reduces overlap but
does not guarantee separation from every long title, custom frame or mode.
Placement tests cover both observed tile aspects, portrait/landscape sources,
and extreme configured centers. Actual screen appearance needs inspection
after the user loads the updated DLL.

## Verification and staging on 2026-10-25

72 root tests and 23 configuration-app tests passed (7/1 optional tests
skipped). Both Clippy checks passed with warnings denied; both release builds
passed. The native verifier checked every runtime guard, including the new
BPM getters and layout anchors, against the exact input above. The release
tool also loaded the user's current Temari PMX and normal VMD and rendered a
real-D3D9 offline pose at one second. This validates model/config loading, not
placement in the game. Evidence: `target/mmd-validation/temari-current-motion.png`.

Staged plugin SHA-256:
`0e10b239b84319fbb483efaf88772ca8eb23f2f4163502110cadbb3d48e8af76`.
Configuration app SHA-256:
`58816dede7662781a9510e2fb205a956329893fad665039a37e26af5bf2b7ae7`.
The test-host configuration preserves the user's newer Temari model/motion
paths and all other settings; only the placement preset and explicitly
disabled BPM matching were added. Before/after hashes and backups are recorded
in `target/mmd-validation/tempo-placement-update.json`. No restart command was
sent. Actual in-game testing of this update is pending the user's manual
restart; earlier PMX playback, scratch reactions and gameplay hiding were
already verified separately.
