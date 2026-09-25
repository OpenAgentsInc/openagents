#!/usr/bin/env python3
"""Tabulate the check-grades measurement from its records.

Usage: analyze.py [RECORDS_DIR] [--out records/results.json]

RECORDS_DIR (default records/) is what `coder-one accept grade` wrote:
grades/, rows.jsonl, and summary.json. The rules are in protocol.md: the
threshold is 0.5, frozen; AUC over (pass, fail) pairs with ties counting
half, on deduplicated workspaces with a known reward; admission needs the
graded key to separate for more scripts than the raw score on every task
with both outcomes, and to lower no script's AUC. The threshold sweep is
for reading only.
"""

import argparse
import glob
import json
import math
import os

HERE = os.path.dirname(os.path.abspath(__file__))
THRESHOLD = 0.5
SWEEP = [0.3, 0.4, 0.5, 0.6, 0.7]


def wilson(k, n, z=1.959964):
    if n == 0:
        return None
    p = k / n
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return [round(max(0.0, c - h), 3), round(min(1.0, c + h), 3)]


def frac(score):
    if score is None:
        return -1.0
    passed, total = score
    return 0.0 if total == 0 else passed / total


def auc(passes, fails):
    if not passes or not fails:
        return None
    s = 0.0
    for p in passes:
        for f in fails:
            s += 1.0 if p > f else 0.5 if p == f else 0.0
    return s / (len(passes) * len(fails))


def follows_at(grades, threshold):
    out = set()
    for line in grades["lines"]:
        sup = line.get("support") or {}
        ps = [p for p in (sup.get("task"), sup.get("baseline"), sup.get("standard")) if p is not None]
        if grades["split"] == "lines" and ps and max(ps) >= threshold:
            out.add(line["id"])
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("records", nargs="?", default=os.path.join(HERE, "records"))
    ap.add_argument("--out", default=None)
    ap.add_argument("--compare", default=None, help="another records directory, for Jev drift")
    args = ap.parse_args()
    grades = {}
    for path in glob.glob(os.path.join(args.records, "grades", "*.json")):
        g = json.load(open(path))
        grades[os.path.basename(path)[:-5]] = g
    rows = [json.loads(l) for l in open(os.path.join(args.records, "rows.jsonl"))]
    by_script = {}
    for r in rows:
        by_script.setdefault(r["script"], []).append(r)
    results = {"threshold": THRESHOLD, "scripts": [], "sweep": {}, "tasks": {}}
    for script, mine in sorted(by_script.items()):
        g = grades[script.replace("/", "--")]
        known = [r for r in mine if r["dup_of"] is None and r["reward"] is not None]
        passes = [r for r in known if r["reward"] >= 1.0]
        fails = [r for r in known if r["reward"] < 1.0]
        row = {
            "script": script,
            "task": mine[0]["task"],
            "split": g["split"],
            "lines": len(g["lines"]),
            "follows": sum(1 for l in g["lines"] if l["grade"] == "follows"),
            "advisory": sum(1 for l in g["lines"] if l["grade"] == "advisory"),
            "basis": {b: sum(1 for l in g["lines"] if l["basis"] == b) for b in ("task", "baseline", "standard")},
            "passes": len(passes),
            "fails": len(fails),
            "raw_full_on_fails": sum(1 for r in fails if r["score"] and r["score"][0] == r["score"][1]),
            "raw_full_on_passes": sum(1 for r in passes if r["score"] and r["score"][0] == r["score"][1]),
            "graded_full_on_fails": sum(1 for r in fails if r["supported"] and r["supported"][0] == r["supported"][1]),
            "graded_full_on_passes": sum(1 for r in passes if r["supported"] and r["supported"][0] == r["supported"][1]),
            "raw_auc": auc([frac(r["score"]) for r in passes], [frac(r["score"]) for r in fails]),
            "graded_auc": auc(
                [(frac(r["supported"]), frac(r["score"])) for r in passes],
                [(frac(r["supported"]), frac(r["score"])) for r in fails],
            ),
            # Advisory lines that failed on a pass: the wrong expectations
            # the grade kept from ranking.
            "advisory_failing_passes": sorted(
                {l["id"] for r in passes for l in g["lines"] if l["grade"] == "advisory" and r["lines"].get(l["id"]) is False}
            ),
            "follows_failing_passes": sorted(
                {l["id"] for r in passes for l in g["lines"] if l["grade"] == "follows" and r["lines"].get(l["id"]) is False}
            ),
            "reproduced": [r["recorded"] == r["score"] for r in mine if r["recorded"] is not None],
        }
        results["scripts"].append(row)
        for t in SWEEP:
            ids = follows_at(g, t)

            def key(r):
                if not r["lines"]:
                    return (-1.0, frac(r["score"]))
                ok = sum(1 for i in ids if r["lines"].get(i))
                return ((ok / len(ids)) if ids else 0.0, frac(r["score"]))

            a = auc([key(r) for r in passes], [key(r) for r in fails])
            results["sweep"].setdefault(str(t), []).append({"script": script, "follows": len(ids), "graded_auc": a})
    for task in sorted({s["task"] for s in results["scripts"]}):
        mine = [s for s in results["scripts"] if s["task"] == task]
        both = [s for s in mine if s["raw_auc"] is not None]
        raw_sep = sum(1 for s in both if s["raw_auc"] >= 1.0)
        graded_sep = sum(1 for s in both if s["graded_auc"] >= 1.0)
        lowered = sum(1 for s in both if s["graded_auc"] < s["raw_auc"])
        raised = sum(1 for s in both if s["graded_auc"] > s["raw_auc"])
        results["tasks"][task] = {
            "scripts": len(mine),
            "scripts_with_both_outcomes": len(both),
            "raw_separates": raw_sep,
            "graded_separates": graded_sep,
            "graded_separates_where_raw_did_not": sum(1 for s in both if s["graded_auc"] >= 1.0 > s["raw_auc"]),
            "auc_raised": raised,
            "auc_lowered": lowered,
            "admitted_here": (graded_sep > raw_sep and lowered == 0) if both else None,
        }
    lines = [l for g in grades.values() for l in g["lines"]]
    follows = sum(1 for l in lines if l["grade"] == "follows")
    results["lines"] = {"total": len(lines), "follows": follows, "advisory": len(lines) - follows,
                        "follows_share": round(follows / len(lines), 3), "follows_interval": wilson(follows, len(lines))}
    classes = {}
    for l in lines:
        c = (l.get("authority") or {}).get("class")
        if c:
            classes[c] = classes.get(c, 0) + 1
    results["classes"] = classes
    results["green_at_start"] = sum(
        1 for l in lines if ((l.get("authority") or {}).get("evidence") or {}).get("green_at_start") is True
    )
    if args.compare:
        other = {}
        for path in glob.glob(os.path.join(args.compare, "grades", "*.json")):
            for l in json.load(open(path))["lines"]:
                other[(os.path.basename(path), l["id"])] = l
        flips = deltas = 0
        pairs = 0
        for name, g in grades.items():
            for l in g["lines"]:
                o = other.get((name + ".json", l["id"]))
                if not o or not l.get("support") or not o.get("support"):
                    continue
                a = max(p for p in l["support"].values() if p is not None)
                b = max(p for p in o["support"].values() if p is not None)
                pairs += 1
                deltas = max(deltas, abs(a - b))
                flips += (a >= THRESHOLD) != (b >= THRESHOLD)
        results["drift"] = {"lines": pairs, "support_flips_at_threshold": flips, "max_abs_delta": round(deltas, 3)}
    tasks_with_both = [t for t in results["tasks"].values() if t["admitted_here"] is not None]
    results["admitted"] = bool(tasks_with_both) and all(t["admitted_here"] for t in tasks_with_both)
    text = json.dumps(results, indent=2)
    if args.out:
        with open(args.out, "w") as f:
            f.write(text + "\n")
    print(text)


if __name__ == "__main__":
    main()
