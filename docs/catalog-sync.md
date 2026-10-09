# Inventory catalog provenance and verification

The current **candidate source** catalog was derived read-only from Road to Vostok
Steam build **25837777** (game 0.2.1.0, Godot 4.6.3). The Toolkit catalog is
compiled into its Rust executable: changing `data/*.json` does **not** update
already published `.exe` files. The recommended v0.1.16 ZIP remains unchanged,
with its earlier Build 2 catalog. The new catalog is compiled into a locally
tested v1.1 Windows EXE, but that candidate has not been installed or published.

## What's new in Build 2

`data/catalog-manifest.json` records **318** game-registered resources: **273**
inventory items and **45** furniture resources, including **29** weapon stats.
Compared with the prior Build 2 catalog (305/263/42), Patch 0.2.1.0 adds ten
inventory items: Magazine (Crosswords), Air Freshener, Engine Radiator, Suitcase
(Driver), Museum Plate, Screws, Glue, Hand Warmer, Leather, and Phoenix. The
three resources newly registered *relative to our prior catalog* are Calendar
(from the 0.2.0.5 hotfix), Radio, and Bookshelf.
No previously cataloged resource path or weapon-stat entry was removed or
changed. Exact paths and footprints come from recovered item data, not guesses.
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
   temporary directory. GDRE converts the 318 paired resources to `.tres`
   without lossy conversions. Keep recovered scripts there as `Scripts/`.
   Avoid full recovery of textures, audio, scenes and proprietary game assets.
3. Run `python3 tools/sync_game_resources.py <temporary-recovered-root>
   --build-id 25837777` from the repository root. This creates derived
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

This resync recovered exactly 318 registered `.tres` resources from the local
0.2.1.0 PCK with **0 failed** and **0 lossy** conversions. The offline tests
also assert source counts, manifest consistency, old save paths, all 15 new
inventory footprints, and searchable Toolkit IDs. Results on other Steam
builds must be checked separately.
