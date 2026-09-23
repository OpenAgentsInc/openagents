"""Render the Terminal-Bench 4.0 scoreboard: our arms beside the leaderboard.

Reads graded trials from the local jobs directory (and retained traces when
the jobs are gone) and the per-task leaderboard reference, and prints a
Markdown table per task plus totals. A trial counts when the verifier wrote a
reward; setup, disk, and revoked-token failures are not results and are
listed separately.

    python3 tools/tb4_scoreboard.py [--arms a,b,c] [--jobs DIR]
"""

from __future__ import annotations

import argparse
import glob
import json
import os
from pathlib import Path

HERE = Path(__file__).resolve().parent.parent
REFERENCE = HERE / "reference" / "tb4-leaderboard.json"
DEFAULT_ARMS = [
    "coder-one-tunable-v5",
    "coder-one-tunable-v4",
    "coder-one-tunable-v3",
    "coder-one-tunable-v2",
    "claude-code-opus",
]


def graded(jobs: Path, arm: str) -> dict[str, list[tuple[float, float | None]]]:
    """Per task, the (reward, cost) of every graded trial of `arm`."""
    out: dict[str, list[tuple[float, float | None]]] = {}
    for job in sorted(glob.glob(str(jobs / f"tb4--{arm}--*"))):
        task = job.split("--")[-1]
        if task[-2:-1] == "-" and task[-1].isdigit():
            task = task[:-2]
        for result in glob.glob(f"{job}/*__*/result.json"):
            try:
                data = json.loads(Path(result).read_text())
            except (OSError, ValueError):
                continue
            reward = ((data.get("verifier_result") or {}).get("rewards") or {}).get("reward")
            if reward is None:
                continue
            cost = None
            usage = Path(result).parent / "agent" / "episode" / "evaluation" / "usage.json"
            if usage.exists():
                try:
                    total = json.loads(usage.read_text()).get("cost") or {}
                    cost = total.get("amount_usd")
                    if cost is None:
                        cost = total.get("lower_bound_usd")
                except (OSError, ValueError):
                    pass
            if cost is None:
                cost = (data.get("agent_result") or {}).get("cost_usd")
            out.setdefault(task, []).append((float(reward), cost))
    return out


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--arms", default=",".join(DEFAULT_ARMS))
    parser.add_argument(
        "--jobs", default=os.path.expanduser("~/.openagents/terminal-bench/jobs")
    )
    args = parser.parse_args()
    arms = [a for a in args.arms.split(",") if a]
    reference = json.loads(REFERENCE.read_text())
    entries = reference["entries"]
    tasks = sorted({t for e in entries for t in e["tasks"]})

    def rate(entry: dict, task: str) -> float | None:
        cell = entry["tasks"].get(task)
        if not cell or not cell.get("trials"):
            return None
        return cell["successes"] / cell["trials"]

    best = max(entries, key=lambda e: e["metrics"]["accuracy"])
    opus5 = next(
        (e for e in entries if e["model"] == "Opus 5" and e["reasoning_effort"] == "max"),
        None,
    )
    results = {arm: graded(Path(args.jobs), arm) for arm in arms}

    head = ["Task", "Best any row", f"{best['model']} {best['reasoning_effort']}"]
    if opus5:
        head.append("Opus 5 max")
    head += arms
    print("| " + " | ".join(head) + " |")
    print("| " + " | ".join("---" for _ in head) + " |")
    totals = {arm: [0, 0, 0.0] for arm in arms}
    for task in tasks:
        row = [f"`{task}`"]
        top = max((rate(e, task) or 0.0) for e in entries)
        row.append("never solved" if top == 0 else f"{top:.0%}")
        row.append(f"{rate(best, task):.0%}" if rate(best, task) is not None else "—")
        if opus5:
            row.append(f"{rate(opus5, task):.0%}" if rate(opus5, task) is not None else "—")
        for arm in arms:
            cells = results[arm].get(task)
            if not cells:
                row.append("·")
                continue
            passed = sum(1 for r, _ in cells if r == 1.0)
            cost = sum(c for _, c in cells if c is not None)
            totals[arm][0] += passed
            totals[arm][1] += len(cells)
            totals[arm][2] += cost
            row.append(f"{passed}/{len(cells)} · ${cost:.2f}")
        print("| " + " | ".join(row) + " |")
    print()
    print("| Arm | Graded trials | Passed | Pass rate | Cost of graded trials |")
    print("| --- | ---: | ---: | ---: | ---: |")
    for arm in arms:
        passed, n, cost = totals[arm]
        share = f"{passed / n:.0%}" if n else "—"
        print(f"| {arm} | {n} | {passed} | {share} | ${cost:.2f} |")


if __name__ == "__main__":
    main()
