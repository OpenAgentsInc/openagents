# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
"""Independent oracle for chapter 222 (shadow, then promote).

Rebuilds psionic/fixtures/shadow.out line for line with the standard library:
SplitMix64 draws, the simulated models, SHA-256 identities from hashlib, the
shadow statistics, and the activation rules. With --tests it prints the values
psionic/lib/shadow.coax pins by hand.
"""
import hashlib
import math
import pathlib
import struct
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
MASK = (1 << 64) - 1
GOLDEN = 0x9E3779B97F4A7C15
WRONG = 1 << 20
JOIN = 0.8
NAME = 32
BLOB = 56


def bits(seed, counter):
    z = (seed + GOLDEN * (counter + 1)) & MASK
    z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & MASK
    z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & MASK
    return z ^ (z >> 31)


def uniform(seed, counter):
    return (bits(seed, counter) >> 11) / 2.0**53


def truth(world, i):
    return int(3 * uniform(world, 3 * i))


def joined(world, i):
    return uniform(world, 3 * i + 2) + JOIN >= 1


def answer(m, world, i):
    name, skill, weight, seed = m
    t = truth(world, i)
    score = (1 - weight) * uniform(world, 3 * i + 1) + weight * uniform(seed, i)
    if 1 + skill - score >= 1:
        return t
    return (t + 1 + int(2 * uniform(seed, WRONG + i))) % 3


def blob(m):
    name, skill, weight, seed = m
    return name.encode().ljust(NAME, b"\0") + struct.pack("<ddQ", skill, weight, seed)


def digest(b):
    return hashlib.sha256(b).digest()


def records(world, a, c, n):
    return [(answer(a, world, i), answer(c, world, i), truth(world, i) if joined(world, i) else -1) for i in range(n)]


def report(active, candidate, recs, limits):
    disagreement, margin, need = limits
    r = dict(active=active, candidate=candidate, requests=len(recs), joined=0, disagree=0, aw=0, cw=0, verdict="wait")
    for a, c, o in recs:
        r["disagree"] += a != c
        if o == -1:
            continue
        r["joined"] += 1
        r["aw"] += a != o
        r["cw"] += c != o
    n = len(recs)
    r["rate"] = r["disagree"] / n
    r["rate_bound"] = r["rate"] + 2 * math.sqrt(r["rate"] * (1 - r["rate"]) / n)
    if r["joined"] < need:
        return r
    j = r["joined"]
    s = sq = 0.0
    for a, c, o in recs:
        if o == -1:
            continue
        d = float(c != o) - float(a != o)
        s += d
        sq += d * d
    r["difference"] = s / j
    r["difference_bound"] = r["difference"] + 2 * math.sqrt((sq - s * s / j) / (j - 1) / j)
    if r["rate_bound"] > disagreement:
        r["verdict"] = "hold: disagreement"
    elif r["difference_bound"] > margin:
        r["verdict"] = "hold: regression"
    else:
        r["verdict"] = "promote"
    return r


VERDICT_CODE = {"wait": 0, "promote": 1, "hold: disagreement": 2, "hold: regression": 3}


def report_digest(r):
    counts = struct.pack("<5q", r["requests"], r["joined"], r["disagree"], r["aw"], r["cw"])
    return digest(r["active"] + r["candidate"] + counts + bytes([VERDICT_CODE[r["verdict"]]]))


def activate(now, registry, expected, b, rep, approval):
    """now = (digest, version, name); returns (status, new now)."""
    if expected != now[1]:
        return "version", now
    d = digest(b)
    names = [name for name, dd in registry if dd == d]
    if not names or d != approval[0]:
        return "digest", now
    if rep["verdict"] != "promote" or rep["candidate"] != d or rep["active"] != now[0] or report_digest(rep) != approval[1]:
        return "approval", now
    return "ok", (d, now[1] + 1, names[-1])


def r4(v):
    return math.floor(v * 10000 + 0.5) / 10000


def short(d):
    return d[:6].hex()


def example():
    world, limits = 164, (0.12, 0.005, 1000)
    v3 = ("router-v3", 0.85, 0.2, 31)
    models = [("router-v4", 0.89, 0.2, 41), ("router-small", 0.83, 0.2, 51), ("router-other", 0.9, 0.9, 61)]
    registry = [(m[0], digest(blob(m))) for m in [v3] + models]
    active = (digest(blob(v3)), 1, "router-v3")
    out = [f"active: router-v3 ({short(active[0])}), version 1; {len(registry)} models registered",
           "shadowed beside router-v3 on 4000 requests:"]
    reports = []
    for m in models:
        recs = records(world, v3, m, 4000)
        r = report(active[0], digest(blob(m)), recs, limits)
        reports.append(r)
        out.append(f"  {m[0]} ({short(r['candidate'])}): disagrees on {r['disagree']} of {r['requests']} requests, "
                   f"{r4(r['rate'])} + 2 SE = {r4(r['rate_bound'])} (limit 0.12)")
        out.append(f"    errors on {r['joined']} joined: active {r['aw']}, candidate {r['cw']}; extra error "
                   f"{r4(r['difference'])} + 2 SE = {r4(r['difference_bound'])} (margin 0.005) -> {r['verdict']}")
    early = report(active[0], reports[2]["candidate"], recs[:1000], limits)
    out.append(f"  router-other after its first 1000 requests: {early['joined']} joined -> {early['verdict']}")
    ok = (reports[0]["candidate"], report_digest(reports[0]))
    out.append(f"approval: router-v4 {short(ok[0])} with report {short(ok[1])}")
    status, nxt = activate(active, registry, 1, blob(models[0]), reports[0], ok)
    assert status == "ok"
    out.append(f"activate router-v4 expecting version 1: active is now {nxt[2]} ({short(nxt[0])}), version {nxt[1]}")
    out.append("refused, each leaving the active model unchanged:")
    forged = (reports[1]["candidate"], report_digest(reports[1]))
    out.append(f"  router-small, whose report holds, with an approval of it: {activate(nxt, registry, 2, blob(models[1]), reports[1], forged)[0]}")
    out.append(f"  router-v4 again, by a caller who still expects version 1: {activate(nxt, registry, 1, blob(models[0]), reports[0], ok)[0]}")
    changed = bytearray(blob(models[0]))
    changed[NAME] ^= 1
    out.append(f"  router-v4's bytes with one bit changed after approval: {activate(active, registry, 1, bytes(changed), reports[0], ok)[0]}")
    out.append(f"  router-v4's report again, now that router-v4 itself is active: {activate(nxt, registry, 2, blob(models[0]), reports[0], ok)[0]}")
    edited = ("router-v4", 0.9, 0.2, 41)
    clash = any(name == edited[0] and d != digest(blob(edited)) for name, d in registry)
    out.append(f"  registering router-v4 again with other bytes: {'immutable' if clash else 'ok'}")
    return out


def tests():
    m = ("a", 0.8, 0.2, 1)
    print("blob", blob(m).hex(), "digest", digest(blob(m)).hex())
    print("draws", bits(1, 0), uniform(7, 3), [truth(164, i) for i in range(8)], [joined(164, i) for i in range(8)])
    print("answers", [answer(("x", 0.85, 0.2, 31), 164, i) for i in range(12)], [answer(("y", 0.3, 0.5, 9), 164, i) for i in range(12)])


if __name__ == "__main__":
    if "--tests" in sys.argv:
        tests()
        sys.exit()
    out = example()
    fixture = (ROOT / "psionic/fixtures/shadow.out").read_text().splitlines()
    for n, (a, b) in enumerate(zip(out, fixture)):
        assert a == b, f"line {n + 1}:\n  oracle  {a}\n  fixture {b}"
    assert len(out) == len(fixture), (len(out), len(fixture))
    print(f"shadow oracle: the example's {len(out)} lines are rebuilt line for line")
