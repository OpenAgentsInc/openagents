#!/usr/bin/env python3
"""Tabulates the matched mini-task runs for issue #9637.

Each argument is the output of one `coder-one minitask run ... --json`
named `<arm>-<task>-<attempt>.out`. The script writes one JSON line per run
(arm, task, attempt, grade, sessions, whether the review ran and what
started it, the review's time and cost, and the run's cost) and prints the
per-arm totals. The `v15` arm is the finish-rule experiment's
`microluna-v15` runs, `v15-<task>-<attempt>.out`.

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
    rows = []
    for path in sorted(sys.argv[1:]):
        arm, rest = os.path.basename(path)[:-4].split("-", 1)
        task, attempt = rest.rsplit("-", 1)
        value = result(path)
        if value is None:
            rows.append({"arm": arm, "task": task, "attempt": int(attempt), "error": "no result"})
            continue
        manifest = value["manifest"]
        sessions = manifest["session"]["sessions"]
        moves = manifest["session"].get("moves") or []
        rule = next((m for m in moves if m.get("kind") == "lean.review_rule"), None)
        reviews = [s for s in sessions if s.get("group") == "the self-check"]
        rows.append({
            "arm": arm,
            "task": task,
            "attempt": int(attempt),
            "grade": manifest["grade"]["verdict"],
            "sessions": len(sessions),
            "statuses": [s["status"] for s in sessions],
            "review_ran": bool(reviews),
            "trigger": rule and rule.get("trigger"),
            "review_reason": rule and rule.get("reason"),
            "review_ms": sum(s.get("milliseconds") or 0 for s in reviews),
            "review_usd": round(sum(s.get("cost_usd") or 0.0 for s in reviews), 6),
            "cost_usd": round(sum(s.get("cost_usd") or 0.0 for s in sessions), 6),
            "stopped": manifest["session"].get("stopped"),
        })
    arms = defaultdict(lambda: {"runs": 0, "passed": 0, "reviews": 0, "review_ms": 0,
                                "review_usd": 0.0, "cost_usd": 0.0})
    for row in rows:
        print(json.dumps(row))
        if "error" in row:
            continue
        total = arms[row["arm"]]
        total["runs"] += 1
        total["passed"] += row["grade"] == "passed"
        total["reviews"] += row["review_ran"]
        total["review_ms"] += row["review_ms"]
        total["review_usd"] += row["review_usd"]
        total["cost_usd"] += row["cost_usd"]
    for arm, total in sorted(arms.items()):
        print(f"# {arm}: {total['passed']} of {total['runs']} passed, "
              f"{total['reviews']} reviews ({total['review_ms'] / 1000:.0f} s, "
              f"${total['review_usd']:.4f}), ${total['cost_usd']:.4f} in all", file=sys.stderr)


if __name__ == "__main__":
    main()
