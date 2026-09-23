"""``tbench replay`` and ``tbench verify``: rerun a stage without a model.

A trial's expensive part is the executor session. Everything after it,
the checks, the repair brief, and the verifier, reads only the workspace
the session left, so each can be rerun in seconds against that workspace:

- ``--stage checks`` starts a container from the task's kept image
  (``tbench.warm_docker``), restores the workspace, and runs
  ``coder-one snapshot checks`` with the trial's own check subject. Pass
  ``--artifact`` to run a changed build of Coder One.
- ``--stage repair`` does the same and also writes the repair brief the
  episode's policy would send.
- ``--stage verify`` runs the task's verifier through
  ``harbor trial regrade`` on the workspace's declared artifacts, in the
  kept verifier image.

The workspace is the trial's post-executor snapshot
(``agent/episode/snapshot/workspace.tar.gz``, from a policy with
``verify.snapshot``) when it has one. Otherwise it is the artifacts Harbor
collected when the trial ended, which is the final state after any repair,
and only the paths the task declares; every result names its source.

``tbench verify --task T --candidate DIR`` runs a task's verifier on any
directory, laid out as the directory at ``--mount`` (``/app`` by default).

Each replay writes ``replay.json`` under
``~/.openagents/terminal-bench/replays/<trial>--<stage>--<stamp>/`` with the
result and its loop time.
"""

from __future__ import annotations

import json
import shutil
import subprocess
import sys
import tarfile
import tempfile
import time
import uuid
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path, PurePosixPath
from typing import Any, Callable

from . import paths
from .warm_docker import image_exists, warm_tag

SCHEMA = "openagents.tbench.replay.v1"
STAGES = ("checks", "repair", "verify")
WARM_ENVIRONMENT = "tbench.warm_docker:WarmDockerEnvironment"
SNAPSHOT = PurePosixPath("agent/episode/snapshot/workspace.tar.gz")
EPISODE = Path("agent/episode")
# What the check replay reads from a bundle; the executor streams beyond
# the first and the survey stay home.
BUNDLE_FILES = (
    "manifest.json",
    "episode.atif.jsonl",
    "artifacts/requirements.json",
    "artifacts/state.json",
    "artifacts/composition.json",
    "artifacts/delegate-1.stream.jsonl",
    "verification/checks.json",
    "verification/support.json",
    "snapshot/subject.json",
    "snapshot/snapshot.json",
)
REMOTE = PurePosixPath("/opt/openagents/replay")


class ReplayError(RuntimeError):
    """A replay that couldn't run, with why."""


def replays_dir() -> Path:
    return paths.state_dir() / "replays"


def _read_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text())
    except (OSError, json.JSONDecodeError):
        return None


def find_trial(reference: str, jobs_dir: Path | None = None) -> Path:
    """A trial directory from a path, ``<job>/<trial>``, or a trial name."""
    path = Path(reference).expanduser()
    if (path / "config.json").is_file():
        return path
    jobs = jobs_dir or paths.jobs_dir()
    if (jobs / reference / "config.json").is_file():
        return jobs / reference
    matches = sorted(jobs.glob(f"*/{reference}/config.json"))
    if len(matches) == 1:
        return matches[0].parent
    if len(matches) > 1:
        raise ReplayError(
            f"{reference} names {len(matches)} trials; give <job>/<trial>"
        )
    raise ReplayError(f"no trial {reference} under {jobs}")


def replayable(trial: Path) -> bool:
    """Whether the trial left a workspace to replay: a snapshot, or at least
    one artifact Harbor collected."""
    if (trial / SNAPSHOT).is_file():
        return True
    entries = _read_json(trial / "artifacts" / "manifest.json")
    return isinstance(entries, list) and any(
        isinstance(e, dict)
        and e.get("status") == "ok"
        and not e.get("service")
        and not str(e.get("source") or "").startswith("/logs/")
        and (trial / str(e.get("destination") or "-")).exists()
        for e in entries
    )


def failing_trials(match: str, limit: int, jobs_dir: Path | None = None) -> list[Path]:
    """Graded Coder One trials with reward 0 in jobs whose name contains
    ``match``, newest first."""
    jobs = jobs_dir or paths.jobs_dir()
    found = []
    for result_path in jobs.glob("*/*/result.json"):
        trial = result_path.parent
        if match not in trial.parent.name:
            continue
        if not (trial / EPISODE / "episode.atif.jsonl").is_file():
            continue
        if not replayable(trial):
            continue
        result = _read_json(result_path) or {}
        reward = ((result.get("verifier_result") or {}).get("rewards") or {}).get(
            "reward"
        )
        if reward is None or reward >= 1:
            continue
        found.append((result.get("finished_at") or "", trial))
    found.sort(reverse=True)
    return [trial for _, trial in found[:limit]]


def task_dir(trial: Path) -> Path:
    """The task directory the trial ran, from its Harbor config."""
    config = _read_json(trial / "config.json") or {}
    raw = (config.get("task") or {}).get("path")
    if not raw or not Path(raw).is_dir():
        raise ReplayError(
            f"{trial.name}: its task directory {raw!r} isn't on this host; "
            "run `tbench tasks checkout --catalog tb4`"
        )
    return Path(raw)


def original_reward(trial: Path) -> float | None:
    result = _read_json(trial / "result.json") or {}
    return ((result.get("verifier_result") or {}).get("rewards") or {}).get("reward")


def trial_artifact(trial: Path) -> Path | None:
    """The Coder One binary the trial's job ran, from its job config."""
    config = _read_json(trial.parent / "tbench" / "job-config.json") or {}
    for agent in config.get("agents") or []:
        path = (agent.get("kwargs") or {}).get("artifact_path")
        if path and Path(path).is_file():
            return Path(path)
    return None


def workdir_of(trial: Path) -> str:
    """The task's working directory, as the episode recorded it."""
    subject = _read_json(trial / EPISODE / "snapshot" / "subject.json") or {}
    live = subject.get("live") or {}
    if live.get("dir"):
        return str(live["dir"])
    manifest = _read_json(trial / EPISODE / "manifest.json") or {}
    return str(manifest.get("workdir") or "/app")


def artifact_entries(trial: Path) -> list[dict[str, Any]]:
    entries = _read_json(trial / "artifacts" / "manifest.json")
    return entries if isinstance(entries, list) else []


@dataclass
class Workspace:
    """A candidate laid out from ``/``: ``root/app/x.py`` is ``/app/x.py``."""

    root: Path
    source: str  # snapshot | artifacts | candidate
    full: bool  # whether it replaces the workdir, or overlays the image
    note: str = ""


def _safe_extract(archive: Path, target: Path) -> None:
    with tarfile.open(archive) as tar:
        tar.extractall(target, filter="data")


def workspace_of(trial: Path, scratch: Path) -> Workspace:
    """The trial's post-executor snapshot, or its collected artifacts."""
    root = scratch / "root"
    root.mkdir(parents=True, exist_ok=True)
    snapshot = trial / SNAPSHOT
    if snapshot.is_file():
        _safe_extract(snapshot, root)
        return Workspace(root, "snapshot", True, "the workspace right after the first executor")
    copied = 0
    for entry in artifact_entries(trial):
        if entry.get("status") != "ok" or entry.get("service"):
            continue
        source = str(entry.get("source") or "")
        if not source.startswith("/") or source.startswith("/logs/"):
            continue
        host = trial / str(entry.get("destination") or "")
        target = root / source.lstrip("/")
        if host.is_dir():
            shutil.copytree(host, target, dirs_exist_ok=True)
            copied += 1
        elif host.is_file():
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(host, target)
            copied += 1
    if not copied:
        raise ReplayError(
            f"{trial.name} has no snapshot and Harbor collected no artifacts"
        )
    return Workspace(
        root,
        "artifacts",
        False,
        "the task's declared artifacts at the end of the trial, after any "
        "repair, over the task image's own files",
    )


def candidate_workspace(candidate: Path, mount: str, scratch: Path) -> Workspace:
    """A directory that holds what ``mount`` should hold."""
    if not candidate.is_dir():
        raise ReplayError(f"no candidate directory at {candidate}")
    root = scratch / "root"
    shutil.copytree(candidate, root / mount.strip("/"), dirs_exist_ok=True)
    return Workspace(root, "candidate", False, f"{candidate} as {mount}")


def _docker(args: list[str], timeout: int = 600, **kwargs: Any) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["docker", *args], capture_output=True, text=True, timeout=timeout, **kwargs
    )


def task_image(task: Path) -> tuple[str, str]:
    """The kept image of the task's environment, built when missing.

    Returns the tag and whether it was ``warm`` or built ``cold``.
    """
    from harbor.environments.definition import environment_content_hash

    environment = task / "environment"
    tag = warm_tag(task.name, "environment", environment_content_hash(environment))
    if image_exists(tag):
        return tag, "warm"
    built = _docker(["build", "-q", "-t", tag, str(environment)], timeout=3600)
    if built.returncode != 0:
        raise ReplayError(f"docker build of {task.name} failed: {built.stderr[-800:]}")
    return tag, "cold"


def bundle_copy(trial: Path, target: Path) -> Path:
    """The files of the trial's episode bundle the check replay reads."""
    episode = trial / EPISODE
    for name in BUNDLE_FILES:
        source = episode / name
        if source.is_file():
            (target / name).parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target / name)
    return target


@dataclass
class Replay:
    """One replay's result and timings."""

    trial: str
    stage: str
    source: str
    reward: float | None
    seconds: dict[str, float] = field(default_factory=dict)
    result: dict[str, Any] = field(default_factory=dict)
    out: Path | None = None
    error: str | None = None

    def record(self) -> dict[str, Any]:
        return {
            "schema": SCHEMA,
            "trial": self.trial,
            "stage": self.stage,
            "source": self.source,
            "original_reward": self.reward,
            "seconds": self.seconds,
            "result": self.result,
            "out": str(self.out) if self.out else None,
            "error": self.error,
        }


class Clock:
    def __init__(self) -> None:
        self.started = time.monotonic()
        self.mark = self.started
        self.seconds: dict[str, float] = {}

    def lap(self, name: str) -> None:
        now = time.monotonic()
        self.seconds[name] = round(now - self.mark, 2)
        self.mark = now

    def total(self) -> dict[str, float]:
        self.seconds["total"] = round(time.monotonic() - self.started, 2)
        return self.seconds


def replay_checks(
    trial: Path,
    *,
    repair: bool,
    artifact: Path | None,
    out: Path,
) -> Replay:
    """Run the checks, and with ``repair`` the repair brief, in the task's
    image against the trial's workspace."""
    clock = Clock()
    binary = artifact or trial_artifact(trial)
    if binary is None or not binary.is_file():
        raise ReplayError(
            f"{trial.name}: no Coder One artifact; pass --artifact (a static "
            "Linux build from scripts/build-coder-one-linux.sh)"
        )
    task = task_dir(trial)
    stage = "repair" if repair else "checks"
    with tempfile.TemporaryDirectory(prefix="tbench-replay-") as scratch_name:
        scratch = Path(scratch_name)
        workspace = workspace_of(trial, scratch)
        bundle = bundle_copy(trial, scratch / "bundle")
        clock.lap("prepare")
        image, cache = task_image(task)
        clock.lap("image")
        name = f"tbench-replay-{uuid.uuid4().hex[:10]}"
        started = _docker(["run", "-d", "--name", name, "--entrypoint", "sh", image, "-c", "sleep infinity"])
        if started.returncode != 0:
            raise ReplayError(f"docker run failed: {started.stderr[-800:]}")
        try:
            clock.lap("container")
            workdir = workdir_of(trial)
            if workspace.full:
                _docker(["exec", "-u", "0", name, "sh", "-c", f"rm -rf {workdir}/* {workdir}/.[!.]* 2>/dev/null; true"])
            copied = _docker(["cp", f"{workspace.root}/.", f"{name}:/"])
            if copied.returncode != 0:
                raise ReplayError(f"docker cp of the workspace failed: {copied.stderr[-800:]}")
            _docker(["exec", "-u", "0", name, "mkdir", "-p", str(REMOTE / "bin"), str(REMOTE / "out")])
            _docker(["cp", str(binary), f"{name}:{REMOTE / 'bin' / 'coder-one'}"])
            _docker(["cp", f"{bundle}/.", f"{name}:{REMOTE / 'bundle'}"])
            _docker(["exec", "-u", "0", name, "sh", "-c", f"chmod -R a+rwX {REMOTE}"])
            clock.lap("restore")
            command = [
                str(REMOTE / "bin" / "coder-one"),
                "snapshot",
                "checks",
                "--bundle",
                str(REMOTE / "bundle"),
                "--workdir",
                workdir,
                "--out",
                str(REMOTE / "out"),
                "--json",
            ]
            if repair:
                command.append("--repair")
            ran = _docker(["exec", "-w", workdir, name, *command], timeout=3600)
            clock.lap("checks")
            out.mkdir(parents=True, exist_ok=True)
            _docker(["cp", f"{name}:{REMOTE / 'out'}/.", str(out)])
            if ran.returncode != 0:
                raise ReplayError(
                    f"coder-one snapshot checks exited {ran.returncode}: "
                    f"{(ran.stderr or ran.stdout)[-800:]}"
                )
            try:
                result = json.loads(ran.stdout)
            except json.JSONDecodeError as exc:
                raise ReplayError(f"unreadable checks output: {exc}") from exc
        finally:
            _docker(["rm", "-f", name])
    result["image_cache"] = cache
    result["workspace"] = workspace.note
    replay = Replay(trial.name, stage, workspace.source, original_reward(trial), clock.total(), result, out)
    return replay


def _synthetic_source(
    target: Path, task: Path, workspace: Workspace, trial: Path | None
) -> Path:
    """A trial directory Harbor's regrade accepts, holding ``workspace``'s
    declared artifacts and nothing else."""
    from harbor.models.task.task import Task
    from harbor.models.trial.config import AgentConfig, TrialConfig
    from harbor.models.trial.config import TaskConfig as TrialTaskConfig
    from harbor.models.task.id import LocalTaskId
    from harbor.models.trial.result import AgentInfo, TrialResult

    target.mkdir(parents=True)
    if trial is not None:
        shutil.copy2(trial / "config.json", target / "config.json")
        shutil.copy2(trial / "result.json", target / "result.json")
    else:
        loaded = Task(task)
        name = f"candidate__{uuid.uuid4().hex[:7]}"
        config = TrialConfig(
            task=TrialTaskConfig(path=task),
            trial_name=name,
            trials_dir=target.parent,
            agent=AgentConfig(name="nop"),
        )
        result = TrialResult(
            task_name=loaded.name,
            trial_name=name,
            trial_uri=target.as_uri(),
            task_id=LocalTaskId(path=task),
            task_checksum="candidate",
            config=config,
            agent_info=AgentInfo(name="candidate", version="0"),
        )
        (target / "config.json").write_text(config.model_dump_json(indent=2))
        (target / "result.json").write_text(result.model_dump_json(indent=2))
    import tomllib

    declared = tomllib.loads((task / "task.toml").read_text()).get("artifacts") or []
    entries = [
        {
            "source": "/logs/artifacts",
            "destination": "artifacts/logs/artifacts",
            "type": "directory",
            "status": "empty",
            "service": None,
        }
    ]
    (target / "artifacts" / "logs" / "artifacts").mkdir(parents=True)
    for item in declared:
        source = item if isinstance(item, str) else str(item.get("source") or "")
        if not source or (isinstance(item, dict) and item.get("service")):
            continue
        host = workspace.root / source.strip("/")
        destination = f"artifacts/{source.strip('/')}"
        kind = "directory" if source.endswith("/") or host.is_dir() else "file"
        status = "empty"
        if host.is_dir():
            shutil.copytree(host, target / destination, dirs_exist_ok=True)
            status = "ok"
        elif host.is_file():
            (target / destination).parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(host, target / destination)
            status = "ok"
        entries.append(
            {
                "source": source,
                "destination": destination,
                "type": kind,
                "status": status,
                "service": None,
            }
        )
    (target / "artifacts" / "manifest.json").write_text(json.dumps(entries, indent=2))
    return target


def harbor_binary() -> str:
    beside = Path(sys.executable).parent / "harbor"
    return str(beside) if beside.exists() else (shutil.which("harbor") or "harbor")


def run_verifier(
    task: Path,
    workspace: Workspace,
    out: Path,
    trial: Path | None = None,
    clock: Clock | None = None,
) -> dict[str, Any]:
    """The task's verifier on the workspace's declared artifacts."""
    clock = clock or Clock()
    source = _synthetic_source(out / "source" / "candidate", task, workspace, trial)
    clock.lap("prepare")
    name = f"verify__{uuid.uuid4().hex[:7]}"
    ran = subprocess.run(
        [
            harbor_binary(),
            "trial",
            "regrade",
            str(source),
            "-p",
            str(task),
            "-e",
            WARM_ENVIRONMENT,
            "-o",
            str(out),
            "--trial-name",
            name,
        ],
        capture_output=True,
        text=True,
        timeout=7200,
    )
    clock.lap("verifier")
    result = _read_json(out / name / "result.json") or {}
    rewards = (result.get("verifier_result") or {}).get("rewards") or {}
    starts = []
    try:
        starts = [
            json.loads(line)
            for line in (out / name / "tbench-environment.jsonl").read_text().splitlines()
        ]
    except (OSError, json.JSONDecodeError):
        pass
    exception = result.get("exception_info") or {}
    return {
        "reward": rewards.get("reward"),
        "rewards": rewards,
        "exit": ran.returncode,
        "exception": exception.get("exception_type"),
        "message": (exception.get("exception_message") or "")[-800:] or None,
        "image_cache": next((s.get("cache") for s in starts if s.get("role") == "tests"), None),
        "trial_dir": str(out / name),
        "stdout_tail": (ran.stdout or "")[-400:] if ran.returncode else None,
    }


def replay_verify(trial: Path, out: Path) -> Replay:
    clock = Clock()
    task = task_dir(trial)
    with tempfile.TemporaryDirectory(prefix="tbench-replay-") as scratch_name:
        workspace = workspace_of(trial, Path(scratch_name))
        clock.lap("workspace")
        result = run_verifier(task, workspace, out, trial, clock)
    result["workspace"] = workspace.note
    return Replay(trial.name, "verify", workspace.source, original_reward(trial), clock.total(), result, out)


def replay(
    trial: Path, stage: str, *, artifact: Path | None = None, out: Path | None = None
) -> Replay:
    """One stage of one trial, with the result written to ``replay.json``."""
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    out = out or replays_dir() / f"{trial.name}--{stage}--{stamp}-{uuid.uuid4().hex[:4]}"
    try:
        if stage == "verify":
            result = replay_verify(trial, out)
        else:
            result = replay_checks(trial, repair=stage == "repair", artifact=artifact, out=out)
    except (ReplayError, OSError, subprocess.TimeoutExpired) as exc:
        result = Replay(trial.name, stage, "-", original_reward(trial), {}, {}, out, str(exc))
    out.mkdir(parents=True, exist_ok=True)
    (out / "replay.json").write_text(json.dumps(result.record(), indent=2) + "\n")
    return result


def replay_many(
    trials: list[Path],
    stage: str,
    *,
    artifact: Path | None = None,
    jobs: int = 4,
    echo: Callable[[str], None] = print,
) -> list[Replay]:
    """Replays several trials at once, printing each as it finishes."""
    done: list[Replay] = []

    def one(trial: Path) -> Replay:
        result = replay(trial, stage, artifact=artifact)
        echo(line(result))
        return result

    with ThreadPoolExecutor(max_workers=max(1, jobs)) as pool:
        done = list(pool.map(one, trials))
    return done


def verdict(result: Replay) -> str:
    """What the replay says, in a few words."""
    if result.error:
        return f"error: {result.error[:120]}"
    if result.stage == "verify":
        reward = result.result.get("reward")
        return f"reward {reward}" if reward is not None else (
            f"no reward ({result.result.get('exception') or 'see trial dir'})"
        )
    detected = "detected" if result.result.get("detected") else "passed"
    retained = result.result.get("retained") or {}
    before = retained.get("detected")
    text = f"checks {detected}"
    if before is not None:
        text += f" (episode: {'detected' if before else 'passed'})"
    repair = result.result.get("repair")
    if repair:
        text += f", repair {'would run' if repair.get('triggered') else 'would not run'}"
    return text


def line(result: Replay) -> str:
    total = result.seconds.get("total")
    return (
        f"{result.trial:<40} {result.stage:<7} {result.source:<10} "
        f"reward {'-' if result.reward is None else f'{result.reward:g}':<4} "
        f"{'-' if total is None else f'{total:5.1f}s'}  {verdict(result)}"
    )


def summary(results: list[Replay], wall: float) -> dict[str, Any]:
    """Recall against the verifier for a checks replay, and the times."""
    graded = [r for r in results if not r.error and r.reward is not None]
    failing = [r for r in graded if r.reward < 1]
    passing = [r for r in graded if r.reward >= 1]
    detected = lambda r: bool(r.result.get("detected"))  # noqa: E731
    return {
        "replays": len(results),
        "errors": sum(1 for r in results if r.error),
        "wall_sec": round(wall, 1),
        "failing": len(failing),
        "failing_detected": sum(1 for r in failing if detected(r)),
        "passing": len(passing),
        "passing_flagged": sum(1 for r in passing if detected(r)),
    }
