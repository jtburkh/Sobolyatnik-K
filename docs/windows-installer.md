# Experimental Windows installer

The [published `v0.1.8-experimental` prerelease](https://github.com/jtburkh/Sobolyatnik-K/releases/tag/v0.1.8-experimental)
installs the in-game radar and Metro only. It **does not install the terminal
Toolkit or provide an uninstaller**. A [separate Toolkit bundle](windows-bundle.md)
is being tested; it does not change this version or its checksum. Do not copy an
unversioned `main` branch script and pipe it to `iex`/`Invoke-Expression`.

With Road to Vostok closed, paste this **one line into PowerShell**:

```powershell
$ErrorActionPreference='Stop'; $p=Join-Path $env:TEMP 'sobolyatnik-k-v0.1.8.ps1'; Invoke-WebRequest -UseBasicParsing -Uri 'https://github.com/jtburkh/Sobolyatnik-K/releases/download/v0.1.8-experimental/install-sobolyatnik.ps1' -OutFile $p; if ((Get-FileHash -LiteralPath $p -Algorithm SHA256).Hash -ne 'FC8B4D38F2373B750BD9457BD3D779A5CE613E541F6643AEFFD99634B3F0DF72') { throw 'Installer checksum mismatch; nothing was executed' }; & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $p
```

The installer script and VMZ are both open to inspection in this repository.
The pinned script is checked **before execution**. It checks the radar VMZ
SHA-256 `66fd97a4c1487be6688ef94b59ee709a3baa8c7886c490d2f03af7b8c395eefc`
and both official upstream Metro 3.2.1 release assets (modloader.gd:
`60fcf7feec0a47c6472e3b7a190b46987b374618ae3d149c081b222542bc6135`,
override.cfg: `9750a66fcf0cb1d9bf84284271f52e064f455cdd5a1cdc4981a007e4011a684b`)
before touching the game directory. PowerShell's policy bypass applies only to
the child process running this hash-verified script; it does not change the
system policy. The pinned release includes **only** the radar VMZ and installer,
not the quarantined full bridge or test probes.

## Prerequisites and safeguards

- **Windows/Steam**, Road to Vostok **build 25632875**, game closed.
- On a **clean** game, the script downloads Metro Mod Loader **3.2.1** directly
  from the [official GitHub release](https://github.com/ametrocavich/vostok-mod-loader/releases/tag/v3.2.1),
  checks both upstream asset hashes, and installs only `modloader.gd` and
  `override.cfg` next to `RTV.exe`. That first-time loader installation changes
  game startup behavior, so **read the installer before running the one-liner**.
  If both files already exist, the script verifies the loader version and does
  not overwrite either file. A partial or different-version loader requires
  manual resolution; the script never silently replaces it. No game binaries,
  saves, or user configuration are edited.
- Steam libraries (including non-default libraries) are discovered from Steam
  registry data and `libraryfolders.vdf`. Ambiguous installations require an
  explicit `-GamePath`. If another VMZ is installed, the script refuses to
  proceed rather than silently removing a third-party mod.
- If an older `RtVRadarLoot.vmz` exists, it is backed up under
  `%LOCALAPPDATA%\Sobolyatnik-K\backups\` and hash-verified before an atomic
  replacement. Re-running with the same verified archive is a no-op.
- `-DryRun` and `-ArchivePath <local VMZ>` support inspection and offline tests.
  Pre-install validation failures leave existing files in place; a newly added
  loader is removed if installing the VMZ fails before commit. The old VMZ is
  retained as a verified rollback before replacement. With the game closed,
  remove **the mod alone** by moving `RtVRadarLoot.vmz` out of `mods`;
  do not remove Metro or edit the character save as part of this operation.

This is an **experimental** Build 2 mod, not crash-free or universally
compatible. Wider stability testing is still in progress.
