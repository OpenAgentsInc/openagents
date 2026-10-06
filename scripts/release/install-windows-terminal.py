#!/usr/bin/env python3
"""Install a verified Windows terminal package into an explicit empty directory."""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import re
import shutil
import zipfile
import tempfile
import urllib.request

NAMES = ("openagents-terminal.exe", "openagents.exe", "microcoder.exe")


def digest(path):
    hasher = hashlib.sha256()
    with path.open("rb") as source:
        while block := source.read(1024 * 1024):
            hasher.update(block)
    return hasher.hexdigest()


def fetch(url, path):
    with urllib.request.urlopen(url, timeout=120) as source, path.open("wb") as destination:
        shutil.copyfileobj(source, destination)


def install(base, version, destination):
    if platform.system() != "Windows" or platform.machine() not in ("x86_64", "AMD64"):
        raise ValueError("This package requires Windows x86-64.")
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+(?:-rc\.[1-9][0-9]*)?", version):
        raise ValueError("Invalid version.")
    if destination.exists():
        raise ValueError("The destination must not exist.")
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=destination.parent) as directory:
        root = Path(directory)
        fetch(f"{base}/{version}/windows-x86_64/SHA256SUMS", root / "SHA256SUMS")
        sums = dict((line.split()[1], line.split()[0]) for line in (root / "SHA256SUMS").read_text().splitlines())
        fetch(f"{base}/{version}/windows-x86_64/release-manifest.json", root / "release-manifest.json")
        if sums.get("release-manifest.json") != digest(root / "release-manifest.json"):
            raise ValueError("Manifest checksum mismatch.")
        manifest = json.loads((root / "release-manifest.json").read_text())
        archive_name = "OpenAgents-Terminal-windows-x86_64.zip"
        if (manifest.get("schema") != "openagents.native-terminal.release.v1" or
            manifest.get("prefix") != "openagents-terminal" or manifest.get("version") != version or
            manifest.get("platform") != "windows-x86_64" or manifest.get("archive") != archive_name or
            manifest.get("backend") != "conpty" or not manifest.get("distribution") or
            set(manifest.get("executables", {})) != set(NAMES)):
            raise ValueError("Invalid Windows terminal manifest.")
        fetch(f"{base}/{version}/windows-x86_64/qualification.json", root / "qualification.json")
        qualification = json.loads((root / "qualification.json").read_text())
        checks = ("isolated_install", "startup_input", "request_proposal_result", "clipboard_unicode_ime",
                  "resize_fullscreen", "output_frame_workload", "uninstall_cleanup",
                  "ctrl_signals", "close_reopen", "host_restart", "clipboard_refusal")
        if (digest(root / "qualification.json") != manifest.get("qualification_sha256") or
            sums.get("qualification.json") != digest(root / "qualification.json") or
            qualification.get("schema") != "openagents.native-terminal.windows-qualification.v1" or
            any(qualification.get(name) != manifest.get(name) for name in
                ("commit", "platform", "distribution", "backend", "executables", "version")) or
            any(qualification.get("checks", {}).get(name) != "passed" for name in checks)):
            raise ValueError("Qualification does not match this package.")
        fetch(f"{base}/{version}/windows-x86_64/{archive_name}", root / archive_name)
        if digest(root / archive_name) != manifest.get("archive_sha256") or sums.get(archive_name) != digest(root / archive_name):
            raise ValueError("Archive checksum mismatch.")
        unpacked = root / "unpacked"
        unpacked.mkdir()
        with zipfile.ZipFile(root / archive_name) as archive:
            members = archive.infolist()
            expected = {f"openagents-terminal/{name}" for name in NAMES}
            if {item.filename for item in members} != expected or len(members) != len(expected):
                raise ValueError("Unexpected archive entries.")
            for item in members:
                if item.is_dir() or item.file_size > 1024**3 or (item.external_attr >> 16) & 0o170000 == 0o120000:
                    raise ValueError("Invalid archive entry.")
                target = unpacked / item.filename
                target.parent.mkdir(parents=True, exist_ok=True)
                with archive.open(item) as source, target.open("wb") as destination_file:
                    shutil.copyfileobj(source, destination_file)
        bundle = unpacked / "openagents-terminal"
        for name in NAMES:
            if digest(bundle / name) != manifest["executables"][name]:
                raise ValueError(f"Executable checksum mismatch: {name}")
        shutil.copy2(root / "release-manifest.json", bundle)
        bundle.rename(destination)
    print(destination / "openagents-terminal.exe")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--destination", type=Path, required=True)
    parser.add_argument("--base", default="https://storage.googleapis.com/openagentsgemini-cli-releases/openagents-terminal")
    args = parser.parse_args()
    install(args.base, args.version, args.destination.resolve())


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError) as error:
        raise SystemExit(str(error)) from error
