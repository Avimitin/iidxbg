"""Verify read-only card-session layouts against exact game binaries.

Requires pefile/capstone and a freshly built iidxbg-tool. No DLL execution.
"""
import argparse
import hashlib
from pathlib import Path
import capstone as cs
from capstone.x86 import X86_OP_IMM, X86_OP_MEM, X86_REG_RIP, X86_REG_RDX
import pefile
from native_profiles import load_profile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("game", type=Path)
args = parser.parse_args()
data = args.game.read_bytes()
profile = load_profile(hashlib.sha256(data).hexdigest())
layout = profile["session"]
assert layout, "no verified session layout"
pe = pefile.PE(data=data)
base = pe.OPTIONAL_HEADER.ImageBase
md = cs.Cs(cs.CS_ARCH_X86, cs.CS_MODE_64)
md.detail = True

def instructions(name, length):
    rva = profile["rvas"][name]
    guard = next(g for g in profile["guards"] if g["rva"] == rva)
    expected = bytes.fromhex(guard["hex"])
    assert pe.get_data(rva, len(expected)) == expected, name
    return list(md.disasm(pe.get_data(rva, length), base + rva))

def rip_targets(code):
    return {i.address + i.size + op.mem.disp - base for i in code
            for op in i.operands if op.type == X86_OP_MEM and op.mem.base == X86_REG_RIP}

def immediates(code):
    return {op.imm for i in code for op in i.operands if op.type == X86_OP_IMM}

init = instructions("session_initialized", 0x1b)
assert {layout["initialized"], layout["initialized"] + 4} <= rip_targets(init)
assert layout["joined"] in rip_targets(instructions("session_joined", 0x1e))
assert layout["joined"] in rip_targets(instructions("session_leave", 0x17))
formatter = instructions("session_card_format", 0x37)
assert layout["identity"] in rip_targets(formatter)
assert layout["stride"] in immediates(formatter)
setter = instructions("session_card_set", 0x220)
root = next(i.address + i.size + i.operands[1].mem.disp - base for i in setter
            if i.mnemonic == "lea" and i.operands[1].type == X86_OP_MEM
            and i.operands[1].mem.base == X86_REG_RIP)
assert layout["stride"] in immediates(setter)
copies = []
for load, store in zip(setter, setter[1:]):
    if load.mnemonic != "movzx" or store.mnemonic != "mov":
        continue
    source, destination = load.operands[1], store.operands[0]
    if source.type == X86_OP_MEM and source.mem.base == X86_REG_RDX and destination.type == X86_OP_MEM:
        copies.append((source.mem.disp, root + destination.mem.disp))
assert copies == [(i, layout["identity"] + 7 - i) for i in range(7, -1, -1)], copies
ready = instructions("session_ready", 0x58)
displacements = {op.mem.disp for i in ready for op in i.operands if op.type == X86_OP_MEM}
assert layout["ready"] - root in displacements
assert layout["identity"] + 24 - root in displacements
assert layout["stride"] in immediates(ready)
clear = instructions("session_clear", 0x37)
start = next(t for t in rip_targets(clear) if t > layout["initialized"])
assert start <= layout["ready"] < start + layout["stride"]
assert layout["stride"] in immediates(clear)
assert layout["identity"] + layout["stride"] + 28 <= profile["image_size"]
print(f"PASS: {profile['label']}: session guards, globals, stride, UID byte order and ready/clear bounds")
