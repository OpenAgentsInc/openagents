#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
"""Chapter 158's independent reference, written from the chapter's specification.

It shares no code with psionic/lib/drift.coax. Sums use math.fsum (exactly
rounded) where the library uses chapter 3's compensated accumulator. Maps are
fitted with chapter 156's oracle. The run is computed in closed form rather
than by stepping time: every outcome that isn't lost joins at its arrival, and
a refit sees exactly the records decided in its windows that weren't lost,
since all of their outcomes arrive before it. `--header` writes the C
expectations the native harness compares.
"""
import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tabular_oracle import uniform  # noqa: E402
from calibrate_oracle import apply, fit, sigmoid  # noqa: E402

SEED = 158
WINDOW = 1000
DECISIONS = 12000
DRIFT, SHIFT = 4000, 1.5
WATCH_SEED = 160
POLICY = dict(threshold=0.25, explore=0.1, sure=0.1, wrong_yes=1, wrong_no=3)
THRESHOLDS = (0.25, 0.5, 0.05)
FLOOR = 1e-6


def logit(seed, t):
    return 6 * uniform(seed, 5 * t) - 3


def raw(seed, t):
    return sigmoid(2 * logit(seed, t))


def label(seed, t, shift):
    return float(math.floor(uniform(seed, 5 * t + 1) + sigmoid(logit(seed, t) - shift)))


def arrival(seed, t, delay):
    lost = math.floor(uniform(seed, 5 * t + 3) + 0.1)
    return (t + 1 + math.floor(delay * uniform(seed, 5 * t + 2))) * (1 - lost)


def ece_loss(p, y):
    """Chapter 12's calibration error (top label, 15 bins) and clamped log loss."""
    conf, hit = [[] for _ in range(15)], [0] * 15
    for v, t in zip(p, y):
        top = 0.5 + abs(v - 0.5)
        b = 7
        while top > (b + 1) / 15:
            b += 1
        conf[b].append(top)
        hit[b] += (v >= 0.5) == (t == 1)
    ece = math.fsum(abs(math.fsum(c) - h) for c, h in zip(conf, hit)) / len(p)
    clamp = [min(max(v, FLOOR), 1 - FLOOR) for v in p]
    loss = math.fsum(-math.log(c if t == 1 else 1 - c) for c, t in zip(clamp, y)) / len(p)
    return ece, loss


def refit(records, current, fit_range, gate_range):
    fp = [r["raw"] for r in records[fit_range[0]:fit_range[1]] if r["observed"]]
    fy = [r["label"] for r in records[fit_range[0]:fit_range[1]] if r["observed"]]
    gp = [r["raw"] for r in records[gate_range[0]:gate_range[1]] if r["observed"]]
    gy = [r["label"] for r in records[gate_range[0]:gate_range[1]] if r["observed"]]
    now = ece_loss([apply(current, v) for v in gp], gy)
    gate = dict(adopted=False, kind=current["kind"], ece=[now[0], -1.0, -1.0, -1.0], loss=[now[1], -1.0, -1.0, -1.0])
    chosen, best = current, now[1]
    for kind in (1, 2, 3):
        if len(fp) < 200:
            continue
        m = fit(kind, fp, fy)
        if len(m["knots"]) > 256:
            continue
        e, l = ece_loss([apply(m, v) for v in gp], gy)
        gate["ece"][kind], gate["loss"][kind] = e, l
        if e < now[0] and l < now[1] and l < best:
            chosen, best = m, l
            gate.update(adopted=True, kind=kind)
    return chosen, gate


def decide(m, p, u):
    c = apply(m, p)
    greedy = int(c >= POLICY["threshold"])
    if not POLICY["sure"] <= c <= 1 - POLICY["sure"]:
        return c, 1 << greedy, greedy, 1.0
    if u < POLICY["explore"]:
        return c, 3, 1 - greedy, POLICY["explore"]
    return c, 3, greedy, 1 - POLICY["explore"]


def experiment():
    calibration = [dict(raw=raw(SEED + 1, t), label=label(SEED + 1, t, 0), observed=True) for t in range(3 * WINDOW)]
    first, first_gate = refit(calibration, dict(kind=0), (0, 2 * WINDOW), (2 * WINDOW, 3 * WINDOW))
    m, version, records, refits, versions = first, 1, [], [], []
    for t in range(DECISIONS):
        if t > 0 and t % (4 * WINDOW) == 0:
            m2, gate = refit(records, m, (t - 4 * WINDOW, t - 2 * WINDOW), (t - 2 * WINDOW, t - WINDOW))
            refits.append(gate)
            if gate["adopted"]:
                m, version = m2, version + 1
        shift = SHIFT if t >= DRIFT else 0
        a = arrival(SEED, t, WINDOW // 2)
        c, eligible, action, prop = decide(m, raw(SEED, t), uniform(SEED, 5 * t + 4))
        y = label(SEED, t, shift)
        cost = action * (1 - y) * POLICY["wrong_yes"] + (1 - action) * y * POLICY["wrong_no"]
        records.append(dict(raw=raw(SEED, t), calibrated=c, eligible=eligible, action=action, propensity=prop,
                            version=version, observed=a > 0, joined_at=a if a > 0 else -1,
                            label=y if a > 0 else 0.0, cost=cost if a > 0 else 0.0, truth=y))
        if t % WINDOW == WINDOW - 1:
            versions.append(version)
    windows = []
    for w in range(DECISIONS // WINDOW):
        rows = [r for r in records[w * WINDOW:(w + 1) * WINDOW] if r["observed"]]
        p, y = [r["calibrated"] for r in rows], [r["label"] for r in rows]
        observed = ece_loss(p, y)[0]
        exceed = 0
        for b in range(200):
            base = (w * 200 + b) * 1000
            z = [float(math.floor(uniform(WATCH_SEED, base + j) + p[j])) for j in range(len(p))]
            exceed += ece_loss(p, z)[0] >= observed
        windows.append(dict(joined=len(rows), ece=observed, exceed=exceed, alert=exceed < 2))
    values = []
    for th in THRESHOLDS:
        target = [int(r["calibrated"] >= th) for r in records]
        truth = math.fsum(a * (1 - r["truth"]) * POLICY["wrong_yes"] + (1 - a) * r["truth"] * POLICY["wrong_no"]
                          for a, r in zip(target, records)) / len(records)
        if any((r["eligible"] >> a) & 1 == 0 for a, r in zip(target, records)):
            values.append(dict(status=5, used=0, value=0.0, half=0.0, truth=truth))
            continue
        terms = [r["cost"] / r["propensity"] if r["action"] == a else 0.0 for a, r in zip(target, records) if r["observed"]]
        mean = math.fsum(terms) / len(terms)
        var = math.fsum((x - mean) * (x - mean) for x in terms) / (len(terms) - 1) / len(terms)
        values.append(dict(status=0, used=len(terms), value=mean, half=1.96 * math.sqrt(var), truth=truth))
    return dict(first=first_gate, records=records, refits=refits, versions=versions, windows=windows, values=values)


def show(r):
    print("first", r["first"])
    for k, g in enumerate(r["refits"]):
        print(f"refit {k}", g)
    lost = sum(not x["observed"] for x in r["records"])
    explored = sum(x["propensity"] == POLICY["explore"] for x in r["records"])
    print(f"lost {lost}, explored {explored}, versions {r['versions']}")
    for w, x in enumerate(r["windows"]):
        print(w, x)
    for th, v in zip(THRESHOLDS, r["values"]):
        print(th, v)


def doubles(xs):
    return ",".join(repr(float(x)) for x in xs)


def gate_line(g):
    return f"{{{int(g['adopted'])},{g['kind']},{{{doubles(g['ece'])}}},{{{doubles(g['loss'])}}}}}"


def header(r):
    rec = r["records"]
    lines = ["/* Generated by psionic/tests/drift_oracle.py --header. Do not edit. */", "#include <stdint.h>",
             "typedef struct {int64_t adopted,kind;double ece[4],loss[4];} dr_oracle_gate;",
             f"static const dr_oracle_gate dr_oracle_first = {gate_line(r['first'])};",
             f"static const int64_t dr_oracle_refits = {len(r['refits'])};",
             f"static const dr_oracle_gate dr_oracle_refit[{len(r['refits'])}] = {{{','.join(gate_line(g) for g in r['refits'])}}};",
             f"static const int64_t dr_oracle_versions[12] = {{{','.join(map(str, r['versions']))}}};",
             f"static const int64_t dr_oracle_counts[6] = {{{sum(not x['observed'] for x in rec)},"
             f"{sum(x['observed'] for x in rec)},{sum(x['propensity'] == POLICY['explore'] for x in rec)},"
             f"{sum(x['action'] for x in rec)},{sum(x['eligible'] for x in rec)},{sum(x['joined_at'] for x in rec)}}};",
             f"static const double dr_oracle_sums[3] = {{{doubles([math.fsum(x['calibrated'] for x in rec), math.fsum(x['propensity'] for x in rec), math.fsum(x['cost'] for x in rec)])}}};",
             f"static const int64_t dr_oracle_joined[12] = {{{','.join(str(w['joined']) for w in r['windows'])}}};",
             f"static const double dr_oracle_ece[12] = {{{doubles(w['ece'] for w in r['windows'])}}};",
             f"static const int64_t dr_oracle_exceed[12] = {{{','.join(str(w['exceed']) for w in r['windows'])}}};",
             f"static const int64_t dr_oracle_value_status[3] = {{{','.join(str(v['status']) for v in r['values'])}}};",
             f"static const int64_t dr_oracle_used[3] = {{{','.join(str(v['used']) for v in r['values'])}}};",
             f"static const double dr_oracle_value[3][3] = {{{','.join('{' + doubles([v['value'], v['half'], v['truth']]) + '}' for v in r['values'])}}};"]
    print("\n".join(lines))


if __name__ == "__main__":
    result = experiment()
    if "--header" in sys.argv:
        header(result)
    else:
        show(result)
