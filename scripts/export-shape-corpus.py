"""Export unique untextured GE2D inputs from an already-local inventory.

The corpus is private test data, never a repository or release asset.
"""
import argparse
import hashlib
import json
import struct
from pathlib import Path

from ifstools import IFS

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("inventory", type=Path)
args = parser.parse_args()
root = args.inventory.resolve()
if str(root).startswith("\\\\"):
    raise SystemExit("Use the local analysis copy, not the game share.")
output = root / "shape-corpus"
output.mkdir(exist_ok=True)
geometry = json.loads((root / "geometry.json").read_text(encoding="utf-8"))
seen = set()
manifest = []


def native_endian(data):
    # Exact afp-utils b74a538e... RVA 0x14bf0. Color bytes are not swapped.
    if data[:4] == b"D2EG":
        return data
    if data[:4] != b"GE2D":
        raise ValueError("Unknown GE2D magic")
    result = bytearray(data)
    flags = struct.unpack_from(">I", data, 16)[0]
    vertices, textures, _, labels, draws, _ = struct.unpack_from(">6H", data, 20)
    positions, uv, _, names, records = struct.unpack_from(">5I", data, 32)

    def swap(offset, width):
        result[offset:offset + width] = data[offset:offset + width][::-1]

    for offset in [0, 4, 8, 12, 16, 32, 36, 40, 44, 48]:
        swap(offset, 4)
    for offset in range(20, 32, 2):
        swap(offset, 2)
    if flags & 4:
        for offset in range(52, 68, 4):
            swap(offset, 4)
    for start, count in [(positions, vertices * 2), (uv, textures * 2), (names, labels)]:
        for offset in range(start, start + count * 4, 4):
            swap(offset, 4)
    for index in range(draws):
        offset = records + index * 16
        count = struct.unpack_from(">H", data, offset + 4)[0]
        indices = struct.unpack_from(">I", data, offset + 12)[0]
        swap(offset + 4, 2)
        swap(offset + 12, 4)
        for item in range(indices, indices + count * 2, 2):
            swap(item, 2)
    return bytes(result)


for archive in geometry:
    wanted = {}
    for shape in archive["shapes"]:
        if shape["flags"] == [9] and shape["sha256"] not in seen:
            seen.add(shape["sha256"])
            wanted[shape["name"]] = shape["sha256"]
    if not wanted:
        continue
    source = root / "archives" / archive["archive"]
    if hashlib.sha256(source.read_bytes()).hexdigest() != archive["sha256"]:
        raise ValueError(f"Archive changed since analysis: {source}")
    container = IFS(str(source), super_disable=True)
    try:
        for item in container.tree.all_files:
            if item.name not in wanted or not item.full_path.replace("\\", "/").startswith("geo/"):
                continue
            digest = wanted.pop(item.name)
            data = item.load()
            if hashlib.sha256(data).hexdigest() != digest:
                raise ValueError(f"Geometry changed: {source}/{item.name}")
            native = native_endian(data)
            (output / f"{digest}.geo").write_bytes(native)
            manifest.append(dict(archive=archive["archive"], name=item.name,
                                 sha256=digest, native_sha256=hashlib.sha256(native).hexdigest()))
        if wanted:
            raise ValueError(f"Missing shapes in {source}: {wanted}")
    finally:
        container.file.close()
(output / "manifest.json").write_text(json.dumps(manifest, indent=2), encoding="utf-8")
print(f"Exported {len(manifest)} distinct shapes to {output}")
