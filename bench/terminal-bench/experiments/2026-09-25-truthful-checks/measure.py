#!/usr/bin/env python3
"""Measure Rust-produced predictions, clustering repeated trials by task."""
import argparse
import collections
import json
import random
from pathlib import Path

SIGNALS = ["checks.final", "verdict.combined", "verdict.corroborated", "microluna.local-score"]


def rate(k, n):
    if not n:
        return {"correct": k, "total": n, "value": None, "wilson_95": None}
    z = 1.96
    p = k / n
    d = 1 + z * z / n
    center = (p + z * z / (2 * n)) / d
    half = z * ((p * (1 - p) / n + z * z / (4 * n * n)) ** 0.5) / d
    return {"correct": k, "total": n, "value": p, "wilson_95": [max(0, center - half), min(1, center + half)]}


def counts(rows, signal):
    calls = [r for r in rows if r["calls"][signal] == "fail"]
    correct = sum(r["reward"] < 1 for r in calls)
    return correct, len(calls), sum(r["reward"] < 1 for r in rows)


def stats(rows, signal):
    k, n, failures = counts(rows, signal)
    passes = [r for r in rows if r["calls"][signal] == "pass"]
    return {"fail_precision": rate(k, n), "failure_recall": rate(k, failures),
            "pass_precision": rate(sum(r["reward"] == 1 for r in passes), len(passes)),
            "unknown": sum(r["calls"][signal] is None for r in rows)}


def paired_bootstrap(rows, newer, baseline):
    grouped = collections.defaultdict(list)
    for row in rows:
        grouped[row["task"]].append(row)
    groups = list(grouped.values())
    rng = random.Random(9584)
    differences = {"precision": [], "recall": []}
    for _ in range(10000):
        sample = [r for g in rng.choices(groups, k=len(groups)) for r in g]
        a, b = counts(sample, newer), counts(sample, baseline)
        for metric, index in [("precision", 1), ("recall", 2)]:
            if a[index] and b[index]:
                differences[metric].append(a[0] / a[index] - b[0] / b[index])
    result = {}
    for metric, values in differences.items():
        values.sort()
        result[metric] = {"valid_resamples": len(values), "undefined_resamples": 10000 - len(values),
                          "percentile_95": [values[int(.025 * (len(values) - 1))], values[int(.975 * (len(values) - 1))]] if values else None}
    return result


def measure(rows):
    return {"trials": len(rows), "tasks": len({r["task"] for r in rows}),
            "signals": {s: stats(rows, s) for s in SIGNALS},
            "paired_task_bootstrap": {s: paired_bootstrap(rows, "verdict.corroborated", s)
                                      for s in ["checks.final", "verdict.combined"]}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("historical", type=Path)
    parser.add_argument("microluna", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    historical = json.loads(args.historical.read_text())["predictions"]
    micro = json.loads(args.microluna.read_text())["predictions"]
    result = {"schema": "openagents.truthful-checks-comparison.v1", "seed": 9584, "resamples": 10000,
              "historical_reused_holdout": measure([r for r in historical if r["split"] == "held-out"]),
              "microluna_development": measure(micro),
              "notes": ["Wilson intervals are descriptive trial-level intervals.",
                        "Bootstrap resamples whole tasks; undefined precision is omitted and counted.",
                        "The historical comparison was previously inspected; the Microluna tasks were repeatedly studied.",
                        "Neither cohort is untouched confirmation of generalization."]}
    args.output.write_text(json.dumps(result, indent=2) + "\n")


if __name__ == "__main__":
    main()
