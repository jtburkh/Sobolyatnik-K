# Sobolyatnik-K for Windows: Shot Alerts radar + Toolkit

The [**v0.1.12-experimental** prerelease](https://github.com/jtburkh/Sobolyatnik-K/releases/tag/v0.1.12-experimental)
installs the **new Shot Alerts radar VMZ**, Windows x64 `rtv-toolkit.exe`, and a
safe uninstall option. It is experimental, **unsigned**, and intended only for
Road to Vostok Steam build **25632875**. Install the game through Steam,
**close it**, then paste this **one line into PowerShell**:

```powershell
$ErrorActionPreference='Stop'; $p=Join-Path $env:TEMP 'sobolyatnik-k-v0.1.12.ps1'; Invoke-WebRequest -UseBasicParsing -Uri 'https://github.com/jtburkh/Sobolyatnik-K/releases/download/v0.1.12-experimental/setup-sobolyatnik.ps1' -OutFile $p; if ((Get-FileHash -LiteralPath $p -Algorithm SHA256).Hash -ne 'E4BED9C344E0FBE89492F4C833EAB0C3245CBBE6519109E0AED212F1A27F6775') { throw 'Installer checksum mismatch; nothing was executed' }; & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $p
```

The setup script is checked **before execution**. It verifies each release
asset again before use: Shot Alerts VMZ
`6d2d06e55831f174483595d44fde6ba5c3a50f7fec183d2c66a5953ecb98efdd`,
Windows Toolkit `0a82b65014522d63a804fdcf577173b953e046e49d200681d1ddf1b33cba8bd7`,
radar installer `cf66879c05c3a8404458e24b1e75f6943895c604a497398551db8535ba01c39d`,
and uninstaller `9a0620b45a1c62395c2cf3dc897efcd30f9600a517e8922a648e1257cadcb877`.
On a clean game, the pinned radar installer downloads and verifies both official
[Metro Mod Loader 3.2.1 assets](https://github.com/ametrocavich/vostok-mod-loader/releases/tag/v3.2.1).
Do not use an unversioned `irm | iex` command. The policy bypass applies only
to the child process running this hash-verified setup, not to the system policy.

## After installation

- The in-game radar **starts visible in Shot Alerts**, with only human-AI shot
  markers fading for five seconds. The panel stays visible between shots. `F7`
  cycles Shot Alerts → All + Shots → AI Only → Trails Only → Loot Only (when
  enabled); `F8` hides/shows the HUD. Player and vehicle shots are excluded.
- Launch **Sobolyatnik-K Toolkit** from the Start Menu; it is also installed at
  `%LOCALAPPDATA%\Programs\Sobolyatnik-K\rtv-toolkit.exe`. The in-game radar
  works without opening the Toolkit. Live terminal radar and shot alerts need
  the mod and Toolkit to use the same UDP address (normally `127.0.0.1:47777`).
- Edit your character's equipment and inventory **only while Road to Vostok
  is closed**. The Toolkit keeps a timestamped `.rtvbak.*` backup before
  replacing a save. The Build 2 [catalog](catalog-sync.md) contains 263 derived
  inventory items, 15 newly registered. Keep your own backups; some new slot
  footprints still need a disposable-save in-game check. Avoid historical
  `--spawn-*` commands.
- A radar-only v0.1.8 installation can be adopted; if its VMZ differs, setup
  saves a verified backup in `%LOCALAPPDATA%\Sobolyatnik-K\backups\`. It will
  not rewrite an existing Metro `override.cfg`. Different Metro versions,
  changed loader scripts, partial installations, other VMZs, ambiguous Steam
  paths, and other game builds are refused.
- The executable is **not code-signed**. Windows may show a publisher or
  SmartScreen warning. Verify the release, hashes and publisher before running
  it; never disable your security software globally.

## Upgrade from an older Windows bundle

**Close the game first.** If v0.1.9 or v0.1.11 is installed, use Windows
*Installed Apps* or that version's existing Start Menu **Uninstall
Sobolyatnik-K** entry. Then run the v0.1.12 setup command above. There is
**no in-place bundle upgrade**: the new installer refuses to overwrite another
version's receipt or Toolkit executable. The earlier version's uninstaller
removes only its verified radar VMZ and Toolkit, retaining Metro (including
`override.cfg`), saves, backups, and game binaries. If uninstall reports a
modified file or unknown receipt, stop and investigate rather than deleting
anything by hand. Previous versioned releases were **not changed**.

## Remove v0.1.12

**Close the game first.** Use **Start → Sobolyatnik-K → Uninstall Sobolyatnik-K**
or **Settings → Apps → Installed apps → Sobolyatnik-K (Experimental) → Uninstall**.
The uninstaller checks its per-user receipt at
`%LOCALAPPDATA%\Sobolyatnik-K\installed.json` and refuses to delete a VMZ or
Toolkit whose hash has changed. It removes only this release's verified radar
and Toolkit, shortcuts, and Windows uninstall entry; it leaves Metro, saves,
backup archives and game binaries alone. It refuses to run while RTV is open.
For the **old v0.1.8 radar-only installer**, which has no receipt or Toolkit,
close RTV and move `RtVRadarLoot.vmz` out of `mods` manually; don't remove a
shared Metro installation.

## Troubleshooting and evidence

- If Steam updated the game, setup refuses builds other than **25632875**;
  don't force an incompatible VMZ onto a new build.
- If a file has changed or another mod owns a path, setup or uninstall will
  refuse to overwrite/delete unknown bytes. There is no force-delete option.
  If setup fails after installing the radar, investigate before retrying.
- Setup and uninstall never auto-elevate. Your account must have write access
  to the Steam library. Neither script changes saves or game binaries;
  bootstrapping Metro on a clean game **does** add its two startup files.
- [Windows CI for the tagged release](https://github.com/jtburkh/Sobolyatnik-K/actions/runs/37135568447)
  built and tested the Windows Toolkit and exercised exact versioned release
  assets on disposable Steam/Metro fixtures: clean install, radar-only
  adoption, v0.1.11 refusal/uninstall/reinstall, hashes, Windows entries,
  `rtv-toolkit.exe --help`, safe removal, and save/Metro retention. All five
  public assets were independently downloaded, SHA-256 checked and tested in
  a second disposable Windows fixture without desktop integration. Neither
  that fixture nor the release workflow installed anything in the real game.
  One earlier owner-approved candidate gameplay session confirmed Shot Alerts
  behavior; it is **not** prolonged combat or native-crash clearance.

For optional terminal networking see [telemetry configuration](telemetry-installation.md).
For implementation and test limits see [the shot observer](ai-shot-observer.md).
Do not combine this mod with the diagnostic bridge or restore the obsolete
`AI.PlayFire` hook.
