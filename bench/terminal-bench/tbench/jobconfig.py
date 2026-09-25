"""Assemble a Harbor ``JobConfig`` from the checked profiles.

The materialized config is written into the job directory before ``harbor``
runs, so the exact resolved input — task pins, agent pins, timeouts,
retry policy — travels with the evidence. Credential values are never in
it: forwarded variables are ``${NAME}`` templates.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from . import paths
from .agents import AgentProfile, agent_config_env, model_for
from .panel import Panel, Task

JOBS_SCHEMA = "openagents.tbench.jobs.v1"


@dataclass(frozen=True)
class JobProfile:
    """A named job profile from ``profiles/jobs.json`` after defaults merge."""

    id: str
    description: str
    task_ids: tuple[str, ...]
    n_attempts: int
    n_concurrent_trials: int
    timeout_multiplier: float
    install_only: bool
    controls: bool
    retry: dict[str, Any]
    environment: dict[str, Any]
    verifier: dict[str, Any]
    # The task catalog the profile draws from; ``None`` is the panel's pin.
    catalog: str | None = None


def load_job_profile(
    profile_id: str, path: Path | None = None
) -> JobProfile:
    """Read one named job profile with the file's defaults merged in."""
    path = path or (paths.PROFILES_DIR / "jobs.json")
    data = json.loads(path.read_text())
    if data.get("schema_version") != JOBS_SCHEMA:
        raise ValueError(
            f"{path}: schema_version {data.get('schema_version')!r} "
            f"is not {JOBS_SCHEMA!r}"
        )
    defaults = data.get("defaults") or {}
    raw = (data.get("profiles") or {}).get(profile_id)
    if raw is None:
        known = ", ".join(sorted((data.get("profiles") or {}).keys()))
        raise KeyError(f"no job profile {profile_id!r} (known: {known})")
    environment = dict(defaults.get("environment") or {})
    environment.update(raw.get("environment") or {})
    verifier = dict(defaults.get("verifier") or {})
    verifier.update(raw.get("verifier") or {})
    retry = dict(defaults.get("retry") or {})
    retry.update(raw.get("retry") or {})
    return JobProfile(
        id=profile_id,
        description=raw.get("description", ""),
        task_ids=tuple(raw.get("tasks") or ()),
        n_attempts=int(raw.get("n_attempts", defaults.get("n_attempts", 1))),
        n_concurrent_trials=int(
            raw.get(
                "n_concurrent_trials", defaults.get("n_concurrent_trials", 1)
            )
        ),
        timeout_multiplier=float(
            raw.get("timeout_multiplier", defaults.get("timeout_multiplier", 1.0))
        ),
        install_only=bool(raw.get("install_only", False)),
        controls=bool(raw.get("controls", False)),
        retry=retry,
        environment=environment,
        verifier=verifier,
        catalog=raw.get("catalog"),
    )


def list_job_profiles(path: Path | None = None) -> dict[str, str]:
    """The named job profiles and their descriptions."""
    path = path or (paths.PROFILES_DIR / "jobs.json")
    data = json.loads(path.read_text())
    return {
        profile_id: (raw or {}).get("description", "")
        for profile_id, raw in (data.get("profiles") or {}).items()
    }


def task_entry(panel: Panel, task: Task, checkout: Path | None) -> dict[str, Any]:
    """One ``tasks[]`` entry pinning the upstream commit and task path.

    With a local checkout present the entry names it directly; without one
    it names the git source and commit and lets Harbor resolve the same
    content into its own cache. Both carry the pin.
    """
    if checkout is not None:
        return {"path": str(checkout / task.path)}
    return {
        "git_url": panel.git_url,
        "git_commit_id": panel.git_commit_id,
        "path": task.path,
    }


def build_job_config(
    panel: Panel,
    profile: JobProfile,
    tasks: list[Task],
    agent: AgentProfile,
    *,
    auth_mode: str | None = None,
    agent_kwargs: dict[str, Any] | None = None,
    checkout: Path | None = None,
    jobs_dir: Path | None = None,
    job_name: str | None = None,
    extra_env: dict[str, str] | None = None,
) -> dict[str, Any]:
    """The ``JobConfig`` mapping for one arm over a profile's task list."""
    kwargs = dict(agent.kwargs)
    kwargs.update(agent_kwargs or {})
    missing = [key for key in agent.required_kwargs if not kwargs.get(key)]
    if missing:
        raise ValueError(
            f"agent {agent.id!r} needs --agent-kwarg for: {', '.join(missing)}"
        )

    env = agent_config_env(profile=agent, auth_mode=auth_mode)
    env.update(extra_env or {})

    agent_config: dict[str, Any] = {
        "kwargs": kwargs,
        "env": env,
        "extra_allowed_hosts": list(agent.extra_allowed_hosts),
    }
    if agent.harbor_import_path:
        agent_config["import_path"] = agent.harbor_import_path
    else:
        agent_config["name"] = agent.harbor_name
    model = model_for(agent, auth_mode)
    if model:
        agent_config["model_name"] = model
    if agent.setup_timeout_sec is not None:
        agent_config["override_setup_timeout_sec"] = agent.setup_timeout_sec

    config: dict[str, Any] = {
        "jobs_dir": str(jobs_dir or paths.jobs_dir()),
        "n_attempts": profile.n_attempts,
        "n_concurrent_trials": profile.n_concurrent_trials,
        "timeout_multiplier": profile.timeout_multiplier,
        "install_only": profile.install_only,
        "retry": profile.retry,
        "environment": profile.environment,
        "verifier": profile.verifier,
        "agents": [agent_config],
        "tasks": [task_entry(panel, task, checkout) for task in tasks],
    }
    if job_name:
        config["job_name"] = job_name
    return config


def write_job_config(config: dict[str, Any], path: Path) -> Path:
    """Persist the materialized config beside the run's evidence."""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(config, indent=2) + "\n")
    return path
