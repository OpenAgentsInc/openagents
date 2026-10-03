#!/usr/bin/env python3
"""The #10250 arms: raw Codex, the routed Codex loop, and the lean Codex
session, with n, Wilson intervals, and bootstrap cost and wall-time ratios
(sum over tasks of per-task means, resampling trials within each task).

Usage: codex_session.py collected.jsonl
"""
import json
import math
import random
import statistics
import sys
from collections import defaultdict

TASKS = ["fix-git", "fix-code-vulnerability", "headless-terminal", "build-cython-ext", "mi-seekable", "mi-one", "bottle-etag"]


def wilson(k, n, z=1.96):
    p = k / n
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return c - h, c + h

ARMS = ["raw-codex", "routed-codex-loop", "routed-codex-session"]


def main(path):
    rows = [json.loads(l) for l in open(path) if l.strip()]
    by = defaultdict(list)
    for r in rows:
        by[(r["arm"], r["task"])].append(r)

    def boot(arm, base, key, iters=4000, seed=10250):
        rnd = random.Random(seed)
        tasks = [t for t in TASKS if by[(arm, t)] and by[(base, t)]]
        pa = {t: [r[key] for r in by[(arm, t)] if r.get(key) is not None] for t in tasks}
        pb = {t: [r[key] for r in by[(base, t)] if r.get(key) is not None] for t in tasks}
        point = sum(statistics.mean(pa[t]) for t in tasks) / sum(statistics.mean(pb[t]) for t in tasks)
        rs = []
        for _ in range(iters):
            a = sum(statistics.mean([rnd.choice(pa[t]) for _ in pa[t]]) for t in tasks)
            b = sum(statistics.mean([rnd.choice(pb[t]) for _ in pb[t]]) for t in tasks)
            rs.append(a / b)
        rs.sort()
        return point, rs[int(0.025 * iters)], rs[int(0.975 * iters)], len(tasks)

    print("\n# #10250: Codex arms\n")
    print("| Arm | n | Passed (Wilson 95%) | Total cost | Median cost/run | Total wall | Median wall/run | Median input tokens | Cache read share |")
    print("|---|---:|---|---:|---:|---:|---:|---:|---:|")
    for a in ARMS:
        rs = [r for r in rows if r["arm"] == a]
        if not rs:
            continue
        k = sum(1 for r in rs if r.get("passed"))
        lo, hi = wilson(k, len(rs))
        costs = [r["cost_usd"] for r in rs if r.get("cost_usd") is not None]
        walls = [r["wall_s"] for r in rs]
        toks = [r.get("input_tokens") or 0 for r in rs]
        read = sum(r.get("cache_read") or 0 for r in rs)
        print(f"| `{a}` | {len(rs)} | {k}/{len(rs)} ({lo:.0%}–{hi:.0%}) | ${sum(costs):.2f} | ${statistics.median(costs):.3f} | "
              f"{sum(walls)/60:.1f} min | {statistics.median(walls):.0f} s | {statistics.median(toks):,.0f} | {read/max(sum(toks),1):.0%} |")

    print("\n| Arm | Against | Tasks | Cost ratio (95% CI) | Wall-time ratio (95% CI) |")
    print("|---|---|---:|---|---|")
    for a, base in [("routed-codex-loop", "raw-codex"), ("routed-codex-session", "raw-codex"),
                    ("routed-codex-session", "routed-codex-loop"), ("routed-codex-session", "raw-claude"),
                    ("routed-codex-session", "routed-claude-lean")]:
        if not any(by[(a, t)] for t in TASKS) or not any(by[(base, t)] for t in TASKS):
            continue
        c, clo, chi, n = boot(a, base, "cost_usd")
        w, wlo, whi, _ = boot(a, base, "wall_s")
        print(f"| `{a}` | `{base}` | {n} | {c:.2f} ({clo:.2f}–{chi:.2f}) | {w:.2f} ({wlo:.2f}–{whi:.2f}) |")

    print("\n| Task | " + " | ".join(f"`{a}`" for a in ARMS) + " |")
    print("|---|" + "---|" * len(ARMS))
    for t in TASKS:
        cells = []
        for a in ARMS:
            rs = by[(a, t)]
            if not rs:
                cells.append("–")
                continue
            k = sum(1 for r in rs if r.get("passed"))
            cs = [r["cost_usd"] for r in rs if r.get("cost_usd") is not None]
            cells.append(f"{k}/{len(rs)} · ${statistics.median(cs):.3f} · {statistics.median([r['wall_s'] for r in rs]):.0f} s")
        print(f"| `{t}` | " + " | ".join(cells) + " |")


if __name__ == "__main__":
    main(sys.argv[1])
