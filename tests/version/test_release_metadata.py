"""Keep the public release title, README one-liner and actual bundle aligned."""

from pathlib import Path
import re
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[2]


class ReleaseMetadataTests(unittest.TestCase):
    def test_current_release_title_tag_zip_and_readme_agree(self):
        version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
        base_version = version.split("-", 1)[0]
        workflow = ROOT / f".github/workflows/sobolyatnik-v{base_version}-experimental-release.yml"
        source = workflow.read_text()
        readme = (ROOT / "README.md").read_text()
        tag = f"v{version}"
        zip_name = f"Sobolyatnik-K-{version}-reviewed25710663-Windows.zip"
        self.assertRegex(source, r"(?m)^\s+- " + re.escape(tag) + r"$")
        title = re.search(r"(?m)^\s+name: 'Sobolyatnik-K ([^']+)'$", source)
        self.assertIsNotNone(title)
        self.assertTrue(title.group(1).startswith(base_version + " Experimental"), title.group(1))
        self.assertIn(zip_name, source)
        self.assertIn(zip_name, readme)
        self.assertIn("/releases/download/" + tag + "/", readme)
        self.assertNotIn("sobolyatnik-k-v0.1.12.ps1", readme)


if __name__ == "__main__":
    unittest.main()
