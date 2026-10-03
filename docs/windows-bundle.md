# Sobolyatnik-K for Windows: radar + Toolkit

The [**v0.1.9-experimental** prerelease](https://github.com/jtburkh/Sobolyatnik-K/releases/tag/v0.1.9-experimental)
installs the in-game radar, a **Windows x64 `rtv-toolkit.exe`**, and a safe
uninstall option. It is experimental, **unsigned**, and tested only for Road to
Vostok Steam build **25632875**. Install the game through Steam, **close it**,
then paste this **one line into PowerShell**:

```powershell
$ErrorActionPreference='Stop'; $p=Join-Path $env:TEMP 'sobolyatnik-k-v0.1.9.ps1'; Invoke-WebRequest -UseBasicParsing -Uri 'https://github.com/jtburkh/Sobolyatnik-K/releases/download/v0.1.9-experimental/setup-sobolyatnik.ps1' -OutFile $p; if ((Get-FileHash -LiteralPath $p -Algorithm SHA256).Hash -ne 'C7F5D7BEB4355840875BB32CE7DDED0C0451B56E3D51B3B5648129BE6F91F2AC') { throw 'Installer checksum mismatch; nothing was executed' }; & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $p
```

The script is checked **before execution**. It downloads only pinned release
assets, verifying the Toolkit (`5e8d95fda3208aecda403cffe68578e5965fd59b0145b71fd390c991f244efa4`),
uninstaller (`38caa7f514ec19e5becdd948d48d14bd7447aad37e35650f08af4952116821a1`),
the [v0.1.8 radar installer](windows-installer.md)
(`fc8b4d38f2373b750bd9457bd3d779a5ce613e541f6643aeffd99634b3f0df72`),
and its radar VMZ (`66fd97a4c1487be6688ef94b59ee709a3baa8c7886c490d2f03af7b8c395eefc`).
On a clean game, that pinned installer fetches and verifies both official
[Metro Mod Loader 3.2.1 assets](https://github.com/ametrocavich/vostok-mod-loader/releases/tag/v3.2.1).
Do not use an unversioned `irm | iex` command. The policy bypass applies only
to the child process running this hash-verified script; it does not change the
system execution policy.

## After installation

- Launch **Sobolyatnik-K Toolkit** from the Start Menu. It is also installed at
  `%LOCALAPPDATA%\Programs\Sobolyatnik-K\rtv-toolkit.exe` for your Windows
  account. The *in-game* radar works without opening this program. `F7` cycles
  radar layers; `F8` hides drawing without stopping telemetry.
- The Toolkit can inspect and edit your character's equipment and inventory
  **while Road to Vostok is closed**. It keeps a timestamped save backup before
  replacing a save. **This v0.1.9 executable still contains the older-build
  item catalog.** New Build 2 [catalog metadata](catalog-sync.md) is on `main`
  but needs a *new* Windows executable release; keep your own save backup and
  avoid the historical `--spawn-*` commands.
- Setup can adopt the exact previously installed v0.1.8 VMZ without rewriting
  Metro's live `override.cfg`. It refuses different Metro versions, changed
  loader scripts, partial loader installations, other installed VMZs, ambiguous
  Steam paths, and a different Steam game build. A replaced radar VMZ receives
  a hash-verified backup in `%LOCALAPPDATA%\Sobolyatnik-K\backups\`.
- The executable is **not code-signed**. Windows may display a publisher or
  SmartScreen warning. Verify the release source, hashes, and your trust in the
  publisher before running it; never disable security software globally.

## Remove it

**Close the game first.** Use **Start → Sobolyatnik-K → Uninstall Sobolyatnik-K**
or **Settings → Apps → Installed apps → Sobolyatnik-K (Experimental) → Uninstall**.
The uninstaller reads a per-user receipt in
`%LOCALAPPDATA%\Sobolyatnik-K\installed.json` and refuses to delete a VMZ or
Toolkit executable whose hash has changed. It removes only this release's
verified radar VMZ, Toolkit, shortcuts and Windows uninstall entry. It
**leaves Metro** (other mods may depend on it), saves, and rollback backups
alone. If the game is running, it stops without removing files.

For the **older v0.1.8 radar-only installer**, no uninstall receipt or Toolkit
exists: close the game and move `RtVRadarLoot.vmz` out of `mods` manually. Do
not remove a shared Metro installation just to remove this mod.

## Troubleshooting and evidence

- If Steam updated the game, setup refuses builds other than **25632875**;
  don't force an incompatible VMZ onto a newer build.
- If a loader/mod/Toolkit file is modified or another mod owns a path, setup
  or uninstall refuses to overwrite/delete unknown bytes. Inspect the files;
  there is no force-delete option. If setup fails after installing the radar,
  rerun the same installer instead of editing game files by hand.
- Setup and uninstall never auto-elevate. Your Windows account must have write
  access to the Steam game library. Neither script changes saves or game
  binaries; installing Metro on a clean game **does** add its two startup files.
- Windows CI compiled, ran and tested the executable and exercised installer,
  Start Menu, Installed Apps, and removal on disposable Steam fixtures
  ([release workflow](https://github.com/jtburkh/Sobolyatnik-K/actions/runs/37087189740)).
  A separate end-to-end check downloaded **all public assets** into a fake Steam
  installation, launched `rtv-toolkit.exe --help`, uninstalled, and verified the
  fake save, rollback backup, and Metro remained unchanged. This is **not** a
  broad real-game crash-clearance claim.

See [telemetry configuration](telemetry-installation.md) for optional live
terminal radar networking and [diagnostics](telemetry-diagnostics.md) for
Build 2 limitations. The tested radar VMZ is byte-for-byte identical to the
v0.1.8 release; gunshots, sensor/vision reads, and summon/artifact features
remain unavailable.
