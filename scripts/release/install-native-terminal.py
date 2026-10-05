#!/usr/bin/env python3
"""Install a verified native package into an explicit, empty directory."""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import shutil
import subprocess
import tempfile
import urllib.request

BASE = "https://storage.googleapis.com/openagentsgemini-cli-releases/openagents-terminal"


def fetch(url, path):
    with urllib.request.urlopen(url, timeout=120) as response, path.open("wb") as file:
        while block := response.read(1024 * 1024):
            file.write(block)


def digest(path):
    h = hashlib.sha256()
    with path.open("rb") as file:
        for block in iter(lambda: file.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--destination", type=Path, required=True)
    parser.add_argument("--base", default=BASE)
    args = parser.parse_args()
    if platform.system() != "Darwin" or platform.machine() != "arm64":
        parser.error("The native package supports macOS arm64.")
    if not all(c.isascii() and (c.isalnum() or c in ".-") for c in args.version) or not args.version:
        parser.error("Invalid version.")
    destination = args.destination.resolve()
    if destination.exists():
        parser.error("The destination must not exist.")
    with tempfile.TemporaryDirectory(prefix="native-terminal-install-") as directory:
        root = Path(directory)
        fetch(f"{args.base}/{args.version}/release-manifest.json", root / "manifest.json")
        fetch(f"{args.base}/{args.version}/SHA256SUMS", root / "SHA256SUMS")
        sums = dict((line.split()[1], line.split()[0]) for line in (root / "SHA256SUMS").read_text().splitlines())
        if sums.get("release-manifest.json") != digest(root / "manifest.json"):
            parser.error("Manifest checksum mismatch.")
        manifest = json.loads((root / "manifest.json").read_text())
        if (manifest.get("schema") != "openagents.native-terminal.release.v1" or
            manifest.get("version") != args.version or manifest.get("platform") != "darwin-arm64" or
            any(manifest.get(key) != "passed" for key in ("signing", "notarization", "gatekeeper"))):
            parser.error("The manifest does not describe an admitted release.")
        archive = root / "OpenAgents-Terminal.zip"
        fetch(f"{args.base}/{args.version}/{archive.name}", archive)
        if digest(archive) != manifest.get("archive_sha256") or digest(archive) != sums.get(archive.name):
            parser.error("Archive checksum mismatch.")
        unpacked = root / "unpacked"
        unpacked.mkdir()
        subprocess.run(["ditto", "-x", "-k", str(archive), str(unpacked)], check=True)
        bundle = unpacked / "OpenAgents Terminal.app"
        for name in ("openagents-terminal", "openagents", "microcoder"):
            binary = bundle / "Contents/MacOS" / name
            if digest(binary) != manifest["executables"][name]:
                parser.error(f"Executable checksum mismatch: {name}")
        subprocess.run(["codesign", "--verify", "--deep", "--strict", str(bundle)], check=True)
        subprocess.run(["spctl", "--assess", "--type", "execute", str(bundle)], check=True)
        destination.mkdir(parents=True)
        shutil.move(str(bundle), destination)
        bundle = destination / "OpenAgents Terminal.app"
        (destination / "release-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
        print(bundle)


if __name__ == "__main__":
    main()
