import hashlib
import pathlib
import struct
import sys
import tempfile
import unittest
from unittest.mock import patch
from zipfile import ZipFile

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2] / "tools"))
import build_hotfix_test_kit as builder
import zip_hotfix_test_kit as zipper


class HotfixKitTests(unittest.TestCase):
    def test_offline_versioned_kit_without_touching_release(self):
        old_installer = (builder.TOOLS / "install-sobolyatnik-v0.1.12.ps1").read_bytes()
        with tempfile.TemporaryDirectory() as temp:
            source = pathlib.Path(temp) / "fake-windows-x64.exe"
            binary = bytearray(512)
            binary[:2] = b"MZ"
            struct.pack_into("<I", binary, 0x3C, 0x80)
            binary[0x80:0x84] = b"PE\0\0"
            struct.pack_into("<H", binary, 0x84, 0x8664)
            source.write_bytes(binary)
            kit = pathlib.Path(temp) / "kit"
            metro = pathlib.Path(temp) / "metro"
            metro.mkdir()
            for name in builder.METRO_HASHES:
                (metro / name).write_bytes(f"fixture {name}".encode())
            fixture_hashes = {name: hashlib.sha256((metro / name).read_bytes()).hexdigest() for name in builder.METRO_HASHES}
            with patch.object(builder, "METRO_HASHES", fixture_hashes):
                results = builder.generate(kit, source, metro)
                with self.assertRaisesRegex(ValueError, "refusing to overwrite"):
                    builder.generate(kit, source, metro)
            version = results["version"]
            setup = (kit / "setup-sobolyatnik.ps1").read_text()
            radar = (kit / "install-sobolyatnik.ps1").read_text()
            self.assertEqual(version, '1.2.0')
            self.assertIn(f"$version = '{version}'", setup)
            self.assertIn("$newRelease = ''", setup)
            self.assertIn("'Sobolyatnik-K'", setup)
            self.assertNotIn("'Sobolyatnik-K (Experimental)'", setup)
            self.assertNotIn("'Sobolyatnik-K (Experimental)'", (kit / "uninstall-sobolyatnik.ps1").read_text())
            self.assertIn("$downloadUrl = ''", radar)
            self.assertIn("if (-not $ArchivePath)", radar)
            self.assertIn("$expectedBuild = '25837777'", radar)
            self.assertIn("$loaderRelease = 'https://github.com/ametrocavich/vostok-mod-loader/releases/download/v3.4.2'", radar)
            self.assertIn(fixture_hashes["modloader.gd"], radar)
            self.assertIn(builder.PREVIOUS_METRO_SCRIPT_HASH, radar)
            self.assertIn("Repair-RadarCache -ValidateOnly", radar)
            self.assertIn("Assert-Hash $backup $oldHash 'Radar cache rollback backup'", radar)
            self.assertIn("Repair-RadarCache\n    Write-Host \"Installed: $destination\"", radar)
            self.assertIn("(verified Metro present)", radar)
            self.assertIn("(?: \\(verified Metro present\\))?", setup)
            self.assertIn('Write-Warning "Road to Vostok Steam build $installedBuild differs', radar)
            self.assertIn('Steam appmanifest not found', radar)
            self.assertIn('Steam build ID missing or invalid', radar)
            self.assertNotIn('Steam build $expectedBuild is required', radar)
            self.assertNotIn("@TOOLKIT_SHA256@", setup)
            self.assertNotIn("@UNINSTALLER_SHA256@", setup)
            with ZipFile(kit / "RtVRadarLoot.vmz") as vmz:
                self.assertIn(f'version="{version}"', vmz.read("mod.txt").decode())
                self.assertIn('name="Sobolyatnik-K v1.2"', vmz.read("mod.txt").decode())
                self.assertIn("RtVRadarSummonBridge.gd", vmz.namelist())
                self.assertIn('"summon_actions": ["spawn_airdrop", "spawn_punisher", "spawn_bogeyman"]', vmz.read("RtVRadarLite.gd").decode())
                base = vmz.read("RtVRadarLite.gd").decode()
                self.assertIn("_overlay_enabled = OVERLAY_DEFAULT", base)
                self.assertIn("_controls_enabled = CONTROLS_DEFAULT and _overlay_enabled", base)
                self.assertNotIn('config.get_value("radar_lite", "controls"', base)
                self.assertNotIn('config.get_value("radar_lite", "overlay"', base)
                self.assertIn('config.get_value("radar_lite", "loot"', base)
                bridge = vmz.read("RtVRadarShotBridge.gd").decode()
                self.assertNotIn(".hook(", bridge)
                self.assertIn("F9 TOGGLE DISTANCE", bridge)
            self.assertEqual(old_installer, (builder.TOOLS / "install-sobolyatnik-v0.1.12.ps1").read_bytes())
            self.assertTrue((kit / "metro" / "LICENSE").is_file())
            self.assertIn("Sobolyatnik-K v1.2 Windows bundle", (kit / "README.txt").read_text())
            self.assertIn("LOCAL CANDIDATE ONLY: not published", (kit / "README.txt").read_text())
            self.assertIn("Metro Mod Loader 3.4.2", (kit / "README.txt").read_text())
            self.assertNotIn("experimental", (kit / "README.txt").read_text().lower())
            self.assertIn("v1.2 Windows bundle", (kit / "run-sobolyatnik.ps1").read_text())
            artifact = pathlib.Path(temp) / "portable.zip"
            self.assertEqual(len(zipper.make_zip(kit, artifact)), 64)
            with ZipFile(artifact) as release:
                self.assertEqual(release.namelist(), sorted(zipper.EXPECTED))
                self.assertIn("metro/LICENSE", release.namelist())
                self.assertNotIn("metro\\LICENSE", release.namelist())
            with self.assertRaisesRegex(ValueError, "refusing to replace"):
                zipper.make_zip(kit, artifact)
            (kit / "unrecognized-secret.txt").write_text("never ship this")
            with self.assertRaisesRegex(ValueError, "members differ"):
                zipper.make_zip(kit, pathlib.Path(temp) / "rejected.zip")

    def test_rejects_non_windows_or_wrong_architecture(self):
        with self.assertRaisesRegex(ValueError, "not a Windows"):
            builder.verify_pe(b"ELF")
        fake = bytearray(512)
        fake[:2] = b"MZ"
        struct.pack_into("<I", fake, 0x3C, 0x80)
        fake[0x80:0x84] = b"PE\0\0"
        struct.pack_into("<H", fake, 0x84, 0x014C)
        with self.assertRaisesRegex(ValueError, "Windows x64"):
            builder.verify_pe(fake)


if __name__ == "__main__":
    unittest.main()
