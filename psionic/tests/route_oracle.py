#!/usr/bin/env python3
"""Chapter 164a's independent reference, written from the chapter's specification.

It shares no code with psionic/lib/route.coax. Sums use math.fsum (exactly
rounded) where the library uses chapter 3's compensated accumulator. Training
reuses chapter 70's oracle trainer, itself independent of the Coax library.
The draws, the old policy's propensities, the estimators and the experiment's
seeds are part of the specification. `--header` writes the C expectations
the native harness compares.
"""
import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tabular_oracle import bits, outputs, sigmoid, train, uniform  # noqa: E402

MODELS = ("small", "coder", "large")
# Each model: weights on length, code, difficulty and an irrelevant feature, then the bias.
WORLD_A = [[-1.0, -3.0, -6.0, 0.0, 4.0], [-0.5, 2.0, -3.0, 0.0, 2.0], [-0.5, 0.5, -2.5, 0.0, 3.5]]
# The same small and coder models, and a much worse large one.
WORLD_B = WORLD_A[:2] + [[-0.5, 0.5, -2.5, 0.0, 0.5]]
DOLLARS = (0.001, 0.01, 0.1)
SECONDS = ((0.3, 0.5), (0.8, 1.5), (2.0, 4.0))
PER_SECOND = 0.02
CONFIG = dict(epochs=40, batch=16, rate=0.5, decay=0.0, init_seed=1, shuffle_seed=2)
SEEDS = 20


def simulate(seed, n, truth):
    x, success, prob = [], [], []
    for i in range(n):
        c = 8 * i
        v = [uniform(seed, c), 1.0 if bits(seed, c + 1) % 5 < 2 else 0.0, uniform(seed, c + 2), 2 * uniform(seed, c + 3) - 1]
        p = [sigmoid(math.fsum([w[4]] + [w[j] * v[j] for j in range(4)])) for w in truth]
        x.append(v)
        prob.append(p)
        success.append([1.0 if uniform(seed, c + 4 + k) < p[k] else 0.0 for k in range(3)])
    return x, success, prob


def price(v):
    return [DOLLARS[k] + PER_SECOND * (SECONDS[k][0] + SECONDS[k][1] * v[0]) for k in range(3)]


def choose(prob, prices):
    out = []
    for p, q in zip(prob, prices):
        best = 0
        for k in (1, 2):
            if p[k] - q[k] > p[best] - q[best]:
                best = k
        out.append(best)
    return out


def value(actions, prob, prices):
    return math.fsum(prob[i][a] - prices[i][a] for i, a in enumerate(actions)) / len(actions)


def old(x, explore, large):
    out = []
    for v in x:
        rule = int(v[1])
        others = [k for k in range(3) if k != rule and (large or k != 2)]
        p = [0.0, 0.0, 0.0]
        for k in others:
            p[k] = explore / len(others)
        p[rule] = 1 - explore
        out.append(p)
    return out


def log(seed, props, success, prices):
    actions, outcome, reward = [], [], []
    for i, p in enumerate(props):
        u = uniform(seed, i)
        a = 0 if u < p[0] else 1 if u < p[0] + p[1] else 2
        actions.append(a)
        outcome.append(success[i][a])
        reward.append(success[i][a] - prices[i][a])
    return actions, outcome, reward


def fit(x, y, rows):
    return train(x, y, rows, 4, 0, True, CONFIG["epochs"], CONFIG["batch"], CONFIG["rate"], CONFIG["decay"],
                 CONFIG["init_seed"], CONFIG["shuffle_seed"])


def scores(params, x):
    z = [outputs(p, x, 4, 0) for p in params]
    return [[sigmoid(z[k][i]) for k in range(3)] for i in range(len(x))]


def averages(actions, outcome):
    means = []
    for k in range(3):
        seen = [o for a, o in zip(actions, outcome) if a == k]
        means.append(sum(seen) / len(seen))
    return [list(means) for _ in actions]


def estimate(target, actions, reward, props, model, prices):
    n = len(target)
    if any(props[i][target[i]] == 0 for i in range(n)):
        return None
    q = [model[i][target[i]] - prices[i][target[i]] for i in range(n)]
    hit = [i for i in range(n) if actions[i] == target[i]]
    naive = math.fsum(reward[i] for i in hit) / len(hit)
    direct = math.fsum(q) / n
    ips = math.fsum(reward[i] / props[i][target[i]] for i in hit) / n
    dr = math.fsum(q + [(reward[i] - q[i]) / props[i][target[i]] for i in hit]) / n
    return len(hit), naive, direct, ips, dr


def judge(router, tasks_seed, log_seed, truth, explore=0.2, large=True):
    """Everything chapter 164a measures about one log of the old policy."""
    x, success, prob = simulate(tasks_seed, 1000, truth)
    prices = [price(v) for v in x]
    target = choose(scores(router, x), prices)
    props = old(x, explore, large)
    actions, outcome, reward = log(log_seed, props, success, prices)
    out = dict(x=x, success=success, actions=actions, outcome=outcome, reward=reward, target=target,
               truth=value(target, prob, prices), earned=math.fsum(reward) / len(reward))
    if not large:
        out["refused"] = estimate(target, actions, reward, props, [[0.5] * 3] * len(x), prices) is None
        return out
    fitted = scores([fit(x, outcome, [i for i in range(len(x)) if actions[i] == k]) for k in range(3)], x)
    out["fitted"] = estimate(target, actions, reward, props, fitted, prices)
    out["average"] = estimate(target, actions, reward, props, averages(actions, outcome), prices)
    return out


def experiment():
    x, success, prob = simulate(164, 1500, WORLD_A)
    router = [fit(x, [s[k] for s in success], list(range(len(x)))) for k in range(3)]
    tx, _, tprob = simulate(165, 1000, WORLD_A)
    prices = [price(v) for v in tx]
    learned = choose(scores(router, tx), prices)
    best = choose(tprob, prices)
    r = dict(router=router,
             always=[value([k] * len(tx), tprob, prices) for k in range(3)],
             rule=value([int(v[1]) for v in tx], tprob, prices),
             learned=value(learned, tprob, prices), best=value(best, tprob, prices),
             counts=[learned.count(k) for k in range(3)],
             agree=sum(a == b for a, b in zip(learned, best)))
    r["log"] = judge(router, 166, 167, WORLD_A)
    errors = [[] for _ in range(6)]
    for s in range(SEEDS):
        j = judge(router, 1000 + s, 2000 + s, WORLD_A)
        f, a = j["fitted"], j["average"]
        for e, v in zip(errors, (f[1], f[2], f[3], f[4], a[2], a[4])):
            e.append(v - j["truth"])
    r["errors"] = [(math.fsum(e) / SEEDS, math.sqrt(math.fsum(v * v for v in e) / SEEDS)) for e in errors]
    a = judge(router, 166, 167, WORLD_A, large=False)
    b = judge(router, 166, 167, WORLD_B, large=False)
    r["narrow"] = a
    r["same_logs"] = all(a[k] == b[k] for k in ("x", "actions", "outcome", "reward"))
    r["world_b"] = b["truth"]
    r["world_b_wide"] = judge(router, 166, 167, WORLD_B)
    return r


def show(r):
    print("always " + ", ".join(f"{m} {v:.4f}" for m, v in zip(MODELS, r["always"])))
    print(f"rule {r['rule']:.4f}, learned {r['learned']:.4f} {r['counts']}, best {r['best']:.4f}, agree {r['agree']}")
    j = r["log"]
    print(f"log: truth {j['truth']:.4f}, earned {j['earned']:.4f}, fitted {j['fitted']}, average {j['average']}")
    for name, (mean, rms) in zip(("naive", "direct", "ips", "dr", "direct avg", "dr avg"), r["errors"]):
        print(f"  {name}: mean {mean:+.4f}, rms {rms:.4f}")
    print(f"narrow refused {r['narrow']['refused']}, same logs {r['same_logs']}, world B {r['world_b']:.4f}, "
          f"world B wide {r['world_b_wide']['truth']:.4f} ips {r['world_b_wide']['fitted'][3]:.4f}")


def c_double(v):
    return repr(float(v))


def header(r):
    lines = ["/* Generated by psionic/tests/route_oracle.py --header. Do not edit. */",
             "#include <stdint.h>"]

    def doubles(name, values):
        lines.append(f"static const double {name}[{len(values)}] = {{{','.join(c_double(v) for v in values)}}};")

    def ints(name, values):
        lines.append(f"static const int64_t {name}[{len(values)}] = {{{','.join(str(int(v)) for v in values)}}};")

    doubles("rt_oracle_router", [p for params in r["router"] for p in params])
    doubles("rt_oracle_values", r["always"] + [r["rule"], r["learned"], r["best"]])
    ints("rt_oracle_counts", r["counts"] + [r["agree"]])
    j = r["log"]
    ints("rt_oracle_log_actions", j["actions"])
    ints("rt_oracle_log_target", j["target"])
    doubles("rt_oracle_log_values", [j["truth"], j["earned"]])
    ints("rt_oracle_log_matched", [j["fitted"][0], j["average"][0]])
    doubles("rt_oracle_log_fitted", j["fitted"][1:])
    doubles("rt_oracle_log_average", j["average"][1:])
    doubles("rt_oracle_errors", [v for pair in r["errors"] for v in pair])
    w = r["world_b_wide"]
    doubles("rt_oracle_world_b", [r["world_b"], w["fitted"][3]])
    ints("rt_oracle_narrow", [int(r["narrow"]["refused"]), int(r["same_logs"])])
    print("\n".join(lines))


if __name__ == "__main__":
    result = experiment()
    if "--header" in sys.argv:
        header(result)
    else:
        show(result)
