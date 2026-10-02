#!/usr/bin/env python3
"""Package five isolated radar variants; none is a release asset.

Only the in-game archives include the drawing script. No variant edits the
user's config; their identities and feature defaults are distinct.
"""

from __future__ import annotations

from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile, ZipInfo

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "godot-mod" / "rtv-radar-lite"
OUTPUT = ROOT / "dist" / "probes"


def build(name: str, manifest: str, script: str, overlay: str | None = None) -> None:
    entries = {"RtVRadarLite.gd": script, "mod.txt": manifest}
    if overlay is not None:
        entries["RtVRadarLiteOverlay.gd"] = overlay
    with ZipFile(OUTPUT / name, "w", compression=ZIP_DEFLATED, compresslevel=9) as archive:
        for filename, contents in sorted(entries.items()):
            info = ZipInfo(filename, (2020, 1, 1, 0, 0, 0))
            info.compress_type = ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            archive.writestr(info, contents.encode("utf-8"), compresslevel=9)
    print(OUTPUT / name)


def main() -> None:
    OUTPUT.mkdir(parents=True, exist_ok=True)
    script = (SOURCE / "RtVRadarLite.gd").read_text()
    overlay = (SOURCE / "RtVRadarLiteOverlay.gd").read_text()
    manifest = (SOURCE / "mod.txt").read_text()
    contacts_default = "const CONTACTS_DEFAULT := false"
    overlay_default = "const OVERLAY_DEFAULT := false"
    controls_default = "const CONTROLS_DEFAULT := false"
    loot_default = "const LOOT_DEFAULT := false"
    name = 'name="RtV Radar Lite Test"'
    mod_id = 'id="rtv_toolkit_radar_lite"'
    if any((script.count(contacts_default) != 1, script.count(overlay_default) != 1,
            script.count(controls_default) != 1, script.count(loot_default) != 1,
            manifest.count(name) != 1, manifest.count(mod_id) != 1)):
        raise SystemExit("radar-lite sources changed; inspect feature defaults before packaging")
    build("RtVRadarLite.vmz", manifest, script)
    contacts_script = script.replace(contacts_default, "const CONTACTS_DEFAULT := true")
    build(
        "RtVRadarContacts.vmz",
        manifest.replace(name, 'name="RtV Radar Contacts Test"').replace(mod_id, 'id="rtv_toolkit_radar_contacts"'),
        contacts_script,
    )
    in_game_script = contacts_script.replace(overlay_default, "const OVERLAY_DEFAULT := true")
    build(
        "RtVRadarInGame.vmz",
        manifest.replace(name, 'name="RtV In-Game Radar Test"').replace(mod_id, 'id="rtv_toolkit_radar_ingame"'),
        in_game_script,
        overlay,
    )
    controls_script = in_game_script.replace(controls_default, "const CONTROLS_DEFAULT := true")
    build(
        "RtVRadarControls.vmz",
        manifest.replace(name, 'name="RtV Radar Controls Test"').replace(mod_id, 'id="rtv_toolkit_radar_controls"'),
        controls_script,
        overlay,
    )
    build(
        "RtVRadarLoot.vmz",
        manifest.replace(name, 'name="Sobolyatnik-K (Sable Hunter) Experimental — 1L108K"').replace(mod_id, 'id="rtv_toolkit_radar_loot"'),
        controls_script.replace(loot_default, "const LOOT_DEFAULT := true"),
        overlay,
    )


if __name__ == "__main__":
    main()
