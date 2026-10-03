#!/usr/bin/env python3
"""Render a source-reviewed installer with the exact Windows EXE/script hashes.

The Windows runner compiles and smoke-tests the executable first. The release
job renders this template only after downloading that same tested artifact.
"""

import argparse
import hashlib
from pathlib import Path
import struct

ROOT = Path(__file__).resolve().parents[1]
TEMPLATE = ROOT / "tools" / "setup-sobolyatnik.ps1.in"
UNINSTALLER = ROOT / "tools" / "uninstall-sobolyatnik.ps1"


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--toolkit", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--template", type=Path, default=TEMPLATE)
    parser.add_argument("--uninstaller", type=Path, default=UNINSTALLER)
    args = parser.parse_args()
    executable = args.toolkit.read_bytes()
    if len(executable) < 512 or executable[:2] != b"MZ":
        raise SystemExit("not a Windows PE executable")
    pe_offset = struct.unpack_from("<I", executable, 0x3C)[0]
    if pe_offset + 6 > len(executable) or executable[pe_offset : pe_offset + 4] != b"PE\0\0":
        raise SystemExit("invalid Windows executable PE header")
    if struct.unpack_from("<H", executable, pe_offset + 4)[0] != 0x8664:
        raise SystemExit("expected an x86-64 Windows executable")
    template = args.template.read_bytes()
    uninstaller = args.uninstaller.read_bytes()
    for placeholder in (b"@TOOLKIT_SHA256@", b"@UNINSTALLER_SHA256@"):
        if template.count(placeholder) != 1:
            raise SystemExit(f"template must have exactly one {placeholder!r}")
    rendered = template.replace(b"@TOOLKIT_SHA256@", sha256(executable).encode("ascii"))
    rendered = rendered.replace(b"@UNINSTALLER_SHA256@", sha256(uninstaller).encode("ascii"))
    if b"@TOOLKIT_SHA256@" in rendered or b"@UNINSTALLER_SHA256@" in rendered:
        raise SystemExit("unrendered placeholder")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(rendered)
    print("Toolkit SHA-256:", sha256(executable))
    print("Uninstaller SHA-256:", sha256(uninstaller))
    print("Installer SHA-256:", sha256(rendered))


if __name__ == "__main__":
    main()
