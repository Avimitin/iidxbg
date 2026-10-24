"""Copy loose game artwork locally, hash it and append previews to the catalog."""
import argparse
import hashlib
import json
import shutil
from pathlib import Path
from PIL import Image

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("source", type=Path)
parser.add_argument("inventory", type=Path)
args = parser.parse_args()
source, root = args.source.resolve(), args.inventory.resolve()
if str(root).startswith("\\\\") or root == source or source in root.parents:
    raise SystemExit("Use a local analysis directory outside the game source.")
catalog_path = root / "catalog.json"
catalog = [entry for entry in json.loads(catalog_path.read_text(encoding="utf-8"))
           if entry["archive"] != "@file"]
manifest = []
for path in sorted(source.rglob("*")):
    if not path.is_file() or path.suffix.lower() not in (".png", ".jpg", ".jpeg", ".bmp", ".dds"):
        continue
    relative = path.relative_to(source)
    local = root / "loose" / relative
    local.parent.mkdir(parents=True, exist_ok=True)
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    if not local.exists() or hashlib.sha256(local.read_bytes()).hexdigest() != digest:
        shutil.copy2(path, local)
    if hashlib.sha256(local.read_bytes()).hexdigest() != digest:
        raise RuntimeError(f"Source changed during copy: {relative}")
    record = dict(path=relative.as_posix(), sha256=digest, bytes=local.stat().st_size)
    try:
        with Image.open(local) as original:
            width, height = original.size
            record.update(width=width, height=height)
            preview = original.convert("RGBA")
            preview.thumbnail((256, 256))
            name = "thumbnails/loose-" + digest + ".png"
            preview.save(root / name, compress_level=2)
            catalog.append(dict(archive="@file", texture=relative.as_posix(),
                                width=width, height=height, preview=name))
    except Exception as error:
        record["error"] = str(error)
    manifest.append(record)
(root / "loose-inventory.json").write_text(json.dumps(manifest, indent=2), encoding="utf-8")
catalog.sort(key=lambda entry: (entry["archive"], entry["texture"]))
catalog_path.write_text(json.dumps(catalog, ensure_ascii=False, separators=(",", ":")), encoding="utf-8")
print(f"{len(manifest)} loose files, {sum('error' in r for r in manifest)} errors; {len(catalog)} catalog entries")
