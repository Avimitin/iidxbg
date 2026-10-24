"""Cross-check geometry region references against a local artwork inventory.

Unresolved references are evidence gaps, not proof of a live missing texture.
This reads analysis JSON only; it never opens or modifies game files.
"""
import argparse
import collections
import hashlib
import json
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("inventory", type=Path)
args = parser.parse_args()
root = args.inventory.resolve()
inputs = {}
for name in ("inventory.json", "geometry.json"):
    data = (root / name).read_bytes()
    inputs[name] = {"sha256": hashlib.sha256(data).hexdigest(),
                    "data": json.loads(data)}
inventory = inputs["inventory.json"]["data"]
geometry = inputs["geometry.json"]["data"]
owners = collections.defaultdict(list)
local_names = {}
for archive in inventory:
    names = {item["name"] for item in archive.get("textures", [])}
    local_names[archive["archive"]] = names
    for name in names:
        owners[name].append(archive["archive"])

counts = collections.Counter()
external = []
unresolved = []
for archive in geometry:
    names = {name for shape in archive["shapes"] for name in shape["regions"]}
    for name in sorted(names):
        counts["archive_local_references"] += 1
        if name in local_names.get(archive["archive"], set()):
            counts["same_archive"] += 1
        elif name in owners:
            counts["other_archive_candidate"] += 1
            external.append({"archive": archive["archive"], "region": name,
                             "candidates": sorted(owners[name])})
        else:
            counts["unresolved"] += 1
            unresolved.append({"archive": archive["archive"], "region": name})

report = {
    "input_sha256": {name: value["sha256"] for name, value in inputs.items()},
    "counts": dict(counts),
    "unresolved_distinct_names": len({item["region"] for item in unresolved}),
    "unreadable_archives": [
        {key: archive[key] for key in ("archive", "sha256", "bytes", "error")}
        for archive in inventory if "error" in archive
    ],
    "other_archive_candidates": external,
    "unresolved": unresolved,
    "interpretation": "Name matches in another archive are candidates, not verified native bindings. "
                      "Unresolved names may be dynamic or unused; do not infer either without runtime evidence.",
}
(root / "coverage-audit.json").write_text(json.dumps(report, indent=2), encoding="utf-8")
print(json.dumps({key: report[key] for key in ("counts", "unresolved_distinct_names")}, indent=2))
