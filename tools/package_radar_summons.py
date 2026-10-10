#!/usr/bin/env python3
"""Package an uninstalled 1.2.0 VMZ preview; never mutate the v1.1 release."""

import argparse
from pathlib import Path
import tomllib
from zipfile import ZIP_DEFLATED, ZipFile, ZipInfo

from build_hotfix_test_kit import replace_once
from package_radar_shots import ROOT, enabled_base

SOURCE = ROOT / "godot-mod" / "rtv-radar-summons"
RANGE = ROOT / "godot-mod" / "rtv-radar-range"
OUTPUT = ROOT / "dist" / "probes" / "RtVRadarSummonsCandidate.vmz"


def package(path: Path) -> None:
    base = enabled_base()
    for flag in ("OVERLAY", "CONTROLS"):
        base = replace_once(
            base,
            f'bool(config.get_value("radar_lite", "{flag.lower()}", {flag}_DEFAULT))',
            f"{flag}_DEFAULT",
        )
    base = replace_once(
        base,
        '\t\t"ai": contacts,',
        '\t\t"summon_actions": ["spawn_airdrop", "spawn_punisher", "spawn_bogeyman"],\n\t\t"ai": contacts,',
    )
    entries = {
        "RtVRadarLite.gd": base,
        "RtVRadarLiteOverlay.gd": (RANGE / "RtVRadarLiteOverlay.gd").read_text(),
        "RtVRadarShotBridge.gd": (RANGE / "RtVRadarShotBridge.gd").read_text(),
        "RtVRadarSummonBridge.gd": (SOURCE / "RtVRadarSummonBridge.gd").read_text(),
        "mod.txt": (SOURCE / "mod.txt").read_text(),
    }
    if '.hook(' in "".join(entries.values()) or "res://Scripts/AI.gd" in entries["mod.txt"]:
        raise ValueError("refusing to package an AI script hook")
    toolkit_version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
    if toolkit_version != "1.2.0" or f'version="{toolkit_version}"' not in entries["mod.txt"]:
        raise ValueError("unreleased VMZ and Toolkit versions differ")
    path.parent.mkdir(parents=True, exist_ok=True)
    with ZipFile(path, "w", compression=ZIP_DEFLATED, compresslevel=9) as archive:
        for name, text in sorted(entries.items()):
            info = ZipInfo(name, (2020, 1, 1, 0, 0, 0))
            info.compress_type = ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            archive.writestr(info, text.encode(), compresslevel=9)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=OUTPUT)
    package(parser.parse_args().output)
