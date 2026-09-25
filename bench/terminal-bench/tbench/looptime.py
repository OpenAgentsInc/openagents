"""Where one trial's wall time went: the loop time of an experiment.

A targeted experiment is only as fast as its slowest phase, so every
``tbench try`` and ``tbench replay`` reports the same breakdown:

- ``env``: Harbor's environment setup, with whether the task image was
  warm (``tbench.warm_docker``), cold, or the task's own pinned image.
- ``agent_setup``: Harbor's agent setup, which installs the toolchain and
  the Coder One artifact and runs its doctor.
- ``agent``: Harbor's agent execution. For a Coder One episode it splits
  further, from the episode's own invocation log, into ``prep`` (the
  requirement map, probes, survey, and briefing), ``executor`` (the first
  executor session), ``checks``, ``support``, ``repair``, and ``later``
  (escalations, a second executor, persistence).
- ``verifier``: Harbor's verifier, including its environment.
- ``total``: the trial from start to finish.

Every figure is seconds read from what the trial recorded; nothing is
estimated. A phase the trial never reached is ``None``.
"""

from __future__ import annotations

import json
from datetime import datetime
from pathlib import Path
from typing import Any

SCHEMA = "openagents.tbench.looptime.v1"

# Top-level episode components, by the loop-time bucket they count toward.
EPISODE_BUCKETS = (
    ("prep", ("task.", "evidence.", "control.route", "exec.explore")),
    ("checks", ("verify.checks", "verify.close")),
    ("support", ("verify.support",)),
    ("repair", ("verify.repair",)),
)
COLUMNS = (
    "env",
    "agent_setup",
    "prep",
    "executor",
    "checks",
    "support",
    "repair",
    "later",
    "verifier",
    "total",
)


def _parse(stamp: str | None) -> datetime | None:
    if not stamp:
        return None
    try:
        return datetime.fromisoformat(stamp.replace("Z", "+00:00"))
    except ValueError:
        return None


def span_seconds(timing: dict[str, Any] | None) -> float | None:
    """Seconds between a Harbor timing's ``started_at`` and ``finished_at``."""
    if not timing:
        return None
    start, end = _parse(timing.get("started_at")), _parse(timing.get("finished_at"))
    if start is None or end is None:
        return None
    return round((end - start).total_seconds(), 1)


def read_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text())
    except (OSError, json.JSONDecodeError):
        return None


def episode_log(trial_dir: Path) -> Path | None:
    """The episode's invocation log: the collected bundle's, else the live tail."""
    for path in (
        trial_dir / "agent" / "episode" / "episode.atif.jsonl",
        trial_dir / "agent" / "live" / "episode.atif.jsonl",
    ):
        if path.is_file():
            return path
    return None


def invocations(log: Path) -> list[dict[str, Any]]:
    """Each invocation in an episode log: id, parent, component, name,
    start and end in epoch milliseconds (end ``None`` while it runs)."""
    found: dict[str, dict[str, Any]] = {}
    try:
        lines = log.read_text(errors="replace").splitlines()
    except OSError:
        return []
    for line in lines:
        try:
            record = json.loads(line)
        except json.JSONDecodeError:
            continue
        step = record.get("step") or {}
        invocation = (step.get("extensions") or {}).get("invocation")
        if not invocation or "id" not in invocation:
            continue
        at = step.get("at")
        if invocation.get("event") == "start":
            found[invocation["id"]] = {
                "id": invocation["id"],
                "parent": invocation.get("parent"),
                "component": invocation.get("component") or "",
                "name": invocation.get("name") or "",
                "start": at,
                "end": None,
            }
        elif invocation["id"] in found:
            found[invocation["id"]]["end"] = at
    return list(found.values())


def episode_phases(log: Path | None) -> dict[str, Any]:
    """The episode's seconds by loop-time bucket, and what runs now."""
    phases: dict[str, Any] = {
        "prep": None,
        "executor": None,
        "checks": None,
        "support": None,
        "repair": None,
        "later": None,
        "current": None,
    }
    if log is None:
        return phases
    rows = invocations(log)
    root = next((row for row in rows if row["parent"] is None), None)
    if root is None:
        return phases
    top = [row for row in rows if row["parent"] == root["id"]]
    first_session = next(
        (row for row in top if row["component"] == "exec.session"), None
    )
    for row in top:
        if row["start"] is None:
            continue
        seconds = ((row["end"] or row["start"]) - row["start"]) / 1000
        if row is first_session:
            bucket = "executor"
        else:
            bucket = next(
                (
                    name
                    for name, prefixes in EPISODE_BUCKETS
                    if any(row["component"].startswith(p) for p in prefixes)
                ),
                "later",
            )
        phases[bucket] = round((phases[bucket] or 0.0) + seconds, 1)
        if row["end"] is None:
            phases["current"] = row["component"]
    if root["end"] is not None:
        phases["current"] = None
    return phases


def environment_records(trial_dir: Path) -> list[dict[str, Any]]:
    """Every ``tbench.warm_docker`` record a trial left, in order."""
    path = trial_dir / "tbench-environment.jsonl"
    records = []
    try:
        lines = path.read_text().splitlines()
    except OSError:
        return []
    for line in lines:
        try:
            record = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(record, dict):
            records.append(record)
    return records


def environment_starts(trial_dir: Path) -> list[dict[str, Any]]:
    """The ``tbench.warm_docker`` start records a trial left, in order."""
    return [r for r in environment_records(trial_dir) if r.get("event", "start") == "start"]


def environment_stops(trial_dir: Path) -> list[dict[str, Any]]:
    """The ``tbench.warm_docker`` stop records a trial left, in order."""
    return [r for r in environment_records(trial_dir) if r.get("event") == "stop"]


def trial_looptime(trial_dir: Path) -> dict[str, Any]:
    """One trial's loop time, reward, cost, and where it stands now."""
    result = read_json(trial_dir / "result.json") or {}
    config = read_json(trial_dir / "config.json") or {}
    task = (result.get("task_name") or "").rsplit("/", 1)[-1]
    if not task:
        task = Path(str((config.get("task") or {}).get("path") or "")).name
    rewards = (result.get("verifier_result") or {}).get("rewards") or {}
    exception = result.get("exception_info") or {}
    phases = episode_phases(episode_log(trial_dir))
    starts = environment_starts(trial_dir)
    agent_env = next((s for s in starts if s.get("role") == "environment"), None)
    verifier_env = next((s for s in starts if s.get("role") == "tests"), None)
    row: dict[str, Any] = {
        "schema": SCHEMA,
        "trial": trial_dir.name,
        "job": trial_dir.parent.name,
        "task": task,
        "stage": stage_of(
            result, phases, starts, (trial_dir / "trial.log").exists()
        ),
        "reward": rewards.get("reward"),
        "exception": exception.get("exception_type"),
        "cost_usd": (result.get("agent_result") or {}).get("cost_usd"),
        "image": agent_env.get("cache") if agent_env else None,
        "verifier_image": verifier_env.get("cache") if verifier_env else None,
        "seconds": {
            "env": span_seconds(result.get("environment_setup")),
            "agent_setup": span_seconds(result.get("agent_setup")),
            "agent": span_seconds(result.get("agent_execution")),
            "verifier": span_seconds(result.get("verifier")),
            "total": span_seconds(
                {
                    "started_at": result.get("started_at"),
                    "finished_at": result.get("finished_at"),
                }
            ),
            **{k: v for k, v in phases.items() if k != "current"},
        },
        "current": phases.get("current"),
    }
    return row


def stage_of(
    result: dict[str, Any],
    phases: dict[str, Any],
    starts: list[dict[str, Any]] | None = None,
    started: bool = False,
) -> str:
    """A one-word stage: where the trial is, or how it ended.

    Harbor writes a trial's timings only as each phase ends, so a running
    trial's stage also reads the environment start records and the
    episode's live log.
    """
    if result.get("finished_at"):
        if result.get("exception_info"):
            return "error"
        return "done"
    roles = {start.get("role") for start in starts or []}
    if "tests" in roles or (result.get("verifier") or {}).get("started_at"):
        return "verify"
    if phases.get("current"):
        return phases["current"]
    for key, label in (
        ("agent_execution", "agent"),
        ("agent_setup", "setup"),
    ):
        if (result.get(key) or {}).get("started_at"):
            return label
    if "environment" in roles:
        return "setup"
    return "env" if started else "queued"


def job_looptimes(job_dir: Path) -> list[dict[str, Any]]:
    """Every trial's loop time in one Harbor job directory."""
    if not job_dir.is_dir():
        return []
    rows = []
    for trial_dir in sorted(job_dir.iterdir()):
        if not trial_dir.is_dir() or trial_dir.name == "tbench":
            continue
        if not any(
            (trial_dir / name).exists()
            for name in ("result.json", "config.json", "trial.log")
        ):
            continue
        rows.append(trial_looptime(trial_dir))
    return rows


def _fmt(value: Any) -> str:
    if value is None:
        return "-"
    if isinstance(value, float):
        if value >= 600:
            return f"{value / 60:.0f}m"
        return f"{value:.0f}s" if value >= 10 else f"{value:.1f}s"
    return str(value)


def totals(rows: list[dict[str, Any]]) -> dict[str, Any]:
    """Sums over rows, for the table's last line and the JSON report."""
    summed: dict[str, Any] = {}
    for column in COLUMNS:
        values = [row["seconds"].get(column) for row in rows]
        known = [v for v in values if v is not None]
        summed[column] = round(sum(known), 1) if known else None
    costs = [row.get("cost_usd") for row in rows if row.get("cost_usd") is not None]
    rewards = [row.get("reward") for row in rows if row.get("reward") is not None]
    return {
        "trials": len(rows),
        "graded": len(rewards),
        "passed": sum(1 for r in rewards if r and r >= 1.0),
        "cost_usd": round(sum(costs), 4) if costs else None,
        "seconds": summed,
    }


def render(rows: list[dict[str, Any]]) -> str:
    """A fixed-width table: one row per trial, then the sums."""
    header = ["trial", "stage", "reward", "cost", "image", *COLUMNS]
    body = []
    for row in rows:
        body.append(
            [
                row["trial"],
                row["stage"],
                "-" if row.get("reward") is None else f"{row['reward']:g}",
                "-" if row.get("cost_usd") is None else f"${row['cost_usd']:.2f}",
                row.get("image") or "-",
                *(_fmt(row["seconds"].get(column)) for column in COLUMNS),
            ]
        )
    summary = totals(rows)
    body.append(
        [
            f"{summary['trials']} trials",
            f"{summary['passed']}/{summary['graded']} passed",
            "",
            "-" if summary["cost_usd"] is None else f"${summary['cost_usd']:.2f}",
            "",
            *(_fmt(summary["seconds"].get(column)) for column in COLUMNS),
        ]
    )
    widths = [
        max(len(str(line[i])) for line in [header, *body]) for i in range(len(header))
    ]
    lines = [
        "  ".join(str(cell).ljust(widths[i]) for i, cell in enumerate(line)).rstrip()
        for line in [header, *body]
    ]
    lines.insert(1, "  ".join("-" * width for width in widths))
    lines.insert(len(lines) - 1, "  ".join("-" * width for width in widths))
    return "\n".join(lines)
