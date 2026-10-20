"""Test native attachment in a private process; never start the game.

Maps the supplied bm2dx.dll with DONT_RESOLVE_DLL_REFERENCES, without running
DllMain/imports. Synthetic patches affect this process only. No game callbacks,
graphics device or provider network requests are used. Outputs stay in target.
"""
import argparse
import ctypes
import hashlib
import json
from pathlib import Path
import shutil
import struct
import time
import uuid

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("plugin", type=Path)
parser.add_argument("game", type=Path)
parser.add_argument("--mode", choices=["native", "chain", "bad-tail"], default="native")
args = parser.parse_args()
assert ctypes.sizeof(ctypes.c_void_p) == 8
root = Path(__file__).resolve().parent.parent
digest = hashlib.sha256(args.game.read_bytes()).hexdigest()
profile = next(p for path in (root / "src/profiles").glob("*.json")
               if (p := json.loads(path.read_text()))["sha256"] == digest)
work = root / "target" / ("smoke-attach-" + uuid.uuid4().hex)
work.mkdir()
shutil.copy2(args.plugin, work / "iidxbg.dll")
shutil.copy2(root / "iidxbg.toml", work / "iidxbg.toml")

kernel = ctypes.WinDLL("kernel32", use_last_error=True)
kernel.LoadLibraryExW.argtypes = [ctypes.c_wchar_p, ctypes.c_void_p, ctypes.c_uint32]
kernel.LoadLibraryExW.restype = ctypes.c_void_p
game = kernel.LoadLibraryExW(str(args.game.resolve()), None, 1)
assert game, ctypes.WinError(ctypes.get_last_error())
kernel.VirtualAlloc.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.c_uint32, ctypes.c_uint32]
kernel.VirtualAlloc.restype = ctypes.c_void_p
kernel.VirtualProtect.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.c_uint32,
                                 ctypes.POINTER(ctypes.c_uint32)]
kernel.GetCurrentProcess.restype = ctypes.c_void_p
kernel.FlushInstructionCache.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_size_t]


def patch(address, data):
    old = ctypes.c_uint32()
    assert kernel.VirtualProtect(address, len(data), 0x40, ctypes.byref(old))
    ctypes.memmove(address, data, len(data))
    unused = ctypes.c_uint32()
    assert kernel.VirtualProtect(address, len(data), old.value, ctypes.byref(unused))
    assert kernel.FlushInstructionCache(kernel.GetCurrentProcess(), address, len(data))


entry = game + profile["rvas"]["background_update"]
before = ctypes.string_at(entry, 16)
if args.mode != "native":
    # Reserve a nearby executable relay; never invoke this synthetic detour.
    relay = None
    for offset in range(0x10000000, 0x20000000, 0x10000):
        relay = kernel.VirtualAlloc(game + offset, 0x1000, 0x3000, 0x40)
        if relay:
            break
    assert relay, "No nearby relay allocation"
    target = ctypes.cast(kernel.GetCurrentProcess, ctypes.c_void_p).value
    patch(relay, b"\xff\x25\0\0\0\0" + struct.pack("<Q", target))
    modified = b"\xe9" + struct.pack("<i", relay - entry - 5) + before[5:]
    if args.mode == "bad-tail":
        modified = modified[:5] + bytes([modified[5] ^ 1]) + modified[6:]
    patch(entry, modified)


class Sdk(ctypes.Structure):
    _fields_ = [("size", ctypes.c_uint32), ("functions", ctypes.c_size_t * 18)]


initializer = ctypes.CFUNCTYPE(ctypes.c_int, ctypes.c_uint32, ctypes.c_void_p, ctypes.c_void_p)
register_type = ctypes.CFUNCTYPE(ctypes.c_int, ctypes.c_void_p, ctypes.c_void_p)
shutdown = None


@register_type
def register_graphics(callback, context):
    assert callback
    return 0  # No READY/frame events: no game function can run.


@initializer
def initialize(version, destroy, output):
    global shutdown
    sdk = ctypes.cast(output, ctypes.POINTER(Sdk)).contents
    assert version == 0 and sdk.size == 152
    sdk.functions[17] = ctypes.cast(register_graphics, ctypes.c_void_p).value
    shutdown = ctypes.CFUNCTYPE(None)(destroy)
    return 0


plugin = ctypes.WinDLL(str(work / "iidxbg.dll"))
plugin.spice_sdk_entry_point.argtypes = [initializer]
plugin.spice_sdk_entry_point.restype = ctypes.c_int
assert plugin.spice_sdk_entry_point(initialize) == 0
deadline = time.monotonic() + 15
while time.monotonic() < deadline:
    log = work / "iidxbg.log"
    text = log.read_text(encoding="utf-8") if log.exists() else ""
    if "Startup failed:" in text or "Attached IIDX" in text:
        break
    time.sleep(0.05)
else:
    raise AssertionError("Worker attachment timed out")

rejected = args.mode == "bad-tail"
assert ("Startup failed:" in text) == rejected, text
assert ("Attached IIDX" in text) != rejected, text
if args.mode == "chain":
    assert "Preserving MinHook chain at background_update" in text, text
    assert ctypes.string_at(entry, 16) == modified, "Existing detour overwritten"
if rejected:
    assert "Live game code differs at background_update" in text, text
    assert "Hook chain rejected:" in text, text
    assert "Unsupported or malformed game DLL" not in text, text
for table in profile["tables"]:
    value = ctypes.c_void_p.from_address(game + table["rva"] + table["slot"] * 8).value
    assert (value == game + table["target"]) == rejected, "Unexpected vtable mutation"
assert shutdown
shutdown()
time.sleep(0.2)
print(f"PASS: {profile['label']} / {args.mode}; private mapping, no game execution")
