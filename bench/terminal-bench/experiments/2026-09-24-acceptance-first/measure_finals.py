#!/usr/bin/env python3
"""Summarize records/microluna-finals.json: suite green against the verifier.

Writes records/microluna-finals-summary.json. No model call.
"""
import json
from pathlib import Path

from measure import wilson

HERE = Path(__file__).resolve().parent


def table(rows):
    green = [r for r in rows if r["green"]]
    red = [r for r in rows if not r["green"]]
    passed = [r for r in rows if r["reward"] >= 1]
    failed = [r for r in rows if r["reward"] < 1]
    return {
        "trials": len(rows),
        "passed": len(passed),
        "green": len(green),
        "pass_given_green": wilson(sum(r["reward"] >= 1 for r in green), len(green)),
        "fail_given_red": wilson(sum(r["reward"] < 1 for r in red), len(red)),
        "passes_kept_green": wilson(sum(r["green"] for r in passed), len(passed)),
        "failures_caught_red": wilson(sum(not r["green"] for r in failed), len(failed)),
        "agreement": wilson(sum(r["green"] == (r["reward"] >= 1) for r in rows), len(rows)),
    }


def main():
    runs = json.loads((HERE / "records" / "microluna-finals.json").read_text())["runs"]
    errors = [r for r in runs if "error" in r]
    ok = [r for r in runs if "error" not in r and r.get("tests")]
    out = {"errors": len(errors), "suites": {}}
    for suite in sorted({r["suite"] for r in ok}):
        rows = [r for r in ok if r["suite"] == suite]
        per_task = {}
        for task in sorted({r["task"] for r in rows}):
            t = [r for r in rows if r["task"] == task]
            per_task[task] = {
                **table(t),
                "tests_green_when_passed": sorted({r["tests_green"] for r in t if r["reward"] >= 1}),
                "tests_green_when_failed": sorted({r["tests_green"] for r in t if r["reward"] < 1}),
                "tests": t[0]["tests"],
                "red_on_passes": sorted({x for r in t if r["reward"] >= 1 for x in r["red"]}),
            }
        out["suites"][suite] = {"all": table(rows), "tasks": per_task}
    (HERE / "records" / "microluna-finals-summary.json").write_text(json.dumps(out, indent=1) + "\n")
    print(json.dumps(out, indent=1))


if __name__ == "__main__":
    main()
