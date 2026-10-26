"""Write flatpak/cargo-sources.json from Cargo.lock, so flatpak-builder can
fetch every crate up front (Flathub builds have no network access).

usage: python3 flatpak/gen-cargo-sources.py   (run after Cargo.lock changes)

Same output format as flatpak-builder-tools' flatpak-cargo-generator, limited
to what this project needs: crates.io dependencies only.
"""
import json
import pathlib
import sys
import tomllib

ROOT = pathlib.Path(__file__).resolve().parent.parent
CRATES_IO = "registry+https://github.com/rust-lang/crates.io-index"

lock = tomllib.loads((ROOT / "Cargo.lock").read_text())
sources = []
for pkg in lock["package"]:
    source = pkg.get("source")
    if source is None:
        continue  # this project itself
    if source != CRATES_IO:
        sys.exit(f"unsupported source for {pkg['name']}: {source}")
    name, version, checksum = pkg["name"], pkg["version"], pkg["checksum"]
    dest = f"cargo/vendor/{name}-{version}"
    sources.append({
        "type": "archive",
        "archive-type": "tar-gzip",
        "url": f"https://static.crates.io/crates/{name}/{name}-{version}.crate",
        "sha256": checksum,
        "dest": dest,
    })
    sources.append({
        "type": "inline",
        "contents": json.dumps({"package": checksum, "files": {}}),
        "dest": dest,
        "dest-filename": ".cargo-checksum.json",
    })

sources.append({
    "type": "inline",
    "contents": (
        '[source.vendored-sources]\ndirectory = "cargo/vendor"\n\n'
        '[source.crates-io]\nreplace-with = "vendored-sources"\n'
    ),
    "dest": "cargo",
    "dest-filename": "config.toml",
})

out = ROOT / "flatpak" / "cargo-sources.json"
out.write_text(json.dumps(sources, indent=2) + "\n")
print(f"{out.relative_to(ROOT)}: {len(sources)} sources")
