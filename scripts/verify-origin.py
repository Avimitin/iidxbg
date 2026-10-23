"""Emulate the guarded Win64 pixel-origin helper; never load/run the game.

Requires pefile and unicorn. Import calls are mocked, including two linked clips.
Checks RCX/XMM1/XMM2 arguments, AFP origin/dirty properties and fractional centers.
"""
import argparse
import hashlib
from native_profiles import load_profile
from pathlib import Path
import struct

import pefile
from unicorn import Uc, UC_ARCH_X86, UC_MODE_64, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_RAX, UC_X86_REG_RCX, UC_X86_REG_RDX, \
    UC_X86_REG_R8, UC_X86_REG_RSP, UC_X86_REG_XMM1, UC_X86_REG_XMM2

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("dll", type=Path)
args = parser.parse_args()
data = args.dll.read_bytes()
digest = hashlib.sha256(data).hexdigest()
profile = load_profile(digest)
pe = pefile.PE(data=data)
base = pe.OPTIONAL_HEADER.ImageBase
rva = profile["rvas"]["set_origin_pixels"]
guard = next(g for g in profile["guards"] if g["rva"] == rva)
assert pe.get_data(rva, 16).hex() == guard["hex"]
imports = {i.ordinal: i.address for e in pe.DIRECTORY_ENTRY_IMPORT
           if e.dll.lower() == b"afp-core.dll" for i in e.imports}

for width, height in [(1920, 1080), (1280, 720), (1921, 1081)]:
    uc = Uc(UC_ARCH_X86, UC_MODE_64)
    mapped = set()

    def map_pe_page(address):
        page = address & ~0xfff
        if page not in mapped:
            uc.mem_map(page, 0x1000)
            uc.mem_write(page, pe.get_data(page - base, 0x1000))
            mapped.add(page)

    map_pe_page(base + rva)
    map_pe_page(base + rva + 0x100)
    stubs = {116: 0x60000100, 106: 0x60000200}
    uc.mem_map(0x60000000, 0x1000)
    for ordinal, stub in stubs.items():
        map_pe_page(imports[ordinal])
        uc.mem_write(imports[ordinal], struct.pack("<Q", stub))
        uc.mem_write(stub, b"\xc3")
    uc.mem_map(0x70000000, 0x10000)
    uc.mem_map(0x71000000, 0x1000)
    stack = 0x7000fef8  # Win64 entry alignment and caller home space.
    end = 0x60000300
    uc.mem_write(stack, struct.pack("<Q", end))
    uc.reg_write(UC_X86_REG_RSP, stack)
    uc.reg_write(UC_X86_REG_RCX, 0x71000000)
    uc.mem_write(0x71000000 + 512, struct.pack("<i", 10))
    center = (width * 0.5, height * 0.5)
    for reg, value in zip([UC_X86_REG_XMM1, UC_X86_REG_XMM2], center):
        uc.reg_write(reg, struct.unpack("<I", struct.pack("<f", value))[0])
    calls = []

    def on_instruction(machine, address, size, context):
        if address == stubs[116]:
            clip = machine.reg_read(UC_X86_REG_RCX)
            prop = machine.reg_read(UC_X86_REG_RDX)
            value = machine.reg_read(UC_X86_REG_R8)
            if prop == 4123:
                value = struct.unpack("<ff", bytes(machine.mem_read(value, 8)))
            calls.append((clip, prop, value))
            machine.reg_write(UC_X86_REG_RAX, 0)
        elif address == stubs[106]:
            assert machine.reg_read(UC_X86_REG_RDX) == 6
            current = machine.reg_read(UC_X86_REG_RCX)
            machine.reg_write(UC_X86_REG_RAX, 11 if current == 10 else 0xffffffff)

    uc.hook_add(UC_HOOK_CODE, on_instruction)
    uc.emu_start(base + rva, end, count=1000)
    assert calls == [(10, 4123, center), (10, 4126, 1),
                     (11, 4123, center), (11, 4126, 1)], calls
    print(f"PASS: {profile['label']} {width}x{height}, origin {center}, both linked clips")
