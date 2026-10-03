# Inventory catalog provenance and verification

The current **source** catalog was derived read-only from Road to Vostok Steam
build **25632875** (Godot 4.6.3). The Toolkit's catalog is compiled into its
Rust executable: updating `data/*.json` in Git **does not update any already
published `.exe`**. Both v0.1.11 and the current [v0.1.12 Windows bundle](windows-bundle.md)
contain this catalog; the earlier v0.1.9 executable remains unchanged. An
existing v0.1.9 or v0.1.11 bundle must be uninstalled before installing v0.1.12.

## What's new in Build 2

`data/catalog-manifest.json` records **305** game-registered resources: **263**
inventory items and **42** furniture resources, including **29** weapon stats.
Relative to build 22914619, this adds **15 inventory items** and one campsite
bench (furniture). The inventory additions are Kantamus Backpack, Foil Hat,
Jam, Soda (Empty), Alcometer, VIRVE, Tactical Watch, Garage Key, Aluminum
Foil, HP-DA and its magazine, Jatimatic and its magazine, M28, and M28 (MOD).
The unchanged `Can_Empty` resource path is now displayed as **Can (Empty)**
(previously **Empty Can**). No previously cataloged resource path was removed.
Item footprints and weapon stats come from recovered game resources rather than
guessed sizes. These data checks do **not** prove that every in-game equipment
slot or new item interaction has been exercised; validate those separately on
a disposable save with the game closed before relying on edits.

## Read-only re-sync workflow (maintainers)

1. Confirm the game's Steam manifest build ID. Use GDRE Tools v2.6.4 or a
   verified compatible version to read the installed `RTV.pck` **without
   modifying it**. Recover `Scripts/*.gdc` in scripts-only mode to a directory
   outside both the game and this repository. In that recovered `Database.gd`,
   use only the registered `preload("res://...tscn")` entries not containing
   `_Rig` (the game excludes those aliases from its master item list).
2. Select just those registered `.tres.remap` paths under `Items/` and
   `Assets/`. Each remap names a corresponding `.godot/exported/*.res` file in
   the same PCK; recover **both** paths for each registered item into a
   temporary directory. GDRE converts the 305 paired resources to `.tres`
   without lossy conversions. Keep recovered scripts there as `Scripts/`.
   Avoid full recovery of textures, audio, scenes and proprietary game assets.
3. Run `python3 tools/sync_game_resources.py <temporary-recovered-root>
   --build-id 25632875` from the repository root. This creates derived
   metadata in `data/items.json`, `data/resources.json`,
   `data/weapon-stats.json`, and `data/catalog-manifest.json`. **Never commit**
   `RTV.pck`, extracted `.tres`/`.res`/`.tscn`/`.gdc` resources, personal
   saves, logs, or dumps.
4. Review `git diff -- data/`. Compare path sets (not just item names), IDs,
   category totals, sizes, stack/condition/amount flags, and weapon stats.
   Investigate missing entries, newly unsupported `type` values, lossy GDRE
   conversions, changes to old resource paths, and renamed items before
   publishing. Preserve old save path compatibility.
5. Run `python3 -m unittest discover -s tests/catalog -p 'test_*.py'`,
   `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features --
   -D warnings`, and `cargo test --locked`. Check known new item footprints
   against the game's inventory using a **disposable test save** with the game
   closed. Update README release status; bundle a newly compiled Toolkit in a
   new hash-pinned prerelease instead of replacing an old release asset.

This resync recovered exactly 305 registered `.tres` resources from the local
Build 2 PCK, with **0 failed** and **0 lossy** conversions. The offline tests
also assert source counts, manifest consistency, old save paths, all 15 new
inventory footprints, and searchable Toolkit IDs. Results on other Steam
builds must be checked separately.
