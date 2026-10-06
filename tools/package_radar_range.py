#!/usr/bin/env python3
"""Build the uninstalled F9-range / Shot Alerts candidate. NOT a release asset.

Never change the hash-pinned v0.1.12 release packager or published VMZ.
"""

from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile, ZipInfo

from package_radar_shots import ROOT, enabled_base

SOURCE = ROOT / "godot-mod" / "rtv-radar-range"
OUTPUT = ROOT / "dist" / "probes" / "RtVRadarRangeCandidate.vmz"


def main() -> None:
    entries = {
        "RtVRadarLite.gd": enabled_base(),
        "RtVRadarLiteOverlay.gd": (SOURCE / "RtVRadarLiteOverlay.gd").read_text(),
        "RtVRadarShotBridge.gd": (SOURCE / "RtVRadarShotBridge.gd").read_text(),
        "mod.txt": (SOURCE / "mod.txt").read_text(),
    }
    if ".hook(" in entries["RtVRadarShotBridge.gd"] or "res://Scripts/AI.gd" in entries["mod.txt"]:
        raise SystemExit("range preview must not rewrite game scripts")
    if "const RANGES_METRES := [50.0, 100.0, 200.0, 400.0]" not in entries["RtVRadarLiteOverlay.gd"]:
        raise SystemExit("missing reviewed detection ranges")
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    with ZipFile(OUTPUT, "w", compression=ZIP_DEFLATED, compresslevel=9) as archive:
        for filename, text in sorted(entries.items()):
            info = ZipInfo(filename, (2020, 1, 1, 0, 0, 0))
            info.compress_type = ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            archive.writestr(info, text.encode("utf-8"), compresslevel=9)
    print(f"Offline-only range candidate: {OUTPUT}")


if __name__ == "__main__":
    main()
