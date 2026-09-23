"""The task panel: which upstream tasks this benchmark runs and on what terms.

``profiles/tasks.json`` is the checked record. It pins the upstream commit,
each task's repository-relative path, its declared resources and agent
timeout, the images it is known to pull, and the architecture those images
publish. A profile never widens a task's allowance; it names the profile
that changed it instead.
"""

from __future__ import annotations

import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from . import paths

TASKS_SCHEMA = "openagents.tbench.tasks.v1"


@dataclass(frozen=True)
class TaskResources:
    cpus: int
    memory_mb: int
    storage_mb: int
    gpus: int


@dataclass(frozen=True)
class Task:
    """One upstream task as the panel declares it."""

    id: str
    path: str  # repository-relative, e.g. "archive/fix-git"
    set: str  # "archive" or "tasks"
    role: str
    resources: TaskResources
    agent_timeout_sec: int
    images: tuple[str, ...] = ()
    image_arches: tuple[str, ...] = ()
    profiles: tuple[str, ...] = ()
    excluded: bool = False
    exclude_reason: str | None = None
    notes: tuple[str, ...] = ()
    # A GPU task runs only where Docker has an NVIDIA runtime; the suite
    # skips it elsewhere and records why, rather than running it on a CPU.
    requires_gpu_runtime: bool = False
    gpu_types: tuple[str, ...] = ()
    # A verifier that runs in its own environment declares its own budget.
    verifier_resources: TaskResources | None = None
    build_timeout_sec: int | None = None
    base_image: str | None = None

    @property
    def peak_resources(self) -> TaskResources:
        """The larger of the agent and verifier environments' budgets.

        The suite scheduler reserves this much for a trial, so a verifier
        that starts while the agent environment is still up stays inside
        the host budget.
        """
        verifier = self.verifier_resources
        if verifier is None:
            return self.resources
        return TaskResources(
            cpus=max(self.resources.cpus, verifier.cpus),
            memory_mb=max(self.resources.memory_mb, verifier.memory_mb),
            storage_mb=max(self.resources.storage_mb, verifier.storage_mb),
            gpus=max(self.resources.gpus, verifier.gpus),
        )

    @property
    def excluded_reason_text(self) -> str:
        return self.exclude_reason or "excluded"


@dataclass(frozen=True)
class Panel:
    """The whole panel plus its upstream pin."""

    git_url: str
    git_commit_id: str
    tasks: tuple[Task, ...]
    source_path: Path | None = None
    extra: dict[str, Any] = field(default_factory=dict)
    # ``None`` for the panel's own pin; a catalog name such as ``tb4`` for a
    # second pinned upstream ref with its own checkout.
    catalog: str | None = None
    ref: str | None = None
    checkout_dir: str | None = None

    def checkout(self) -> Path:
        """Where this panel's pinned upstream checkout lives in the cache."""
        return paths.upstream_checkout(self.checkout_dir)

    def task(self, task_id: str) -> Task:
        for task in self.tasks:
            if task.id == task_id:
                return task
        known = ", ".join(t.id for t in self.tasks)
        raise KeyError(f"no task {task_id!r} in the panel (known: {known})")

    def select(self, ids: list[str] | tuple[str, ...]) -> list[Task]:
        """Resolve ids to tasks, refusing excluded ones rather than dropping
        or silently running them."""
        selected = []
        for task_id in ids:
            task = self.task(task_id)
            if task.excluded:
                raise ValueError(
                    f"task {task_id!r} is excluded: "
                    f"{task.excluded_reason_text}"
                )
            selected.append(task)
        return selected

    def runnable(self, ids: list[str] | tuple[str, ...]) -> tuple[list[Task], list[Task]]:
        """Split ids into runnable tasks and excluded tasks (kept visible)."""
        runnable: list[Task] = []
        excluded: list[Task] = []
        for task_id in ids:
            task = self.task(task_id)
            (excluded if task.excluded else runnable).append(task)
        return runnable, excluded


def _resources(res: dict[str, Any]) -> TaskResources:
    return TaskResources(
        cpus=int(res.get("cpus", 1)),
        memory_mb=int(res.get("memory_mb", 2048)),
        storage_mb=int(res.get("storage_mb", 10240)),
        gpus=int(res.get("gpus", 0)),
    )


def _load_task(raw: dict[str, Any]) -> Task:
    notes = [
        raw[key]
        for key in ("arch_note", "verifier_note", "gpu_note", "note")
        if raw.get(key)
    ]
    return Task(
        id=raw["id"],
        path=raw["path"],
        set=raw.get("set", raw["path"].split("/", 1)[0]),
        role=raw.get("role", ""),
        resources=_resources(raw.get("resources") or {}),
        agent_timeout_sec=int(raw.get("agent_timeout_sec", 900)),
        images=tuple(raw.get("images") or ()),
        image_arches=tuple(raw.get("image_arches") or ()),
        profiles=tuple(raw.get("profiles") or ()),
        excluded=bool(raw.get("excluded", False)),
        exclude_reason=raw.get("exclude_reason"),
        notes=tuple(notes),
        requires_gpu_runtime=bool(raw.get("requires_gpu_runtime", False)),
        gpu_types=tuple(raw.get("gpu_types") or ()),
        verifier_resources=(
            _resources(raw["verifier_resources"])
            if raw.get("verifier_resources")
            else None
        ),
        build_timeout_sec=(
            int(raw["build_timeout_sec"]) if raw.get("build_timeout_sec") else None
        ),
        base_image=raw.get("base_image"),
    )


def load_panel(path: Path | None = None, catalog: str | None = None) -> Panel:
    """Read the checked task panel, or one of its named catalogs.

    With ``catalog`` unset this is the panel's own pin and task list. A
    catalog, such as ``tb4``, is a second upstream ref with its own pin,
    checkout directory, and tasks; its task ids may repeat the panel's,
    because the same task name at another ref is another task.
    """
    path = path or (paths.PROFILES_DIR / "tasks.json")
    data = json.loads(path.read_text())
    if data.get("schema_version") != TASKS_SCHEMA:
        raise ValueError(
            f"{path}: schema_version {data.get('schema_version')!r} "
            f"is not {TASKS_SCHEMA!r}"
        )
    section = data
    if catalog is not None:
        catalogs = data.get("catalogs") or {}
        if catalog not in catalogs:
            known = ", ".join(sorted(catalogs)) or "none"
            raise KeyError(f"no task catalog {catalog!r} (known: {known})")
        section = catalogs[catalog]
    upstream = section["upstream"]
    tasks = tuple(_load_task(raw) for raw in section["tasks"])
    ids = [t.id for t in tasks]
    if len(ids) != len(set(ids)):
        raise ValueError(f"{path}: duplicate task ids in panel")
    reserved = ("git_url", "git_commit_id", "ref", "checkout_dir")
    return Panel(
        git_url=upstream["git_url"],
        git_commit_id=upstream["git_commit_id"],
        tasks=tasks,
        source_path=path,
        extra={k: v for k, v in upstream.items() if k not in reserved},
        catalog=catalog,
        ref=upstream.get("ref"),
        checkout_dir=upstream.get("checkout_dir"),
    )


def catalog_names(path: Path | None = None) -> list[str]:
    """The named catalogs beside the panel's own pin."""
    path = path or (paths.PROFILES_DIR / "tasks.json")
    return sorted((json.loads(path.read_text()).get("catalogs") or {}).keys())
