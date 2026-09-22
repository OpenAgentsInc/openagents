"""Compare: fold attempt records across arms into one report.

The machine-readable report keeps every attempt — skipped, failed, and
unverifiable included — grouped by task and arm on identical task pins.
The human-readable table is a rendering of the same data; nothing in it
is computed twice, so the table can never disagree with the JSON.

Repetitions of one arm on one task pool across job names (a `-2` suffix
is the same arm run again) only when their pins match: the task checksum,
the observed agent version, the model, and the artifact digest a contract
arm ran. Attempts that differ in any of these are separate cells, never
one sample. An attempt preserved as ``interrupted`` before a resume
reran it is counted apart and never scored.
"""

from __future__ import annotations

import json
import math
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

REPORT_SCHEMA = "openagents.tbench.report.v1"
SMALL_SAMPLE_LABEL = "small development sample"

# The two-sided 95% normal quantile for the Wilson score interval.
WILSON_Z = 1.959963984540054

# A trial passes when its verifier reward reaches this value; Terminal-Bench
# verifiers score 0 or 1.
PASS_REWARD = 1.0


def _load_attempts(jobs_dir: Path) -> list[dict[str, Any]]:
    """Every attempt record under a jobs dir, across all job names."""
    records = []
    for path in sorted(jobs_dir.glob("*/tbench/attempts/*.json")):
        try:
            records.append(json.loads(path.read_text()))
        except (OSError, json.JSONDecodeError):
            continue
    return records


def wilson_interval(
    passes: int, n: int, z: float = WILSON_Z
) -> tuple[float, float] | None:
    """The Wilson score interval for a pass rate, or None below two trials.

    One trial says what happened once; it supports no interval, and the
    report labels it a single trial instead.
    """
    if n < 2:
        return None
    p = passes / n
    denom = 1 + z * z / n
    centre = (p + z * z / (2 * n)) / denom
    half = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / denom
    # At 0 and n passes the bound is exactly 0 or 1; say so instead of
    # returning rounding error.
    low = 0.0 if passes == 0 else max(0.0, centre - half)
    high = 1.0 if passes == n else min(1.0, centre + half)
    return (low, high)


def _spread(values: list[Any]) -> dict[str, Any]:
    """Mean, min, and max of the known numbers; ``n`` says how many."""
    known = [v for v in values if isinstance(v, (int, float))]
    if not known:
        return {"n": 0, "mean": None, "min": None, "max": None}
    return {
        "n": len(known),
        "mean": sum(known) / len(known),
        "min": min(known),
        "max": max(known),
    }


def pin_key(record: dict[str, Any]) -> dict[str, Any]:
    """The identity two attempts must share to pool as repetitions."""
    agent = record.get("agent") or {}
    task = record.get("task") or {}
    return {
        "task_checksum": task.get("checksum"),
        "agent_version": agent.get("observed_version"),
        "model": agent.get("observed_model") or agent.get("requested_model"),
        "artifact_sha256": agent.get("artifact_sha256"),
    }


def _pin_label(pin: dict[str, Any]) -> str:
    parts = [
        f"version {pin['agent_version'] or 'unknown'}",
        f"model {pin['model'] or 'unknown'}",
    ]
    if pin["artifact_sha256"]:
        parts.append(f"artifact sha256 {pin['artifact_sha256'][:12]}")
    if pin["task_checksum"]:
        parts.append(f"task {pin['task_checksum'][:12]}")
    return ", ".join(parts)


def _cell(arm: str, pin: dict[str, Any], records: list[dict[str, Any]]) -> dict[str, Any]:
    """One task-arm-pin cell: every repetition, and the statistics over it."""
    trials = [
        r for r in records if (r.get("attempt") or {}).get("kind") != "interrupted"
    ]
    interrupted = len(records) - len(trials)
    rewards = [r["outcome"]["reward"] for r in trials]
    scored = [x for x in rewards if isinstance(x, (int, float))]
    passes = sum(1 for x in scored if x >= PASS_REWARD)
    interval = wilson_interval(passes, len(scored))
    costs = [r["cost"] for r in trials]
    provenances = sorted(
        {c.get("provenance") or "unknown" for c in costs if c.get("amount_usd") is not None}
    )
    no_trace = sum(
        1 for r in trials if (r.get("completeness") or {}).get("trace") == "absent"
    )
    bundle_missing = sum(
        1
        for r in trials
        if (r.get("completeness") or {}).get("bundle")
        in ("collection_failed", "empty")
    )
    return {
        "arm": arm,
        "pin": pin,
        "pin_label": _pin_label(pin),
        "jobs": sorted({r["attempt"].get("job") for r in records if r["attempt"].get("job")}),
        "attempts": len(trials),
        "interrupted": interrupted,
        "scored": len(scored),
        "passes": passes,
        "single_trial": len(scored) == 1,
        "pass_rate": passes / len(scored) if scored else None,
        "pass_rate_interval": (
            {"method": "wilson", "level": 0.95, "low": interval[0], "high": interval[1]}
            if interval
            else None
        ),
        "rewards": rewards,
        "reward_mean": sum(scored) / len(scored) if scored else None,
        "terminal_statuses": [r["outcome"]["terminal_status"] for r in trials],
        "agent_wall_ms": [r["timing"]["agent_execution_ms"] for r in trials],
        "agent_wall_ms_spread": _spread(
            [r["timing"]["agent_execution_ms"] for r in trials]
        ),
        "total_wall_ms": [r["timing"]["total_ms"] for r in trials],
        "usage": [r["usage"] for r in trials],
        "cost": costs,
        "cost_usd_spread": _spread([c.get("amount_usd") for c in costs]),
        "cost_provenances": provenances,
        "counts": [r["counts"] for r in trials],
        "completeness": [r["completeness"] for r in trials],
        "without_trace": no_trace,
        "bundle_not_collected": bundle_missing,
        "image_states": [
            (r.get("environment") or {}).get("image_state", "unknown") for r in trials
        ],
        "attempt_ids": [r["attempt"]["id"] for r in trials],
    }


def compare(
    jobs_dir: Path,
    *,
    arms: list[str] | None = None,
    label: str = SMALL_SAMPLE_LABEL,
) -> dict[str, Any]:
    """The ``openagents.tbench.report.v1`` report for a jobs dir."""
    attempts = _load_attempts(jobs_dir)
    if arms:
        attempts = [a for a in attempts if a["attempt"]["arm"] in arms]

    by_task: dict[str, dict[tuple[str, str], list[dict[str, Any]]]] = {}
    pins: dict[str, dict[str, Any]] = {}
    for record in attempts:
        task = record["task"]["name"] or "?"
        arm = record["attempt"]["arm"]
        key = (arm, json.dumps(pin_key(record), sort_keys=True))
        by_task.setdefault(task, {}).setdefault(key, []).append(record)
        pins.setdefault(task, {}).update(
            {
                "path": record["task"].get("path"),
                "git_commit_id": record["task"].get("git_commit_id")
                or (record["task"].get("pin") or {}).get("git_commit_id"),
                "checksum": record["task"].get("checksum"),
            }
        )

    # A comparison only pairs arms on identical pins; a checksum or commit
    # mismatch inside one task name is called out, never silently pooled.
    pin_warnings = []
    for task in pins:
        records = [r for cell in by_task[task].values() for r in cell]
        commits = {
            r["task"].get("git_commit_id")
            or (r["task"].get("pin") or {}).get("git_commit_id")
            for r in records
        }
        if len(commits) > 1:
            pin_warnings.append(
                f"{task}: mixed commits {sorted(c or '?' for c in commits)}"
            )
        checksums = {r["task"].get("checksum") for r in records}
        if len(checksums) > 1:
            pin_warnings.append(
                f"{task}: mixed task checksums "
                f"{sorted((c or '?')[:12] for c in checksums)}"
            )

    groups = []
    for task in sorted(by_task):
        cells = [
            _cell(arm, json.loads(pin), records)
            for (arm, pin), records in sorted(by_task[task].items())
        ]
        arm_counts: dict[str, int] = {}
        for cell in cells:
            arm_counts[cell["arm"]] = arm_counts.get(cell["arm"], 0) + 1
        for cell in cells:
            cell["arm_split_by_pin"] = arm_counts[cell["arm"]] > 1
        groups.append({"task": task, "pin": pins[task], "arms": cells})

    return {
        "schema": REPORT_SCHEMA,
        "label": label,
        "generated_at": datetime.now(timezone.utc).isoformat(
            timespec="milliseconds"
        ),
        "jobs_dir": str(jobs_dir),
        "pin_warnings": pin_warnings,
        "tasks": groups,
        "attempts_total": len(attempts),
    }


def _fmt_ms(ms: Any) -> str:
    if ms is None or ms == "unknown":
        return "unknown"
    seconds = ms / 1000
    if seconds < 90:
        return f"{seconds:.1f}s"
    return f"{seconds / 60:.1f}m"


def _fmt_spread(spread: dict[str, Any], fmt: Any) -> str:
    if not spread["n"]:
        return "unknown"
    if spread["n"] == 1:
        return fmt(spread["mean"])
    return f"{fmt(spread['mean'])} ({fmt(spread['min'])}-{fmt(spread['max'])})"


def _fmt_usd(value: float) -> str:
    return f"${value:.4f}"


def _fmt_rate(cell: dict[str, Any]) -> str:
    if not cell["scored"]:
        return "no scored trial"
    if cell["single_trial"]:
        return f"{cell['passes']}/1 single trial"
    interval = cell["pass_rate_interval"]
    return (
        f"{cell['passes']}/{cell['scored']} "
        f"[{interval['low']:.2f}, {interval['high']:.2f}]"
    )


def render_table(report: dict[str, Any]) -> str:
    """The human-readable rendering of a compare report."""
    lines = [
        f"# Terminal-Bench comparison — {report['label']}",
        "",
        f"attempts: {report['attempts_total']}   jobs dir: {report['jobs_dir']}",
        "",
    ]
    for warning in report.get("pin_warnings") or []:
        lines.append(f"WARNING: {warning}")
    if report.get("pin_warnings"):
        lines.append("")
    for group in report["tasks"]:
        pin = group["pin"]
        lines.append(
            f"## {group['task']}  "
            f"(commit {(pin.get('git_commit_id') or '?')[:12]}, "
            f"checksum {(pin.get('checksum') or '?')[:12]})"
        )
        header = (
            f"{'arm':<26} {'n':>2} {'pass [95% Wilson]':<22} {'reward':>6} "
            f"{'agent time mean (min-max)':<26} {'cost mean (min-max)':<36} "
            f"status"
        )
        lines.append(header)
        lines.append("-" * len(header))
        for cell in group["arms"]:
            statuses: dict[str, int] = {}
            for status in cell["terminal_statuses"]:
                statuses[status] = statuses.get(status, 0) + 1
            status_text = ",".join(
                f"{name}x{count}" if count > 1 else name
                for name, count in sorted(statuses.items())
            )
            notes = []
            if cell["interrupted"]:
                notes.append(f"+{cell['interrupted']} interrupted")
            if cell["without_trace"]:
                notes.append(f"{cell['without_trace']} without trace")
            if cell["bundle_not_collected"]:
                notes.append(
                    f"{cell['bundle_not_collected']} bundle not collected"
                )
            if cell["cost_usd_spread"]["n"] and cell["cost_usd_spread"]["n"] < cell["attempts"]:
                notes.append(
                    f"cost known for {cell['cost_usd_spread']['n']}/{cell['attempts']}"
                )
            if len(cell["cost_provenances"]) > 1:
                notes.append("mixed cost provenance")
            reward = (
                "?" if cell["reward_mean"] is None else f"{cell['reward_mean']:.2f}"
            )
            cost = _fmt_spread(cell["cost_usd_spread"], _fmt_usd)
            if cell["cost_provenances"] and cost != "unknown":
                cost += f" {'/'.join(cell['cost_provenances'])}"
            lines.append(
                f"{cell['arm']:<26} {cell['attempts']:>2} "
                f"{_fmt_rate(cell):<22} {reward:>6} "
                f"{_fmt_spread(cell['agent_wall_ms_spread'], _fmt_ms):<26} "
                f"{cost:<36} {status_text}"
                + (f"  [{'; '.join(notes)}]" if notes else "")
            )
            if cell["arm_split_by_pin"]:
                lines.append(f"{'':<4}pin: {cell['pin_label']}")
        lines.append("")
    lines.append(
        "Reward is the verifier's; status is the trial's terminal state. "
        "Pass rates count scored trials only, with a 95% Wilson interval "
        "from two scored trials up. An arm listed twice ran under different "
        "pins, which never pool. 'unknown' is a recorded absence, not a zero."
    )
    return "\n".join(lines)
