# Sobolyatnik-K — Sable Hunter 1L108K

Sobolyatnik-K is a mod for **Road to Vostok**: an in-game radar for tracking shots, people, movement trails, and loot, plus the **powerful** RtV Toolkit terminal app for managing your character while the game is closed. Not affiliated with the developer. Many thanks to Antti for making a fun game!

## Install on Windows

**Recommended download: [v1.1](https://github.com/jtburkh/Sobolyatnik-K/releases/tag/v1.1.0).** Close Road to Vostok and the Toolkit, and back up important saves. If another Sobolyatnik-K bundle is installed, **uninstall it first through Windows *Installed apps***. Uninstall preserves Metro, saves, and rollback backups; v1.1 setup refuses to overwrite another installed version. Then paste this **one line into PowerShell**. It verifies the public ZIP checksum *before* extracting and running the offline installer:

```powershell
$ErrorActionPreference='Stop'; $n='Sobolyatnik-K-v1.1-Windows.zip'; $u='https://github.com/jtburkh/Sobolyatnik-K/releases/download/v1.1.0/'; $d=Join-Path $env:TEMP ('sobolyatnik-'+[guid]::NewGuid().ToString('N')); New-Item -ItemType Directory -Path $d | Out-Null; $z=Join-Path $d $n; Invoke-WebRequest -UseBasicParsing -Uri ($u+$n) -OutFile $z; Invoke-WebRequest -UseBasicParsing -Uri ($u+$n+'.sha256') -OutFile ($z+'.sha256'); $m=[regex]::Match([IO.File]::ReadAllText($z+'.sha256'), '^([a-fA-F0-9]{64})  '+[regex]::Escape($n)+'\s*$'); if (-not $m.Success -or (Get-FileHash -LiteralPath $z -Algorithm SHA256).Hash -ine $m.Groups[1].Value) { throw 'Download checksum mismatch; nothing was installed' }; Expand-Archive -LiteralPath $z -DestinationPath $d; & powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $d 'run-sobolyatnik.ps1'); if ($LASTEXITCODE -ne 0) { throw 'Sobolyatnik-K setup did not complete' }
```

The ZIP contains the in-game `RtVRadarLoot.vmz`, matching Windows `rtv-toolkit.exe`, installers, and hash-pinned official Metro Mod Loader **3.4.2** with its MIT license. Fresh installs include Metro; an existing verified Metro 3.2.1 or 3.4.2 remains untouched. Setup checks each bundled file, refuses unknown/conflicting mods and loaders or a running game, and safely backs up and clears **only** an outdated radar mount cache when needed. It does not change saves, game binaries, or an existing Metro script. Standalone VMZ/EXE downloads are available on GitHub with individual checksums but are not standalone installers.

**Steam build 25837777 (RTV 0.2.1.0) was source-reviewed.** If your Steam appmanifest reports a different *valid* build, setup warns but lets you continue; compatibility there is unverified. Missing or invalid game information still blocks installation. Version numbers, build matching, mock tests, and synthetic installer checks do **not** establish broad crash-free gameplay or guarantee observing shots at 400 m.

**Uninstall:** Close the game and Toolkit, then use *Settings → Apps → Installed apps → Sobolyatnik-K* (or its Start Menu shortcut). Uninstall leaves Metro, saves, and backups alone. See the [v1.1 Windows guide](docs/windows-v1.1.md) or [download the ZIP and checksum manually](https://github.com/jtburkh/Sobolyatnik-K/releases/tag/v1.1.0).

## In game

**Shot Alerts** is visible by default: enemy, Nomad and boss gunshots appear as fixed markers and fade after five seconds. `F7` cycles radar views (AI, trails and loot), `F8` hides/shows the HUD, and `F9` cycles 50/100/200/400 m **display** ranges. Player and vehicle shots are excluded. The range does not change AI perception or guarantee detecting gunshots at 400 m. The radar does not modify character saves.

![Actual Road to Vostok gameplay in Village, with the Sobolyatnik-K radar HUD in the upper-right corner](assets/screenshots/in-game-village.webp)

*In-game screenshot from an earlier radar version. Broad combat/crash stability on the current game build is not established.*

## Windows Toolkit

The included `rtv-toolkit.exe` provides inventory and equipment editing, a live tactical radar, and a Backups tab. Its updated catalog has **273 inventory items**, including Antti's ten new items from patch 0.2.1.0. Close the game before changing a save; the Toolkit makes `.rtvbak.*` backups, and restoring requires typed confirmation. Its radar `+`/`-` range control is separate from the in-game `F9` setting. See the [radar notes](docs/radar-range-preview.md) and [telemetry configuration](docs/telemetry-installation.md). Do not expose its unauthenticated localhost telemetry listener outside a trusted machine.

<details>
<summary>Toolkit screenshots</summary>

![Character and vitals; personal save path redacted](assets/screenshots/toolkit-character.png)
![Equipment and weapon details; personal save path redacted](assets/screenshots/toolkit-equipment.png)
![Inventory grid; personal save path redacted](assets/screenshots/toolkit-inventory.png)
![Earlier live radar; personal save path redacted](assets/screenshots/toolkit-radar.png)

</details>

## Development and bug reports

Report problems with your Steam build ID, reproduction steps and **redacted** logs through [GitHub Issues](https://github.com/jtburkh/Sobolyatnik-K/issues); do not post private saves or recovered game resources. Source pushes require a version bump and run an advisory Steam build check; a matching build ID alone does not prove gameplay compatibility. See [runtime and safety research](docs/runtime-discovery.md), [third-party notices](THIRD_PARTY_NOTICES.md), and the [MIT license](LICENSE).
