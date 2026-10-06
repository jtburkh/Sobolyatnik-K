#!/usr/bin/env python3
"""Build a portable, deterministic ZIP from an already tested offline kit.

Reject missing/extra entries or symlinks; Windows Compress-Archive writes
backslash member names that fail to extract as directories on other systems.
"""

import argparse
import hashlib
from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile, ZipInfo

EXPECTED = (
    "RtVRadarLoot.vmz",
    "TEST-README.txt",
    "install-sobolyatnik.ps1",
    "metro/LICENSE",
    "metro/modloader.gd",
    "metro/override.cfg",
    "rtv-toolkit.exe",
    "run-test-install.ps1",
    "setup-sobolyatnik.ps1",
    "uninstall-sobolyatnik.ps1",
)


def make_zip(kit: Path, output: Path) -> str:
    if output.exists():
        raise ValueError(f"refusing to replace existing ZIP: {output}")
    names = []
    for path in kit.rglob("*"):
        if path.is_symlink():
            raise ValueError(f"test kit contains a symlink: {path}")
        if path.is_file():
            names.append(path.relative_to(kit).as_posix())
        elif not path.is_dir():
            raise ValueError(f"test kit contains an unsupported file: {path}")
    if sorted(names) != sorted(EXPECTED):
        raise ValueError(f"test kit members differ from expected list: {sorted(names)!r}")
    output.parent.mkdir(parents=True, exist_ok=True)
    with ZipFile(output, "w", compression=ZIP_DEFLATED, compresslevel=9) as archive:
        for name in sorted(EXPECTED):
            info = ZipInfo(name, (2020, 1, 1, 0, 0, 0))
            info.compress_type = ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            archive.writestr(info, (kit / name).read_bytes(), compresslevel=9)
    with ZipFile(output) as archive:
        if archive.namelist() != sorted(EXPECTED) or archive.testzip() is not None:
            raise ValueError("generated ZIP contents did not verify")
    return hashlib.sha256(output.read_bytes()).hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--kit", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    print("ZIP SHA-256:", make_zip(args.kit, args.output))


if __name__ == "__main__":
    main()
