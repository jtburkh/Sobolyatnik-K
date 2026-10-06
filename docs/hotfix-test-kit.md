# 0.1.13-test.1: controlled Build 25710663 test kit (not a release)

**Status:** owner-only, UNRELEASED Windows test artifact; no tag or GitHub Release. The published v0.1.12-experimental assets and guard remain unchanged. This kit is designed for a clean computer on Road to Vostok Steam build **25710663** (public depot **4116347595363715363**). If Steam changes again, stop and prepare a newly reviewed and versioned kit; do not edit the Steam manifest or bypass the guard. Build identity is **not** a compatibility test.

## One folder, one version

From a successful `hotfix-test-kit` CI job, download `UNRELEASED-0.1.13-test.1-build25710663-clean-test-kit` and extract *all* files, including `metro/`, into one folder. `TEST-README.txt` contains per-file SHA-256 hashes. The artifact contains the versioned VMZ, Windows x64 Toolkit, offline setup, radar installer, uninstaller, wrapper, and hash-pinned upstream Metro Mod Loader 3.2.1 with its MIT license. No connection is required to install the kit. Keep the extracted folder until the test is finished.

On the clean Windows test computer, before running anything: close RTV and the Toolkit; use only a disposable character/save. If a real character exists, back up the entire `%APPDATA%\Road to Vostok` folder to external storage and verify that copy before the test. Never ask the Toolkit preview to restore a production save as part of this test. Confirm the Steam build and inspect other mods/Metro installs; the installer refuses conflicting files and receipts. You can manually compare `Get-FileHash -Algorithm SHA256 .\rtv-toolkit.exe` (and the other hashes) against `TEST-README.txt`.

Open **PowerShell (not as Administrator)** inside the extracted folder and run:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\run-test-install.ps1 -DryRun
# Only after reviewing the printed paths and verifying RTV is closed:
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\run-test-install.ps1
```

If Steam is in a different library, add `-GamePath 'D:\SteamLibrary\steamapps\common\Road to Vostok'` to the dry run and install commands. Do not use the v0.1.12 GitHub setup command with this kit. Setup checks local file hashes, requires the exact supported Steam build, refuses foreign loaders/receipts, makes rollback copies where needed, and does not write saves. No data is sent to a server by the installer; only the game mod's existing localhost telemetry protocol applies while playing.

For test acceptance, confirm: game launches and Metro loads without GDScript errors, the radar defaults to **Shot Alerts**, F7/F8 still operate, F9 cycles 50/100/200/400 m, the Toolkit `+`/`-` changes its own range (not synchronized with HUD), contacts/loot/trails filter by display radius, shot markers stay fixed to the scene and fade in about five seconds, and no native crash occurs during a **limited, documented** disposable-save combat test. Record the game build, timestamps, reproduction steps, relevant *redacted* logs, and whether the save/Metro state remains intact. The 400 m display does **not** guarantee detection of shots at 400 m. The Toolkit Backups tab is a preview; its production restore is Windows-only and should not be exercised on an important save.

Close the game and Toolkit before rolling back:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\run-test-install.ps1 -Uninstall
```

Uninstall verifies ownership and leaves saves, rollback backups and Metro untouched. If Metro was absent before the test, it **remains installed afterward**; this is deliberate to avoid deleting another mod's loader. To return to a truly clean game, compare to the pre-test inventory and remove Metro only after confirming no other mod uses it and making a backup. On any failure, stop, preserve logs and receipts, do not overwrite conflicting files or force-uninstall altered assets. Restoring a save remains a separate, owner-authorized operation while RTV is closed.

## Release-manager gates

| Stage | Required evidence | Outcome |
| --- | --- | --- |
| Every code push | Advance Cargo/package and candidate mod from the remote tip (`0.1.13-test.N`); CI and pre-push check the version; review upstream README edits and run advisory Steam check. | Source update only. No release tag. |
| Test artifact | Rust/Clippy/format/Godot source mocks, deterministic VMZ, actual Windows EXE with the same version, pinned Metro/license, hashes, Windows fixture clean install/build mismatch/tamper/uninstall/Windows integration, exact v0.1.12 upgrade refusal and uninstall/reinstall. | One CI test artifact, not compatibility clearance. |
| Compatibility decision | `python3 tools/check_steam_build.py --strict --installer <candidate install-sobolyatnik.ps1>` plus independent game test on a disposable save, logs, source-contract review, rollback and owner sign-off. | Only then decide whether to publish a *separately versioned* experimental release. |
| Publication | Immutable tag and assets; record SHA-256s, provenance, release notes, known limitations and install/uninstall instructions. Do not retag or replace an old release. Smoke-test the downloaded assets, not just the workspace files. | Public experimental release, if specifically approved. |

`--strict` without `--installer` intentionally checks published v0.1.12 and still fails on Build 25710663. The candidate-specific strict identity check passing does not mean runtime compatibility or broad combat safety. The latest Build 2 catalog and save/schema changes require independent gameplay review. Do not publish private recovered scripts, saves or Beads data. A later code push must use `0.1.13-test.2` (or a newer version), even if the change is just a build script.
