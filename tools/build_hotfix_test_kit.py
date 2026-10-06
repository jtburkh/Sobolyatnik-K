#!/usr/bin/env python3
"""Build a separate, fully offline clean-machine test kit (NOT a release).

Uses the hash-checked v0.1.12 installer design but never mutates its sources,
release packagers, published assets or Steam files. Include official hash-pinned
MIT-licensed Metro 3.2.1 and its license, so missing assets never fetch online.
"""

import argparse
import hashlib
from pathlib import Path
import shutil
import struct
import tomllib
from zipfile import ZIP_DEFLATED, ZipFile, ZipInfo

from package_radar_shots import ROOT, enabled_base

BUILD_ID = "25710663"  # Source-reviewed Build 2 hotfix 0.2.0.5, not an open-ended bypass.
SOURCE = ROOT / "godot-mod" / "rtv-radar-range"
TOOLS = ROOT / "tools"
OLD_VMZ_HASH = "6d2d06e55831f174483595d44fde6ba5c3a50f7fec183d2c66a5953ecb98efdd"
METRO_HASHES = {
    "modloader.gd": "60fcf7feec0a47c6472e3b7a190b46987b374618ae3d149c081b222542bc6135",
    "override.cfg": "9750a66fcf0cb1d9bf84284271f52e064f455cdd5a1cdc4981a007e4011a684b",
    "LICENSE": "a35d18b4f7f13235a40003ad97705e54bd37e5625dfe63cc6ecffb3a3822b02f",
}


def sha256(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def replace_once(text: str, old: str, new: str) -> str:
    if text.count(old) != 1:
        raise ValueError(f"review source drift: expected one occurrence of {old!r}, found {text.count(old)}")
    return text.replace(old, new)


def replace_count(text: str, old: str, new: str, count: int) -> str:
    if text.count(old) != count:
        raise ValueError(f"review source drift: expected {count} occurrences of {old!r}, found {text.count(old)}")
    return text.replace(old, new)


def version() -> str:
    package_version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
    if not package_version.startswith("0.1.13-test.") or not package_version.rsplit(".", 1)[-1].isdigit():
        raise ValueError("expected a distinct numbered 0.1.13-test.N prerelease version")
    manifest = (SOURCE / "mod.txt").read_text()
    if f'version="{package_version}"' not in manifest:
        raise ValueError("Toolkit and candidate radar manifest versions differ")
    return package_version


def verify_pe(executable: bytes) -> None:
    if len(executable) < 512 or executable[:2] != b"MZ":
        raise ValueError("Toolkit is not a Windows PE executable")
    offset = struct.unpack_from("<I", executable, 0x3C)[0]
    if offset + 6 > len(executable) or executable[offset:offset + 4] != b"PE\0\0":
        raise ValueError("Toolkit has no valid PE signature")
    if struct.unpack_from("<H", executable, offset + 4)[0] != 0x8664:
        raise ValueError("Toolkit must be Windows x64")


def package_radar(path: Path, kit_version: str) -> str:
    entries = {
        "RtVRadarLite.gd": enabled_base(),
        "RtVRadarLiteOverlay.gd": (SOURCE / "RtVRadarLiteOverlay.gd").read_text(),
        "RtVRadarShotBridge.gd": (SOURCE / "RtVRadarShotBridge.gd").read_text(),
        "mod.txt": (
            '[mod]\n'
            f'name="Sobolyatnik-K Controlled Hotfix Test — 1L108K"\n'
            'id="rtv_toolkit_radar_loot"\n'
            f'version="{kit_version}"\n'
            'priority=0\n\n'
            '[autoload]\n'
            'RtVRadarShots="res://RtVRadarShotBridge.gd"\n'
        ),
    }
    if '.hook(' in entries['RtVRadarShotBridge.gd'] or 'res://Scripts/AI.gd' in entries['mod.txt']:
        raise ValueError("refusing AI script rewrite")
    with ZipFile(path, "w", compression=ZIP_DEFLATED, compresslevel=9) as archive:
        for filename, text in sorted(entries.items()):
            info = ZipInfo(filename, (2020, 1, 1, 0, 0, 0))
            info.compress_type = ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            archive.writestr(info, text.encode("utf-8"), compresslevel=9)
    return sha256(path.read_bytes())


def generate(kit: Path, toolkit: Path, metro: Path) -> dict[str, str]:
    kit_version = version()
    executable = toolkit.read_bytes()
    verify_pe(executable)
    for name, expected in METRO_HASHES.items():
        if sha256((metro / name).read_bytes()) != expected:
            raise ValueError(f"official Metro 3.2.1 {name} hash mismatch")
    if kit.exists() and any(kit.iterdir()):
        raise ValueError(f"refusing to overwrite an existing kit: {kit}")
    kit.mkdir(parents=True, exist_ok=True)
    (kit / "metro").mkdir()
    for name in METRO_HASHES:
        shutil.copyfile(metro / name, kit / "metro" / name)
    shutil.copyfile(toolkit, kit / "rtv-toolkit.exe")
    radar_hash = package_radar(kit / "RtVRadarLoot.vmz", kit_version)

    radar = (TOOLS / "install-sobolyatnik-v0.1.12.ps1").read_text()
    for old, new in (
        ("$version = '0.1.12'", f"$version = '{kit_version}'"),
        ("$expectedBuild = '25632875'", f"$expectedBuild = '{BUILD_ID}'"),
        (f"$expectedHash = '{OLD_VMZ_HASH}'", f"$expectedHash = '{radar_hash}'"),
        ("$downloadUrl = 'https://github.com/jtburkh/Sobolyatnik-K/releases/download/v0.1.12-experimental/RtVRadarLoot.vmz'", "$downloadUrl = '' # All preview radar bytes must be supplied locally."),
        ("Assert-GameClosed\n$game = Resolve-Game", "if (-not $ArchivePath) { throw 'This test kit requires its local -ArchivePath; no VMZ is downloaded.' }\nAssert-GameClosed\n$game = Resolve-Game"),
        ("Launch Road to Vostok. F7 cycles radar layers; F8 hides/shows the overlay.", "Controlled test only. F7 modes, F8 hide/show, F9 range. Preserve rollback before testing."),
    ):
        radar = replace_once(radar, old, new)
    (kit / "install-sobolyatnik.ps1").write_text(radar, encoding="utf-8", newline="\n")
    radar_installer_hash = sha256(radar.encode("utf-8"))

    uninstaller = (TOOLS / "uninstall-sobolyatnik-v0.1.12.ps1").read_text()
    uninstaller = replace_once(uninstaller, "$version = '0.1.12-experimental'", f"$version = '{kit_version}'")
    uninstaller = replace_once(uninstaller, f"$radarHash = '{OLD_VMZ_HASH}'", f"$radarHash = '{radar_hash}'")
    uninstaller = replace_once(uninstaller, "if ($entry.DisplayName -ne 'Sobolyatnik-K (Experimental)'", f"if ($entry.DisplayName -ne 'Sobolyatnik-K ({kit_version} TEST)'")
    for old, new in (("Sobolyatnik-K Toolkit.lnk", "Sobolyatnik-K Toolkit (TEST).lnk"), ("Uninstall Sobolyatnik-K.lnk", "Uninstall Sobolyatnik-K (TEST).lnk")):
        uninstaller = replace_count(uninstaller, old, new, 2)
    (kit / "uninstall-sobolyatnik.ps1").write_text(uninstaller, encoding="utf-8", newline="\n")
    uninstaller_hash = sha256(uninstaller.encode("utf-8"))

    setup = (TOOLS / "setup-sobolyatnik-v0.1.12.ps1.in").read_text()
    for old, new in (
        ("$version = '0.1.12-experimental'", f"$version = '{kit_version}'"),
        (f"$radarHash = '{OLD_VMZ_HASH}'", f"$radarHash = '{radar_hash}'"),
        ("$radarInstallerHash = 'cf66879c05c3a8404458e24b1e75f6943895c604a497398551db8535ba01c39d'", f"$radarInstallerHash = '{radar_installer_hash}'"),
        ("$newRelease = 'https://github.com/jtburkh/Sobolyatnik-K/releases/download/v0.1.12-experimental'", "$newRelease = '' # Offline test kit: never fetch preview bytes from the old release."),
        ("@('0.1.9-experimental', '0.1.11-experimental')", "@('0.1.9-experimental', '0.1.11-experimental', '0.1.12-experimental')"),
        ("$downloads = New-Object System.Collections.Generic.List[string]\ntry {", "$downloads = New-Object System.Collections.Generic.List[string]\ntry {\n    if (-not $ToolkitPath -or -not $UninstallerPath -or -not $RadarInstallerPath -or -not $ArchivePath -or -not $LoaderSourceDirectory) {\n        throw 'This offline test kit requires local Toolkit, uninstaller, radar installer, VMZ, and Metro; nothing was installed.'\n    }"),
        ("-Name DisplayVersion -Value '0.1.12'", f"-Name DisplayVersion -Value '{kit_version}'"),
        ("@TOOLKIT_SHA256@", sha256(executable)),
        ("@UNINSTALLER_SHA256@", uninstaller_hash),
    ):
        setup = replace_once(setup, old, new)
    setup = replace_count(setup, "Sobolyatnik-K (Experimental)", f"Sobolyatnik-K ({kit_version} TEST)", 2)
    for old, new in (("Sobolyatnik-K Toolkit.lnk", "Sobolyatnik-K Toolkit (TEST).lnk"), ("Uninstall Sobolyatnik-K.lnk", "Uninstall Sobolyatnik-K (TEST).lnk")):
        setup = replace_count(setup, old, new, 2)
    (kit / "setup-sobolyatnik.ps1").write_text(setup, encoding="utf-8", newline="\n")
    setup_hash = sha256(setup.encode("utf-8"))

    wrapper = (TOOLS / "run-hotfix-test-install.ps1").read_text()
    wrapper = replace_once(wrapper, "@KIT_VERSION@", kit_version)
    (kit / "run-test-install.ps1").write_text(wrapper, encoding="utf-8", newline="\n")
    lines = [
        f"UNRELEASED Sobolyatnik-K {kit_version} owner test kit for Steam build {BUILD_ID}.",
        "No v0.1.12 asset was changed. No game or save was touched during packaging.",
        "This has passed offline/Windows fixture checks only; it is NOT broad combat clearance.",
        "Close Road to Vostok. Use a disposable save and clean Steam test installation.",
        "The included Metro Mod Loader 3.2.1 is upstream MIT licensed; see metro/LICENSE.",
        "Extract the entire GitHub Actions artifact, INCLUDING the metro folder, into one folder on Windows.",
        "From PowerShell in that folder: .\\run-test-install.ps1 -DryRun",
        "Review the paths and hashes. To opt into the controlled test: .\\run-test-install.ps1",
        "If Steam updates again, the installer refuses; never change the manifest or build guard.",
        "Close the game before uninstall: .\\run-test-install.ps1 -Uninstall",
        "Uninstall retains Metro, character saves and rollback copies.",
        "",
        "SHA-256 (verify the downloaded artifact files before running):",
    ]
    for name in ("RtVRadarLoot.vmz", "rtv-toolkit.exe", "install-sobolyatnik.ps1", "uninstall-sobolyatnik.ps1", "setup-sobolyatnik.ps1", "run-test-install.ps1", "metro/modloader.gd", "metro/override.cfg", "metro/LICENSE"):
        lines.append(f"{sha256((kit / name).read_bytes())}  {name}")
    (kit / "TEST-README.txt").write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    return {"version": kit_version, "radar_sha256": radar_hash, "setup_sha256": setup_hash, "toolkit_sha256": sha256(executable)}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--toolkit", type=Path, required=True, help="tested Windows x64 executable")
    parser.add_argument("--output", type=Path, required=True, help="new, empty output directory")
    parser.add_argument("--metro", type=Path, required=True, help="verified official Metro 3.2.1 files, including LICENSE")
    args = parser.parse_args()
    for key, value in generate(args.output, args.toolkit, args.metro).items():
        print(f"{key}: {value}")


if __name__ == "__main__":
    main()
