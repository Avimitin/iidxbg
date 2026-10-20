"""Read-only reproduction of the 2026-10-21 background-surface investigation.

Requires pefile and capstone. These are analysis anchors, NOT runtime profiles.
Run with either of the exact DLLs named in docs/*-background-surfaces.md.
"""

import argparse
import hashlib
import json
import struct
from pathlib import Path

import capstone
import pefile


PROFILES = {
    "c61b6dcb8894062e56d60da8ca90053b27f129e1a8e8da5e54457aa42602397d": {
        "label": "IIDX 33 / 20260812",
        "size": 18467992,
        "image_size": 0xBB95000,
        "rect": 0xD88520,
        "lanes": 0xD88530,
        "sprite_vtable": 0xE6C810,
        "sprite_slots": {40: 0x34C240, 72: 0x34C300, 104: 0x34C450,
                         120: 0x34C6A0, 160: 0x34C8A0, 208: 0x34C990},
        "scene_vtable": 0xD84788,
        "scene_slots": {13: 0x8EB820, 14: 0x8EBEB0, 15: 0x8EC1F0},
        "strings": {0xCF1180: b"/1/music_sel_bg.mp4",
                    0xD9FA98: b"lane_sud", 0xD9FAA8: b"lane_lift_sud",
                    0xDC6E30: b"gr_line_max"},
        # Code-window start, byte length, required direct call/tail-jump targets.
        "calls": [
            (0x600F40, 0x300, [0x950510, 0x94FC00]),
            (0x94F7C0, 0x41, [0x94F810]),
            (0x93D930, 0x170, [0x94F7C0]),
            (0x9020A0, 0x400, [0x90ADF0]),
            (0x90ADF0, 0x7E1, [0x8742D0, 0x874330, 0x90B5E0]),
            (0x90B5E0, 0x1F0, [0x34AA90, 0x344080]),
            (0x90B8F0, 0x310, [0x90A1E0, 0x90BC70, 0x90A430]),
            (0x90A430, 0x40A, [0x90BC00]),
            (0x3539C0, 0xC0, [0x340030]),
            (0xA3D650, 0x220, [0xA3D870, 0x874000, 0x90ACD0, 0x82C930]),
            (0xA3C850, 0x250, [0x8322C0, 0x90ACD0, 0x34AC50]),
            (0xA3D980, 0xE0, [0xA3C850]),
        ],
        "stubs": {},
    },
    "3755587f44785effe5008f74cfe7dccbdf423ad09feead48e00a90f78a42fe83": {
        "label": "IIDX 34 / 20260917",
        "size": 18200216,
        "image_size": 0xC5FF000,
        "rect": 0xD56260,
        "lanes": 0xD56270,
        "sprite_vtable": 0xE368C8,
        "sprite_slots": {40: 0x3364C0, 72: 0x336580, 104: 0x3366D0,
                         120: 0x336920, 160: 0x336B20, 208: 0x336C10},
        "scene_vtable": 0xD52508,
        "scene_slots": {16: 0x8D7B70, 17: 0x8D7F60, 18: 0x8D8250},
        "strings": {0xCC5F00: b"/0/music_sel_bg.mp4", 0xD95000: b"gr_line_max"},
        "calls": [
            (0x5FCDE0, 0x27, [0x5F2F50, 0x5F30C0]),
            (0x936270, 0x41, [0x9362C0]),
            (0x9248B0, 0x170, [0x936270]),
            (0x8ECD40, 0x400, [0x8F5AE0]),
            (0x8F5AE0, 0x870, [0x860BF0, 0x860C50, 0x8F6350]),
            (0x8F6350, 0x1F0, [0x334D10, 0x32DA80]),
            (0x8F66E0, 0x310, [0x8F4E50, 0x8F6A60, 0x8F50E0]),
            (0x8F50E0, 0x430, [0x8F69F0]),
            (0x343050, 0xC0, [0x329BB0]),
            (0xA20AF0, 0x220, [0xA20D10, 0x860920, 0x8F59A0, 0x818EE0]),
            (0xA1FCF0, 0x250, [0x81E9F0, 0x8F59A0, 0x334ED0]),
            (0xA20E20, 0xE0, [0xA1FCF0]),
        ],
        "stubs": {0x5F3070: "32c0c3", 0x5F3080: "c20000",
                  0x5F3090: "c20000", 0x5F30A0: "c20000",
                  0x5F30B0: "c20000", 0x5F30C0: "b001c3",
                  0x5F30D0: "c20000"},
    },
}


def require(condition, description):
    if not condition:
        raise ValueError(description)


def verify(path):
    raw = path.read_bytes()
    digest = hashlib.sha256(raw).hexdigest()
    require(digest in PROFILES, f"Unknown DLL SHA-256: {digest}")
    profile = PROFILES[digest]
    pe = pefile.PE(data=raw, fast_load=True)
    base = pe.OPTIONAL_HEADER.ImageBase
    require(base == 0x180000000, "Unexpected preferred image base")
    require(len(raw) == profile["size"], "File size differs")
    require(pe.OPTIONAL_HEADER.SizeOfImage == profile["image_size"], "Image size differs")
    print(f'{profile["label"]}: {digest}')

    rect = struct.unpack("<4i", pe.get_data(profile["rect"], 16))
    require(rect == (0, 0, 1920, 723), "Cover clipping rectangle differs")
    require(pe.get_string_at_rva(profile["lanes"]) == b"lane000", "Lane table differs")
    # 259 is the record stride in the native indexed load, not strlen(name).
    require(pe.get_string_at_rva(profile["lanes"] + 259) == b"lane001",
            "Lane table record stride differs")
    print(f"  Cover clip rectangle: {rect}; lane table stride: 259")

    for table, slots, multiplier in [
        (profile["sprite_vtable"], profile["sprite_slots"], 1),
        (profile["scene_vtable"], profile["scene_slots"], 8),
    ]:
        for slot, target in slots.items():
            actual = struct.unpack("<Q", pe.get_data(table + slot * multiplier, 8))[0]
            require(actual == base + target, f"Vtable mismatch at {table:#x}, slot {slot}")
    for rva, expected in profile["strings"].items():
        require(pe.get_string_at_rva(rva) == expected, f"String mismatch at {rva:#x}")

    md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_64)
    md.detail = True
    for rva, size, expected in profile["calls"]:
        instructions = list(md.disasm(pe.get_data(rva, size), base + rva))
        targets = {
            ins.operands[0].imm - base
            for ins in instructions
            if ins.mnemonic in ("call", "jmp") and ins.operands
            and ins.operands[0].type == capstone.CS_OP_IMM
        }
        for target in expected:
            require(target in targets, f"Missing call/tail jump {rva:#x} -> {target:#x}")
        print(f'  {rva:#x} references {", ".join(hex(t) for t in expected)}')
    for rva, expected in profile["stubs"].items():
        pattern = bytes.fromhex(expected)
        require(pe.get_data(rva, len(pattern)) == pattern, f"Stub differs at {rva:#x}")
    runtime = next(p for file in (Path(__file__).resolve().parent.parent / "src/profiles").glob("*.json")
                   if (p := json.loads(file.read_text()))["sha256"] == digest)
    for guard in runtime["guards"]:
        require(pe.get_data(guard["rva"], 16).hex() == guard["hex"],
                f"Runtime guard differs at {guard['rva']:#x}")
    pe.parse_data_directories(directories=[1])
    imports = {i.ordinal: i.address - base for m in pe.DIRECTORY_ENTRY_IMPORT
               if m.dll.lower() == b"afp-core.dll" for i in m.imports}
    require(imports[21] == runtime["surface_data"]["afp_group_get"], "AFP group getter import")
    require(imports[20] == runtime["surface_data"]["afp_group_set"], "AFP group setter import")
    print(f"  All {len(runtime['guards'])} runtime guards and AFP group imports match")
    print("PASS: static evidence reproduced; no game execution or rendering validation.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("dll", type=Path)
    args = parser.parse_args()
    try:
        verify(args.dll)
    except (OSError, ValueError, pefile.PEFormatError) as exc:
        parser.exit(1, f"FAIL: {exc}\n")
