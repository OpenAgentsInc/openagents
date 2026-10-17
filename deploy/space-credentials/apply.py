#!/usr/bin/env python3
"""Install the reviewed credential adapter into the pinned public SPACE source."""
import argparse
import shutil
import subprocess
from pathlib import Path

PIN = "de87b436c35dac056564bdd5b2a113c9046e189b"


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
    replacements = {
        "version.workspace = true": 'version = "1.0.0"',
        "edition.workspace = true": 'edition = "2024"',
        "rust-version.workspace = true": 'rust-version = "1.97"',
        "publish.workspace = true": "publish = false",
    }
    contents = "\n".join(replacements.get(line, line) for line in manifest.read_text().splitlines()) + "\n"
    manifest.write_text(contents)
    workspace = source / "Cargo.toml"
    contents = workspace.read_text()
    if contents.count("members = [") != 1:
        raise SystemExit("The workspace member declaration changed.")
    workspace.write_text(contents.replace("members = [", 'members = [\n    "crates/openagents-credentials",', 1))
    subprocess.run(["git", "-C", str(source), "apply", str(patch)], check=True)


if __name__ == "__main__":
    main()
