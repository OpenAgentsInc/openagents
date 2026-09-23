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
  than the whole budget is skipped, with the reason recorded, unless the
  budget allows oversize tasks; then it runs only when nothing else does.
- Order: pending trials start largest first by default, or smallest first,
  or in the order the profile lists its tasks.
- Disk: no trial starts while the Docker volume has less free space than
  the floor. When a task's trials have all finished and free space is
  within the prune margin of the floor, the task's leftover images go;
  under the floor, Docker's unused build cache goes too.
- GPUs: a task that needs a GPU is skipped, with the reason recorded, when
  Docker can't hand a container one (no NVIDIA CDI spec or runtime) or when
  the GPU budget is 0. GPU trials take GPU slots, one by default, so they
  run one at a time. A GPU trial also holds the host-wide GPU slot, a lock
  file under the state directory, for its whole life, so schedulers for
  different arms never run two GPU trials on the host at once.

A setup timeout is never a result. The scheduler moves that job directory
to ``failed/`` and runs the trial once more, as the runbook does by hand.

A usage limit is never a result either. When a finished trial's Claude or
Codex session was throttled by its provider (``tbench.usage_limit``), the
scheduler moves the job directory to ``failed/`` with a ``-usage-limit-``
suffix, queues the trial again, and pauses every arm on that provider until
the limit resets: the reset time the session stated, or a backoff when it
stated none. The pause is a host-wide file under the state directory, so
every scheduler on the host honors it. Claude trials also take one of a few
host-wide Claude slots (``--max-claude-concurrent``, 2 by default), lock
files like the GPU slot, because every suite draws on one subscription.
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

from . import credentials, host, paths, usage_limit
from .panel import Task
from .results import TrialPaths

STATUS_SCHEMA = "openagents.tbench.suite-status.v1"
PAUSES_SCHEMA = "openagents.tbench.usage-pauses.v1"

# A usage limit's pause runs this long past the reset time it states, so a
# trial doesn't start in the minute the quota comes back.
RESET_MARGIN_SEC = 60

# Exceptions that end a trial before the agent did any work. Harbor's
# names; the trial is retried once after its job dir moves to failed/.
SETUP_TIMEOUTS = frozenset({"AgentSetupTimeoutError", "EnvironmentStartTimeoutError"})


def setup_failed(result: dict[str, Any], exception: str | None) -> bool:
    """Whether a trial ended before its agent ran: a setup timeout, or any
    exception, such as a registry reset while the environment built, in a
    result whose agent execution Harbor recorded as never started."""
    if exception in SETUP_TIMEOUTS:
        return True
    return bool(
        exception
        and "agent_execution" in result
        and result["agent_execution"] is None
        and not (result.get("verifier_result") or {}).get("rewards")
    )
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

# The orders pending trials start in.
ORDERS = ("largest", "smallest", "listed")


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


def gpu_lock_path() -> Path:
    """The host-wide GPU slot every scheduler on this host shares."""
    return paths.state_dir() / "gpu.lock"


def acquire_gpu_slot(lock: Path, job: str) -> int | None:
    """Take the host-wide GPU slot for ``job``, or ``None`` when it's held.

    Returns a file descriptor holding an exclusive ``flock`` on ``lock``.
    The launcher passes it to the trial's process and the scheduler then
    closes its own copy, so the lock lasts exactly as long as the trial's
    ``tbench`` process: the kernel releases it when that process exits,
    whether or not a scheduler still runs. The file names the holder, for
    an operator who wants to know.
    """
    lock.parent.mkdir(parents=True, exist_ok=True)
    fd = os.open(lock, os.O_RDWR | os.O_CREAT, 0o644)
    try:
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        os.close(fd)
        return None
    os.ftruncate(fd, 0)
    holder = {"job": job, "scheduler_pid": os.getpid(), "at": utc_now()}
    os.write(fd, (json.dumps(holder) + "\n").encode())
    return fd


def claude_slot_dir() -> Path:
    """Where the host-wide Claude slots' lock files live."""
    return paths.state_dir() / "claude-slots"


def acquire_claude_slot(directory: Path, job: str, slots: int) -> int | None:
    """Take one of ``slots`` host-wide Claude slots for ``job``, or ``None``.

    Slot ``i`` is the lock file ``claude-<i>.lock``; like the GPU slot, the
    trial's process keeps the descriptor, so the slot lasts as long as the
    trial whether or not a scheduler still runs.
    """
    for index in range(slots):
        fd = acquire_gpu_slot(directory / f"claude-{index}.lock", job)
        if fd is not None:
            return fd
    return None


def claude_slot_holders(directory: Path, slots: int) -> list[dict[str, Any]]:
    """Who holds each busy host-wide Claude slot."""
    held = []
    for index in range(slots):
        holder = gpu_slot_holder(directory / f"claude-{index}.lock")
        if holder:
            held.append(holder)
    return held


def pause_path() -> Path:
    """The host-wide provider pauses every scheduler reads."""
    return paths.state_dir() / "usage-pauses.json"


def read_pauses(path: Path) -> dict[str, dict[str, Any]]:
    """Each paused provider's pause: ``until`` (epoch seconds) and why."""
    data = _read_json(path) or {}
    providers = data.get("providers")
    return providers if isinstance(providers, dict) else {}


def record_pause(
    path: Path,
    provider: str,
    until: float,
    *,
    reason: str,
    job: str,
) -> dict[str, Any]:
    """Pause ``provider`` until ``until``, keeping a later pause already set."""
    path.parent.mkdir(parents=True, exist_ok=True)
    with (path.parent / f".{path.name}.lock").open("a+") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        providers = read_pauses(path)
        current = providers.get(provider) or {}
        if float(current.get("until") or 0) < until:
            providers[provider] = {
                "until": int(until),
                "until_iso": datetime.fromtimestamp(until, tz=timezone.utc).isoformat(
                    timespec="seconds"
                ),
                "reason": reason,
                "job": job,
                "set_at": utc_now(),
            }
        staging = path.with_suffix(".json.tmp")
        staging.write_text(
            json.dumps({"schema": PAUSES_SCHEMA, "providers": providers}, indent=2) + "\n"
        )
        staging.replace(path)
        return providers[provider]


def gpu_slot_holder(lock: Path) -> dict[str, Any] | None:
    """Who holds the host-wide GPU slot, or ``None`` when it's free."""
    if not lock.exists():
        return None
    fd = os.open(lock, os.O_RDONLY)
    try:
        try:
            fcntl.flock(fd, fcntl.LOCK_SH | fcntl.LOCK_NB)
        except BlockingIOError:
            return _read_json(lock) or {"job": "unknown"}
        fcntl.flock(fd, fcntl.LOCK_UN)
        return None
    finally:
        os.close(fd)


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
    # The order pending trials start in: largest, smallest, or listed.
    order: str = "largest"
    # Run a task larger than the whole budget alone, rather than skip it.
    allow_oversize: bool = False
    # Trials at once, across every suite on the host, whose arm runs
    # Claude: one subscription serves them all. 0 turns the cap off.
    max_claude_concurrent: int = 2
    # How long a provider pauses after a usage limit that states no reset.
    usage_backoff_sec: float = usage_limit.DEFAULT_BACKOFF_SEC

    def __post_init__(self) -> None:
        if self.order not in ORDERS:
            raise ValueError(
                f"order must be one of {', '.join(ORDERS)}, not {self.order!r}"
            )

    def to_json(self) -> dict[str, Any]:
        return {
            "max_cpus": self.max_cpus,
            "max_mem_gb": self.max_mem_gb,
            "min_free_disk_gb": self.min_free_disk_gb,
            "prune_margin_gb": self.prune_margin_gb,
            "max_concurrent": self.max_concurrent,
            "max_gpus": self.max_gpus,
            "reserve_after_sec": self.reserve_after_sec,
            "order": self.order,
            "allow_oversize": self.allow_oversize,
            "max_claude_concurrent": self.max_claude_concurrent,
            "usage_backoff_sec": self.usage_backoff_sec,
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
    usage_limits: int = 0
    credential_failures: int = 0
    # The agent profile this trial runs, when a schedule has several arms.
    arm: str | None = None

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
            "usage_limits": self.usage_limits,
            "credential_failures": self.credential_failures,
            **({"arm": self.arm} if self.arm else {}),
        }


@dataclass
class JobState:
    """What a job directory says about its trial."""

    # new | finished | usage_limited | credentials | setup_timeout |
    # interrupted | incomplete | refused
    kind: str
    reward: float | None = None
    exception: str | None = None
    reason: str | None = None
    usage_limit: dict[str, Any] | None = None
    credential_failure: dict[str, Any] | None = None
    # When the trial finished, in epoch seconds, when its result says.
    finished_at: float | None = None


def _read_json(path: Path) -> dict[str, Any] | None:
    try:
        return json.loads(path.read_text())
    except (OSError, json.JSONDecodeError):
        return None


def _epoch(text: Any) -> float | None:
    """Epoch seconds of an ISO 8601 timestamp, or ``None``."""
    if not isinstance(text, str):
        return None
    try:
        return datetime.fromisoformat(text.replace("Z", "+00:00")).timestamp()
    except ValueError:
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
        limit = (
            usage_limit.trial_usage_limit(trial, result)
            if not setup_failed(result, exception) and exception != CANCELLED
            else None
        )
        if exception == CANCELLED:
            states.append(JobState("interrupted", exception=exception))
        elif limit is not None:
            states.append(
                JobState(
                    "usage_limited",
                    exception=exception,
                    usage_limit=limit,
                    finished_at=_epoch(result.get("finished_at")),
                )
            )
        elif setup_failed(result, exception):
            states.append(JobState("setup_timeout", exception=exception))
        elif (
            failure := credentials.trial_credential_failure(trial, result)
        ) is not None:
            states.append(
                JobState("credentials", exception=exception, credential_failure=failure)
            )
        else:
            states.append(
                JobState(
                    "finished",
                    reward=float(reward) if isinstance(reward, (int, float)) else None,
                    exception=exception,
                )
            )
    for kind in (
        "usage_limited",
        "credentials",
        "finished",
        "setup_timeout",
        "interrupted",
        "incomplete",
    ):
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
    # Takes the host-wide GPU slot for a job: a descriptor that holds it,
    # or None while another trial on the host holds it.
    gpu_slot: Callable[[str], int | None] = lambda job: acquire_gpu_slot(
        gpu_lock_path(), job
    )
    # Takes one of the host-wide Claude slots: a descriptor that holds it,
    # or None while every slot is held.
    claude_slot: Callable[[str, int], int | None] = lambda job, slots: acquire_claude_slot(
        claude_slot_dir(), job, slots
    )

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
CLAUDE_SETUP_TOKEN = credentials.SETUP_TOKEN


def fresh_environment(
    base: Mapping[str, str],
    login: Path = CLAUDE_CREDENTIALS,
    *,
    login_fallback: bool = True,
) -> dict[str, str]:
    """The environment for one job, with a current Claude access token.

    A suite runs for many hours, and a subscription access token lasts
    about eight. When the scheduler's environment carries
    `CLAUDE_CODE_OAUTH_TOKEN`, each job gets the token the Claude CLI's
    credential file holds now, which the CLI refreshes, rather than the one
    the scheduler started with. The value is never logged.

    With ``login_fallback`` off, only the long-lived token replaces the
    scheduler's value; a targeted experiment that started on the long-lived
    token never switches to the expiring login.
    """
    env = dict(base)
    if "CLAUDE_CODE_OAUTH_TOKEN" not in env:
        return env
    # A long-lived token from `claude setup-token` outlives the host's
    # login refreshes, which revoke the access token a running trial holds.
    long_lived = credentials.read_setup_token(CLAUDE_SETUP_TOKEN)
    if long_lived:
        env["CLAUDE_CODE_OAUTH_TOKEN"] = long_lived
        return env
    token = credentials.login_token(login) if login_fallback else ""
    if token:
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
        arm_args: Mapping[str, list[str]] | None = None,
        login_fallback: bool = True,
        arm_profiles: Mapping[str, str] | None = None,
    ) -> None:
        self.profile = profile
        self.arm = arm
        # An arm named apart from the agent profile it runs.
        self.arm_profiles = dict(arm_profiles or {})
        # Flags for one arm's trials only, when a schedule has several arms.
        self.arm_args = {name: list(flags) for name, flags in (arm_args or {}).items()}
        self.login_fallback = login_fallback
        self.logs = logs
        self.extra_args = list(extra_args or [])
        self.python = python
        self.children: dict[int, subprocess.Popen[bytes]] = {}

    def command(self, trial: Trial, verb: str) -> list[str]:
        arm = trial.arm or self.arm
        return [
            self.python,
            "-m",
            "tbench",
            verb,
            "--profile",
            self.profile,
            "--agent",
            self.arm_profiles.get(arm, arm),
            "--task",
            trial.task.id,
            "--job-name",
            trial.job,
            *self.extra_args,
            *self.arm_args.get(arm, []),
        ]

    def start(
        self, trial: Trial, verb: str, hold: int | tuple[int, ...] | None = None
    ) -> int:
        """Start the job; ``hold`` is descriptors the trial keeps open."""
        holds = (hold,) if isinstance(hold, int) else tuple(hold or ())
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
            env=fresh_environment(os.environ, login_fallback=self.login_fallback),
            pass_fds=holds,
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
        providers: frozenset[str] = frozenset(),
        pauses: Path | None = None,
        wall: Callable[[], float] = time.time,
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
        # The providers this arm's sessions bill; a usage limit on one
        # pauses the arm, and `anthropic` takes a host-wide Claude slot.
        self.providers = frozenset(providers)
        self.pauses = pauses or pause_path()
        self.wall = wall
        self.paused_note: str | None = None
        # Providers whose credentials failed a trial: no trial that bills
        # one starts again until the operator fixes them and restarts.
        self.blocked: dict[str, str] = {}
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
                else (
                    "stopping"
                    if self.stopping
                    else ("held" if self.done() else "running")
                )
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
            "providers": sorted(self.providers),
            "paused": self.paused(),
            "usage_limited": sum(t.usage_limits for t in self.trials),
            "credential_failures": sum(t.credential_failures for t in self.trials),
            "blocked": dict(self.blocked),
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
            elif state.kind == "usage_limited":
                self._usage_limited(trial, state)
            elif state.kind == "credentials":
                # Found on a restart, after the operator fixed what the
                # failure named: set it aside and run the trial again.
                self._credential_failure(trial, state, block=False)
            else:
                # New, interrupted, incomplete, or refused on an earlier
                # start: all run again now. A refusal is retried because
                # the operator may have fixed what it named.
                trial.state = PENDING
            if trial.state == PENDING:
                reason = self.budget_refusal(trial)
                if reason:
                    trial.state = SKIPPED
                    trial.reason = reason
                    self.event(f"skipped {trial.job}: {reason}")

    def budget_refusal(self, trial: Trial) -> str | None:
        """Why the budget never lets ``trial`` start, or ``None``.

        A GPU task never starts under a GPU budget of 0, whatever else the
        budget allows. A task larger than the whole CPU, memory, or GPU
        budget starts, alone, only when the budget allows oversize tasks.
        """
        budget = self.budget
        if trial.gpus and budget.max_gpus <= 0:
            return f"needs {trial.gpus} GPU and the GPU budget is 0 (--max-gpus 0)"
        over = []
        if trial.cpus > budget.max_cpus:
            over.append(f"{trial.cpus} CPUs over the {budget.max_cpus:g}-CPU budget")
        if trial.memory_gb > budget.max_mem_gb:
            over.append(
                f"{trial.memory_gb:g} GiB over the {budget.max_mem_gb:g} GiB budget"
            )
        if trial.gpus > budget.max_gpus:
            over.append(f"{trial.gpus} GPUs over the {budget.max_gpus}-GPU budget")
        if over and not budget.allow_oversize:
            return (
                "larger than the whole budget ("
                + "; ".join(over)
                + "); --allow-oversize runs it alone"
            )
        return None

    def _set_aside(self, trial: Trial, label: str) -> Path:
        """Move the trial's job dir to ``failed/`` under ``label``."""
        job_dir = self.jobs_dir / trial.job
        target = self.failed / f"{trial.job}-{label}-{int(time.time())}"
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
        return target

    def _usage_limited(self, trial: Trial, state: JobState) -> None:
        """Set a throttled trial aside, queue it again, and pause its provider.

        A usage limit says nothing about the agent, so the trial always runs
        again; the pause keeps it, and every other trial on the provider,
        from starting until the limit resets.
        """
        limit = state.usage_limit or {}
        target = self._set_aside(trial, "usage-limit")
        trial.usage_limits += 1
        trial.state = PENDING
        trial.reward = None
        trial.exception = state.exception
        provider = limit.get("provider")
        providers = (
            {provider} if provider in usage_limit.PROVIDERS.values() else set(self.providers)
        )
        now = self.wall()
        resets = limit.get("resets_at")
        if isinstance(resets, (int, float)):
            until = float(resets) + RESET_MARGIN_SEC
            why = f"resets at {limit.get('resets_at_iso') or int(resets)}"
        else:
            # Counted from when the trial ended, so a limit a restarted
            # scheduler finds long after the fact doesn't pause anything.
            until = (state.finished_at or now) + self.budget.usage_backoff_sec
            why = (
                "no reset time stated; backing off "
                f"{self.budget.usage_backoff_sec / 60:g} min"
            )
        if until <= now:
            why += ", already past"
        else:
            for name in sorted(providers):
                record_pause(
                    self.pauses,
                    name,
                    until,
                    reason=f"{trial.job}: {limit.get('message') or 'usage limit'}",
                    job=trial.job,
                )
        trial.reason = (
            f"requeued after a {'/'.join(sorted(providers)) or 'provider'} usage "
            f"limit ({why})"
        )
        self.event(f"{trial.job}: usage limit, moved to {target}; {trial.reason}")

    def trial_providers(self, trial: Trial) -> frozenset[str]:
        """The providers one trial's sessions bill."""
        return self.providers

    def hold_reason(self, trial: Trial) -> str | None:
        """Why no trial from ``trial`` on in the order starts now, or ``None``."""
        blocked = sorted(self.trial_providers(trial) & set(self.blocked))
        if blocked:
            return "; ".join(f"{name} {self.blocked[name]}" for name in blocked)
        return None

    def _credential_failure(self, trial: Trial, state: JobState, *, block: bool = True) -> None:
        """Set a trial its credentials failed aside, and queue it again.

        A credential failure says nothing about the agent, so it is never a
        result. Credentials don't recover by waiting, so no further trial
        on the provider starts until the operator fixes them and restarts.
        """
        failure = state.credential_failure or {}
        target = self._set_aside(trial, "credentials")
        trial.credential_failures += 1
        trial.state = PENDING
        trial.reward = None
        trial.exception = state.exception
        trial.reason = f"requeued after a credential failure ({failure.get('message')})"
        if block:
            for name in sorted(self.trial_providers(trial)):
                self.blocked[name] = (
                    f"credentials failed in {trial.job} ({failure.get('source')}); "
                    "fix them and restart"
                )
        self.event(f"{trial.job}: credential failure, moved to {target}; {trial.reason}")

    def paused(self) -> dict[str, dict[str, Any]]:
        """This arm's providers that are paused now, with each pause."""
        now = self.wall()
        return {
            name: pause
            for name, pause in read_pauses(self.pauses).items()
            if name in self.providers and float(pause.get("until") or 0) > now
        }

    def _move_aside(self, trial: Trial, state: JobState) -> None:
        """Move a setup timeout to ``failed/`` and queue one retry."""
        self._set_aside(trial, "setup-timeout")
        target = trial.failed_moves[-1]
        # Earlier moves count too, so a restart doesn't retry forever.
        trial.retries = (
            sum(1 for move in trial.failed_moves if "-setup-timeout-" in Path(move).name)
            + self._earlier_moves(trial)
        )
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
        if self.budget_refusal(trial):
            return False
        if not running:
            # Alone, a trial fits: it's within every budget, or the budget
            # allows an oversize task to run alone.
            return True
        gpus_in_use = sum(t.gpus for t in running)
        if trial.gpus and gpus_in_use + trial.gpus > self.budget.max_gpus:
            return False
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

    def _order_key(self) -> Callable[[Trial], tuple]:
        if self.budget.order == "smallest":
            return lambda t: (t.cpus, t.memory_gb, t.task.id, t.attempt)
        if self.budget.order == "listed":
            listed = {id(t): index for index, t in enumerate(self.trials)}
            return lambda t: (listed[id(t)],)
        return lambda t: (-t.cpus, -t.memory_gb, t.task.id, t.attempt)

    def _disk_wait(self) -> None:
        self.event(
            f"free disk {self.free_gb:.1f} GiB is under the "
            f"{self.budget.min_free_disk_gb} GiB floor; waiting"
        )

    def launch_ready(self) -> list[Trial]:
        """Start every pending trial that fits now, in the budget's order."""
        if self.stopping:
            return []
        paused = self.paused()
        if paused:
            note = "; ".join(
                f"{name} paused until {pause.get('until_iso')}"
                for name, pause in sorted(paused.items())
            )
            if note != self.paused_note:
                self.paused_note = note
                self.event(f"not starting trials: {note}")
            return []
        if self.paused_note:
            self.paused_note = None
            self.event("usage-limit pause over; starting trials again")
        started: list[Trial] = []
        now = self.clock()
        pending = sorted(
            (t for t in self.trials if t.state == PENDING), key=self._order_key()
        )
        for trial in pending:
            if trial.waiting_since is None:
                trial.waiting_since = now
        disk_checked = False
        for trial in pending:
            held = self.hold_reason(trial)
            if held:
                if trial.reason != held:
                    trial.reason = held
                    self.event(f"not starting {trial.job}: {held}")
                break
            running = [t for t in self.trials if t.state == RUNNING]
            fits = self._fits(trial, running) and self._image_gate(trial, running)
            if fits and trial.gpus and not disk_checked:
                # Check the disk before taking the host-wide GPU slot.
                disk_checked = True
                if not self._disk_ok():
                    self._disk_wait()
                    break
            claude: int | None = None
            slots = self.budget.max_claude_concurrent
            if fits and slots > 0 and "anthropic" in self.trial_providers(trial):
                claude = self.host.claude_slot(trial.job, slots)
                if claude is None:
                    reason = (
                        f"waiting for a host-wide Claude slot ({slots} Claude "
                        "trials at once across every suite)"
                    )
                    if trial.reason != reason:
                        trial.reason = reason
                        self.event(f"{trial.job}: {reason}")
                    # Every pending trial of this arm needs a slot, so none
                    # starts until one frees.
                    break
            slot: int | None = None
            if fits and trial.gpus:
                slot = self.host.gpu_slot(trial.job)
                if slot is None:
                    if claude is not None:
                        os.close(claude)
                    reason = "waiting for the host-wide GPU slot another trial holds"
                    if trial.reason != reason:
                        trial.reason = reason
                        self.event(f"{trial.job}: {reason}")
                    # Another suite's trial may hold the slot for hours, so
                    # this wait never holds back this suite's backfill.
                    continue
            if not fits:
                if now - (trial.waiting_since or now) >= self.budget.reserve_after_sec:
                    # Hold the rest back so this trial gets its turn.
                    break
                continue
            if not disk_checked:
                disk_checked = True
                if not self._disk_ok():
                    if claude is not None:
                        os.close(claude)
                    self._disk_wait()
                    break
            verb = "resume" if inspect_job(self.jobs_dir / trial.job).kind in (
                "interrupted",
                "incomplete",
            ) else "run"
            holds = tuple(fd for fd in (slot, claude) if fd is not None)
            try:
                trial.pid = self.launcher.start(
                    trial, verb, hold=holds[0] if len(holds) == 1 else (holds or None)
                )
            finally:
                # The trial's process holds its own copy of each slot now.
                for fd in holds:
                    os.close(fd)
            if holds:
                trial.reason = None
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
            elif state.kind == "usage_limited":
                self._usage_limited(trial, state)
            elif state.kind == "credentials":
                self._credential_failure(trial, state)
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
        if all(t.state in TERMINAL for t in self.trials):
            return True
        # With nothing running, a hold can't lift by itself: the schedule
        # ends and a restart picks up the held trials.
        if any(t.state == RUNNING for t in self.trials):
            return False
        pending = [t for t in self.trials if t.state == PENDING]
        return bool(pending) and all(self.hold_reason(t) for t in pending)

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
    # A scheduler started before these settings existed doesn't record them.
    if "order" in budget:
        lines.append(
            f"  order {budget['order']} · GPUs {in_use.get('gpus', 0)}/"
            f"{budget.get('max_gpus')} · oversize "
            + ("runs alone" if budget.get("allow_oversize") else "skipped")
        )
    if "max_claude_concurrent" in budget:
        lines.append(
            f"  providers {', '.join(status.get('providers') or []) or 'none'} · "
            f"Claude slots {budget['max_claude_concurrent'] or 'uncapped'} · "
            f"usage-limited {status.get('usage_limited', 0)}"
        )
    for name, why in sorted((status.get("blocked") or {}).items()):
        lines.append(f"  blocked  {name}: {why}")
    for name, pause in sorted((status.get("paused") or {}).items()):
        lines.append(
            f"  paused   {name} until {pause.get('until_iso')}: {pause.get('reason')}"
        )
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
