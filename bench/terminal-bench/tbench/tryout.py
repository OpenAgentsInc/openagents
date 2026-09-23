"""``tbench try``: one arm on a few tasks, in minutes, with its loop time.

A targeted experiment runs a named arm over a handful of tasks with N
attempts each as one Harbor job, without the suite scheduler. It:

- starts every environment from a kept task image
  (``tbench.warm_docker``) unless ``--cold`` asks for a fresh build;
- prints a table of every trial each time one changes stage, with the
  episode's current component while the agent runs, the reward and cost
  as each trial finishes, and the loop time of each phase
  (``tbench.looptime``);
- writes ``tbench/looptime.json`` into the job and retains the evidence
  into the traces directory, unless ``--no-retain``.

The job is named ``try--<arm>--<UTC stamp>``, so a try never resumes or
pools with a suite's jobs.
"""

from __future__ import annotations

import dataclasses
import json
import threading
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable

from . import looptime, paths
from .runner import RunError, RunRequest, run

WARM_ENVIRONMENT = "tbench.warm_docker:WarmDockerEnvironment"
TRY_SCHEMA = "openagents.tbench.try.v1"


def job_name(arm: str, now: datetime | None = None) -> str:
    stamp = (now or datetime.now(timezone.utc)).strftime("%Y%m%dT%H%M%SZ")
    return f"try--{arm}--{stamp}"


def prepare(
    request: RunRequest,
    *,
    attempts: int,
    concurrency: int | None,
    warm: bool,
    name: str | None = None,
) -> RunRequest:
    """The request narrowed to a try: attempts, concurrency, and images."""
    environment = dict(request.profile.environment)
    if warm:
        environment["import_path"] = WARM_ENVIRONMENT
    trials = len(request.tasks) * attempts
    request.profile = dataclasses.replace(
        request.profile,
        n_attempts=attempts,
        n_concurrent_trials=max(1, min(concurrency or trials, trials)),
        environment=environment,
    )
    request.job_name = name or job_name(request.agent.id)
    return request


class Watcher:
    """Prints the job's table whenever a trial changes stage."""

    def __init__(
        self,
        job_dir: Path,
        interval: float,
        echo: Callable[[str], None] = print,
    ) -> None:
        self.job_dir = job_dir
        self.interval = interval
        self.echo = echo
        self._seen: dict[str, tuple[Any, ...]] = {}
        self._stop = threading.Event()
        self._thread: threading.Thread | None = None
        self.started = time.monotonic()

    def poll(self) -> bool:
        """Reads the job once; prints the table if anything changed."""
        rows = looptime.job_looptimes(self.job_dir)
        state = {
            row["trial"]: (row["stage"], row.get("reward"), row.get("image"))
            for row in rows
        }
        if not rows or state == self._seen:
            return False
        self._seen = state
        elapsed = time.monotonic() - self.started
        self.echo(
            f"\n[{time.strftime('%H:%M:%S')}] {self.job_dir.name}, "
            f"{elapsed / 60:.1f} min in"
        )
        self.echo(looptime.render(rows))
        return True

    def _loop(self) -> None:
        while not self._stop.wait(self.interval):
            try:
                self.poll()
            except Exception as exc:  # a read race must not end the run
                self.echo(f"try: couldn't read the job yet ({exc})")

    def __enter__(self) -> Watcher:
        self._thread = threading.Thread(target=self._loop, daemon=True)
        self._thread.start()
        return self

    def __exit__(self, *_exc: Any) -> None:
        self._stop.set()
        if self._thread is not None:
            self._thread.join(timeout=5)


def report(job_dir: Path, request: RunRequest, wall_sec: float) -> dict[str, Any]:
    """The try's summary, as written to ``tbench/looptime.json``."""
    rows = looptime.job_looptimes(job_dir)
    return {
        "schema": TRY_SCHEMA,
        "job": job_dir.name,
        "arm": request.agent.id,
        "profile": request.profile.id,
        "tasks": [task.id for task in request.tasks],
        "attempts": request.profile.n_attempts,
        "concurrency": request.profile.n_concurrent_trials,
        "environment": request.profile.environment.get("import_path"),
        "wall_sec": round(wall_sec, 1),
        "totals": looptime.totals(rows),
        "trials": rows,
    }


def run_try(
    request: RunRequest,
    *,
    interval: float = 10.0,
    retain: bool = True,
    echo: Callable[[str], None] = print,
    runner: Callable[[RunRequest], Path] = run,
) -> tuple[Path, dict[str, Any], int]:
    """Runs the try and returns ``(job_dir, report, exit code)``."""
    jobs_dir = request.jobs_dir or paths.jobs_dir()
    job_dir = jobs_dir / (request.job_name or "")
    echo(
        f"try: {request.agent.id} on {', '.join(t.id for t in request.tasks)}, "
        f"{request.profile.n_attempts} attempt(s) each, "
        f"{request.profile.n_concurrent_trials} at once, "
        + (
            "kept task images"
            if request.profile.environment.get("import_path") == WARM_ENVIRONMENT
            else "fresh task images"
        )
    )
    echo(f"try: job {job_dir}")
    started = time.monotonic()
    code = 0
    with Watcher(job_dir, interval, echo) as watcher:
        try:
            runner(request)
        except RunError as exc:
            echo(f"try: {exc}")
            code = 1
        watcher.poll()
    summary = report(job_dir, request, time.monotonic() - started)
    tbench_dir = job_dir / "tbench"
    if tbench_dir.is_dir():
        (tbench_dir / "looptime.json").write_text(json.dumps(summary, indent=2) + "\n")
    echo("")
    echo(looptime.render(summary["trials"]))
    echo(f"\ntry: wall time {summary['wall_sec'] / 60:.1f} min")
    if retain and summary["trials"]:
        from .retain import retain_jobs

        retained, errors = retain_jobs([str(job_dir)])
        for item in retained:
            echo(f"try: retained {item.job}/{item.trial} at {item.destination}")
        for error in errors:
            echo(f"try: retain: {error}")
    return job_dir, summary, code
