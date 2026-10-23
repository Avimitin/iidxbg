---
name: iidx-spice-control
description: Control and visually verify an IIDX game running under spice2x using its Python API and a Moonlight client. Use for game navigation, native play options, main/subscreen captures, background playback checks, or requested game restart/exit.
---

# IIDX control through spice2x

Use the official `spiceapi` Python client for game buttons, coins, card insertion,
touch, screenshots, and process control. Use Moonlight for desktop keys such as
PgUp. The API exposes named cabinet inputs; it is not a general Windows keyboard
injection API. Native options are changed by those inputs and touchscreen menus,
not by inventing a settings API or writing guessed memory offsets.

Read [the control flow](references/control-flow.md) for navigation, options and
subscreen checks. Read [the test environment](references/environment.md) when
locating the SDK/game, handling launch/deployment, or using the monitor shortcut.
The recorded observations are from IIDX34 on 2026-10-23; do not infer other-build
compatibility from a version name.

## Working loop

1. Establish which game and host the user intends to control. Reuse connection
   details already supplied. Discover a running API before attempting launch.
2. List the available buttons and capture both screens. Decide the next input
   from the visible state. Wait for transitions, then capture again; long blind
   input sequences skipped or missed tutorial actions in this session.
3. Hold inputs only for the required interaction and release the exact inputs
   in `finally`. Do not reset unrelated inputs that the user may be holding.
4. For motion checks, capture pairs several seconds apart. A successful prepare
   log or a first frame is not evidence of moving video. Test hide/show and scene
   exit as well: concentration initially left a stale frame over results.
5. Record the actual mode, screen, settings, build hash and evidence paths. State
   what was observed and what remains untested. Restore temporary configuration
   only if it still matches the test copy; preserve intervening user edits.

Do not start a stopped game merely to document controls or validate this skill.
Restart/exit the game when requested or necessary within an already authorized
test. Use `control_exit`, never machine shutdown/reboot, to stop IIDX. Do not touch
the separate IIDX33 installation. Game assets and game DLLs stay unchanged.

## Reusable helper

`scripts/spice_control.py` provides `buttons`, `capture`, `press`, `touch`, `coin`,
`card`, `exit`, and `restart`. Its default port is the session's 55573; override
it for another installation. Supply `IIDX_SPICE_HOST`, `IIDX_SPICE_API_PYTHON`
(directory containing `spiceapi`), and `IIDX_SPICE_PASSWORD` in the process
environment. Never commit credentials or include them in command examples/logs.

Run from the skill directory, with output paths in the project's ignored target:

```powershell
py scripts/spice_control.py buttons
py scripts/spice_control.py capture D:/Share/iidxbg/target/main.jpg --screen 0
py scripts/spice_control.py capture D:/Share/iidxbg/target/sub.jpg --screen 1
py scripts/spice_control.py press 'P1 Start'
py scripts/spice_control.py press 'VEFX' 'EFFECT'
py scripts/spice_control.py touch 100 196
```

The helper can also be imported for a sequence that keeps Start held while other
buttons are tapped; see the reference. `restart` sends one request and reports
that startup still needs verification. Do not repeatedly send it after a timeout.
The stopped game is left stopped unless the current task calls for launch.
