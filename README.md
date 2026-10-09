# Sobolyatnik-K — Sable Hunter 1L108K

Sobolyatnik-K is a mod for **Road to Vostok**: an in-game radar for tracking shots, people, movement trails, and loot, plus the **powerful** RtV Toolkit terminal app for managing your character while the game is closed. Not affiliated with the developer. Many thanks to Antti for making a fun game!

## Install on Windows

**Recommended download: [v0.1.16](https://github.com/jtburkh/Sobolyatnik-K/releases/tag/v0.1.16).** Its radar VMZ and Windows Toolkit EXE byte-match the version confirmed working on the maintainer's machine. This is not a guarantee for every game build or computer. **Close Road to Vostok**, then paste this **one line into PowerShell**. It verifies the ZIP checksum, extracts the radar, Toolkit and Metro loader, and runs setup:

```powershell
$ErrorActionPreference='Stop'; $n='Sobolyatnik-K-0.1.16-experimental.1-reviewed25710663-Windows.zip'; $u='https://github.com/jtburkh/Sobolyatnik-K/releases/download/v0.1.16/'; $d=Join-Path $env:TEMP ('sobolyatnik-'+[guid]::NewGuid().ToString('N')); New-Item -ItemType Directory -Path $d | Out-Null; $z=Join-Path $d $n; Invoke-WebRequest -UseBasicParsing -Uri ($u+$n) -OutFile $z; Invoke-WebRequest -UseBasicParsing -Uri ($u+$n+'.sha256') -OutFile ($z+'.sha256'); $m=[regex]::Match([IO.File]::ReadAllText($z+'.sha256'), '^([a-fA-F0-9]{64})  '+[regex]::Escape($n)+'\s*$'); if (-not $m.Success -or (Get-FileHash -LiteralPath $z -Algorithm SHA256).Hash -ine $m.Groups[1].Value) { throw 'Download checksum mismatch; nothing was installed' }; Expand-Archive -LiteralPath $z -DestinationPath $d; & powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $d 'run-sobolyatnik.ps1'); if ($LASTEXITCODE -ne 0) { throw 'Sobolyatnik-K setup did not complete' }; & { $ErrorActionPreference='Stop'; if (Get-Process -Name RTV -ErrorAction SilentlyContinue) { throw 'Close Road to Vostok first.' }; $receiptPath=Join-Path $env:LOCALAPPDATA 'Sobolyatnik-K\installed.json'; if (-not (Test-Path -LiteralPath $receiptPath -PathType Leaf)) { throw 'No bundle receipt: install v0.1.16 first.' }; $receipt=Get-Content -LiteralPath $receiptPath -Raw | ConvertFrom-Json; $expected='276844b086eb27f559b71aa74743f49e6ee045c639a7203b1b1ce02ca98be1a4'; if ($receipt.Version -ne '0.1.16-experimental.1' -or $receipt.RadarSha256 -ine $expected) { throw 'This is not the verified v0.1.16 bundle; no cache was changed.' }; $vmz=Join-Path $receipt.GamePath 'mods\RtVRadarLoot.vmz'; if ((Get-FileHash -LiteralPath $vmz -Algorithm SHA256).Hash -ine $expected) { throw 'Installed VMZ hash differs from v0.1.16; no cache was changed.' }; $cache=Join-Path $env:APPDATA 'Road to Vostok\vmz_mount_cache\RtVRadarLoot.zip'; if (-not (Test-Path -LiteralPath $cache)) { Write-Host 'No Metro radar cache found; nothing was changed.'; return }; $cacheInfo=Get-Item -LiteralPath $cache; if (-not $cacheInfo.PSIsContainer -and ($cacheInfo.Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Cache is a link; refusing to change it.' }; if ($cacheInfo.PSIsContainer) { throw 'Cache path is a directory; refusing to change it.' }; $oldHash=(Get-FileHash -LiteralPath $cache -Algorithm SHA256).Hash; if ($oldHash -ieq $expected) { Write-Host 'Metro radar cache already matches v0.1.16; no change made.'; return }; $backupDir=Join-Path $env:LOCALAPPDATA 'Sobolyatnik-K\backups'; New-Item -ItemType Directory -Path $backupDir -Force | Out-Null; $backup=Join-Path $backupDir ('RtVRadarLoot-cache-' + [guid]::NewGuid().ToString('N') + '.zip'); Copy-Item -LiteralPath $cache -Destination $backup -ErrorAction Stop; if ((Get-FileHash -LiteralPath $backup -Algorithm SHA256).Hash -ine $oldHash) { throw 'Cache backup did not verify; cache was not removed.' }; if (Get-Process -Name RTV -ErrorAction SilentlyContinue) { throw 'Game started during backup; cache was not removed.' }; Remove-Item -LiteralPath $cache -Force -ErrorAction Stop; Write-Host "Backed up old radar cache. Metro will rebuild it from verified v0.1.16 on the next game launch. Backup: $backup" }
```

If another Sobolyatnik-K bundle is installed, **close RTV and uninstall it using Windows *Installed apps* before running the line above**; setup refuses to overwrite a different version. Uninstall preserves Metro, saves and rollback backups. The v0.1.16 ZIP contains both `RtVRadarLoot.vmz` and `rtv-toolkit.exe`; individual downloads are not needed.

The installer verifies the individual files, refuses conflicting mods and bundles, and will not run while the game is open. The same line checks the installed radar and safely backs up any outdated radar-only loader cache before the next launch; it does not edit saves, game binaries, or Metro itself. **Steam build `25710663` was reviewed. If yours differs, setup warns but lets you continue; compatibility on another build is unverified.** Back up important saves before trying a new mod.

**Uninstall:** Close the game and Toolkit, then use *Settings → Apps → Installed apps → Sobolyatnik-K* (or its Start Menu shortcut). Uninstall preserves Metro, saves and backups. See the [v0.1.16 installation guide and known limitations](docs/windows-0.1.16.md) or [download the v0.1.16 ZIP and checksum manually](https://github.com/jtburkh/Sobolyatnik-K/releases/tag/v0.1.16).

## In game

**Shot Alerts** is visible by default: enemy, Nomad and boss gunshots appear as fixed markers and fade after five seconds. `F7` cycles radar views (AI, trails and loot), `F8` hides/shows the HUD, and `F9` cycles 50/100/200/400 m **display** ranges. Player and vehicle shots are excluded. The range does not change AI perception or guarantee detecting gunshots at 400 m. The radar does not modify character saves.

![Actual Road to Vostok gameplay in Village, with the Sobolyatnik-K radar HUD in the upper-right corner](assets/screenshots/in-game-village.webp)

*In-game screenshot from an earlier radar version. This release is experimental; broad combat/crash stability on newer game builds is not established.*

## Windows Toolkit

The included `rtv-toolkit.exe` provides inventory and equipment editing, a live tactical radar, and a Backups tab. Close the game before changing a save; the Toolkit makes `.rtvbak.*` backups, and restoring requires typed confirmation. Its radar `+`/`-` range control is separate from the in-game `F9` setting. See the [radar notes](docs/radar-range-preview.md) and [telemetry configuration](docs/telemetry-installation.md). Do not expose its unauthenticated localhost telemetry listener outside a trusted machine.

<details>
<summary>Toolkit screenshots</summary>

![Character and vitals; personal save path redacted](assets/screenshots/toolkit-character.png)
![Equipment and weapon details; personal save path redacted](assets/screenshots/toolkit-equipment.png)
![Inventory grid; personal save path redacted](assets/screenshots/toolkit-inventory.png)
![Earlier live radar; personal save path redacted](assets/screenshots/toolkit-radar.png)

</details>

<!-- Historical 0.1.18 release metadata retained only for source-level consistency tests; not player installation links.
Sobolyatnik-K-0.1.18-experimental.1-reviewed25710663-Windows.zip
https://github.com/jtburkh/Sobolyatnik-K/releases/download/v0.1.18-experimental.1/RtVRadarLoot.vmz)
https://github.com/jtburkh/Sobolyatnik-K/releases/download/v0.1.18-experimental.1/RtVRadarLoot.vmz.sha256)
https://github.com/jtburkh/Sobolyatnik-K/releases/download/v0.1.18-experimental.1/rtv-toolkit.exe)
https://github.com/jtburkh/Sobolyatnik-K/releases/download/v0.1.18-experimental.1/rtv-toolkit.exe.sha256)
-->

## Development and bug reports

Report problems with your Steam build ID, reproduction steps and **redacted** logs through [GitHub Issues](https://github.com/jtburkh/Sobolyatnik-K/issues); do not post private saves or recovered game resources. Source pushes require a version bump and run an advisory Steam build check; a matching build ID alone does not prove gameplay compatibility. See [runtime and safety research](docs/runtime-discovery.md), [third-party notices](THIRD_PARTY_NOTICES.md), and the [MIT license](LICENSE).
