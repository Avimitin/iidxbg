# Observed IIDX34 control flow

Tested 2026-10-23 on IIDX34 / 010-0917, game SHA-256
`c98e183ec81ecb9e1cb13c237f9c9cb0b3f65b6fdaff6fd5e6a526008c546534`.
This is an observation-driven procedure, not a timing-independent macro.

## Connection, inputs and captures

Use the official spice2x Python client: `spiceapi.Connection(host, port, password)`.
It handles the encrypted session/refresh protocol; do not send ad hoc JSON over
a raw socket. Keep one connection per sequence, and close it afterward. A new
connection is needed after restarting the game.

| Operation | API call / tested behavior |
| --- | --- |
| List cabinet inputs | `buttons_read(c)`; button names are case-sensitive |
| Press simultaneous inputs | `buttons_write(c, [[name, 1], ...])` |
| Release overrides | `buttons_write_reset(c, [[name], ...])`, in `finally` |
| Add test credits | `coin_insert(c, 2)` worked; the coin-mech button did not in this setup |
| Insert card | `card_insert(c, 0, card_id)`; use a test card, not another person's card |
| Keypad | `keypads_write(c, 0, digits)`; cabinet keypad, not desktop typing |
| Main capture | `capture_get_jpg(c, 0, 90, 1)` -> data, width, height; 1920x1080 here |
| Subscreen capture | Same call with screen 1; 1280x720 here, even with PgUp overlay closed |
| Touch | `touch_write(c, [[contact_id, x, y]])`, then `touch_write_reset(c, [contact_id])` |
| Stop game | `control_exit(c)`; confirm API goes offline |
| Restart game | `control_restart(c)` once; verify new startup log/connection |

Validate all chord button names before writing any of them. An unknown `Effect`
in a chord failed after another input could already have been written; the
correct button is `EFFECT`. Typical taps were 0.15-0.55 seconds with a 0.25-0.5
second gap. Cabinet names used: `P1 Start`, `P1 1` through `P1 7`, `P1 TT+`,
`P1 TT-`, `VEFX`, `EFFECT`.

Card injection is available, but a complete newly registered card login was not
validated in this run. Use guest mode for rendering checks unless the task needs
an account. Do not assume injected card data bypasses PIN or registration screens.

## Guest Standard SP navigation

1. At the title/attract screen, insert two test credits and tap `P1 Start`.
2. At the e-amusement card screen, tap `P1 Start` again to play as a guest. Allow
   the loading transition to finish; a fixed three-second wait was often short.
3. The tested mode/tutorial sequence used white `P1 1`, `P1 Start`, then white
   `P1 1` to reach Standard. Check the actual screen between steps; do not select
   a mode just because it occupies a position seen in an earlier run.
4. Once the gameplay tutorial is actually visible, press `VEFX` + `EFFECT`
   together to skip. An input sent during loading is easily missed.
5. Song selection initially has six tutorial pages. Advance with five separate
   white taps, confirm the last page says Start to exit, then tap `P1 Start`.
   If still on the final tutorial page, tap Start again after it settles.
6. `Caramel Pain` BEGINNER 1 was a useful default for extended observation.
   White confirms it. Beginner selection adds a play-options confirmation:
   select the highlighted Play Start with another white tap after the page loads.

`VEFX` cycles chart difficulty. Three taps from BEGINNER selected ANOTHER 8 in
the observed Caramel Pain menu. Guest first-stage selection was limited to level
8, so the suggestion to choose level 12 was not available in that session. For
fast result checks use an available harder chart or EX HARD; for subscreen
inspection use BEGINNER and allow enough time to operate the menus.

## Native play options through API inputs

Hold `P1 Start` in selection to show options. `EFFECT` while Start is held switches
between beginner and standard option layouts. Inspect the layout before changing
values; the examples below assume the observed standard layout and initial OFF.

| Setting | Input / observed result |
| --- | --- |
| Lane cover | `P1 7` cycles OFF, SUDDEN+, HIDDEN+, SUD+ & HID+, LIFT, LIFT & SUD+ |
| Combined covers | Five `P1 7` taps from OFF selected LIFT & SUD+ |
| Score graph | Brief `P1 TT-` selected PACEMAKER rotate from the observed no-graph selection; wait at least 0.5s and read the highlighted option |
| Fast fail | `P1 4` (gauge up) from OFF wrapped to EX HARD; verify the highlight |
| SUDDEN+ height in play | Hold `P1 Start` + `P1 TT+` about 3s; observed SUB-LENGTH around 621-624 |

Release Start, allow the options to close, then confirm the song. A white press
immediately after release was sometimes ignored. Do not repeat five cover taps
without checking the current option; the initial state can persist/change.

Example with the helper imported from its scripts directory:

```python
from spice_control import Client
import time

with Client.from_env() as game:
    with game.held('P1 Start'):
        time.sleep(0.5)
        # Only when the screenshot confirms beginner layout:
        game.press('EFFECT', duration=0.15)
        time.sleep(0.3)
        # Only when the standard layout confirms cover OFF:
        for _ in range(5):
            game.press('P1 7', duration=0.15)
            time.sleep(0.25)
        game.press('P1 TT-', duration=0.12)
        time.sleep(0.5)
        game.capture('target/options.jpg', screen=0)
```

Black margins on a narrow cover are not proof of failure. The MF video path
preserves aspect ratio; a short cover may expose only its black area. Increase
SUDDEN+ height to expose actual content. The user accepted these margins. The
run verified shared LIFT/SUDDEN+ descriptors and visible SUDDEN+ motion; it did
not establish visible LIFT content at every height or a general LIFT adjustment
recipe. Preserve the game's clipping, labels and graph bars.

## PgUp, Premium Area and concentration

PgUp toggles the Spice subscreen overlay on the main game window. It is a desktop
key sent through a focused Moonlight client, not an API cabinet button. Direct
Spice screen-1 captures and touch calls do not require the overlay to be open.

Premium Area is the background behind the native subscreen controls. It was
observed during selection, gameplay and results, on the base-only native path.

During gameplay:

1. Inspect screen 1. If the assistant is closed, tap its top-left icon near
   `(100, 36)` on the 1280x720 canvas.
2. The assistant exposes a CONCENTRATION MODE switch near `(100, 196)`. Tap it
   and wait several seconds for the native UI transition.
3. Capture two screen-1 images three seconds apart to verify full-screen motion.
4. Tap near `(640, 360)` to dismiss concentration temporarily. Premium and the
   assistant should return; with concentration enabled, it automatically shows
   again. Verify playback resumes after that show.
5. Dismiss, then tap the switch off to keep the ordinary subscreen visible.
   Retire the song and verify Premium continues on the result subscreen.

Coordinates are observations for this canvas, not universal constants. Screen
layout changes, an open settings page, or the assistant already being visible
can make the same tap do something else. `touch_write` uses a contact ID; keep
one test contact and release it in `finally`.

Observed home/settings navigation: home near `(640, 35)`, Settings near
`(790, 330)`, assistant close near `(610, 128)`. The Settings gear opened
turntable-resistance/lighting settings; concentration was in the assistant,
not that gear page. Do not change resistance while searching for concentration.

## Verification and finishing a run

Capture both rendering and transitions. In the live test, use distinct rooms
24065 for Premium/results/covers and 6 for selection/graph/concentration when
they are on air; these are test examples, not stable sources. No need to use an
account or change providers merely to exercise controls.

Check selection motion, covers/graph during gameplay, result motion, Premium
across scenes, concentration dismiss/re-show, and return to results. A prior
concentration bug froze its last frame over the result subscreen despite native
show(0); it was fixed by masking retained descriptors and restoring visibility
on the next show. Include this regression in future subscreen tests.

The run actually covered guest SP, failed/F results, default concentration index
0, and exact 010-0917. A/AA/AAA, DP/P2, customized concentration indices, induced
device loss, other hashes and long network outages were not verified. Save
captures/log excerpts/config snapshots under ignored `target/`, not in commits.
Restore temporary TOML edits byte-for-byte only after confirming no intervening
changes. Stop the game when requested; leave other games/streaming processes alone.
