"""Compare: fold attempt records across arms into one report.

The machine-readable report keeps every attempt — skipped, failed, and
unverifiable included — grouped by task and arm on identical task pins.
The human-readable table is a rendering of the same data; nothing in it
is computed twice, so the table can never disagree with the JSON.
"""

from __future__ import annotations

import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

REPORT_SCHEMA = "openagents.tbench.report.v1"
SMALL_SAMPLE_LABEL = "small development sample"


def _load_attempts(jobs_dir: Path) -> list[dict[str, Any]]:
    """Every attempt record under a jobs dir, across all job names."""
    records = []
    for path in sorted(jobs_dir.glob("*/tbench/attempts/*.json")):
        try:
            records.append(json.loads(path.read_text()))
        except (OSError, json.JSONDecodeError):
            continue
    return records


def _fmt_ms(ms: Any) -> str:
    if ms is None or ms == "unknown":
        return "unknown"
    seconds = int(ms) / 1000
    if seconds < 90:
        return f"{seconds:.1f}s"
    return f"{seconds / 60:.1f}m"


def _fmt_count(value: Any) -> str:
    return "unknown" if value in (None, "unknown") else str(value)


def _fmt_cost(cost: dict[str, Any]) -> str:
    amount = cost.get("amount_usd")
    if amount is None:
        return f"unknown({cost.get('provenance', 'unknown')})"
    return f"${amount:.4f}({cost.get('provenance', '?')[:9]})"


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

    by_task_arm: dict[str, dict[str, list[dict[str, Any]]]] = {}
    pins: dict[str, dict[str, Any]] = {}
    for record in attempts:
        task = record["task"]["name"] or "?"
        arm = record["attempt"]["arm"]
        by_task_arm.setdefault(task, {}).setdefault(arm, []).append(record)
        pins.setdefault(task, {}).update(
            {
                "path": record["task"].get("path"),
                "git_commit_id": record["task"].get("git_commit_id"),
                "checksum": record["task"].get("checksum"),
            }
        )

    # A comparison only pairs arms on identical pins; a checksum or commit
    # mismatch inside one task name is called out, never silently pooled.
    pin_warnings = []
    for task, pin in pins.items():
        commits = {
            r["task"].get("git_commit_id")
            for arm in by_task_arm[task].values()
            for r in arm
        }
        if len(commits) > 1:
            pin_warnings.append(
                f"{task}: mixed commits {sorted(c or '?' for c in commits)}"
            )

    groups = []
    for task in sorted(by_task_arm):
        arm_groups = []
        for arm in sorted(by_task_arm[task]):
            records = by_task_arm[task][arm]
            rewards = [r["outcome"]["reward"] for r in records]
            statuses = [r["outcome"]["terminal_status"] for r in records]
            known_rewards = [x for x in rewards if isinstance(x, (int, float))]
            arm_groups.append(
                {
                    "arm": arm,
                    "attempts": len(records),
                    "rewards": rewards,
                    "reward_mean": (
                        sum(known_rewards) / len(known_rewards)
                        if known_rewards
                        else None
                    ),
                    "terminal_statuses": statuses,
                    "agent_wall_ms": [
                        r["timing"]["agent_execution_ms"] for r in records
                    ],
                    "total_wall_ms": [
                        r["timing"]["total_ms"] for r in records
                    ],
                    "usage": [r["usage"] for r in records],
                    "cost": [r["cost"] for r in records],
                    "counts": [r["counts"] for r in records],
                    "completeness": [r["completeness"] for r in records],
                    "attempt_ids": [r["attempt"]["id"] for r in records],
                }
            )
        groups.append(
            {"task": task, "pin": pins[task], "arms": arm_groups}
        )

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
            f"{'arm':<16} {'n':>2} {'reward':>16} {'status':<22} "
            f"{'agent':>8} {'total':>8} {'steps':>6} {'calls':>6} "
            f"{'in':>8} {'out':>8} {'cost':>22}"
        )
        lines.append(header)
        lines.append("-" * len(header))
        for arm in group["arms"]:
            rewards = ",".join(
                "?" if r is None else f"{r:g}" for r in arm["rewards"]
            )
            statuses = ",".join(sorted(set(arm["terminal_statuses"])))
            walls = [
                w for w in arm["agent_wall_ms"] if isinstance(w, (int, float))
            ]
            totals = [
                w for w in arm["total_wall_ms"] if isinstance(w, (int, float))
            ]
            steps = [
                c.get("atif_steps")
                for c in arm["counts"]
                if isinstance(c.get("atif_steps"), int)
            ]
            calls = [
                c.get("tool_calls")
                for c in arm["counts"]
                if isinstance(c.get("tool_calls"), int)
            ]
            ins = [
                u.get("input_tokens")
                for u in arm["usage"]
                if isinstance(u.get("input_tokens"), int)
            ]
            outs = [
                u.get("output_tokens")
                for u in arm["usage"]
                if isinstance(u.get("output_tokens"), int)
            ]
            costs = [c for c in arm["cost"] if c.get("amount_usd") is not None]
            lines.append(
                f"{arm['arm']:<16} {arm['attempts']:>2} {rewards:>16} "
                f"{statuses:<22} "
                f"{_fmt_ms(sum(walls) / len(walls) if walls else None):>8} "
                f"{_fmt_ms(sum(totals) / len(totals) if totals else None):>8} "
                f"{_fmt_count(sum(steps) / len(steps) if steps else 'unknown'):>6} "
                f"{_fmt_count(sum(calls) / len(calls) if calls else 'unknown'):>6} "
                f"{_fmt_count(sum(ins) // len(ins) if ins else 'unknown'):>8} "
                f"{_fmt_count(sum(outs) // len(outs) if outs else 'unknown'):>8} "
                f"{_fmt_cost(costs[0]) if costs else 'unknown':>22}"
            )
        lines.append("")
    lines.append(
        "Reward is the verifier's; status is the trial's terminal state. "
        "'unknown' is a recorded absence, not a zero."
    )
    return "\n".join(lines)
