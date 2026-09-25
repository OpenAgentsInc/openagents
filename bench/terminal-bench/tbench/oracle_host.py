"""The host's oracle step for Coder One trials (issue #9656).

``checks.oracle`` has a separate Luna session write an acceptance oracle
from the task's words alone. The writer runs in a fresh container of the
task's image, which the lean loop can't start from inside a Harbor task
container. Because the oracle depends only on the task's words and image,
not on a candidate, the host writes it before the trial starts:

1. When the arm's policy manifest turns ``executor.microluna.lean.oracle``
   on, :func:`write` runs ``coder-one checks oracle write`` on the host,
   with the task's directory and the image Harbor built or pulled for the
   trial, and bounds it to fit in the agent's setup time.
2. :func:`deliver` places the finished ``oracle.json`` and ``spec.json``
   in the trial at ``/opt/openagents/oracle``, owned by root and
   read-only, and the episode learns the oracle's digest from
   ``CODER_ONE_ORACLE_DIGEST``. The loop checks the files against it and
   refuses a mismatch.
3. A step that produced no oracle is recorded as ``unavailable``, and the
   loop runs without one. A missing oracle is never a pass.

The record, ``oracle-host.json`` beside the agent's logs, holds the
oracle's digest, the host step's record, and its cost. :func:`cost` is the
declared rule the adapter's context and ``tbench cohort`` both count: a
known cost counts as recorded; when the writer's session may have had a
request open, the writer counts its whole spend bound, and a Jev cost that
wasn't recorded counts :data:`JEV_BOUND_USD`.

The switch is still refused by the episode's validation, and the component
isn't admitted; this step is the plumbing a later admission would use.
"""

from __future__ import annotations

import hashlib
import json
import subprocess
from decimal import Decimal
from pathlib import Path, PurePosixPath
from typing import Any, Callable

SCHEMA = "openagents.tbench.oracle-host.v1"
REMOTE_DIR = PurePosixPath("/opt/openagents/oracle")
DIR_ENV = "CODER_ONE_ORACLE_DIR"
DIGEST_ENV = "CODER_ONE_ORACLE_DIGEST"
UNAVAILABLE = "unavailable"
RECORD_NAME = "oracle-host.json"
# The Jev spend counted when the host step's Jev cost wasn't recorded. The
# spec's Jev questions cost $0.0058 across 17 tasks in the offline
# measurement; this is more than 25 times one task's share.
JEV_BOUND_USD = Decimal("0.01")
# Harbor's default agent setup timeout, in seconds, and the time left for
# installing the agent after the host step.
SETUP_DEFAULT_SEC = 360.0
INSTALL_MARGIN_SEC = 150.0
# Time the host step needs beyond the writer's session: reading the
# task's files, Jev's questions, and the containers.
STEP_MARGIN_SEC = 60.0
MIN_WRITER_SEC = 60


def settings(policy: dict[str, Any] | None) -> dict[str, Any] | None:
    """The manifest's ``executor.microluna.lean.oracle``, or ``None``."""
    lean = (((policy or {}).get("policy") or {}).get("executor") or {})
    lean = ((lean.get("microluna") or {}).get("lean") or {})
    found = lean.get("oracle")
    return found if isinstance(found, dict) else None


def setup_budget_sec(logs_dir: Path) -> float:
    """The agent's setup time for this trial, as Harbor resolves it from
    the trial's ``lock.json``, or Harbor's default when it isn't there."""
    try:
        lock = json.loads((Path(logs_dir).parent / "lock.json").read_text())
    except (OSError, ValueError):
        lock = {}
    agent = (lock.get("agent") or {}) if isinstance(lock, dict) else {}
    base = agent.get("override_setup_timeout_sec")
    if not isinstance(base, (int, float)) or base <= 0:
        base = SETUP_DEFAULT_SEC
    multiplier = lock.get("agent_setup_timeout_multiplier") if isinstance(lock, dict) else None
    if multiplier is None and isinstance(lock, dict):
        multiplier = lock.get("timeout_multiplier", 1.0)
    return float(base) * float(multiplier or 1.0)


def task_image(environment: Any) -> str | None:
    """The image Harbor started the trial from: the task's prebuilt image,
    or the one Harbor built for it. ``None`` when the environment doesn't
    say."""
    config = getattr(environment, "task_env_config", None)
    prebuilt = getattr(config, "docker_image", None)
    if getattr(environment, "_use_prebuilt", False) and prebuilt:
        return str(prebuilt)
    try:
        built = getattr(environment, "_main_image_name", None)
    except Exception:
        built = None
    return str(built) if built else (str(prebuilt) if prebuilt else None)


def money(value: Any) -> Decimal | None:
    if isinstance(value, bool) or value is None:
        return None
    try:
        number = Decimal(str(value))
    except ArithmeticError:
        return None
    return number if number.is_finite() and number >= 0 else None


def cost(step: dict[str, Any] | None, settings_: dict[str, Any] | None = None) -> dict[str, Any]:
    """The host step's cost under the declared rule.

    ``recorded_usd`` is the sum the step's record proves, or ``None`` when
    part of it isn't known. ``counted_usd`` is always a number: a writer
    session that may have had a request open counts its whole spend bound
    (or what it recorded, if more), and a Jev cost that wasn't recorded
    counts :data:`JEV_BOUND_USD`. A step that never ran, or ran into its
    time bound, counts both bounds.
    """
    bound = money((settings_ or {}).get("writer_usd"))
    recorded = (step or {}).get("cost") if isinstance(step, dict) else None
    if not isinstance(recorded, dict):
        luna_bound = bound if bound is not None else Decimal("0.08")
        return {
            "rule": "oracle-host-v1",
            "recorded_usd": None,
            "counted_usd": str(luna_bound + JEV_BOUND_USD),
            "known": False,
        }
    luna = money(recorded.get("luna_usd")) or Decimal(0)
    luna_bound = money(recorded.get("luna_bound_usd"))
    if luna_bound is None:
        luna_bound = bound if bound is not None else Decimal(0)
    jev = money(recorded.get("jev_usd"))
    luna_known = recorded.get("luna_known") is True
    counted_luna = luna if luna_known else max(luna, luna_bound)
    counted_jev = jev if jev is not None else JEV_BOUND_USD
    known = luna_known and jev is not None
    return {
        "rule": "oracle-host-v1",
        "recorded_usd": str(luna + jev) if known else None,
        "counted_usd": str(counted_luna + counted_jev),
        "known": known,
    }


Runner = Callable[..., subprocess.CompletedProcess]


def write(
    writer: str | None,
    task_dir: Path | None,
    image: str | None,
    out: Path,
    settings_: dict[str, Any],
    setup_sec: float,
    env: dict[str, str],
    run: Runner = subprocess.run,
) -> dict[str, Any]:
    """Run the host step and return the trial's record of it.

    ``writer`` is a ``coder-one`` binary that runs on this host. The step
    writes into ``out``. Every failure is a record with ``status:
    unavailable`` and a reason; nothing here raises.
    """
    out.mkdir(parents=True, exist_ok=True)
    wall = int(min(
        float(settings_.get("writer_sec") or 600),
        setup_sec - INSTALL_MARGIN_SEC - STEP_MARGIN_SEC,
    ))
    base: dict[str, Any] = {
        "schema": SCHEMA,
        "image": image,
        "task_dir": str(task_dir) if task_dir else None,
        "writer_sec": wall,
        "setup_sec": setup_sec,
    }

    def unavailable(why: str, step: dict[str, Any] | None = None, ran: bool = False) -> dict[str, Any]:
        record = {**base, "status": UNAVAILABLE, "why": why, "digest": None, "step": step}
        # A step that never started spent nothing.
        record["cost"] = cost(step, settings_) if ran else {
            "rule": "oracle-host-v1", "recorded_usd": "0", "counted_usd": "0", "known": True,
        }
        return record

    if not writer or not Path(writer).is_file():
        return unavailable("no coder-one binary that runs on the host was given")
    if task_dir is None or not (task_dir / "instruction.md").is_file():
        return unavailable("the task's directory or its instruction wasn't found")
    if not image:
        return unavailable("the trial's image isn't known")
    if wall < MIN_WRITER_SEC:
        return unavailable(
            f"the agent's setup time ({setup_sec:.0f} s) leaves the writer under "
            f"{MIN_WRITER_SEC} s; raise the job's agent setup timeout"
        )
    command = [
        str(writer), "checks", "oracle", "write", str(task_dir),
        "--image", image,
        "--out", str(out),
        "--writer-sec", str(wall),
    ]
    if settings_.get("writer_turns") is not None:
        command += ["--writer-turns", str(settings_["writer_turns"])]
    if settings_.get("writer_usd") is not None:
        command += ["--session-usd", str(settings_["writer_usd"])]
    try:
        finished = run(
            command,
            env=env,
            capture_output=True,
            text=True,
            timeout=wall + STEP_MARGIN_SEC,
        )
    except subprocess.TimeoutExpired:
        return unavailable("the host step ran past its time bound", ran=True)
    except OSError as error:
        return unavailable(f"the host step couldn't start: {error}")
    (out / "step.log").write_text(
        f"exit {finished.returncode}\n{finished.stdout or ''}{finished.stderr or ''}"
    )
    try:
        step = json.loads((out / "record.json").read_text())
    except (OSError, ValueError):
        return unavailable(f"the host step exited {finished.returncode} without a record", ran=True)
    digest = step.get("digest")
    if finished.returncode != 0 or not isinstance(digest, str) or not digest:
        return unavailable(step.get("why") or "the host step left no oracle", step, ran=True)
    try:
        oracle = json.loads((out / "oracle.json").read_text())
    except (OSError, ValueError):
        return unavailable("the host step's oracle.json can't be read", step, ran=True)
    if oracle.get("digest") != digest:
        return unavailable("oracle.json's digest isn't the one the host step recorded", step, ran=True)
    files = ["oracle.json"] + (["spec.json"] if (out / "spec.json").is_file() else [])
    return {
        **base,
        "status": "delivered",
        "why": None,
        "digest": digest,
        "source": step.get("status"),
        "files": {
            name: hashlib.sha256((out / name).read_bytes()).hexdigest() for name in files
        },
        "step": step,
        "cost": cost(step, settings_),
    }


def episode_env(record: dict[str, Any] | None) -> dict[str, str]:
    """What the episode reads: the oracle's directory and digest, or
    ``unavailable``."""
    if record is None:
        return {}
    if record.get("status") == "delivered":
        return {DIR_ENV: str(REMOTE_DIR), DIGEST_ENV: str(record["digest"])}
    return {DIGEST_ENV: UNAVAILABLE}


async def deliver(agent: Any, environment: Any, out: Path, record: dict[str, Any]) -> None:
    """Place the finished files in the trial, owned by root and read-only.
    Only the files the record names go in."""
    if record.get("status") != "delivered":
        return
    await agent.exec_as_root(environment, command=f"mkdir -p {REMOTE_DIR}")
    for name in record.get("files") or {}:
        await environment.upload_file(str(out / name), str(REMOTE_DIR / name))
    await agent.exec_as_root(
        environment,
        command=(
            f"chown -R 0:0 {REMOTE_DIR} && chmod 0444 {REMOTE_DIR}/* && chmod 0555 {REMOTE_DIR}"
        ),
    )


def save(logs_dir: Path, record: dict[str, Any]) -> None:
    (Path(logs_dir) / RECORD_NAME).write_text(json.dumps(record, indent=2) + "\n")


def load(logs_dir: Path) -> dict[str, Any] | None:
    try:
        return json.loads((Path(logs_dir) / RECORD_NAME).read_text())
    except (OSError, ValueError):
        return None
