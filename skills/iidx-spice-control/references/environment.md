# Environment and launch observations

Session date: 2026-10-23. These are local deployment facts, not portable defaults.

| Item | Observed location/value |
| --- | --- |
| Repository | `D:\Share\iidxbg` |
| Game host | `2dx.lan.sh1mar.in` |
| Game modules share | `\\2dx.lan.sh1mar.in\IIDX34\modules` |
| Same directory on game host | `D:\IIDX34\modules` |
| Launcher share | `\\2dx.lan.sh1mar.in\IIDX34\iidx-xepher-asio.bat` |
| Launcher on game host | `D:\IIDX34\iidx-xepher-asio.bat` |
| Spice API | TCP 55573; password supplied at runtime, omitted from this skill |
| Official Python client | `D:\Share\chart-requester\reference\spice2x\src\spice2x\api\resources\python` |
| Additional analysis | `D:\Share\chart-requester` and the iidxbg `docs` directory |
| Remote desktop | Sunshine on game host, connected Moonlight client locally |
| Separate installation | Desktop Sparkle Shower is IIDX33; leave it untouched |

Executing a UNC launcher from the local shell can launch on the wrong computer.
Start the game on the game host via the already connected desktop or a verified
remote-execution facility. Do not assume SMB access implies remote execution.
Reuse the current task's supplied connection information; request missing secrets
only when required for an authorized action.

## Restart, stop and plugin replacement

`spiceapi.control_restart(c)` successfully restarted the existing elevated game
with its current launcher arguments, avoiding a fresh UAC prompt in this session.
The old request timed out when the process disappeared; that did not mean restart
failed. Send one request, observe the API transition and inspect the new
`iidxbg.log` startup/hash before deciding whether another attempt is needed.
This does not bypass Windows elevation policy. If an initial launch requires a
secure-desktop UAC confirmation, do not repeatedly inject clicks into it.

Stop with `control_exit(c)`, not `control_shutdown` or `control_reboot` (those
affect the machine). Confirm the API becomes unavailable; if it stays available,
inspect rather than escalating to broad process termination. Do not close
Sunshine, Moonlight, or another game as a side effect.

For an authorized plugin deployment, back up `iidxbg.dll` and user TOML first.
Renaming the loaded plugin to a unique `.bak` name and copying the replacement
to `iidxbg.dll` worked; the old process retains its mapping until restart. Verify
the next startup log's plugin SHA-256. This applies to the plugin only. Never
replace `bm2dx.dll` or game assets as part of this workflow. Do not overwrite a
user config with repository defaults or silently overwrite an existing backup.

## Exact-build monitor shortcut

When launch-speed work is authorized, the repo's
`scripts/spice-monitor-bypass.py` applies a reversible **memory-only** shortcut
after the API is available. It checks the on-disk hash and both live instructions
before writing, then verifies readback. Every restart removes the memory patch.
Read `docs/iidx34-010-0917-analysis.md` for the recorded evidence.

- Supported game SHA-256:
  `c98e183ec81ecb9e1cb13c237f9c9cb0b3f65b6fdaff6fd5e6a526008c546534`.
- RVAs `0x91b0a8` and `0x91b37b`: validation/retry bypass and three-second
  measurement, respectively. Do not apply these offsets to another hash.
- The API memory calls in this script take translated **file offsets**; do not
  replace them with the documented RVAs. The script uses PE section translation.
- `--restore` returns the original live instructions. No disk patch is needed.

Example from the repository root, using a password already supplied in the
process environment (do not print it):

```powershell
py scripts/spice-monitor-bypass.py '\\2dx.lan.sh1mar.in\IIDX34\modules\bm2dx.dll' --api-python $env:IIDX_SPICE_API_PYTHON --host $env:IIDX_SPICE_HOST --port 55573 --password $env:IIDX_SPICE_PASSWORD
```

## Moonlight coordinates and PgUp

An earlier claim that Moonlight rejected synthetic input was incorrect. The
desktop capture was 3840x2160 while its rendered preview was 2048x1152; clicking
preview coordinates without the 1.875 scaling factor targeted the wrong place.
Use the current image's actual dimensions and window rectangle; neither scale
nor the previously observed HWND is a permanent constant.

On Windows, establish DPI awareness before reading rectangles or positioning the
cursor. Enumerate visible windows by title to find the current Moonlight stream,
focus/click that window, and use its physical pixel coordinates. The tested
stream window was 1280x800; never hardcode its old handle.

PgUp worked with a mapped scan code and extended-key flag after Moonlight had
focus. For an already identified window, the essential key operation was:

```python
import ctypes, time
u = ctypes.windll.user32
# SetProcessDPIAware, identify/focus the stream, and verify foreground first.
scan = u.MapVirtualKeyW(0x21, 0)  # VK_PRIOR / Page Up
u.keybd_event(0x21, scan, 1, 0)  # extended key down
try:
    time.sleep(0.15)
finally:
    u.keybd_event(0x21, scan, 3, 0)  # extended key up
```

Capture afterward to prove the overlay toggled. Prefer Spice captures for game
evidence; a scaled desktop preview is useful for locating windows, not for
inferring the game's rendering resolution.
