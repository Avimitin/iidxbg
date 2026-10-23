"""Reproduce selection frame-character ABI evidence without running the game.

Requires pefile, capstone and unicorn. Checks exact build identity, scene/frame
offsets, character layer construction, AFP imports and wrapper calling convention.
AFP imports are mocked for the getter/setter emulation.
"""
import argparse
import hashlib
from native_profiles import load_profile
import struct
from pathlib import Path

import capstone
import pefile
from unicorn import Uc, UC_ARCH_X86, UC_MODE_64, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_RAX, UC_X86_REG_RCX, UC_X86_REG_RDX, \
    UC_X86_REG_R8, UC_X86_REG_RSP


def verify(path):
    raw = path.read_bytes()
    digest = hashlib.sha256(raw).hexdigest()
    profile = load_profile(digest)
    pe = pefile.PE(data=raw)
    base = pe.OPTIONAL_HEADER.ImageBase
    assert base == 0x180000000
    if "33" in profile["label"]:
        frame, build, store, factory, clip_init, getter = (
            152, 0x5DD110, 0x5DD40E, 0x344080, 0x340030, 0x341330)
        construct, construct_start = 0x8EB74D, 0x8EB050
        root_store = 0x3400F5
        frame_update = 0x5DE940
        chara_name = 0xCA15A8
    elif profile["sha256"] == "c98e183ec81ecb9e1cb13c237f9c9cb0b3f65b6fdaff6fd5e6a526008c546534":
        frame, build, store, factory, clip_init, getter = (
            160, 0x5CF1F0, 0x5CF4EA, 0x32DA80, 0x329BB0, 0x32ACD0)
        construct, construct_start = 0x8DA46E, 0x8DA400
        root_store = 0x329C0A
        frame_update = 0x5D0A30
        chara_name = 0xC76290
    else:
        frame, build, store, factory, clip_init, getter = (
            160, 0x5CF1B0, 0x5CF4AA, 0x32DA80, 0x329BB0, 0x32ACD0)
        construct, construct_start = 0x8DA42E, 0x8DA3C0
        root_store = 0x329C0A
        frame_update = 0x5D09F0
        chara_name = 0xC76290
    data = profile["surface_data"]
    assert data["select_frame_chara"] == frame + 120
    assert data["select_frame_priority"] == frame + 28
    assert pe.get_string_at_rva(chara_name) == b"frame_chara_01"
    imports = {i.ordinal: i.address for e in pe.DIRECTORY_ENTRY_IMPORT
               if e.dll.lower() == b"afp-core.dll" for i in e.imports}
    for key, ordinal in [("afp_property_get", 115), ("afp_property_set", 116)]:
        assert data[key] == imports[ordinal] - base
    dim = profile["rvas"]["layer_dimensions"]
    assert struct.unpack("<Q", pe.get_data(data["layer_vtable"] + 312, 8))[0] == base + dim
    guard = next(g for g in profile["guards"] if g["rva"] == dim)
    assert pe.get_data(dim, 16).hex() == guard["hex"]
    cs = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_64)
    cs.detail = True

    def instructions(rva, size):
        return list(cs.disasm(pe.get_data(rva, size), base + rva))

    def calls(rva, size):
        return {i.operands[0].imm - base for i in instructions(rva, size)
                if i.mnemonic == "call" and i.operands[0].type == capstone.CS_OP_IMM}

    assert build in calls(construct, 5)
    assert frame_update in calls(profile["rvas"]["select_update"], 0x290)
    assert any(
        (i.mnemonic == "lea" and any(o.type == capstone.CS_OP_MEM and o.mem.disp == frame
                                    for o in i.operands))
        or (i.mnemonic == "add" and i.op_str.startswith("rcx,") and
            i.operands[1].type == capstone.CS_OP_IMM and i.operands[1].imm == frame)
        for i in instructions(construct_start, construct + 5 - construct_start)
    ), "frame constructor offset"
    assert any(i.mnemonic == "lea" and any(
        o.type == capstone.CS_OP_MEM and o.mem.disp == frame for o in i.operands)
        for i in instructions(profile["rvas"]["select_update"], 0x290)), "frame update offset"
    ins = instructions(store, 8)[0]
    assert ins.mnemonic == "mov" and ins.operands[0].mem.disp == 120
    assert factory in calls(build, 0x500)
    assert clip_init in calls(factory, 0x230)
    ins = instructions(root_store, 8)[0]
    assert ins.mnemonic == "mov" and ins.operands[0].mem.disp == 512

    for initial, fail in [(0, False), (1, False), (1, True)]:
        uc = Uc(UC_ARCH_X86, UC_MODE_64)
        mapped = set()

        def map_page(address):
            page = address & ~0xfff
            if page not in mapped:
                uc.mem_map(page, 0x1000)
                uc.mem_write(page, pe.get_data(page - base, 0x1000))
                mapped.add(page)

        setter = profile["rvas"]["set_visible"]
        for rva in [getter, setter, setter + 0x100, dim, dim + 0x80]:
            map_page(base + rva)
        stubs = {115: 0x60000100, 116: 0x60000200, 106: 0x60000300, 71: 0x60000500}
        uc.mem_map(0x60000000, 0x1000)
        for ordinal, stub in stubs.items():
            map_page(imports[ordinal])
            uc.mem_write(imports[ordinal], struct.pack("<Q", stub))
            uc.mem_write(stub, b"\xc3")
        uc.mem_map(0x70000000, 0x10000)
        uc.mem_map(0x71000000, 0x1000)
        uc.mem_write(0x71000200, struct.pack("<I", 42))
        uc.mem_write(0x71000008, struct.pack("<I", 123))
        written = []

        def hook(machine, address, size, context):
            clip = machine.reg_read(UC_X86_REG_RCX)
            prop = machine.reg_read(UC_X86_REG_RDX)
            val = machine.reg_read(UC_X86_REG_R8)
            if address == stubs[115]:
                assert (clip, prop) == (42, 4103)
                machine.mem_write(val, bytes([initial]))
                machine.reg_write(UC_X86_REG_RAX, 0xffffffff if fail else 0)
            elif address == stubs[116]:
                written.append((clip, prop, val))
                machine.reg_write(UC_X86_REG_RAX, 0)
            elif address == stubs[106]:
                assert (clip, prop) == (42, 6)
                machine.reg_write(UC_X86_REG_RAX, 0xffffffff)
            elif address == stubs[71]:
                assert clip == 123
                machine.mem_write(prop, bytes(16) + struct.pack("<hh", 1920, 1080))
                machine.reg_write(UC_X86_REG_RAX, 0)

        uc.hook_add(UC_HOOK_CODE, hook)

        def run(rva, value=0):
            stack, end = 0x7000fef8, 0x60000400
            uc.mem_write(stack, struct.pack("<Q", end))
            uc.reg_write(UC_X86_REG_RSP, stack)
            uc.reg_write(UC_X86_REG_RCX, 0x71000000)
            uc.reg_write(UC_X86_REG_RDX, value)
            uc.emu_start(base + rva, end, count=1000)
            return uc.reg_read(UC_X86_REG_RAX) & 0xff

        assert run(getter) == (0 if fail else initial)
        run(setter, initial)
        assert written == [(42, 4103, initial), (42, 4126, 1)]
        uc.reg_write(UC_X86_REG_R8, 0x71000304)
        run(dim, 0x71000300)
        assert struct.unpack("<ii", bytes(uc.mem_read(0x71000300, 8))) == (1920, 1080)
    print(f"PASS: {profile['label']} ({digest})")
    print("Scene offsets, character factory/root, import ABI and visibility emulation verified.")
    print("Offline evidence only; no live game/rendering validation.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("dll", type=Path)
    verify(parser.parse_args().dll)
