#!/usr/bin/env python3
"""Count how often a green frozen acceptance suite agrees with the verifier.

Reads the retained `coder-one accept offline` records under `records/` and
writes `records/summary.json`. It makes no model call and runs no container.

A trial counts when its verifier reward is known and the suite has at least
one test. A zero-test suite can't be green, and `accept validity` leaves it
out too. A trial whose reward is unknown is left out here; `accept validity`
counts it as a verifier failure.
"""
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
RECORDS = HERE / "records"
PASSES = ["pass1-uncalibrated", "pass2-calibrated", "pass3-facts"]


def wilson(k, n):
    if not n:
        return {"k": k, "n": n, "value": None, "wilson_95": None}
    z = 1.96
    p = k / n
    d = 1 + z * z / n
    center = (p + z * z / (2 * n)) / d
    half = z * ((p * (1 - p) / n + z * z / (4 * n * n)) ** 0.5) / d
    return {
        "k": k,
        "n": n,
        "value": round(p, 4),
        "wilson_95": [round(max(0.0, center - half), 4), round(min(1.0, center + half), 4)],
    }


def table(rows):
    green = [r for r in rows if r["green"]]
    red = [r for r in rows if not r["green"]]
    passes = [r for r in rows if r["passed"]]
    failures = [r for r in rows if not r["passed"]]
    return {
        "trials": len(rows),
        "tasks": len({r["task"] for r in rows}),
        "green": len(green),
        "red": len(red),
        "green_and_passed": sum(r["passed"] for r in green),
        "green_and_failed": sum(not r["passed"] for r in green),
        "red_and_passed": sum(r["passed"] for r in red),
        "red_and_failed": sum(not r["passed"] for r in red),
        "pass_given_green": wilson(sum(r["passed"] for r in green), len(green)),
        "fail_given_red": wilson(sum(not r["passed"] for r in red), len(red)),
        "failures_caught": wilson(sum(not r["green"] for r in failures), len(failures)),
        "passes_kept": wilson(sum(r["green"] for r in passes), len(passes)),
        "agreement": wilson(sum(r["green"] == r["passed"] for r in rows), len(rows)),
    }


def measure(name):
    rows, tasks, excluded = [], [], []
    writer = jev = 0.0
    for path in sorted((RECORDS / name).glob("*/validity.json")):
        value = json.loads(path.read_text())
        suite = value["suite"]
        writer += suite["writer_usd"]
        jev += suite["jev_usd"]
        task_rows = []
        for entry in value["trials"]:
            trial = entry["trial"]
            run = entry.get("run")
            reason = None
            if run is None:
                reason = "no run: " + str(entry.get("error"))
            elif run["total"] == 0:
                reason = "the suite has no tests"
            elif trial["reward"] is None:
                reason = "the verifier reward is unknown"
            if reason:
                excluded.append({"task": value["task"], "trial": trial["trial"], "reason": reason})
                continue
            row = {
                "task": value["task"],
                "trial": trial["trial"],
                "job": trial["job"],
                "snapshot_graded": trial["snapshot_graded"],
                "passed": trial["reward"] >= 1.0,
                "green": run["green"],
                "tests_green": run["passed"],
                "tests": run["total"],
            }
            rows.append(row)
            task_rows.append(row)
        calls = {r["green"] for r in task_rows}
        tasks.append({
            "task": value["task"],
            "suite_status": suite["status"],
            "tests": len(suite["tests"]),
            "rejected": len(suite["rejected"]),
            "gaps": len(suite["gaps"]),
            "writer_usd": round(suite["writer_usd"], 6),
            "jev_usd": round(suite["jev_usd"], 6),
            "counted_trials": len(task_rows),
            "passed": sum(r["passed"] for r in task_rows),
            "green": sum(r["green"] for r in task_rows),
            "green_and_passed": sum(r["green"] and r["passed"] for r in task_rows),
            "red_and_passed": sum((not r["green"]) and r["passed"] for r in task_rows),
            "same_call_for_every_trial": len(calls) <= 1,
        })
    graded = [r for r in rows if r["snapshot_graded"]]
    return {
        "pass": name,
        "suites": len(tasks),
        "spend_usd": {"writer": round(writer, 6), "jev": round(jev, 6), "total": round(writer + jev, 6)},
        "snapshot_graded": table(graded),
        "all_snapshots": table(rows),
        "tasks": tasks,
        "excluded": excluded,
        "rows": rows,
    }


def main():
    out = {
        "schema": "openagents.acceptance-first.summary.v1",
        "source": "records/<pass>/<task>/validity.json from coder-one accept offline",
        "passes": [measure(name) for name in PASSES],
    }
    (RECORDS / "summary.json").write_text(json.dumps(out, indent=1) + "\n")
    for p in out["passes"]:
        for subset in ("snapshot_graded", "all_snapshots"):
            t = p[subset]
            print(
                f"{p['pass']:<20} {subset:<16} trials {t['trials']:>2} tasks {t['tasks']:>2} "
                f"green {t['green_and_passed']}/{t['green']} passed, red {t['red_and_failed']}/{t['red']} failed, "
                f"pass|green {t['pass_given_green']['wilson_95']}, failures caught "
                f"{t['failures_caught']['k']}/{t['failures_caught']['n']}"
            )
        print(f"{'':<20} spend {p['spend_usd']}; excluded {len(p['excluded'])}")


if __name__ == "__main__":
    main()
