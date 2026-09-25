"""Summarize every fire loop run on this machine as Markdown tables.

Reads each job under the jobs directory whose name starts with ``fire--``:
the watcher's ``fire/report.json`` and, when the harness finished the
trial, ``verifier/reward.txt``. Jev cost is the logged input tokens at the
published rate; Luna cost is the list-price estimate the logs carry.

Run: python3 bench/terminal-bench/fire/summarize.py [JOBS_DIR]
"""

import json
import statistics
import sys
from collections import defaultdict
from pathlib import Path

JOBS = Path(sys.argv[1]) if len(sys.argv) > 1 else Path.home() / ".openagents/terminal-bench/jobs"


def arm_of(trial: Path) -> str:
    config = trial.parent / "config.json"
    try:
        data = json.loads(config.read_text())
        agent = data.get("agents", [{}])[0] if isinstance(data.get("agents"), list) else data.get("agent", {})
        kwargs = agent.get("kwargs", {}) or {}
        policy = kwargs.get("policy", "")
        if policy:
            return Path(policy).stem
    except (OSError, ValueError, AttributeError, IndexError):
        pass
    return "?"


rows = []
for job in sorted(JOBS.glob("fire--*")):
    for trial in job.glob("*__*"):
        report = trial / "fire" / "report.json"
        if not report.is_file():
            continue
        r = json.loads(report.read_text())
        reward = None
        if (trial / "result.json").is_file() and (trial / "verifier" / "reward.txt").is_file():
            try:
                reward = float((trial / "verifier" / "reward.txt").read_text().strip())
            except ValueError:
                reward = None
        rows.append({
            "job": job.name,
            "task": r["task"],
            "arm": arm_of(trial),
            "outcome": r["outcome"],
            "rule": (r.get("stop") or {}).get("rule"),
            "seconds": r["seconds"],
            "luna": r["usd"]["model"],
            "jev": r["usd"]["jev"],
            "judge": r["usd"]["judge"],
            "reward": reward,
        })

print("| Job | Arm | Task | Outcome | Reward | Time | Luna | Jev | Judge |")
print("| --- | --- | --- | --- | --- | --- | --- | --- | --- |")
for x in rows:
    outcome = x["outcome"] + (f" ({x['rule']})" if x["rule"] else "")
    reward = "—" if x["reward"] is None else f"{x['reward']:g}"
    m, s = divmod(round(x["seconds"]), 60)
    print(f"| `{x['job'][6:]}` | `{x['arm']}` | `{x['task']}` | {outcome} | {reward} | {m}:{s:02d} | ${x['luna']:.4f} | ${x['jev']:.5f} | ${x['judge']:.4f} |")

print()
print("| Arm | Task | Graded runs | Passes | Median time | Mean Luna + Jev per run |")
print("| --- | --- | ---: | ---: | --- | --- |")
groups = defaultdict(list)
for x in rows:
    if x["reward"] is not None:
        groups[(x["arm"], x["task"])].append(x)
for (arm, task), xs in sorted(groups.items()):
    med = statistics.median(x["seconds"] for x in xs)
    m, s = divmod(round(med), 60)
    cost = statistics.mean(x["luna"] + x["jev"] for x in xs)
    print(f"| `{arm}` | `{task}` | {len(xs)} | {sum(1 for x in xs if x['reward'] >= 1)} | {m}:{s:02d} | ${cost:.4f} |")
