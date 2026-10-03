# Human-AI shot observer: design and limited evidence

**For installation, use the [hash-verified v0.1.12 Windows bundle](windows-bundle.md).**
The new published Shot Alerts VMZ is versioned separately; the old v0.1.8 and
v0.1.11 assets and tags were not changed. `RtVRadarShotsCandidate.vmz` is an
isolated development artifact, **not** a second mod to install alongside the
release. Do not modify the real game or original save for experiments without
fresh owner approval, a disposable save, and verified rollback. One controlled,
owner-driven gameplay session confirmed corrected candidate behavior but did
**not** establish broad combat/crash safety.

## Scope and source

- Only active human AI under `Map/AI/Enemies` and `Map/AI/Nomads` (including
  bosses there); never the player or BTR/vehicle child audio.
- `RtVRadarShotBridge.gd` extends the unchanged minimal snapshot bridge and
  listens to new game-owned root `AudioInstance3D` nodes. It defers inspection
  until the original game code has assigned stream, position and attenuation.
  A candidate shot must match the AI weapon's `fireSemi`, `fireAuto`, or
  `fireSuppressed` audio clip **and** originate within 0.75 m of an active AI
  muzzle with the original 50/400 audio settings. Matching attenuation alone
  would misclassify explosions. No `AI.gd` rewrite, `PlayFire` hook, resource
  override, or player audio hook is included.
- Events use the sound instance's **fixed firing position**, one event per
  observed sound node. UDP gunshots carry the current `map_id` so late packets
  from another scene are rejected. The in-game HUD draws warm rings and the
  Windows Toolkit draws warm stars; both fade smoothly over five seconds.
  HUD shots reset on map change, and Toolkit shots are cleared on map change.
- **Shot Alerts is the first, default visible mode.** `F7` cycles
  **Shot Alerts → All + Shots → AI Only → Trails Only → Loot Only** and back
  (the loot entry is omitted if loot is disabled). The radar stays visible,
  including between shots: its grid, player position and `SHOT ALERTS` label
  remain on screen. Only human-AI shot markers within 100 m appear and fade
  over five seconds; persistent AI contacts, loot and trails are not drawn in
  this mode. Selecting it with `F7` briefly confirms `RADAR: SHOT ALERTS`
  (1.8 seconds). `F8` alone hides or restores the radar; incoming shots never
  override F8. All other F7 modes retain their original behavior. No existing
  user configuration was changed.
- False negatives are preferable to explosion false positives. A silent shot,
  differently wired weapon clip or audio played before the autoload
  is ready will not be shown. The observer emitted packets in two limited
  Build 2 sessions; individual sounds were not independently correlated with
  visible muzzle flashes. Large numbers of nearby AI have not been profiled,
  and the old combat native-crash cause is still unknown.

## Safe offline checks

```bash
python3 tools/package_radar_shots_release.py
python3 tools/test_radar_shots.py --engine /path/to/Godot_v4.6.3-stable_linux.x86_64 --release-archive
cargo test --locked --all-targets
```

The smoke project is temporary, with mock AI/audio and no game binaries. It
asserts one real clip-matched AI event, rejection of an explosion using the same
attenuation settings plus mismatched/distant/player/vehicle sounds, fixed
position, gradual opacity, expiry, scene cleanup, **Shot Alerts first by
default** with a visible shot-only radar, unchanged other F7 layers (with and
without loot), and F8 hide choice. The versioned release archive is written to
`dist/probes/RtVRadarShotsRelease.vmz` (SHA-256
`6d2d06e55831f174483595d44fde6ba5c3a50f7fec183d2c66a5953ecb98efdd`).
The older `tools/package_radar_shots.py` still produces an isolated candidate
(`RtVRadarShotsCandidate.vmz`), not a second installed mod. Packaging alone
does not install anything. The original rollback archive still has pinned
SHA-256 `66fd97a4c1487be6688ef94b59ee709a3baa8c7886c490d2f03af7b8c395eefc`.

## Limited live evidence and rollback

On 2026-10-03, an owner-approved session with the previous candidate on the
existing Windows/Steam installation recorded 1,129 snapshots and 56 candidate
AI-shot UDP packets from two shooter IDs on Village. The game then closed;
the published VMZ, Metro configuration and original save directory were
restored and hash-checked. This shows Build 2 can emit candidate packets but
does **not** verify that all packets were true shots, that markers visibly
faded, or that combat is broadly stable.

A second controlled session preselected a hidden-radar interpretation of Shot
Alerts, which confused the user. It recorded 185 snapshots but **no AI-shot
events**. A third controlled session started with the default visible radar
and an F7 selection hint; it recorded 85 snapshots and **no AI-shot events**.
Both test installs and disposable saves were restored afterward. The owner then
clarified that **the radar itself should stay visible in Shot Alerts** while
only shot markers appear. Shot Alerts must also be the **first mode shown**,
not an option hidden at the end of the F7 cycle. This correction has
passed offline checks.

A fourth, owner-driven disposable-save session tested the **first, visible
Shot Alerts mode**. The owner confirmed it worked as designed; the local
loopback observer recorded 644 snapshots and 23 candidate shot packets from
two distinct AI shooters on Village. After RTV closed, the published VMZ,
Metro override, game executable and entire original save directory were
verified unchanged against pre-test copies and hashes. This validates the
specific tested behavior, **not** longer combat stability, every weapon/audio
case, or immunity to the previously observed native crash. The separately
versioned [v0.1.12-experimental release](https://github.com/jtburkh/Sobolyatnik-K/releases/tag/v0.1.12-experimental)
uses the same tested radar/observer code with a release manifest. Its exact
published VMZ and Windows installer passed offline checks, Windows CI and an
independent public-download fixture; the release archive was **not** installed
in the original game as part of that fixture.
