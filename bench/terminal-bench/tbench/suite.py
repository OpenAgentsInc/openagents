"""Run a whole task suite on one host: the resource-aware scheduler.

``tbench suite run`` runs one arm over every task of a profile, a given
number of attempts each, as many trials at once as the host's budgets
allow. Each trial is its own job, named as the runbook names a
repetition (``<profile>--<arm>--<task>``, then ``-2``, ``-3``), and runs
through ``tbench run``, so every trial keeps the evidence tree, attempt
record, and refusal handling a single run has.

The scheduler holds no state that the job directories don't: on every
start it reads each job directory to decide what is finished, what was
interrupted, and what never ran, and it adopts a trial whose ``tbench``
process is still running from before a restart. Starting it again is
always safe, and it never runs a finished trial twice.

Budgets:

- CPUs and memory: a trial reserves its task's declared budget, the larger
  of the agent and separate verifier environments. A task that needs more
  than the whole budget runs only when nothing else does.
- Disk: no trial starts while the Docker volume has less free space than
  the floor. When a task's trials have all finished and free space is
  within the prune margin of the floor, the task's leftover images go;
  under the floor, Docker's unused build cache goes too.
- GPUs: a task that needs a GPU is skipped, with the reason recorded, when
  Docker can't hand a container one (no NVIDIA CDI spec or runtime). GPU
  trials take GPU slots, one by default, so they run one at a time.

A setup timeout is never a result. The scheduler moves that job directory
to ``failed/`` and runs the trial once more, as the runbook does by hand.
"""

from __future__ import annotations

import contextlib
import fcntl
import json
import os
import signal
import subprocess
import sys
import time
from collections.abc import Callable, Iterator, Mapping
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from . import host, paths
from .panel import Task
from .results import TrialPaths

STATUS_SCHEMA = "openagents.tbench.suite-status.v1"

# Exceptions that end a trial before the agent did any work. Harbor's
# names; the trial is retried once after its job dir moves to failed/.
SETUP_TIMEOUTS = frozenset({"AgentSetupTimeoutError", "EnvironmentStartTimeoutError"})
CANCELLED = "CancelledError"

# A trial whose process exited without a finished result is resumed this
# many times before the scheduler gives up on it.
MAX_RESUMES = 2

# Trial states in the status file.
PENDING, RUNNING, FINISHED, SKIPPED, REFUSED, FAILED = (
    "pending",
    "running",
    "finished",
    "skipped",
    "refused",
    "failed",
)
TERMINAL = frozenset({FINISHED, SKIPPED, REFUSED, FAILED})


def utc_now() -> str:
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


def job_name(profile: str, arm: str, task: str, attempt: int) -> str:
    """The runbook's job name: the first attempt bare, then ``-2``, ``-3``."""
    base = f"{profile}--{arm}--{task}"
    return base if attempt == 1 else f"{base}-{attempt}"


def suite_dir(profile: str, arm: str) -> Path:
    """Where one suite's status file, lock, and process logs live."""
    return paths.state_dir() / "suites" / f"{profile}--{arm}"


def failed_dir() -> Path:
    return paths.state_dir() / "failed"


@dataclass(frozen=True)
class Budget:
    max_cpus: float = 24
    max_mem_gb: float = 100
    min_free_disk_gb: float = 40
    # Prune a finished task's images when free space is within this much
    # of the floor.
    prune_margin_gb: float = 20
    max_concurrent: int | None = None
    # GPU trials at once. This host has one 16 GB card, so GPU trials run
    # one at a time; a trial that asks for more GPUs than this runs alone.
    max_gpus: int = 1
    # A pending trial that has waited this long stops smaller trials from
    # backfilling past it, so large tasks can't starve.
    reserve_after_sec: float = 1800

    def to_json(self) -> dict[str, Any]:
        return {
            "max_cpus": self.max_cpus,
            "max_mem_gb": self.max_mem_gb,
            "min_free_disk_gb": self.min_free_disk_gb,
            "prune_margin_gb": self.prune_margin_gb,
            "max_concurrent": self.max_concurrent,
            "max_gpus": self.max_gpus,
            "reserve_after_sec": self.reserve_after_sec,
        }


@dataclass
class Trial:
    task: Task
    attempt: int
    job: str
    state: str = PENDING
    reason: str | None = None
    retries: int = 0
    resumes: int = 0
    pid: int | None = None
    started_at: str | None = None
    finished_at: str | None = None
    reward: float | None = None
    exception: str | None = None
    waiting_since: float | None = None
    failed_moves: list[str] = field(default_factory=list)

    @property
    def cpus(self) -> int:
        return self.task.peak_resources.cpus

    @property
    def gpus(self) -> int:
        return self.task.peak_resources.gpus

    @property
    def memory_gb(self) -> float:
        return self.task.peak_resources.memory_mb / 1024

    def to_json(self) -> dict[str, Any]:
        return {
            "task": self.task.id,
            "attempt": self.attempt,
            "job": self.job,
            "state": self.state,
            "reason": self.reason,
            "retries": self.retries,
            "resumes": self.resumes,
            "pid": self.pid,
            "started_at": self.started_at,
            "finished_at": self.finished_at,
            "reward": self.reward,
            "exception": self.exception,
            "cpus": self.cpus,
            "memory_mb": self.task.peak_resources.memory_mb,
            "gpus": self.gpus,
            "failed_moves": self.failed_moves,
        }


@dataclass
class JobState:
    """What a job directory says about its trial."""

    kind: str  # new | finished | setup_timeout | interrupted | incomplete | refused
    reward: float | None = None
    exception: str | None = None
    reason: str | None = None


def _read_json(path: Path) -> dict[str, Any] | None:
    try:
        return json.loads(path.read_text())
    except (OSError, json.JSONDecodeError):
        return None


def inspect_job(job_dir: Path) -> JobState:
    """Classify a job directory from its trials' ``result.json`` files."""
    if not job_dir.is_dir():
        return JobState("new")
    from .runner import trial_dirs

    trials = trial_dirs(job_dir)
    states: list[JobState] = []
    for trial in trials:
        result = _read_json(trial / "result.json")
        if result is None:
            states.append(JobState("incomplete"))
            continue
        exception = (result.get("exception_info") or {}).get("exception_type")
        rewards = (result.get("verifier_result") or {}).get("rewards") or {}
        reward = rewards.get("reward")
        if exception == CANCELLED:
            states.append(JobState("interrupted", exception=exception))
        elif exception in SETUP_TIMEOUTS:
            states.append(JobState("setup_timeout", exception=exception))
        else:
            states.append(
                JobState(
                    "finished",
                    reward=float(reward) if isinstance(reward, (int, float)) else None,
                    exception=exception,
                )
            )
    for kind in ("finished", "setup_timeout", "interrupted", "incomplete"):
        for state in states:
            if state.kind == kind:
                return state
    refusals = TrialPaths(job_dir).tbench_dir / "refusals"
    if refusals.is_dir():
        latest = sorted(refusals.glob("*.json"))
        if latest:
            record = _read_json(latest[-1]) or {}
            return JobState("refused", reason=record.get("reason"))
    return JobState("new")


def running_pid(job: str) -> int | None:
    """The pid of a live ``tbench`` process working on ``job``, if any.

    Reads ``/proc`` so a restarted scheduler finds the trials it started
    before, even when its status file is gone.
    """
    proc = Path("/proc")
    if not proc.is_dir():
        return None
    for entry in proc.iterdir():
        if not entry.name.isdigit():
            continue
        try:
            argv = (entry / "cmdline").read_bytes().split(b"\0")
        except OSError:
            continue
        words = [word.decode(errors="replace") for word in argv if word]
        if "tbench" not in " ".join(words[:4]):
            continue
        if "--job-name" in words:
            index = words.index("--job-name")
            if index + 1 < len(words) and words[index + 1] == job:
                return int(entry.name)
    return None


def pid_alive(pid: int) -> bool:
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    # A zombie child still answers kill(0); it has exited.
    try:
        stat = Path(f"/proc/{pid}/stat").read_text()
        return stat.rsplit(")", 1)[1].split()[0] != "Z"
    except (OSError, IndexError):
        return True


@dataclass
class Host:
    """The host questions the scheduler asks, replaceable in tests."""

    gpu_refusal: Callable[[str], str | None]
    free_disk_gb: Callable[[], float]
    # The built images of a task's trials that exist now.
    task_images: Callable[[Task], list[str]]
    remove_images: Callable[[list[str]], list[str]]
    prune_build_cache: Callable[[], bool]

    @classmethod
    def docker(cls, checkout: Path) -> Host:
        info = host.docker_info()
        root = host.docker_root(info)

        return cls(
            gpu_refusal=lambda task_id: host.gpu_refusal(task_id, info),
            free_disk_gb=lambda: host.free_disk_gb(root),
            task_images=lambda task: host.task_images(task.id),
            remove_images=host.remove_images,
            prune_build_cache=host.prune_build_cache,
        )


CLAUDE_CREDENTIALS = Path.home() / ".claude" / ".credentials.json"


def fresh_environment(
    base: Mapping[str, str], credentials: Path = CLAUDE_CREDENTIALS
) -> dict[str, str]:
    """The environment for one job, with a current Claude access token.

    A suite runs for many hours, and a subscription access token lasts
    about eight. When the scheduler's environment carries
    `CLAUDE_CODE_OAUTH_TOKEN`, each job gets the token the Claude CLI's
    credential file holds now, which the CLI refreshes, rather than the one
    the scheduler started with. The value is never logged.
    """
    env = dict(base)
    if "CLAUDE_CODE_OAUTH_TOKEN" not in env:
        return env
    try:
        token = json.loads(credentials.read_text())["claudeAiOauth"]["accessToken"]
    except (OSError, ValueError, KeyError, TypeError):
        return env
    if isinstance(token, str) and token:
        env["CLAUDE_CODE_OAUTH_TOKEN"] = token
    return env


class Launcher:
    """Starts ``tbench run``/``tbench resume`` for one job, detached."""

    def __init__(
        self,
        *,
        profile: str,
        arm: str,
        logs: Path,
        extra_args: list[str] | None = None,
        python: str = sys.executable,
    ) -> None:
        self.profile = profile
        self.arm = arm
        self.logs = logs
        self.extra_args = list(extra_args or [])
        self.python = python
        self.children: dict[int, subprocess.Popen[bytes]] = {}

    def command(self, trial: Trial, verb: str) -> list[str]:
        return [
            self.python,
            "-m",
            "tbench",
            verb,
            "--profile",
            self.profile,
            "--agent",
            self.arm,
            "--task",
            trial.task.id,
            "--job-name",
            trial.job,
            *self.extra_args,
        ]

    def start(self, trial: Trial, verb: str) -> int:
        self.logs.mkdir(parents=True, exist_ok=True)
        log = (self.logs / f"{trial.job}.log").open("ab")
        log.write(f"\n== {utc_now()} tbench {verb} {trial.job}\n".encode())
        log.flush()
        # A session of its own: the trial outlives a scheduler restart, and
        # a Ctrl-C at the scheduler's terminal doesn't reach it twice.
        process = subprocess.Popen(
            self.command(trial, verb),
            stdout=log,
            stderr=subprocess.STDOUT,
            stdin=subprocess.DEVNULL,
            start_new_session=True,
            cwd=paths.PACKAGE_DIR,
            env=fresh_environment(os.environ),
        )
        log.close()
        self.children[process.pid] = process
        return process.pid

    def alive(self, pid: int) -> bool:
        child = self.children.get(pid)
        if child is not None:
            if child.poll() is None:
                return True
            del self.children[pid]
            return False
        return pid_alive(pid)

    def interrupt(self, pid: int) -> None:
        with contextlib.suppress(ProcessLookupError):
            os.kill(pid, signal.SIGINT)


class SuiteError(RuntimeError):
    pass


@contextlib.contextmanager
def suite_lock(directory: Path) -> Iterator[None]:
    """One scheduler per suite; a second start is refused, not queued."""
    directory.mkdir(parents=True, exist_ok=True)
    handle = (directory / "lock").open("a+")
    try:
        fcntl.flock(handle, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError as exc:
        handle.seek(0)
        holder = handle.read().strip() or "unknown"
        handle.close()
        raise SuiteError(
            f"a scheduler already runs this suite (pid {holder}); "
            "`tbench suite status` shows it and `tbench suite stop` ends it"
        ) from exc
    handle.seek(0)
    handle.truncate()
    handle.write(str(os.getpid()))
    handle.flush()
    try:
        yield
    finally:
        fcntl.flock(handle, fcntl.LOCK_UN)
        handle.close()


class Scheduler:
    """Fits trials into the budgets, starts them, and records every change."""

    def __init__(
        self,
        *,
        profile: str,
        arm: str,
        pin: dict[str, Any],
        tasks: list[Task],
        attempts: int,
        budget: Budget,
        jobs_dir: Path,
        directory: Path,
        launcher: Launcher,
        host_: Host,
        clock: Callable[[], float] = time.monotonic,
        find_running: Callable[[str], int | None] = running_pid,
        failed: Path | None = None,
    ) -> None:
        self.profile = profile
        self.arm = arm
        self.pin = pin
        self.budget = budget
        self.jobs_dir = jobs_dir
        self.directory = directory
        self.launcher = launcher
        self.host = host_
        self.clock = clock
        self.find_running = find_running
        self.failed = failed or failed_dir()
        self.trials = [
            Trial(task, attempt, job_name(profile, arm, task.id, attempt))
            for task in tasks
            for attempt in range(1, attempts + 1)
        ]
        self.stopping = False
        self.events: list[dict[str, Any]] = []
        self.pruned: list[dict[str, Any]] = []
        self.pruned_tasks: set[str] = set()
        self.started_at = utc_now()
        self.free_gb: float | None = None
        # Print each event as it happens; a plan turns this off.
        self.echo = True

    # -- recording ---------------------------------------------------------

    def event(self, message: str, **fields: Any) -> None:
        record = {"at": utc_now(), "message": message, **fields}
        self.events.append(record)
        self.events = self.events[-200:]
        if self.echo:
            print(f"[{record['at']}] {message}", flush=True)

    def status(self) -> dict[str, Any]:
        counts: dict[str, int] = {}
        for trial in self.trials:
            counts[trial.state] = counts.get(trial.state, 0) + 1
        running = [t for t in self.trials if t.state == RUNNING]
        finished = [t for t in self.trials if t.state == FINISHED]
        passes = sum(1 for t in finished if t.reward is not None and t.reward >= 1.0)
        return {
            "schema": STATUS_SCHEMA,
            "profile": self.profile,
            "arm": self.arm,
            "pin": self.pin,
            "budget": self.budget.to_json(),
            "scheduler_pid": os.getpid(),
            "state": (
                "done"
                if all(t.state in TERMINAL for t in self.trials)
                else ("stopping" if self.stopping else "running")
            ),
            "started_at": self.started_at,
            "updated_at": utc_now(),
            "counts": counts,
            "passes": passes,
            "graded": sum(1 for t in finished if t.reward is not None),
            "in_use": {
                "cpus": sum(t.cpus for t in running),
                "memory_gb": round(sum(t.memory_gb for t in running), 2),
                "gpus": sum(t.gpus for t in running),
                "trials": len(running),
            },
            "free_disk_gb": None if self.free_gb is None else round(self.free_gb, 1),
            "pruned": self.pruned,
            "trials": [t.to_json() for t in self.trials],
            "events": self.events[-50:],
        }

    def write_status(self) -> Path:
        self.directory.mkdir(parents=True, exist_ok=True)
        path = self.directory / "status.json"
        staging = path.with_suffix(".json.tmp")
        staging.write_text(json.dumps(self.status(), indent=2) + "\n")
        staging.replace(path)
        return path

    # -- reading the job directories ---------------------------------------

    def _apply(self, trial: Trial, state: JobState) -> None:
        trial.exception = state.exception
        if state.kind == "finished":
            trial.state = FINISHED
            trial.reward = state.reward
            trial.reason = None
        elif state.kind == "refused":
            trial.state = REFUSED
            trial.reason = state.reason

    def reconcile(self) -> None:
        """Set every trial's state from its job dir and live processes."""
        for trial in self.trials:
            if trial.task.requires_gpu_runtime:
                reason = self.host.gpu_refusal(trial.task.id)
                if reason:
                    trial.state = SKIPPED
                    trial.reason = reason
                    continue
            pid = self.find_running(trial.job)
            if pid is not None:
                trial.state = RUNNING
                trial.pid = pid
                trial.started_at = trial.started_at or utc_now()
                self.event(f"adopted running trial {trial.job} (pid {pid})")
                continue
            state = inspect_job(self.jobs_dir / trial.job)
            if state.kind == "finished":
                self._apply(trial, state)
            elif state.kind == "setup_timeout":
                self._move_aside(trial, state)
            else:
                # New, interrupted, incomplete, or refused on an earlier
                # start: all run again now. A refusal is retried because
                # the operator may have fixed what it named.
                trial.state = PENDING

    def _move_aside(self, trial: Trial, state: JobState) -> None:
        """Move a setup timeout to ``failed/`` and queue one retry."""
        job_dir = self.jobs_dir / trial.job
        target = self.failed / f"{trial.job}-setup-timeout-{int(time.time())}"
        suffix = 1
        while target.exists():
            target = target.with_name(f"{target.name}-{suffix}")
            suffix += 1
        self.failed.mkdir(parents=True, exist_ok=True)
        if job_dir.exists():
            job_dir.rename(target)
        held = job_dir.parent / f".tbench-held--{job_dir.name}"
        if held.exists():
            held.rename(target.with_name(target.name + "-held"))
        trial.failed_moves.append(str(target))
        # Earlier moves count too, so a restart doesn't retry forever.
        trial.retries = len(trial.failed_moves) + self._earlier_moves(trial)
        if trial.retries > 1:
            trial.state = FAILED
            trial.exception = state.exception
            trial.reason = (
                f"{state.exception} twice; a setup timeout is never a result, "
                "and the retry also timed out"
            )
            self.event(f"{trial.job}: second setup timeout, giving up")
        else:
            trial.state = PENDING
            trial.reason = f"retry after {state.exception}"
            self.event(f"{trial.job}: {state.exception}, moved to {target}; retrying once")

    def _earlier_moves(self, trial: Trial) -> int:
        if not self.failed.is_dir():
            return 0
        prefix = f"{trial.job}-setup-timeout-"
        current = {Path(p).name for p in trial.failed_moves}
        return sum(
            1
            for path in self.failed.iterdir()
            if path.name.startswith(prefix)
            and not path.name.endswith("-held")
            and path.name not in current
        )

    # -- the loop ------------------------------------------------------------

    def _fits(self, trial: Trial, running: list[Trial]) -> bool:
        if self.budget.max_concurrent is not None and len(running) >= self.budget.max_concurrent:
            return False
        gpus_in_use = sum(t.gpus for t in running)
        if trial.gpus and gpus_in_use and gpus_in_use + trial.gpus > self.budget.max_gpus:
            return False
        if not running:
            return True
        cpus = sum(t.cpus for t in running) + trial.cpus
        memory = sum(t.memory_gb for t in running) + trial.memory_gb
        return cpus <= self.budget.max_cpus and memory <= self.budget.max_mem_gb

    def _image_gate(self, trial: Trial, running: list[Trial]) -> bool:
        """Start a task's later attempts once its first build is done.

        Each trial builds the task's image from the same Dockerfile, and
        attempts started together would all build it from nothing. The
        first attempt builds it; the others wait until a built image of the
        task exists, so they build from Docker's layer cache, or until no
        attempt of the task is running.
        """
        siblings = [t for t in running if t.task.id == trial.task.id]
        if not siblings:
            return True
        return bool(self.host.task_images(trial.task))

    def _disk_ok(self) -> bool:
        self.free_gb = self.host.free_disk_gb()
        if self.free_gb >= self.budget.min_free_disk_gb:
            return True
        self.prune(force=True)
        self.free_gb = self.host.free_disk_gb()
        return self.free_gb >= self.budget.min_free_disk_gb

    def launch_ready(self) -> list[Trial]:
        """Start every pending trial that fits now, largest first."""
        if self.stopping:
            return []
        started: list[Trial] = []
        now = self.clock()
        pending = sorted(
            (t for t in self.trials if t.state == PENDING),
            key=lambda t: (-t.cpus, -t.memory_gb, t.task.id, t.attempt),
        )
        for trial in pending:
            if trial.waiting_since is None:
                trial.waiting_since = now
        disk_checked = False
        for trial in pending:
            running = [t for t in self.trials if t.state == RUNNING]
            if not self._fits(trial, running) or not self._image_gate(trial, running):
                if now - (trial.waiting_since or now) >= self.budget.reserve_after_sec:
                    # Hold the rest back so this trial gets its turn.
                    break
                continue
            if not disk_checked:
                disk_checked = True
                if not self._disk_ok():
                    self.event(
                        f"free disk {self.free_gb:.1f} GiB is under the "
                        f"{self.budget.min_free_disk_gb} GiB floor; waiting"
                    )
                    break
            verb = "resume" if inspect_job(self.jobs_dir / trial.job).kind in (
                "interrupted",
                "incomplete",
            ) else "run"
            trial.pid = self.launcher.start(trial, verb)
            trial.state = RUNNING
            trial.started_at = utc_now()
            trial.finished_at = None
            trial.waiting_since = None
            started.append(trial)
            self.event(
                f"started {trial.job} ({verb}, {trial.cpus} CPUs, "
                f"{trial.memory_gb:g} GiB, pid {trial.pid})"
            )
        return started

    def poll(self) -> list[Trial]:
        """Settle every running trial whose process exited."""
        settled = []
        for trial in self.trials:
            if trial.state != RUNNING or trial.pid is None:
                continue
            if self.launcher.alive(trial.pid):
                continue
            settled.append(trial)
            trial.pid = None
            trial.finished_at = utc_now()
            state = inspect_job(self.jobs_dir / trial.job)
            if state.kind == "finished":
                self._apply(trial, state)
                self.event(
                    f"finished {trial.job}: reward {state.reward}"
                    + (f", {state.exception}" if state.exception else "")
                )
            elif state.kind == "setup_timeout":
                self._move_aside(trial, state)
            elif state.kind == "refused":
                self._apply(trial, state)
                self.event(f"refused {trial.job}: {state.reason}")
            elif self.stopping:
                trial.state = PENDING
                trial.reason = "interrupted by a scheduler stop; resumes on the next start"
            else:
                trial.resumes += 1
                if trial.resumes > MAX_RESUMES:
                    trial.state = FAILED
                    trial.reason = (
                        f"exited {MAX_RESUMES + 1} times without a finished "
                        f"result ({state.kind}); see the job's log under "
                        f"{self.launcher.logs}"
                    )
                    self.event(f"giving up on {trial.job}: {trial.reason}")
                else:
                    trial.state = PENDING
                    trial.reason = f"exited without a finished result ({state.kind}); resuming"
                    self.event(f"{trial.job}: {trial.reason}")
        if settled:
            self.prune()
        return settled

    def prune(self, *, force: bool = False) -> None:
        """Free disk near the floor: finished tasks' leftover images first.

        Harbor removes a trial's image when the trial ends, so what stays is
        an image a crashed trial left, the base images, and Docker's build
        cache. Near the floor this removes a finished task's leftover
        images; under the floor it also drops the unused build cache.
        """
        free = self.host.free_disk_gb()
        threshold = self.budget.min_free_disk_gb + self.budget.prune_margin_gb
        if free >= threshold and not force:
            return
        done: dict[str, Task] = {}
        for trial in self.trials:
            done.setdefault(trial.task.id, trial.task)
        for trial in self.trials:
            if trial.state not in TERMINAL:
                done.pop(trial.task.id, None)
        for task_id, task in sorted(done.items()):
            if task_id in self.pruned_tasks:
                continue
            images = self.host.task_images(task)
            removed = self.host.remove_images(images) if images else []
            self.pruned_tasks.add(task_id)
            after = self.host.free_disk_gb()
            self.pruned.append(
                {
                    "task": task_id,
                    "images": removed,
                    "at": utc_now(),
                    "free_before_gb": round(free, 1),
                    "free_after_gb": round(after, 1),
                }
            )
            self.event(f"pruned {task_id}: {len(removed)} images, {after:.1f} GiB free")
            free = after
            if free >= threshold:
                return
        if force and free < self.budget.min_free_disk_gb:
            self.host.prune_build_cache()
            self.event("pruned the Docker build cache")

    def stop(self) -> None:
        """Stop starting trials and interrupt the running ones once."""
        if self.stopping:
            return
        self.stopping = True
        self.event("stopping: interrupting running trials; a restart resumes them")
        for trial in self.trials:
            if trial.state == RUNNING and trial.pid is not None:
                self.launcher.interrupt(trial.pid)

    def done(self) -> bool:
        if self.stopping:
            return not any(t.state == RUNNING for t in self.trials)
        return all(t.state in TERMINAL for t in self.trials)

    def run(self, *, interval: float = 15.0, sleep: Callable[[float], None] = time.sleep) -> dict[str, Any]:
        self.reconcile()
        self.write_status()
        while True:
            self.poll()
            self.launch_ready()
            self.write_status()
            if self.done():
                break
            sleep(interval)
        self.event("suite stopped" if self.stopping else "suite finished")
        self.write_status()
        return self.status()


def read_status(profile: str, arm: str) -> dict[str, Any] | None:
    return _read_json(suite_dir(profile, arm) / "status.json")


def status_lines(status: dict[str, Any]) -> list[str]:
    """The status file as a short text summary."""
    counts = status.get("counts") or {}
    in_use = status.get("in_use") or {}
    budget = status.get("budget") or {}
    lines = [
        f"{status.get('profile')} / {status.get('arm')}: {status.get('state')} "
        f"(pid {status.get('scheduler_pid')}, updated {status.get('updated_at')})",
        "  "
        + " · ".join(f"{name} {counts[name]}" for name in sorted(counts))
        + f" · passes {status.get('passes', 0)}/{status.get('graded', 0)} graded",
        f"  in use: {in_use.get('trials', 0)} trials, {in_use.get('cpus', 0)}/"
        f"{budget.get('max_cpus')} CPUs, {in_use.get('memory_gb', 0)}/"
        f"{budget.get('max_mem_gb')} GiB · free disk "
        f"{status.get('free_disk_gb')} GiB (floor {budget.get('min_free_disk_gb')})",
    ]
    for trial in status.get("trials") or []:
        if trial["state"] in (RUNNING, SKIPPED, REFUSED, FAILED) or trial.get("reason"):
            detail = trial.get("reason") or (
                f"pid {trial.get('pid')} since {trial.get('started_at')}"
                if trial["state"] == RUNNING
                else ""
            )
            lines.append(f"  {trial['state']:<8} {trial['job']}: {detail}")
    for prune in status.get("pruned") or []:
        lines.append(
            f"  pruned {prune['task']}: {len(prune['images'])} images, "
            f"{prune['free_after_gb']} GiB free after"
        )
    return lines
