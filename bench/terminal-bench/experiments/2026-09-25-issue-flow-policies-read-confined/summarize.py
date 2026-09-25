#!/usr/bin/env python3
"""Summarize completed, retained runs using the registered decision rule."""

import argparse
import json
import math
from pathlib import Path
import statistics


def wilson(passed, total):
    z = 1.959963984540054
    p = passed / total
    denominator = 1 + z * z / total
    center = (p + z * z / (2 * total)) / denominator
    radius = z * math.sqrt(p * (1 - p) / total + z * z / (4 * total * total)) / denominator
    return [max(0, center - radius), min(1, center + radius)]


def measure(rows):
    passed = sum(row["passed"] for row in rows)
    complete = all(row["cost"]["total_usd"] is not None for row in rows)
    counted = sum(row["counted_usd"] for row in rows)
    return {
        "attempts": len(rows), "passed": passed, "pass_wilson_95": wilson(passed, len(rows)),
        "entries_passed": sorted({row["entry"] for row in rows if row["passed"]}),
        "contaminated": sum(row["contaminated"] for row in rows),
        "finished": sum(row["outcome"] == "finished" for row in rows),
        "finished_and_passed": sum(row["outcome"] == "finished" and row["passed"] for row in rows),
        "checks_passed": sum(row["checks_passed"] for row in rows),
        "checks_total": sum(row["checks_total"] for row in rows),
        "cost_complete": complete, "counted_usd": counted,
        "mean_cost_usd": counted / len(rows) if complete else None,
        "cost_per_pass_usd": counted / passed if passed and complete else None,
        "mean_total_seconds": statistics.mean(row["total_seconds"] for row in rows),
        "mean_flow_seconds": statistics.mean(row["flow_seconds"] for row in rows),
        "mean_grading_seconds": statistics.mean(row["grading_seconds"] for row in rows),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("out", type=Path)
    parser.add_argument("--write", type=Path)
    args = parser.parse_args()
    state = json.loads((args.out / "state.json").read_text())
    assert state["status"] in ("complete", "stopped"), "study is incomplete"
    rows = []
    journal = [json.loads(line) for line in (args.out / "runs.jsonl").read_text().splitlines()]
    for attempt in journal:
        manifest = json.loads(Path(attempt["manifest"]).read_text())
        rows.append({
            "slot": attempt["slot"], "entry": attempt["entry"], "arm": attempt["arm"],
            "manifest": attempt["manifest"], "id": manifest["id"],
            "policy": manifest["policy"], "outcome": manifest["outcome"],
            "passed": manifest["grade"]["verdict"] == "passed" and not manifest["contaminated"],
            "contaminated": manifest["contaminated"],
            "blocked_attempts": manifest["contamination"]["blocked"],
            "gate_incomplete": manifest["gate_incomplete"],
            "checks_passed": manifest["grade"]["passed"], "checks_total": manifest["grade"]["total"],
            "failed_checks": [c["id"] for c in manifest["grade"]["checks"] if not c["passed"]],
            "cost": manifest["cost"], "counted_usd": attempt["counted_usd"],
            "total_seconds": manifest["milliseconds"] / 1000,
            "flow_seconds": manifest["flow_milliseconds"] / 1000,
            "grading_seconds": manifest["grading_milliseconds"] / 1000,
        })
    arms = {arm: measure([row for row in rows if row["arm"] == arm]) for arm in ("lean", "requirements")}
    assert arms["lean"]["attempts"] == arms["requirements"]["attempts"], "unbalanced comparison"
    assert len(rows) in (8, 16), "a partial round needs a separate report, not the complete-round decision"
    repeats = len(rows) // 8
    delta = arms["lean"]["passed"] - arms["requirements"]["passed"]
    complete_cost = all(arm["cost_complete"] for arm in arms.values())
    cheaper = complete_cost and arms["lean"]["mean_cost_usd"] <= .8 * arms["requirements"]["mean_cost_usd"]
    switch = delta >= repeats or (0 <= delta < repeats and cheaper)
    veto = arms["lean"]["contaminated"] > 0 and arms["requirements"]["contaminated"] == 0
    decision = "lean" if switch and not veto else "requirements"
    per_entry = {}
    for entry in sorted({row["entry"] for row in rows}):
        per_entry[entry] = {arm: measure([row for row in rows if row["entry"] == entry and row["arm"] == arm]) for arm in arms}
    result = {"schema": "openagents.issue-flow-policy-comparison.v1", "state": state,
              "repeats_per_cell": repeats, "arms": arms, "per_entry": per_entry,
              "pass_delta_lean_minus_requirements": delta, "lean_at_least_20_percent_cheaper": cheaper,
              "contamination_veto": veto, "selected_default": decision, "runs": rows}
    text = json.dumps(result, indent=2) + "\n"
    if args.write:
        args.write.write_text(text)
    else:
        print(text, end="")


if __name__ == "__main__":
    main()
