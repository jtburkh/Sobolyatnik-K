"""Public, offline consistency checks for the derived (not extracted) item catalog."""

import json
from collections import Counter
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]


def load(filename):
    return json.loads((ROOT / 'data' / filename).read_text(encoding='utf-8'))


class CatalogTests(unittest.TestCase):
    def test_build_two_manifest_and_resources_agree(self):
        manifest = load('catalog-manifest.json')
        items = load('items.json')
        resources = load('resources.json')
        weapons = load('weapon-stats.json')
        self.assertEqual(manifest['steam_build_id'], '25632875')
        self.assertEqual((len(items), len(resources), len(weapons)), (263, 305, 29))
        self.assertEqual(manifest['registered_resources'], len(resources))
        self.assertEqual(manifest['inventory_resources'], len(items))
        self.assertEqual(manifest['furniture_resources'], len(resources) - len(items))
        self.assertEqual(manifest['weapon_stats'], len(weapons))
        self.assertEqual(manifest['categories'], dict(sorted(Counter(r['category'] for r in resources).items())))
        self.assertEqual(len({r['path'] for r in resources}), len(resources))
        self.assertEqual(len({r['database_key'] for r in resources}), len(resources))
        self.assertEqual(len({item['path'] for item in items}), len(items))
        self.assertEqual(len({item['id'] for item in items}), len(items))
        self.assertEqual(len({w['path'] for w in weapons}), len(weapons))

        by_path = {r['path']: r for r in resources}
        for item in items:
            path = item['path']
            self.assertTrue(path.startswith('res://Items/'), path)
            self.assertEqual(by_path[path]['scope'], 'inventory')
            self.assertEqual(item, {k: v for k, v in by_path[path].items() if k not in ('scope', 'database_key')})
            self.assertGreater(item['width'], 0)
            self.assertGreater(item['height'], 0)
        for weapon in weapons:
            self.assertEqual(by_path[weapon['path']]['category'], 'Weapons')
            self.assertGreater(weapon['magazine_size'], 0)

    def test_new_items_and_legacy_save_paths(self):
        by_id = {item['id']: item for item in load('items.json')}
        for id, footprint in {
            'Backpack_Kantamus': (4, 5), 'Hat_Foil': (2, 2), 'Jam': (1, 1),
            'Soda_Empty': (1, 1), 'Alcometer': (1, 1), 'VIRVE': (1, 2),
            'Watch_Tactical': (1, 1), 'Key_Garage': (1, 1), 'Aluminum_Foil': (3, 1),
            'HP-DA': (3, 2), 'HP-DA_Magazine': (1, 1), 'Jatimatic': (4, 2),
            'Jatimatic_Magazine': (1, 2), 'M28': (7, 2), 'M28_MOD': (7, 2),
        }.items():
            with self.subTest(item=id):
                self.assertEqual((by_id[id]['width'], by_id[id]['height']), footprint)
        for path in (
            'res://Items/Medical/Bandage/Bandage.tres',
            'res://Items/Weapons/Colt_1911/Colt_1911.tres',
            'res://Items/Consumables/Can_Empty/Can_Empty.tres',
        ):
            self.assertIn(path, {item['path'] for item in by_id.values()})
        self.assertEqual(by_id['Can_Empty']['name'], 'Can (Empty)')


if __name__ == '__main__':
    unittest.main()
