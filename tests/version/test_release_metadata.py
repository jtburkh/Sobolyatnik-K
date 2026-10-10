"""Keep README on the verified public release through staged publication."""

from pathlib import Path
import re
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[2]


class ReleaseMetadataTests(unittest.TestCase):
    def test_v11_release_source_workflow_and_assets_agree(self):
        version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
        self.assertEqual(version, "1.2.0")
        self.assertIn('version="1.2.0"', (ROOT / "godot-mod/rtv-radar-summons/mod.txt").read_text())
        workflow = (ROOT / ".github/workflows/sobolyatnik-v1.1-release.yml").read_text()
        builder = (ROOT / "tools/build_hotfix_test_kit.py").read_text()
        manifest = (ROOT / "godot-mod/rtv-radar-range/mod.txt").read_text()
        self.assertRegex(workflow, r"(?m)^\s+- v1\.1\.0$")
        self.assertIn("name: 'Sobolyatnik-K v1.1'", workflow)
        self.assertIn("Sobolyatnik-K-v1.1-Windows.zip", workflow)
        self.assertIn('draft: false', workflow)
        self.assertIn('prerelease: false', workflow)
        self.assertIn('version="1.1.0"', manifest)
        self.assertIn('BUILD_ID = "25837777"', builder)
        self.assertIn('30c39e3846957f43925e49ef2449c2ca8f6940c77446af169cfafc858975bcc8', builder)
        for asset in ("RtVRadarLoot.vmz", "rtv-toolkit.exe"):
            self.assertIn("dist/release/" + asset + "\n", workflow)
            self.assertIn("dist/release/" + asset + ".sha256\n", workflow)

    def test_v12_release_source_workflow_and_assets_agree(self):
        workflow = (ROOT / ".github/workflows/sobolyatnik-v1.2-release.yml").read_text()
        self.assertRegex(workflow, r"(?m)^\s+- v1\.2\.0$")
        self.assertIn("name: 'Sobolyatnik-K v1.2'", workflow)
        self.assertIn("Sobolyatnik-K-v1.2-Windows.zip", workflow)
        self.assertIn("'rtv-toolkit 1\\.2\\.0'", workflow)
        self.assertIn("draft: false", workflow)
        self.assertIn("prerelease: false", workflow)
        self.assertIn("v12-from-v11-migration-smoke.ps1", workflow)
        self.assertIn("test_radar_summons.py --engine", workflow)
        self.assertIn("--player-bundle dist/release/RtVRadarLoot.vmz", workflow)
        for asset in ("RtVRadarLoot.vmz", "rtv-toolkit.exe"):
            self.assertIn("dist/release/" + asset + "\n", workflow)
            self.assertIn("dist/release/" + asset + ".sha256\n", workflow)

    def test_current_release_title_tag_zip_and_readme_agree(self):
        readme = (ROOT / "README.md").read_text().split("<!-- Historical", 1)[0]
        match = re.search(
            r"\*\*Recommended download: \[([^]]+)\]"
            r"\(https://github\.com/jtburkh/Sobolyatnik-K/releases/tag/(v[^)]+)\)",
            readme,
        )
        self.assertIsNotNone(match)
        label, public_tag = match.groups()
        if public_tag == "v0.1.16":
            # Historical Git source tag is unchanged; the public release alias
            # now serves the same immutable bytes from the shorter v0.1.16 tag.
            self.assertEqual(label, "v0.1.16")
            workflow = ROOT / ".github/workflows/sobolyatnik-v0.1.16-experimental-release.yml"
            source = workflow.read_text()
            self.assertRegex(source, r"(?m)^\s+- v0\.1\.16-experimental\.1$")
            self.assertIn("name: 'Sobolyatnik-K 0.1.16 Experimental", source)
            zip_name = "Sobolyatnik-K-0.1.16-experimental.1-reviewed25710663-Windows.zip"
            self.assertNotIn("/releases/download/v0.1.16-experimental.1/", readme)
            self.assertNotIn("Sobolyatnik-K-v1.1-Windows.zip", readme)
        elif public_tag in ("v1.1.0", "v1.2.0"):
            label_version = "v1.1" if public_tag == "v1.1.0" else "v1.2"
            self.assertEqual(label, label_version)
            workflow = ROOT / f".github/workflows/sobolyatnik-{label_version}-release.yml"
            source = workflow.read_text()
            self.assertRegex(source, rf"(?m)^\s+- {re.escape(public_tag)}$")
            self.assertIn(f"name: 'Sobolyatnik-K {label_version}'", source)
            zip_name = f"Sobolyatnik-K-{label_version}-Windows.zip"
            for asset in ("RtVRadarLoot.vmz", "rtv-toolkit.exe"):
                self.assertIn("dist/release/" + asset + "\n", source)
                self.assertIn("dist/release/" + asset + ".sha256\n", source)
            self.assertNotIn("experimental", readme.lower())
        else:
            self.fail(f"Unreviewed README release tag: {public_tag}")
        self.assertIn(zip_name, source)
        self.assertIn(zip_name, readme)
        self.assertIn("/releases/download/" + public_tag + "/", readme)
        self.assertIn("/releases/tag/" + public_tag + ")", readme)
        self.assertNotIn("v0.1.17-experimental.1", readme)
        self.assertNotIn("v0.1.18-experimental.1", readme)
        self.assertEqual(readme.count("```powershell"), 1)
        self.assertNotIn("sobolyatnik-k-v0.1.12.ps1", readme)


if __name__ == "__main__":
    unittest.main()
