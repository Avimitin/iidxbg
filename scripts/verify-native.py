"""Read-only IIDX 33/34 results-background evidence checks.

Requires pefile and capstone. Does not load the DLL, render, or access the network.
See docs/feasibility.md for interpretation and limitations.
"""

import argparse
import hashlib
from native_profiles import load_profile
import struct
from pathlib import Path

import capstone
import pefile


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("dll", type=Path)
    args = parser.parse_args()
    data = args.dll.read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    profile = load_profile(digest)
    pe = pefile.PE(data=data)
    base = pe.OPTIONAL_HEADER.ImageBase
    require(base == 0x180000000 and pe.FILE_HEADER.Machine == 0x8664,
            "Unexpected image base or architecture")
    md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_64)
    md.detail = True
    require(pe.OPTIONAL_HEADER.SizeOfImage == profile["image_size"], "Image size")
    print(profile["label"])
    for guard in profile["guards"]:
        expected = bytes.fromhex(guard["hex"])
        require(pe.get_data(guard["rva"], len(expected)) == expected,
                f"Native guard mismatch: {guard['rva']:x}")
    for item in profile["tables"]:
        actual = struct.unpack("<Q", pe.get_data(item["rva"] + item["slot"] * 8, 8))[0]
        require(actual == base + item["target"], "Runtime hook slot mismatch")
    print("PASS: every runtime native-entry and hook-slot guard")

    def u32(rva):
        return struct.unpack("<I", pe.get_data(rva, 4))[0]

    def pointer(rva):
        return struct.unpack("<Q", pe.get_data(rva, 8))[0] - base

    def table(rva, name, entries):
        descriptor = u32(pointer(rva - 8) + 12)
        encoded = name.encode() + b"\0"
        require(pe.get_data(descriptor + 16, len(encoded)) == encoded,
                f"Unexpected RTTI at {rva:x}")
        for slot, target in entries.items():
            require(pointer(rva + slot * 8) == target,
                    f"Unexpected slot {slot} at {rva:x}")

    def function(rva):
        entry = next(e.struct for e in pe.DIRECTORY_ENTRY_EXCEPTION
                     if e.struct.BeginAddress == rva)
        return list(md.disasm(pe.get_data(rva, entry.EndAddress - rva), base + rva))

    def calls(rva):
        return {i.operands[0].imm - base for i in function(rva)
                if i.mnemonic == "call" and i.operands[0].type == capstone.CS_OP_IMM}

    def callsite(rva, target):
        insn = next(md.disasm(pe.get_data(rva, 5), base + rva))
        require(insn.mnemonic == "call"
                and insn.operands[0].type == capstone.CS_OP_IMM
                and insn.operands[0].imm == base + target,
                f"Unexpected direct call at {rva:x}")

    if profile["sha256"] == "c61b6dcb8894062e56d60da8ca90053b27f129e1a8e8da5e54457aa42602397d":
        table(0xDA4800, ".?AVCStageResultScene@@",
              {7: 0x92ADD0, 13: 0x92C2D0, 14: 0x92D970, 15: 0x92DF20, 24: 0x92DE20})
        table(0xDA2188, ".?AVStageResultDrawBg@@",
              {0: 0x91BF80, 1: 0x91BFB0, 2: 0x91C120})
        table(0xDCBA00, ".?AVAfpTextureEx@@", {})
        require(profile["layout"] == dict(scene_vtable=0xDA4800, scene_background=256,
                                          background_archive=272, background_movie=296),
                "IIDX 33 layout profile")
        for site, target in [(0x929ADD, 0x91BD90), (0x91C284, 0x8322C0),
                             (0x91C2A7, 0x340520), (0x91C2B7, 0x340A50),
                             (0x91C2BE, 0x3407B0)]:
            callsite(site, target)
        # Constructor passes scene +256 to the background constructor.
        insn = next(md.disasm(pe.get_data(0x929AD6, 7), base + 0x929AD6))
        require(insn.mnemonic == "lea" and insn.operands[1].mem.disp == 256,
                "IIDX 33 background offset")
        strings = set()
        for insn in function(0x91C120):
            for op in insn.operands:
                if op.type == capstone.CS_OP_MEM and op.mem.base == capstone.x86.X86_REG_RIP:
                    rva = insn.address + insn.size + op.mem.disp - base
                    strings.add(pe.get_data(rva, 32).split(b"\0")[0])
        require({b"illust", b"result_bg"} <= strings, "IIDX 33 binding strings")
        require({0x355650, 0x355990} <= calls(0xA61CA0), "Wrapper registration")
        require(0x355720 in calls(0xA61E30), "Wrapper unregister")
        require(pe.get_data(0xA62140, 5) == bytes.fromhex("488d4118c3"), "Descriptor +24")
        require(0x92A280 in calls(0x92ADD0), "Deleting destructor")
        bind_import = next(i.address for e in pe.DIRECTORY_ENTRY_IMPORT
                           for i in e.imports if e.dll.lower() == b"afp-core.dll"
                           and i.ordinal == 137)
        for rva in (0x340440, 0x340520):
            require(any(i.mnemonic == "call" and i.operands[0].type == capstone.CS_OP_MEM
                        and i.operands[0].mem.base == capstone.x86.X86_REG_RIP
                        and i.address + i.size + i.operands[0].mem.disp == bind_import
                        for i in function(rva)), "AFP binding import")
        print("PASS: IIDX 33 RTTI, scene ownership, result binding, texture lifecycle")
        print("Offline evidence only: no hook, GPU upload, layout or frame-time validation.")
        return

    if profile["sha256"] == "c98e183ec81ecb9e1cb13c237f9c9cb0b3f65b6fdaff6fd5e6a526008c546534":
        table(0xD72950, ".?AVCStageResultScene@@",
              {16: 0x9140C0, 17: 0x9155C0, 18: 0x915B50, 27: 0x915A50})
        table(0xD70598, ".?AVStageResultDrawBg@@",
              {0: 0x906D70, 1: 0x906DA0, 2: 0x906EB0})
        table(0xD9B358, ".?AVAfpTextureEx@@", {0: 0xA4FB20})
        require(profile["layout"] == dict(scene_vtable=0xD72950, scene_background=264,
                                          background_archive=272, background_movie=296),
                "IIDX 34 / 010-0917 layout profile")
        for site, target in [(0x911F15, 0x906B80),
                             (0x907014, 0x81EA30), (0x907037, 0x329F30),
                             (0x907047, 0x32A460), (0x90704E, 0x32A1C0),
                             (0xA4F9C9, 0x344B60), (0xA4FA35, 0x344EA0),
                             (0x63DAAD, 0x329E50)]:
            callsite(site, target)
        for site, target, value in [(0x90700A, 0xD70620, b"illust\0"),
                                    (0x907030, 0xCA56F0, b"result_bg\0")]:
            insn = next(md.disasm(pe.get_data(site, 7), base + site))
            operand = insn.operands[1]
            require(insn.mnemonic == "lea" and operand.type == capstone.CS_OP_MEM
                    and operand.mem.base == capstone.x86.X86_REG_RIP
                    and insn.address + insn.size + operand.mem.disp == base + target,
                    f"Unexpected string reference at {site:x}")
            require(pe.get_data(target, len(value)) == value, "Unexpected string")
        require(pe.get_data(0x911F0E, 7) == bytes.fromhex("488d8e08010000"),
                "Background no longer at scene +264")
        require(pe.get_data(0x907021, 7) == bytes.fromhex("498b9510010000"),
                "Archive resource no longer at background +272")
        require({0x620B10, 0x818F20} <= calls(0x906B80),
                "Background archive construction/loading path changed")
        require({0xA4FD80, 0x329E50} <= calls(0x63DA20),
                "Existing descriptor binding example changed")
        require({0x344B60, 0x344EA0} <= calls(0xA4F8E0),
                "Texture wrapper no longer registers and builds a descriptor")
        require(0x344C30 in calls(0xA4FA70),
                "Texture destructor no longer unregisters its handle")
        getter = list(md.disasm(pe.get_data(0xA4FD80, 5), base + 0xA4FD80))
        require(getter[0].mnemonic == "lea"
                and getter[0].operands[1].type == capstone.CS_OP_MEM
                and getter[0].operands[1].mem.disp == 24
                and getter[1].mnemonic == "ret", "Descriptor getter changed")
        for rva in (0x329E50, 0x329F30):
            require(any(i.mnemonic == "call"
                        and i.operands[0].type == capstone.CS_OP_MEM
                        and i.operands[0].mem.base == capstone.x86.X86_REG_RIP
                        and i.address + i.size + i.operands[0].mem.disp == base + 0xC66CA8
                        for i in function(rva)), "AFP texture binding import changed")
        # Premium-area and concentration backgrounds.
        require(pe.get_data(0x8F94D8, 4) == bytes.fromhex("4c8b7148"),
                "Concentration window no longer at CPremiumArea +72")
        for site, target in [(0x8F9470, 0x9AF8C0), (0x8F94BF, 0x9AF8C0),
                             (0x8F9523, 0x860680), (0x8F952E, 0x6375C0),
                             (0x8F9570, 0x9AEEF0), (0x994AEB, 0x6447A0)]:
            callsite(site, target)
        require(pe.get_data(0x9ACF00, 14) == bytes.fromhex("488b49484885c90f85e3a8c8ffc3"),
                "Concentration show wrapper changed")
        require({0x9C1EE0, 0x334D10} <= calls(0x6447A0),
                "Premium-area background construction path changed")
        require(0x638420 in calls(0x6375C0),
                "Concentration clip lookup path changed")
        require(struct.unpack("<4i", pe.get_data(0xD8BBF0, 16)) == (0, 1080, 1280, 720),
                "Native 1280x720 background clip changed")
        for rva, text in [(0xC8DF78, b"sub_customize_bg%03d"),
                          (0xC8DFE0, b"concentration_bg_set/bg"),
                          (0xC8FA98, b"sub_monitor_base_bg")]:
            require(pe.get_data(rva, len(text)) == text, "Background string changed")
        require(profile["surface_data"]["premium_area_base"] == 16
                and profile["surface_data"]["premium_area_custom"] == 24,
                "PremiumAreaBG field offsets changed")
        print("PASS: IIDX 34 / 010-0917 RTTI, scene ownership, binding, texture lifecycle")
        print("PASS: premium-area and concentration background construction/show paths")
        print("Offline evidence only: no hook, GPU upload, layout or frame-time validation.")
        return

    table(0xD72950, ".?AVCStageResultScene@@",
          {16: 0x914080, 17: 0x915580, 18: 0x915B10, 27: 0x915A10})
    table(0xD70598, ".?AVStageResultDrawBg@@",
          {0: 0x906D30, 1: 0x906D60, 2: 0x906E70})
    table(0xD9B358, ".?AVAfpTextureEx@@", {})
    print("PASS: results scene, background component and dynamic texture RTTI/vtables")

    for site, target in [(0x911ED5, 0x906B40),
                         (0x906FD4, 0x81E9F0), (0x906FF7, 0x329F30),
                         (0x907007, 0x32A460), (0x90700E, 0x32A1C0),
                         (0xA4F989, 0x344B60), (0xA4F9F5, 0x344EA0),
                         (0x63DA6D, 0x329E50)]:
        callsite(site, target)
    for site, target, value in [(0x906FCA, 0xD70620, b"illust\0"),
                                (0x906FF0, 0xCA56F0, b"result_bg\0")]:
        insn = next(md.disasm(pe.get_data(site, 7), base + site))
        operand = insn.operands[1]
        require(insn.mnemonic == "lea" and operand.type == capstone.CS_OP_MEM
                and operand.mem.base == capstone.x86.X86_REG_RIP
                and insn.address + insn.size + operand.mem.disp == base + target,
                f"Unexpected string reference at {site:x}")
        require(pe.get_data(target, len(value)) == value, "Unexpected string")
    require(pe.get_data(0x911ECE, 7) == bytes.fromhex("488d8e08010000"),
            "Background no longer at scene +264")
    require(pe.get_data(0x906FE1, 7) == bytes.fromhex("498b9510010000"),
            "Archive resource no longer at background +272")
    print("PASS: scene owns background; illust binds result_bg or is hidden")

    require({0x620AD0, 0x818EE0} <= calls(0x906B40),
            "Background archive construction/loading path changed")
    require({0xA4FD40, 0x329E50} <= calls(0x63D9E0),
            "Existing descriptor binding example changed")
    require({0x344B60, 0x344EA0} <= calls(0xA4F8A0),
            "Texture wrapper no longer registers and builds a descriptor")
    require(0x344C30 in calls(0xA4FA30),
            "Texture destructor no longer unregisters its handle")
    # This leaf has no unwind/pdata entry; decode its known five-byte body.
    getter = list(md.disasm(pe.get_data(0xA4FD40, 5), base + 0xA4FD40))
    require(getter[0].mnemonic == "lea"
            and getter[0].operands[1].type == capstone.CS_OP_MEM
            and getter[0].operands[1].mem.disp == 24
            and getter[1].mnemonic == "ret", "Descriptor getter changed")
    for rva in (0x329E50, 0x329F30):
        require(any(i.mnemonic == "call"
                    and i.operands[0].type == capstone.CS_OP_MEM
                    and i.operands[0].mem.base == capstone.x86.X86_REG_RIP
                    and i.address + i.size + i.operands[0].mem.disp == base + 0xC66CA8
                    for i in function(rva)), "AFP texture binding import changed")
    print("PASS: archive and dynamic-descriptor setters share afp_core_137")
    print("Offline evidence only: no hook, GPU upload, layout or frame-time validation.")


if __name__ == "__main__":
    main()
