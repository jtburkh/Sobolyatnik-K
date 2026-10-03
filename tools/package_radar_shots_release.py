#!/usr/bin/env python3
"""Build the separately versioned Windows Shot Alerts VMZ from tested sources.

Never overwrite the v0.1.8/v0.1.11 radar archive or publish this output without
Windows fixture and controlled Build 2 gameplay validation.
"""

from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile, ZipInfo

from package_radar_shots import ROOT, SOURCE, enabled_base

OUTPUT = ROOT / "dist" / "probes" / "RtVRadarShotsRelease.vmz"


def main() -> None:
    entries = {
        "RtVRadarLite.gd": enabled_base(),
        "RtVRadarLiteOverlay.gd": (SOURCE / "RtVRadarLiteOverlay.gd").read_text(),
        "RtVRadarShotBridge.gd": (SOURCE / "RtVRadarShotBridge.gd").read_text(),
        "mod.txt": (SOURCE / "mod-release.txt").read_text(),
    }
    if 'id="rtv_toolkit_radar_loot"' not in entries["mod.txt"] or 'version="0.1.12"' not in entries["mod.txt"]:
        raise SystemExit("unexpected release manifest")
    if ".hook(" in entries["RtVRadarShotBridge.gd"] or "res://Scripts/AI.gd" in entries["mod.txt"]:
        raise SystemExit("refusing an AI hook or script override")
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    with ZipFile(OUTPUT, "w", compression=ZIP_DEFLATED, compresslevel=9) as archive:
        for filename, text in sorted(entries.items()):
            info = ZipInfo(filename, (2020, 1, 1, 0, 0, 0))
            info.compress_type = ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            archive.writestr(info, text.encode("utf-8"), compresslevel=9)
    print(f"Versioned Shot Alerts VMZ: {OUTPUT}")


if __name__ == "__main__":
    main()
