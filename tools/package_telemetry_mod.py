#!/usr/bin/env python3
"""Build the Road to Vostok telemetry proof mod as a deterministic VMZ archive."""

from __future__ import annotations

import argparse
from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile, ZipInfo

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "godot-mod" / "rtv-telemetry"
DEFAULT_OUTPUT = ROOT / "dist" / "RtVTelemetryProof.vmz"
FIXED_TIMESTAMP = (2020, 1, 1, 0, 0, 0)


def build(output: Path) -> None:
    required = [
        SOURCE / "mod.txt",
        SOURCE / "RtVTelemetryProof" / "Main.gd",
        SOURCE / "RtVTelemetryProof" / "RadarOverlay.gd",
        SOURCE / "RtVTelemetryProof" / "CerebralMultiplier.gd",
        SOURCE / "RtVTelemetryProof" / "CerebralMultiplier.tscn",
    ]
    missing = [path for path in required if not path.is_file()]
    if missing:
        raise SystemExit("missing telemetry mod source: " + ", ".join(map(str, missing)))

    output.parent.mkdir(parents=True, exist_ok=True)
    with ZipFile(output, "w", compression=ZIP_DEFLATED, compresslevel=9) as archive:
        for path in sorted(item for item in SOURCE.rglob("*") if item.is_file()):
            relative = path.relative_to(SOURCE).as_posix()
            info = ZipInfo(relative, FIXED_TIMESTAMP)
            info.compress_type = ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            archive.writestr(info, path.read_bytes(), compresslevel=9)
    print(output)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args()
    build(args.output.resolve())


if __name__ == "__main__":
    main()
