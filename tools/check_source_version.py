#!/usr/bin/env python3
"""Require a new coherent Toolkit/radar prerelease version for code pushes.

Use --base <remote tip> in the pre-push hook and CI. Documentation-only
changes do not require a bump; any change to shipped code, tests or build
workflow does. The release decision remains separate from this source gate.
"""

import argparse
from pathlib import Path
import re
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parent.parent
VERSION_PATTERN = re.compile(r"(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?\Z")


def rank(value: str) -> tuple[int, int, int, int, tuple[tuple[int, int | str], ...]]:
    match = VERSION_PATTERN.fullmatch(value)
    if not match:
        raise ValueError(f"unsupported package version {value!r}; use a SemVer-compatible version")
    major, minor, patch, prerelease = match.groups()
    identifiers = tuple((0, int(part)) if part.isdecimal() else (1, part) for part in (prerelease or "").split(".") if part)
    return (int(major), int(minor), int(patch), int(prerelease is None), identifiers)


def git(*args: str) -> str:
    return subprocess.check_output(["git", *args], cwd=ROOT, text=True, stderr=subprocess.PIPE).strip()


def package_version(text: str) -> str:
    return tomllib.loads(text)["package"]["version"]


def check(base: str) -> str:
    current = package_version((ROOT / "Cargo.toml").read_text())
    rank(current)
    packages = tomllib.loads((ROOT / "Cargo.lock").read_text())["package"]
    matches = [package["version"] for package in packages if package["name"] == "rtv-toolkit"]
    if matches != [current]:
        raise ValueError(f"Cargo.lock package version {matches!r} != Cargo.toml {current!r}")
    radar = (ROOT / "godot-mod/rtv-radar-range/mod.txt").read_text()
    if not re.search(r'^version="' + re.escape(current) + r'"$', radar, re.MULTILINE):
        raise ValueError("candidate radar manifest and Toolkit package versions differ")
    git("cat-file", "-e", f"{base}^{{commit}}")  # Never silently waive the gate on a missing base.
    changes = git("diff", "--name-only", f"{base}...HEAD").splitlines()
    relevant = [path for path in changes if path.startswith(("src/", "godot-mod/", "tools/", "tests/", ".github/workflows/")) or path in ("Cargo.toml", "Cargo.lock", "build.rs")]
    if not relevant:
        return f"No code changes since {base}; {current} is consistent (no version bump needed)."
    previous = package_version(git("show", f"{base}:Cargo.toml"))
    if rank(current) <= rank(previous):
        raise ValueError(f"Code changed ({relevant[0]}), but version {current} did not advance beyond {previous}. Bump Cargo.toml, Cargo.lock, and the candidate mod manifest before pushing.")
    return f"Version advanced {previous} -> {current}; {len(relevant)} code paths changed."


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", required=True, help="remote branch tip or PR base commit")
    args = parser.parse_args()
    try:
        print(check(args.base))
    except (ValueError, subprocess.CalledProcessError) as error:
        print(f"VERSION GATE: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
