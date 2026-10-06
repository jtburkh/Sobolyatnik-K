# Sobolyatnik-K 0.1.14 Experimental — Windows installation

[Download the Windows release](https://github.com/jtburkh/Sobolyatnik-K/releases/tag/v0.1.14-experimental.1) for Road to Vostok **Steam build 25710663**. The ZIP includes the in-game radar VMZ, Windows x64 Toolkit, installer, uninstaller and official Metro Mod Loader 3.2.1 with its MIT license. The radar opens in **Shot Alerts** mode; `F7` cycles modes, `F8` hides/shows the HUD, and `F9` changes its 50/100/200/400 m display range. In the Toolkit, `+`/`-` adjust its own independent radar display range. Ranges filter what is drawn, not AI perception or shot detection.

**Experimental:** This is the first public bundle targeting game build 25710663. The installer and component checks passed Windows test fixtures, but that is not a guarantee of crash-free gameplay. Keep a backup of important saves; a disposable character is sensible for a first run. Do not use a backup restore on an important save until you've confirmed its behavior for your setup. The 400 m radar view does not guarantee hearing shots at 400 m.

## Install

1. Close Road to Vostok and the Toolkit. Back up `%APPDATA%\Road to Vostok` if you have a character you want to keep.
2. Download [`Sobolyatnik-K-0.1.14-experimental.1-build25710663-Windows.zip`](https://github.com/jtburkh/Sobolyatnik-K/releases/download/v0.1.14-experimental.1/Sobolyatnik-K-0.1.14-experimental.1-build25710663-Windows.zip) and its [adjacent `.sha256` file](https://github.com/jtburkh/Sobolyatnik-K/releases/download/v0.1.14-experimental.1/Sobolyatnik-K-0.1.14-experimental.1-build25710663-Windows.zip.sha256). Verify the ZIP hash before extracting it. `README.txt` inside lists hashes for every component.
3. Extract the *entire ZIP*, including `metro/`, into one folder. Open PowerShell in that folder **without Administrator privileges** and check the installation plan:

   ```powershell
   powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\run-sobolyatnik.ps1 -DryRun
   ```

4. If the game path, build and files look right, install:

   ```powershell
   powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\run-sobolyatnik.ps1
   ```

If Steam is installed in a different library, pass `-GamePath 'D:\SteamLibrary\steamapps\common\Road to Vostok'` to both commands. The installer checks hashes, requires the exact Steam build, refuses conflicting mod files and other bundle receipts, and does not write character saves or game binaries. It needs no internet connection after you have downloaded the ZIP. Do **not** edit a Steam manifest or bypass a build refusal. An older installed bundle must be removed using its own verified uninstaller first.

## Uninstall and report an issue

Close the game and Toolkit, then, from the extracted folder, run:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\run-sobolyatnik.ps1 -Uninstall
```

Uninstall removes only verified files owned by this bundle. It deliberately keeps Metro, saves and rollback backups. If Metro was not present before installing, it will still be present afterward; remove it separately only after verifying that no other mod relies on it. If any file has changed, the uninstaller stops rather than deleting someone else's work.

If something fails, note the Steam build, what you did, the location in the game, and what you expected versus what happened. Keep relevant **redacted** logs and report the problem on [GitHub Issues](https://github.com/jtburkh/Sobolyatnik-K/issues). Do not upload character saves, recovered game resources or personal paths publicly. Never install the diagnostic bridge alongside this radar.

## Release integrity

This release is **separately versioned** from v0.1.12 and v0.1.13-test.2; neither older release is edited or retagged. The release workflow builds the matching Windows MSVC EXE, packs the range/Shot Alerts VMZ and bundled Metro, tests clean and mismatched-build installs, installation receipts, Windows shortcuts, v0.1.12 upgrade refusal and uninstall/reinstall, then checks the ZIP before publishing. The live Steam build identity check is separate from game compatibility. Check the downloaded ZIP's checksum, not just a source-tree build. A future code push must increment the Cargo and mod versions first.
