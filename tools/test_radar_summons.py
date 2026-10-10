#!/usr/bin/env python3
"""Test the summons VMZ (including exact release bytes) in a disposable Godot fixture."""

import argparse
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
from zipfile import ZipFile

from package_radar_summons import OUTPUT, ROOT, package


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--engine", type=Path, required=True)
    parser.add_argument("--player-bundle", type=Path, help="Test the exact VMZ from a Windows player bundle")
    args = parser.parse_args()
    if args.player_bundle is None:
        package(OUTPUT)
    bundle = args.player_bundle if args.player_bundle is not None else OUTPUT
    with tempfile.TemporaryDirectory(prefix="rtv-summon-mock-") as work:
        project = Path(work)
        (project / "project.godot").write_text('[application]\nconfig/name="Summon Mock"\n')
        with ZipFile(bundle) as archive:
            if set(archive.namelist()) != {
                "RtVRadarLite.gd", "RtVRadarLiteOverlay.gd", "RtVRadarShotBridge.gd",
                "RtVRadarSummonBridge.gd", "mod.txt",
            }:
                raise ValueError("unexpected VMZ contents")
            for name in archive.namelist():
                if name.endswith(".gd"):
                    (project / name).write_bytes(archive.read(name))
        shutil.copytree(ROOT / "tests/telemetry-diagnostic/Resources", project / "Resources")
        (project / "Scripts").mkdir()
        shutil.copyfile(ROOT / "tests/telemetry-diagnostic/Scripts/MockGameData.gd", project / "Scripts/MockGameData.gd")
        shutil.copytree(ROOT / "tests/radar-summons/Scripts", project / "Scripts", dirs_exist_ok=True)
        shutil.copyfile(ROOT / "tests/radar-summons/summon-smoke.gd", project / "summon-smoke.gd")
        env = os.environ.copy()
        env["XDG_DATA_HOME"] = str(project / "user-data")
        result = subprocess.run(
            [str(args.engine.resolve()), "--headless", "--path", str(project), "--script", "res://summon-smoke.gd"],
            capture_output=True, text=True, timeout=40, check=False, env=env,
        )
        output = result.stdout + result.stderr
        print(output)
        if result.returncode != 0 or "SCRIPT ERROR" in output or "ERROR:" in output or "SUMMON SMOKE OK" not in output:
            raise SystemExit(result.returncode or 1)


if __name__ == "__main__":
    main()
