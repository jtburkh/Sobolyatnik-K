#!/usr/bin/env python3
"""Exercise an AI-only shot-observer candidate in a disposable Godot project.

This does not touch Road to Vostok, its installation, or private saves.
"""

import argparse
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
from zipfile import ZipFile

from package_radar_shots import ROOT, SOURCE, enabled_base
from package_radar_shots_release import OUTPUT as RELEASE_ARCHIVE


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--engine", required=True, type=Path)
    parser.add_argument("--release-archive", action="store_true", help="exercise the actual versioned VMZ payload")
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="rtv-radar-shots-mock-") as work:
        project = Path(work)
        (project / "project.godot").write_text('[application]\nconfig/name="AI Shot Observer Mock"\n')
        if args.release_archive:
            with ZipFile(RELEASE_ARCHIVE) as archive:
                for name in ("RtVRadarLite.gd", "RtVRadarShotBridge.gd", "RtVRadarLiteOverlay.gd"):
                    (project / name).write_bytes(archive.read(name))
        else:
            (project / "RtVRadarLite.gd").write_text(enabled_base())
            shutil.copyfile(SOURCE / "RtVRadarShotBridge.gd", project / "RtVRadarShotBridge.gd")
            shutil.copyfile(SOURCE / "RtVRadarLiteOverlay.gd", project / "RtVRadarLiteOverlay.gd")
        shutil.copytree(ROOT / "tests" / "telemetry-diagnostic" / "Resources", project / "Resources")
        scripts = project / "Scripts"
        scripts.mkdir()
        shutil.copyfile(ROOT / "tests" / "telemetry-diagnostic" / "Scripts" / "MockGameData.gd", scripts / "MockGameData.gd")
        shutil.copyfile(ROOT / "tests" / "radar-lite" / "Scripts" / "MockVariant.gd", scripts / "MockVariant.gd")
        for script in (ROOT / "tests" / "radar-shots" / "Scripts").glob("*.gd"):
            shutil.copyfile(script, scripts / script.name)
        shutil.copyfile(ROOT / "tests" / "radar-shots" / "shot-smoke.gd", project / "shot-smoke.gd")
        env = os.environ.copy()
        env["XDG_DATA_HOME"] = str(project / "user-data")
        for no_loot in (False, True):
            command = [str(args.engine.resolve()), "--headless", "--path", str(project), "--script", "res://shot-smoke.gd"]
            if no_loot:
                command.extend(("--", "--no-loot"))
            try:
                result = subprocess.run(
                    command, capture_output=True, text=True, check=False, timeout=30, env=env,
                )
            except subprocess.TimeoutExpired as error:
                print((error.stdout or b"").decode(errors="replace"))
                print((error.stderr or b"").decode(errors="replace"))
                raise SystemExit("Godot shot smoke timed out") from error
            output = result.stdout + result.stderr
            print(output)
            if result.returncode != 0 or "SCRIPT ERROR" in output or "ERROR:" in output:
                raise SystemExit(result.returncode or 1)
            markers = (
                "SHOT SMOKE OK: AI fire sound observed once at its muzzle",
                "SHOT SMOKE OK: enemy, Nomad and boss AI included",
                "SHOT SMOKE OK: explosion, tail, distant, player/vehicle sounds rejected",
                "SHOT SMOKE OK: five-second fade and scene-local expiration",
                "SHOT SMOKE OK: Shot Alerts is the first visible mode by default",
                "SHOT SMOKE OK: F7 cycles from default Shot Alerts through all existing layers, F8 and fade",
            )
            for marker in markers:
                if marker not in output:
                    raise SystemExit(f"missing smoke assertion: {marker}")


if __name__ == "__main__":
    main()
