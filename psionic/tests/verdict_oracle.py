#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
"""Chapter 239's independent reference, written from the chapter's specification.

It shares no code with psionic/lib/verdict.coax. The gates are written again
here; the runs, bootstraps and floors come from chapter 238's oracle, the maps
and measures from chapter 213's, and the thresholds and costs from chapter
214's, each of which is itself independent of the library. Sums use math.fsum.
`--header` writes the C expectations the native harness compares.
"""
import math
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import calibrate_oracle as cal  # noqa: E402
import noise_oracle as ns  # noqa: E402
import threshold_oracle as th  # noqa: E402

UNVERIFIABLE, PASSED, FAILED = range(3)
(NONE, BEYOND_FLOOR, WITHIN_FLOOR, NEAR_FLOOR, NO_HEADROOM, NO_FLOOR, FEW_TASKS, LOSS_LOWER, LOSS_HIGHER,
 MORE_CONFIDENT_ERRORS, LOSS_UNCLEAR, OVER_COST, OVER_TIME, UNMEASURED) = range(14)
MIN_TASKS = 10
REPLICATES = 1000
CANDIDATES = ((-0.5, 4000), (0.5, 2000), (1.5, 3000), (2.5, 5000))
LIMITS = (0.5, 30.0)


def clear_above(below, replicates):
    """At most 1 replicate in 40 at or below the line."""
    return 40 * below <= replicates


def clear_below(below, replicates):
    return 40 * (replicates - below) <= replicates


def decision(a, b, task, part, which, floor, ceiling, seed):
    noise = floor if floor is not None else 0.0
    room = ns.headroom(a, part, which, ceiling, noise)
    gain = ns.bootstrap(a, b, task, part, which, noise, seed, REPLICATES)
    if room["refused"]:
        verdict = (UNVERIFIABLE, NO_HEADROOM)
    elif floor is None:
        verdict = (UNVERIFIABLE, NO_FLOOR)
    elif gain["tasks"] < MIN_TASKS:
        verdict = (UNVERIFIABLE, FEW_TASKS)
    elif clear_above(gain["below"], REPLICATES):
        verdict = (PASSED, BEYOND_FLOOR)
    elif clear_below(gain["below"], REPLICATES):
        verdict = (FAILED, WITHIN_FLOOR)
    else:
        verdict = (UNVERIFIABLE, NEAR_FLOOR)
    return dict(verdict=verdict, room=room, gain=gain)


def probability(a, b, y, task, seed):
    def per_item(p):
        loss = [math.log(cal.clamp(v) if t == 1 else 1 - cal.clamp(v)) for v, t in zip(p, y)]
        right = [(v >= 0.5) == (t == 1) for v, t in zip(p, y)]
        wrong = [-1.0 if max(v, 1 - v) >= 0.9 and not r else 0.0 for v, r in zip(p, right)]
        return loss, right, wrong
    (la, ra, wa), (lb, rb, wb) = per_item(a), per_item(b)
    zero = [0] * len(y)
    lower = ns.bootstrap(la, lb, task, zero, 0, 0.0, seed, REPLICATES)
    higher = ns.bootstrap(lb, la, task, zero, 0, 0.0, seed, REPLICATES)
    confident = ns.bootstrap(wb, wa, task, zero, 0, 0.0, seed, REPLICATES)
    if lower["tasks"] < MIN_TASKS:
        verdict = (UNVERIFIABLE, FEW_TASKS)
    elif clear_above(higher["below"], REPLICATES):
        verdict = (FAILED, LOSS_HIGHER)
    elif clear_above(confident["below"], REPLICATES):
        verdict = (FAILED, MORE_CONFIDENT_ERRORS)
    elif clear_above(lower["below"], REPLICATES):
        verdict = (PASSED, LOSS_LOWER)
    else:
        verdict = (UNVERIFIABLE, LOSS_UNCLEAR)
    return dict(verdict=verdict, loss=[-math.fsum(la) / len(y), -math.fsum(lb) / len(y)],
                errors=[int(-sum(wa)), int(-sum(wb))], ece=[cal.measure(a, y)["ece"], cal.measure(b, y)["ece"]],
                correct=[sum(ra), sum(rb)], below=[lower["below"], higher["below"], confident["below"]])


def evaluate(quality, cost, seconds, cost_limit, seconds_limit):
    def measured(v):
        return v == v and 0 <= v <= 1e300
    if quality[0] == FAILED:
        return quality
    if measured(cost) and measured(cost_limit) and cost > cost_limit:
        return (FAILED, OVER_COST)
    if measured(seconds) and measured(seconds_limit) and seconds > seconds_limit:
        return (FAILED, OVER_TIME)
    if not all(measured(v) for v in (cost, seconds, cost_limit, seconds_limit)):
        return (UNVERIFIABLE, UNMEASURED)
    return quality


def experiment():
    out = {}
    task = [ns.task_of(i) for i in range(ns.ITEMS)]
    part = [ns.part_of(i) for i in range(ns.ITEMS)]
    base = [ns.run(r, 0) for r in range(ns.BASELINE_RUNS)]
    floors = [ns.floor_of(base, part, w, ns.BLOCK)["floor"] for w in (0, 1)]
    a = ns.means(base[:ns.BLOCK])
    out["candidates"] = []
    for skill, first in CANDIDATES:
        b = ns.means([ns.run(first + q, skill) for q in range(ns.BLOCK)])
        out["candidates"].append([decision(a, b, task, part, w, floors[w], 1.0, 7) for w in (0, 1)])

    items = ns.elephant()
    correct = [float(ns.first_max(p) == g) for _, g, p in items]
    out["elephant"] = decision(correct, correct, list(range(12)), [0] * 12, 0, None, 1.0, 7)

    fits, _, tests = (cal.simulate(156 + s, cal.MODELS[0][2], cal.N) for s in range(3))
    out["calibration"] = []
    for k, kind in ((0, 1), (1, 2)):
        fp, fy = cal.question(fits, k)
        tp, ty = cal.question(tests, k)
        m = cal.fit(kind, fp, fy)
        mapped = [cal.apply(m, v) for v in tp]
        out["calibration"].append(probability(tp, mapped, ty, [i // 4 for i in range(len(tp))], 7))

    ce, cm, cy = th.simulate(158, 4000, 0.9)
    groups, zero = [i // 4 for i in range(4000)], [0] * 4000
    out["thresholds"] = []
    for k, (c, baseline) in enumerate(((th.VOICE, "elephant"), (th.BACKGROUND, "mammoth"))):
        runs = []
        for s in range(16):
            e, m, y = th.simulate(157 if s == 0 else 1000 + s, 2000, 0.9)
            edges, actions = th.record(c, th.estimate_rho(e, m, y))
            items = [th.item(c, "record", edges, actions, p, q, t) for p, q, t in zip(ce, cm, cy)]
            runs.append([-i[0] for i in items])
            if s == 0:
                mean = math.fsum(i[0] for i in items) / 4000
                seconds = math.fsum(i[1] for i in items) / 4000
        floor = ns.floor_of(runs, zero, 0, 1)["floor"]
        base_items = [th.item(c, baseline, [0.0], [th.NO], p, q, t) for p, q, t in zip(ce, cm, cy)]
        a = [-i[0] for i in base_items]
        d = decision(a, runs[0], groups, zero, 0, floor, 0.0, 7)
        out["thresholds"].append(dict(base=math.fsum(i[0] for i in base_items) / 4000, mean=mean, seconds=seconds,
                                      floor=floor, decision=d,
                                      evaluated=evaluate(d["verdict"], mean, seconds, 1.0, LIMITS[k])))
    return out


def show(r):
    for (skill, _), ds in zip(CANDIDATES, r["candidates"]):
        for w, d in enumerate(ds):
            print(skill, w, round(d["gain"]["gain"], 4), d["gain"]["below"], d["verdict"])
    print("elephant", r["elephant"]["verdict"])
    for p in r["calibration"]:
        print(p)
    for t in r["thresholds"]:
        print({k: v for k, v in t.items() if k != "decision"}, t["decision"]["gain"], t["decision"]["verdict"])


def doubles(xs):
    return ",".join(repr(float(x)) for x in xs)


def header(r):
    dec = lambda d: f"{{{d['verdict'][0]},{d['verdict'][1]},{d['gain']['gain']!r},{d['gain']['below']},{d['gain']['tasks']}}}"
    lines = ["/* Generated by psionic/tests/verdict_oracle.py --header. Do not edit. */", "#include <stdint.h>",
             "typedef struct {int64_t verdict,reason;double gain;int64_t below,tasks;} vd_oracle_decision;",
             "typedef struct {int64_t verdict,reason;double loss[2];int64_t errors[2];double ece[2];int64_t correct[2],below[3];} vd_oracle_probability;",
             "typedef struct {double base,mean,seconds,floor;vd_oracle_decision decision;int64_t verdict,reason;} vd_oracle_threshold;",
             f"static const vd_oracle_decision vd_oracle_candidates[4][2] = {{{','.join('{' + ','.join(map(dec, ds)) + '}' for ds in r['candidates'])}}};",
             f"static const vd_oracle_decision vd_oracle_elephant = {dec(r['elephant'])};"]
    probs = []
    for p in r["calibration"]:
        probs.append(f"{{{p['verdict'][0]},{p['verdict'][1]},{{{doubles(p['loss'])}}},{{{p['errors'][0]},{p['errors'][1]}}},"
                     f"{{{doubles(p['ece'])}}},{{{p['correct'][0]},{p['correct'][1]}}},{{{','.join(map(str, p['below']))}}}}}")
    lines.append(f"static const vd_oracle_probability vd_oracle_calibration[2] = {{{','.join(probs)}}};")
    ts = [f"{{{doubles([t['base'], t['mean'], t['seconds'], t['floor']])},{dec(t['decision'])},{t['evaluated'][0]},{t['evaluated'][1]}}}"
          for t in r["thresholds"]]
    lines.append(f"static const vd_oracle_threshold vd_oracle_thresholds[2] = {{{','.join(ts)}}};")
    print("\n".join(lines))


if __name__ == "__main__":
    result = experiment()
    if "--header" in sys.argv:
        header(result)
    else:
        show(result)
