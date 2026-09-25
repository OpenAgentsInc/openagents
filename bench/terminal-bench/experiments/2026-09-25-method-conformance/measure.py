#!/usr/bin/env python3
"""Join verify.method_conformance's offline records with rewards, and count.

Reads `<records>/<task>/conformance.json` and `labels.json`, written by
`coder-one checks conformance offline`, and writes `<records>/summary.json`.
Every identification and check result comes from the records; this script
only joins them with rewards and counts, by the rules in `protocol.md`:

- a (workspace, method) pair fails when any function tied to the method
  failed a property, passes when every tied function ran and passed every
  property, and is unknown otherwise;
- fail precision: of graded failing pairs, the fraction whose workspace
  failed the verifier; pass agreement: of graded passing pairs, the
  fraction whose workspace passed; each with a 95% Wilson interval;
- by method within task, pooled over the report set, and at the
  workspace level. The source task is reported apart and never pooled.

Usage: measure.py RECORDS [--source TASK]...
"""
import argparse
import json
import math
from collections import Counter, defaultdict
from pathlib import Path


def wilson(k, n, z=1.959964):
    if n == 0:
        return None
    p = k / n
    centre = (p + z * z / (2 * n)) / (1 + z * z / n)
    half = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / (1 + z * z / n)
    return [round(max(0.0, centre - half), 4), round(min(1.0, centre + half), 4)]


def ratio(k, n):
    return {"k": k, "n": n, "value": round(k / n, 4) if n else None, "wilson_95": wilson(k, n)}


def row_verdict(row):
    checked = row["checked"]
    if checked["status"] != "ran":
        return "unknown"
    if any(p["status"] != "passed" for p in checked.get("properties", [])):
        return "fail"
    return "pass"


def pair_outcome(verdicts):
    if "fail" in verdicts:
        return "fail"
    if verdicts and all(v == "pass" for v in verdicts):
        return "pass"
    return "unknown"


def label(reward):
    if reward is None:
        return None
    return "pass" if reward >= 0.999 else "fail"


def load(records):
    pairs = []
    workspaces = []
    tasks = {}
    for path in sorted(Path(records).glob("*/conformance.json")):
        record = json.loads(path.read_text())
        labels = json.loads((path.parent / "labels.json").read_text())
        by_trial = {t["trial"]: t for t in labels["trials"]}
        task = record["task"]
        tied = Counter(d["method"] for d in record["distinct"] if d["method"])
        tasks[task] = {
            "image": record.get("image"),
            "workspaces": len(record["trials"]),
            "restore_errors": sum(1 for t in record["trials"] if t.get("error")),
            "distinct_functions": len(record["distinct"]),
            "tied_functions": dict(tied),
            "jev_usd": record.get("jev_usd", 0.0),
        }
        for t in record["trials"]:
            lab = by_trial.get(t["trial"], {})
            verdict = label(lab.get("reward"))
            by_method = defaultdict(list)
            statuses = Counter()
            for row in t.get("rows", []):
                by_method[row["method"]].append(row_verdict(row))
                statuses[row["checked"]["status"]] += 1
            outcomes = {m: pair_outcome(v) for m, v in by_method.items()}
            workspaces.append({
                "task": task,
                "trial": t["trial"],
                "kind": t["kind"],
                "candidates": t.get("candidates", 0),
                "tied": t.get("tied", 0),
                "check_error": t.get("check_error"),
                "statuses": dict(statuses),
                "outcomes": outcomes,
                "label": verdict,
                "reward_source": lab.get("reward_source"),
                "failures": [
                    {k: f[k] for k in ("method", "function", "property", "observed", "expected")}
                    for f in t.get("failures", [])
                ],
            })
            for method, outcome in outcomes.items():
                pairs.append({"task": task, "trial": t["trial"], "method": method,
                              "outcome": outcome, "label": verdict})
    return tasks, workspaces, pairs


def rates(items):
    fails = [p for p in items if p["outcome"] == "fail" and p["label"]]
    passes = [p for p in items if p["outcome"] == "pass" and p["label"]]
    return {
        "pairs": len(items),
        "fail": sum(1 for p in items if p["outcome"] == "fail"),
        "pass": sum(1 for p in items if p["outcome"] == "pass"),
        "unknown": sum(1 for p in items if p["outcome"] == "unknown"),
        "graded_fail": len(fails),
        "graded_pass": len(passes),
        "fail_precision": ratio(sum(1 for p in fails if p["label"] == "fail"), len(fails)),
        "pass_agreement": ratio(sum(1 for p in passes if p["label"] == "pass"), len(passes)),
        "tasks_with_graded_fail": len({p["task"] for p in fails}),
    }


def workspace_units(workspaces):
    units = []
    for w in workspaces:
        if not w["outcomes"]:
            continue
        units.append({"task": w["task"], "trial": w["trial"], "method": "any",
                      "outcome": pair_outcome(list(w["outcomes"].values())), "label": w["label"]})
    return units


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("records")
    parser.add_argument("--source", action="append", default=["embedding-drift-monitor"])
    args = parser.parse_args()
    tasks, workspaces, pairs = load(args.records)
    source = set(args.source)
    report = [p for p in pairs if p["task"] not in source]
    held = [p for p in pairs if p["task"] in source]
    summary = {
        "sources": sorted(source),
        "tasks": tasks,
        "totals": {
            "tasks": len(tasks),
            "workspaces": len(workspaces),
            "graded_workspaces": sum(1 for w in workspaces if w["label"]),
            "workspaces_with_tied_function": sum(1 for w in workspaces if w["tied"]),
            "restore_errors": sum(t["restore_errors"] for t in tasks.values()),
            "distinct_functions": sum(t["distinct_functions"] for t in tasks.values()),
            "tied_functions": sum(sum(t["tied_functions"].values()) for t in tasks.values()),
            "jev_usd": round(sum(t["jev_usd"] for t in tasks.values()), 6),
            "check_statuses": dict(sum((Counter(w["statuses"]) for w in workspaces), Counter())),
        },
        "report_set": {
            "by_method": {m: rates([p for p in report if p["method"] == m])
                          for m in sorted({p["method"] for p in report})},
            "by_task_method": {
                f"{t} / {m}": rates([p for p in report if p["task"] == t and p["method"] == m])
                for t, m in sorted({(p["task"], p["method"]) for p in report})
            },
            "pooled_pairs": rates(report),
            "workspace_level": rates([u for u in workspace_units(workspaces) if u["task"] not in source]),
        },
        "source_task": {
            "by_method": {m: rates([p for p in held if p["method"] == m])
                          for m in sorted({p["method"] for p in held})},
            "pooled_pairs": rates(held),
            "workspace_level": rates([u for u in workspace_units(workspaces) if u["task"] in source]),
        },
        "workspaces": [w for w in workspaces if w["tied"]],
    }
    pooled = summary["report_set"]["pooled_pairs"]
    low = (pooled["fail_precision"]["wilson_95"] or [0.0])[0]
    summary["admission"] = {
        "rule": "graded fail pairs >= 5 on >= 2 report-set tasks, and the pooled fail precision's lower Wilson bound >= 0.6",
        "graded_fail": pooled["graded_fail"],
        "tasks_with_graded_fail": pooled["tasks_with_graded_fail"],
        "fail_precision_low": low,
        "admitted": pooled["graded_fail"] >= 5 and pooled["tasks_with_graded_fail"] >= 2 and low >= 0.6,
    }
    out = Path(args.records) / "summary.json"
    out.write_text(json.dumps(summary, indent=2) + "\n")
    print(json.dumps({k: summary[k] for k in ("totals", "admission")}, indent=2))
    print(json.dumps(summary["report_set"]["by_method"], indent=1))
    print(json.dumps(summary["source_task"], indent=1))


if __name__ == "__main__":
    main()
