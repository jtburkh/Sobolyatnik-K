#!/usr/bin/env python3
"""Build a fully offline, versioned Windows player bundle.

Uses the hash-checked v0.1.12 installer design but never mutates its sources,
release packagers, published assets or Steam files. Include official hash-pinned
MIT-licensed Metro 3.4.2 and its license, so installation needs no network.
"""

import argparse
import hashlib
from pathlib import Path
import shutil
import struct
import tomllib
from zipfile import ZIP_DEFLATED, ZipFile, ZipInfo

from package_radar_shots import ROOT, enabled_base

BUILD_ID = "25837777"  # Steam content update 0.2.1.0; identity alone does not prove gameplay compatibility.
SOURCE = ROOT / "godot-mod" / "rtv-radar-range"
TOOLS = ROOT / "tools"
OLD_VMZ_HASH = "6d2d06e55831f174483595d44fde6ba5c3a50f7fec183d2c66a5953ecb98efdd"
METRO_HASHES = {
    "modloader.gd": "30c39e3846957f43925e49ef2449c2ca8f6940c77446af169cfafc858975bcc8",
    "override.cfg": "9750a66fcf0cb1d9bf84284271f52e064f455cdd5a1cdc4981a007e4011a684b",
    "LICENSE": "a35d18b4f7f13235a40003ad97705e54bd37e5625dfe63cc6ecffb3a3822b02f",
}
PREVIOUS_METRO_SCRIPT_HASH = "60fcf7feec0a47c6472e3b7a190b46987b374618ae3d149c081b222542bc6135"


def sha256(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def replace_once(text: str, old: str, new: str) -> str:
    if text.count(old) != 1:
        raise ValueError(f"review source drift: expected one occurrence of {old!r}, found {text.count(old)}")
    return text.replace(old, new)


def version() -> str:
    package_version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
    if package_version != "1.1.0":
        raise ValueError("expected the separately versioned 1.1.0 candidate")
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
    base = enabled_base()
    # Old diagnostic installations could leave these opt-out flags in the
    # preserved user config. A player bundle must always expose its radar and
    # F7/F8/F9 controls; do not rewrite user config or their saves. The other
    # config preferences (network/loot/contacts) remain unchanged.
    for flag in ("OVERLAY", "CONTROLS"):
        base = replace_once(
            base,
            f'bool(config.get_value("radar_lite", "{flag.lower()}", {flag}_DEFAULT))',
            f"{flag}_DEFAULT",
        )
    entries = {
        "RtVRadarLite.gd": base,
        "RtVRadarLiteOverlay.gd": (SOURCE / "RtVRadarLiteOverlay.gd").read_text(),
        "RtVRadarShotBridge.gd": (SOURCE / "RtVRadarShotBridge.gd").read_text(),
        "mod.txt": (
            '[mod]\n'
            f'name="Sobolyatnik-K v1.1"\n'
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
            raise ValueError(f"official Metro 3.4.2 {name} hash mismatch")
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
        ("$loaderRelease = 'https://github.com/ametrocavich/vostok-mod-loader/releases/download/v3.2.1'", "$loaderRelease = 'https://github.com/ametrocavich/vostok-mod-loader/releases/download/v3.4.2'"),
        (f"$loaderScriptHash = '{PREVIOUS_METRO_SCRIPT_HASH}'", f"$loaderScriptHash = '{METRO_HASHES['modloader.gd']}'"),
        (f"$expectedHash = '{OLD_VMZ_HASH}'", f"$expectedHash = '{radar_hash}'"),
        ("$downloadUrl = 'https://github.com/jtburkh/Sobolyatnik-K/releases/download/v0.1.12-experimental/RtVRadarLoot.vmz'", "$downloadUrl = '' # Versioned radar bytes must be supplied locally."),
        ("Assert-GameClosed\n$game = Resolve-Game", "if (-not $ArchivePath) { throw 'This offline bundle requires its local -ArchivePath; no VMZ is downloaded.' }\nAssert-GameClosed\n$game = Resolve-Game"),
        ("Launch Road to Vostok. F7 cycles radar layers; F8 hides/shows the overlay.", "Launch Road to Vostok. F7 cycles radar modes, F8 hides/shows it, and F9 changes the view range."),
    ):
        radar = replace_once(radar, old, new)
    # Default fresh installs to Metro 3.4.2 but never overwrite an existing
    # exact official 3.2.1 script. The owner reports v0.1.16 still works on the
    # updated game with 3.2.1; a loader upgrade is a separate operation.
    radar = replace_once(
        radar,
        "    if ([IO.File]::ReadAllText($scriptPath) -notmatch 'const\\s+MODLOADER_VERSION\\s*:=\\s*\"3\\.2\\.1\"') {\n"
        "        throw 'Only Metro Mod Loader 3.2.1 has been tested with this VMZ. No files were changed.'\n"
        "    }\n"
        "    if ((Get-FileHash -LiteralPath $scriptPath -Algorithm SHA256).Hash -ine $loaderScriptHash) {\n"
        "        throw 'Existing Metro 3.2.1 script differs from the tested official release. No loader files were overwritten.'\n"
        "    }",
        "    $installedHash = (Get-FileHash -LiteralPath $scriptPath -Algorithm SHA256).Hash\n"
        f"    if ($installedHash -ine $loaderScriptHash -and $installedHash -ine '{PREVIOUS_METRO_SCRIPT_HASH}') {{\n"
        "        throw 'Metro loader is neither the verified 3.2.1 nor 3.4.2 script. No loader files were overwritten.'\n"
        "    }",
    )
    radar = radar.replace("Metro 3.2.1", "Metro 3.4.2")
    radar = radar.replace("Metro Mod Loader 3.2.1", "Metro Mod Loader 3.4.2")
    radar = replace_once(
        radar,
        "    if ($metroState -eq 'Existing' -and -not $needsMod) {\n"
        "        Write-Host \"Already installed: $destination (Metro 3.4.2 present)\"\n"
        "        return\n"
        "    }",
        "    if ($metroState -eq 'Existing' -and -not $needsMod) {\n"
        "        if ($DryRun) { Write-Host 'Dry run: would check radar cache.' }\n"
        "        else { Repair-RadarCache }\n"
        "        Write-Host \"Already installed: $destination (verified Metro present)\"\n"
        "        return\n"
        "    }",
    )
    radar = replace_once(
        radar,
        "    if ($DryRun) {\n"
        "        if ($metroState -eq 'Missing')",
        "    if ($DryRun) {\n"
        "        Write-Host 'Dry run: would check radar cache.'\n"
        "        if ($metroState -eq 'Missing')",
    )
    radar = replace_once(
        radar,
        '    Write-Host "Installed: $destination"',
        '    Repair-RadarCache\n    Write-Host "Installed: $destination"',
    )
    radar = replace_once(
        radar,
        "Assert-GameClosed\n$game = Resolve-Game",
        """function Repair-RadarCache {
    param([switch] $ValidateOnly)
    # Metro 3.2.1 can reuse a stale user:// VMZ mount by mtime; 3.4.2 uses a
    # source sidecar. Only our one cache is ever removed, after a verified copy.
    if (-not $env:APPDATA -or -not $env:LOCALAPPDATA) {
        throw 'Windows app data directories are unavailable; cache was not touched.'
    }
    $userDir = Join-Path $env:APPDATA 'Road to Vostok'
    $cacheDir = Join-Path $userDir 'vmz_mount_cache'
    foreach ($path in @($userDir, $cacheDir)) {
        if (Test-Path -LiteralPath $path) {
            $item = Get-Item -LiteralPath $path -Force
            if (-not $item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
                throw "Radar cache directory is not a normal directory: $path"
            }
        }
    }
    $cache = Join-Path $cacheDir 'RtVRadarLoot.zip'
    if (-not (Test-Path -LiteralPath $cache)) { return }
    $file = Get-Item -LiteralPath $cache -Force
    if ($file.PSIsContainer -or ($file.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw 'Radar cache is not a normal file; refusing to change it.'
    }
    if ($ValidateOnly) { return }
    $oldHash = (Get-FileHash -LiteralPath $cache -Algorithm SHA256).Hash
    if ($oldHash -ieq $expectedHash) { return }
    $backupDir = Join-Path $env:LOCALAPPDATA 'Sobolyatnik-K\\backups'
    New-Item -ItemType Directory -Path $backupDir -Force | Out-Null
    $backup = Join-Path $backupDir ('RtVRadarLoot-cache-' + [guid]::NewGuid().ToString('N') + '.zip')
    Copy-Item -LiteralPath $cache -Destination $backup -ErrorAction Stop
    Assert-Hash $backup $oldHash 'Radar cache rollback backup'
    Assert-GameClosed
    Assert-Archive $destination
    Remove-Item -LiteralPath $cache -Force -ErrorAction Stop
    Write-Host "Backed up outdated radar-only cache to $backup; Metro rebuilds it next launch."
}

Assert-GameClosed
$game = Resolve-Game""",
    )
    radar = replace_once(
        radar,
        "    Assert-Archive $archive\n    Write-Host \"Verified Sobolyatnik-K $version (SHA-256 $expectedHash).\"",
        "    Assert-Archive $archive\n    Repair-RadarCache -ValidateOnly\n    Write-Host \"Verified Sobolyatnik-K $version (SHA-256 $expectedHash).\"",
    )
    # Keep published v0.1.12 immutable: only the new, locally generated installer
    # changes from an exact-build refusal to an advisory warning. Missing or
    # malformed manifests still fail closed, and all file/ownership checks stay.
    radar = replace_once(
        radar,
        "    if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf) -or\n"
        "        (Get-VdfField ([IO.File]::ReadAllText($manifestPath)) 'buildid') -ne $expectedBuild) {\n"
        "        throw \"Steam build $expectedBuild is required. Check $manifestPath before installing onto a different game build.\"\n"
        "    }",
        "    if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {\n"
        "        throw \"Steam appmanifest not found at $manifestPath; no files were installed.\"\n"
        "    }\n"
        "    $steamManifest = [IO.File]::ReadAllText($manifestPath)\n"
        "    $appIds = [regex]::Matches($steamManifest, '(?m)^\\s*\"appid\"\\s*\"([^\"\\r\\n]*)\"')\n"
        "    if ($appIds.Count -ne 1 -or $appIds[0].Groups[1].Value -ne '1963610') {\n"
        "        throw \"Steam appmanifest is not for Road to Vostok: $manifestPath; no files were installed.\"\n"
        "    }\n"
        "    $buildIds = [regex]::Matches($steamManifest, '(?m)^\\s*\"buildid\"\\s*\"([^\"\\r\\n]*)\"')\n"
        "    if ($buildIds.Count -ne 1 -or $buildIds[0].Groups[1].Value -notmatch '^[1-9][0-9]*$') {\n"
        "        throw \"Steam build ID missing or invalid in $manifestPath; no files were installed.\"\n"
        "    }\n"
        "    $installedBuild = $buildIds[0].Groups[1].Value\n"
        "    if ($installedBuild -ne $expectedBuild) {\n"
        "        Write-Warning \"Road to Vostok Steam build $installedBuild differs from the reviewed build $expectedBuild. "
        "Compatibility is unverified. Continuing installation at your risk; keep a backup of important saves.\"\n"
        "    }",
    )
    (kit / "install-sobolyatnik.ps1").write_text(radar, encoding="utf-8", newline="\n")
    radar_installer_hash = sha256(radar.encode("utf-8"))

    uninstaller = (TOOLS / "uninstall-sobolyatnik-v0.1.12.ps1").read_text()
    uninstaller = replace_once(uninstaller, "$version = '0.1.12-experimental'", f"$version = '{kit_version}'")
    uninstaller = replace_once(uninstaller, f"$radarHash = '{OLD_VMZ_HASH}'", f"$radarHash = '{radar_hash}'")
    uninstaller = replace_once(uninstaller, "'Sobolyatnik-K (Experimental)'", "'Sobolyatnik-K'")
    (kit / "uninstall-sobolyatnik.ps1").write_text(uninstaller, encoding="utf-8", newline="\n")
    uninstaller_hash = sha256(uninstaller.encode("utf-8"))

    setup = (TOOLS / "setup-sobolyatnik-v0.1.12.ps1.in").read_text()
    for old, new in (
        ("$version = '0.1.12-experimental'", f"$version = '{kit_version}'"),
        (f"$radarHash = '{OLD_VMZ_HASH}'", f"$radarHash = '{radar_hash}'"),
        ("$radarInstallerHash = 'cf66879c05c3a8404458e24b1e75f6943895c604a497398551db8535ba01c39d'", f"$radarInstallerHash = '{radar_installer_hash}'"),
        ("(?: \\(Metro 3\\.2\\.1 present\\))?", "(?: \\(verified Metro present\\))?"),
        ("$newRelease = 'https://github.com/jtburkh/Sobolyatnik-K/releases/download/v0.1.12-experimental'", "$newRelease = '' # Offline bundle: never fetch assets from an old release."),
        ("@('0.1.9-experimental', '0.1.11-experimental')", "@('0.1.9-experimental', '0.1.11-experimental', '0.1.12-experimental', '0.1.16-experimental.1', '0.1.17-experimental.1', '0.1.18-experimental.1')"),
        ("$downloads = New-Object System.Collections.Generic.List[string]\ntry {", "$downloads = New-Object System.Collections.Generic.List[string]\ntry {\n    if (-not $ToolkitPath -or -not $UninstallerPath -or -not $RadarInstallerPath -or -not $ArchivePath -or -not $LoaderSourceDirectory) {\n        throw 'This offline bundle requires local Toolkit, uninstaller, radar installer, VMZ, and Metro; nothing was installed.'\n    }"),
        ("-Name DisplayVersion -Value '0.1.12'", f"-Name DisplayVersion -Value '{kit_version}'"),
        ("@TOOLKIT_SHA256@", sha256(executable)),
        ("@UNINSTALLER_SHA256@", uninstaller_hash),
    ):
        setup = replace_once(setup, old, new)
    if setup.count("'Sobolyatnik-K (Experimental)'") != 2:
        raise ValueError("review source drift: Windows app label changed")
    setup = setup.replace("'Sobolyatnik-K (Experimental)'", "'Sobolyatnik-K'")
    setup = replace_once(setup, "Install Sobolyatnik-K 0.1.12 Experimental", "Install Sobolyatnik-K v1.1")
    (kit / "setup-sobolyatnik.ps1").write_text(setup, encoding="utf-8", newline="\n")
    setup_hash = sha256(setup.encode("utf-8"))

    wrapper = (TOOLS / "run-hotfix-test-install.ps1").read_text()
    wrapper = replace_once(wrapper, "@KIT_VERSION@", kit_version)
    wrapper = replace_once(wrapper, f"{kit_version} experimental Windows bundle", f"v1.1 Windows bundle")
    (kit / "run-sobolyatnik.ps1").write_text(wrapper, encoding="utf-8", newline="\n")
    lines = [
        f"Sobolyatnik-K v1.1 Windows bundle (package {kit_version}) for Road to Vostok Steam build {BUILD_ID}.",
        "Includes the in-game Shot Alerts radar, selectable 50/100/200/400m views, and Windows Toolkit.",
        "Older radar_lite overlay/controls=false preferences are ignored by this player release; other user config remains untouched.",
        "Close Road to Vostok before installing or editing saves. Back up important saves first.",
        "Verify the ZIP and adjacent .sha256 from GitHub release v1.1.0 before extracting.",
        "Version numbers are not a guarantee of broad gameplay or crash-free compatibility.",
        "Extract the entire ZIP including the metro folder into one folder on Windows.",
        "From PowerShell in that folder, first run: .\\run-sobolyatnik.ps1 -DryRun",
        "After reviewing paths, install: .\\run-sobolyatnik.ps1",
        "If the Steam build differs from the reviewed build, the installer warns but allows installation. Compatibility is unverified.",
        "To uninstall, close the game and Toolkit, then run: .\\run-sobolyatnik.ps1 -Uninstall",
        "Uninstall retains Metro, character saves and rollback copies.",
        "First release on this game build: gameplay stability is not yet broadly established. Report bugs with redacted logs.",
        "The included official Metro Mod Loader 3.4.2 is MIT licensed; see metro/LICENSE.",
        "An existing hash-verified Metro 3.2.1 is retained; no loader is overwritten.",
        "",
        "SHA-256 (internal file hashes for this bundle):",
    ]
    for name in ("RtVRadarLoot.vmz", "rtv-toolkit.exe", "install-sobolyatnik.ps1", "uninstall-sobolyatnik.ps1", "setup-sobolyatnik.ps1", "run-sobolyatnik.ps1", "metro/modloader.gd", "metro/override.cfg", "metro/LICENSE"):
        lines.append(f"{sha256((kit / name).read_bytes())}  {name}")
    (kit / "README.txt").write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    return {"version": kit_version, "radar_sha256": radar_hash, "setup_sha256": setup_hash, "toolkit_sha256": sha256(executable)}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--toolkit", type=Path, required=True, help="tested Windows x64 executable")
    parser.add_argument("--output", type=Path, required=True, help="new, empty output directory")
    parser.add_argument("--metro", type=Path, required=True, help="verified official Metro 3.4.2 files, including LICENSE")
    args = parser.parse_args()
    for key, value in generate(args.output, args.toolkit, args.metro).items():
        print(f"{key}: {value}")


if __name__ == "__main__":
    main()
