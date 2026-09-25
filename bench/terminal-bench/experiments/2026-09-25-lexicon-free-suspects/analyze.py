#!/usr/bin/env python3
"""Score the two suspect arms against the mechanically labeled defect sites.

Usage: analyze.py EXPORT_JSON [--out records/results.json]

EXPORT_JSON is `coder-one component suite evidence.departures --export
FILE` over the fixtures `build.py` writes. The rules are in protocol.md:
rows listed at p >= 0.5, most likely first, 8 at most; hit at 3 and 8,
false positives per task, and site recall at 8 over the scored
off-sample tasks; 95% Wilson intervals over tasks; task-grouped
bootstrap intervals for each difference, 10,000 resamples, seed 9652.
"""

import argparse
import json
import math
import os
import random

HERE = os.path.dirname(os.path.abspath(__file__))
ARMS = ("keywords", "lexicon-free")
THRESHOLD = 0.5
MAX_LISTED = 8
KS = (3, 8)
RESAMPLES = 10_000
SEED = 9652
USD_PER_MILLION_INPUT = 0.042


def wilson(k, n, z=1.959964):
    if n == 0:
        return None
    p = k / n
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return [round(max(0.0, c - h), 3), round(min(1.0, c + h), 3)]


def owner(spans, lines_of_file, line):
    """The span a comment at `line` belongs to (protocol.md, Labels)."""
    # A comment run directly above a definition belongs to it.
    starts = {s[0]: s for s in spans}
    k = line
    while k <= len(lines_of_file):
        t = lines_of_file[k - 1].strip() if k - 1 < len(lines_of_file) else ""
        if k in starts and k != line:
            return starts[k]
        comment = t.startswith(("#", "//", "/*", "*", "@", '"""', "'''")) or t.endswith("*/")
        if k == line or comment:
            k += 1
            continue
        break
    inside = [s for s in spans if s[0] <= line <= s[1]]
    return max(inside, key=lambda s: s[0]) if inside else None


def names(row, task_labels, texts):
    """The site keys a row names."""
    if row["line"] in task_labels.get("module_docstrings", {}).get(row["file"], []):
        return []
    spans = [tuple(s) for s in task_labels["spans"].get(row["file"], [])]
    text = texts.get(row["file"], [])
    s = owner(spans, text, row["line"])
    out = []
    for key, site in task_labels["sites"].items():
        if site["file"] != row["file"]:
            continue
        if s is None:
            # Outside every function: its line or the next code line. A
            # module docstring names nothing.
            nxt = row["line"] + 1
            while nxt <= len(text) and (not text[nxt - 1].strip()
                                        or text[nxt - 1].strip().startswith(("#", "//"))):
                nxt += 1
            if row["text"].startswith("in `module`"):
                continue
            if any(l in (row["line"], nxt) for l in site["lines"]):
                out.append(key)
        elif s[3]:
            # A class names the sites of the methods it holds, and its own.
            if s[0] <= site["start"] and site["end"] <= s[1]:
                out.append(key)
        elif site["name"] == s[2] and site["start"] == s[0]:
            out.append(key)
    return out


def listed(rows):
    mine = [r for r in rows if r.get("p") is not None and r["p"] >= THRESHOLD]
    mine.sort(key=lambda r: -r["p"])
    return mine[:MAX_LISTED]


def per_task(rows, task_labels, texts):
    top = listed(rows)
    named = [names(r, task_labels, texts) for r in top]
    return {
        "candidates": len(rows),
        "listed": len(top),
        **{f"hit_{k}": int(any(named[:k])) for k in KS},
        "false_positives": sum(1 for n in named if not n),
        "sites": len(task_labels["sites"]),
        "sites_named": len(set(key for n in named for key in n)),
        "rows": [{"file": r["file"], "line": r["line"], "p": r["p"], "names": n,
                  "text": r["text"][:120]} for r, n in zip(top, named)],
    }


def summary(tasks):
    n = len(tasks)
    out = {"tasks": n}
    for k in KS:
        hits = sum(t[f"hit_{k}"] for t in tasks)
        out[f"hit_{k}"] = {"k": hits, "n": n, "share": round(hits / n, 3) if n else None,
                           "ci": wilson(hits, n)}
    fp = [t["false_positives"] for t in tasks]
    out["false_positives_per_task"] = round(sum(fp) / n, 3) if n else None
    sites = sum(t["sites"] for t in tasks)
    named = sum(t["sites_named"] for t in tasks)
    out["site_recall_8"] = {"k": named, "n": sites,
                            "share": round(named / sites, 3) if sites else None,
                            "ci": wilson(named, sites)}
    out["candidates"] = sum(t["candidates"] for t in tasks)
    out["tasks_with_candidates"] = sum(1 for t in tasks if t["candidates"])
    out["usd"] = round(sum(t["usd"] for t in tasks), 6)
    out["usd_per_task"] = round(out["usd"] / n, 7) if n else None
    return out


def bootstrap(a, b, metric):
    """95% percentile interval of mean(b) - mean(a), resampling tasks."""
    rng = random.Random(SEED)
    n = len(a)
    diffs = []
    for _ in range(RESAMPLES):
        idx = [rng.randrange(n) for _ in range(n)]
        diffs.append(sum(metric(b[i]) - metric(a[i]) for i in idx) / n)
    diffs.sort()
    point = sum(metric(b[i]) - metric(a[i]) for i in range(n)) / n
    return {"point": round(point, 4),
            "ci": [round(diffs[int(0.025 * RESAMPLES)], 4),
                   round(diffs[int(0.975 * RESAMPLES) - 1], 4)]}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("export")
    parser.add_argument("--workspaces", default=os.path.expanduser(
        "~/.openagents/coder-one/lexicon-free-suspects/workspaces"))
    parser.add_argument("--out", default=os.path.join(HERE, "records/results.json"))
    args = parser.parse_args()
    export = json.load(open(args.export))
    labels = json.load(open(os.path.join(HERE, "labels.json")))["tasks"]
    by = {}
    for f in export["fixtures"]:
        task, arm = f["fixture"].rsplit("--", 1)
        out = f.get("output") or {}
        by.setdefault(task, {})[arm] = {
            "rows": out.get("rows", []),
            # What the answers cost when they were asked live.
            "usd": f.get("recorded_live_cost_usd") or f.get("cost_usd") or 0.0,
        }
    result = {"schema": "openagents.lexicon-free-suspects-results.v1",
              "implementation": export["implementation"], "jev_mode": export["jev_mode"],
              "threshold": THRESHOLD, "max_listed": MAX_LISTED, "tasks": {}}
    for task, arms in sorted(by.items()):
        lab = labels[task]
        texts = {}
        for path in lab["spans"]:
            try:
                with open(os.path.join(args.workspaces, task, path), errors="replace") as f:
                    texts[path] = f.read().split("\n")
            except OSError:
                texts[path] = []
        result["tasks"][task] = {
            "in_sample": lab["in_sample"], "scored": lab["scored"],
            "functions": lab["functions"], "changed_functions": lab["changed_functions"],
            **{arm: dict(per_task(arms[arm]["rows"], lab, texts), usd=arms[arm]["usd"])
               for arm in ARMS},
        }
    groups = {
        "off_sample_scored": [t for t, v in result["tasks"].items()
                              if v["scored"] and not v["in_sample"]],
        "off_sample_all": [t for t, v in result["tasks"].items()
                           if not v["in_sample"] and labels[t]["sites"]],
        "in_sample": [t for t, v in result["tasks"].items() if v["in_sample"]],
    }
    result["groups"] = {}
    for name, tasks in groups.items():
        g = {"tasks": tasks}
        for arm in ARMS:
            g[arm] = summary([result["tasks"][t][arm] for t in tasks])
        a = [result["tasks"][t]["keywords"] for t in tasks]
        b = [result["tasks"][t]["lexicon-free"] for t in tasks]
        if len(tasks) > 1:
            g["difference"] = {
                **{f"hit_{k}": bootstrap(a, b, lambda t, k=k: t[f"hit_{k}"]) for k in KS},
                "false_positives_per_task": bootstrap(a, b, lambda t: t["false_positives"]),
                "usd_per_task": bootstrap(a, b, lambda t: t["usd"]),
            }
        result["groups"][name] = g
    ks = result["groups"]["off_sample_scored"]
    kw, lf = ks["keywords"], ks["lexicon-free"]
    result["at_least_as_good"] = (
        lf["hit_3"]["k"] >= kw["hit_3"]["k"] and lf["hit_8"]["k"] >= kw["hit_8"]["k"]
        and lf["false_positives_per_task"] <= kw["false_positives_per_task"] + 1)
    result["jev_usd_total"] = round(sum(
        v[arm]["usd"] for v in result["tasks"].values() for arm in ARMS), 6)
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    with open(args.out, "w") as f:
        json.dump(result, f, indent=2)
        f.write("\n")
    for name, g in result["groups"].items():
        print(f"== {name} ({len(g['tasks'])} tasks)")
        for arm in ARMS:
            s = g[arm]
            print(f"  {arm:13} hit@3 {s['hit_3']['k']}/{s['hit_3']['n']} {s['hit_3']['ci']}  "
                  f"hit@8 {s['hit_8']['k']}/{s['hit_8']['n']} {s['hit_8']['ci']}  "
                  f"fp/task {s['false_positives_per_task']}  "
                  f"recall@8 {s['site_recall_8']['k']}/{s['site_recall_8']['n']}  "
                  f"candidates {s['candidates']} on {s['tasks_with_candidates']} tasks  "
                  f"usd {s['usd']}")
        if "difference" in g:
            print("  difference", json.dumps(g["difference"]))
    print("at least as good:", result["at_least_as_good"], " jev usd:", result["jev_usd_total"])


if __name__ == "__main__":
    main()
