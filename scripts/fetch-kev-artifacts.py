"""Acquire immutable Kev bundles without changing historical golden fixtures."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import urllib.request
import venv


WORKSPACE = Path(__file__).resolve().parent.parent
HUB = "https://huggingface.co"
CONVERT = """
import json, sys, torch
from pathlib import Path
from safetensors.torch import save_file
source, destination = map(Path, sys.argv[1:])
meta = torch.load(source, map_location='cpu', weights_only=True)
save_file(meta['head'], str(destination / 'head.safetensors'))
(destination / 'head_meta.json').write_text(json.dumps(
    {k: v for k, v in meta.items() if k != 'head'}, indent=2))
"""


def digest(path):
    h = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(8 << 20), b""):
            h.update(chunk)
    return {"sha256": h.hexdigest(), "bytes": path.stat().st_size}


def verify(path, expected):
    if digest(path) != expected:
        raise ValueError(f"Digest or size mismatch: {path}; move it aside before retrying")


def component(value):
    if not isinstance(value, str) or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]*", value):
        raise ValueError(f"Invalid artifact path component: {value!r}")


def validate(lock):
    if lock.get("schema_version") != 1:
        raise ValueError("Unsupported artifact lock schema")
    component(lock["variant"])
    for part in ("adapter", "base"):
        spec = lock[part]
        component(spec["directory"])
        if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", spec["repo"]):
            raise ValueError("Invalid Hub repository")
        if not re.fullmatch(r"[0-9a-f]{40}", spec["revision"]):
            raise ValueError("A full immutable Hub revision is required")
    for files in (lock["adapter"]["files"], lock["base"]["files"], lock["derived_files"]):
        if not files:
            raise ValueError("Artifact file lists must not be empty")
        for name, expected in files.items():
            component(name)
            if set(expected) != {"sha256", "bytes"} or not re.fullmatch(r"[0-9a-f]{64}", expected["sha256"]):
                raise ValueError(f"Invalid digest record: {name}")
            if not isinstance(expected["bytes"], int) or expected["bytes"] <= 0:
                raise ValueError(f"Invalid size: {name}")
    if "head.pt" not in lock["adapter"]["files"]:
        raise ValueError("The source head must be pinned")
    if set(lock["derived_files"]) != {"head.safetensors", "head_meta.json"}:
        raise ValueError("Both converted head files must be pinned")
    if lock["adapter"]["files"].keys() & lock["derived_files"].keys():
        raise ValueError("Source and derived files must be distinct")
    if "config.json" not in lock["base"]["files"] or not any(
        name.startswith("model") and name.endswith(".safetensors")
        for name in lock["base"]["files"]
    ):
        raise ValueError("Base config and weights must be pinned")


def fetch(spec, directory, name, expected, hub):
    destination = directory / name
    if destination.exists():
        # The preflight verified every existing file before any download.
        return
    partial = destination.with_name(name + ".part")
    url = f"{hub}/{spec['repo']}/resolve/{spec['revision']}/{name}"
    print(f"  Fetching {spec['repo']}@{spec['revision']}/{name}", flush=True)
    try:
        # Discard an interrupted transfer; never accept or append unchecked bytes.
        with urllib.request.urlopen(url, timeout=120) as response, partial.open("wb") as out:
            shutil.copyfileobj(response, out, length=8 << 20)
        verify(partial, expected)
        partial.replace(destination)
    finally:
        partial.unlink(missing_ok=True)


def convert(source, destination, root):
    python = os.environ.get("KEV_CONVERTER_PYTHON")
    if not python:
        directory = root / ".convert-venv"
        python = str(directory / "bin/python")
        if not Path(python).exists():
            venv.create(directory, with_pip=True)
        if subprocess.run([python, "-c", "import torch, safetensors"], capture_output=True).returncode:
            subprocess.run([python, "-m", "pip", "install", "torch==2.8.0", "safetensors==0.8.0"], check=True)
    subprocess.run([python, "-c", CONVERT, str(source), str(destination)], check=True)


def fetch_lock(lock, root, *, hub=HUB, converter=convert):
    validate(lock)
    adapter = root / lock["adapter"]["directory"]
    base = root / lock["base"]["directory"]
    # Check warm directories before conversion, network access, or base downloads.
    for directory, files in (
        (adapter, lock["adapter"]["files"]),
        (adapter, lock["derived_files"]),
        (base, lock["base"]["files"]),
    ):
        for name, expected in files.items():
            path = directory / name
            if path.exists():
                verify(path, expected)
    if base.exists():
        unexpected = {p.name for p in base.glob("model*.safetensors")} - lock["base"]["files"].keys()
        if unexpected:
            raise ValueError(f"Unpinned base weights in {base}: {sorted(unexpected)}")
    adapter.mkdir(parents=True, exist_ok=True)
    base.mkdir(parents=True, exist_ok=True)
    for name, expected in lock["adapter"]["files"].items():
        fetch(lock["adapter"], adapter, name, expected, hub)
    if any(not (adapter / name).exists() for name in lock["derived_files"]):
        with tempfile.TemporaryDirectory(prefix=".head-", dir=adapter) as temporary:
            destination = Path(temporary)
            converter(adapter / "head.pt", destination, root)
            # Validate both outputs before publishing either one.
            for name, expected in lock["derived_files"].items():
                verify(destination / name, expected)
            for name in lock["derived_files"]:
                if not (adapter / name).exists():
                    (destination / name).replace(adapter / name)
    metadata = json.loads((adapter / "head_meta.json").read_text())
    if metadata.get("base") != lock["base"]["repo"]:
        raise ValueError("Head metadata and pinned base repository disagree")
    if metadata.get("base_revision") and metadata["base_revision"] != lock["base"]["revision"]:
        raise ValueError("Head metadata and pinned base revision disagree")
    for name, expected in lock["base"]["files"].items():
        fetch(lock["base"], base, name, expected, hub)
    # A receipt is written only after the complete bundle passes verification.
    receipt = adapter / "artifact-lock.json"
    temporary = receipt.with_suffix(".json.part")
    temporary.write_text(json.dumps(lock, indent=2) + "\n")
    temporary.replace(receipt)
    identity = hashlib.sha256(json.dumps(lock, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    print(f"{lock['variant']}: verified artifact lock {identity}", flush=True)
    return identity


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("variants", nargs="*", help="Historical fixture variants; default: kev-0.5b")
    parser.add_argument("--lock", type=Path, help="A separately pinned candidate artifact lock")
    parser.add_argument("--root", type=Path, default=Path(os.environ.get("KEV_ARTIFACTS", WORKSPACE.parent / "kev-artifacts")))
    args = parser.parse_args()
    if args.lock and args.variants:
        parser.error("Use --lock or variant names, not both")
    fixtures = WORKSPACE / "crates/kev/fixtures"
    paths = [args.lock] if args.lock else [
        (fixtures if name == "kev-0.5b" else fixtures / "variants" / name) / "artifact-lock.json"
        for name in args.variants or ["kev-0.5b"]
    ]
    for path in paths:
        lock = json.loads(path.read_text())
        fetch_lock(lock, args.root)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        print(f"kev artifacts: {error}", file=sys.stderr)
        sys.exit(1)
