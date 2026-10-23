"""Read the compiled Rust build profiles without maintaining duplicate tables."""
import json
import os
from pathlib import Path
import subprocess


def load_profiles():
    root = Path(__file__).resolve().parent.parent
    override = os.environ.get("IIDXBG_TOOL")
    candidates = ([Path(override)] if override else
                  [root / "target" / mode / "iidxbg-tool.exe"
                   for mode in ("debug", "release")])
    candidates = [path for path in candidates if path.is_file()]
    if not candidates:
        raise RuntimeError("Build iidxbg-tool first using scripts/build.ps1; "
                           "or set IIDXBG_TOOL to the freshly built tool.")
    tool = max(candidates, key=lambda path: path.stat().st_mtime_ns)
    sources = [root / "src/profile.rs", root / "src/bin/tool.rs",
               *(root / "src/profiles").rglob("*.rs")]
    if any(path.stat().st_mtime_ns > tool.stat().st_mtime_ns for path in sources):
        raise RuntimeError(f"{tool} predates the profile sources; rebuild iidxbg-tool.")
    return json.loads(subprocess.check_output([str(tool.resolve()), "profiles"], text=True))


def load_profile(digest):
    for profile in load_profiles():
        if profile["sha256"] == digest:
            return profile
    raise RuntimeError(f"Unsupported DLL SHA-256: {digest}")
