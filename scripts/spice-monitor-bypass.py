"""Exact-build, reversible in-memory monitor-check shortcut for local TDJ tests.

Requires pefile and spiceapi (use --api-python for the official checkout).
The game file is read/hash checked but never written. Apply after the Spice API
starts on each launch. See docs/iidx34-010-0917-analysis.md for instruction evidence.
"""
import argparse
import hashlib
import sys
from pathlib import Path

import pefile

HASH = "c98e183ec81ecb9e1cb13c237f9c9cb0b3f65b6fdaff6fd5e6a526008c546534"
PATCHES = [
    # Bypass validation/retry, retain native completion and measured timing.
    (0x91B0A8, "0f8540010000", "90e940010000"),
    # 20 * selected refresh rate -> 3 * rate (359-sample FPS window at 120Hz).
    (0x91B37B, "8d1480c1e202", "8d1440909090"),
]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("game_dll", type=Path)
    parser.add_argument("--api-python", type=Path)
    parser.add_argument("--host", required=True)
    parser.add_argument("--port", type=int, default=55573)
    parser.add_argument("--password", required=True)
    parser.add_argument("--restore", action="store_true")
    args = parser.parse_args()
    data = args.game_dll.read_bytes()
    if hashlib.sha256(data).hexdigest() != HASH:
        raise RuntimeError("unsupported on-disk DLL hash; no writes performed")
    pe = pefile.PE(data=data)
    if args.api_python:
        sys.path.insert(0, str(args.api_python))
    import spiceapi
    con = spiceapi.Connection(args.host, args.port, args.password)
    changes = []
    # Validate every patch before making any write.
    for rva, original, patched in PATCHES:
        offset = pe.get_offset_from_rva(rva)
        if data[offset:offset + 6].hex() != original:
            raise RuntimeError(f"disk instruction guard failed at {rva:#x}")
        live = spiceapi.memory_read(con, "bm2dx.dll", offset, 6).lower()
        if live not in (original, patched):
            raise RuntimeError(f"live instruction guard failed at {rva:#x}: {live}")
        desired = original if args.restore else patched
        changes.append((rva, offset, live, desired))
    applied = []
    try:
        for rva, offset, live, desired in changes:
            if live != desired:
                spiceapi.memory_write(con, "bm2dx.dll", desired, offset)
                applied.append((offset, live))
            if spiceapi.memory_read(con, "bm2dx.dll", offset, 6).lower() != desired:
                raise RuntimeError(f"readback failed at {rva:#x}")
            print(f"RVA {rva:#x}: {desired}")
    except Exception:
        for offset, live in reversed(applied):
            spiceapi.memory_write(con, "bm2dx.dll", live, offset)
        raise
    print("Restored original in-memory instructions" if args.restore else
          "Applied 3-second measurement and validation bypass in memory; disk unchanged")


if __name__ == "__main__":
    main()
