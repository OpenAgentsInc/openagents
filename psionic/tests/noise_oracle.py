#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
"""Chapter 238's independent reference, written from the chapter's specification.

It shares no code with psionic/lib/noise.coax. Sums use math.fsum (exactly
rounded) where the library uses chapter 4's compensated accumulator. The
Elephant probabilities are read from chapter 16's recorded run rather than
copied. `--header` writes the C expectations the native harness compares.
"""
import math
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from tabular_oracle import uniform  # noqa: E402

SEED = 179
TASKS, PER_TASK = 40, 10
ITEMS = TASKS * PER_TASK
SATURATED = 32
SPREAD, SHIFT = 3.0, 0.3
BASELINE_RUNS, BLOCK = 64, 4
PAIRS, PAIR_REPLICATES = 20, 200
REPLICATES = 1000
CANDIDATES = ((0.5, 2000), (1.5, 3000))
Z = 1.96


def sigmoid(z):
    return 1 / (1 + math.exp(-z))


def task_of(i):
    return i // PER_TASK


def part_of(i):
    return int(task_of(i) >= SATURATED)


def difficulty(i):
    t = task_of(i)
    d = 3 * uniform(SEED, t) - 1.5 + uniform(SEED, 1000 + i) - 0.5
    return d - 5 if t >= SATURATED else d


def run(r, skill):
    base = 100000 * (r + 1)
    shift = SHIFT * (2 * uniform(SEED, base + 999) - 1)
    out = []
    for i in range(ITEMS):
        g = shift + SPREAD * (2 * uniform(SEED, base + task_of(i)) - 1)
        out.append(float(math.floor(uniform(SEED, base + 1000 + i) + sigmoid(skill - difficulty(i) + g))))
    return out


def means(runs):
    return [math.fsum(col) / len(runs) for col in zip(*runs)]


def floor_of(runs, part, which, block):
    picked = [i for i in range(len(part)) if part[i] == which]
    blocks = [math.fsum(r[i] for r in runs[k:k + block] for i in picked) / (block * len(picked))
              for k in range(0, len(runs), block)]
    mu = math.fsum(blocks) / len(blocks)
    sd = math.sqrt(math.fsum((m - mu) ** 2 for m in blocks) / (len(blocks) - 1))
    return dict(mean=blocks, sd=sd, floor=Z * math.sqrt(2) * sd)


def bootstrap(a, b, cluster, part, which, floor, seed, replicates):
    sums, counts = {}, {}
    for i in range(len(a)):
        if part[i] == which:
            sums.setdefault(cluster[i], []).append(b[i] - a[i])
            counts[cluster[i]] = counts.get(cluster[i], 0) + 1
    present = sorted(sums)
    total = {t: math.fsum(sums[t]) for t in present}
    p = len(present)
    gain = math.fsum(total.values()) / sum(counts.values())
    reps = []
    for k in range(replicates):
        drawn = [present[math.floor(uniform(seed, k * p + j) * p)] for j in range(p)]
        reps.append(math.fsum(total[t] for t in drawn) / sum(counts[t] for t in drawn))
    mu = math.fsum(reps) / replicates
    se = math.sqrt(math.fsum((x - mu) ** 2 for x in reps) / (replicates - 1))
    return dict(tasks=p, items=sum(counts.values()), gain=gain, se=se, below=sum(x <= floor for x in reps))


def headroom(score, part, which, ceiling, floor):
    picked = [score[i] for i in range(len(score)) if part[i] == which]
    open_ = sum(v < ceiling for v in picked)
    room = 0.0 if open_ == 0 else ceiling - math.fsum(picked) / len(picked)
    return dict(items=len(picked), open=open_, room=room, refused=room <= floor)


def elephant():
    """Chapter 16's twelve held-out items: kind, gold option and probabilities."""
    text = open(os.path.join(HERE, "..", "fixtures", "quality", "heldout-v1-elephant-cpu.txt")).read()
    rows = re.findall(r"^(choice|noul|score)-\S+ .*?gold=(\d+).*\n  p=([0-9.,]+)$", text, re.M)
    kinds = {"choice": 0, "noul": 1, "score": 2}
    items = [(kinds[k], int(g), [float(x) for x in p.split(",")]) for k, g, p in rows]
    assert len(items) == 12
    return items


def first_max(p):
    return max(range(len(p)), key=lambda i: (p[i], -i))


def experiment():
    task = [task_of(i) for i in range(ITEMS)]
    part = [part_of(i) for i in range(ITEMS)]
    each = list(range(ITEMS))
    base = [run(r, 0) for r in range(BASELINE_RUNS)]
    floors = [floor_of(base, part, w, BLOCK) for w in (0, 1)]
    mean = means(base)
    rooms = [headroom(mean, part, w, 1.0, floors[w]["floor"]) for w in (0, 1)]
    pairs = []
    for j in range(PAIRS):
        first = BASELINE_RUNS + 2 * BLOCK * j
        a = means([run(first + q, 0) for q in range(BLOCK)])
        b = means([run(first + BLOCK + q, 0) for q in range(BLOCK)])
        pairs.append((bootstrap(a, b, each, part, 0, 0.0, 10 + j, PAIR_REPLICATES),
                      bootstrap(a, b, task, part, 0, 0.0, 10 + j, PAIR_REPLICATES)))
    a = means(base[:BLOCK])
    candidates = []
    for skill, first in CANDIDATES:
        b = means([run(first + q, skill) for q in range(BLOCK)])
        fl = floors[0]["floor"]
        candidates.append(tuple(bootstrap(a, b, c, part, 0, x, 7, REPLICATES) for c in (each, task) for x in (0.0, fl)))
    items = elephant()
    kind = [k for k, _, _ in items]
    correct = [float(first_max(p) == g) for _, g, p in items]
    brier = [-math.fsum((v - (i == g)) ** 2 for i, v in enumerate(p)) for _, g, p in items]
    every = [0] * 12
    ele = dict(correct=correct, brier=brier,
               accuracy=[headroom(correct, kind, w, 1.0, 0.0) for w in (0, 1, 2)] + [headroom(correct, every, 0, 1.0, 0.0)],
               loss=[headroom(brier, kind, w, 0.0, 0.0) for w in (0, 1, 2)] + [headroom(brier, every, 0, 0.0, 0.0)])
    return dict(base=base, floors=floors, rooms=rooms, pairs=pairs, candidates=candidates, elephant=ele)


def spread(xs):
    mu = math.fsum(xs) / len(xs)
    return math.sqrt(math.fsum((x - mu) ** 2 for x in xs) / (len(xs) - 1))


def show(r):
    for w, f in enumerate(r["floors"]):
        print(f"part {w}: blocks {[round(m, 4) for m in f['mean']]} sd {f['sd']:.4f} floor {f['floor']:.4f}", r["rooms"][w])
    gains = [t["gain"] for _, t in r["pairs"]]
    print(f"A/A gain spread {spread(gains):.4f}; mean item se {math.fsum(i['se'] for i, _ in r['pairs']) / PAIRS:.4f};"
          f" mean task se {math.fsum(t['se'] for _, t in r['pairs']) / PAIRS:.4f}; largest {max(map(abs, gains)):.4f}")
    for (skill, _), c in zip(CANDIDATES, r["candidates"]):
        print(skill, *c)
    for k in ("accuracy", "loss"):
        print(k, r["elephant"][k])


def doubles(xs):
    return ",".join(repr(float(x)) for x in xs)


def boot_line(b):
    return f"{{{b['tasks']},{b['items']},{b['gain']!r},{b['se']!r},{b['below']}}}"


def room_line(m):
    return f"{{{m['items']},{m['open']},{m['room']!r},{int(m['refused'])}}}"


def header(r):
    sums = [math.fsum(x) for x in r["base"]]
    lines = ["/* Generated by psionic/tests/noise_oracle.py --header. Do not edit. */", "#include <stdint.h>",
             "typedef struct {int64_t tasks,items;double gain,se;int64_t below;} ns_oracle_boot;",
             "typedef struct {int64_t items,open;double room;int64_t refused;} ns_oracle_room;",
             f"static const double ns_oracle_run_sums[{BASELINE_RUNS}] = {{{doubles(sums)}}};",
             f"static const double ns_oracle_block_mean[2][16] = {{{','.join('{' + doubles(f['mean']) + '}' for f in r['floors'])}}};",
             f"static const double ns_oracle_floor[2][2] = {{{','.join('{' + doubles([f['sd'], f['floor']]) + '}' for f in r['floors'])}}};",
             f"static const ns_oracle_room ns_oracle_rooms[2] = {{{','.join(room_line(m) for m in r['rooms'])}}};",
             f"static const ns_oracle_boot ns_oracle_pairs[{PAIRS}][2] = {{{','.join('{' + boot_line(i) + ',' + boot_line(t) + '}' for i, t in r['pairs'])}}};",
             f"static const ns_oracle_boot ns_oracle_candidates[2][4] = {{{','.join('{' + ','.join(map(boot_line, c)) + '}' for c in r['candidates'])}}};",
             f"static const double ns_oracle_elephant_brier[12] = {{{doubles(r['elephant']['brier'])}}};",
             f"static const ns_oracle_room ns_oracle_elephant_rooms[2][4] = {{{','.join('{' + ','.join(room_line(m) for m in r['elephant'][k]) + '}' for k in ('accuracy', 'loss'))}}};"]
    print("\n".join(lines))


if __name__ == "__main__":
    result = experiment()
    if "--header" in sys.argv:
        header(result)
    else:
        show(result)
