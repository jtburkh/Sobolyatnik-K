# Sobolyatnik-K — Sable Hunter 1L108K

Sobolyatnik-K is an unofficial mod for **Road to Vostok**: an in-game radar for tracking people, movement trails, and loot, plus the RtV Toolkit terminal app for managing your character while the game is closed. Not affiliated with the developer. Many thanks to Antti for making a fun game!

## Install the Radar and Toolkit (Windows / Steam)

Install Road to Vostok **Steam build 25632875**, close the game, then paste this **one line into PowerShell**:

```powershell
$ErrorActionPreference='Stop'; $p=Join-Path $env:TEMP 'sobolyatnik-k-v0.1.12.ps1'; Invoke-WebRequest -UseBasicParsing -Uri 'https://github.com/jtburkh/Sobolyatnik-K/releases/download/v0.1.12-experimental/setup-sobolyatnik.ps1' -OutFile $p; if ((Get-FileHash -LiteralPath $p -Algorithm SHA256).Hash -ne 'E4BED9C344E0FBE89492F4C833EAB0C3245CBBE6519109E0AED212F1A27F6775') { throw 'Installer checksum mismatch; nothing was executed' }; & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $p
```

The command checks the setup script's SHA-256 **before running it**. The script checks every download, installs [official Metro Mod Loader 3.2.1](https://github.com/ametrocavich/vostok-mod-loader/releases/tag/v3.2.1) if neither loader file exists, and installs the [**v0.1.12-experimental Shot Alerts bundle**](https://github.com/jtburkh/Sobolyatnik-K/releases/tag/v0.1.12-experimental): a **new** radar VMZ and Windows x64 `rtv-toolkit.exe`. It adds Start Menu launch/uninstall shortcuts and a Windows *Installed Apps* entry. A radar-only v0.1.8 installation can be adopted with a verified backup; unsupported builds and conflicting mods are refused. **Already installed the v0.1.9 or v0.1.11 bundle?** Close the game, uninstall that version from Windows *Installed Apps*, then run this command. Setup refuses to overwrite its receipt or executable. Uninstall retains Metro, saves and backups. See [installation and safety details](docs/windows-bundle.md).

**Uninstall:** Close the game, then choose **Uninstall Sobolyatnik-K** from the Start Menu (or *Settings → Apps → Installed apps*). Only the verified radar VMZ and Toolkit are removed; Metro, saves, and rollback backups stay. The earlier [v0.1.8 radar-only installer](docs/windows-installer.md) did not provide an uninstaller, and its release was not changed.

![Concept illustration of the Sobolyatnik-K radar unit](assets/SOBOLYATNIK-K.png)

*AI-generated concept illustration of the fictional unit—not an in-game screenshot or an equippable item.*

## What Sobolyatnik-K does in game

The compact, dark tactical HUD starts in the **visible, shot-only Shot Alerts** mode. `F7` cycles the other modes with **red enemies**, **blue Nomads**, **pink bosses**, movement trails and hollow loot markers; `F8` hides or shows the drawing without stopping telemetry. The radar is independent of equipped inventory items and does not modify your character save.

![Actual Road to Vostok gameplay in Village, with the Sobolyatnik-K radar HUD in the upper-right corner](assets/screenshots/in-game-village.webp)

*Actual in-game screenshot from an earlier radar version, not the concept illustration above; the radar appears in the upper-right corner.*

This is an **experimental Build 2** mod, not a promise of crash-free play. AI sensor/vision reads, the old artifact, and summon commands are **not** part of the released playable radar. Do not install the diagnostic bridge alongside it. See [stability and feature limitations](docs/telemetry-diagnostics.md).

### Shot Alerts — included in v0.1.12

**Shot Alerts is the first, visible radar mode.** Only human-AI gunshots (enemies, Nomads and bosses) within 100 m appear as fixed-position markers that fade over five seconds. The radar itself stays on screen between shots. `F7` cycles **Shot Alerts → All + Shots → AI Only → Trails Only → Loot Only** and back (skipping Loot Only when disabled); `F8` hides/shows the radar. Player and vehicle shots are excluded. The included Windows Toolkit also fades received shot markers.

An owner-driven disposable-save Build 2 session confirmed the corrected candidate behavior and recorded 23 shot packets; the **published versioned VMZ** passed separate offline and Windows installer tests. This does **not** establish broad combat/crash safety or every weapon case. See the [observer design, tests and limitations](docs/ai-shot-observer.md). Do not install the separate source candidate VMZ beside the released radar.

## Toolkit: manage your character offline

The companion `rtv-toolkit` terminal program can inspect and edit character inventory and equipment, validate item placement, and check a save without opening its UI.  It also includes a text based radar display for doing tactical level analysis of the area. **Close Road to Vostok before editing a save.** Before replacing a save, it checks for external changes and keeps a timestamped `.rtvbak.*` backup. Keep your own backup of important saves too.

![Toolkit Character pane showing the character schematic, vitals and selected weapon details; personal save path redacted](assets/screenshots/toolkit-character.png)

*Character view from a real session; the personal save path is redacted.*

The Windows Toolkit executable is included in the installer above and can be opened from **Start → Sobolyatnik-K → Sobolyatnik-K Toolkit**. For a source build instead, [install Rust](https://rustup.rs/) and Git:

```powershell
git clone https://github.com/jtburkh/Sobolyatnik-K.git
cd Sobolyatnik-K
cargo build --release
.\target\release\rtv-toolkit.exe
.\target\release\rtv-toolkit.exe --check
```

By default it looks for `Character.tres`; use `--save <path-to-Character.tres>` to choose another save. The **v0.1.12 executable retains [Build 2's derived inventory catalog](docs/catalog-sync.md)** (263 items, including 15 newly registered ones). The earlier v0.1.9 executable does not; install the new version to use those entries. Unknown item footprints are rejected rather than guessed. In-game slot behavior for every new item still needs a spot-check on a disposable save. Avoid `--spawn-*` commands with the current radar bridge. Run `rtv-toolkit.exe --help` for CLI options.

**Unreleased source preview:** a Backups tab (`5`) lists only `.rtvbak.*` files belonging to the selected character save, with UTC timestamps and sizes. `r` refreshes and `R` requests a restore; it requires typing `RESTORE`, refuses unsaved edits, an externally changed save or backup, and (on Windows) a running game. Before replacing the save it retains the previous version as a new backup. It never prunes or deletes backups. **This tab is not in the published v0.1.12 installer or EXE.**

**Unreleased radar-range source preview:** both displays default to 100 m and offer 50/100/200/400 m detection views. `F9` cycles the in-game HUD and shows the selected range; `+`/`-` adjust the Toolkit radar, whose scope and telemetry lists filter to that distance. The controls are independent (there is no range synchronization over UDP). This changes **display only**, not collected packets or gunshot sound matching. The published v0.1.12 HUD still has a fixed 100 m view, and its Toolkit starts at 200 m. No range-preview VMZ or Toolkit binary is part of that release. For owner testing, the latest [CI run on `main`](https://github.com/jtburkh/Sobolyatnik-K/actions/workflows/ci.yml) provides **UNRELEASED** range-VMZ, Windows-Toolkit, and Linux/WSL-Toolkit artifacts (GitHub sign-in required). They are separate previews, **not** an installer or an update to v0.1.12; never load the preview VMZ alongside the released radar. See [range preview, testing and limitations](docs/radar-range-preview.md).

The terminal also has a live Radar tab (`4`) when telemetry is configured, but it is **not required** for the in-game HUD. The included Toolkit colors live enemy, Nomad, and boss contacts to match the in-game mod. See [telemetry configuration](docs/telemetry-installation.md) and the [protocol](docs/telemetry-protocol.md) for advanced Windows/WSL setups; do not expose the unauthenticated UDP listener outside a trusted machine.

<details>
<summary>See the Equipment, Inventory and live Radar panes</summary>

**Equipment:** inspect slots, weapon stats, attachments, condition and ammo.

![Toolkit Equipment pane showing item slots and weapon details; personal save path redacted](assets/screenshots/toolkit-equipment.png)

**Inventory:** view the grid, item details and stored condition.

![Toolkit Inventory pane showing items and the placement grid; personal save path redacted](assets/screenshots/toolkit-inventory.png)

**Radar:** screenshot from the earlier v0.1.11 build. `LIVE` requires a running game with a matching local UDP address; the new v0.1.12 build additionally shows fading human-AI shot markers when such events are received. The personal save path is redacted in all Toolkit screenshots.

![Toolkit Radar pane showing enemy and Nomad contacts and nearby loot; personal save path redacted](assets/screenshots/toolkit-radar.png)

</details>

## Develop and verify

**Before a source push:** run `python3 tools/check_steam_build.py` (or enable the repository-local `.githooks/pre-push`). The [Steam build check skill](.agents/skills/steam-build-gate/SKILL.md) compares the live public branch, your local Steam appmanifest and the version-pinned installer and **warns without blocking source pushes**. Use `--strict` for release-readiness checks; matching IDs alone never prove mod compatibility. As of the 0.2.0.5 hotfix, Steam public build `25710663` differs from the v0.1.12 installer's tested `25632875`; do not bypass its refusal or install the preview VMZ on the updated game without controlled revalidation.

The [v0.1.12 release](https://github.com/jtburkh/Sobolyatnik-K/releases/tag/v0.1.12-experimental) includes the versioned Shot Alerts VMZ (`6d2d06e55831f174483595d44fde6ba5c3a50f7fec183d2c66a5953ecb98efdd`). Build it with `python3 tools/package_radar_shots_release.py`; for optional Godot 4.6.3 mock tests of the exact package use `python3 tools/test_radar_shots.py --engine /path/to/godot --release-archive`. The older `tools/package_radar_lite.py` produces the unchanged v0.1.8 rollback VMZ (`66fd97a4…`), **not** the current installer asset. Other probe VMZs, including the standalone Shot Alerts candidate, are not additional mods to install alongside the published radar. The [runtime research](docs/runtime-discovery.md) and [third-party notices](THIRD_PARTY_NOTICES.md) provide further context. Neither private saves, dumps, game binaries, nor local builds are in this repository.

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
python3 tools/test_radar_lite.py --engine /path/to/godot  # optional Godot 4.6.3 mock tests
```

Mock tests and limited in-game testing do not establish universal runtime stability. License: [MIT](LICENSE). Road to Vostok and its content remain the property of their respective owners.
