"""Generate dependency license notices from the exact Cargo.lock dependency tree."""
import json
import pathlib
import subprocess

root = pathlib.Path(__file__).resolve().parent.parent
metadata = json.loads(subprocess.check_output(
    ["cargo", "metadata", "--locked", "--format-version", "1"], cwd=root))
output = ["iidxbg third-party dependencies\n",
          "Generated from Cargo.lock; includes transitive and platform dependencies.\n",
          "Build/verification and SDK scaffolding adapted from MIT-licensed bilimani.\n"
          "Its copyright and permission notice are retained in LICENSE.\n"]
for package in sorted(metadata["packages"], key=lambda p: (p["name"], p["version"])):
    if package["name"] == "iidxbg":
        continue
    directory = pathlib.Path(package["manifest_path"]).parent
    output.append(f"\n{'=' * 72}\n{package['name']} {package['version']}\n"
                  f"License: {package.get('license')}\n"
                  f"Source: {package.get('repository') or package.get('source')}\n")
    files = set()
    for pattern in ("LICENSE*", "LICENCE*", "COPYING*", "NOTICE*",
                    "license*", "licence*", "copying*", "notice*"):
        files.update(p for p in directory.glob(pattern) if p.is_file())
    if package.get("license_file"):
        files.add(directory / package["license_file"])
    for path in sorted(files):
        output.append(f"\n--- {path.name} ---\n{path.read_text(encoding='utf-8', errors='replace')}\n")
(root / "THIRD-PARTY-NOTICES.txt").write_text("\n".join(output), encoding="utf-8")
print(f"Notices generated for {len(metadata['packages']) - 1} dependencies.")
