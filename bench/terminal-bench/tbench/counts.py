"""Step and call counts from a retained ATIF trajectory.

The counts name their semantics explicitly: ``atif_steps`` is the raw step
count of the trajectory document, and each call kind is counted by its
``tool_name``/decision markers. A trajectory the parser cannot read
yields ``unknown`` fields rather than zeroes.
"""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

UNKNOWN = "unknown"

# Tool names Harbor's own converters use for shell executions.
SHELL_TOOL_NAMES = {"bash", "shell", "execute_command", "run_command"}


def count_trajectory(trajectory: dict[str, Any]) -> dict[str, Any]:
    """Count steps and calls in one parsed ATIF ``Trajectory`` document."""
    steps = trajectory.get("steps") or []
    counts: dict[str, Any] = {
        "semantics": "atif-v1-steps-and-calls",
        "atif_steps": len(steps),
        "model_invocations": 0,
        "typed_decisions": 0,
        "tool_calls": 0,
        "shell_commands": 0,
        "retries": 0,
        "subagent_steps": 0,
    }
    for step in steps:
        if step.get("source") != "agent":
            continue
        if step.get("subagent_trajectory_ref"):
            counts["subagent_steps"] += 1
        for call in step.get("tool_calls") or []:
            counts["tool_calls"] += 1
            name = (call.get("tool_name") or "").lower()
            if name in SHELL_TOOL_NAMES:
                counts["shell_commands"] += 1
            if call.get("is_retry") or (call.get("extra") or {}).get("retry"):
                counts["retries"] += 1
        extra = step.get("extra") or {}
        if extra.get("decision_call") or extra.get("typed_decision"):
            counts["typed_decisions"] += 1
        if step.get("model_name") or step.get("message"):
            counts["model_invocations"] += 1
    return counts


def unknown_counts(reason: str) -> dict[str, Any]:
    """The count record when no readable trajectory exists."""
    return {
        "semantics": "atif-v1-steps-and-calls",
        "atif_steps": UNKNOWN,
        "model_invocations": UNKNOWN,
        "typed_decisions": UNKNOWN,
        "tool_calls": UNKNOWN,
        "shell_commands": UNKNOWN,
        "retries": UNKNOWN,
        "subagent_steps": UNKNOWN,
        "note": reason,
    }


def counts_for_trial(trial_dir: Path) -> dict[str, Any]:
    """The count record for one trial, or explicit unknowns."""
    trajectory_path = trial_dir / "agent" / "trajectory.json"
    if not trajectory_path.exists():
        return unknown_counts("no trajectory.json retained")
    try:
        return count_trajectory(json.loads(trajectory_path.read_text()))
    except (OSError, json.JSONDecodeError) as exc:
        return unknown_counts(f"trajectory unreadable: {exc}")
