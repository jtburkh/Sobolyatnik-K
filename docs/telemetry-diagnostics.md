# Build 2 radar crash and inventory isolation — historical investigation

**Not installation instructions or a live inventory of installed mods.** These
notes record staged experiments over time; phrases like “currently installed”
refer to the session being described. For the supported experimental download,
use the [current radar + Toolkit guide](windows-bundle.md). No test here proves
that the radar is universally crash-free.

Three sessions with enabled 0.8.0 ended in the same native `RTV.exe` access
violation (Steam build 25632875, `c0000005`, RVA `0x2b39803`, null read at
address `0x34`). The old `AI.gd` hook was absent. A comparable no-mod session
included several enemy/Nomad firefights without crashing. The later 0.8.2
`player_only` diagnostic also coincided with inventory clicks acting like Fast
Equip; inventory returned to normal after removing it and restarting. This
points at mod-enabled behavior but does not identify a root cause. Both VMZs
were quarantined outside the game; **at that stage** the `mods` directory was empty.

The local dumps under `%LOCALAPPDATA%/CrashDumps/RTV.exe.*.dmp` are private;
do not upload them without the owner's permission. F8 and disconnecting the
Toolkit cannot disable an installed VMZ.

## Offline-built controls (not installed)

`python3 tools/package_loader_probes.py` produces two deterministic VMZs under
`dist/probes/`, separately from the bridge and excluded from release archives:

- `RtVLoaderBaseline.vmz`: only a `[mod]` manifest; no scripts, autoloads,
  hooks, game reads, or telemetry. Tests what enabling *any* VMZ does to the
  loader's static mounting and core Menu hook.
- `RtVInertAutoload.vmz`: a separate manifest plus a `Node` autoload that
  prints one startup line. It has no `_input`, `_process`, game reads, save
  writes, or hooks. Tests whether registering a minimal mod autoload matters.

Neither probe is a playable radar. Only **one** may be installed at a time,
with the game closed and explicit owner permission. First inspect inventory
behavior; do **not** run a combat stress test as the first probe check. If
either probe changes inventory behavior or crashes, stop, preserve the log,
remove it after closing the game, and investigate the loader integration. A
stable probe narrows the fault but cannot prove safety. Both controls have now
passed the user's inventory check; the manifest-only probe also survived normal
combat. Neither is installed now. The inert probe's log showed only Attic play,
not combat; no new crash dump appeared.

## Radar-lite isolation and in-game candidate

`python3 tools/package_radar_lite.py` builds **five separate test VMZs**;
install only one at a time, with permission and RTV closed. None includes
summons, sensor scans, game-state writes, or AI script hooks. Only the opt-in
loot VMZ scans interactable containers. All use the
same UDP v1 player/map telemetry at 5 Hz. The variants have distinct manifest
identities and avoid changing the user's config:

- `dist/probes/RtVRadarLite.vmz`: player telemetry only. An earlier 0.1.0
  version passed the user's inventory test; it was removed after the test.
- `dist/probes/RtVRadarContacts.vmz`: player + active AI positions/factions,
  no overlay. Version 0.1.1 passed the user's inventory and substantial
  firefight test with hits taken: the log had no script errors, and the
  3,780-sample trace had paired read/send phases, up to six contacts, and no
  new crash dump. It was removed after RTV closed. This is evidence of
  *that* session, not universal runtime safety. The initial `LINK WAITING`
  report was because the Toolkit bound to `127.0.0.1:47777` while the bridge
  sent to the WSL IP; binding the Toolkit to the same WSL IP on port 47777 corrected
  the listener mismatch.
- `dist/probes/RtVRadarInGame.vmz`: the simple AI positions plus a small
  always-visible radar in a CanvasLayer. **Earlier 0.1.2** passed a live
  inventory, visible red diamond, and incidental combat test (~6.5 minutes,
  1,935 complete samples, no new dump). That exact VMZ is preserved in
  `dist/held/RtVRadarInGame-0.1.2-first-pass.vmz` for rollback.
- `dist/probes/RtVRadarControls.vmz`: earlier **0.1.3** passed the user's
  inventory, movement-trail, AI-versus-Nomad color, and combat check (2,574
  paired samples, no new dump). Its exact VMZ is preserved as
  `dist/held/RtVRadarControls-0.1.3-combat-passed.vmz`.
  F7 cycles AI+trails / AI-only / trails-only; F8 hides/shows drawing but
  does **not** stop collection or UDP. Neither script consumes game input.
- `dist/probes/RtVRadarLoot.vmz`: **0.1.8 Sobolyatnik-K (Sable Hunter) Experimental — 1L108K** (historical rollback VMZ; one positive Village↔Attic session, not general release clearance). Same
  controls/AI collector, plus an isolated 1 Hz scan of `Interactable` proxies
  for live `LootContainer` objects, with fresh reads of `storage`/`loot` for
  nonempty interactable containers. The radar draws **hollow** green unlocked,
  amber locked, and pale-gray corpse squares within 100 m; F7 adds a loot-only
  layer. Earlier **0.1.4** showed the user a filled green square; **0.1.5**
  made it hollow but the user found the text/box sizes too large and the colors
  too bright. The 0.1.6 HUD restored the old tactical palette, 13px title/
  9px footer, finer grid and 5px outlined loot square; red enemies and blue
  Nomads are intentionally preserved. User reported 0.1.6 styling, inventory,
  loot and F7/F8 working well; logs had no new crash dump, but a few incomplete
  loot reads recur near map transitions. Version 0.1.7 only renames the HUD
  title to `SOBOLYATNIK-K` with `1L108K` and the manifest display name to
  `Sobolyatnik-K (Sable Hunter) Experimental — 1L108K`; loader ID and VMZ
  filename remain fixed, and the 0.1.7 collector is byte-identical to 0.1.6.
  The 0.1.8 candidate additionally clears cached loot references when the
  scene or player disappears, filters Interactable proxies to the active scene,
  and skips cached containers outside it; this is a protective hypothesis for
  incomplete trace phases, **not** a proven native-crash fix. The exact 0.1.6 VMZ is held at
  `dist/held/RtVRadarLoot-0.1.6-validated-style.vmz` for rollback, and the
  naming-only 0.1.7 VMZ is held at
  `dist/held/RtVRadarLoot-0.1.7-pre-scene-guard.vmz`. The only installed VMZ
  is currently 0.1.8. The user reported that it seemed to work; read-only
  post-session checks found Village↔Attic transitions with AI hits in the Godot
  log, no logged script errors or new crash dump, and 2,360 paired contact,
  loot-read and UDP phases (473 paired scans, zero unmatched loot reads).
  This is evidence for that one session, not proof of crash root cause or broad
  stability. The separately built terminal preview is
  `dist/preview/rtv-toolkit-sobolyatnik` (do not replace a running Toolkit).
  Investigate incomplete loot reads before adding additional game reads.

Contacts-enabled variants create a new capped 1 MiB
`user://rtv-radar-lite-trace-*.log`, flushing contact-read and UDP phases.
The trace narrows a failure but cannot attribute a native crash by itself;
flushing also changes timing. The loot VMZ additionally flushes `loot.scan`
and `loot.read` phases. All five **current** 0.1.8 archives passed a synthetic
mock under the official Godot 4.6.3 Linux binary (matching RTV's Godot engine
version). The mock verifies stale-scene loot exclusion but does **not** reproduce
RTV's game lifecycle or prove native stability. Earlier in-game results apply
to exact preserved older archives, not automatically to these builds:

```bash
python3 tools/test_radar_lite.py --engine /path/to/godot
```

## Flushed bridge trace (0.8.3, not installed)

`dist/RtVTelemetryProof.vmz` is an offline-built diagnostic, not a supported
release. It still defaults to `player_only`: no AI/loot reads, overlay,
CM-7 artifact, file-based command polling, or `AI.gd` hooks. If a later
bridge test is explicitly approved, it writes a **new** `user://rtv-telemetry-trace-*.log` for each launch, capped at 2 MiB. Each record includes a
monotonic timestamp, sequence, and phase; the file is flushed before and
after player/map reads, optional AI/loot reads, overlay snapshot updates, and
UDP sends. Left clicks record the *booleans* for the game's `item_equip`,
`item_transfer`, and `item_drop` input actions—not cursor location or inventory
contents. The log is outside saves but is still a new user-data file.

The last `*.enter` without matching `*.done` can identify a bridge call in
progress, **not prove** it caused the native crash. If the last sample is
complete, a crash may have occurred elsewhere. A `trace.limit_reached` record
means logging stopped at its size cap. Flushing on every checkpoint can alter
frame timing and might mask or worsen a timing-sensitive failure. These logs
cannot retroactively explain the existing dumps and cannot identify the
native function at RVA `0x2b39803` without symbols. Do not patch `RTV.exe`
or saves to obtain those symbols.

If tracing overhead matters, `[diagnostic] trace=false` disables file
writing and `trace_mouse=false` disables click-state checks in the 0.8.3
bridge. With no profile set (or an unknown value), it selects `player_only`.
Other profiles must be approved separately and selected with the game closed:

| Profile | AI reads | Loot reads | Godot overlay | Artifact/commands |
|---|---|---|---|---|
| `player_only` (default) | No | No | No | No |
| `overlay_only` | No | No | Player-only drawing | No |
| `ai_only` | Yes | No | No | No |
| `loot_only` | No | Yes | No | No |
| `overlay_ai` | Yes | No | Contact drawing | No |

```ini
[diagnostic]
profile="ai_only"
```

There is **no `full` profile**. Do not send `--spawn-boss`, `--spawn-airdrop`,
`b`, or `p` commands during diagnostic tests: queued requests are not consumed
and may run under a future full bridge. F7/F9 menus are disabled; F8 only hides
the optional overlay. Empty AI/loot arrays in isolated profiles are expected.

## Test order and stop condition

1. Preserve the no-mod baseline, user saves, and existing crash dumps. Obtain
   permission before **each** installation or config change; confirm RTV is
   closed and the `mods` directory contains only the expected current VMZ.
   Verify and preserve that exact VMZ before replacing it.
2. Test the manifest-only probe, then (only if stable) the inert-autoload
   probe in separate sessions. Check inventory clicks and drag/drop first.
   Verify which one actually loaded in `godot.log` each time.
3. Only after both controls are stable and the owner agrees, consider the
   smaller radar-lite player-only build first. Check inventory before combat;
   on success, discuss whether to enable AI contacts in a separate launch.
   The flushed 0.8.3 `player_only` trace is an alternative if more detailed
   failure diagnostics are needed, not a prerequisite to the lite bridge.
   If either player-only build malfunctions,
   **stop**: do not proceed to AI/loot/overlay or firefights. Preserve its
   flushed trace and the Godot log, then remove the VMZ when the game is closed.
4. The earlier player-only, contacts, 0.1.2 in-game, and 0.1.3 controls
   builds passed separate inventory/combat checks; the actual F7/F8 keys still
   need an in-game report. For the **new loot** build, first check inventory,
   live nearby loot squares, and disappearance after emptying a container,
   without intentional combat. Stop at any abnormality, close RTV, and roll
   back to preserved 0.1.3. Compare combat only after this phase is stable and
   the owner agrees. A stable session is not release clearance. Defer vision
   sensor scans and game-state-writing CM-7/summons; do not reenable the AI
   hook for gunshots while Metro's Build 2 rewrite does not compile.

**Toolkit contact colors:** the [v0.1.11 executable](windows-bundle.md) renders
live enemies red, Nomads light blue regardless of reputation, and bosses pink
in both the terminal radar and contact list, matching
`RtVRadarLiteOverlay.gd`. The old v0.1.9 `.exe` and September
`dist/preview/rtv-toolkit-nomad-blue` predate this correction and the Build 2
catalog. Do not replace or kill an active TUI without owner permission, and
re-check the WSL IP after a restart if using
`--telemetry-bind <WSL IPv4 address>:47777`.

The game itself may autosave during tests; use a disposable or backed-up game
session if that matters. Source checks and synthetic tests cannot establish
native Godot 4.6.3 stability. The owner can decline any additional game test.

The synthetic profile/trace smoke test uses a **temporary mock project**, not
RTV or its saves:

```bash
python3 tools/test_telemetry_diagnostics.py --engine /path/to/godot
```
