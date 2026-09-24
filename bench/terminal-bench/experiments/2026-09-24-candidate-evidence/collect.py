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
    experiment = root / "experiments/candidate-evidence-9607"
    status = read(experiment / "status.json")
    rows = []
    for item in status.get("trials", []):
        job = item.get("job") or item.get("job_name")
        if not job:
            job = f"tb4--{item['arm']}--{item['task']}--candidate-evidence-9607-r{item['attempt']}"
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
        if trial:
            for path in sorted(trial.glob("agent/episode/artifacts/lean-*/selection.json")):
                moves = read(path)
                selections.append({"path": str(path.relative_to(trial)), "moves": moves})
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
        })
    summary = []
    for task in read(experiment / "pins.json").get("tasks", []):
        for arm in read(experiment / "experiment.json").get("arms", []):
            group = [r for r in rows if r["task"] == task and r["arm"] == arm]
            completed = [r for r in group if r["reward"] is not None]
            passes = sum(r["reward"] == 1 for r in completed)
            known = [r["cost_usd"] for r in group if r["cost_usd"] is not None]
            total = sum(known) if len(known) == len(group) else None
            summary.append({"task": task, "arm": arm, "planned": len(group),
                            "graded": len(completed), "passes": passes,
                            "known_cost_usd": sum(known), "total_cost_usd": total,
                            "cost_per_pass_usd": total / passes if total is not None and passes else None})
    return {"schema": "openagents.microluna.candidate-evidence.v1",
            "collected_at": datetime.now(timezone.utc).isoformat(),
            "pins": read(experiment / "pins.json"), "state": status.get("state"),
            "summary": summary, "trials": rows}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--state-root", type=Path, default=Path.home() / ".openagents/terminal-bench")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    body = collect(args.state_root)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(body, indent=2) + "\n")
    print(json.dumps(body["summary"], indent=2))
