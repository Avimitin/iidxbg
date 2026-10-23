"""Emulate the exact crash-build primitive publishers and shader callback branch.

Requires pefile and unicorn. Never loads the game. AFP ordinal 51 is mocked;
the native updater instructions and post-draw callback decision are real.
"""
import argparse
import hashlib
from native_profiles import load_profile
from pathlib import Path
import struct

import pefile
from unicorn import Uc, UC_ARCH_X86, UC_MODE_64, UC_HOOK_CODE
from unicorn.x86_const import (
    UC_X86_REG_RAX, UC_X86_REG_RCX, UC_X86_REG_RDX, UC_X86_REG_R8,
    UC_X86_REG_R13, UC_X86_REG_RSP,
)

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("dll", type=Path)
args = parser.parse_args()
data = args.dll.read_bytes()
profile = load_profile("c98e183ec81ecb9e1cb13c237f9c9cb0b3f65b6fdaff6fd5e6a526008c546534")
assert hashlib.sha256(data).hexdigest() == profile["sha256"], "Unsupported build"
pe = pefile.PE(data=data)
base = pe.OPTIONAL_HEADER.ImageBase
iat = next(i.address for e in pe.DIRECTORY_ENTRY_IMPORT
           if e.dll.lower() == b"afp-core.dll" for i in e.imports if i.ordinal == 51)
OBJ, STACK, STUB, END, PAYLOAD, DEVICE = (
    0x71000000, 0x72000f08, 0x73000000, 0x73000100, 0x74000000, 0x75000000)


def machine():
    uc = Uc(UC_ARCH_X86, UC_MODE_64)
    for address in (OBJ, STACK & ~0xfff, STUB, PAYLOAD, DEVICE):
        uc.mem_map(address, 0x1000)
    uc.mem_write(STUB, b"\xc3")
    uc.mem_write(STACK, struct.pack("<Q", END))
    uc.reg_write(UC_X86_REG_RSP, STACK)
    return uc


def map_code(uc, address):
    page = address & ~0xfff
    uc.mem_map(page, 0x1000)
    uc.mem_write(page, pe.get_data(page - base, 0x1000))


def callback_target(payload):
    uc = machine()
    map_code(uc, base + 0x338ed8)
    # Poison adjacent bytes with the dump's invalid callback. The real payload
    # is only 16 bytes; the unguarded renderer nevertheless reads offset 32.
    uc.mem_write(PAYLOAD, bytes(48))
    uc.mem_write(PAYLOAD + 32, struct.pack("<Q", 0x3f8000003f800000))
    uc.mem_write(PAYLOAD, payload)
    uc.reg_write(UC_X86_REG_RDX, PAYLOAD)
    uc.reg_write(UC_X86_REG_R13, DEVICE - 0x18000)
    attempted = []

    def inspect(machine, address, size, context):
        if address == base + 0x338f00:
            attempted.append(machine.reg_read(UC_X86_REG_RAX))
            machine.emu_stop()  # Stop immediately before the indirect jump.

    uc.hook_add(UC_HOOK_CODE, inspect)
    uc.emu_start(base + 0x338ed8, base + 0x338f03, count=30)
    return attempted


for name in ("box_fill_update", "box_outline_update", "line_update"):
    rva = profile["rvas"][name]
    guard = next(g for g in profile["guards"] if g["rva"] == rva)
    assert pe.get_data(rva, len(guard["hex"]) // 2).hex() == guard["hex"]
    for handle in (0, 123):
        for count in (0, 4):
            for repaired in (False, True):
                uc = machine()
                map_code(uc, base + rva)
                map_code(uc, iat)
                uc.mem_write(iat, struct.pack("<Q", STUB))
                obj = bytearray(b"\xa5" * 56)
                struct.pack_into("<I", obj, 8, handle)
                struct.pack_into("<I", obj, 24, count)
                struct.pack_into("<Q", obj, 32, 0x115392010)
                struct.pack_into("<I", obj, 44, 0x10000)
                if repaired and handle:
                    struct.pack_into("<I", obj, 44, 0)
                uc.mem_write(OBJ, bytes(obj))
                uc.reg_write(UC_X86_REG_RCX, OBJ)
                calls = []

                def capture(machine, address, size, context):
                    if address == STUB:
                        assert machine.reg_read(UC_X86_REG_RCX) == handle
                        assert machine.reg_read(UC_X86_REG_R8) == 16
                        source = machine.reg_read(UC_X86_REG_RDX)
                        assert source == OBJ + 40
                        calls.append(bytes(machine.mem_read(source, 16)))
                        machine.reg_write(UC_X86_REG_RAX, 0x12345678)

                uc.hook_add(UC_HOOK_CODE, capture)
                uc.emu_start(base + rva, END, count=100)
                if handle:
                    expected = struct.pack("<IIQ", count, 0 if repaired else 0x10000,
                                           0x115392010)
                    assert calls == [expected]
                    assert uc.reg_read(UC_X86_REG_RAX) == 0x12345678
                    assert callback_target(calls[0]) == (
                        [] if repaired else [0x3f8000003f800000])
                else:
                    assert calls == []
                    assert bytes(uc.mem_read(OBJ, 56)) == bytes(obj)
    print(f"PASS: {name}: original poison reproduced; repair preserves count/pointer, "
          "skips bogus callback, inactive object unchanged")

# Control: the unchanged renderer still dispatches a legitimate 48-byte shader.
shader = bytearray(48)
struct.pack_into("<I", shader, 4, 0x10000)
struct.pack_into("<Q", shader, 32, STUB)
assert callback_target(bytes(shader)) == [STUB]
print("PASS: legitimate 48-byte shader post-callback still dispatched")
