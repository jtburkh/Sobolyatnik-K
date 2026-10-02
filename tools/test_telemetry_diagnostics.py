#!/usr/bin/env python3
"""Run the isolated bridge profile smoke test in a temporary Godot project.

Requires a Godot-compatible engine binary; does not launch Road to Vostok or
write to its installation or saves. Example:
    python3 tools/test_telemetry_diagnostics.py --engine /path/to/godot
"""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EXPECTED = (
    "SMOKE OK: trace flushed snapshot and input phases",
    "SMOKE OK: player_only ai=0 loot=0",
    "SMOKE OK: overlay_only ai=0 loot=0",
    "SMOKE OK: ai_only ai=1 loot=0",
    "SMOKE OK: loot_only ai=0 loot=1",
    "SMOKE OK: overlay_ai ai=1 loot=0",
    "SMOKE OK: diagnostic overlay and summon guard",
)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--engine", type=Path, required=True)
    args = parser.parse_args()
    engine = args.engine.resolve()
    if not engine.is_file():
        parser.error(f"Godot engine not found: {engine}")

    fixtures = ROOT / "tests" / "telemetry-diagnostic"
    with tempfile.TemporaryDirectory(prefix="rtv-telemetry-smoke-") as directory:
        project = Path(directory)
        shutil.copytree(
            ROOT / "godot-mod" / "rtv-telemetry" / "RtVTelemetryProof",
            project / "RtVTelemetryProof",
        )
        shutil.copytree(fixtures / "Scripts", project / "Scripts")
        shutil.copytree(fixtures / "Resources", project / "Resources")
        shutil.copy2(fixtures / "diagnostic-smoke.gd", project / "diagnostic-smoke.gd")
        (project / "project.godot").write_text(
            '[application]\nconfig/name="RtV Diagnostics Smoke"\n', encoding="utf-8"
        )
        env = os.environ.copy()
        env["XDG_DATA_HOME"] = str(project / "user-data")
        run = subprocess.run(
            [str(engine), "--headless", "--path", str(project), "--script", "res://diagnostic-smoke.gd"],
            capture_output=True,
            text=True,
            timeout=45,
            check=False,
            env=env,
        )
        output = run.stdout + run.stderr
        print(output)
        missing = [line for line in EXPECTED if line not in output]
        if run.returncode != 0 or missing or "SCRIPT ERROR:" in output:
            raise SystemExit(f"diagnostic smoke failed (exit={run.returncode}, missing={missing})")


if __name__ == "__main__":
    main()
