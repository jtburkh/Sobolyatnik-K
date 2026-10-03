#!/usr/bin/env python3
"""Build an uninstalled AI-only shot-observer candidate, never a release asset.

The v0.1.8 radar-only sources/VMZ remain byte-identical to the published archive.
Do not install this beside another radar VMZ or without controlled opt-in.
"""

from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile, ZipInfo

ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / "godot-mod" / "rtv-radar-lite" / "RtVRadarLite.gd"
SOURCE = ROOT / "godot-mod" / "rtv-radar-shots"
OUTPUT = ROOT / "dist" / "probes" / "RtVRadarShotsCandidate.vmz"


def enabled_base() -> str:
    script = BASE.read_text()
    for flag in ("CONTACTS", "OVERLAY", "CONTROLS", "LOOT"):
        disabled = f"const {flag}_DEFAULT := false"
        if script.count(disabled) != 1:
            raise SystemExit(f"unexpected radar source flag: {disabled}")
        script = script.replace(disabled, f"const {flag}_DEFAULT := true")
    if ".hook(" in script or "res://Scripts/AI.gd" in script:
        raise SystemExit("refusing a radar candidate with an AI hook")
    return script


def main() -> None:
    entries = {
        "RtVRadarLite.gd": enabled_base(),
        "RtVRadarLiteOverlay.gd": (SOURCE / "RtVRadarLiteOverlay.gd").read_text(),
        "RtVRadarShotBridge.gd": (SOURCE / "RtVRadarShotBridge.gd").read_text(),
        "mod.txt": (SOURCE / "mod.txt").read_text(),
    }
    if ".hook(" in entries["RtVRadarShotBridge.gd"] or "res://Scripts/AI.gd" in entries["mod.txt"]:
        raise SystemExit("shot candidate must not rewrite game scripts")
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    with ZipFile(OUTPUT, "w", compression=ZIP_DEFLATED, compresslevel=9) as archive:
        for filename, text in sorted(entries.items()):
            info = ZipInfo(filename, (2020, 1, 1, 0, 0, 0))
            info.compress_type = ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            archive.writestr(info, text.encode("utf-8"), compresslevel=9)
    print(f"Offline-only candidate: {OUTPUT}")


if __name__ == "__main__":
    main()
