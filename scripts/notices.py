"""Generate dependency license notices from the exact Cargo.lock dependency tree."""
import json
import pathlib
import subprocess

root = pathlib.Path(__file__).resolve().parent.parent
packages = {}
for manifest in (root / "Cargo.toml", root / "ui/Cargo.toml"):
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--format-version", "1",
         "--manifest-path", str(manifest), "--filter-platform", "x86_64-pc-windows-msvc"], cwd=root))
    for package in metadata["packages"]:
        packages[package["id"]] = package
output = ["iidxbg third-party dependencies\n",
          "Generated from Cargo.lock; includes transitive and platform dependencies.\n",
          "Build/verification and SDK scaffolding adapted from MIT-licensed bilimani.\n"
          "Its copyright and permission notice are retained in LICENSE.\n"]
for package in sorted(packages.values(), key=lambda p: (p["name"], p["version"])):
    if package["name"] in ("iidxbg", "iidxbg-config"):
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
    if package["name"] == "minhook":
        # The Rust wrapper's MIT license does not cover its bundled C library.
        files.add(directory / "minhook" / "LICENSE.txt")
    for path in sorted(files):
        output.append(f"\n--- {path.relative_to(directory)} ---\n"
                      f"{path.read_text(encoding='utf-8', errors='replace')}\n")
output.append((root / "licenses/ifs-reader.txt").read_text("utf-8"))
notice = "\n".join(line.rstrip() for line in "\n".join(output).splitlines()) + "\n"
(root / "THIRD-PARTY-NOTICES.txt").write_text(notice, encoding="utf-8")
print(f"Notices generated for {len(packages) - 2} dependencies and the IFS reader references.")
