#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
"""Chapter 242's independent reference, written from the chapter's specification.

It shares no code with psionic/lib/search.coax. Manifest digests use hashlib
over chapter 237's encoding; the operators, the world, the measurement tiers,
successive halving and the per-task frontier are written again here; the floor
and the decision gate come from chapters 238 and 239's oracles. Sums are plain
left-to-right loops, as in the library. `--tests` prints the values the Coax
checks pin; `--header` writes the C expectations the native harness compares.
"""
import hashlib
import os
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import noise_oracle as ns  # noqa: E402
import verdict_oracle as vd  # noqa: E402

MASK = (1 << 64) - 1
SKILL, PRICE, LONG, CHECK, LAMBDA = (0.0, 1.5), (1.0, 6.0), 2.0, 0.5, 0.04
ESCALATE, RETRY, WIDEN, VERIFY = range(4)
OPS = ("escalate", "retry", "widen", "verify")
SEED = (0, 0, 0, 0)
DEV_SEED, HELD_SEED, TASKS = 183, 9183, 60
TIERS = ((12, 1, 301), (30, 2, 302), (60, 4, 303))
KEEPS = (16, 8)
HELD_TRIALS, HELD_TRIAL_SEED = 4, 501


def uniform(seed, counter):
    z = (seed + 0x9E3779B97F4A7C15 * (counter + 1)) & MASK
    z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & MASK
    z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & MASK
    return ((z ^ (z >> 31)) >> 11) / 9007199254740992.0


def text(s):
    b = s.encode()
    return struct.pack("<Q", len(b)) + b


def digest(p):
    model, retries, context, verify = p
    return hashlib.sha256(text("psionic.policy.v1") + bytes([model]) + struct.pack("<Q", retries) + bytes([context, verify])).digest()


def apply(p, op):
    model, retries, context, verify = p
    if op == ESCALATE and model == 0:
        return (1, retries, context, verify)
    if op == RETRY and retries < 3:
        return (model, retries + 1, context, verify)
    if op == WIDEN and context == 0:
        return (model, retries, 1, verify)
    if op == VERIFY and verify == 0:
        return (model, retries, context, 1)
    return None


def expand():
    """Breadth first from the simplest policy; a policy grows by its own operator or a later one."""
    entries = [dict(policy=SEED, parent=-1, op=0)]
    k = 0
    while k < len(entries):
        for op in range(entries[k]["op"], 4):
            child = apply(entries[k]["policy"], op)
            if child is not None:
                entries.append(dict(policy=child, parent=k, op=op))
        k += 1
    # Every one of the 2 x 4 x 2 x 2 policies, once.
    assert sorted(e["policy"] for e in entries) == [(m, r, c, v) for m in range(2) for r in range(4) for c in range(2) for v in range(2)]
    assert len({digest(e["policy"]) for e in entries}) == len(entries)
    return entries


def tasks(seed, n):
    return [(4 * uniform(seed, 3 * i) - 2, int(uniform(seed, 3 * i + 1) + 0.3) > 0, int(uniform(seed, 3 * i + 2) + 0.4) > 0)
            for i in range(n)]


def chance(p, task):
    model, _, context, _ = p
    d, needs, _ = task
    z = SKILL[model] - d
    if needs:
        z += 1.5 if context else -1.0
    return 0.5 + z / (2 * (1 + abs(z)))


def trial(seed, p, task, i, t):
    model, retries, context, verify = p
    q = chance(p, task)
    attempts = 1 + retries if verify and task[2] else 1
    cost = 0.0
    for a in range(attempts):
        cost += PRICE[model] * (LONG if context else 1.0) + (CHECK if verify else 0.0)
        if int(uniform(seed, (i * 16 + t) * 4 + a) + q) > 0:
            return True, a + 1, cost
    return False, attempts, cost


def measure(seed, p, ts, trials):
    cells, spend = [], 0.0
    for i, task in enumerate(ts):
        solved, total = 0, 0.0
        for t in range(trials):
            ok, _, c = trial(seed, p, task, i, t)
            solved += ok
            total += c
            spend += c
        success, cost = solved / trials, total / trials
        cells.append((success, cost, success - LAMBDA * cost))
    return cells, spend


def plain(xs):
    t = 0.0
    for x in xs:
        t += x
    return t


def search(entries, ts, tiers=TIERS, keeps=KEEPS):
    alive = list(range(len(entries)))
    rounds, spend = [], 0.0
    for k, (n, trials, seed) in enumerate(tiers):
        cells = {}
        for e in alive:
            cells[e], s = measure(seed, entries[e]["policy"], ts[:n], trials)
            spend += s
        total = {e: plain(c[2] for c in cells[e]) for e in alive}
        order = sorted(alive, key=lambda e: (-total[e], e))
        r = dict(measured=len(alive), best=order[0], kept=0, frontier=0, spend=spend, cells=cells, order=order)
        rounds.append(r)
        if k == len(tiers) - 1:
            break
        kept = order[:keeps[k]]
        extra = []
        for i in range(n):
            top = max(cells[e][i][2] for e in alive)
            holders = [e for e in alive if cells[e][i][2] == top]
            if len(holders) == 1 and holders[0] not in kept and holders[0] not in extra:
                extra.append(holders[0])
        r["kept"], r["frontier"] = len(kept), len(extra)
        alive = sorted(kept + extra)
    return dict(winner=rounds[-1]["best"], rounds=rounds, spend=spend)


def experiment():
    entries = expand()
    dev = tasks(DEV_SEED, TASKS)
    s = search(entries, dev)
    last = s["rounds"][-1]
    w = s["winner"]
    # Every policy measured in full, for comparison only: what the search saved,
    # and per task whether another policy scores higher than the winner.
    full = [measure(TIERS[-1][2], e["policy"], dev, TIERS[-1][1]) for e in entries]
    full_total = [plain(c[2] for c in f[0]) for f in full]
    full_best = sorted(range(len(entries)), key=lambda e: (-full_total[e], e))[0]
    off, oracle = 0, 0.0
    for i in range(TASKS):
        top = max(f[0][i][2] for f in full)
        off += top > full[w][0][i][2]
        oracle += top
    # The winner is written down; now the held-out tasks are read.
    held = tasks(HELD_SEED, TASKS)
    base = [c[2] for c in measure(HELD_TRIAL_SEED, SEED, held, HELD_TRIALS)[0]]
    runs = [[c[2] for c in measure(HELD_TRIAL_SEED + r, entries[w]["policy"], held, HELD_TRIALS)[0]] for r in range(16)]
    f = ns.floor_of(runs, [0] * TASKS, 0, 1)
    gate = vd.decision(base, runs[0], list(range(TASKS)), [0] * TASKS, 0, f["floor"], 1.0, 7)
    return dict(entries=entries, dev=dev, search=s, off=off, oracle=oracle / TASKS, winner_total=plain(c[2] for c in last["cells"][w]) / TASKS,
                full_spend=plain(f[1] for f in full), full_best=full_best, held=held, base=plain(base) / TASKS,
                held_winner=plain(runs[0]) / TASKS, floor=f["floor"], gate=gate)


def tests():
    entries = expand()
    print("entries", len(entries), [(i, e["policy"], e["parent"], e["op"]) for i, e in enumerate(entries)])
    print("digest seed", digest(SEED).hex(), "digest last", digest(entries[-1]["policy"]).hex())
    dev = tasks(5, 6)
    print("tasks", [tuple(map(repr, t)) for t in dev])
    for p in ((0, 0, 0, 0), (1, 3, 1, 1), (0, 2, 0, 1)):
        cells, spend = measure(11, p, dev, 3)
        print(p, [tuple(map(repr, c)) for c in cells], repr(spend), "score", repr(plain(c[2] for c in cells)))
    ts = tasks(41, 8)
    for keep in (4, 1, 32):
        s = search(entries, ts, tiers=((4, 1, 21), (8, 2, 22)), keeps=(keep,))
        print("search keep", keep, s["winner"], repr(s["spend"]), [(r["measured"], r["best"], r["kept"], r["frontier"], repr(r["spend"]), r["order"][:6]) for r in s["rounds"]])
    s = search(entries, ts, tiers=((4, 1, 21),), keeps=())
    print("search", s["winner"], repr(s["spend"]), [(r["measured"], r["best"], r["kept"], r["frontier"], repr(r["spend"])) for r in s["rounds"]])


def hexes(d):
    return "{" + ",".join(map(str, d)) + "}"


def header(r):
    s = r["search"]
    lines = ["/* Generated by psionic/tests/search_oracle.py --header. Do not edit. */", "#include <stdint.h>",
             "typedef struct {int64_t model,retries,context,verify,parent,op;uint8_t digest[32];} sr_oracle_entry;",
             "typedef struct {int64_t measured,best,kept,frontier;double spend;} sr_oracle_round;",
             f"static const sr_oracle_entry sr_oracle_entries[{len(r['entries'])}] = {{"
             + ",".join(f"{{{e['policy'][0]},{e['policy'][1]},{e['policy'][2]},{e['policy'][3]},{e['parent']},{e['op']},{hexes(digest(e['policy']))}}}"
                        for e in r["entries"]) + "};",
             f"static const double sr_oracle_dev[{TASKS}][3] = {{{','.join(f'{{{t[0]!r},{int(t[1])},{int(t[2])}}}' for t in r['dev'])}}};",
             f"static const sr_oracle_round sr_oracle_rounds[3] = {{{','.join(f'{{{x['measured']},{x['best']},{x['kept']},{x['frontier']},{x['spend']!r}}}' for x in s['rounds'])}}};",
             f"static const int64_t sr_oracle_winner = {s['winner']};",
             f"static const double sr_oracle_spend = {s['spend']!r};",
             f"static const double sr_oracle_full_spend = {r['full_spend']!r};",
             f"static const int64_t sr_oracle_full_best = {r['full_best']};",
             f"static const int64_t sr_oracle_off = {r['off']};",
             f"static const double sr_oracle_oracle = {r['oracle']!r};",
             f"static const double sr_oracle_winner_total = {r['winner_total']!r};",
             f"static const double sr_oracle_base = {r['base']!r};",
             f"static const double sr_oracle_held_winner = {r['held_winner']!r};",
             f"static const double sr_oracle_floor = {r['floor']!r};",
             f"static const int64_t sr_oracle_gate[4] = {{{r['gate']['verdict'][0]},{r['gate']['verdict'][1]},{r['gate']['gain']['below']},{r['gate']['gain']['tasks']}}};",
             f"static const double sr_oracle_gate_gain = {r['gate']['gain']['gain']!r};"]
    print("\n".join(lines))


def show(r):
    s = r["search"]
    w = r["entries"][s["winner"]]
    print("winner", s["winner"], w["policy"], "spend", s["spend"], "full", r["full_spend"], "full best", r["full_best"])
    for x in s["rounds"]:
        print("  round", x["measured"], x["best"], x["kept"], x["frontier"], x["spend"], x["order"][:4])
    print("off frontier", r["off"], "oracle", r["oracle"], "winner", r["winner_total"])
    print("held base", r["base"], "winner", r["held_winner"], "floor", r["floor"], r["gate"]["verdict"], r["gate"]["gain"])


if __name__ == "__main__":
    if "--tests" in sys.argv:
        tests()
    else:
        result = experiment()
        header(result) if "--header" in sys.argv else show(result)
