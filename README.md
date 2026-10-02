# Sobolyatnik-K — Sable Hunter 1L108K

An experimental in-game radar for **Road to Vostok**, with a separate terminal Toolkit for managing your character while the game is closed. Unofficial; not affiliated with the game developer.

## Install the radar (Windows / Steam)

Install Road to Vostok **Steam build 25632875**, close the game, then paste this **one line into PowerShell**:

```powershell
$ErrorActionPreference='Stop'; $p=Join-Path $env:TEMP 'sobolyatnik-k-v0.1.8.ps1'; Invoke-WebRequest -UseBasicParsing -Uri 'https://github.com/jtburkh/Sobolyatnik-K/releases/download/v0.1.8-experimental/install-sobolyatnik.ps1' -OutFile $p; if ((Get-FileHash -LiteralPath $p -Algorithm SHA256).Hash -ne 'FC8B4D38F2373B750BD9457BD3D779A5CE613E541F6643AEFFD99634B3F0DF72') { throw 'Installer checksum mismatch; nothing was executed' }; & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $p
```

This command checks the installer hash **before running it**. The installer verifies its downloads, installs [official Metro Mod Loader 3.2.1](https://github.com/ametrocavich/vostok-mod-loader/releases/tag/v3.2.1) if neither loader file exists, and installs the [v0.1.8-experimental radar VMZ](https://github.com/jtburkh/Sobolyatnik-K/releases/tag/v0.1.8-experimental). It refuses unsupported builds, conflicting VMZs, and partial or incompatible Metro installations. An older radar VMZ is backed up before replacement. See [installer details and safeguards](docs/windows-installer.md).

**Current installer scope:** It installs the **in-game radar and Metro only**. It does **not** install the separate `rtv-toolkit.exe` terminal application or provide an uninstall command yet. The radar works without the terminal. To remove the radar now, **close the game**, then move `RtVRadarLoot.vmz` out of your Steam game's `mods` folder; leave Metro in place if other mods might use it. The installer does not edit saves or game binaries.

![Concept illustration of the Sobolyatnik-K radar unit](assets/SOBOLYATNIK-K.png)

*AI-generated concept illustration of the fictional unit—not an in-game screenshot or an equippable item.*

## What Sobolyatnik-K does in game

The compact, dark tactical HUD shows **red enemies**, **blue Nomads**, short movement trails, and small **hollow loot markers**. `F7` cycles radar layers; `F8` hides or shows the drawing without stopping telemetry. The radar is independent of equipped inventory items and does not modify your character save.

This is an **experimental Build 2** mod, not a promise of crash-free play. Gunshot telemetry, AI sensor/vision reads, the old artifact, and summon commands are **not** part of this playable radar. Do not install the diagnostic bridge alongside it. See [stability and feature limitations](docs/telemetry-diagnostics.md).

## Toolkit: manage your character offline

The companion `rtv-toolkit` terminal program can inspect and edit character inventory and equipment, validate item placement, and check a save without opening its UI. **Close Road to Vostok before editing a save.** Before replacing a save, it checks for external changes and keeps a timestamped `.rtvbak.*` backup. Keep your own backup of important saves too.

The Toolkit is **not bundled in the current experimental installer**. To use it now, [install Rust](https://rustup.rs/) and Git, then build it from source:

```powershell
git clone https://github.com/jtburkh/Sobolyatnik-K.git
cd Sobolyatnik-K
cargo build --release
.\target\release\rtv-toolkit.exe
.\target\release\rtv-toolkit.exe --check
```

By default it looks for `Character.tres`; use `--save <path-to-Character.tres>` to choose another save. The item catalog was derived from an **earlier game build** and still needs Build 2 verification; unknown item footprints are rejected rather than guessed. Avoid `--spawn-*` commands with the current radar bridge. Run `rtv-toolkit.exe --help` for CLI options.

The terminal also has a live Radar tab (`4`) when telemetry is configured, but it is **not required** for the in-game HUD. See [telemetry configuration](docs/telemetry-installation.md) and the [protocol](docs/telemetry-protocol.md) for advanced Windows/WSL setups; do not expose the unauthenticated UDP listener outside a trusted machine.

## Develop and verify

To package the five isolated radar variants from source, install Python 3 and run `python3 tools/package_radar_lite.py`. Only `dist/probes/RtVRadarLoot.vmz` is the playable radar; the other VMZs are diagnostic variants, not additional mods to install together. The [runtime research](docs/runtime-discovery.md) and [third-party notices](THIRD_PARTY_NOTICES.md) provide further context. Neither private saves, dumps, game binaries, nor local builds are in this repository.

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
python3 tools/test_radar_lite.py --engine /path/to/godot  # optional Godot 4.6.3 mock tests
```

Mock tests and limited in-game testing do not establish universal runtime stability. License: [MIT](LICENSE). Road to Vostok and its content remain the property of their respective owners.
