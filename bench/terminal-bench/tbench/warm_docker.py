"""Harbor's Docker environment with task images kept between trials.

Harbor 0.22.0 builds a trial's ``main`` image as the compose project's
untagged image and removes it with ``docker compose down --rmi local``
when the trial ends, so the next trial of the same task builds it again.
Docker's build cache makes that rebuild faster, but the suite prunes the
cache under disk pressure, and a pruned cache costs minutes per task.

``WarmDockerEnvironment`` names every image a task builds with a tag
derived from Harbor's own content hash of the build context:

    tbench-warm/<task>:<role>-<hash>          the main service
    tbench-warm/<task>-<service>:<role>-<hash> a service the task's compose builds

``<role>`` is the build context's directory name: ``environment`` for the
agent's environment and ``tests`` for a separate verifier's. When every
image a task builds is already kept, the environment starts from them
through Harbor's prebuilt-image path and builds nothing. Otherwise Harbor
builds as usual, and once the environment is up each built image gets its
kept tag. Compose 5 removes every image it built at
``down --rmi local``, even one named by an ``image`` field, but that only
untags the name compose gave it; the kept tag holds the image. A changed
Dockerfile or context changes the hash, so a stale image is never reused.

Each start appends one line to ``tbench-environment.jsonl`` in the trial
directory: the role, the tag, whether the image was warm or cold, and the
milliseconds the start took. ``tbench images list`` shows the kept images
and ``tbench images prune`` removes them.

Select it with the environment import path
``tbench.warm_docker:WarmDockerEnvironment``. It extends
``CdiDockerEnvironment``, so GPU tasks work as they do under the ``tb4``
profile.
"""

from __future__ import annotations

import json
import re
import subprocess
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable

import yaml

from tbench.gpu_docker import CdiDockerEnvironment

WARM_REPOSITORY = "tbench-warm"
RECORD_NAME = "tbench-environment.jsonl"
RECORD_SCHEMA = "openagents.tbench.environment-start.v1"
# Characters of Harbor's content hash a tag keeps.
HASH_CHARS = 20


def _component(text: str) -> str:
    """One lowercase path component Docker accepts in a repository name."""
    text = re.sub(r"[^a-z0-9._-]+", "-", text.lower()).strip("._-")
    return text or "task"


def warm_tag(environment_name: str, role: str, content_id: str, service: str | None = None) -> str:
    """The kept image's reference for one service of one build context."""
    repository = _component(environment_name)
    if service and service != "main":
        repository = f"{repository}-{_component(service)}"
    return f"{WARM_REPOSITORY}/{repository}:{_component(role)}-{content_id[:HASH_CHARS]}"


def built_services(compose_path: Path) -> list[str]:
    """The services a task's own compose file builds, other than ``main``."""
    if not compose_path.is_file():
        return []
    try:
        compose = yaml.safe_load(compose_path.read_text()) or {}
    except (OSError, yaml.YAMLError):
        return []
    services = compose.get("services") or {}
    return sorted(
        name
        for name, body in services.items()
        if name != "main" and isinstance(body, dict) and body.get("build")
    )


def image_cache(
    task_image: str | None,
    tags: dict[str, str],
    force_build: bool,
    exists: Callable[[str], bool],
) -> str:
    """How an environment starts: from the task's own pinned image
    (``task-image``), from kept images (``warm``), or by building
    (``cold``). Warm needs every built service's image kept."""
    if task_image:
        return "task-image"
    if not force_build and all(exists(tag) for tag in tags.values()):
        return "warm"
    return "cold"


def image_exists(reference: str) -> bool:
    try:
        found = subprocess.run(
            ["docker", "image", "inspect", reference],
            capture_output=True,
            timeout=30,
        )
    except (OSError, subprocess.TimeoutExpired):
        return False
    return found.returncode == 0


def warm_images() -> list[dict[str, Any]]:
    """Every kept image: reference, size, and creation time."""
    try:
        listed = subprocess.run(
            [
                "docker",
                "images",
                "--format",
                "{{json .}}",
                "--filter",
                f"reference={WARM_REPOSITORY}/*",
            ],
            capture_output=True,
            text=True,
            timeout=60,
        )
    except (OSError, subprocess.TimeoutExpired):
        return []
    images = []
    for line in (listed.stdout or "").splitlines():
        try:
            row = json.loads(line)
        except json.JSONDecodeError:
            continue
        images.append(
            {
                "reference": f"{row.get('Repository')}:{row.get('Tag')}",
                "size": row.get("Size"),
                "created": row.get("CreatedAt"),
            }
        )
    return sorted(images, key=lambda image: image["reference"])


def remove_warm_images(match: str | None = None) -> list[str]:
    """Remove kept images whose reference contains ``match``; all without it."""
    removed = []
    for image in warm_images():
        reference = image["reference"]
        if match and match not in reference:
            continue
        done = subprocess.run(
            ["docker", "image", "rm", reference], capture_output=True, timeout=300
        )
        if done.returncode == 0:
            removed.append(reference)
    return removed


class WarmDockerEnvironment(CdiDockerEnvironment):
    """Harbor's Docker environment that keeps and reuses task images."""

    def __init__(self, *args: Any, **kwargs: Any) -> None:
        super().__init__(*args, **kwargs)
        self._warm_override_dir: tempfile.TemporaryDirectory[str] | None = None
        self._warm_override_path: Path | None = None
        self._warm_role = self.environment_dir.name or "environment"
        # The task's own image pin, if any, wins: nothing is built.
        self._task_image = self.task_env_config.docker_image
        self._warm_main = warm_tag(self.environment_name, self._warm_role, self.environment_id)

    def _warm_tags(self) -> dict[str, str]:
        """Every service the task builds, with the tag its image is kept as."""
        tags = {"main": self._warm_main}
        for service in built_services(self._environment_docker_compose_path):
            tags[service] = warm_tag(
                self.environment_name, self._warm_role, self.environment_id, service
            )
        return tags

    def _warm_override(self, tags: dict[str, str]) -> Path:
        """A compose override that starts every built service from its kept
        image. Compose builds nothing for a service whose image exists."""
        services = {name: {"image": tag} for name, tag in tags.items()}
        self._warm_override_dir = tempfile.TemporaryDirectory(prefix="tbench-warm-")
        path = Path(self._warm_override_dir.name) / "docker-compose-warm.json"
        path.write_text(json.dumps({"services": services}, indent=2))
        return path

    async def _keep_built_images(self, tags: dict[str, str]) -> list[str]:
        """Tag each built service's image so ``down --rmi local`` keeps it.

        Compose 5 removes every image it built at ``down --rmi local``,
        even one named by an ``image`` field, but only untags a name it
        gave; a second tag keeps the image.
        """
        kept = []
        for service, tag in tags.items():
            found = await self._run_docker_compose_command(
                ["ps", "--all", "-q", service], check=False
            )
            container = (found.stdout or "").strip().splitlines()
            if found.return_code != 0 or not container:
                continue
            image = subprocess.run(
                ["docker", "inspect", "--format", "{{.Image}}", container[-1]],
                capture_output=True,
                text=True,
                timeout=30,
            )
            if image.returncode != 0 or not image.stdout.strip():
                continue
            tagged = subprocess.run(
                ["docker", "tag", image.stdout.strip(), tag],
                capture_output=True,
                timeout=30,
            )
            if tagged.returncode == 0:
                kept.append(tag)
        return kept

    @property
    def _docker_compose_paths(self) -> list[Path]:
        paths = list(super()._docker_compose_paths)
        if self._warm_override_path is not None:
            paths.append(self._warm_override_path)
        return paths

    def _record(self, entry: dict[str, Any]) -> None:
        try:
            path = self.trial_paths.trial_dir / RECORD_NAME
            with path.open("a") as handle:
                handle.write(json.dumps(entry) + "\n")
        except OSError:
            pass

    async def start(self, force_build: bool):
        started = time.monotonic()
        tags = self._warm_tags()
        kept: list[str] = []
        cache = image_cache(self._task_image, tags, force_build, image_exists)
        if cache == "warm":
            # Harbor's prebuilt path: `up` starts the kept images and
            # nothing is built.
            self._warm_override_path = self._warm_override(tags)
            # A copy: the task's own config is shared with the verifier's
            # environment, which must not start from this image.
            self.task_env_config = self.task_env_config.model_copy(
                update={"docker_image": self._warm_main}
            )
            self._env_vars.prebuilt_image_name = self._warm_main
        try:
            await super().start(force_build)
            if cache == "cold":
                kept = await self._keep_built_images(tags)
        finally:
            self._record(
                {
                    "schema": RECORD_SCHEMA,
                    "role": self._warm_role,
                    "session": self.session_id,
                    "image": self._task_image or self._warm_main,
                    "cache": cache,
                    "kept": kept,
                    "start_ms": int((time.monotonic() - started) * 1000),
                    "at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
                }
            )

    async def stop(self, delete: bool):
        try:
            await super().stop(delete)
        finally:
            if self._warm_override_dir is not None:
                self._warm_override_dir.cleanup()
                self._warm_override_dir = None
