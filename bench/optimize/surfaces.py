"""Seed candidate = staged text surfaces from OpenAgentsInc/openagents#122.

Reads `surfaces/coder/*.v1.json`. Does not write them. Applying a candidate
to those files is a landing change and belongs to a later reviewed cycle
(ledger O1), not this lane.
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

SURFACE_IDS = ("system-prompt", "tool-descriptions", "catalog-lines")


def repo_root_from(start: Path | None = None) -> Path:
    here = Path(start or __file__).resolve()
    for parent in [here, *here.parents]:
        if (parent / "surfaces" / "coder" / "index.json").is_file() and (
            parent / "bench" / "run-suite.sh"
        ).is_file():
            return parent
    raise FileNotFoundError("could not find repository root (surfaces/coder/index.json)")


def surfaces_dir(root: Path | None = None) -> Path:
    return (root or repo_root_from()) / "surfaces" / "coder"


def surface_filename(surface: str) -> str:
    return f"{surface}.v1.json"


def load_seed_candidate(root: Path | None = None) -> dict[str, str]:
    """GEPA seed: component name → current artifact file text."""
    directory = surfaces_dir(root)
    seed: dict[str, str] = {}
    for surface in SURFACE_IDS:
        path = directory / surface_filename(surface)
        if not path.is_file():
            raise FileNotFoundError(f"staged surface missing: {path}")
        seed[surface] = path.read_text()
    return seed


def load_surface_index(root: Path | None = None) -> dict[str, Any]:
    path = surfaces_dir(root) / "index.json"
    return json.loads(path.read_text())


def load_suite_task_ids(suite_path: Path) -> list[str]:
    payload = json.loads(Path(suite_path).read_text())
    tasks = payload.get("tasks") or []
    ids: list[str] = []
    for task in tasks:
        if isinstance(task, dict) and task.get("id"):
            ids.append(str(task["id"]))
        elif isinstance(task, str):
            ids.append(task)
    return ids
