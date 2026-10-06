---
name: steam-build-gate
description: Warn about Road to Vostok's live Steam public build, local Steam install and pinned Sobolyatnik-K installer before GitHub source pushes; use strict checks before release approval. Preserve the installed game.
---

# Steam build pre-push gate

Run from the public `Sobolyatnik-K` repository before **every push** (even a source-only one):

```bash
python3 tools/check_steam_build.py
```

When Steam is installed outside the default WSL/Windows library, pass `--manifest /path/to/appmanifest_1963610.acf`. The script reads only the Steam appmanifest, the version-pinned v0.1.12 radar installer, and a live HTTPS Steam PICS-derived response. It prints only build/depot IDs, never the manifest's private account fields. It does not run Steam, install the game, write to saves, or alter Metro. It uses the **third-party** `api.steamcmd.net` mirror, not a Valve-owned API: Steam's anonymous `ISteamApps/UpToDateCheck` currently returns no app info for this game. Do not substitute an unverified SteamDB page, cached result, or fabricated build number if the live query fails.

For **source pushes**, this is advisory: it warns on a different or unknown build and exits zero. Report the live/local/installer IDs (or the lookup error) but **do not block the user's source push**. A zero exit in this mode is *not* evidence that IDs match. For **release readiness**, run `python3 tools/check_steam_build.py --strict`: mismatched or unavailable information exits nonzero. Even a strict-mode success establishes build identity only, **not** mod compatibility or approval to install. For a new build, create/update a Beads issue, inspect changed interfaces read-only, run offline/Windows fixture checks, then seek explicit owner permission for controlled, game-closed, disposable-save in-game validation. Never bypass the published installer pin or change the immutable v0.1.12 release; make a separately versioned installer only after validation. Do not push private Beads data, game files, saves or recovered proprietary resources.

The repository contains an optional **warning-only** `.githooks/pre-push`. Enable for this checkout only with `git config --local core.hooksPath .githooks`; do not overwrite an existing hooks path without reviewing it. It never authorizes a new installer or suppresses warnings. Do not change the published installer build gate to match a new number without compatibility validation.

For synthetic, offline gate tests:

```bash
python3 -m unittest discover -s tests/steam -p 'test_*.py'
```

The live source is https://api.steamcmd.net/v1/info/1963610 (public branch `depots.branches.public.buildid` and depot `1963611` public `gid`). Valve's public API documentation for `UpToDateCheck` is https://partner.steamgames.com/doc/webapi/isteamapps ; it is not suitable for this game's anonymous build check. Match the local Steam `appmanifest_1963610.acf` before relying on a mirror response.
