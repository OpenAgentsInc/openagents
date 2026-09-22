"""Run and resume: materialize the pinned job, invoke Harbor, collect.

Every run writes, beside Harbor's own output under the job dir, a
``tbench/`` tree holding the materialized job config, one attempt record
per trial, and one episode manifest per trial. Harbor's resume keeps
existing trial results, so collection is idempotent: records are rewritten
from whatever ``result.json`` files exist.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from . import paths
from .agents import AgentProfile, configured_auth_modes
from .counts import counts_for_trial
from .jobconfig import JobProfile, build_job_config, task_entry, write_job_config
from .panel import Panel, Task
from .results import (
    TrialPaths,
    attempt_record,
    episode_manifest,
    load_trial_results,
    sha256_file,
)


@dataclass
class RunRequest:
    """One arm over one job profile's task list."""

    panel: Panel
    profile: JobProfile
    agent: AgentProfile
    tasks: list[Task]
    auth_mode: str | None = None
    agent_kwargs: dict[str, Any] | None = None
    checkout: Path | None = None
    jobs_dir: Path | None = None
    job_name: str | None = None


class RunError(RuntimeError):
    """The run could not start or did not produce a readable result."""


def _check_credentials(agent: AgentProfile, auth_mode: str | None) -> str | None:
    """Refuse to start an arm whose credentials are absent."""
    if not agent.auth_modes:
        return auth_mode
    modes = configured_auth_modes(agent)
    if auth_mode:
        mode = agent.auth_modes.get(auth_mode)
        if mode is None:
            raise RunError(
                f"agent {agent.id} has no auth mode {auth_mode!r}"
            )
        if not mode.configured():
            names = ", ".join(
                mode.requires_all + mode.requires_any
            )
            raise RunError(
                f"auth mode {auth_mode!r} needs env: {names}; "
                "credentials are checked by name, never by value"
            )
        return auth_mode
    if not modes:
        known = ", ".join(sorted(agent.auth_modes))
        raise RunError(
            f"no credentials for {agent.id}; known modes: {known}. "
            "A missing credential is a setup failure, not a task failure."
        )
    return modes[0].name


def materialize(request: RunRequest) -> tuple[Path, dict[str, Any]]:
    """Write the job config and return ``(job_dir, config)``.

    The job name is deterministic per arm, profile, and pin: re-running a
    name resumes the same job rather than forking a second evidence tree.
    """
    # install_only proves the in-container install; it never runs the
    # agent, so absent credentials do not block it.
    auth_mode = (
        request.auth_mode
        if request.profile.install_only
        else _check_credentials(request.agent, request.auth_mode)
    )
    jobs_dir = request.jobs_dir or paths.jobs_dir()
    job_name = request.job_name or (
        f"{request.profile.id}--{request.agent.id}"
    )
    config = build_job_config(
        request.panel,
        request.profile,
        request.tasks,
        request.agent,
        auth_mode=auth_mode,
        agent_kwargs=request.agent_kwargs,
        checkout=request.checkout,
        jobs_dir=jobs_dir,
        job_name=job_name,
    )
    job_dir = jobs_dir / job_name
    trial_paths = TrialPaths(job_dir)
    trial_paths.tbench_dir.mkdir(parents=True, exist_ok=True)
    config["job_name"] = job_name
    write_job_config(config, trial_paths.config_path)
    write_job_config(
        {
            "agent_id": request.agent.id,
            "profile_id": request.profile.id,
            "auth_mode": auth_mode,
        },
        trial_paths.context_path,
    )
    return job_dir, config


def run(request: RunRequest, *, harbor_argv0: str = "harbor") -> Path:
    """Materialize the job and run Harbor over it."""
    job_dir, config = materialize(request)
    command = [
        harbor_argv0,
        "run",
        "--config",
        str(TrialPaths(job_dir).config_path),
        "--jobs-dir",
        str(config["jobs_dir"]),
        "--job-name",
        config["job_name"],
        "--yes",
    ]
    env = os.environ.copy()
    completed = subprocess.run(command, env=env)
    collect(job_dir, request)
    if completed.returncode != 0:
        raise RunError(
            f"harbor run exited {completed.returncode}; "
            f"per-trial evidence under {job_dir}"
        )
    return job_dir


def resume(request: RunRequest, *, harbor_argv0: str = "harbor") -> Path:
    """Resume an existing job dir; evidence already retained stays retained."""
    job_dir, config = materialize(request)
    command = [
        harbor_argv0,
        "job",
        "resume",
        "--job-path",
        str(job_dir),
    ]
    completed = subprocess.run(command)
    collect(job_dir, request)
    if completed.returncode != 0:
        raise RunError(
            f"harbor job resume exited {completed.returncode}; "
            f"per-trial evidence under {job_dir}"
        )
    return job_dir


def collect(job_dir: Path, request: RunRequest) -> list[Path]:
    """Write attempt records and episode manifests from Harbor's output.

    Safe to call repeatedly and after partial runs: each record is derived
    from the trial's own ``result.json``, so resume loses nothing.
    """
    trial_paths = TrialPaths(job_dir)
    trial_paths.attempts_dir.mkdir(parents=True, exist_ok=True)
    trial_paths.manifests_dir.mkdir(parents=True, exist_ok=True)
    context: dict[str, Any] = {}
    if trial_paths.context_path.is_file():
        context = json.loads(trial_paths.context_path.read_text())
    written: list[Path] = []
    pin = {
        "git_url": request.panel.git_url,
        "git_commit_id": request.panel.git_commit_id,
    }
    for trial_dir, trial_result in load_trial_results(job_dir):
        task_path = str(
            (trial_result.get("task_id") or {}).get("path") or ""
        )
        task_notes = next(
            (
                list(task.notes)
                for task in request.tasks
                if task_path.endswith(task.path)
                or (trial_result.get("task_name") or "").endswith(task.id)
            ),
            [],
        )
        attempt_path = trial_paths.attempts_dir / f"{trial_dir.name}.json"
        manifest_path = trial_paths.manifests_dir / f"{trial_dir.name}.json"
        record = attempt_record(
            trial_result,
            job_name=job_dir.name,
            trial_dir=trial_dir,
            arm=request.agent.id,
            profile_id=request.profile.id,
            auth_mode=context.get("auth_mode") or request.auth_mode,
            declared_cost_provenance=request.agent.cost_provenance,
            pin=pin,
            counts=counts_for_trial(trial_dir),
            evidence={
                "trial_result": str(trial_dir / "result.json"),
                "manifest": str(manifest_path),
            },
        )
        manifest = episode_manifest(
            record,
            trial_dir=trial_dir,
            arm=request.agent.id,
            task_notes=task_notes,
        )
        record["completeness"] = {
            "trace": (
                "present"
                if manifest["evidence"]["trajectory"]["resolved"]
                or manifest["evidence"]["native_traces"]
                else "absent"
            ),
            "usage": record["usage"]["coverage"],
            "cost": record["completeness"]["cost"],
            "artifacts": (
                "present" if manifest["evidence"]["artifacts"] else "absent"
            ),
        }
        attempt_path.write_text(json.dumps(record, indent=2) + "\n")
        manifest["evidence"]["attempt_record"] = {
            "kind": "attempt-record",
            "resolved": True,
            "path": str(attempt_path),
            "sha256": sha256_file(attempt_path),
        }
        manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")
        written.extend([attempt_path, manifest_path])
    return written


def harbor_available(argv0: str = "harbor") -> bool:
    from shutil import which

    return which(argv0) is not None
