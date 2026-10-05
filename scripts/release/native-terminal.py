#!/usr/bin/env python3
"""Build the standalone macOS app and both helpers from one source commit."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import plistlib
import shutil
import subprocess
import tarfile
import tempfile
import tomllib

REPO = Path(__file__).resolve().parents[2]
PREFIX = "openagents-terminal"
APP = "OpenAgents Terminal.app"


def run(*args, **kwargs):
    return subprocess.run(args, check=True, **kwargs)


def output(*args):
    return subprocess.check_output(args, cwd=REPO, text=True).strip()


def digest(path):
    h = hashlib.sha256()
    with path.open("rb") as file:
        for block in iter(lambda: file.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def build(args):
    if platform.system() != "Darwin" or platform.machine() != "arm64":
        raise ValueError("The native release supports macOS arm64; build on that platform.")
    commit = output("git", "rev-parse", f"{args.commit}^{{commit}}")
    tree = output("git", "rev-parse", f"{commit}^{{tree}}")
    target = os.environ.get("CARGO_TARGET_DIR")
    if not target or not Path(target).is_absolute():
        raise ValueError("Set CARGO_TARGET_DIR to an absolute, reusable directory outside the checkout.")
    destination = Path(args.out).resolve()
    destination.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="native-terminal-source-") as directory:
        source = Path(directory)
        archive = source / "source.tar"
        with archive.open("wb") as file:
            run("git", "archive", commit, cwd=REPO, stdout=file)
        with tarfile.open(archive) as file:
            file.extractall(source, filter="data")
        archive.unlink()
        versions = [tomllib.loads((source / f"crates/{crate}/Cargo.toml").read_text())["package"]["version"]
                    for crate in ("terminal-app", "openagents-cli", "microcoder")]
        if len(set(versions)) != 1:
            raise ValueError("App and helper versions must match.")
        version = versions[0]
        stage = destination / version
        if stage.exists():
            raise ValueError(f"Refusing to replace {stage}.")
        stage.mkdir()
        bundle = stage / APP
        contents = bundle / "Contents"
        binaries = contents / "MacOS"
        binaries.mkdir(parents=True)
        env = dict(os.environ, OPENAGENTS_BUILD_COMMIT=commit)
        # Separate graphs keep the CLI's Verse features out of the GUI executable.
        run("cargo", "build", "--locked", "--release", "-p", "terminal-app", cwd=source, env=env)
        shutil.copy2(Path(target) / "release/openagents-terminal", binaries)
        run("cargo", "build", "--locked", "--release", "-p", "openagents-cli", "-p", "microcoder", cwd=source, env=env)
        for name in ("openagents", "microcoder"):
            shutil.copy2(Path(target) / f"release/{name}", binaries)
        for binary in binaries.iterdir():
            identity = subprocess.check_output(["file", str(binary)], text=True)
            if "Mach-O" not in identity or "arm64" not in identity:
                raise ValueError(f"Wrong executable architecture: {binary.name}")
        with (contents / "Info.plist").open("wb") as file:
            plistlib.dump({"CFBundleIdentifier": "com.openagents.terminal", "CFBundleName": "OpenAgents Terminal",
                          "CFBundleExecutable": "openagents-terminal", "CFBundlePackageType": "APPL",
                          "CFBundleShortVersionString": version.split("-")[0], "CFBundleVersion": version.replace("-rc.", "."),
                          "NSHighResolutionCapable": True, "LSMinimumSystemVersion": "13.0"}, file)
        manifest = {"schema": "openagents.native-terminal.release.v1", "version": version,
                    "commit": commit, "tree": tree, "prefix": PREFIX, "platform": "darwin-arm64",
                    "toolchain": output("rustc", "--version"), "bundle": APP,
                    "executables": {name: digest(binaries / name) for name in ("openagents-terminal", "openagents", "microcoder")},
                    "signing": "not-run", "notarization": "not-run", "gatekeeper": "not-run", "public_readback": "not-run"}
        (stage / "release-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
        print(stage)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    command = commands.add_parser("build")
    command.add_argument("--commit", default="origin/main")
    command.add_argument("--out", default=str(REPO / "dist/releases/openagents-terminal"))
    args = parser.parse_args()
    build(args)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, subprocess.CalledProcessError) as error:
        raise SystemExit(str(error)) from error
