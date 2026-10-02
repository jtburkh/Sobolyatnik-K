# Telemetry bridge installation and lifecycle

## Packaging decision

Sobolyatnik-K's Rust executable does **not** silently modify the Road to Vostok directory. The separately packaged, playable 0.1.7 radar is `dist/probes/RtVRadarLoot.vmz` (see the [README](../README.md)); `RtVTelemetryProof.vmz` is a quarantined 0.8.3 diagnostic, **not** a release asset. Public releases are paused while Build 2 stability work continues.

Automatic embedded installation was evaluated and intentionally rejected:

- the game directory may require administrator permission;
- Metro Mod Loader is a separate third-party project with its own release lifecycle;
- silently creating `override.cfg` changes game startup behavior;
- Steam updates may replace game or loader files;
- native Windows, Proton, and WSL expose different installation paths;
- an explicit copy is easy to inspect, reverse, and troubleshoot.

The radar VMZ is installed manually; it is not required for offline save editing. Never install the diagnostic bridge alongside the playable radar.

## Build 2 crash investigation (0.8.3 tracing, not a release)

Build 2 (Steam build 25632875, Godot 4.6.3) broke Metro Mod Loader 3.2.1's
old `AI.gd` rewrite. Version 0.8.0 removed the AI `PlayFire` hook and restored
AI animation, but **three repeatable native RTV.exe null-dereference crashes**
occurred in combat while 0.8.0 was enabled. Several comparable enemy/Nomad
firefights without mods did not crash. No precise fault source is proven.
The 0.8.1 color-only VMZ has never been installed and is **not a crash fix**.

Version 0.8.2 was installed briefly in `player_only`; inventory mouse
controls malfunctioned and recovered after its removal and a restart. It is
quarantined. The **uninstalled 0.8.3 trace build** keeps player-only telemetry
as its default (no AI/loot reads, CanvasLayer, CM-7, commands, or `AI.gd`
hook), and adds a flushed, capped, per-session user-data trace; it is **not a
crash fix**. Two smaller manifest/autoload probes isolate the loader before
any further bridge testing. Do not install any probe without permission and
following the [controlled crash-isolation plan](telemetry-diagnostics.md).

## Diagnostic setup outline (requires explicit consent; not for normal play)

1. Exit Road to Vostok and verify no `RTV.exe` process remains.
2. Install Metro Mod Loader 3.2.1 or newer from <https://modworkshop.net/mod/55623> according to its documentation.
3. Copy `RtVTelemetryProof.vmz` into the game's `mods` directory without extracting it.
4. Start the game and confirm the loader lists `RtV Telemetry Trace Diagnostics`, version `0.8.3`, as enabled.
5. Start the terminal application. Select Radar with `4`.

The expected native Windows layout is:

```text
Road to Vostok/
├── RTV.exe
├── RTV.pck
├── override.cfg
├── modloader.gd
└── mods/
    └── RtVTelemetryProof.vmz
```

## Integrity and version verification

The archive builder fixes ZIP entry timestamps, ordering, permissions, and compression level, so identical source creates identical VMZ bytes:

```bash
python3 tools/package_telemetry_mod.py
sha256sum dist/RtVTelemetryProof.vmz
```

After copying, compare the source and destination hashes. On PowerShell:

```powershell
Get-FileHash .\RtVTelemetryProof.vmz -Algorithm SHA256
Get-FileHash "C:\Program Files (x86)\Steam\steamapps\common\Road to Vostok\mods\RtVTelemetryProof.vmz" -Algorithm SHA256
```

The optional `user://rtv-telemetry.cfg` overlay settings are:

```ini
[overlay]
enabled=true
range=200
size=230
toggle_key="F8"
show_ai=true
show_bosses=true
show_trails=true
show_loot=true
show_gunshots=true
```

Range is clamped to 50–400 metres and size to 180–320 pixels. The overlay is
mouse-pass-through and appears only while player telemetry is available.

If movement becomes jerky with telemetry installed, compare the same map with
and without the VMZ. F8 hides drawing but does not stop snapshot collection;
versions 0.7.1+ reduce collection overhead by caching property availability and
loot collision-shape references, and disables per-shot/per-second debug logs by
default. To restore verbose logs for troubleshooting, add:

```ini
[debug]
verbose_logging=true
```

Do not replace or install another VMZ while investigating the combat crashes.
Any approved diagnostic test must happen with the game closed, preserve the
baseline, and stop immediately if another crash occurs. F8 does not disable
sampling. See `telemetry-diagnostics.md`.

The loader validates the root manifest during startup. The game log at `%APPDATA%\Road to Vostok\logs\godot.log` should contain:

```text
[RTV_TELEMETRY_PROOF] loaded; UDP v1 -> 127.0.0.1:47777
```

Before promoting any new full-feature candidate to a supported release, follow `telemetry-diagnostics.md`, then verify on Steam build 25632875:

1. With the bridge disabled, load a populated map and confirm AI walks, animates, fights, and does not show `AI/100` debug labels. Check `godot.log` for parse errors.
2. Exit the game. Only after step 1, back up the installed VMZ and explicitly opt into testing the new archive; do not overwrite it during play. Keep a way to disable/reset the loader.
3. Load a map and check `godot.log` for `loaded; UDP v1`, no `AI.gd` parse errors, and **no** `Hook declared: res://Scripts/AI.gd`. Confirm animation remains normal.
4. Check Radar `LINK LIVE`, active enemy and Nomad contacts, correct Boss tags, friendly Nomads at reputation >=50, updated loot after AI looting, and vision/LOS during fights. Gunshots must remain absent.
5. Confirm native summon commands and menus on a disposable game session, then compare frame times/stutter with and without the VMZ. If any step fails, disable the candidate and keep the issue open.

**The following CM-7 and summon behavior is historical full-bridge design, disabled in the 0.8.3 diagnostic build.** Before CM-7 recovery, a compact upper-right signal finder should appear when a playable map loads. Recovering the world artifact permanently unlocks the Radar; `F7` controls layers, `F8` toggles visibility, and `F9` opens the confirmed summon menu. The Ratatui panel provides the independent receiving-side check: `LINK LIVE`, an increasing packet counter, player age near zero, and zero rejected packets under normal operation.

## Removal and recovery

1. Exit Road to Vostok.
2. Delete `mods/RtVTelemetryProof.vmz` to remove only the telemetry bridge.
3. Use Metro Mod Loader's reset/disable procedure if the loader itself should also be removed.
4. Delete `user://rtv-telemetry.cfg` only if a WSL/network or overlay override was created.
5. Delete `user://rtv-toolkit-progression.cfg` to reset CM-7 recovery and lock the in-game control surface again.
6. Delete `user://rtv-toolkit-command.cfg` and `user://rtv-toolkit-command-result.cfg` if a pending/result runtime request should also be cleared.

Removing the bridge has no effect on character saves. If a game update changes `AI.gd`, disable the bridge until current-build discovery and mod-loader compatibility are revalidated. The Rust application will remain usable and show Radar as `WAITING` or `STALE`.

## Known limitations

- Godot instance IDs are session-scoped, not persistent identities.
- UDP is best-effort and unauthenticated; use localhost or a trusted local interface only.
- Scene transitions temporarily interrupt snapshots.
- No gunshot events are emitted by the Build 2 bridge while the AI script hook is disabled. Fifteen-second AI trails exist only in memory and are not recorded.
- Vision cones show the game's angular/range sensor envelope, not wall-clipped polygons. A red cone means the game's sensor prioritizes the player and currently has line of sight. Living Nomads use the standard AI diamond marker in light blue regardless of reputation; hostile Nomads can still show a red player-visibility cone.
- Loot discovery includes visible, interactable, non-empty containers within 425 m. It intentionally omits empty containers and disabled pre-death corpse inventories.
- Boss styling depends on `AI.variant.faction == Boss`, not the removed `AI.boss` field.
- Build 2 creates both a Punisher and Bogeyman in the native boss pool. The narrow `spawn_boss` action selects only the Punisher and is rejected once that instance has been consumed; the bridge does not manufacture replacements.
- In-game movement trails are sampled positions, not inferred or hooked footstep sounds. The Ratatui Radar remains the detailed view for vision cones and elevation diagnostics.
- The CM-7 is a consumed mod artifact, not a persisted `ItemData` inventory resource. This prevents the character save from depending on an installed mod after removal.
- Runtime telemetry is read-only, but save writes must still be performed with the game closed.
