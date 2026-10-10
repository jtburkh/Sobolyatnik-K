# Sobolyatnik-K v1.2 — Windows

The versioned [v1.2.0 GitHub release](https://github.com/jtburkh/Sobolyatnik-K/releases/tag/v1.2.0) provides `Sobolyatnik-K-v1.2-Windows.zip` and its adjacent `.sha256`. **Use the README's single checksum-verifying PowerShell install command only once the README recommends v1.2.** Until the public release assets are verified, the README recommends the existing v1.1 release. The ZIP includes the matching 1.2.0 VMZ and Windows Toolkit, offline setup/uninstall and hash-pinned MIT Metro 3.4.2. Standalone VMZ/EXE downloads are not full installers; no Nexusmods ZIP is available.

## Before installation

1. Close Road to Vostok and the Toolkit; back up important saves. Setup and uninstall reject a running game. Never edit or save a character through the Toolkit while RTV runs.
2. **Uninstall another installed Sobolyatnik-K bundle first** via Windows *Settings → Apps → Installed apps*. The new installer refuses to overwrite a different receipt, VMZ, EXE, shortcut or registration. Uninstall retains existing verified Metro, saves and rollback backups. Do not run an older setup over v1.2 to downgrade.
3. Download the ZIP and `.sha256` from the same GitHub release and verify the ZIP hash **before** extracting the complete archive and running `run-sobolyatnik.ps1`. Prefer the README's one-line verified install command when it points to these public assets. Do not use the local GNU trial ZIP or run files from a partial extraction.

Fresh installation includes Metro Mod Loader 3.4.2; hash-verified existing Metro 3.2.1 or 3.4.2 is left unchanged. Unknown or partial Metro, conflicting mods and missing/invalid Steam appmanifest data block installation. A different *valid* Steam build warns but allows installation; **25837777** (RTV 0.2.1.0) was reviewed and tested for one owner gameplay session, not broadly cleared. Setup backs up and reconciles only a stale radar mount cache. It does not change saves, game executables, other mods' caches or an existing Metro script. Backups are retained, not automatically pruned.

## Controls and limits

Shot Alerts are visible by default; scene-local shot markers fade after five seconds. **F7** cycles layers, **F8** toggles HUD, **F9** selects the display-only 50/100/200/400 m range. **F10** selects CASA airdrop → Punisher → Bogeyman and **F11** confirms; the HUD reports queued/rejected actions, not proof of a subsequent encounter. In the Toolkit's **Radar** tab, use `p`, `b`, or Shift+B for the same actions, then `y`/Enter to confirm while fresh compatible telemetry is present. The Toolkit's `+`/`-` view range is independent. Historical standalone spawn CLI flags fail closed. No `AI.PlayFire` hook or AI-perception change is included.

The owner observed the CASA drop and killed both bosses in the local v1.2 VMZ trial (Punisher was challenging); a game-closed log corroborated both kills with no script errors in that session. Toolkit-triggered summons **have not** been observed live. MSVC release CI, mock Godot and synthetic installer fixtures are not a guarantee of crash-free combat, independent-PC safety, reliable 400 m gunshot observation or future Steam build compatibility. Report issues with build ID and redacted logs, never private saves or recovered game resources. See the [development/validation record](summon-candidate-v1.2.md).

**Uninstall:** With the game and Toolkit closed, use Windows *Installed apps → Sobolyatnik-K* or its Start Menu uninstaller. Only hash-verified owned files are removed; Metro, saves and rollback backups remain.
