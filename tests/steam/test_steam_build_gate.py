"""Offline, synthetic-only pre-push gate tests. Never inspect or write game files."""

from contextlib import redirect_stderr, redirect_stdout
import io
import sys
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "tools"))
import check_steam_build as gate


def response(build="25632875", depot="3936399382394666318"):
    return {"status": "success", "data": {gate.APP_ID: {
        "appid": gate.APP_ID, "common": {"name": "Road to Vostok"},
        "depots": {
            "branches": {"public": {"buildid": build}},
            gate.DEPOT_ID: {"manifests": {"public": {"gid": depot}}},
        },
    }}}


class BuildGateTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="rtv-steam-build-gate-")
        self.addCleanup(self.temp.cleanup)
        root = Path(self.temp.name)
        self.manifest = root / "appmanifest_1963610.acf"
        self.installer = root / "pinned-installer.ps1"
        self.installer.write_text("$expectedBuild = '25632875'\n", encoding="utf-8")
        self.set_manifest("25632875", "3936399382394666318")

    def set_manifest(self, build, depot, appid="1963610"):
        self.manifest.write_text(
            f'"AppState"\n{{\n "appid" "{appid}"\n "buildid" "{build}"\n'
            f' "InstalledDepots"\n {{\n  "1963611"\n  {{\n   "manifest" "{depot}"\n  }}\n }}\n}}\n',
            encoding="utf-8",
        )

    def test_matching_build_and_depot_allow_identity_check_not_compatibility(self):
        report, blockers = gate.check(self.manifest, self.installer, response())
        self.assertEqual(blockers, [])
        self.assertIn("Steam public 25632875", report)
        self.assertIn("pinned installer 25632875", report)

    def test_hotfix_blocks_even_when_steam_install_is_current(self):
        self.set_manifest("25710663", "4116347595363715363")
        _, blockers = gate.check(self.manifest, self.installer, response("25710663", "4116347595363715363"))
        self.assertEqual(len(blockers), 1)
        self.assertIn("NOT been validated", blockers[0])

    def test_stale_or_wrong_local_install_blocks(self):
        _, blockers = gate.check(self.manifest, self.installer, response("25710663", "4116347595363715363"))
        self.assertEqual(len(blockers), 3)
        self.set_manifest("25632875", "4116347595363715363")
        _, blockers = gate.check(self.manifest, self.installer, response())
        self.assertTrue(any("depot manifest differs" in block for block in blockers))

    def test_bad_app_or_response_fails_closed(self):
        self.set_manifest("25632875", "3936399382394666318", appid="9")
        with self.assertRaises(gate.BuildCheckError):
            gate.check(self.manifest, self.installer, response())
        self.set_manifest("25632875", "3936399382394666318")
        for data in ({"status": "error"}, response("0"), {"status": "success", "data": {}}):
            with self.subTest(data=data), self.assertRaises(gate.BuildCheckError):
                gate.check(self.manifest, self.installer, data)
        with patch.object(gate, "fetch_public", side_effect=gate.BuildCheckError("offline")):
            with self.assertRaises(gate.BuildCheckError):
                gate.check(self.manifest, self.installer)

    def test_source_push_warns_but_release_strict_mode_rejects_mismatch_and_outage(self):
        def call_cli(arguments, result=None, failure=None):
            stdout, stderr = io.StringIO(), io.StringIO()
            with patch.object(sys, "argv", ["check_steam_build.py", *arguments]), \
                 patch.object(gate, "default_manifest", return_value=self.manifest), \
                 patch.object(gate, "check", return_value=result, side_effect=failure), \
                 redirect_stdout(stdout), redirect_stderr(stderr):
                code = gate.main()
            return code, stdout.getvalue(), stderr.getvalue()

        mismatch = ("Steam public 25710663; pinned installer 25632875", ["untested public build"])
        advisory, report, warning = call_cli([], mismatch)
        self.assertEqual(advisory, 0)
        self.assertIn("25710663", report)
        self.assertIn("WARNING: untested public build", warning)
        strict, _, strict_message = call_cli(["--strict"], mismatch)
        self.assertEqual(strict, 1)
        self.assertIn("RELEASE NOT CLEARED", strict_message)
        offline, _, warning = call_cli([], failure=gate.BuildCheckError("offline"))
        self.assertEqual(offline, 0)
        self.assertIn("cannot verify", warning)
        strict_offline, _, strict_message = call_cli(["--strict"], failure=gate.BuildCheckError("offline"))
        self.assertEqual(strict_offline, 2)
        self.assertIn("RELEASE NOT CLEARED", strict_message)
        matched, message, _ = call_cli([], ("IDs match", []))
        self.assertEqual(matched, 0)
        self.assertIn("NOT mod compatibility clearance", message)

    def test_missing_or_duplicate_installer_pin_and_manifest_fields_fail_closed(self):
        self.installer.write_text("# missing pin\n", encoding="utf-8")
        with self.assertRaises(gate.BuildCheckError):
            gate.check(self.manifest, self.installer, response())
        self.installer.write_text("$expectedBuild = '25632875'\n$expectedBuild = '25710663'\n", encoding="utf-8")
        with self.assertRaises(gate.BuildCheckError):
            gate.check(self.manifest, self.installer, response())
        self.installer.write_text("$expectedBuild = '25632875'\n", encoding="utf-8")
        self.manifest.write_text(self.manifest.read_text() + '\n "buildid" "25632875"\n')
        with self.assertRaises(gate.BuildCheckError):
            gate.check(self.manifest, self.installer, response())


if __name__ == "__main__":
    unittest.main()
