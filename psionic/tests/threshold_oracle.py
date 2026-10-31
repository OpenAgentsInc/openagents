#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
"""Chapter 214's independent reference, written from the chapter's specification.

It shares no code with psionic/lib/threshold.coax. Sums use math.fsum
(exactly rounded) where the library uses chapter 4's compensated accumulator.
Band edges are found by a different search: a binary search over the integer
bit patterns of nonnegative doubles, for the first number whose cheapest
action differs from the band's. The draws, the costs and the experiment's
seeds are part of the specification. `--header` writes the C expectations the
native harness compares.
"""
import math
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from tabular_oracle import uniform  # noqa: E402

NO, YES, ASK, ESCALATE = range(4)
FIELDS = ("wrong_yes", "wrong_no", "ask", "per_second", "elephant_dollars", "elephant_seconds",
          "mammoth_dollars", "mammoth_seconds", "person_seconds")
VOICE = dict(wrong_yes=1, wrong_no=3, ask=0.8, per_second=2, elephant_dollars=0.0005, elephant_seconds=0.15,
             mammoth_dollars=0.01, mammoth_seconds=1.2, person_seconds=1)
BACKGROUND = dict(wrong_yes=5, wrong_no=1, ask=0.2, per_second=0.001, elephant_dollars=0.0005,
                  elephant_seconds=0.15, mammoth_dollars=0.01, mammoth_seconds=1.2, person_seconds=300)


def sigmoid(z):
    # The library's form, exp(min(z, 0)) / (1 + exp(-|z|)), is part of the specification.
    return math.exp((z - abs(z)) / 2) / (1 + math.exp(-abs(z)))


def posterior(p, rho, up):
    a, b = (p * rho, (1 - p) * (1 - rho)) if up else (p * (1 - rho), (1 - p) * rho)
    return a / (a + b)


def answer(c, p):
    costs = [p * c["wrong_no"], (1 - p) * c["wrong_yes"], c["ask"] + c["per_second"] * c["person_seconds"]]
    best = min(range(3), key=lambda k: (costs[k], k))
    return best, costs[best]


def escalate(c, p, rho):
    up = p * rho + (1 - p) * (1 - rho)
    after = up * answer(c, posterior(p, rho, True))[1] + (1 - up) * answer(c, posterior(p, rho, False))[1]
    return c["mammoth_dollars"] + c["per_second"] * c["mammoth_seconds"] + after


def best(c, p, rho):
    a, now = answer(c, p)
    return ESCALATE if escalate(c, p, rho) < now else a


def bits(x):
    return struct.unpack("<q", struct.pack("<d", x))[0]


def double(b):
    return struct.unpack("<d", struct.pack("<q", b))[0]


def record(c, rho):
    """Bands seen at k/4096; each edge the first double above the last grid point with the old action."""
    edges, actions = [0.0], [NO]
    last = 0.0
    for k in range(1, 4097):
        p = k / 4096
        a = best(c, p, rho)
        if a != actions[-1]:
            lo, hi = bits(last), bits(p)
            while hi - lo > 1:
                mid = (lo + hi) // 2
                if best(c, double(mid), rho) == actions[-1]:
                    lo = mid
                else:
                    hi = mid
            edges.append(double(hi))
            actions.append(a)
        last = p
    assert len(actions) <= 4
    return edges, actions


def decide(edges, actions, p):
    k = 0
    for b in range(1, len(edges)):
        if edges[b] <= p:
            k = b
    return actions[k]


def simulate(seed, n, rho):
    e, m, y = [], [], []
    for i in range(n):
        c = 3 * i
        p = sigmoid(8 * uniform(seed, c) - 4)
        label = math.floor(uniform(seed, c + 1) + p)
        wrong = math.floor(uniform(seed, c + 2) + (1 - rho))
        e.append(p)
        y.append(float(label))
        m.append(posterior(p, rho, label != wrong))
    return e, m, y


def estimate_rho(e, m, y):
    return sum(1 for a, b, t in zip(e, m, y) if (b > a) == (t == 1)) / len(e)


def item(c, policy, edges, actions, p, q, y):
    seconds, dollars = c["elephant_seconds"], c["elephant_dollars"]
    act = YES if p >= 0.5 else NO
    escalated = False
    if policy == "mammoth":
        seconds, dollars, escalated = 0.0, 0.0, True
    elif policy == "fixed":
        escalated = 0.2 < p < 0.8
    elif policy == "record":
        act = decide(edges, actions, p)
        escalated = act == ESCALATE
    if escalated:
        seconds += c["mammoth_seconds"]
        dollars += c["mammoth_dollars"]
        act = answer(c, q)[0] if policy == "record" else (YES if q >= 0.5 else NO)
    asked = act == ASK
    if asked:
        seconds += c["person_seconds"]
        dollars += c["ask"]
    wrong = (act == YES and y == 0) or (act == NO and y == 1)
    if wrong:
        dollars += c["wrong_yes"] if act == YES else c["wrong_no"]
    return dollars + c["per_second"] * seconds, seconds, wrong, asked, escalated and policy != "mammoth"


def interval(x):
    mean = math.fsum(x) / len(x)
    var = math.fsum((v - mean) * (v - mean) for v in x) / (len(x) - 1)
    return mean, 1.96 * math.sqrt(var / len(x))


POLICIES = ("elephant", "mammoth", "fixed", "record")


def experiment():
    e, m, y = simulate(157, 2000, 0.9)
    rho = estimate_rho(e, m, y)
    ce, cm, cy = simulate(158, 4000, 0.9)
    out = dict(rho=rho, consumers=[])
    records = [record(VOICE, rho), record(BACKGROUND, rho)]
    out["differ"] = sum(decide(*records[0], p) != decide(*records[1], p) for p in ce)
    out["agree"] = sum(decide(*r, p) == best(c, p, rho) for p in ce for r, c in zip(records, (VOICE, BACKGROUND)))
    for c, (edges, actions) in zip((VOICE, BACKGROUND), records):
        runs = []
        for policy in POLICIES:
            items = [item(c, policy, edges, actions, p, q, t) for p, q, t in zip(ce, cm, cy)]
            costs = [i[0] for i in items]
            mean, half = interval(costs)
            runs.append(dict(costs=costs, mean=mean, half=half, errors=sum(i[2] for i in items),
                             asked=sum(i[3] for i in items), escalated=sum(i[4] for i in items),
                             seconds=math.fsum(i[1] for i in items) / len(items)))
        top = 0
        for k in range(3):
            if runs[k]["mean"] < runs[top]["mean"]:
                top = k
        paired = interval([a - b for a, b in zip(runs[3]["costs"], runs[top]["costs"])])
        out["consumers"].append(dict(edges=edges, actions=actions, runs=runs, best=top, paired=paired))
    return out


def show(r):
    print(f"rho {r['rho']}, differ {r['differ']}, agree {r['agree']}")
    for name, c in zip(("voice", "background"), r["consumers"]):
        print(name, list(zip(c["actions"], c["edges"])))
        for policy, run in zip(POLICIES, c["runs"]):
            print(f"  {policy}: {run['mean']:.4f} ± {run['half']:.4f}, errors {run['errors']}, asked {run['asked']}, "
                  f"escalated {run['escalated']}, {run['seconds']:.2f} s")
        print(f"  record minus {POLICIES[c['best']]}: {c['paired'][0]:.4f} ± {c['paired'][1]:.4f}")


def header(r):
    lines = ["/* Generated by psionic/tests/threshold_oracle.py --header. Do not edit. */", "#include <stdint.h>",
             f"static const double th_oracle_rho = {r['rho']!r};",
             f"static const int64_t th_oracle_counts[2] = {{{r['differ']},{r['agree']}}};"]
    for k, c in enumerate(r["consumers"]):
        edges = c["edges"] + [0.0] * (4 - len(c["edges"]))
        actions = c["actions"] + [0] * (4 - len(c["actions"]))
        lines.append(f"static const int64_t th_oracle_bands_{k} = {len(c['edges'])};")
        lines.append(f"static const double th_oracle_edges_{k}[4] = {{{','.join(repr(v) for v in edges)}}};")
        lines.append(f"static const int64_t th_oracle_actions_{k}[4] = {{{','.join(map(str, actions))}}};")
        runs = c["runs"]
        lines.append(f"static const double th_oracle_means_{k}[4][3] = {{"
                     + ",".join(f"{{{run['mean']!r},{run['half']!r},{run['seconds']!r}}}" for run in runs) + "};")
        lines.append(f"static const int64_t th_oracle_tallies_{k}[4][3] = {{"
                     + ",".join(f"{{{run['errors']},{run['asked']},{run['escalated']}}}" for run in runs) + "};")
        lines.append(f"static const int64_t th_oracle_best_{k} = {c['best']};")
        lines.append(f"static const double th_oracle_paired_{k}[2] = {{{c['paired'][0]!r},{c['paired'][1]!r}}};")
    print("\n".join(lines))


if __name__ == "__main__":
    result = experiment()
    if "--header" in sys.argv:
        header(result)
    else:
        show(result)
