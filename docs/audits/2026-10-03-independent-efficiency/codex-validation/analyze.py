#!/usr/bin/env python3
"""Analysis for the frozen #10356 validation panel (protocol.md).

Usage: analyze.py ROWS.jsonl > results.json
Prints a Markdown summary to stderr.
"""
import json
import math
import random
import statistics
import sys
from collections import defaultdict

ARMS = ["raw-codex", "routed-codex", "raw-claude", "routed-lean"]
PAIRS = [("routed-codex", "raw-codex"), ("routed-lean", "raw-claude"),
         ("raw-codex", "raw-claude"), ("routed-codex", "raw-claude"), ("routed-lean", "raw-codex")]


def wilson(k, n, z=1.96):
    if n == 0:
        return (None, None)
    p = k / n
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return (round(c - h, 3), round(c + h, 3))


def ratio_ci(rows, a, b, key, n=2000):
    by = defaultdict(dict)
    for r in rows:
        by[(r["task"], r["trial"])][r["arm"]] = r
    keys = [k for k, v in by.items() if a in v and b in v and v[a].get(key) is not None and v[b].get(key) is not None]
    if not keys:
        return None
    def ratio(ks):
        num = sum(by[k][a][key] for k in ks)
        den = sum(by[k][b][key] for k in ks)
        return num / den if den else None
    point = ratio(keys)
    rng = random.Random(10356)
    boots = sorted(x for x in (ratio([rng.choice(keys) for _ in keys]) for _ in range(n)) if x is not None)
    return dict(ratio=round(point, 3), lo=round(boots[int(0.025 * len(boots))], 3),
                hi=round(boots[int(0.975 * len(boots)) - 1], 3), pairs=len(keys))


def main():
    rows = [json.loads(l) for l in open(sys.argv[1]) if l.strip()]
    out = dict(runs=len(rows), arms={}, ratios={}, per_task={})
    for arm in ARMS:
        rs = [r for r in rows if r["arm"] == arm]
        k = sum(1 for r in rs if r.get("passed"))
        cost = sum(r.get("cost_usd") or 0 for r in rs)
        walls = [r["wall_s"] for r in rs if r.get("wall_s") is not None]
        outside = [r["wall_s"] - r["route_wall_s"] for r in rs if r.get("route_wall_s") is not None]
        out["arms"][arm] = dict(
            n=len(rs), passed=k, pass_ci=wilson(k, len(rs)), total_cost=round(cost, 4),
            cost_per_checked=round(cost / k, 4) if k else None,
            median_wall_s=round(statistics.median(walls), 1) if walls else None,
            median_wall_passed_s=round(statistics.median([r["wall_s"] for r in rs if r.get("passed")]), 1) if k else None,
            median_outside_engine_s=round(statistics.median(outside), 1) if outside else None,
            jev_usd=round(sum(r.get("jev_usd") or 0 for r in rs), 4),
            failures=[f'{r["task"]}#{r["trial"]}' for r in rs if not r.get("passed")])
    for a, b in PAIRS:
        out["ratios"][f"{a}/{b}"] = dict(cost=ratio_ci(rows, a, b, "cost_usd"), wall=ratio_ci(rows, a, b, "wall_s"))
    for t in sorted({r["task"] for r in rows}):
        out["per_task"][t] = {arm: dict(
            passed=sum(1 for r in rows if r["task"] == t and r["arm"] == arm and r.get("passed")),
            n=sum(1 for r in rows if r["task"] == t and r["arm"] == arm),
            median_wall_s=round(statistics.median([r["wall_s"] for r in rows if r["task"] == t and r["arm"] == arm] or [0]), 1),
            cost=round(sum(r.get("cost_usd") or 0 for r in rows if r["task"] == t and r["arm"] == arm), 4))
            for arm in ARMS}
    json.dump(out, sys.stdout, indent=1)
    w = sys.stderr.write
    w("| Arm | Passed (Wilson 95%) | Total cost | Cost per checked result | Median wall | Median outside engine |\n|---|---|---|---|---|---|\n")
    for arm, a in out["arms"].items():
        w(f'| {arm} | {a["passed"]}/{a["n"]} ({a["pass_ci"][0]:.0%}–{a["pass_ci"][1]:.0%}) | ${a["total_cost"]:.2f} | '
          f'${a["cost_per_checked"] or 0:.3f} | {a["median_wall_s"]} s | {a["median_outside_engine_s"] or "–"} s |\n')
    w("\n| Ratio | Cost (95% CI) | Wall time (95% CI) |\n|---|---|---|\n")
    for k, v in out["ratios"].items():
        c, t = v["cost"], v["wall"]
        f = lambda x: f'{x["ratio"]} ({x["lo"]}–{x["hi"]})' if x else "–"
        w(f"| {k} | {f(c)} | {f(t)} |\n")


if __name__ == "__main__":
    main()
