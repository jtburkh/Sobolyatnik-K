# Unreleased selectable radar range preview

This is source-only work, **not in v0.1.12**, not approved for installation or game testing. Keep the released VMZ, installer, and rollback assets unchanged. Do not install two radar autoloads together. Windows remains the supported release platform.

The in-game HUD and Toolkit independently select **50, 100, 200 or 400 m** and default to **100 m** in this preview. On the HUD, **F9** cycles these distances and briefly confirms the choice; the current range remains printed at the top of the scope and `F9 RANGE` appears in the footer. The Toolkit retains its `+`/`-` range keys and shows `DETECT <range>m` on the scope and telemetry sidebar. F6 is used by Build 2's ragdoll debug script; F9 was not found in the reviewed Build 2 scripts, but needs an owner-controlled in-game conflict check before any release. The choices are not synchronized across the UDP link and reset to 100 m on startup.

**Meaning of distance:** a horizontal (X/Z) radius around the player for *displayed* contacts, trails, loot, and fixed shot markers. Toolkit AI/loot/shot sidebar counts and lists follow the same view filter; its packet totals are raw telemetry counters. Switching ranges scales the scope and changes what is visible without relocating or prolonging shots. It does **not** change how far game entities or loot are collected, how far telemetry is sent, AI perception, the game audio's 50/400 attenuation signature, or the shot observer's muzzle/weapon-clip checks. 400 m view cannot promise the observer will hear or classify shots at 400 m. F7 mode order, F8 hide/show, scene-local markers and five-second shot fade remain unchanged.

Offline-only packaging and Godot 4.6.3 mock test (never installs into Road to Vostok):

```sh
python3 tools/package_radar_range.py
python3 tools/test_radar_shots.py --engine /path/to/Godot_v4.6.3-stable_linux.x86_64 --range-preview
cargo test --all
```

The packager writes `dist/probes/RtVRadarRangeCandidate.vmz`; it is not a release artifact. Source is in `godot-mod/rtv-radar-range/` and deliberately leaves `godot-mod/rtv-radar-shots/` plus the pinned v0.1.12 release packager untouched.

After a push to `main`, open the [CI workflow](https://github.com/jtburkh/Sobolyatnik-K/actions/workflows/ci.yml), select the run for that commit, then download the **UNRELEASED-radar-range-preview-vmz**, **UNRELEASED-rtv-toolkit-windows-x64** and (if needed) **UNRELEASED-rtv-toolkit-linux-x64** artifacts. GitHub sign-in is required for workflow artifacts. Extract each archive before testing. The Windows artifact is independently compiled and tested with MSVC by GitHub Actions; the radar artifact is an *offline candidate* without an installer. Do not use the v0.1.12 setup script to install these previews or put the candidate VMZ beside the published radar. Use a disposable test install/save, verify hashes from the workflow run, and preserve the prior VMZ/Metro/character data for rollback. A successful build or download is not game safety clearance.

A **local Windows x64 GNU preview** with the range and Backups source was cross-compiled and copied to `dist/probes/windows-range-backups-preview/rtv-toolkit.exe` (ignored by Git). SHA-256: `1c472998fa3c1229dfbce37575542dffd44caae88908e1758eba07bfab2cdbe5`. On Windows, its `--help` and `--check --save <synthetic fixture>` passed, and all 69 cross-compiled Rust tests passed with synthetic fixtures. The internal Cargo version still reads `0.1.11`; this is **not** the published v0.1.12 EXE and is not installed by its setup script. That historical preview may be removed/rebuilt at any time. It is **not** the new test kit: see the [current Experimental Windows installation guide](windows-0.1.16.md), which provides a matching EXE, VMZ and installer for the current reviewed build. Gameplay stability remains experimental.

Before publishing, independently review the separate Backups tab and run the **new-version** Windows installer/migration/uninstall fixture tests and owner-authorized disposable gameplay validation. Preserve all original saves, Metro and binaries. Do not represent offline mock or CLI results as broad combat stability clearance.
