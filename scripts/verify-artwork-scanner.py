"""Compare built-in scanner output with the independent ifstools catalog.

Both inputs are private/local. No extraction or modification of game files.
Native-size atlas previews must match pixel-for-pixel; resized previews and
rasterized vector shapes intentionally use different resampling algorithms.
"""
import argparse
import hashlib
import json
from pathlib import Path
from PIL import Image

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("reference", type=Path)
parser.add_argument("scanned", type=Path)
parser.add_argument("--subset", action="store_true")
parser.add_argument("--report", type=Path)
parser.add_argument("--pixel-limit", type=int, default=2048,
                    help="Evenly sample native-size previews; 0 checks every preview")
args = parser.parse_args()
reference = json.loads(args.reference.read_text("utf-8"))
scanned = json.loads(args.scanned.read_text("utf-8"))
key = lambda entry: (entry["archive"], entry["texture"])
old = {key(e): e for e in reference}
new = {key(e): e for e in scanned}
missing = [] if args.subset else sorted(old.keys() - new.keys())
extra = sorted(new.keys() - old.keys())
dimensions = []
pixels = []
compared = 0
common = sorted(old.keys() & new.keys())
eligible = [k for k in common if not k[0].startswith("@") and not k[1].startswith("@shape/")
            and max(old[k]["width"], old[k]["height"]) <= 256
            and old[k].get("preview") and new[k].get("preview")]
step = max(1, (len(eligible) + args.pixel_limit - 1) // args.pixel_limit) if args.pixel_limit > 0 else 1
sample = set(eligible[::step])
for k in common:
    a, b = old[k], new[k]
    if (a["width"], a["height"]) != (b["width"], b["height"]):
        dimensions.append(k)
    if k not in sample:
        continue
    with Image.open(args.reference.parent / a["preview"]) as ia:
        with Image.open(args.scanned.parent / b["preview"]) as ib:
            if ia.size != ib.size or ia.convert("RGBA").tobytes() != ib.convert("RGBA").tobytes():
                pixels.append(k)
    compared += 1
result = dict(reference_sha256=hashlib.sha256(args.reference.read_bytes()).hexdigest(),
              scanned_sha256=hashlib.sha256(args.scanned.read_bytes()).hexdigest(),
              reference_entries=len(reference), scanned_entries=len(scanned),
              missing=missing, extra=extra, dimensions=dimensions,
              native_previews_compared=compared, pixel_mismatches=pixels)
if args.report:
    args.report.write_text(json.dumps(result, indent=2), "utf-8")
print(json.dumps({k: len(v) if isinstance(v, list) else v for k, v in result.items()}, indent=2))
raise SystemExit(bool(missing or extra or dimensions or pixels))
