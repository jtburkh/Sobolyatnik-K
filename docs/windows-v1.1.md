# Sobolyatnik-K v1.1 — Windows

Use the **[one-line checksum-verifying install command in the README](../README.md#install-on-windows)** once it recommends v1.1. The GitHub [v1.1.0 release](https://github.com/jtburkh/Sobolyatnik-K/releases/tag/v1.1.0) supplies the full `Sobolyatnik-K-v1.1-Windows.zip` and adjacent `.sha256`: the archive includes the in-game radar VMZ, `rtv-toolkit.exe`, installers/uninstaller, and hash-pinned official MIT Metro Mod Loader 3.4.2 with its license. The GitHub release may also provide the VMZ and EXE as separate SHA-256-checked downloads; **neither alone is a full installer**. A future Nexusmods VMZ-only package is tracked separately and is **not** the Windows Toolkit bundle.

## Before installation

1. **Close Road to Vostok and the Toolkit.** Back up important saves under `%APPDATA%\Road to Vostok` if you rely on them. Setup and uninstall will refuse while RTV is running.
2. If an older Sobolyatnik-K Windows bundle is installed, uninstall it through Windows **Installed apps** before installing v1.1. The new installer refuses to overwrite a different receipt, EXE, Start Menu entry or registry registration. Uninstall preserves Metro, saves and existing rollback backups. Do **not** delete the loader or manually replace an owned VMZ or EXE.
3. Use the README's single PowerShell command only after its public v1.1 ZIP and checksum have been verified. It downloads and validates the ZIP before extraction and runs its offline setup. Do not run a script from an unverified or partially extracted archive.

## What setup does

The bundle verifies its individual file hashes. Fresh installs use Metro 3.4.2; an already installed **hash-verified official** Metro 3.2.1 or 3.4.2 remains untouched. Partial, patched or unknown loaders and conflicting VMZs block installation. Setup reads the Steam appmanifest and refuses missing/invalid game information or a running RTV process. Build **25837777** (RTV 0.2.1.0) was source-reviewed; a different *valid* Steam build triggers an explicit compatibility warning but allows installation. That warning is at install time only, not on every game launch.

If only the Sobolyatnik-K radar's Metro VMZ mount cache is stale, setup verifies the installed VMZ, creates a hash-checked backup of that single cache file, then removes it so Metro rebuilds it next launch. It does not change other mods' caches, user preferences, saves, game binaries, or an existing Metro script. Backups are retained; no automatic pruning.

## In game and Toolkit

Shot Alerts begins visible: AI gunshots appear as fixed scene-local markers and fade after five seconds. **F7** cycles modes, **F8** shows/hides the HUD, and **F9** changes the 50/100/200/400 m *display* range. The Toolkit has separate `+`/`-` radar-range controls. The Toolkit's offline catalog includes 273 inventory items from build 25837777, including Antti's ten 0.2.1.0 additions, and 45 furniture resources; no older known item paths were removed. Save editing remains offline-only, with `.rtvbak.*` backups and typed RESTORE confirmation. Keep a separate copy of important saves.

The mod does **not** rewrite `AI.PlayFire`, alter AI perception, or guarantee observing shots at 400 m. Source review, mock Godot tests and synthetic installer fixtures do **not** establish broad combat/crash stability, independent-PC compatibility, new item slot behavior, or safety on every future Steam build. Please report issues with the Steam build ID and redacted logs; do not attach saves or proprietary game resources.

**Uninstall:** Close RTV and the Toolkit, then use Windows **Installed apps → Sobolyatnik-K** or the installed Start Menu uninstaller. Only verified owned radar/Toolkit files are removed; Metro, saves, and rollback backups remain.
