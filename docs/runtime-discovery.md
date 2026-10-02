# Road to Vostok runtime discovery

This document preserves historical evidence from Steam build **22914619** (0.1.1.3). Its `AI.boss`, `/AI/Agents`, and `PlayFire` hook findings **do not apply** to Build 2 (25632875). See the Build 2 addendum below and `docs/telemetry-protocol.md` for the current candidate contract.

## Build examined

- Steam app: `1963610`
- Steam build ID: `22914619`
- Recovered `project.godot` version: `0.1.1.3`
- Engine feature tag: Godot `4.6`
- Installed `RTV.exe` SHA-256: `2f115ad73bd58fc73cdd96b4f4e1681eb8c7f876ca74e7766158fb2644dd555d`
- Recovered source used for evidence: `/tmp/rtv-current-data`

“Source verified” below means verified in recovered current-build scripts or project settings. “Runtime pending” means that the probe still needs to demonstrate the conclusion in a running game.

## Player

| Finding | Status | Evidence |
|---|---|---|
| `Player` is the registered group name and capitalization. | Runtime verified | `project.godot:57-77` declares the global groups, including `Player`. AI line-of-sight checks also use `is_in_group("Player")` in `Scripts/AI.gd:1384-1391`. |
| Authoritative world position is `GameData.playerPosition: Vector3`. | Source verified | Declared in `Scripts/GameData.gd:42`; written from the controller transform in `Scripts/Controller.gd:252` and `:465`. |
| Authoritative facing vector is `GameData.playerVector: Vector3`. | Source verified | Declared in `Scripts/GameData.gd:43`; written from the camera basis in `Scripts/Controller.gd:253` and `:466`. |
| Heading can be derived without finding the camera node. | Source verified; convention runtime pending | For normal play, controller line 253 stores `-camera.global_basis.z`. The probe converts it with `atan2(forward.x, -forward.z)`, making Godot forward `-Z` equal to 0 degrees. The alternate controller path at line 466 stores the opposite basis and must be observed to determine when it applies. |
| A stable per-instance player ID can use the `Player` group node’s `get_instance_id()`. | Runtime verified | The proof returned stable ID `2124951902500` after entering the playable scene. A preceding scene used a different node ID, confirming this is intentionally instance/session scoped rather than persistent identity. |
| Direct camera access is not required for the initial snapshot. | Source verified | Position and forward direction are already copied into the shared `res://Resources/GameData.tres` resource. |

The shared runtime resource is loaded by game scripts with `preload("res://Resources/GameData.tres")`; see `Scripts/AI.gd:9` and `Scripts/Camera.gd:5`.

## AI discovery

| Finding | Status | Evidence |
|---|---|---|
| `AI` is the registered group name and capitalization. | Source and runtime fallback verified | `project.godot:57-77` declares `AI`. `Scripts/Explosion.gd:86` and `:101` identify targets through `is_in_group("AI")`. |
| The general AI implementation is `res://Scripts/AI.gd`. | Source verified | `Scripts/AI.gd:1` extends `CharacterBody3D` and contains movement, navigation, combat, damage, and death logic. |
| Active general AI are maintained under `/root/Map/AI/Agents`. | Runtime verified | `Scripts/AISpawner.gd:48-50` binds `$Agents`; spawned AI are reparented to it at lines 170, 195, 219, 239, and 257. `Scripts/AI.gd:201-204` resolves its spawner at `/root/Map/AI`. |
| The spawner does not expose an agent array. | Source verified | It exposes `activeAgents` as a count (`Scripts/AISpawner.gd:33`) and the `$Agents` node as the collection (`:50`). Inactive preallocated AI live in `$A_Pool`/`$B_Pool`, so scanning only the `AI` group may include pool members. |
| AI world position is the `CharacterBody3D.global_position`. | Source verified | Spawn code assigns `newAgent.global_position`/`global_transform`; AI navigation and distance logic repeatedly use `global_position`, including `Scripts/AI.gd:531-534`. |
| Stable per-session AI IDs can use `get_instance_id()`. | Runtime verified | IDs `2172263652467` and `2179544926583` remained stable across repeated samples. Pool objects are reused, so this is not a persistent cross-session identity. |
| Alive/dead is exposed as `dead`; health is exposed as `health`. | Source verified | Declared in `Scripts/AI.gd:60-62`; `Death()` sets `dead = true` at `:1547-1549`; damage subtracts from `health` at `:1491-1496`. |

The first probe prefers `/root/Map/AI/Agents` and only falls back to the `AI` group while filtering nodes whose `pause` property is true. A map with no manager or no active AI is valid and logs `AI: none active`.

## AI firearm event

| Finding | Status | Evidence |
|---|---|---|
| Weapon possession alone is not a firing event. | Source verified | `SelectWeapon()` at `Scripts/AI.gd:382-446` selects/configures equipment independently of combat. |
| An actual shot occurs in `AI.Fire(delta)` only when `fireTime <= 0`. | Source verified | `Scripts/AI.gd:1308-1353` calls `Raycast()`, `PlayFire()`, `PlayTail()`, and `MuzzleVFX()` in that branch, then resets timing through `FireFrequency()`. |
| The narrowest currently identified hook is `AI.PlayFire()`. | Source and runtime verified | `PlayFire()` is defined at `Scripts/AI.gd:1997-2008`; the only recovered call is the actual-shot branch at line 1324. The loader's `ai-playfire-pre` hook fired once per observed shot and exposed the firing AI through `RTVModLib._caller`. |
| Shot location must be captured at event time. | Runtime verified | The callback copies the caller’s `global_position` directly into each UDP event. Repeated live events retained their own positions while the firing AI moved. |

The installed bridge declares `res://Scripts/AI.gd="PlayFire"` in its hook manifest and registers `ai-playfire-pre` after the loader emits `frameworks_ready`. It does not infer shots from AI weapon possession or combat state.

## Additional useful AI telemetry

These fields exist directly on `AI.gd` and can be considered after the position path works:

- `health`, `dead`, `boss`
- `currentState`, using the `State` enum declared at lines 55-56
- `weapon`, `weaponData`, `muzzle` at lines 79-82
- `playerVisible`, `lastKnownLocation`, and distance readings at lines 87-93
- `agent: NavigationAgent3D` at line 41; destination is `agent.target_position`
- `currentPoint`, `patrolArea`, `pathTarget`, and `returnPosition`
- selected `backpack`, `secondary`, and generated loot `container`

The AI does **not** expose a conventional inventory collection. `SelectWeapon()`, `SelectBackpack()`, and `SelectClothing()` choose from scene children, while `ActivateContainer()` enables generated drop/container content on death. Telemetry should preserve that distinction rather than inventing an AI inventory model.

Faction/type is not a single field on the base AI. `AISpawner.gd:29-32` chooses separate Bandit, Guard, Military, or Punisher scenes based on zone/boss status. A useful type may be derived from the instantiated scene/script/name only after observing actual runtime nodes.

## Mod loading

Road to Vostok does not expose a native mod directory in the recovered game code. The current community loader uses normal Godot mechanisms:

1. `override.cfg` adds `res://modloader.gd` as an early autoload.
2. The loader mounts `.vmz`/ZIP resources with `ProjectSettings.load_resource_pack()`.
3. A root `mod.txt` declares mod metadata and autoload scripts.

At discovery time the installed game directory contained only `RTV.exe` and `RTV.pck`; no loader was installed. The proof mod is intentionally a plain persistent autoload and does not replace game scripts.

## Runtime proof result

Milestone A passed on build `22914619` on 2026-09-04. The installed proof autoload produced live entries including:

```text
[RTV_TELEMETRY_PROOF] Player: id=2124951902500 x=43.500 y=5.262 z=55.000 heading=270.00
[RTV_TELEMETRY_PROOF] AI #1: id=2172263652467 x=-26.100 y=5.200 z=-28.000 alive=true
[RTV_TELEMETRY_PROOF] AI #2: id=2179544926583 x=147.210 y=5.000 z=-144.002 alive=true
```

Earlier samples captured player positions `(0, 0, -5.5)` and headings `180.00`, `93.80`, and `17.85`, proving that movement/turning updates are live rather than static save data. The same session safely emitted `AI: none active` before enemies spawned, then discovered one and subsequently two active AI without a scene-tree-wide per-frame scan.

A later combat session confirmed that dead AI remain under `Agents` briefly with `alive=false`; the radar marks these contacts with `×` until the game despawns them or they stop being reported.

Proof log prefix: `[RTV_TELEMETRY_PROOF]`.

## UDP proof result

Milestone B passed on 2026-09-04. Version 1 JSON snapshots traveled at approximately 10 Hz from the Windows Godot process to the Rust `TelemetryReceiver`. A representative decoded packet was:

```text
snapshot #335: player=2118157129958 pos=[42.41, 4.65, 55.10] heading=325.55 ai=3
```

Subsequent packets captured continuously changing player position and heading while preserving three AI contacts. The receiver also passed loopback tests proving malformed datagrams are reported without terminating the worker.

The production default remains `127.0.0.1:47777`. For this WSL development session only, `user://rtv-telemetry.cfg` points Godot to the current WSL virtual-network address and the Rust probe binds `0.0.0.0:47777`; a native Windows build needs neither override.

## Radar and gunshot proof result

Milestones C–E passed in a live combat session on build `22914619`. The loader reported both the declared current-build hook and successful callback registration:

```text
[ModLoader][Info] Hook declared: res://Scripts/AI.gd :: PlayFire [RtV Telemetry Proof]
[RTV_TELEMETRY_PROOF] AI.PlayFire gunshot hook active
```

Twenty-two real AI firing callbacks were observed. Representative event-time positions from one moving shooter were:

```text
[RTV_TELEMETRY_PROOF] gunshot shooter=2273967098442 position=[32.018928527832,0.00999995693564415,-95.7612762451172]
[RTV_TELEMETRY_PROOF] gunshot shooter=2273967098442 position=[34.6824531555176,0.00999995693564415,-94.4196166992188]
```

The independently running Rust application received the UDP stream with zero rejected packets and captured the matching event on the player-centered HYBRID radar:

```text
PACKETS 8628  //  REJECTED 0
PLAYER  X +45.2  Y +0.3  Z -87.4  HEADING 360.00°
AI CONTACTS  4
RECENT SHOTS  1
✦ #1710   0.1s
```

The marker appeared at the event position while the ordinary AI contact continued to move, then disappeared after the five-second state-retention window. The same session confirmed changing player heading rotates the contact picture while the player marker remains centered, dead AI are distinguishable from active AI, and live editing remains responsive while telemetry is flowing.

## Advanced overlay discovery

Current-build source verification established the following additive snapshot semantics for bridge 0.4.0:

- `AI.gd:15` exposes the authoritative `boss` flag; boss status is not inferred from health.
- `AI.gd:17` exposes the `eyes` node. Although `Sensor()` names `-eyes.basis.z` as its view direction, its player direction is reversed (`AI.gd:557-558`), making the effective detection cone point along world `+eyes.basis.z`.
- The `viewRadius > 0.5` test at `AI.gd:560` gives a 60-degree half-angle (120-degree aperture).
- `LOSCheck()` at `AI.gd:581-590` sets 25 m darkness, 100 m fog, or 200 m normal range. `extraVisibility` adds 50 m for five seconds after qualifying firearm detection, while boss AI bypass darkness/fog reductions.
- `playerVisible` at `AI.gd:89` is set only when the LOS ray collides with the `Player` group. It is used to color a cone as actively sighted, not to infer whether the player can see the AI.
- Elevation is the existing world-Y difference between AI/container and player roots; horizontal radar projection remains X/Z.
- `LootContainer.gd` is the authoritative container type. Runtime discovery starts from the existing `Interactable` proxy group, walks to the `LootContainer` ancestor, and deduplicates by instance ID.
- `storaged` selects `storage` after access and `loot` before access. Empty containers are omitted. Hidden/process-disabled stashes and collision-disabled pre-death corpse containers are excluded; locked and active corpse containers retain distinct classifications.

These are additive version-1 JSON fields. New receivers accept old bridge snapshots through defaults, and old receivers ignore the added fields.

Map identity uses the normal Godot scene tree rather than inferred coordinates: `SceneTree.current_scene.scene_file_path` is the stable runtime identifier, with the current scene root name as a fallback. The bridge derives the display label from the scene filename (for example, `res://Scenes/Village.tscn` becomes `Village`). Every AI snapshot is observed inside that same loaded scene, so Rust records the map identity alongside each tracked AI. This can identify the current map and the map of a visible boss, but cannot discover bosses in scenes that Godot has not loaded.

Bridge 0.4.0 then passed a live map test on build `22914619`. The Rust receiver held `LINK LIVE` with zero rejected packets while the advanced overlay discovered 55 non-empty interactable containers and two AI. Representative readings included `Boxes` at 9.1 m, `Garbage` at 16.2 m, `Wooden Crate` at 32.5 m, and `Crate Special` at 60.1 m. One AI rendered 3 m below the player and another 1 m below; movement samples and effective cone points stayed heading-relative while the player remained centered. No Godot parse/runtime error was emitted. Boss appearance was verified against synthetic protocol data and renderer tests because the live scene contained no boss; the source of truth remains the directly exported `AI.boss` field.

## Native airdrop command proof

Bridge 0.4.1 passed a live, single-command proof using the game-owned `EventSystem.Airdrop()` path. RtV Toolkit atomically queued command `1047085-1788621195032006477`; the bridge consumed it once and returned `status="accepted"` with `native EventSystem.Airdrop queued`. The current-build logs then confirmed the complete native event lifecycle:

```text
[RTV_TELEMETRY_PROOF] command 1047085-1788621195032006477: native EventSystem.Airdrop queued
Airdrop Collided: Obstruction
CASA: Distance cleared
```

This preserved the official CASA flight, release/parachute physics, collision, AI-hotspot creation, and generated airdrop contents. The control surface recognizes only the documented `spawn_airdrop` and `spawn_boss` actions; malformed or unknown actions are removed and rejected, and request removal occurs before execution for at-most-once behavior.

## Native boss spawn discovery

Build `22914619` provides a narrow game-owned path in `AISpawner.gd`. `CreatePools()` instantiates exactly one `res://AI/Punisher/AI_Punisher.tscn` into `$B_Pool`, sets its authoritative `boss` flag, assigns the owning spawner, moves it off-map, and pauses it. `SpawnBoss(spawnPosition)` refuses an empty pool, reparents that native instance into `$Agents`, assigns the supplied position plus a game-owned waypoint and player last-known location, then invokes `AI.ActivateBoss()` and increments `activeAgents`.

Bridge 0.5.0 exposes that existing method without accepting a path, scene, position, count, or script from the command file. Before consuming `spawn_boss`, it verifies the current `AISpawner.gd` node and native `$B_Pool`, requires the map's `AI_WP` group, and chooses only from current-map `AI_SP` nodes farther away than the spawner's configured `spawnDistance`. The one pool member naturally prevents duplicate boss creation. The request is removed before the deferred native call and is therefore at most once. Static source and packaged-bridge validation are complete; unlike the airdrop section above, no live boss was spawned during implementation validation.

## In-game radar overlay

Bridge 0.6.0 adds a separately drawn Godot `CanvasLayer` and mouse-pass-through `Control` in the upper-right viewport corner. It consumes the same in-process snapshot dictionaries sent over UDP, so map identity, AI IDs, positions, and authoritative boss flags cannot diverge between the in-game and Ratatui displays. The `AI.PlayFire()` hook also copies each real firing position into a five-second local overlay history. Projection uses the same player-centered, heading-relative X/Z transform as the Rust core; the player remains fixed at centre and current heading remains up.

Bridge 0.6.0 initially proved the compact `CanvasLayer` approach. Bridge 0.7.0 extends it with an `F7` layer modal for normal AI, bosses, movement trails, loot, and gunshots, plus an `F9` selection-and-confirmation modal restricted to the already verified `EventSystem.Airdrop()` and `AISpawner.SpawnBoss()` paths. The movement layer samples meaningful AI position changes every 400 ms, keeps at most 15 seconds, and resets after a 30 m teleport/pool-reuse jump; it does not claim to detect acoustic footsteps.

The CM-7 Cerebral Multiplier is implemented as a custom procedural Godot world artifact: a heavy olive steel/Bakelite case, amber emissive display, oversized controls and ports, cyan status lamp, Cyrillic marking, side handles, collision body, and normal `Item` interaction semantics. While locked, the display exposes only the artifact's bearing and distance. The bridge places one instance at the nearest current-scene `AI_SP` (falling back to `AI_WP`) at least 25 m away when possible. Interaction stores only `cerebral_multiplier=true` in `user://rtv-toolkit-progression.cfg`, consumes the artifact, and unlocks the in-game interface permanently.

The artifact deliberately does not enter `Character.tres`. A conventional custom `ItemData` would leave the save referencing a mod resource and could make that save unloadable after uninstalling the VMZ. Treating CM-7 as a consumed progression artifact preserves the requested find-and-recover experience without creating that dependency. The external Rust UI remains ungated.

Godot 4.7 validation includes full script/resource parsing, synthetic 230×230 drawing with normal AI, bosses, loot, trails and gunshots, modal navigation/confirmation, CM-7 scene construction and interaction, complete bridge startup plus safe waypoint placement, and native summon dispatch against then-current-build-compatible test doubles. Live gameplay visual and recovery validation remains pending; no production save or progression file was modified during implementation.

## Build 2 addendum — Steam build 25632875 (source-verified only)

Read-only recovery of 183 scripts from the Build 2 executable into `/tmp/rtv-build2-scripts` found `AI.gd` now extends `Node3D`; the `boss` and `pause` properties and `/root/Map/AI/Agents` hierarchy from the historical tables are gone. The spawner instead maintains active `$Enemies` and `$Nomads` nodes and E/N/B pools. `AI.variant: AIData` holds faction `Nomad`, `Bandit`, `Guard`, `Military`, or `Boss`; AI's `active`, `dead`, and `health` are still present. `Sensor.gd` sets `viewAngle=150` and normal `viewDistance=150`, adjusts range for fog/darkness/boost, and measures `PVisible` against its *priority* (which may be another AI). Friendly Nomads at reputation >=50 do not target the player. `SpawnBoss(boss, state, spawnPosition, currentPoint)` now needs four arguments; `Police.gd` supplies `("Punisher", "Attack", position, waypoint)`.

Metro Mod Loader 3.2.1's existing opt-in `AI.PlayFire` wrapper injects a loadout reference to the removed `boss` variable, producing `Identifier "boss" not declared` and broken AI. The 0.8.0 candidate drops the `[hooks]` entry and all `.hook()` calls; the loader's opt-in gate should therefore avoid wrapping `AI.gd` if no other enabled mod requests it. Shot events cannot be emitted by this candidate. Initial Godot 4.8.dev syntax checks, Rust tests, deterministic packaging, and source analysis passed, but did not establish runtime stability. The 0.8.0 no-hook bridge initially restored AI movement and radar contacts, but three enabled-mod firefights ended in the same native `RTV.exe` access violation (read at null+`0x34`, RVA `0x2b39803`); a subsequent no-mod session contained several enemy/Nomad firefights without crashing. The game `mods` directory is now empty, with the 0.8.0 VMZ preserved outside the game. The 0.8.2 player-only diagnostic was later installed briefly but coincided with an inventory mouse regression that resolved after its removal/restart. The game is mod-free again. Uninstalled manifest-only/inert-autoload probes and a 0.8.3 flushed-trace diagnostic now provide a safer isolation path; see `docs/telemetry-diagnostics.md`. Keep full radar disabled until crash isolation succeeds.
