#!/usr/bin/env python3
"""Summarize the #10209 study: per arm and per task, with n and intervals."""
import json
import math
import random
import statistics
import sys
from collections import defaultdict

rows = [json.loads(l) for l in open(sys.argv[1]) if l.strip()]
ARMS = ["raw-claude", "routed-claude-on", "routed-claude-off", "routed-codex-on", "routed-codex-off", "routed-claude-lean"]
TASKS = ["fix-git", "fix-code-vulnerability", "headless-terminal", "build-cython-ext", "mi-seekable", "mi-one", "bottle-etag"]


def wilson(k, n, z=1.96):
    if n == 0:
        return (float("nan"),) * 2
    p = k / n
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return c - h, c + h


def cost(r):
    return r.get("cost_usd")


by = defaultdict(list)
for r in rows:
    by[(r["arm"], r["task"])].append(r)

print("## Per arm\n")
print("| Arm | n | Passed (Wilson 95%) | Total cost | Median cost/run | Total wall | Median wall/run | Median input tokens | Unpriced runs |")
print("|---|---:|---|---:|---:|---:|---:|---:|---:|")
summary = {}
for a in ARMS:
    rs = [r for r in rows if r["arm"] == a]
    if not rs:
        continue
    k = sum(1 for r in rs if r.get("passed"))
    lo, hi = wilson(k, len(rs))
    costs = [cost(r) for r in rs if cost(r) is not None]
    walls = [r["wall_s"] for r in rs]
    toks = [r.get("input_tokens") for r in rs if r.get("input_tokens") is not None]
    summary[a] = dict(n=len(rs), k=k, cost=sum(costs), wall=sum(walls))
    print(f"| `{a}` | {len(rs)} | {k}/{len(rs)} ({lo:.0%}–{hi:.0%}) | ${sum(costs):.2f} | ${statistics.median(costs):.3f} | "
          f"{sum(walls)/60:.1f} min | {statistics.median(walls):.0f} s | {statistics.median(toks):,.0f} | {len(rs)-len(costs)} |")


def boot_ratio(arm, base, key, iters=4000, seed=10209):
    """Ratio of arm's total to base's total over the tasks both ran, with a
    bootstrap interval resampling trials within each task."""
    rnd = random.Random(seed)
    tasks = [t for t in TASKS if by[(arm, t)] and by[(base, t)]]
    def tot(sample):
        return sum(sample)
    def val(r):
        return r.get(key) if key != "wall_s" else r["wall_s"]
    pa = {t: [val(r) for r in by[(arm, t)] if val(r) is not None] for t in tasks}
    pb = {t: [val(r) for r in by[(base, t)] if val(r) is not None] for t in tasks}
    # Compare per-task means so unequal n per task does not tilt the total.
    point = sum(statistics.mean(pa[t]) for t in tasks) / sum(statistics.mean(pb[t]) for t in tasks)
    ratios = []
    for _ in range(iters):
        a = sum(statistics.mean([rnd.choice(pa[t]) for _ in pa[t]]) for t in tasks)
        b = sum(statistics.mean([rnd.choice(pb[t]) for _ in pb[t]]) for t in tasks)
        ratios.append(a / b)
    ratios.sort()
    return point, ratios[int(0.025 * iters)], ratios[int(0.975 * iters)], len(tasks)


print("\n## Against raw Claude Code (sum over tasks of per-task means; 95% bootstrap over trials within task)\n")
print("| Arm | Tasks | Cost ratio (CI) | Cost saving | Wall ratio (CI) | Time saving |")
print("|---|---:|---|---:|---|---:|")
for a in ARMS[1:]:
    if not any(by[(a, t)] for t in TASKS):
        continue
    c, clo, chi, n = boot_ratio(a, "raw-claude", "cost_usd")
    w, wlo, whi, _ = boot_ratio(a, "raw-claude", "wall_s")
    print(f"| `{a}` | {n} | {c:.2f} ({clo:.2f}–{chi:.2f}) | {1-c:+.0%} | {w:.2f} ({wlo:.2f}–{whi:.2f}) | {1-w:+.0%} |")

print("\n## Recipe on against off, same engine\n")
print("| Engine | Tasks | Cost ratio on/off (CI) | Wall ratio on/off (CI) |")
print("|---|---:|---|---|")
for e in ["claude", "codex"]:
    on, off = f"routed-{e}-on", f"routed-{e}-off"
    if any(by[(on, t)] for t in TASKS) and any(by[(off, t)] for t in TASKS):
        c, clo, chi, n = boot_ratio(on, off, "cost_usd")
        w, wlo, whi, _ = boot_ratio(on, off, "wall_s")
        print(f"| {e} | {n} | {c:.2f} ({clo:.2f}–{chi:.2f}) | {w:.2f} ({wlo:.2f}–{whi:.2f}) |")

if any(by[("routed-claude-lean", t)] for t in TASKS):
    print("\n## Lean session against the routed Claude loop (#10246)\n")
    print("| Base | Tasks | Cost ratio lean/base (CI) | Wall ratio lean/base (CI) |")
    print("|---|---:|---|---|")
    for base in ["routed-claude-on", "routed-claude-off"]:
        c, clo, chi, n = boot_ratio("routed-claude-lean", base, "cost_usd")
        w, wlo, whi, _ = boot_ratio("routed-claude-lean", base, "wall_s")
        print(f"| `{base}` | {n} | {c:.2f} ({clo:.2f}–{chi:.2f}) | {w:.2f} ({wlo:.2f}–{whi:.2f}) |")

print("\n## Per task (passes/n · median cost · median wall)\n")
print("| Task | " + " | ".join(f"`{a}`" for a in ARMS) + " |")
print("|---|" + "---|" * len(ARMS))
for t in TASKS:
    cells = []
    for a in ARMS:
        rs = by[(a, t)]
        if not rs:
            cells.append("–")
            continue
        k = sum(1 for r in rs if r.get("passed"))
        cs = [cost(r) for r in rs if cost(r) is not None]
        mc = f"${statistics.median(cs):.3f}" if cs else "?"
        cells.append(f"{k}/{len(rs)} · {mc} · {statistics.median([r['wall_s'] for r in rs]):.0f} s")
    print(f"| `{t}` | " + " | ".join(cells) + " |")

# Recipe facts on the routed-on arms.
print("\n## Recipe on: what it did\n")
for a in ["routed-claude-on", "routed-codex-on", "routed-claude-lean"]:
    rs = [r for r in rows if r["arm"] == a]
    if not rs:
        continue
    ended = defaultdict(int)
    for r in rs:
        ended[r.get("ending")] += 1
    frozen = sum(1 for r in rs if (r.get("checks_frozen") or 0) > 0)
    kn = sum(1 for r in rs if (r.get("knowledge_kept") or 0) > 0)
    klass = defaultdict(int)
    for r in rs:
        klass[r.get("klass")] += 1
    jev = sum((r.get("jev_usd") or 0) for r in rs)
    tot = sum((r.get("cost_usd") or 0) for r in rs)
    print(f"- `{a}`: endings {dict(ended)}; class {dict(klass)}; runs with checks kept {frozen}/{len(rs)}; "
          f"with knowledge kept {kn}/{len(rs)}; Jev ${jev:.4f} of ${tot:.2f} ({jev/tot:.2%}).")

print("\n## Study spend\n")
print(f"Runs: {len(rows)}; total list-price spend ${sum(cost(r) or 0 for r in rows):.2f}; "
      f"agent wall time {sum(r['wall_s'] for r in rows)/3600:.1f} h.")
