# Runtime telemetry protocol

RtV Toolkit receives best-effort UDP datagrams from the Godot telemetry bridge. The default destination/listener is `127.0.0.1:47777`. JSON is used for version 1 so captures are human-readable; Rust decoding is behind the `PacketDecoder` trait so a later binary codec does not affect receiver, state, or radar code.

## Snapshot

Sent approximately ten times per second while a `Player` group node exists:

```json
{
  "version": 1,
  "type": "snapshot",
  "timestamp_ms": 12345678,
  "map": {
    "id": "res://Scenes/Village.tscn",
    "name": "Village"
  },
  "player": {
    "id": 123,
    "position": [100.5, 3.2, -44.1],
    "heading": 217.3
  },
  "ai": [
    {
      "id": 501,
      "position": [145.2, 3.1, -81.9],
      "alive": true,
      "heading": 217.3,
      "vision_range": 150.0,
      "vision_half_angle": 75.0,
      "player_visible": false,
      "boss": false,
      "faction": "Bandit",
      "friendly": false
    }
  ],
  "loot": [
    {
      "id": 801,
      "position": [110.0, 3.0, -40.0],
      "name": "Military Crate",
      "locked": false,
      "corpse": false
    }
  ]
}
```

`map.id` is the loaded Godot scene's resource path (falling back to its root node name when no path exists); `map.name` is its human-readable filename or node name. It identifies only the currently loaded playable scene. The bridge cannot observe AI or bosses in unloaded maps.

Positions are Godot world coordinates `[x, y, z]`. Heading is degrees clockwise from world `-Z` and normalized to `[0, 360)`. IDs are Godot instance IDs: stable for one object instance, but not persistent across scenes or game sessions.

Build 2 AI heading follows the sensor's +Z view axis; its normal 150-degree view angle gives a 75-degree half-cone. `vision_range` reads `Sensor.viewDistance` directly (normally 150 m; 100 m in fog, 50 m in darkness, up to 200 m during a detection boost). Forced boss sensors have no angular view cone, so `vision_half_angle` is null. `player_visible` is true only if the sensor's current priority is the player **and** `PVisible` is true: AI can instead be targeting other AI. `boss` derives from `AI.variant.faction == Boss`. The additive `faction` is Nomad, Bandit, Guard, Military, Boss, or Unknown. Nomads count as `friendly` while player reputation is at least 50; with lower reputation, they can target the player. Older packets omit both fields and default to unknown/not-friendly.

`loot` contains visible, currently interactable containers with at least one item, limited to 425 m around the player. Empty/looted containers, rejected hidden stashes, and disabled pre-death corpse containers are omitted. Locked and corpse containers remain explicitly classified.

## Gunshot

Legacy bridges sent one event per actual AI shot. **Build 2 bridges 0.8.0–0.8.3 emit no gunshots:** their AI script hook is intentionally removed because Metro Mod Loader 3.2.1's AI rewrite does not compile on the current game build. The receiver still accepts older gunshot packets and retains them for five seconds. Legacy packet shape:

```json
{
  "version": 1,
  "type": "gunshot",
  "timestamp_ms": 12345678,
  "shooter_id": 501,
  "position": [145.2, 3.1, -81.9]
}
```

The Rust state retains a shot for five seconds.

## Compatibility and failure behavior

- Diagnostic bridges 0.8.2–0.8.3 add `diagnostic_profile` at snapshot top level (ignored by older Rust receivers) and defaults to `player_only`: it sends empty `ai` and `loot` arrays by design. The opt-in `overlay_only`, `ai_only`, `loot_only`, and `overlay_ai` modes isolate individual read/draw paths; no mode enables commands or artifact placement. This is not evidence that no AI or loot is present in the game.
- The additive `map` object defaults to unknown when receiving snapshots from a pre-0.5.0 bridge.
- Unknown JSON fields are ignored.
- Unknown versions and message types are rejected without stopping the receiver.
- Malformed datagrams increment the rejected-packet counter.
- UDP loss and reordering are acceptable; snapshots replace observations by entity ID.
- AI and loot absent from snapshots expire after two seconds.
- AI movement trails sample meaningful movement for up to fifteen seconds and reset on teleport/pool reuse.
- The link becomes `STALE` two seconds after the last valid packet.
- The receiver runs on a dedicated standard-library thread and never blocks the Ratatui event loop.

## Narrow runtime command

Historical full bridges accepted two deliberately constrained, file-based commands. **Diagnostic bridges 0.8.2–0.8.3 do not poll or consume commands. Do not send one during its tests; it may remain pending for a later full bridge.** They are not part of the UDP protocol and cannot evaluate arbitrary scripts. RtV Toolkit atomically creates `user://rtv-toolkit-command.cfg`:

```ini
[command]
id="<unique command ID>"
action="spawn_airdrop"
```

The only other accepted action value is `action="spawn_boss"`.

For `spawn_airdrop`, a playable map and the current-build `EventSystem.gd` node must exist; the bridge calls the game's native `EventSystem.Airdrop()` method. For `spawn_boss`, the Build 2 bridge checks the native `B_Pool/AI_Punisher` (the pool also contains Bogeyman), requires current-map AI waypoints, chooses a current-map game-owned `AI_SP` point beyond `spawnDistance`, and calls `AISpawner.SpawnBoss("Punisher", "Attack", spawn_position, native_waypoint)`; this matches the game's `Police.gd` call. It never loads or instantiates a caller-selected scene. Both commands need in-game validation on Build 2 before being considered proven.

The bridge removes each request before execution, providing at-most-once behavior across crashes or restarts. It writes acceptance or rejection to `user://rtv-toolkit-command-result.cfg` and logs the command ID. Unknown or malformed actions are rejected and removed.

After CM-7 recovery, the in-game `F9` modal exposes the same two-action allowlist directly inside Godot. Selection opens a separate confirmation screen before exactly one native call is queued. The modal accepts no paths, positions, counts, scripts, or arbitrary arguments.

## Network override

The Godot bridge optionally reads `user://rtv-telemetry.cfg`:

```ini
[network]
host="127.0.0.1"
port=47777
```

Normal native Windows use should keep the localhost default and requires no file. WSL 2 development may point `host` at the current WSL virtual-machine address and start the toolkit with `--telemetry-bind 0.0.0.0:47777`.
