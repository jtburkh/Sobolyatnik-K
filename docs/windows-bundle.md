# Proposed Windows Toolkit bundle (not released)

The published `v0.1.8-experimental` installer **only** installs the Metro loader
and the in-game radar. Do not expect it to install `rtv-toolkit.exe`, show a
Start Menu shortcut, or provide an uninstaller. Do not replace that already
published hash-pinned script or tag in place.

A separate **`v0.1.9-experimental`** release is in preparation. It will retain
the **exact** 0.1.8 VMZ (`66fd97a4c1487be6688ef94b59ee709a3baa8c7886c490d2f03af7b8c395eefc`)
while adding a tested Windows x64 `rtv-toolkit.exe`, source-visible setup script,
and uninstall script. Windows CI must compile and execute the binary and test
installation and uninstall against disposable Steam fixtures before publication.
The build job will embed the **actual tested binary SHA-256** in the setup
script; only after its release asset is independently downloaded and checked
should a pinned one-line PowerShell command replace the old command in the
README. Until then **there is no new one-line installer to run**. Do not run
`tools/setup-sobolyatnik.ps1.in` from `main`: it is a source template, not a
published installer. Windows may warn that the new executable is unsigned;
this project does not claim code signing.

## Planned behavior

- Setup verifies the installer, executable and uninstaller SHA-256s, invokes
  the exact v0.1.8 radar installer by its pinned SHA-256, checks game-closed
  state and Steam build 25632875, and writes the Toolkit under
  `%LOCALAPPDATA%\Programs\Sobolyatnik-K\` for the current Windows user.
- It creates a Start Menu launch/uninstall shortcut and a per-user Windows
  Installed Apps uninstall entry. It can adopt a **verified** v0.1.8 VMZ on a
  second computer without replacing it; an existing Metro override/config
  is never overwritten. VMZ rollback backups are retained.
- Uninstall requires Road to Vostok to be closed and refuses to delete modified
  Toolkit or VMZ files. It removes only the verified radar VMZ and Toolkit,
  preserving Metro Mod Loader (which other mods may use), saves, and backups.
  Existing users of the old radar-only installer can still remove just the
  VMZ manually with the game closed; that version has no ownership receipt.
- `-DryRun`, local asset paths, and disposable fixture mode exist for review
  and tests. A failed setup may leave the already-installed radar in place;
  rerunning setup is safer than deleting files or altering saves by hand.

## If something goes wrong

- **Unsupported Steam build or unexpected Metro/VMZ:** setup stops rather than
  rewriting another mod or version. Do not delete game files to force it.
- **Toolkit or VMZ modified after installation:** uninstall refuses to remove
  unknown bytes. Inspect the changed files and preserve backups before trying
  again; it has no force-delete switch.
- **Existing v0.1.8 installation:** the planned bundle can adopt the exact VMZ
  without overwriting Metro's live `override.cfg`. The old v0.1.8 script itself
  will never grow an uninstaller; its manual removal instructions remain in
  [the radar-only guide](windows-installer.md).
- **Permissions or a running game:** close Road to Vostok before setup/uninstall
  and use an account allowed to write to that Steam library. The scripts do not
  auto-elevate or touch any user's character save.

The terminal's item catalog is still from an older Road to Vostok build.
**Close the game before using its offline save editor**, retain separate save
backups, and do not invoke the historical `--spawn-*` options with the current
radar bridge. No signing or broad crash-clearance claim is implied by bundling
the executable.
