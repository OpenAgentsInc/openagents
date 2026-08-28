"""Digest-pinned plugin catalog packing for Harbor A/B rows (#120)."""

from __future__ import annotations

import os
import tarfile
from pathlib import Path

REMOTE_PLUGINS_TAR = "/tmp/oa-plugins.tar"
REMOTE_PLUGINS_DIR = "/plugins"


def plugins_enabled() -> bool:
    """Harbor WITH-plugin A/B rows set OPENAGENTS_CODER_PLUGINS=1.

    Discovery walks up from cwd looking for `plugins/`. The task workspace
    has none, so the absent condition is the default: do not upload a
    catalog. The present condition installs the digest-pinned artifacts at
    `/plugins`, which the walk finds from any cwd.
    """
    return os.environ.get("OPENAGENTS_CODER_PLUGINS", "").strip().lower() in {
        "1",
        "true",
        "yes",
        "on",
    }


def plugins_src_dir() -> Path:
    configured = os.environ.get("OPENAGENTS_CODER_PLUGINS_DIR", "").strip()
    if configured:
        return Path(configured)
    return Path(__file__).resolve().parents[2] / "plugins"


def pack_plugin_catalog(src: Path, dest_tar: Path) -> int:
    """Write a tar of digest-pinned plugin artifacts. Returns plugin count."""
    if not src.is_dir():
        raise FileNotFoundError(f"plugin catalog missing at {src}")
    dest_tar.parent.mkdir(parents=True, exist_ok=True)
    count = 0
    with tarfile.open(dest_tar, "w") as tar:
        for child in sorted(src.iterdir()):
            if not child.is_dir() or child.name in {"pdk", "target"}:
                continue
            manifest = child / "manifest.json"
            wasm_files = sorted(child.glob("*.wasm"))
            if not manifest.is_file() or not wasm_files:
                continue
            tar.add(manifest, arcname=f"{child.name}/manifest.json")
            for wasm in wasm_files:
                tar.add(wasm, arcname=f"{child.name}/{wasm.name}")
            count += 1
    if count == 0:
        raise FileNotFoundError(f"no digest-pinned plugins under {src}")
    return count
