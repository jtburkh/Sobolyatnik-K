import pathlib
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2] / "tools"))
import check_source_version as gate


class VersionGateTests(unittest.TestCase):
    def test_order(self):
        self.assertLess(gate.rank("0.1.11"), gate.rank("0.1.13-test.1"))
        self.assertLess(gate.rank("0.1.13-test.1"), gate.rank("0.1.13-test.2"))
        self.assertLess(gate.rank("0.1.13-test.2"), gate.rank("0.1.13"))
        self.assertLess(gate.rank("0.1.13-rc.1"), gate.rank("0.1.13-rc.2"))
        with self.assertRaises(ValueError):
            gate.rank("0.1.13-test..x")

    def check_synthetic(self, previous, current, changed):
        with tempfile.TemporaryDirectory() as temp:
            root = pathlib.Path(temp)
            (root / "godot-mod/rtv-radar-range").mkdir(parents=True)
            (root / "Cargo.toml").write_text(f'[package]\nname="rtv-toolkit"\nversion="{current}"\n')
            (root / "Cargo.lock").write_text(f'[[package]]\nname="rtv-toolkit"\nversion="{current}"\n')
            (root / "godot-mod/rtv-radar-range/mod.txt").write_text(f'[mod]\nversion="{current}"\n')

            def fake_git(*args):
                if args[0] == "cat-file":
                    return ""
                if args[0] == "diff":
                    return changed
                if args[0] == "show":
                    return f'[package]\nversion="{previous}"\n'
                raise AssertionError(args)

            with patch.object(gate, "ROOT", root), patch.object(gate, "git", fake_git):
                return gate.check("base")

    def test_blocks_source_push_without_bump(self):
        with self.assertRaisesRegex(ValueError, "did not advance"):
            self.check_synthetic("0.1.13-test.1", "0.1.13-test.1", "src/app.rs")

    def test_accepts_next_test_version(self):
        self.assertIn("0.1.13-test.2", self.check_synthetic("0.1.13-test.1", "0.1.13-test.2", "tools/install.py"))

    def test_documentation_change_needs_no_bump(self):
        self.assertIn("No code changes", self.check_synthetic("0.1.13-test.1", "0.1.13-test.1", "README.md"))

    def test_manifest_mismatch_is_never_silent(self):
        with tempfile.TemporaryDirectory() as temp:
            root = pathlib.Path(temp)
            (root / "godot-mod/rtv-radar-range").mkdir(parents=True)
            (root / "Cargo.toml").write_text('[package]\nversion="0.1.13-test.1"\n')
            (root / "Cargo.lock").write_text('[[package]]\nname="rtv-toolkit"\nversion="0.1.13-test.1"\n')
            (root / "godot-mod/rtv-radar-range/mod.txt").write_text('[mod]\nversion="0.1.13-test.2"\n')
            with patch.object(gate, "ROOT", root), self.assertRaisesRegex(ValueError, "radar manifest"):
                gate.check("base")


if __name__ == "__main__":
    unittest.main()
