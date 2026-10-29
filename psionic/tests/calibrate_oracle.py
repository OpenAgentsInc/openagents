#!/usr/bin/env python3
"""Chapter 156's independent reference, written from the chapter's specification.

It shares no code with psionic/lib/calibrate.coax. Sums use math.fsum
(exactly rounded) where the library uses chapter 3's compensated accumulator.
The temperature is found by a different search: a binary search over the
integer bit patterns of doubles in [0.05, 20] for the first slope whose log
loss derivative is positive. Isotonic steps use a stack of exact fractions.
The simulator, the splits and the experiment's seeds are part of the
specification. `--header` writes the C expectations the native harness
compares.
"""
import math
import os
import struct
import sys
from fractions import Fraction

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tabular_oracle import uniform  # noqa: E402

FLOOR = 1e-6
N = 4000
QUESTIONS = 2
SET = 1
KINDS = ("raw", "temperature", "binned", "isotonic")
MODELS = ((1, 156, (2, 0, 0.6, -0.6)), (2, 256, (1, 0.9, 1.5, 0)))


def sigmoid(z):
    # The library's form, exp(min(z, 0)) / (1 + exp(-|z|)), is part of the specification.
    return math.exp((z - abs(z)) / 2) / (1 + math.exp(-abs(z)))


def clamp(p):
    return min(max(p, FLOOR), 1 - FLOOR)


def logit(p):
    q = clamp(p)
    return math.log(q / (1 - q))


def simulate(seed, model, n):
    q, p, y = [], [], []
    for i in range(n):
        k = i % 2
        z = 6 * uniform(seed, 2 * i) - 3
        q.append(k)
        y.append(float(math.floor(uniform(seed, 2 * i + 1) + sigmoid(z))))
        p.append(sigmoid(model[2 * k] * z + model[2 * k + 1]))
    return q, p, y


def bits(x):
    return struct.unpack("<q", struct.pack("<d", x))[0]


def double(b):
    return struct.unpack("<d", struct.pack("<q", b))[0]


def slope(p, y):
    z = [logit(v) for v in p]

    def rising(a):
        return math.fsum((sigmoid(a * zi) - yi) * zi for zi, yi in zip(z, y)) > 0

    lo, hi = bits(0.05), bits(20.0)
    while hi - lo > 1:
        mid = (lo + hi) // 2
        if rising(double(mid)):
            hi = mid
        else:
            lo = mid
    return double(hi)


def binned(p, y):
    n = len(p)
    groups = {}
    for i in range(n):
        groups.setdefault(i * 10 // n, []).append(i)
    knots = [p[g[0]] for g in groups.values()]
    values = [clamp(sum(y[i] for i in g) / len(g)) for g in groups.values()]
    return knots, values


def isotonic(p, y):
    stack = []  # [knot, yes, count]
    for v, t in zip(p, y):
        stack.append([v, int(t), 1])
        while len(stack) > 1 and Fraction(stack[-2][1], stack[-2][2]) >= Fraction(stack[-1][1], stack[-1][2]):
            top = stack.pop()
            stack[-1][1] += top[1]
            stack[-1][2] += top[2]
    return [s[0] for s in stack], [clamp(s[1] / s[2]) for s in stack]


def fit(kind, p, y):
    order = sorted(range(len(p)), key=lambda i: p[i])
    sp, sy = [p[i] for i in order], [y[i] for i in order]
    if kind == 1:
        return dict(kind=1, slope=slope(p, y), knots=[], values=[])
    knots, values = (binned if kind == 2 else isotonic)(sp, sy)
    return dict(kind=kind, slope=1.0, knots=knots, values=values)


def apply(m, v):
    if m["kind"] == 0:
        return v
    if m["kind"] == 1:
        return clamp(sigmoid(m["slope"] * logit(v)))
    below = [j for j, k in enumerate(m["knots"]) if k <= v]
    return m["values"][below[-1] if below else 0]


def measure(p, y):
    conf, hit = [[] for _ in range(15)], [0] * 15
    errors = 0
    for v, t in zip(p, y):
        top = max(v, 1 - v)
        right = (v >= 0.5) == (t == 1)
        b = max(b for b in range(15) if b == 0 or top > b / 15)
        conf[b].append(top)
        hit[b] += right
        errors += top >= 0.9 and not right
    n = len(p)
    ece = math.fsum(abs(math.fsum(c) - h) for c, h in zip(conf, hit)) / n
    brier = math.fsum(2 * (v - t) ** 2 for v, t in zip(p, y)) / n
    loss = math.fsum(-math.log(clamp(v) if t == 1 else 1 - clamp(v)) for v, t in zip(p, y)) / n
    return dict(ece=ece, brier=brier, loss=loss, errors=errors)


def reliability(p, y):
    out = []
    for b in range(10):
        rows = [(v, t) for v, t in zip(p, y) if min(int(v * 10), 9) == b]
        if len(rows) < 20:
            out.append((len(rows), -1.0, -1.0))
        else:
            out.append((len(rows), math.fsum(v for v, _ in rows) / len(rows), sum(t for _, t in rows) / len(rows)))
    return out


def question(split, k):
    q, p, y = split
    rows = [i for i in range(len(q)) if q[i] == k]
    return [p[i] for i in rows], [y[i] for i in rows]


def experiment():
    out = []
    for model, seed, params in MODELS:
        fits, gates, tests = (simulate(seed + s, params, N) for s in range(3))
        r = dict(model=model, questions=[])
        for k in range(QUESTIONS):
            fp, fy = question(fits, k)
            gp, gy = question(gates, k)
            tp, ty = question(tests, k)
            raw = measure(gp, gy)
            chosen, best, kinds = dict(kind=0), raw["loss"], []
            for kind in (1, 2, 3):
                m = fit(kind, fp, fy)
                g = measure([apply(m, v) for v in gp], gy)
                passes = g["ece"] < raw["ece"] and g["loss"] < raw["loss"]
                kinds.append(dict(map=m, gate=g, passes=passes))
                if passes and g["loss"] < best:
                    chosen, best = m, g["loss"]
            mapped = [apply(chosen, v) for v in tp]
            r["questions"].append(dict(raw_gate=raw, kinds=kinds, chosen=chosen["kind"],
                                       raw=measure(tp, ty), mapped=measure(mapped, ty),
                                       reliability=(reliability(tp, ty), reliability(mapped, ty))))
        out.append(r)
    return out


def show(result):
    for r in result:
        print(f"model {r['model']}")
        for k, x in enumerate(r["questions"]):
            print(f"  question {k}: gate raw ECE {x['raw_gate']['ece']:.4f}, chosen {KINDS[x['chosen']]}")
            for kind, c in zip(KINDS[1:], x["kinds"]):
                print(f"    {kind}: ECE {c['gate']['ece']:.4f}, log loss {c['gate']['loss']:.4f}, "
                      f"{'passes' if c['passes'] else 'fails'}, slope {c['map']['slope']!r}, knots {len(c['map']['knots'])}")
            print(f"    test: ECE {x['raw']['ece']:.4f} -> {x['mapped']['ece']:.4f}, "
                  f"confident errors {x['raw']['errors']} -> {x['mapped']['errors']}")


def report(r):
    return f"{{{r['ece']!r},{r['brier']!r},{r['loss']!r},{r['errors']}}}"


def header(result):
    lines = ["/* Generated by psionic/tests/calibrate_oracle.py --header. Do not edit. */", "#include <stdint.h>",
             "typedef struct {double ece,brier,loss;int64_t errors;} cf_oracle_report;",
             "typedef struct {double slope;int64_t knots;double first,last;} cf_oracle_map;",
             "typedef struct {cf_oracle_report raw_gate;cf_oracle_report gate[3];int64_t passes[3];cf_oracle_map map[3];"
             "int64_t chosen;cf_oracle_report raw,mapped;int64_t count[2][10];double mean[2][10],rate[2][10];} cf_oracle_question;",
             "static const cf_oracle_question cf_oracle[2][2] = {"]
    for r in result:
        qs = []
        for x in r["questions"]:
            maps = []
            for c in x["kinds"]:
                m = c["map"]
                first, last = (m["values"][0], m["values"][-1]) if m["knots"] else (0.0, 0.0)
                maps.append(f"{{{m['slope']!r},{len(m['knots'])},{first!r},{last!r}}}")
            rel = x["reliability"]
            count = ",".join("{" + ",".join(str(b[0]) for b in t) + "}" for t in rel)
            mean = ",".join("{" + ",".join(repr(b[1]) for b in t) + "}" for t in rel)
            rate = ",".join("{" + ",".join(repr(b[2]) for b in t) + "}" for t in rel)
            qs.append(f"{{{report(x['raw_gate'])},{{{','.join(report(c['gate']) for c in x['kinds'])}}},"
                      f"{{{','.join(str(int(c['passes'])) for c in x['kinds'])}}},{{{','.join(maps)}}},{x['chosen']},"
                      f"{report(x['raw'])},{report(x['mapped'])},{{{count}}},{{{mean}}},{{{rate}}}}}")
        lines.append("  {" + ",\n   ".join(qs) + "},")
    lines.append("};")
    print("\n".join(lines))


if __name__ == "__main__":
    result = experiment()
    if "--header" in sys.argv:
        header(result)
    else:
        show(result)
