#!/usr/bin/env python3
"""Generate RtV Toolkit catalogs from a GDRE-recovered Road to Vostok project.

The game's Scripts/Database.gd is treated as the allow-list. Constants whose
name contains `_Rig` are excluded because Database.gd itself excludes them when
building its master item list.
"""

from __future__ import annotations

import argparse
import json
import re
from collections import Counter
from pathlib import Path
from typing import Any

TYPE_TO_CATEGORY = {
    "Ammo": "Ammo",
    "Armor": "Armor",
    "Attachment": "Attachments",
    "Backpack": "Backpacks",
    "Belt Pouch": "Belts",
    "Clothing": "Clothing",
    "Consumable": "Consumables",
    "Consumables": "Consumables",
    "Electronics": "Electronics",
    "Fish": "Fishing",
    "Fishing": "Fishing",
    "Furniture": "Furniture",
    "Grenade": "Grenades",
    "Helmet": "Helmets",
    "Instrument": "Instruments",
    "Key": "Keys",
    "Knife": "Knives",
    "Literature": "Books",
    "Lore": "Lore",
    "Medical": "Medical",
    "Misc": "Misc",
    "Rig": "Rigs",
    "Weapon": "Weapons",
}

WEAPON_STAT_FIELDS = (
    ("weaponType", "type"),
    ("weaponAction", "action"),
    ("caliber", "caliber"),
    ("damage", "damage"),
    ("penetration", "penetration"),
    ("fireRate", "fire_rate"),
    ("magazineSize", "magazine_size"),
    ("kick", "kick"),
    ("kickPower", "kick_power"),
    ("verticalRecoil", "vertical_recoil"),
    ("horizontalRecoil", "horizontal_recoil"),
    ("weight", "weight"),
    ("value", "value"),
    ("rarity", "rarity"),
)


def weapon_stats(root: Path, database: Path) -> list[dict[str, Any]]:
    """Pull display stats for weapons registered by Database.gd."""
    stats: dict[str, dict[str, Any]] = {}
    for _key, resource_path in database_entries(database):
        if "Weapons" not in resource_path:
            continue
        path = root / resource_path.removeprefix("res://")
        if not path.exists():
            continue
        text = path.read_text(encoding="utf-8")
        fields = resource_fields(text)
        if not fields:
            continue

        def number_or_none(field: str) -> float | None:
            raw = fields.get(field)
            if raw is None:
                return None
            try:
                return float(raw)
            except ValueError:
                return None

        entry: dict[str, Any] = {
            "id": path.stem,
            "path": "res://" + path.relative_to(root).as_posix(),
            "name": parse_string(fields.get("name")) or path.stem,
            "weapon_type": parse_string(fields.get("weaponType")),
            "action": parse_string(fields.get("weaponAction")),
            "caliber": parse_string(fields.get("caliber")),
        }
        for source, target in WEAPON_STAT_FIELDS[3:]:
            entry[target] = number_or_none(source)
        if entry["fire_rate"]:
            entry["rpm"] = round(60.0 / entry["fire_rate"])
        if entry["weapon_type"] and entry["damage"] is not None:
            stats[entry["path"]] = entry
    return sorted(stats.values(), key=lambda item: (item["name"] or "", item["path"]))


def item_data_classes(scripts: Path) -> set[str]:
    parents: dict[str, str] = {}
    for path in scripts.glob("*.gd"):
        text = path.read_text(encoding="utf-8")
        class_match = re.search(r"^class_name\s+(\w+)", text, re.MULTILINE)
        extends_match = re.search(r"^extends\s+(\w+)", text, re.MULTILINE)
        if class_match and extends_match:
            parents[class_match.group(1)] = extends_match.group(1)

    classes = {"ItemData"}
    while True:
        previous = len(classes)
        classes.update(name for name, parent in parents.items() if parent in classes)
        if len(classes) == previous:
            return classes


def parse_string(raw: str | None) -> str:
    if raw is None:
        return ""
    raw = raw.strip()
    if raw.startswith("&"):
        raw = raw[1:]
    try:
        value = json.loads(raw)
        return value.strip() if isinstance(value, str) else str(value)
    except json.JSONDecodeError:
        return raw.strip('"').strip()


def resource_fields(text: str) -> dict[str, str]:
    parts = text.split("\n[resource]\n", 1)
    if len(parts) != 2:
        return {}
    result: dict[str, str] = {}
    for line in parts[1].splitlines():
        match = re.match(r"^(\w+)\s*=\s*(.+)$", line)
        if match:
            result[match.group(1)] = match.group(2).strip()
    return result


def number(fields: dict[str, str], name: str) -> int:
    return int(float(fields.get(name, "0")))


def parse_item(path: Path, root: Path, classes: set[str]) -> dict[str, Any] | None:
    text = path.read_text(encoding="utf-8")
    first_line = text.splitlines()[0] if text else ""
    class_match = re.search(r'script_class="([^"]+)"', first_line)
    if not class_match or class_match.group(1) not in classes:
        return None

    fields = resource_fields(text)
    raw_type = parse_string(fields.get("type"))
    category = TYPE_TO_CATEGORY.get(raw_type)
    if category is None:
        return None
    size = re.fullmatch(r"Vector2\(([-\d.]+),\s*([-\d.]+)\)", fields.get("size", ""))
    width, height = (int(float(size.group(1))), int(float(size.group(2)))) if size else (1, 1)
    if width <= 0 or height <= 0:
        raise ValueError(f"invalid size {width}x{height}: {path}")

    max_amount = number(fields, "maxAmount")
    return {
        "category": category,
        "id": path.stem,
        "name": parse_string(fields.get("name")) or path.stem,
        "path": "res://" + path.relative_to(root).as_posix(),
        "width": width,
        "height": height,
        "stackable": fields.get("stackable") == "true",
        "show_condition": fields.get("showCondition") == "true",
        "show_amount": fields.get("showAmount") == "true",
        "default_amount": number(fields, "defaultAmount"),
        "max_amount": max_amount or None,
    }


def database_entries(database: Path) -> list[tuple[str, str]]:
    pattern = re.compile(
        r'^const\s+(\w+)\s*=\s*preload\("(res://[^"]+\.tscn)"\)', re.MULTILINE
    )
    entries = []
    for key, scene_path in pattern.findall(database.read_text(encoding="utf-8")):
        if "_Rig" not in key:
            entries.append((key, scene_path.removesuffix(".tscn") + ".tres"))
    return entries


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("recovered", type=Path, help="GDRE-recovered project root")
    parser.add_argument("--items-output", type=Path, default=Path("data/items.json"))
    parser.add_argument("--resources-output", type=Path, default=Path("data/resources.json"))
    parser.add_argument("--manifest-output", type=Path, default=Path("data/catalog-manifest.json"))
    parser.add_argument("--weapon-stats-output", type=Path, default=Path("data/weapon-stats.json"))
    parser.add_argument("--build-id", default="unknown")
    args = parser.parse_args()

    root = args.recovered.resolve()
    classes = item_data_classes(root / "Scripts")
    parsed: dict[str, dict[str, Any]] = {}
    for base in (root / "Items", root / "Assets"):
        for path in base.rglob("*.tres"):
            item = parse_item(path, root, classes)
            if item:
                if item["path"] in parsed:
                    raise ValueError(f'duplicate resource path: {item["path"]}')
                parsed[item["path"]] = item

    resources = []
    for database_key, resource_path in database_entries(root / "Scripts" / "Database.gd"):
        try:
            item = dict(parsed[resource_path])
        except KeyError as error:
            raise ValueError(f"Database.gd resource was not recovered: {resource_path}") from error
        item["database_key"] = database_key
        item["scope"] = "furniture" if item["category"] == "Furniture" else "inventory"
        resources.append(item)

    resources.sort(key=lambda item: (item["category"], item["name"], item["path"]))
    weap_stats = weapon_stats(root, root / "Scripts" / "Database.gd")
    items = [
        {key: value for key, value in item.items() if key not in {"database_key", "scope"}}
        for item in resources
        if item["scope"] == "inventory"
    ]
    category_counts = dict(sorted(Counter(item["category"] for item in resources).items()))
    manifest = {
        "steam_build_id": args.build_id,
        "source": "Road to Vostok RTV.pck and Scripts/Database.gd",
        "registered_resources": len(resources),
        "inventory_resources": len(items),
        "furniture_resources": len(resources) - len(items),
        "categories": category_counts,
        "weapon_stats": len(weap_stats),
    }

    for output, data in [
        (args.items_output, items),
        (args.resources_output, resources),
        (args.manifest_output, manifest),
        (args.weapon_stats_output, weap_stats),
    ]:
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
        print(f"Wrote {output} ({len(data) if isinstance(data, list) else 'manifest'})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
