"""Collect the pinned experiment without reading outcomes into an agent prompt.

Run with the tbench Python environment. The output includes every planned
attempt, including pending or invalid ones. Unknown costs remain unknown.
"""

import argparse
import json
from datetime import datetime, timezone
from pathlib import Path

from tbench.looptime import span_seconds


def read(path):
    return json.loads(path.read_text()) if path.is_file() else {}


def collect(root):
    experiment = root / "experiments/iteration-speed-9618"
    status = read(root / "suites/tb4--coder-one-microluna-v13-retained/status.json")
    for item in status.get("trials", []):
        item["arm"] = "coder-one-microluna-v13-retained"
    rows = []
    for item in status.get("trials", []):
        job = item.get("job") or item.get("job_name")
        if not job:
            job = f"tb4--{item['arm']}--{item['task']}--iteration-speed-9618-r{item['attempt']}"
        job_dir = root / "jobs" / job
        results = sorted(job_dir.glob("*/result.json"))
        if len(results) > 1:
            raise ValueError(f"Multiple current results for {job}")
        trial = results[0].parent if results else None
        result = read(results[0]) if results else {}
        usage = read(trial / "agent/episode/evaluation/usage.json") if trial else {}
        agent = result.get("agent_result") or {}
        cost = (usage.get("cost") or {}).get("amount_usd")
        selections = []
        loops = []
        if trial:
            for path in sorted(trial.glob("agent/episode/artifacts/lean-*/selection.json")):
                moves = read(path)
                selections.append({"path": str(path.relative_to(trial)), "moves": moves})
            for path in sorted(trial.glob("agent/episode/artifacts/microluna-*.json")):
                loop = read(path)
                if not isinstance(loop, dict) or "sessions" not in loop:
                    continue
                loops.append({"path": str(path.relative_to(trial)), "stopped": loop.get("stopped"),
                              "moves": loop.get("moves"),
                              "sessions": [{key: session.get(key) for key in
                                            ("number", "status", "finish", "turns", "calls", "cost_usd",
                                             "milliseconds", "trace", "changed", "read_only")}
                                           for session in loop["sessions"]]})
        ctrf = read(trial / "verifier/ctrf.json") if trial else {}
        rows.append({
            "job": job, "task": item["task"], "arm": item["arm"],
            "attempt": item["attempt"], "state": item["state"],
            "reason": item.get("reason"), "trial": trial.name if trial else None,
            "reward": ((result.get("verifier_result") or {}).get("rewards") or {}).get("reward"),
            "exception": result.get("exception_info"),
            "cost_usd": cost, "harbor_cost_usd": agent.get("cost_usd"),
            "usage": usage,
            "agent_seconds": span_seconds(result.get("agent_execution")),
            "trial_seconds": span_seconds(result),
            "network": read(trial / "network-policy.json") if trial else {},
            "selection": selections,
            "loops": loops,
            "verifier_summary": (ctrf.get("results") or {}).get("summary"),
            "verifier_failures": [test.get("name") for test in (ctrf.get("results") or {}).get("tests", [])
                                  if test.get("status") == "failed"],
        })
    interruptions = []
    for item in status.get("trials", []):
        job_dir = root / "jobs" / item["job"]
        for path in sorted(job_dir.glob("tbench/interrupted/*/result.json")):
            result = read(path)
            usage = read(path.parent / "agent/episode/evaluation/usage.json")
            interruptions.append({"job": item["job"], "arm": item["arm"],
                                  "task": item["task"], "trial": path.parent.name,
                                  "exception": result.get("exception_info"),
                                  "recorded_cost_usd": (usage.get("cost") or {}).get("amount_usd"),
                                  "usage": usage,
                                  "agent_seconds": span_seconds(result.get("agent_execution")),
                                  "trial_seconds": span_seconds(result)})
    summary = []
    for task in read(experiment / "pins.json").get("tasks", []):
        for arm in read(experiment / "pins.json").get("policies", {}):
            group = [r for r in rows if r["task"] == task and r["arm"] == arm]
            completed = [r for r in group if r["reward"] is not None]
            passes = sum(r["reward"] == 1 for r in completed)
            known = [r["cost_usd"] for r in group if r["cost_usd"] is not None]
            total = sum(known) if len(known) == len(group) else None
            interrupted = [r for r in interruptions if r["task"] == task and r["arm"] == arm]
            interrupted_known = [r["recorded_cost_usd"] for r in interrupted
                                 if r["recorded_cost_usd"] is not None]
            with_interrupted = (total + sum(interrupted_known)
                                if total is not None and len(interrupted_known) == len(interrupted) else None)
            summary.append({"task": task, "arm": arm, "planned": len(group),
                            "graded": len(completed), "passes": passes,
                            "known_cost_usd": sum(known), "total_cost_usd": total,
                            "cost_per_pass_usd": total / passes if total is not None and passes else None,
                            "recorded_interrupted_cost_usd": sum(interrupted_known),
                            "recorded_cost_including_interruptions_usd": with_interrupted})
    return {"schema": "openagents.microluna.iteration-speed.v1",
            "collected_at": datetime.now(timezone.utc).isoformat(),
            "pins": read(experiment / "pins.json"), "state": status.get("state"),
            "summary": summary, "trials": rows, "interruptions": interruptions,
            "cost_limitations": "Usage valuations, not invoices. Cancelled in-flight calls may have no usage response; recorded interrupted cost is a lower bound."}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--state-root", type=Path, default=Path.home() / ".openagents/terminal-bench")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    body = collect(args.state_root)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(body, indent=2) + "\n")
    print(json.dumps(body["summary"], indent=2))
