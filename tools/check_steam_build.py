#!/usr/bin/env python3
"""Read-only, advisory Steam public-build check; use --strict for release readiness.

The public Steam Web API does not expose this game's build to anonymous callers.
api.steamcmd.net mirrors Steam PICS app_info (third party, not Valve); compare its
public branch and depot manifest with Steam's *local* appmanifest as a second
observation. Neither source proves mod/installer compatibility.
"""

import argparse
import json
import os
from pathlib import Path
import re
import sys
from urllib.request import Request, urlopen

APP_ID = "1963610"
DEPOT_ID = "1963611"
SOURCE = f"https://api.steamcmd.net/v1/info/{APP_ID}"
INSTALLER = Path(__file__).resolve().parent / "install-sobolyatnik-v0.1.12.ps1"


class BuildCheckError(Exception):
    pass


def digits(value: object, label: str) -> str:
    if isinstance(value, bool) or not re.fullmatch(r"[1-9][0-9]*", str(value)):
        raise BuildCheckError(f"missing or invalid {label}")
    return str(value)


def only_field(text: str, field: str) -> str:
    values = re.findall(r'(?m)^\s*"' + re.escape(field) + r'"\s*"([^"\r\n]*)"', text)
    if len(values) != 1:
        raise BuildCheckError(f"expected exactly one {field} field in Steam appmanifest")
    return digits(values[0], f"appmanifest {field}")


def installed_build(manifest: Path) -> tuple[str, str]:
    try:
        text = manifest.read_text(encoding="utf-8-sig")
    except OSError as error:
        raise BuildCheckError(f"cannot read local Steam appmanifest {manifest}: {error}") from error
    if only_field(text, "appid") != APP_ID:
        raise BuildCheckError("local Steam appmanifest is not Road to Vostok")
    build = only_field(text, "buildid")
    # A matching build but a different depot manifest is still NOT equivalent.
    depot = re.search(
        r'"InstalledDepots"\s*\{\s*"' + DEPOT_ID + r'"\s*\{\s*"manifest"\s*"([0-9]+)"',
        text, re.S,
    )
    if depot is None:
        raise BuildCheckError(f"missing installed depot {DEPOT_ID} manifest")
    return build, digits(depot.group(1), "installed depot manifest")


def supported_build(installer: Path = INSTALLER) -> str:
    try:
        text = installer.read_text(encoding="utf-8-sig")
    except OSError as error:
        raise BuildCheckError(f"cannot read pinned installer: {error}") from error
    values = re.findall(r"(?m)^\$expectedBuild\s*=\s*'([0-9]+)'\s*$", text)
    if len(values) != 1:
        raise BuildCheckError("could not identify a unique supported build in the installer")
    return digits(values[0], "installer supported build")


def public_build(info: dict) -> tuple[str, str]:
    try:
        if info["status"] != "success":
            raise BuildCheckError("Steam PICS mirror did not return success")
        app = info["data"][APP_ID]
        if str(app["appid"]) != APP_ID or app["common"]["name"] != "Road to Vostok":
            raise BuildCheckError("Steam PICS mirror returned the wrong app")
        build = digits(app["depots"]["branches"]["public"]["buildid"], "public build")
        depot = digits(app["depots"][DEPOT_ID]["manifests"]["public"]["gid"], "public depot manifest")
        return build, depot
    except (KeyError, TypeError, ValueError) as error:
        raise BuildCheckError(f"Steam PICS mirror response incomplete: {error}") from error


def fetch_public() -> dict:
    request = Request(SOURCE, headers={"Accept": "application/json", "Cache-Control": "no-cache", "User-Agent": "Sobolyatnik-K-build-gate/1"})
    try:
        with urlopen(request, timeout=15) as response:
            if response.geturl() != SOURCE:
                raise BuildCheckError("Steam PICS mirror redirected unexpectedly")
            payload = response.read(1_000_001)
    except (OSError, TimeoutError) as error:
        raise BuildCheckError(f"could not query live Steam PICS mirror: {error}") from error
    if len(payload) > 1_000_000:
        raise BuildCheckError("Steam PICS mirror response exceeds size limit")
    try:
        return json.loads(payload)
    except (UnicodeDecodeError, ValueError) as error:
        raise BuildCheckError(f"invalid Steam PICS mirror JSON: {error}") from error


def default_manifest() -> Path:
    candidates = [
        Path("/mnt/c/Program Files (x86)/Steam/steamapps") / f"appmanifest_{APP_ID}.acf",
        Path(os.environ.get("ProgramFiles(x86)", "C:/Program Files (x86)")) / "Steam" / "steamapps" / f"appmanifest_{APP_ID}.acf",
    ]
    for path in candidates:
        if path.is_file():
            return path
    raise BuildCheckError("local Steam appmanifest not found; supply --manifest PATH (no game files will be modified)")


def check(manifest: Path, installer: Path = INSTALLER, info: dict | None = None) -> tuple[str, list[str]]:
    local, local_depot = installed_build(manifest)
    supported = supported_build(installer)
    remote, remote_depot = public_build(fetch_public() if info is None else info)
    report = f"Steam public {remote} (depot {remote_depot}); local {local} (depot {local_depot}); pinned installer {supported}"
    problems = []
    if remote != local:
        problems.append("local Steam build differs from the live public branch")
    if remote_depot != local_depot:
        problems.append("local depot manifest differs from the live public depot")
    if remote != supported:
        problems.append("published installer has NOT been validated for this public build")
    return report, problems


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, help="path to Steam appmanifest_1963610.acf if Steam uses a different library")
    parser.add_argument("--strict", action="store_true", help="fail on mismatch or unavailable data when checking release readiness")
    args = parser.parse_args()
    try:
        report, problems = check(args.manifest or default_manifest())
    except BuildCheckError as error:
        print(f"WARNING: cannot verify Steam build: {error}", file=sys.stderr)
        if args.strict:
            print("RELEASE NOT CLEARED: build identity could not be verified.", file=sys.stderr)
            return 2
        print("Source push may proceed; release requires a verified build and compatibility checks.", file=sys.stderr)
        return 0
    print(report, flush=True)
    if problems:
        for problem in problems:
            print(f"WARNING: {problem}", file=sys.stderr)
        if args.strict:
            print("RELEASE NOT CLEARED: validate the new game build before a separately versioned release.", file=sys.stderr)
            return 1
        print("Source push may proceed; do not relax the old installer guard.", file=sys.stderr)
        return 0
    print("Build identity matches; this is NOT mod compatibility clearance. Follow the compatibility/fixture checklist before a release.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
