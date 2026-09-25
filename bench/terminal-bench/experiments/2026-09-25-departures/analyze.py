#!/usr/bin/env python3
"""Score the departure miners against the task anatomy's decisive facts.

Usage: analyze.py SUITE_JSON [--out records/results.json]

SUITE_JSON is the output of
`coder-one component suite evidence.departures --json` over the fixtures
`build_workspaces.py` writes. The rules are in protocol.md: rows listed
at each source's threshold (0.5, frozen), at most 8 per source; recall
over the anatomy's instruction and workspace facts, verifier-only
separately; precision over listed rows; 95% Wilson intervals.
"""

import argparse
import json
import math
import os

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "../../../.."))
ANATOMY = os.path.join(ROOT, "docs/terminal-bench/2026-09-24-task-anatomy.json")
SOURCES = ["rationale", "docstring", "standard-method"]
THRESHOLD = {"rationale": 0.5, "docstring": 0.5, "standard-method": 0.5}
MAX_LISTED = 8
FIT = {
    "atrx-vep-crispr", "biped-contact-dynamics", "bun-sourcemap-leak",
    "data-anonymization", "html-js-filter", "intrastat-meldung", "ks-solver-cpp",
    "layout-config-recreation", "vba-userform-port", "vf2-speedup-networkx",
}
TASKS = sorted(FIT | {
    "coq-block-bound", "embedding-drift-monitor", "fin-saccr-rwa", "gsea-proteomics",
    "interleaved-vigenere", "session-window-debug", "shadow-relay", "sound-change-cascade",
})


def wilson(k, n, z=1.959964):
    if n == 0:
        return None
    p = k / n
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return [round(max(0.0, c - h), 3), round(min(1.0, c + h), 3)]


def listed(rows, source, threshold):
    mine = [r for r in rows if r["kind"] == source and r["p"] is not None and r["p"] >= threshold]
    mine.sort(key=lambda r: -r["p"])
    return mine[:MAX_LISTED]


def score(rows_by_task, facts, labels, sources, thresholds, tasks):
    """Recall and precision for the union of `sources` over `tasks`."""
    named = set()
    hits = total = 0
    for task in tasks:
        label = {(l["kind"], l["file"], l["line"]): l["facts"] for l in labels.get(task, [])}
        for source in sources:
            for row in listed(rows_by_task.get(task, []), source, thresholds[source]):
                total += 1
                got = label.get((row["kind"], row["file"], row["line"]), [])
                if got:
                    hits += 1
                named.update((task, f) for f in got)
    out = {"listed": total, "hits": hits,
           "precision": round(hits / total, 3) if total else None,
           "precision_ci": wilson(hits, total)}
    for group, kinds in (("visible", {"instruction", "workspace"}), ("verifier_only", {"verifier-only"})):
        pool = [(t, f["id"]) for t in tasks for f in facts.get(t, []) if f["source_kind"] in kinds]
        k = sum(1 for key in pool if key in named)
        out[group] = {"named": k, "facts": len(pool),
                      "recall": round(k / len(pool), 3) if pool else None,
                      "recall_ci": wilson(k, len(pool)),
                      "which": sorted(f"{t}:{f}" for t, f in pool if (t, f) in named)}
    return out


def ceiling(rows_by_task, facts, labels, source, tasks):
    """What every candidate of a source names, before Jev ranks it."""
    everything = {s: 0.0 for s in SOURCES}
    unbounded = dict(rows_by_task)
    for task in tasks:
        rows = [dict(r, p=1.0) for r in rows_by_task.get(task, [])]
        unbounded[task] = rows
    global MAX_LISTED
    saved, MAX_LISTED = MAX_LISTED, 10 ** 6
    try:
        return score(unbounded, facts, labels, [source], everything, tasks)
    finally:
        MAX_LISTED = saved


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("suite")
    parser.add_argument("--out", default=os.path.join(HERE, "records/results.json"))
    args = parser.parse_args()
    suite = json.load(open(args.suite))["result"]
    anatomy = json.load(open(ANATOMY))["tasks"]
    facts = {t: v["decisive_facts"] for t, v in anatomy.items()}
    labels = json.load(open(os.path.join(HERE, "labels.json")))["labels"]
    rows_by_task = {f["fixture"]: (f.get("output") or {}).get("rows", []) for f in suite["fixtures"]}
    assert sorted(rows_by_task) == TASKS, sorted(rows_by_task)
    report = sorted(set(TASKS) - FIT)
    result = {
        "schema": "openagents.departures-offline-results.v1",
        "implementation": suite["implementation"],
        "jev_mode": suite["jev_mode"],
        "thresholds": THRESHOLD,
        "max_listed": MAX_LISTED,
        "jev_usd": round(sum((f.get("metrics") or {}).get("jev_usd", 0) for f in suite["fixtures"]), 6),
        "candidates": {t: {s: sum(1 for r in rows if r["kind"] == s) for s in SOURCES}
                       for t, rows in rows_by_task.items() if rows},
        "fit_candidates": sum(len(rows_by_task[t]) for t in FIT),
        "by_source": {}, "unions": {}, "report_only": {}, "ceiling": {}, "sweep": {},
    }
    for s in SOURCES:
        result["by_source"][s] = score(rows_by_task, facts, labels, [s], THRESHOLD, TASKS)
        result["report_only"][s] = score(rows_by_task, facts, labels, [s], THRESHOLD, report)
        result["ceiling"][s] = ceiling(rows_by_task, facts, labels, s, TASKS)
        result["sweep"][s] = {
            f"{t / 100:.2f}": {k: v for k, v in score(
                rows_by_task, facts, labels, [s], {**THRESHOLD, s: t / 100}, TASKS).items()
                if k in ("listed", "hits", "precision")} | {
                "visible_named": score(rows_by_task, facts, labels, [s],
                                       {**THRESHOLD, s: t / 100}, TASKS)["visible"]["named"]}
            for t in range(5, 100, 5)
        }
    for extra in SOURCES[1:]:
        result["unions"][f"rationale+{extra}"] = score(
            rows_by_task, facts, labels, ["rationale", extra], THRESHOLD, TASKS)
    result["unions"]["all"] = score(rows_by_task, facts, labels, SOURCES, THRESHOLD, TASKS)
    base = result["by_source"]["rationale"]
    result["admitted"] = []
    for extra in SOURCES[1:]:
        union = result["unions"][f"rationale+{extra}"]
        own = result["by_source"][extra]
        raises = union["visible"]["named"] > base["visible"]["named"]
        keeps = own["precision"] is not None and base["precision"] is not None \
            and own["precision"] >= base["precision"]
        result["by_source"][extra]["admission"] = {"raises_recall": raises, "keeps_precision": keeps}
        if raises and keeps:
            result["admitted"].append(extra)
    result["rows"] = {t: rows for t, rows in rows_by_task.items() if rows}
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    with open(args.out, "w") as f:
        json.dump(result, f, indent=2)
        f.write("\n")
    for s in SOURCES:
        b = result["by_source"][s]
        print(s, "listed", b["listed"], "hits", b["hits"], "precision", b["precision"], b["precision_ci"],
              "visible", b["visible"]["named"], "/", b["visible"]["facts"], b["visible"]["recall_ci"],
              "verifier-only", b["verifier_only"]["named"], "/", b["verifier_only"]["facts"])
    for name, u in result["unions"].items():
        print(name, "precision", u["precision"], "visible", u["visible"]["named"], u["visible"]["which"])
    print("admitted", result["admitted"], "usd", result["jev_usd"])


if __name__ == "__main__":
    main()
