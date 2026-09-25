#!/usr/bin/env python3
"""Tabulates the matched mini-task runs for issue #9638.

Each argument is the output of one `coder-one minitask run ... --json`
named `<arm>-<task>-<attempt>.out`. The script writes one JSON line per run
(arm, task, attempt, grade, sessions, turns per session, refusals, and
cost) and prints the per-arm totals.

    python3 minitasks.py OUT... > records/minitasks.jsonl
"""

import json
import os
import sys
from collections import defaultdict


def result(path):
    text = open(path).read()
    at = text.rfind("\n{\n")
    if at < 0:
        return None
    return json.loads(text[at:])


def main():
    arms = defaultdict(lambda: {"runs": 0, "passed": 0, "sessions": 0, "turns": 0,
                                "refusals": 0, "unverified": 0, "cost_usd": 0.0})
    for path in sorted(sys.argv[1:]):
        arm, rest = os.path.basename(path)[:-4].split("-", 1)
        task, attempt = rest.rsplit("-", 1)
        value = result(path)
        if value is None:
            print(json.dumps({"arm": arm, "task": task, "attempt": int(attempt),
                              "error": "no result"}))
            continue
        manifest = value["manifest"]
        sessions = manifest["session"]["sessions"]
        row = {
            "arm": arm,
            "task": task,
            "attempt": int(attempt),
            "grade": manifest["grade"]["verdict"],
            "sessions": len(sessions),
            "turns": [s["turns"] for s in sessions],
            "statuses": [s["status"] for s in sessions],
            "refusals": [s.get("finish_refusals", 0) for s in sessions],
            "unverified": [s.get("unverified", False) for s in sessions],
            "cost_usd": round(sum(s.get("cost_usd") or 0.0 for s in sessions), 6),
            "stopped": manifest["session"].get("stopped"),
        }
        print(json.dumps(row))
        total = arms[arm]
        total["runs"] += 1
        total["passed"] += row["grade"] == "passed"
        total["sessions"] += row["sessions"]
        total["turns"] += sum(row["turns"])
        total["refusals"] += sum(row["refusals"])
        total["unverified"] += sum(row["unverified"])
        total["cost_usd"] += row["cost_usd"]
    for arm, total in sorted(arms.items()):
        turns = total["turns"] / total["sessions"] if total["sessions"] else 0
        print(f"# {arm}: {total['passed']} of {total['runs']} passed, "
              f"{turns:.1f} turns per session over {total['sessions']} sessions, "
              f"{total['refusals']} refusals, {total['unverified']} unverified, "
              f"${total['cost_usd']:.4f}", file=sys.stderr)


if __name__ == "__main__":
    main()
