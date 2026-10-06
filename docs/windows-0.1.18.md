# Sobolyatnik-K 0.1.18 Experimental — Windows

Use the **[one-line installer in the README](../README.md#install-on-windows)** for the current release. The bundle contains the in-game radar, Windows Toolkit, checked setup/uninstall scripts and the official Metro Mod Loader 3.2.1 with its MIT license. You can also [download the ZIP and SHA-256 checksum manually](https://github.com/jtburkh/Sobolyatnik-K/releases/tag/v0.1.18-experimental.1); extract *all* files, including `metro/`, into one folder and run `powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\run-sobolyatnik.ps1` from that folder. The release also offers [the mod VMZ](https://github.com/jtburkh/Sobolyatnik-K/releases/download/v0.1.18-experimental.1/RtVRadarLoot.vmz) and [the Windows x64 Toolkit EXE](https://github.com/jtburkh/Sobolyatnik-K/releases/download/v0.1.18-experimental.1/rtv-toolkit.exe) individually with their own `.sha256` files; they do not install Metro or provide guarded rollback on their own.

**Before installing:** Close Road to Vostok and the Toolkit. Back up `%APPDATA%\Road to Vostok` if you have a character you want to keep. The installation has only been source-reviewed against Steam build **25710663**. If your Steam appmanifest reports another valid build, the installer **warns and continues**; the mod may not work on that build. It does not automatically check your game again when Steam updates an existing installation. Missing or malformed Steam appmanifests, wrong app IDs, conflicting mod files, modified loaders/receipts or a running game still stop installation. A previous prototype's `[radar_lite] controls=false` or `overlay=false` setting may remain in `rtv-telemetry.cfg` even after uninstall; this version **ignores those two legacy settings** so F9 and the HUD work on the new player bundle. It does not rewrite or delete that file, other config preferences, or saves.

## Inspect, install and uninstall manually

In PowerShell inside the extracted ZIP folder, preview the install plan without writing to the game:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\run-sobolyatnik.ps1 -DryRun
```

Install if you are comfortable with the game build warning and the reported paths:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\run-sobolyatnik.ps1
```

If you keep Steam in another library, append `-GamePath 'D:\SteamLibrary\steamapps\common\Road to Vostok'`. The bundle refuses to overwrite an older installation receipt: close RTV and uninstall the old bundle using its own Windows *Installed apps* entry, then run the new setup. Do not edit the Steam manifest, loader or receipt to bypass other safety checks.

For uninstall, **close the game and Toolkit**, then use the Windows *Installed apps* entry or run:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\run-sobolyatnik.ps1 -Uninstall
```

Uninstall only removes hash-verified owned files; it leaves saves, backups, Metro and other mods alone. Metro stays on disk if it was first installed with the bundle, because other mods may depend on it. Inspect and remove it separately only when it is safe to do so.

## What to expect and report

`F7` cycles HUD modes; `F8` hides/shows it; `F9` cycles 50/100/200/400 m **display** ranges. A persistent `F9 TOGGLE DISTANCE` label below the HUD displays the chosen range while playing. Toolkit `+`/`-` controls its own independent radar range. Shot Alerts is visible by default; scene-local markers fade after five seconds. The 400 m display does not guarantee observing gunshots at 400 m. This release is **experimental**. Installer fixtures and Godot mocks are not a promise of crash-free play on every game build; for problems, include the Steam build ID, steps to reproduce and **redacted** logs in [GitHub Issues](https://github.com/jtburkh/Sobolyatnik-K/issues). Never post private saves or game resources publicly.
