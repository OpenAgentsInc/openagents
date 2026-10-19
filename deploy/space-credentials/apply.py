#!/usr/bin/env python3
"""Install the reviewed credential adapter into the pinned public SPACE source."""
import argparse
import shutil
import subprocess
from pathlib import Path

PIN = "de87b436c35dac056564bdd5b2a113c9046e189b"


def standalone_manifest(text):
    """Replace workspace fields and omit the workspace's generated feature union."""
    replacements = {
        "version.workspace = true": 'version = "1.0.0"',
        "edition.workspace = true": 'edition = "2024"',
        "rust-version.workspace = true": 'rust-version = "1.97"',
        "publish.workspace = true": "publish = false",
    }
    hack_dependency = 'openagents-workspace-hack = { version = "0.1" }'
    return "\n".join(
        replacements.get(line, line) for line in text.splitlines()
        if line.strip() != hack_dependency
    ) + "\n"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    args = parser.parse_args()
    source = args.source.resolve()
    repository = Path(__file__).resolve().parents[2]
    head = subprocess.check_output(["git", "-C", str(source), "rev-parse", "HEAD"], text=True).strip()
    if head != PIN:
        raise SystemExit("The adapter requires the reviewed SPACE commit.")
    patch = Path(__file__).with_name("gateway.patch")
    subprocess.run(["git", "-C", str(source), "apply", "--check", str(patch)], check=True)
    destination = source / "crates/openagents-credentials"
    if destination.exists():
        raise SystemExit("The adapter destination already exists.")
    shutil.copytree(repository / "crates/computer-credentials", destination)
    manifest = destination / "Cargo.toml"
    manifest.write_text(standalone_manifest(manifest.read_text()))
    workspace = source / "Cargo.toml"
    contents = workspace.read_text()
    if contents.count("members = [") != 1:
        raise SystemExit("The workspace member declaration changed.")
    workspace.write_text(contents.replace("members = [", 'members = [\n    "crates/openagents-credentials",', 1))
    subprocess.run(["git", "-C", str(source), "apply", str(patch)], check=True)


if __name__ == "__main__":
    main()
