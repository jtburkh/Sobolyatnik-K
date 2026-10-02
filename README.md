# Sobolyatnik-K

**Sobolyatnik-K (Sable Hunter) Experimental — 1L108K** is an unofficial radar mod and companion terminal application for [Road to Vostok](https://roadtovostok.com/). The terminal also includes an offline inventory/save editor. This project is not affiliated with the game developer.

> **Build 2 status:** The small in-game radar has passed staged inventory, loot, AI, and combat sessions on Steam build 25632875 (Godot 4.6.3). The naming-only 0.1.7 archive was not checked in-game. The currently installed 0.1.8 archive adds scene-transition loot-cache guards; it passed an offline mock under the same Godot 4.6.3 engine version and one positive user gameplay session across Village↔Attic with no new dump or incomplete loot trace phase. This is **not** universal crash clearance. No version is guaranteed crash-free. Keep a known-good rollback archive. Do not use the older 0.8.0–0.8.2 telemetry bridge; it coincided with combat crashes or inventory regression.

## What works today

- A compact, mouse-pass-through in-game radar: red enemies, blue Nomads, 15-second movement trails, and small hollow loot markers. `F7` cycles layers; `F8` hides/shows drawing **without disabling telemetry**.
- Live UDP snapshots of player position, active AI positions/factions, and available loot for the Rust terminal's Radar view. The terminal can show map, contact distances, tracks, and loot; features requiring additional sensor data may be unavailable with the minimal bridge.
- Offline inventory and equipment viewing/editing, collision-checked item placement, save validation, an external-change guard, and timestamped backups before a save is replaced. **Close the game before editing saves.**

**Not restored on Build 2:** gunshot telemetry (the Metro loader's AI hook rewrite fails to compile), AI sensor/vision reads, the historical CM-7 artifact, and summon commands. Do not queue summons while using the current radar bridge. The old full bridge and diagnostic builds are **not** playable substitutes for this radar. Each remaining feature requires a separate stability test; see [diagnostics](docs/telemetry-diagnostics.md).

## Build

Requires a current Rust toolchain and Python 3:

```bash
cargo build --release
python3 tools/package_radar_lite.py
```

The executable remains `target/release/rtv-toolkit` (`.exe` on Windows) for command-line compatibility. The radar archive is `dist/probes/RtVRadarLoot.vmz`; version 0.1.8 has passed an initial in-game session but remains under stability testing. The other archives from that packaging script are isolated diagnostic variants, **not additional mods to install together**. Generated archives, local builds, backups, game files, crash dumps, and saves are not tracked in this repository.

## In-game radar

A [hash-verified one-line Windows installer](docs/windows-installer.md) is
available **only after** the explicitly tagged `v0.1.8-experimental` prerelease
assets appear on GitHub. Until then, use the manual source build below. The
installer downloads pinned official Metro Mod Loader 3.2.1 files for a clean
game, but never replaces an existing loader or bypasses Build 2 checks.

Install [Metro Mod Loader](https://modworkshop.net/mod/55623) following its own instructions. With **Road to Vostok closed**, put only `RtVRadarLoot.vmz` in the game's `mods` directory (remove conflicting older radar VMZs first). Do not extract the archive. The loader should list **Sobolyatnik-K (Sable Hunter) Experimental — 1L108K**. To revert, close the game before restoring a verified earlier VMZ. The radar VMZ itself does not edit game binaries, loader files, or saves; a first-time Metro installation does add the loader's two startup files.

The bridge sends UDP to `127.0.0.1:47777` by default. For a Toolkit running under WSL 2, set the game's `%APPDATA%/Road to Vostok/rtv-telemetry.cfg` to the **current** WSL IP (which may change on restart):

```ini
[network]
host="<WSL IPv4 address>"
port=47777
```

Launch the terminal bound to that same address:

```bash
rtv-toolkit --telemetry-bind <WSL IPv4 address>:47777
```

Do not expose the unauthenticated UDP listener beyond a trusted local machine. The terminal's `4` tab shows the radar; `F7`/`F8` control the *in-game* overlay. The radar currently reads game state but does not modify saves or spawn entities.

## Offline save editor

Run `rtv-toolkit` to locate a character save automatically, or specify one explicitly:

```bash
rtv-toolkit --save "/path/to/Character.tres"
rtv-toolkit --check --save "/path/to/Character.tres"
```

The catalog in `data/` is generated from **an earlier game build (22914619)** and still needs verification against Build 2. Unknown item footprints fail validation rather than being guessed. Before writing a save, the editor checks placement, refuses to overwrite a save changed externally since loading, and retains a timestamped `.rtvbak.*` copy. Keep separate backups of important saves.

The catalog can be regenerated from locally recovered resources with `tools/sync_game_resources.py`; **do not add game binaries or extracted proprietary assets to this repository**. See [third-party notices](THIRD_PARTY_NOTICES.md).

## Development and limitations

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
python3 tools/package_radar_lite.py
```

Optional Godot mock tests require a local Godot executable:

```bash
python3 tools/test_radar_lite.py --engine /path/to/godot
```

Mock tests do not prove Road to Vostok's Godot 4.6.3 runtime stability. Details on the telemetry protocol, Build 2 reverse-engineering, and the staged crash investigation are in [`docs/`](docs/). The alternative `godot-mod/rtv-telemetry` bridge is **diagnostic source only** and is not a supported release asset. Public releases are on hold pending further stability work.

License: [MIT](LICENSE). Road to Vostok, its content and names remain the property of their respective owners.
