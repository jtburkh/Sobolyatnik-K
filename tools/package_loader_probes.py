#!/usr/bin/env python3
"""Build uninstalled minimal VMZ controls for RtV loader/autoload isolation.

These contain no telemetry and are deliberately excluded from releases.
"""

from __future__ import annotations

import argparse
from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile, ZipInfo

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "godot-mod" / "rtv-loader-probes"
FIXED_TIMESTAMP = (2020, 1, 1, 0, 0, 0)
PROBES = {
    "manifest-only": "RtVLoaderBaseline.vmz",
    "inert-autoload": "RtVInertAutoload.vmz",
}


def build(source: Path, output: Path) -> None:
    if not (source / "mod.txt").is_file():
        raise SystemExit(f"missing probe manifest: {source / 'mod.txt'}")
    output.parent.mkdir(parents=True, exist_ok=True)
    with ZipFile(output, "w", compression=ZIP_DEFLATED, compresslevel=9) as archive:
        for path in sorted(item for item in source.rglob("*") if item.is_file()):
            info = ZipInfo(path.relative_to(source).as_posix(), FIXED_TIMESTAMP)
            info.compress_type = ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            archive.writestr(info, path.read_bytes(), compresslevel=9)
    print(output)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, default=ROOT / "dist" / "probes")
    args = parser.parse_args()
    for folder, filename in PROBES.items():
        build(SOURCE / folder, args.output_dir.resolve() / filename)


if __name__ == "__main__":
    main()
