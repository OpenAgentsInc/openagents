"""Derive task catalog entries from an upstream checkout's ``task.toml`` files.

The checked catalog in ``profiles/tasks.json`` records what each task
declares: its resources, its timeouts, whether its verifier runs in a
separate environment, and the base image its Dockerfile starts from. This
module reads those declarations so the catalog can be regenerated from a
pinned checkout, and so a test can tell when the checked entries drift from
the checkout they claim to describe.
"""

from __future__ import annotations

import re
import tomllib
from pathlib import Path
from typing import Any

_FROM = re.compile(r"^\s*FROM\s+(?:--platform=\S+\s+)?(\S+)")


def _resources(section: dict[str, Any]) -> dict[str, int]:
    return {
        "cpus": int(section.get("cpus", 1)),
        "memory_mb": int(section.get("memory_mb", 2048)),
        "storage_mb": int(section.get("storage_mb", 10240)),
        "gpus": int(section.get("gpus", 0)),
    }


def base_image(dockerfile: Path) -> str | None:
    """The image the Dockerfile's final stage starts from.

    The final stage is the one Harbor runs, so its base decides the C
    library and the tools a prebuilt agent install finds.
    """
    if not dockerfile.is_file():
        return None
    image = None
    for line in dockerfile.read_text(errors="replace").splitlines():
        match = _FROM.match(line)
        if match:
            image = match.group(1)
    return image


def entry_from_task_dir(
    task_dir: Path, *, relative_path: str, profiles: list[str]
) -> dict[str, Any]:
    """One catalog entry for the task at ``task_dir``."""
    config = tomllib.loads((task_dir / "task.toml").read_text())
    metadata = config.get("metadata") or {}
    environment = config.get("environment") or {}
    verifier = config.get("verifier") or {}
    agent = config.get("agent") or {}
    resources = _resources(environment)
    entry: dict[str, Any] = {
        "id": task_dir.name,
        "path": relative_path,
        "set": relative_path.split("/", 1)[0],
        "role": " / ".join(
            part
            for part in (metadata.get("category"), metadata.get("subcategory"))
            if part
        ),
        "resources": resources,
        "agent_timeout_sec": int(float(agent.get("timeout_sec", 900))),
        "build_timeout_sec": int(float(environment.get("build_timeout_sec", 600))),
        "verifier_timeout_sec": int(float(verifier.get("timeout_sec", 600))),
        "images": [],
        "image_arches": [],
        "base_image": base_image(task_dir / "environment" / "Dockerfile"),
        "compose": (task_dir / "environment" / "docker-compose.yaml").is_file(),
        "profiles": profiles,
    }
    if verifier.get("environment_mode") == "separate" and isinstance(
        verifier.get("environment"), dict
    ):
        entry["verifier_resources"] = _resources(verifier["environment"])
    if resources["gpus"] > 0:
        entry["requires_gpu_runtime"] = True
        entry["gpu_types"] = list(environment.get("gpu_types") or [])
    if "expert_time_estimate_hours" in metadata:
        entry["expert_hours"] = float(metadata["expert_time_estimate_hours"])
    return entry


def entries_from_checkout(
    checkout: Path, *, task_set: str = "tasks", profiles: list[str]
) -> list[dict[str, Any]]:
    """Every task under ``checkout/<task_set>``, sorted by id."""
    root = checkout / task_set
    return [
        entry_from_task_dir(
            task_dir, relative_path=f"{task_set}/{task_dir.name}", profiles=profiles
        )
        for task_dir in sorted(root.iterdir())
        if (task_dir / "task.toml").is_file()
    ]
