"""Run and resume: materialize the pinned job, invoke Harbor, collect.

Every run writes, beside Harbor's own output under the job dir, a
``tbench/`` tree holding the materialized job config, one attempt record
per trial, and one episode manifest per trial. Harbor's resume keeps
existing trial results, so collection is idempotent: records are rewritten
from whatever ``result.json`` files exist.

Four things here exist because a real trial showed they were needed:

- Harbor deletes every subdirectory of an existing job dir that has no
  ``result.json``, ``tbench/`` included, when it resumes a job or reruns
  a job name. ``tbench/`` waits in a sibling directory while Harbor runs.

- Harbor runs in a session of its own. A Ctrl-C reaches ``tbench`` only,
  which forwards one SIGINT to Harbor and waits while Harbor cancels its
  trials and tears their environments down. Before this, ``subprocess``
  killed Harbor a quarter of a second after the interrupt and left the
  trial's container running.
- A run that is refused before Harbor starts, for missing credentials or
  an artifact that doesn't match its pin, leaves a refusal record in the
  job's ``tbench/refusals/`` rather than only a line on stderr.
- ``harbor job resume`` deletes the directory of every cancelled trial and
  of every trial that never wrote ``result.json``, then runs it again.
  ``resume`` copies those directories to ``tbench/interrupted/`` first, so
  the interrupted attempt and its partial evidence survive the rerun.
"""

from __future__ import annotations

import json
import os
import shutil
import signal
import subprocess
import threading
from collections.abc import Iterator
from contextlib import contextmanager
from dataclasses import dataclass
from datetime import datetime, timezone
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
    sha256_file,
)

REFUSAL_SCHEMA = "openagents.tbench.refusal.v1"

# Harbor's exception type for a trial it cancelled; `harbor job resume`
# removes trials with this type by default and runs them again.
CANCELLED_ERROR_TYPE = "CancelledError"

# The signals tbench forwards to Harbor, once, as SIGINT.
FORWARDED_SIGNALS = (signal.SIGINT, signal.SIGTERM, signal.SIGHUP)


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


class RunRefused(RunError):
    """The run was refused before Harbor started; nothing was spent."""

    def __init__(self, stage: str, message: str) -> None:
        super().__init__(message)
        self.stage = stage


def _needs(mode: Any) -> str:
    """An auth mode's required variables, by name only."""
    parts = list(mode.requires_all)
    if mode.requires_any:
        parts.append("one of " + " or ".join(mode.requires_any))
    return ", ".join(parts) or "nothing"


def _check_credentials(agent: AgentProfile, auth_mode: str | None) -> str | None:
    """Refuse to start an arm whose credentials are absent."""
    if not agent.auth_modes:
        return auth_mode
    modes = configured_auth_modes(agent)
    if auth_mode:
        mode = agent.auth_modes.get(auth_mode)
        if mode is None:
            raise RunRefused(
                "credentials",
                f"agent {agent.id} has no auth mode {auth_mode!r}",
            )
        if not mode.configured():
            raise RunRefused(
                "credentials",
                f"auth mode {auth_mode!r} needs env: {_needs(mode)}; "
                "credentials are checked by name, never by value",
            )
        return auth_mode
    if not modes:
        known = "; ".join(
            f"{name} needs {_needs(mode)}"
            for name, mode in sorted(agent.auth_modes.items())
        )
        raise RunRefused(
            "credentials",
            f"no credentials for {agent.id}; known modes: {known}. "
            "A missing credential is a setup failure, not a task failure.",
        )
    return modes[0].name


def _check_artifact(kwargs: dict[str, Any]) -> None:
    """Refuse a pinned artifact that is missing or doesn't match its digest.

    The contract adapter checks the same pin, but it does so while Harbor
    constructs the agent, which aborts the whole Harbor job without a
    trial result. Checking here refuses the run before Harbor starts.
    """
    path, pinned = kwargs.get("artifact_path"), kwargs.get("artifact_sha256")
    if not path or not pinned:
        return
    artifact = Path(str(path)).expanduser()
    if not artifact.is_file():
        raise RunRefused(
            "artifact",
            f"artifact not found: {artifact}; there is no fallback binary",
        )
    digest = sha256_file(artifact)
    if digest.lower() != str(pinned).lower():
        raise RunRefused(
            "artifact",
            f"artifact sha256 {digest} doesn't match the pin {pinned}; "
            "refusing to run an unpinned build",
        )


def _check_gpu(tasks: list[Task]) -> None:
    """Refuse a GPU task when Docker has no NVIDIA runtime.

    Only asks Docker when a selected task needs a GPU, so CPU-only runs
    never depend on ``docker info``.
    """
    from . import host

    needing = [task for task in tasks if task.requires_gpu_runtime]
    if not needing:
        return
    info = host.docker_info()
    for task in needing:
        reason = host.gpu_refusal(task.id, info)
        if reason:
            raise RunRefused("gpu_runtime", reason)


def _job_name(request: RunRequest) -> str:
    return request.job_name or f"{request.profile.id}--{request.agent.id}"


def write_refusal(
    job_dir: Path, request: RunRequest, refused: RunRefused
) -> Path:
    """Record a run refused before Harbor started, beside the job's evidence."""
    stamp = datetime.now(timezone.utc)
    refusals = TrialPaths(job_dir).tbench_dir / "refusals"
    refusals.mkdir(parents=True, exist_ok=True)
    path = refusals / f"{stamp.strftime('%Y%m%dT%H%M%S%fZ')}.json"
    record = {
        "schema": REFUSAL_SCHEMA,
        "job": job_dir.name,
        "arm": request.agent.id,
        "profile": request.profile.id,
        "tasks": [task.id for task in request.tasks],
        "refused_at": stamp.isoformat(timespec="milliseconds"),
        "stage": refused.stage,
        "terminal_status": "setup_failure",
        "reason": str(refused),
        "spend": (
            "none: refused before Harbor started, so no environment was "
            "built and no inference ran"
        ),
    }
    path.write_text(json.dumps(record, indent=2) + "\n")
    return path


def materialize(
    request: RunRequest, *, record_refusal: bool = False
) -> tuple[Path, dict[str, Any]]:
    """Write the job config and return ``(job_dir, config)``.

    The job name is deterministic per arm, profile, and pin: re-running a
    name resumes the same job rather than forking a second evidence tree.
    With ``record_refusal``, a refused run leaves a refusal record in the
    job dir before the error propagates.
    """
    jobs_dir = request.jobs_dir or paths.jobs_dir()
    job_name = _job_name(request)
    job_dir = jobs_dir / job_name
    try:
        # install_only proves the in-container install; it never runs the
        # agent, so absent credentials do not block it.
        auth_mode = (
            request.auth_mode
            if request.profile.install_only
            else _check_credentials(request.agent, request.auth_mode)
        )
        kwargs = dict(request.agent.kwargs)
        kwargs.update(request.agent_kwargs or {})
        _check_artifact(kwargs)
        _check_gpu(request.tasks)
    except RunRefused as refused:
        if record_refusal:
            path = write_refusal(job_dir, request, refused)
            raise RunRefused(
                refused.stage, f"{refused} (refusal recorded at {path})"
            ) from refused
        raise
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


def _harbor_child_setup() -> None:
    """Give Harbor the default SIGINT disposition, even under a parent that
    ignores it, so asyncio installs its cancelling handler."""
    signal.signal(signal.SIGINT, signal.SIG_DFL)


def run_harbor(command: list[str]) -> tuple[int, str | None]:
    """Run Harbor in its own session and return ``(exit code, signal)``.

    The first SIGINT, SIGTERM, or SIGHUP that reaches ``tbench`` is
    forwarded to Harbor once, as SIGINT: asyncio turns a first SIGINT into
    a cancellation of Harbor's main task, so every trial records
    ``CancelledError``, collects its outputs, and stops its environment.
    Later signals aren't forwarded, because a second SIGINT makes asyncio
    raise immediately and skip that cleanup, and so does Harbor's own
    SIGTERM handler. ``tbench`` keeps waiting until Harbor exits. The
    second value names the first signal received, or is None.
    """
    process = subprocess.Popen(
        command, start_new_session=True, preexec_fn=_harbor_child_setup
    )
    received: list[str] = []

    def forward(signum: int, _frame: Any) -> None:
        received.append(signal.Signals(signum).name)
        if len(received) > 1:
            return
        try:
            process.send_signal(signal.SIGINT)
        except ProcessLookupError:
            pass

    previous: dict[int, Any] = {}
    if threading.current_thread() is threading.main_thread():
        for signum in FORWARDED_SIGNALS:
            previous[signum] = signal.signal(signum, forward)
    try:
        returncode = process.wait()
    finally:
        for signum, handler in previous.items():
            signal.signal(signum, handler)
    return returncode, (received[0] if received else None)


def _finish(
    job_dir: Path, request: RunRequest, verb: str, returncode: int, got: str | None
) -> Path:
    collect(job_dir, request)
    if got is not None:
        raise RunError(
            f"interrupted by {got}; Harbor cancelled the job and exited "
            f"{returncode}. Evidence is under {job_dir}; `tbench resume` "
            "continues it"
        )
    if returncode != 0:
        raise RunError(
            f"harbor {verb} exited {returncode}; "
            f"per-trial evidence under {job_dir}"
        )
    return job_dir


def held_path(job_dir: Path) -> Path:
    """Where ``tbench/`` waits while Harbor works on an existing job dir.

    A sibling of the job dir whose inner name isn't ``tbench``, so the
    comparison's ``*/tbench/attempts`` scan never counts it twice.
    """
    return job_dir.parent / f".tbench-held--{job_dir.name}" / "held"


@contextmanager
def tbench_held_aside(job_dir: Path) -> Iterator[Path]:
    """Move ``tbench/`` out of the job dir while Harbor runs.

    When Harbor opens an existing job dir, for `harbor job resume` or for
    a `harbor run` of a job name that already exists, it deletes every
    subdirectory without a ``result.json`` as an unfinished trial. That
    includes ``tbench/``: the attempt records, the refusal records, the
    preserved interrupted trials, and the materialized config. Yields the
    held ``tbench/`` path and moves it back afterwards. A hold left by a
    crashed run is restored first.
    """
    live = TrialPaths(job_dir).tbench_dir
    held = held_path(job_dir)
    if held.exists():
        if live.exists():
            raise RunError(
                f"both {live} and {held} exist; a previous run was "
                "interrupted while Harbor ran. Merge them by hand, then "
                "run again"
            )
        held.rename(live)
        held.parent.rmdir()
    held.parent.mkdir(parents=True, exist_ok=True)
    live.rename(held)
    try:
        yield held
    finally:
        if live.exists():
            # Nothing should create tbench/ while Harbor runs; keep both
            # rather than overwrite either.
            live.rename(held.parent / f"created-during-run-{os.getpid()}")
        held.rename(live)
        try:
            held.parent.rmdir()
        except OSError:
            pass


def run(request: RunRequest, *, harbor_argv0: str = "harbor") -> Path:
    """Materialize the job and run Harbor over it."""
    job_dir, config = materialize(request, record_refusal=True)
    with tbench_held_aside(job_dir) as held:
        command = [
            harbor_argv0,
            "run",
            "--config",
            str(held / TrialPaths(job_dir).config_path.name),
            "--jobs-dir",
            str(config["jobs_dir"]),
            "--job-name",
            config["job_name"],
            "--yes",
        ]
        returncode, got = run_harbor(command)
    return _finish(job_dir, request, "run", returncode, got)


def resume(request: RunRequest, *, harbor_argv0: str = "harbor") -> Path:
    """Resume an existing job dir; evidence already retained stays retained."""
    job_dir, _config = materialize(request, record_refusal=True)
    preserve_interrupted(job_dir)
    command = [
        harbor_argv0,
        "job",
        "resume",
        "--job-path",
        str(job_dir),
    ]
    with tbench_held_aside(job_dir):
        returncode, got = run_harbor(command)
    return _finish(job_dir, request, "job resume", returncode, got)


def trial_dirs(job_dir: Path) -> list[Path]:
    """Every Harbor trial directory in a job dir, finished or not."""
    if not job_dir.is_dir():
        return []
    return sorted(
        path
        for path in job_dir.iterdir()
        if path.is_dir()
        and path.name != "tbench"
        and any(
            (path / name).exists()
            for name in ("result.json", "config.json", "trial.log")
        )
    )


def _read_result(trial_dir: Path) -> dict[str, Any] | None:
    try:
        return json.loads((trial_dir / "result.json").read_text())
    except (OSError, json.JSONDecodeError):
        return None


def preserve_interrupted(job_dir: Path) -> list[Path]:
    """Copy the trials `harbor job resume` would delete to ``tbench/interrupted``.

    Harbor's resume removes a trial directory without ``result.json`` and
    one whose result is a cancellation, then runs that trial again. The
    copies keep the interrupted attempt, and whatever partial bundle it
    collected, as evidence.
    """
    interrupted = TrialPaths(job_dir).tbench_dir / "interrupted"
    kept: list[Path] = []
    for trial_dir in trial_dirs(job_dir):
        result = _read_result(trial_dir)
        exception = (result or {}).get("exception_info") or {}
        if result is not None and (
            exception.get("exception_type") != CANCELLED_ERROR_TYPE
        ):
            continue
        target = interrupted / trial_dir.name
        if not target.exists():
            interrupted.mkdir(parents=True, exist_ok=True)
            shutil.copytree(trial_dir, target, symlinks=True)
        kept.append(target)
    return kept


def _placeholder_result(trial_dir: Path) -> dict[str, Any]:
    """What a trial without ``result.json`` can still say about itself."""
    config: dict[str, Any] = {}
    try:
        config = json.loads((trial_dir / "config.json").read_text())
    except (OSError, json.JSONDecodeError):
        pass
    return {
        "trial_name": trial_dir.name,
        "config": config,
        "task_id": config.get("task") or {},
    }


def _bundle_state(trial_dir: Path) -> str:
    """Whether the contract adapter collected an episode bundle."""
    agent_dir = trial_dir / "agent"
    if (agent_dir / "episode-collection-failed.txt").exists():
        return "collection_failed"
    bundle = agent_dir / "episode"
    if bundle.is_dir():
        if any(path.is_file() for path in bundle.rglob("*")):
            return "present"
        return "empty"
    return "not_applicable"


def collect(job_dir: Path, request: RunRequest) -> list[Path]:
    """Write attempt records and episode manifests from Harbor's output.

    Safe to call repeatedly and after partial runs: each record is derived
    from the trial's own ``result.json``, so resume loses nothing. A trial
    without ``result.json`` gets a record whose status is ``unknown``, and
    a trial preserved under ``tbench/interrupted/`` keeps a record of kind
    ``interrupted``.
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
    live = trial_dirs(job_dir)
    live_names = {path.name for path in live}
    interrupted_dir = trial_paths.tbench_dir / "interrupted"
    preserved = [
        path
        for path in (
            sorted(p for p in interrupted_dir.iterdir() if p.is_dir())
            if interrupted_dir.is_dir()
            else []
        )
        if path.name not in live_names
    ]
    for trial_dir in live + preserved:
        kind = "interrupted" if trial_dir in preserved else "fresh"
        trial_result = _read_result(trial_dir)
        has_result = trial_result is not None
        if trial_result is None:
            trial_result = _placeholder_result(trial_dir)
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
                "trial_result": (
                    str(trial_dir / "result.json") if has_result else None
                ),
                "trial_dir": str(trial_dir),
                "manifest": str(manifest_path),
            },
        )
        record["attempt"]["kind"] = kind
        if not has_result:
            record["outcome"]["terminal_status"] = "unknown"
            record["outcome"]["note"] = (
                "Harbor wrote no result.json: the trial stopped before it "
                "finished, and nothing says why"
            )
        elif kind == "interrupted":
            record["outcome"]["note"] = (
                "Preserved before `harbor job resume` removed this trial "
                "and ran it again; the rerun is a separate attempt"
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
            "bundle": _bundle_state(trial_dir),
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
