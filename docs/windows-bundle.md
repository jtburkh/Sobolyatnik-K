# Sobolyatnik-K for Windows: radar + Toolkit

The [**v0.1.11-experimental** prerelease](https://github.com/jtburkh/Sobolyatnik-K/releases/tag/v0.1.11-experimental)
installs the in-game radar, a **Windows x64 `rtv-toolkit.exe`**, and a safe
uninstall option. It is experimental, **unsigned**, and tested only for Road to
Vostok Steam build **25632875**. Install the game through Steam, **close it**,
then paste this **one line into PowerShell**:

```powershell
$ErrorActionPreference='Stop'; $p=Join-Path $env:TEMP 'sobolyatnik-k-v0.1.11.ps1'; Invoke-WebRequest -UseBasicParsing -Uri 'https://github.com/jtburkh/Sobolyatnik-K/releases/download/v0.1.11-experimental/setup-sobolyatnik.ps1' -OutFile $p; if ((Get-FileHash -LiteralPath $p -Algorithm SHA256).Hash -ne 'A2CBBFA5AA1D6CCC6DA9B664CAD29ECD9A0B1BC4FDA6054E29FDF4BE13E98388') { throw 'Installer checksum mismatch; nothing was executed' }; & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $p
```

The script is checked **before execution**. It downloads only pinned release
assets, verifying the Toolkit (`c0d0d08528465d574666f3956a41d3ae7f17b810679bb9ae5f48dff8f2c732a3`),
uninstaller (`27088abfc7d10040bf624fd79b47ca99dd7a7d2b0eb7c33310e91d41a044209d`),
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
  replacing a save. This **v0.1.11 executable includes the Build 2
  [catalog](catalog-sync.md)** (263 inventory items, 15 new); the terminal radar
  matches the in-game enemy/Nomad/boss contact colors. Keep your own save
  backup and avoid the historical `--spawn-*` commands. Source-derived item
  footprints are not a substitute for in-game slot testing.
- Setup can adopt the exact previously installed v0.1.8 VMZ without rewriting
  Metro's live `override.cfg`. It refuses different Metro versions, changed
  loader scripts, partial loader installations, other installed VMZs, ambiguous
  Steam paths, and a different Steam game build. A replaced radar VMZ receives
  a hash-verified backup in `%LOCALAPPDATA%\Sobolyatnik-K\backups\`.
- The executable is **not code-signed**. Windows may display a publisher or
  SmartScreen warning. Verify the release source, hashes, and your trust in the
  publisher before running it; never disable security software globally.

## Upgrade from v0.1.9

**Close the game.** Use Windows *Installed Apps* or the existing Start Menu
**Uninstall Sobolyatnik-K** entry to remove the old bundle, then run the pinned
v0.1.11 command above. The new installer **refuses to overwrite** a v0.1.9
Toolkit/receipt; there is no in-place upgrade. Old uninstall removes its
verified Toolkit and radar only, retaining Metro (including `override.cfg`),
saves, backups and game binaries. If uninstall reports modified files or an
unknown receipt, stop and inspect them rather than deleting anything by hand.

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
  ([v0.1.11 release workflow](https://github.com/jtburkh/Sobolyatnik-K/actions/runs/37095644880)).
  The exact assets exercised in the release fixture were then independently
  downloaded from the public release and matched their SHA-256 hashes. The
  fixture covers both clean install and v0.1.9 uninstall/reinstall while
  retaining its synthetic save and Metro. This is **not** a broad real-game
  crash-clearance claim.

See [telemetry configuration](telemetry-installation.md) for optional live
terminal radar networking and [diagnostics](telemetry-diagnostics.md) for
Build 2 limitations. The tested radar VMZ is byte-for-byte identical to the
v0.1.8 release; gunshots, sensor/vision reads, and summon/artifact features
remain unavailable.
