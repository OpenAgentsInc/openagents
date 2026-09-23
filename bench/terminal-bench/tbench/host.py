"""What the local Docker host can give a trial: GPU runtime, disk, images.

The suite scheduler asks these questions before it starts a trial and after
a task's trials finish. Each helper shells out to ``docker`` and returns a
plain answer, so the scheduler's tests replace them with fakes.
"""

from __future__ import annotations

import json
import shutil
import subprocess
from pathlib import Path

# The Docker runtimes that pass a GPU through to a container.
GPU_RUNTIMES = ("nvidia",)

DEFAULT_DOCKER_ROOT = Path("/var/lib/docker")


def _docker(args: list[str], timeout: int = 30) -> tuple[int, str]:
    try:
        out = subprocess.run(
            ["docker", *args], capture_output=True, text=True, timeout=timeout
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        return 127, str(exc)
    return out.returncode, (out.stdout or "") + (out.stderr or "")


def docker_info() -> dict:
    """``docker info`` as a mapping, or empty when Docker doesn't answer."""
    code, text = _docker(["info", "--format", "{{json .}}"])
    if code != 0:
        return {}
    try:
        return json.loads(text.strip().splitlines()[-1])
    except (json.JSONDecodeError, IndexError):
        return {}


def gpu_runtime(info: dict | None = None) -> str | None:
    """How Docker can hand a container a GPU here, or ``None``.

    Either an NVIDIA runtime in ``docker info``'s runtimes, or a Container
    Device Interface (CDI) spec for ``nvidia.com/gpu`` in a directory Docker
    reads, which the NVIDIA container toolkit writes and
    ``tbench.gpu_docker:CdiDockerEnvironment`` requests by device name.
    """
    from .gpu_docker import cdi_dirs, nvidia_cdi_spec

    info = docker_info() if info is None else info
    spec = nvidia_cdi_spec(cdi_dirs(info))
    if spec is not None:
        return f"cdi:{spec}"
    runtimes = info.get("Runtimes") or {}
    for name in GPU_RUNTIMES:
        if name in runtimes:
            return name
    return None


def gpu_refusal(task_id: str, info: dict | None = None) -> str | None:
    """Why a GPU task can't run here, or ``None`` when it can."""
    if gpu_runtime(info) is not None:
        return None
    return (
        f"{task_id} needs a GPU, and Docker on this host offers none: no "
        "NVIDIA CDI spec (nvidia.com/gpu) in its CDI directories and no "
        "NVIDIA runtime. Enable the NVIDIA container toolkit to run it; the "
        "harness never substitutes a CPU runtime."
    )


def docker_root(info: dict | None = None) -> Path:
    info = docker_info() if info is None else info
    root = info.get("DockerRootDir")
    return Path(root) if root else DEFAULT_DOCKER_ROOT


def free_disk_gb(path: Path) -> float:
    """Free space on the volume that holds ``path``, in GiB."""
    probe = path
    while not probe.exists() and probe != probe.parent:
        probe = probe.parent
    return shutil.disk_usage(probe).free / 2**30


def task_images(task_id: str) -> list[str]:
    """The images Harbor built for one task's trials that still exist.

    Harbor 0.22.0 builds each trial's environment as the compose project's
    untagged ``main`` image, named ``<trial name>__env-main`` (and
    ``<trial name>__verifier-…`` for a separate verifier), and
    ``docker compose down --rmi local`` removes it when the trial ends. An
    image that outlives its trial is a leftover from a crash or a trial
    still running. The trial name starts with the task id.
    """
    code, text = _docker(["images", "--format", "{{.Repository}}:{{.Tag}}"])
    if code != 0:
        return []
    prefix = f"{task_id.lower()}__"
    return sorted(
        line.strip()
        for line in text.splitlines()
        if line.strip().startswith(prefix)
    )


def remove_images(names: list[str]) -> list[str]:
    """Remove the named images; return the ones Docker removed."""
    removed = []
    for name in names:
        code, _ = _docker(["image", "rm", name], timeout=120)
        if code == 0:
            removed.append(name)
    return removed


def prune_build_cache() -> bool:
    """Drop Docker's unused build cache; in-progress builds keep theirs."""
    code, _ = _docker(["builder", "prune", "--force"], timeout=600)
    return code == 0
