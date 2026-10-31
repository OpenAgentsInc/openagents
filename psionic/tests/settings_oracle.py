#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
"""Chapter 241's independent reference, written from the chapter's specification.

It shares no code with psionic/lib/settings.coax. The simulator, the rule, the
candidates and the coordinate search are written again here; the floors and
the decision gate come from chapters 238 and 239's oracles, which are
themselves independent of the libraries. `--tests` prints the values the Coax
checks pin; `--header` writes the C expectations the native harness compares.
"""
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import noise_oracle as ns  # noqa: E402
import verdict_oracle as vd  # noqa: E402

MASK = (1 << 64) - 1
STEPS, REPEATS, WEIGHT_MAX = 10, 8, 4.0
MISS, ALARM = 4.0, 1.0
START = (0.0, 0.5, 700.0)
FOLDS, PASSES = 5, 8
FIT_RUNS, HELD_RUNS = 40, 80
FIT_SEED, HELD_SEED = 182, 9182
WEIGHT, THRESHOLD, QUIET = range(3)
NONE, ACCEPTED, NO_GAIN, NO_HELD_GAIN = range(4)


def uniform(seed, counter):
    z = (seed + 0x9E3779B97F4A7C15 * (counter + 1)) & MASK
    z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & MASK
    z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & MASK
    return ((z ^ (z >> 31)) >> 11) / 9007199254740992.0


def simulate(seed, runs):
    rows = []
    for g in range(runs):
        prone = uniform(seed, g)
        for s in range(STEPS):
            b = 1000 * (g + 1) + 8 * s
            stalled = int(uniform(seed, b) + 0.1 + 0.4 * prone) > 0
            a1, a2, a3 = uniform(seed, b + 1), uniform(seed, b + 2), uniform(seed, b + 3)
            if stalled:
                row = (g, 0.2 + 0.5 * (a1 + a2) / 2, int(9 * a3), 30 + 700 * uniform(seed, b + 4), True)
            else:
                row = (g, 0.55 * (a1 + a2) / 2, int(4 * a3 * a3), 700 * uniform(seed, b + 4) * uniform(seed, b + 5), False)
            rows.append(row)
    return rows


def score_of(st, row):
    return row[1] + st[WEIGHT] * row[2] / REPEATS


def flagged(st, row):
    return score_of(st, row) >= st[THRESHOLD] or row[3] >= st[QUIET]


def score(rows, st):
    flags = [flagged(st, r) for r in rows]
    stalls = sum(r[4] for r in rows)
    caught = sum(f and r[4] for f, r in zip(flags, rows))
    alarms = sum(f and not r[4] for f, r in zip(flags, rows))
    missed = stalls - caught
    return dict(rows=len(rows), stalls=stalls, flagged=sum(flags), caught=caught, missed=missed, alarms=alarms,
                cost=MISS * missed + ALARM * alarms)


def candidates(rows, st, which):
    if which == WEIGHT:
        points = sorted({(st[THRESHOLD] - r[1]) * REPEATS / r[2] for r in rows
                         if r[1] < st[THRESHOLD] and (st[THRESHOLD] - r[1]) * REPEATS < WEIGHT_MAX * r[2]})
        return [0.0] + [(a + b) / 2 for a, b in zip(points, points[1:])] + [WEIGHT_MAX]
    points = sorted({score_of(st, r) if which == THRESHOLD else r[3] for r in rows})
    return [points[0]] + [(a + b) / 2 for a, b in zip(points, points[1:])] + [points[-1] + 1]


def put(st, which, x):
    return tuple(x if k == which else v for k, v in enumerate(st))


def best(rows, st, which):
    current = score(rows, st)["cost"]
    value, cost = st[which], current
    for x in candidates(rows, st, which):
        c = score(rows, put(st, which, x))["cost"]
        if c < cost:
            value, cost = x, c
    return value, cost, current


def fit(rows, st, cv, folds=FOLDS, passes=PASSES):
    log = []
    for p in range(passes):
        changed = False
        for which in range(3):
            value, cost, current = best(rows, st, which)
            held = 0.0
            if cost >= current:
                outcome = NO_GAIN
            elif cv:
                for f in range(folds):
                    train = [r for r in rows if r[0] % folds != f]
                    test = [r for r in rows if r[0] % folds == f]
                    held += score(test, put(st, which, best(train, st, which)[0]))["cost"]
                outcome = ACCEPTED if held < current else NO_HELD_GAIN
            else:
                outcome = ACCEPTED
            log.append(dict(pass_=p, which=which, outcome=outcome, from_=st[which], to=value, before=current, after=cost, held=held))
            if outcome == ACCEPTED:
                st = put(st, which, value)
                changed = True
        if not changed:
            break
    return st, log


def values(rows, st):
    return [-(MISS if r[4] and not flagged(st, r) else ALARM if flagged(st, r) and not r[4] else 0.0) for r in rows]


def experiment():
    out = {}
    rows = simulate(FIT_SEED, FIT_RUNS)
    out["rows"] = rows
    out["start"] = score(rows, START)
    out["fits"] = {}
    for cv in (True, False):
        settings = []
        for s in range(16):
            r = rows if s == 0 else simulate(1000 + s, FIT_RUNS)
            st, log = fit(r, START, cv)
            settings.append(st)
            if s == 0:
                out["fits"][cv] = dict(settings=st, log=log, score=score(rows, st))
        out["fits"][cv]["all"] = settings
    # Everything above is written down before the held-out runs are read.
    held = simulate(HELD_SEED, HELD_RUNS)
    groups, zero = [r[0] for r in held], [0] * len(held)
    out["held"] = dict(start=score(held, START))
    base = values(held, START)
    for cv in (True, False):
        runs = [values(held, st) for st in out["fits"][cv]["all"]]
        f = ns.floor_of(runs, zero, 0, 1)
        out["held"][cv] = dict(score=score(held, out["fits"][cv]["settings"]),
                               costs=[score(held, st)["cost"] for st in out["fits"][cv]["all"]], floor=f["floor"],
                               mean=f["mean"])
        out["held"][cv]["runs"] = runs
    out["gate_fit"] = vd.decision(base, out["held"][True]["runs"][0], groups, zero, 0, out["held"][True]["floor"], 0.0, 7)
    # The two rules compared by fit set: mean value per step, fit only against cross-validated.
    means = {cv: [-c / len(held) for c in out["held"][cv]["costs"]] for cv in (True, False)}
    out["boot"] = ns.bootstrap(means[True], means[False], list(range(16)), [0] * 16, 0, 0.0, 7, 1000)
    return out


def tests():
    """Values the Coax checks pin: a fit on four simulated runs."""
    rows = simulate(5, 4)
    for cv in (True, False):
        st, log = fit(rows, START, cv, folds=2, passes=4)
        print("cv" if cv else "fit only", [repr(x) for x in st], score(rows, st))
        for step in log:
            print("  ", step)


def show(r):
    print("start", r["start"])
    for cv in (True, False):
        f = r["fits"][cv]
        print("cv" if cv else "fit only", f["settings"], f["score"])
        for step in f["log"]:
            print("  ", step)
    print("held start", r["held"]["start"])
    for cv in (True, False):
        h = r["held"][cv]
        print("held", cv, h["score"], "floor", h["floor"], "costs", h["costs"])
    print("gate_fit", r["gate_fit"]["verdict"], r["gate_fit"]["gain"], r["gate_fit"]["room"])
    print("boot", r["boot"])


def header(r):
    def row(x):
        return f"{{{x[0]},{x[1]!r},{x[2]},{x[3]!r},{int(x[4])}}}"

    def sc(s):
        return f"{{{s['rows']},{s['stalls']},{s['flagged']},{s['caught']},{s['missed']},{s['alarms']},{s['cost']!r}}}"

    def step(s):
        return (f"{{{s['pass_']},{s['which']},{s['outcome']},{s['from_']!r},{s['to']!r},{s['before']!r},{s['after']!r},"
                f"{s['held']!r}}}")

    def dec(d):
        return f"{{{d['verdict'][0]},{d['verdict'][1]},{d['gain']['gain']!r},{d['gain']['below']},{d['gain']['tasks']}}}"
    lines = ["/* Generated by psionic/tests/settings_oracle.py --header. Do not edit. */", "#include <stdint.h>",
             "typedef struct {int64_t group;double p;int64_t repeats;double quiet;int64_t stalled;} st_oracle_row;",
             "typedef struct {int64_t rows,stalls,flagged,caught,missed,alarms;double cost;} st_oracle_score;",
             "typedef struct {int64_t pass,which,outcome;double from,to,before,after,held;} st_oracle_step;",
             "typedef struct {int64_t verdict,reason;double gain;int64_t below,tasks;} st_oracle_decision;",
             f"static const st_oracle_row st_oracle_rows[{len(r['rows'])}] = {{{','.join(map(row, r['rows']))}}};",
             f"static const st_oracle_score st_oracle_start = {sc(r['start'])};",
             f"static const st_oracle_score st_oracle_held_start = {sc(r['held']['start'])};"]
    for cv, name in ((True, "cv"), (False, "fit_only")):
        f, h = r["fits"][cv], r["held"][cv]
        lines.append(f"static const double st_oracle_{name}_settings[16][3] = {{{','.join('{' + ','.join(map(repr, st)) + '}' for st in f['all'])}}};")
        lines.append(f"static const st_oracle_step st_oracle_{name}_log[{len(f['log'])}] = {{{','.join(map(step, f['log']))}}};")
        lines.append(f"static const st_oracle_score st_oracle_{name}_score = {sc(f['score'])};")
        lines.append(f"static const st_oracle_score st_oracle_{name}_held = {sc(h['score'])};")
        lines.append(f"static const double st_oracle_{name}_held_costs[16] = {{{','.join(map(repr, h['costs']))}}};")
        lines.append(f"static const double st_oracle_{name}_floor = {h['floor']!r};")
    lines.append(f"static const st_oracle_decision st_oracle_gate_fit = {dec(r['gate_fit'])};")
    b = r["boot"]
    lines.append(f"static const double st_oracle_boot_gain = {b['gain']!r};")
    lines.append(f"static const int64_t st_oracle_boot_below = {b['below']};")
    print("\n".join(lines))


if __name__ == "__main__":
    if "--tests" in sys.argv:
        tests()
    else:
        result = experiment()
        header(result) if "--header" in sys.argv else show(result)
