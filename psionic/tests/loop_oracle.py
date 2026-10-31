#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
"""Chapter 187's independent reference, written from the chapter's specification.

It shares no code with psionic/lib/loop.coax. The world (loop-sim-v1), the
cycle, the budget shares, the paired statistics and the ablation ledger are
written again here, and the fixed-set digest uses hashlib over chapter 178's
values encoding. Without arguments it rebuilds the example's whole output and
compares it with psionic/fixtures/loop.out; `--tests` prints the values the
Coax checks pin.
"""
import hashlib
import math
import pathlib
import struct
import sys

MASK = (1 << 64) - 1
PROPOSE, REWRITE, GRADE, ANSWER, NUDGE = 2.0, 1.0, 1.0, 1.0, 0.1
CAND_STRIDE, ITEM_STRIDE, ATTEMPTS = 8, 32, 2
SEEDS, CYCLES = 8, 8
NAMES = ("proposer", "reflector", "grader", "student")
VERDICTS = ("improving", "flat", "worse")
KEEP = ("earns its place", "within noise, remove it", "harms, remove it")


def uniform(seed, counter):
    z = (seed + 0x9E3779B97F4A7C15 * (counter + 1)) & MASK
    z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & MASK
    z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & MASK
    return ((z ^ (z >> 31)) >> 11) / 9007199254740992.0


def draw(seed, stream, index):
    return uniform(seed, index * 4 + stream)


def tasks(seed, n):
    return [4 * uniform(seed, i) - 2 for i in range(n)]


def chance(skill, d):
    z = skill - d
    return 0.5 + z / (2 * (1 + abs(z)))


def solves(u, p):
    return int(u + p) > 0


def set_digest(values):
    tag = b"psionic.suite.values.v1"
    body = struct.pack("<Q", len(tag)) + tag + struct.pack("<Q", len(values))
    body += b"".join(struct.pack("<d", v) for v in values)
    return hashlib.sha256(body).hexdigest()


def measure(seed, fixed, skill):
    attempts = accepted = 0
    for i, d in enumerate(fixed):
        p = chance(skill, d)
        for a in range(ATTEMPTS):
            attempts += 1
            if solves(draw(seed, 0, i * ATTEMPTS + a), p):
                accepted += 1
                break
    return attempts, accepted, ANSWER * attempts / accepted if accepted else 1e300


def run(seed, fixed, grading, world, loop, budget):
    """Returns (status, stopped, last cycle, spent, per-cycle list)."""
    low, width = world
    prop, refl, grad, cands, items, threshold, cycles = loop
    spent = [0.0, 0.0, 0.0, 0.0]
    skill, out = 0.0, []
    for c in range(cycles + 1):
        need = [0.0, 0.0, 0.0, ANSWER * ATTEMPTS * len(fixed)]
        if c > 0 and prop:
            need[0] = PROPOSE * cands
            need[1] = REWRITE * cands if refl else 0.0
            need[2] = 2 * GRADE * cands * items if grad else 0.0
        for k in range(4):
            if spent[k] + need[k] > budget[k]:
                return "budget", k, c - 1, spent, out
        took = -1
        if c > 0 and prop:
            effects = []
            for j in range(cands):
                at = c * CAND_STRIDE + j
                e = low + width * draw(seed, 1, at)
                spent[0] += PROPOSE
                if refl:
                    e += NUDGE * (draw(seed, 2, at) - 0.5)
                    spent[1] += REWRITE
                effects.append(e)
            if not grad:
                took = 0
            else:
                best = -1
                for j in range(cands):
                    gain = 0
                    for k in range(items):
                        u = draw(seed, 3, (c * CAND_STRIDE + j) * ITEM_STRIDE + k)
                        gain += solves(u, chance(skill + effects[j], grading[k])) - solves(u, chance(skill, grading[k]))
                        spent[2] += 2 * GRADE
                    if gain > best:
                        best, took = gain, j
                if best < threshold:
                    took = -1
            if took >= 0:
                skill += effects[took]
        attempts, accepted, cost = measure(seed, fixed, skill)
        spent[3] += ANSWER * attempts
        out.append((took, cost, skill, attempts, accepted))
    return "ok", 0, cycles, spent, out


def paired(a, b):
    n = len(a)
    total = 0.0
    for x, y in zip(a, b):
        total += x - y
    mean = total / n
    squares = 0.0
    for x, y in zip(a, b):
        d = x - y - mean
        squares += d * d
    return mean, math.sqrt(squares / (n - 1) / n)


def verdict(mean, se):
    if mean > 2 * se:
        return 0
    if mean < -2 * se:
        return 1 + 1
    return 1


def r4(v):
    return math.floor(v * 10000 + 0.5) / 10000


def r1(v):
    return math.floor(v * 10 + 0.5) / 10


def replicates(fixed, grading, world, loop, budget):
    runs = [run(s, fixed, grading, world, loop, budget) for s in range(1, SEEDS + 1)]
    assert all(r[0] == "ok" for r in runs)
    return runs


def example():
    fixed, grading = tasks(187, 64), tasks(1187, 24)
    rich, barren = (-0.6, 1.0), (-0.6, 0.6)
    full = (True, True, True, 4, 24, 2, CYCLES)
    budget = (64.0, 32.0, 1536.0, 1152.0)
    runs = replicates(fixed, grading, rich, full, budget)
    lines = [f"fixed set: 64 tasks, digest {set_digest(fixed)[:8]}; grading: 24 items; {CYCLES} cycles, {SEEDS} replicate seeds"]
    lines.append("replicate 1, cost per accepted result by cycle: " + " ".join(repr(r4(c[1])) for c in runs[0][4]))
    lines.append("  adopted: " + " ".join("-" if t < 0 else f"#{t}" for t, *_ in runs[0][4][1:]))
    means = []
    for c in range(CYCLES + 1):
        t = 0.0
        for r in runs:
            t += r[4][c][1]
        means.append(repr(r4(t / SEEDS)))
    lines.append(f"mean over {SEEDS} replicates by cycle: " + " ".join(means))
    m, se = paired([r[4][0][1] for r in runs], [r[4][-1][1] for r in runs])
    lines.append(f"is the loop improving? first less last {r4(m)!r} per accepted result, standard error {r4(se)!r} -> {VERDICTS[verdict(m, se)]}")
    student = 0.0
    for r in runs:
        student += r[3][3]
    sp = runs[0][3]
    loop = sp[0] + sp[1] + sp[2]
    lines.append(f"spent per run: proposer {r1(sp[0])!r} of {budget[0]!r}, reflector {r1(sp[1])!r} of {budget[1]!r}, "
                 f"grader {r1(sp[2])!r} of {budget[2]!r}, student {r1(student / SEEDS)!r} of {budget[3]!r} (mean)")
    lines.append(f"break-even: the loop's {loop!r} units are paid back after {math.ceil(loop / m)} accepted results at the lower cost")
    lines.append("ablations, same seeds:")
    settings = (1, 1, 2)
    kept = total = 0
    dropped = 0.0
    for k in range(3):
        flags = [True, True, True]
        flags[k] = False
        ablated = replicates(fixed, grading, rich, (*flags, 4, 24, 2, CYCLES), budget)
        gm, gse = paired([a[4][-1][1] for a in ablated], [r[4][-1][1] for r in runs])
        spend = 0.0
        for r in runs:
            spend += r[3][k]
        spend /= SEEDS
        v = verdict(gm, gse)
        s = "" if settings[k] == 1 else "s"
        lines.append(f"  {NAMES[k]}: {settings[k]} setting{s}, spends {r1(spend)!r} a run; removing it costs "
                     f"{r4(gm)!r} +- {r4(gse)!r} per accepted result -> {KEEP[v]}")
        total += settings[k]
        if v == 0:
            kept += settings[k]
        else:
            dropped += spend
    lines.append(f"complexity ledger: {kept} of {total} settings earn their place; {dropped!r} of {loop!r} units a run go to components that do not")
    bruns = replicates(fixed, grading, barren, full, budget)
    bm, bse = paired([r[4][0][1] for r in bruns], [r[4][-1][1] for r in bruns])
    adopted = sum(1 for r in bruns for t, *_ in r[4] if t >= 0)
    lines.append(f"barren world, proposals in [-0.6, 0): first less last {r4(bm)!r} +- {r4(bse)!r}, {adopted} adopted -> "
                 f"{VERDICTS[verdict(bm, bse)]}; {loop!r} units a run bought nothing")
    tight = (64.0, 32.0, 1500.0, 1152.0)
    t = run(1, fixed, grading, rich, full, tight)
    lines.append(f"grader's share cut to {tight[2]!r}: {t[0]} stop before cycle {t[2] + 1}, the {NAMES[t[1]]} share short")
    moved = set_digest(tasks(188, 64)) != set_digest(fixed)
    lines.append("comparing against runs on another fixed set: " + ("moved" if moved else "ok"))
    return "\n".join(lines) + "\n"


def tests():
    fixed, grading = tasks(187, 64), tasks(1187, 24)
    print("fixed[0..3]", [repr(x) for x in fixed[:3]], "digest", set_digest(fixed))
    print("grading[0]", repr(grading[0]))
    print("chance", repr(chance(0.5, -0.25)), repr(chance(0.0, 1.0)))
    for skill in (0.0, 1.0):
        print("measure seed 1 skill", skill, measure(1, fixed, skill))
    small = fixed[:4]
    print("measure small", measure(3, small, 0.0))
    full = (True, True, True, 4, 24, 2, 3)
    for flags in ((True, True, True), (True, False, True), (True, True, False), (False, True, True)):
        r = run(2, fixed[:8], grading, (-0.6, 1.0), (*flags, 4, 24, 2, 3), (1e9, 1e9, 1e9, 1e9))
        print("run", flags, r[0], r[2], [repr(x) for x in r[3]], [(t, repr(c), repr(k), a, n) for t, c, k, a, n in r[4]])
    for b in ((23.0, 1e9, 1e9, 1e9), (1e9, 11.0, 1e9, 1e9), (1e9, 1e9, 575.0, 1e9), (1e9, 1e9, 1e9, 49.0), (1e9, 1e9, 1e9, 15.0)):
        r = run(2, fixed[:8], grading, (-0.6, 1.0), full, b)
        print("budget", b, r[0], NAMES[r[1]], r[2], [repr(x) for x in r[3]])
    a = [run(sd, fixed[:8], grading, (-0.6, 1.0), full, (1e9,) * 4) for sd in (2, 3, 4)]
    b = [run(sd, fixed[:8], grading, (-0.6, 1.0), (True, True, False, 4, 24, 2, 3), (1e9,) * 4) for sd in (2, 3, 4)]
    print("full lasts", [repr(r[4][-1][1]) for r in a], "firsts", [repr(r[4][0][1]) for r in a])
    print("improvement", paired([r[4][0][1] for r in a], [r[4][-1][1] for r in a]))
    print("grader entry", paired([r[4][-1][1] for r in b], [r[4][-1][1] for r in a]), "spend", sum(r[3][2] for r in a) / 3)
    print("paired", paired([3.0, 5.0, 4.0], [1.0, 2.0, 2.0]))


if __name__ == "__main__":
    if "--tests" in sys.argv:
        tests()
    else:
        root = pathlib.Path(__file__).resolve().parents[2]
        want = (root / "psionic/fixtures/loop.out").read_text()
        got = example()
        if got != want:
            for a, b in zip(got.splitlines(), want.splitlines()):
                if a != b:
                    print("oracle :", a)
                    print("fixture:", b)
            sys.exit(1)
        print("loop oracle: the example's output is rebuilt line for line")
