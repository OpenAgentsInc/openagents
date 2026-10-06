#!/usr/bin/env python3
"""Package qualified standalone Linux binaries without installing a host service."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import tempfile
import urllib.request

NAMES = ("openagents-terminal", "openagents", "microcoder")
CHECKS = ("isolated_install", "startup_input", "request_proposal_result", "clipboard_unicode_ime",
          "resize_fullscreen", "output_frame_workload", "uninstall_cleanup")


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def qualification(path, commit):
    record = json.loads(path.read_text())
    if (record.get("schema") != "openagents.native-terminal.linux-qualification.v1" or
        record.get("commit") != commit or record.get("platform") != "linux-x86_64" or
        not record.get("distribution") or record.get("backend") not in ("wayland", "x11") or
        any(record.get("checks", {}).get(name) != "passed" for name in CHECKS)):
        raise ValueError("A matching Linux qualification with every required check is needed.")
    if set(record.get("executables", {})) != set(NAMES):
        raise ValueError("Qualification must identify all three tested executables.")
    return record


def package(args):
    commit = subprocess.check_output(["git", "rev-parse", "origin/main"], text=True).strip()
    record = qualification(args.qualification, commit)
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+(?:-rc\.[1-9][0-9]*)?", args.version):
        raise ValueError("Invalid version.")
    # No cross-target or distribution coverage is inferred from one successful build.
    for name in NAMES:
        binary = args.binaries / name
        if digest(binary) != record["executables"][name]:
            raise ValueError(f"Tested executable changed: {name}")
        identity = subprocess.check_output(["file", str(binary)], text=True)
        if "ELF 64-bit" not in identity or "x86-64" not in identity:
            raise ValueError(f"Expected a Linux x86-64 executable: {name}")
    stage = args.out / args.version / "linux-x86_64"
    if stage.exists():
        raise ValueError("Refusing to replace an existing staged version.")
    stage.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=stage.parent) as directory:
        pending = Path(directory)
        bundle = pending / "openagents-terminal"
        bundle.mkdir()
        for name in NAMES:
            shutil.copy2(args.binaries / name, bundle / name)
        archive = pending / "OpenAgents-Terminal-linux-x86_64.tar.gz"
        with tarfile.open(archive, "w:gz") as tar:
            tar.add(bundle, arcname=bundle.name)
        shutil.copy2(args.qualification, pending / "qualification.json")
        installer = Path(__file__).with_name("install-linux-terminal.py")
        shutil.copy2(installer, pending / installer.name)
        manifest = {"schema": "openagents.native-terminal.release.v1", "prefix": "openagents-terminal",
                    "platform": "linux-x86_64", "version": args.version, "commit": commit,
                    "distribution": record["distribution"], "backend": record["backend"],
                    "executables": record["executables"], "archive": archive.name,
                    "archive_sha256": digest(archive), "qualification_sha256": digest(args.qualification),
                    "installer_sha256": digest(installer), "public_readback": "not-run"}
        (pending / "release-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
        files = (archive.name, "release-manifest.json", "qualification.json", installer.name)
        (pending / "SHA256SUMS").write_text("".join(f"{digest(pending / name)}  {name}\n" for name in files))
        shutil.rmtree(bundle)
        pending.rename(stage)
    print(stage)


def publish(args):
    stage = args.stage
    manifest = json.loads((stage / "release-manifest.json").read_text())
    record = qualification(stage / "qualification.json", manifest["commit"])
    if any(record.get(name) != manifest.get(name) for name in ("platform", "distribution", "backend", "executables")):
        raise ValueError("Qualification does not match the release manifest.")
    if manifest.get("platform") != "linux-x86_64" or manifest.get("prefix") != "openagents-terminal":
        raise ValueError("Invalid Linux release manifest.")
    files = ("OpenAgents-Terminal-linux-x86_64.tar.gz", "release-manifest.json",
             "qualification.json", "install-linux-terminal.py")
    expected = "".join(f"{digest(stage / name)}  {name}\n" for name in files)
    if (stage / "SHA256SUMS").read_text() != expected:
        raise ValueError("Release checksum file changed.")
    for name, field in ((files[0], "archive_sha256"), (files[2], "qualification_sha256"),
                        (files[3], "installer_sha256")):
        if digest(stage / name) != manifest.get(field):
            raise ValueError(f"Release object changed: {name}")
    if not re.fullmatch(r"[a-z0-9][a-z0-9._-]+", args.bucket):
        raise ValueError("Invalid bucket name.")
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+(?:-rc\.[1-9][0-9]*)?", manifest["version"]):
        raise ValueError("Invalid version.")
    prefix = f"gs://{args.bucket}/openagents-terminal/{manifest['version']}/linux-x86_64"
    objects = (*files, "SHA256SUMS")
    # Conditional creates never overwrite the existing desktop or macOS channel.
    for name in objects:
        subprocess.run(["gcloud", "storage", "cp", "--if-generation-match=0",
                        str(stage / name), f"{prefix}/{name}"], check=True)
    base = f"https://storage.googleapis.com/{args.bucket}/openagents-terminal/{manifest['version']}/linux-x86_64"
    for name in objects:
        with urllib.request.urlopen(f"{base}/{name}", timeout=120) as source:
            hasher = hashlib.sha256()
            while block := source.read(1024 * 1024):
                hasher.update(block)
        if hasher.hexdigest() != digest(stage / name):
            raise ValueError(f"Public readback failed: {name}")
    receipt = {"schema": "openagents.native-terminal.publication.v1", "commit": manifest["commit"],
               "platform": "linux-x86_64", "version": manifest["version"], "base": base,
               "public_readback": "passed", "objects": {name: digest(stage / name) for name in objects}}
    (stage / "publication-receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print("Linux package readback passed.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    command = commands.add_parser("package")
    command.add_argument("--binaries", type=Path, required=True)
    command.add_argument("--qualification", type=Path, required=True)
    command.add_argument("--version", required=True)
    command.add_argument("--out", type=Path, required=True)
    command = commands.add_parser("publish")
    command.add_argument("--stage", type=Path, required=True)
    command.add_argument("--bucket", default="openagentsgemini-cli-releases")
    args = parser.parse_args()
    {"package": package, "publish": publish}[args.command](args)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        raise SystemExit(str(error)) from error
