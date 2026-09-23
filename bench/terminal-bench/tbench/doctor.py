"""Doctor: the read-only preflight the runbook asks for first.

Every check returns a status and a one-line finding rather than raising:
the point is to name precise blockers. ``--smoke`` adds the checks that
cost something — registry and network reachability, image-manifest
inspection — and says so, because a read-only audit should never download
an image or spend inference.
"""

from __future__ import annotations

import json
import os
import platform
import shutil
import subprocess
import tomllib
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

from . import HARBOR_PIN, UPSTREAM_COMMIT, paths
from .agents import AgentProfile, configured_auth_modes
from .panel import Panel

PASS, WARN, FAIL, SKIP = "pass", "warn", "fail", "skip"

# Floor for host disk headroom before image pulls and builds start.
MIN_HOST_DISK_GIB = 20


@dataclass
class Check:
    id: str
    status: str
    detail: str
    hint: str = ""

    def line(self) -> str:
        mark = {PASS: "ok  ", WARN: "warn", FAIL: "FAIL", SKIP: "skip"}[
            self.status
        ]
        suffix = f" — {self.hint}" if self.hint else ""
        return f"  {mark} {self.id:<28} {self.detail}{suffix}"


@dataclass
class Report:
    checks: list[Check] = field(default_factory=list)

    def add(self, check: Check) -> Check:
        self.checks.append(check)
        return check

    @property
    def ok(self) -> bool:
        return not any(c.status == FAIL for c in self.checks)

    def worst(self) -> str:
        for status in (FAIL, WARN, SKIP):
            if any(c.status == status for c in self.checks):
                return status
        return PASS


def _run(command: list[str], timeout: int = 30) -> tuple[int, str]:
    try:
        out = subprocess.run(
            command,
            capture_output=True,
            text=True,
            timeout=timeout,
        )
        return out.returncode, (out.stdout + out.stderr).strip()
    except (OSError, subprocess.TimeoutExpired) as exc:
        return 127, str(exc)


def _docker_info() -> tuple[int, dict[str, Any]]:
    code, text = _run(["docker", "info", "--format", "{{json .}}"], timeout=20)
    if code != 0:
        return code, {}
    try:
        return 0, json.loads(text.splitlines()[-1])
    except (json.JSONDecodeError, IndexError):
        return 1, {}


def check_tooling(report: Report) -> None:
    """The pinned harness pieces: harbor, uv, git, python."""
    try:
        import harbor  # noqa: PLC0415

        version = getattr(harbor, "__version__", "unknown")
        if version == HARBOR_PIN:
            report.add(Check("harbor", PASS, f"harbor {version} installed"))
        else:
            report.add(
                Check(
                    "harbor",
                    FAIL,
                    f"harbor {version}, pinned {HARBOR_PIN}",
                    "uv sync in bench/terminal-bench",
                )
            )
    except ImportError:
        report.add(
            Check("harbor", FAIL, "harbor not importable", "uv sync in bench/terminal-bench")
        )
    for tool in ("uv", "git", "docker"):
        path = shutil.which(tool)
        report.add(
            Check(
                f"tool:{tool}",
                PASS if path else FAIL,
                path or f"{tool} not on PATH",
            )
        )


def check_docker(report: Report, panel: Panel) -> dict[str, Any]:
    """The daemon, its VM capacity, and headroom against the panel."""
    code, info = _docker_info()
    if code != 0 or not info:
        report.add(
            Check(
                "docker-daemon",
                FAIL,
                "docker daemon did not answer",
                "start Docker Desktop, then re-run doctor",
            )
        )
        return {}
    server = info.get("ServerVersion", "?")
    arch = info.get("Architecture", "?")
    ostype = info.get("OSType", "?")
    cpus = int(info.get("NCPU", 0) or 0)
    mem_gib = (int(info.get("MemTotal", 0) or 0)) / (1 << 30)
    report.add(
        Check(
            "docker-daemon",
            PASS,
            f"server {server}, {ostype}/{arch}, {cpus} CPUs, {mem_gib:.1f} GiB",
        )
    )

    runnable = [t for t in panel.tasks if not t.excluded]
    need_cpu = max((t.resources.cpus for t in runnable), default=0)
    need_mem = max((t.resources.memory_mb for t in runnable), default=0) / 1024
    if cpus < need_cpu or mem_gib < need_mem:
        report.add(
            Check(
                "docker-capacity",
                FAIL,
                f"panel needs {need_cpu} CPUs and {need_mem:.0f} GiB; "
                f"VM offers {cpus} CPUs and {mem_gib:.1f} GiB",
                "raise the Docker VM allocation or trim the profile",
            )
        )
    else:
        report.add(
            Check(
                "docker-capacity",
                PASS,
                f"panel max {need_cpu} CPUs / {need_mem:.0f} GiB fits the VM",
            )
        )

    host = platform.machine()
    if host == "arm64" or host == "aarch64":
        amd64 = [
            t.id for t in runnable if "amd64" in t.image_arches and "arm64" not in t.image_arches
        ]
        if amd64:
            report.add(
                Check(
                    "emulation",
                    WARN,
                    f"amd64-only images under emulation: {', '.join(amd64)}",
                    "record the emulation condition on every run",
                )
            )
    return info


def check_disk(report: Report) -> None:
    usage = shutil.disk_usage(Path.home())
    free_gib = usage.free / (1 << 30)
    status = PASS if free_gib >= MIN_HOST_DISK_GIB else FAIL
    report.add(
        Check(
            "host-disk",
            status,
            f"{free_gib:.0f} GiB free",
            "" if status == PASS else "free space before pulling images",
        )
    )
    code, text = _run(["docker", "system", "df", "--format", "{{json .}}"], timeout=20)
    if code == 0 and text:
        report.add(Check("docker-df", PASS, "docker system df answered"))
    else:
        report.add(Check("docker-df", SKIP, "docker df unavailable"))


def check_checkout(report: Report, panel: Panel) -> Path | None:
    """The persistent upstream checkout and its pin."""
    checkout = paths.upstream_checkout()
    if not (checkout / ".git").exists():
        report.add(
            Check(
                "upstream-checkout",
                WARN,
                f"no checkout at {checkout}",
                "tbench tasks checkout clones the pinned upstream",
            )
        )
        return None
    code, head = _run(["git", "-C", str(checkout), "rev-parse", "HEAD"])
    head = head.strip()
    if code == 0 and head == panel.git_commit_id:
        report.add(
            Check("upstream-checkout", PASS, f"checkout at {panel.git_commit_id[:12]}")
        )
    else:
        report.add(
            Check(
                "upstream-checkout",
                FAIL,
                f"HEAD {head[:12] or '?'} != pinned {panel.git_commit_id[:12]}",
                "re-pin or re-clone; do not run against a moved checkout",
            )
        )
    code, dirty = _run(["git", "-C", str(checkout), "status", "--porcelain"])
    if code == 0 and dirty.strip():
        report.add(
            Check(
                "upstream-dirty",
                WARN,
                "the upstream checkout has local changes",
                "local edits must be recorded as a task variant",
            )
        )
    return checkout


def check_task_configs(report: Report, panel: Panel, checkout: Path | None) -> None:
    """Cross-check the panel's declared resources against upstream task.toml."""
    if checkout is None:
        report.add(
            Check(
                "task-configs",
                SKIP,
                "no upstream checkout; declared values unverified",
            )
        )
        return
    mismatches = []
    for task in panel.tasks:
        toml_path = checkout / task.path / "task.toml"
        if not toml_path.exists():
            mismatches.append(f"{task.id}: no task.toml")
            continue
        data = tomllib.loads(toml_path.read_text())
        env = data.get("environment") or {}
        cpus = env.get("cpus", env.get("cpu"))
        memory = env.get("memory_mb", env.get("memory"))
        storage = env.get("storage_mb", env.get("storage"))
        if cpus is not None and int(cpus) != task.resources.cpus:
            mismatches.append(
                f"{task.id}: cpus panel={task.resources.cpus} task.toml={cpus}"
            )
        if memory is not None:
            mem_mb = int(memory)
            if mem_mb < 64:  # some tasks declare GiB
                mem_mb *= 1024
            if mem_mb != task.resources.memory_mb:
                mismatches.append(
                    f"{task.id}: memory panel={task.resources.memory_mb} task.toml={mem_mb}"
                )
        if storage is not None:
            st_mb = int(storage)
            if st_mb < 256:
                st_mb *= 1024
            if st_mb != task.resources.storage_mb:
                mismatches.append(
                    f"{task.id}: storage panel={task.resources.storage_mb} task.toml={st_mb}"
                )
    if mismatches:
        report.add(
            Check(
                "task-configs",
                WARN,
                "; ".join(mismatches[:4]) + (" …" if len(mismatches) > 4 else ""),
                "panel values follow the audit; reconcile before claiming pins",
            )
        )
    else:
        report.add(
            Check(
                "task-configs",
                PASS,
                f"{len(panel.tasks)} task configs match declared resources",
            )
        )


def check_images(report: Report, panel: Panel, online: bool) -> None:
    """Declared image architectures, by manifest inspection."""
    images = {image for task in panel.tasks for image in task.images}
    if not images:
        report.add(Check("image-arch", SKIP, "panel declares no images"))
        return
    if not online:
        report.add(
            Check("image-arch", SKIP, "offline; image manifests not inspected")
        )
        return
    for image in sorted(images):
        code, text = _run(
            ["docker", "buildx", "imagetools", "inspect", image], timeout=60
        )
        if code != 0:
            code, text = _run(["docker", "manifest", "inspect", image], timeout=60)
        if code != 0:
            report.add(
                Check(
                    f"image:{image}",
                    WARN,
                    "manifest not inspectable",
                    "manifest availability is not a runtime test",
                )
            )
            continue
        arches = sorted(
            {line.split()[-1] for line in text.splitlines() if "linux/" in line}
            or {line for line in text.splitlines() if "amd64" in line or "arm64" in line}
        )
        report.add(Check(f"image:{image}", PASS, ", ".join(arches) or "see log"))


def check_credentials(report: Report, profiles: list[AgentProfile]) -> None:
    """Which auth mode each arm could use — names only, never values."""
    for profile in profiles:
        if not profile.auth_modes:
            report.add(
                Check(f"creds:{profile.id}", PASS, "no credential required")
            )
            continue
        modes = configured_auth_modes(profile)
        if modes:
            names = ", ".join(mode.name for mode in modes)
            report.add(
                Check(f"creds:{profile.id}", PASS, f"configured: {names}")
            )
        else:
            report.add(
                Check(
                    f"creds:{profile.id}",
                    WARN,
                    "no auth mode's variables are set",
                    "see the runbook's authentication section",
                )
            )


def check_claude_setup_token(report: Report, path: Path | None = None) -> Check:
    """Whether the long-lived Claude token is in place. Never reads it aloud."""
    from . import credentials

    status = credentials.setup_token_status(path or credentials.SETUP_TOKEN)
    if status["usable"]:
        return report.add(
            Check(
                "claude-setup-token",
                PASS,
                f"present, mode {status['mode']}, written {status['age_days']:g} days ago "
                "(value not shown)",
            )
        )
    if not status["present"]:
        return report.add(
            Check(
                "claude-setup-token",
                WARN,
                "missing; Claude trials fall back to the login token, which a "
                "login refresh revokes, and `tbench experiment` refuses to start",
                credentials.SETUP_TOKEN_HINT,
            )
        )
    return report.add(
        Check("claude-setup-token", FAIL, status["problem"], credentials.SETUP_TOKEN_HINT)
    )


def check_host_agents(report: Report) -> None:
    """Host agent installs are informational; they prove nothing about the container."""
    for binary in ("claude", "codex", "coder"):
        path = shutil.which(binary)
        if not path:
            report.add(Check(f"host:{binary}", SKIP, "not on PATH"))
            continue
        code, text = _run([binary, "--version"], timeout=15)
        first = text.splitlines()[0] if text else "?"
        report.add(
            Check(
                f"host:{binary}",
                PASS if code == 0 else WARN,
                f"{first} at {path}",
                "host installs do not authenticate or version the container",
            )
        )


def run_doctor(
    panel: Panel,
    agents: list[AgentProfile],
    *,
    online: bool = False,
) -> Report:
    """The full read-only preflight."""
    report = Report()
    check_tooling(report)
    check_docker(report, panel)
    check_disk(report)
    checkout = check_checkout(report, panel)
    check_task_configs(report, panel, checkout)
    check_images(report, panel, online)
    check_credentials(report, agents)
    check_claude_setup_token(report)
    check_host_agents(report)
    return report
