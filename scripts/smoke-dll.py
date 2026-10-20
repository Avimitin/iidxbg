"""Load only iidxbg.dll, with an SDK lacking D3D9 support; expect safe refusal.

Windows x64 Python. Never loads the game, installs hooks, or contacts a provider.
The plugin writes its refusal to iidxbg.log beside the supplied plugin DLL.
"""
import argparse
import ctypes
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("dll", type=Path)
args = parser.parse_args()


class Sdk(ctypes.Structure):
    _fields_ = [("size", ctypes.c_uint32), ("functions", ctypes.c_size_t * 18)]


assert ctypes.sizeof(ctypes.c_void_p) == 8, "Use x64 Python"
assert ctypes.sizeof(Sdk) == 152, "Unexpected SDK ABI layout"
plugin = ctypes.WinDLL(str(args.dll.resolve()))
initializer = ctypes.CFUNCTYPE(ctypes.c_int, ctypes.c_uint32,
                             ctypes.c_void_p, ctypes.c_void_p)
seen = []


@initializer
def no_graphics(version, destroy, output):
    sdk = ctypes.cast(output, ctypes.POINTER(Sdk)).contents
    seen.append((version, sdk.size, bool(destroy)))
    # The caller initializes all function pointers to zero.
    return 0


entry = plugin.spice_sdk_entry_point
entry.argtypes = [initializer]
entry.restype = ctypes.c_int
assert entry(no_graphics) == 1, "Plugin did not reject missing D3D9 support"
assert seen == [(0, 152, True)], seen
print("PASS: x64 LoadLibrary/export/SDK ABI; missing graphics support refused before hooks")
