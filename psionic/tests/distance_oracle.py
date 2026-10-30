#!/usr/bin/env python3
"""Chapter 160's independent reference, written from the chapter's specification.

It shares no code with psionic/lib/distance.coax. The items, the backends'
answers, the centred-log distance, the summaries, the simulated rounds, the
limits and one checked round are written again here; the answers' digest uses
hashlib over chapter 178's values encoding. Sums are plain left-to-right loops,
as in the library. `--tests` prints the values the Coax checks pin;
`--header` writes the C expectations the native harness compares.
"""
import hashlib
import math
import struct
import sys

MASK = (1 << 64) - 1
OPTIONS, FLOOR = 8, 1e-6
PASS, LOST, STRIKE = range(3)
ITEMS_SEED, ITEMS = 160, 400
# name, seed, mix, grid, noise, share, shift
BACKENDS = (("reference", 1, 1.0, 0.0, 0.0, 0.0, 0.0),
            ("cpu", 2, 1.0, 0.0, 0.0005, 0.0, 0.0),
            ("gpu-f16", 3, 1.0, 0.0, 0.01, 0.0, 0.0),
            ("quantized", 4, 1.0, 0.25, 0.0, 0.0, 0.0),
            ("other-runtime", 5, 1.0, 0.0, 0.002, 0.5, 0.4),
            ("sibling", 6, 0.6, 0.0, 0.0, 0.0, 0.0))
HONEST, SUBSTITUTES = (1, 2), (3, 4, 5)
VERIFIED, ROUNDS, ROUND_SEED = 8, 20000, 1601
LIVE_SEED, LIVE_ITEMS, LIVE_CHECKED, SECRET = 9160, 8, 3, 0x5EC12E7


def uniform(seed, counter):
    z = (seed + 0x9E3779B97F4A7C15 * (counter + 1)) & MASK
    z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & MASK
    z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & MASK
    return ((z ^ (z >> 31)) >> 11) / 9007199254740992.0


def items(seed, n):
    out = []
    for i in range(n):
        kind = int(3 * uniform(seed, 4 * i))
        r = uniform(seed, 4 * i + 1)
        options = 2 + int(7 * r) if kind == 0 else 2 if kind == 1 else 3 + int(3 * r)
        out.append((kind, options, int(options * uniform(seed, 4 * i + 2))))
    return out


def logits(seed, its):
    out = []
    for i, (_, options, favourite) in enumerate(its):
        out.append([4 * uniform(seed, 4096 + 8 * i + j) - 2 + (2.0 if j == favourite else 0.0) for j in range(options)])
    return out


def answer(seed, its, b):
    _, bseed, mix, grid, noise, share, shift = b
    out = []
    for i, z in enumerate(logits(seed, its)):
        shifted = int(uniform(bseed, 16384 + i) + share) > 0
        w = []
        for j, x in enumerate(z):
            y = mix * x + (1 - mix) * (4 * uniform(bseed, 8 * i + j) - 2)
            if grid > 0:
                y = int(y / grid) * grid
            y += noise * (uniform(bseed, 8192 + 8 * i + j) - uniform(bseed, 12288 + 8 * i + j))
            if shifted:
                y += shift * (2 * uniform(bseed, 24576 + 8 * i + j) - 1)
            w.append(y)
        e = [math.exp(y - w[0]) for y in w]
        t = 0.0
        for x in e:
            t += x
        out.append([x / t for x in e])
    return out


def centred(p):
    lp = [math.log(max(x, FLOOR)) for x in p]
    t = 0.0
    for x in lp:
        t += x
    m = t / len(lp)
    return [x - m for x in lp]


def distance(p, q):
    a, b = centred(p), centred(q)
    return max(abs(x - y) for x, y in zip(a, b))


def top(p):
    best = 0
    for j in range(1, len(p)):
        if p[j] > p[best]:
            best = j
    return best


def quantile(sorted_d, pct):
    k = -(-pct * len(sorted_d) // 100)
    return sorted_d[k - 1]


def summary(d, agree):
    s = sorted(d)
    return dict(median=quantile(s, 50), p90=quantile(s, 90), p99=quantile(s, 99), max=s[-1], agree=agree)


def verdict(ds, limits):
    t = 0.0
    for x in ds:
        t += x
    mean = t / len(ds)
    if mean > limits["strike"] or max(ds) > limits["item"]:
        return STRIKE, mean
    if mean > limits["lost"]:
        return LOST, mean
    return PASS, mean


def pick(seed, n, v, base):
    idx = list(range(n))
    for k in range(v):
        j = k + int((n - k) * uniform(seed, base + k))
        idx[k], idx[j] = idx[j], idx[k]
    return idx[:v]


def simulate(d, limits, seed=ROUND_SEED, rounds=ROUNDS, v=VERIFIED):
    counts, worst = [0, 0, 0], 0.0
    for r in range(rounds):
        ds = [d[i] for i in pick(seed, len(d), v, r * v)]
        k, mean = verdict(ds, limits)
        counts[k] += 1
        worst = max(worst, mean)
    return counts, worst


def limits_from(honest_worst, honest_item):
    lost = 2 * honest_worst
    return dict(lost=lost, strike=3 * lost, item=2 * honest_item)


def gap(counts, honest, substitutes, rounds=ROUNDS):
    """Honest pairs pass every round; each substitute passes at most 5 in 100."""
    return all(counts[k][PASS] == rounds for k in honest) and all(100 * counts[k][PASS] <= 5 * rounds for k in substitutes)


def values_digest(values):
    b = b"".join(struct.pack("<Q", len(s)) + s for s in [b"psionic.suite.values.v1"])
    b += struct.pack("<Q", len(values)) + b"".join(struct.pack("<d", x) for x in values)
    return hashlib.sha256(b).digest()


def flat(answers):
    out = []
    for p in answers:
        out += p + [0.0] * (OPTIONS - len(p))
    return out


def live_round(b, limits):
    its = items(LIVE_SEED, LIVE_ITEMS)
    ref, got = answer(LIVE_SEED, its, BACKENDS[0]), answer(LIVE_SEED, its, b)
    dg = values_digest(flat(got))
    seed = struct.unpack("<Q", dg[:8])[0] ^ SECRET
    checked = pick(seed, LIVE_ITEMS, LIVE_CHECKED, 0)
    ds = [distance(ref[i], got[i]) for i in checked]
    k, mean = verdict(ds, limits)
    return dict(checked=checked, verdict=k, mean=mean, worst=max(ds), digest=dg)


def experiment():
    its = items(ITEMS_SEED, ITEMS)
    answers = [answer(ITEMS_SEED, its, b) for b in BACKENDS]
    pairs = {}
    for k in range(1, len(BACKENDS)):
        d = [distance(answers[0][i], answers[k][i]) for i in range(ITEMS)]
        agree = sum(top(answers[0][i]) == top(answers[k][i]) for i in range(ITEMS))
        pairs[k] = dict(d=d, **summary(d, agree))
    # Limits from the honest pairs: the worst simulated round, the worst item.
    probe = dict(lost=math.inf, strike=math.inf, item=math.inf)
    honest_worst = max(simulate(pairs[k]["d"], probe)[1] for k in HONEST)
    honest_item = max(pairs[k]["max"] for k in HONEST)
    limits = limits_from(honest_worst, honest_item)
    for k in pairs:
        pairs[k]["rounds"], pairs[k]["worst_round"] = simulate(pairs[k]["d"], limits)
    limits["gap"] = gap({k: p["rounds"] for k, p in pairs.items()}, HONEST, SUBSTITUTES)
    live = {k: live_round(BACKENDS[k], limits) for k in (2, 3)}
    return dict(items=its, pairs=pairs, honest_worst=honest_worst, honest_item=honest_item, limits=limits, live=live)


def show(r):
    for k, p in r["pairs"].items():
        print(f"{BACKENDS[k][0]:14s} median {p['median']:.4f} p90 {p['p90']:.4f} p99 {p['p99']:.4f} max {p['max']:.4f} "
              f"agree {p['agree']} rounds {p['rounds']} worst {p['worst_round']:.4f}")
    print("honest worst round", r["honest_worst"], "item", r["honest_item"])
    print("limits", r["limits"])
    for k, x in r["live"].items():
        print("live", BACKENDS[k][0], x["checked"], x["verdict"], x["mean"], x["worst"], x["digest"].hex()[:16])


def simulate_full(d, limits, seed, rounds, v):
    counts, worst, at = [0, 0, 0], 0.0, 0
    for r in range(rounds):
        ds = [d[i] for i in pick(seed, len(d), v, r * v)]
        k, mean = verdict(ds, limits)
        counts[k] += 1
        if r == 0 or mean > worst:
            worst, at = mean, r
    return counts, worst, at


def tests():
    its = items(5, 6)
    print("items", its)
    ref = answer(5, its, BACKENDS[0])
    q = answer(5, its, BACKENDS[3])
    o = answer(5, its, BACKENDS[4])
    print("ref0", [repr(x) for x in ref[0]])
    print("q0", [repr(x) for x in q[0]])
    print("d quantized", [repr(distance(ref[i], q[i])) for i in range(6)], sum(top(ref[i]) == top(q[i]) for i in range(6)))
    print("d other", [repr(distance(ref[i], o[i])) for i in range(6)], sum(top(ref[i]) == top(o[i]) for i in range(6)))
    sib = answer(5, its, BACKENDS[5])
    print("sib0", [repr(x) for x in sib[0]], "sib1", [repr(x) for x in sib[1]])
    print("q1", [repr(x) for x in q[1]], "o all", [repr(x) for x in o[3]])
    cpu = answer(5, its, BACKENDS[1])
    print("cpu1", [repr(x) for x in cpu[1]])
    d = [0.01 * k for k in (3, 1, 4, 1, 5, 9, 2, 6, 5, 3)]
    lim = dict(lost=0.04, strike=0.06, item=0.085)
    print("simulate", simulate_full(d, lim, 3, 50, 3))
    print("pick", pick(3, 10, 3, 0), pick(3, 10, 3, 3))
    its8 = items(77, 8)
    ref8, got8 = answer(77, its8, BACKENDS[0]), answer(77, its8, BACKENDS[3])
    dg = values_digest(flat(got8))
    seed = struct.unpack("<Q", dg[:8])[0] ^ 99
    checked = pick(seed, 8, 3, 0)
    ds = [distance(ref8[i], got8[i]) for i in checked]
    print("round", hex(seed), checked, verdict(ds, dict(lost=0.05, strike=0.1, item=0.12)), repr(max(ds)))
    same = values_digest(flat(ref8))
    print("same seed", hex(struct.unpack("<Q", same[:8])[0] ^ 99))


if __name__ == "__main__":
    tests() if "--tests" in sys.argv else show(experiment())
