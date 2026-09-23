"""Harbor's local Docker environment, with GPUs passed through by CDI.

Harbor 0.22.0's Docker environment declares no GPU support, so a task that
asks for a GPU is refused before it starts. On a host with the NVIDIA
container toolkit, Docker reads a Container Device Interface (CDI) spec
that names ``nvidia.com/gpu=all``, and a compose service can request that
device directly. ``CdiDockerEnvironment`` declares GPU support only when
such a spec exists, and for a task with ``gpus > 0`` adds the device to the
task's ``main`` service in the resource override Harbor already writes.

Select it with the environment import path
``tbench.gpu_docker:CdiDockerEnvironment``; the ``tb4`` job profile does.
A CPU task runs exactly as it does under the stock environment.
"""

from __future__ import annotations

import json
import os
from pathlib import Path
from typing import Any

from harbor.environments.capabilities import EnvironmentCapabilities
from harbor.environments.docker.docker import DockerEnvironment

# Where Docker looks for CDI specs by default; `docker info` reports the
# directories it actually reads as `CDISpecDirs`.
DEFAULT_CDI_DIRS = ("/etc/cdi", "/var/run/cdi")
CDI_DIRS_ENV = "TBENCH_CDI_DIRS"
GPU_DEVICE = "nvidia.com/gpu=all"
GPU_KIND = "nvidia.com/gpu"


def cdi_dirs(info: dict[str, Any] | None = None) -> list[Path]:
    """The CDI spec directories to search.

    ``TBENCH_CDI_DIRS`` (colon-separated) overrides everything, for tests.
    Otherwise the directories ``docker info`` reports, or Docker's defaults
    when the caller has no ``docker info`` at hand.
    """
    override = os.environ.get(CDI_DIRS_ENV)
    if override is not None:
        return [Path(part) for part in override.split(":") if part]
    if info is not None and "CDISpecDirs" in info:
        return [Path(part) for part in info.get("CDISpecDirs") or []]
    return [Path(part) for part in DEFAULT_CDI_DIRS]


def nvidia_cdi_spec(dirs: list[Path] | None = None) -> Path | None:
    """The first CDI spec that declares NVIDIA GPUs, or ``None``."""
    for directory in dirs if dirs is not None else cdi_dirs():
        if not directory.is_dir():
            continue
        for path in sorted(directory.iterdir()):
            if path.suffix not in (".json", ".yaml", ".yml") or not path.is_file():
                continue
            try:
                text = path.read_text(errors="replace")
            except OSError:
                continue
            if GPU_KIND in text:
                return path
    return None


def add_gpu_device(compose_path: Path, device: str = GPU_DEVICE) -> None:
    """Add ``device`` to ``services.main.devices`` in a JSON compose file."""
    compose = json.loads(compose_path.read_text()) if compose_path.is_file() else {}
    services = compose.setdefault("services", {})
    main = services.setdefault("main", {})
    devices = list(main.get("devices") or [])
    if device not in devices:
        devices.append(device)
    main["devices"] = devices
    compose_path.parent.mkdir(parents=True, exist_ok=True)
    compose_path.write_text(json.dumps(compose, indent=2))


class CdiDockerEnvironment(DockerEnvironment):
    """Harbor's Docker environment that can hand a task the host's GPU."""

    @property
    def capabilities(self) -> EnvironmentCapabilities:
        stock = super().capabilities
        return stock.model_copy(update={"gpus": nvidia_cdi_spec() is not None})

    def _write_resources_compose_file(self) -> Path | None:
        path = super()._write_resources_compose_file()
        if path is not None and self._effective_gpus > 0:
            add_gpu_device(path)
        return path
