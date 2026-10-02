#!/usr/bin/env python3
"""Offline Godot UDP smoke test of both radar-lite VMZ variants (no RTV files)."""

from __future__ import annotations

import argparse
import os
import re
import shutil
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--engine", type=Path, required=True, help="Local Godot executable")
    args = parser.parse_args()
    source = (ROOT / "godot-mod/rtv-radar-lite/RtVRadarLite.gd").read_text()
    overlay_source = (ROOT / "godot-mod/rtv-radar-lite/RtVRadarLiteOverlay.gd").read_text()
    original_overlay = (ROOT / "godot-mod/rtv-telemetry/RtVTelemetryProof/RadarOverlay.gd").read_text()
    for palette_name in ("BACKGROUND", "BORDER", "GRID", "TEXT", "MUTED", "PLAYER", "NOMAD", "LOOT", "LOCKED_LOOT", "CORPSE_LOOT"):
        pattern = rf"^const {palette_name} := .+$"
        current = re.search(pattern, overlay_source, re.MULTILINE)
        original = re.search(pattern, original_overlay, re.MULTILINE)
        if current is None or original is None or current.group() != original.group():
            raise SystemExit(f"Radar palette {palette_name} drifted from original")
    if "draw_rect(Rect2(point - Vector2(2.5, 2.5), Vector2(5, 5)), color, false, 1.2)" not in overlay_source:
        raise SystemExit("Loot squares must match the original 5px outline")
    if '"SOBOLYATNIK-K", HORIZONTAL_ALIGNMENT_LEFT, -1, 13' not in overlay_source:
        raise SystemExit("Sobolyatnik title typography regressed")
    if '"1L108K", HORIZONTAL_ALIGNMENT_LEFT, -1, 10' not in overlay_source:
        raise SystemExit("GRAU index missing from compact radar HUD")
    default = "const CONTACTS_DEFAULT := false"
    if source.count(default) != 1 or "func _input" in source or ".hook(" in source:
        raise SystemExit("Lite bridge is no longer isolated: inspect input/hooks/contact gating")
    contacts = source.replace(default, "const CONTACTS_DEFAULT := true")
    in_game = contacts.replace("const OVERLAY_DEFAULT := false", "const OVERLAY_DEFAULT := true")
    controls = in_game.replace("const CONTROLS_DEFAULT := false", "const CONTROLS_DEFAULT := true")
    variants = {
        "player-only": source,
        "contacts": contacts,
        "in-game": in_game,
        "controls": controls,
        "loot": controls.replace("const LOOT_DEFAULT := false", "const LOOT_DEFAULT := true"),
    }
    with tempfile.TemporaryDirectory(prefix="rtv-radar-lite-") as work:
        for variant, script_source in variants.items():
            project = Path(work) / variant
            project.mkdir()
            (project / "project.godot").write_text('[application]\nconfig/name="RtV Radar Lite Smoke"\n')
            (project / "RtVRadarLite.gd").write_text(script_source)
            if variant in ("in-game", "controls", "loot"):
                shutil.copyfile(ROOT / "godot-mod/rtv-radar-lite/RtVRadarLiteOverlay.gd", project / "RtVRadarLiteOverlay.gd")
            shutil.copytree(ROOT / "tests/telemetry-diagnostic/Resources", project / "Resources")
            (project / "Scripts").mkdir()
            shutil.copyfile(ROOT / "tests/telemetry-diagnostic/Scripts/MockGameData.gd", project / "Scripts/MockGameData.gd")
            for script in (ROOT / "tests/radar-lite/Scripts").glob("*.gd"):
                shutil.copyfile(script, project / "Scripts" / script.name)
            shutil.copyfile(ROOT / "tests/telemetry-diagnostic/Scripts/LootContainer.gd", project / "Scripts/LootContainer.gd")
            shutil.copyfile(ROOT / "tests/radar-lite/lite-smoke.gd", project / "lite-smoke.gd")
            env = os.environ.copy()
            env["XDG_DATA_HOME"] = str(project / "user-data")
            result = subprocess.run(
                [str(args.engine.resolve()), "--headless", "--path", str(project), "--script", "res://lite-smoke.gd"],
                capture_output=True,
                text=True,
                check=False,
                timeout=45,
                env=env,
            )
            output = result.stdout + result.stderr
            print(f"{variant}:\n{output}")
            if result.returncode != 0 or "SCRIPT ERROR" in output or "ERROR:" in output:
                raise SystemExit(result.returncode or 1)
            markers = ["SMOKE OK: lite player only", "SMOKE OK: lite one Bandit"]
            if variant != "player-only":
                markers.append("SMOKE OK: contact trace flushed")
            if variant in ("in-game", "controls", "loot"):
                markers.append("SMOKE OK: in-game overlay receives AI and ignores input")
            if variant in ("controls", "loot"):
                markers.append("SMOKE OK: F7 layer/trail and F8 visibility state transitions")
            if variant == "loot":
                markers.append("SMOKE OK: loot available/empty and interaction proxy gating")
            for marker in markers:
                if marker not in output:
                    raise SystemExit(f"{variant}: missing {marker!r}")


if __name__ == "__main__":
    main()
