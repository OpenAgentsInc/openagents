#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
"""Chapter 240's independent reference, written from the chapter's specification.

It shares no code with psionic/lib/env.coax. Tasks, solutions, checkers,
admission, the simulated agent, rollouts and their replay digests are written
again here; digests use hashlib over chapter 237's encoding (a length-prefixed
tag, little-endian u64 fields). `--tests` prints the values the Coax checks pin.
"""
import hashlib
import struct
import sys

MASK = (1 << 64) - 1
SORT, DEDUPE, PREFIX = range(3)
EXACT, LENGTH_ONLY, ANYTHING = range(3)
ADMITTED, EMPTY_PASSES, REFERENCE_FAILS, NEAR_MISS_PASSES = range(4)
ENV_SEED, ENVS, BUDGET = 181, 40, 3
AGENTS = ((1810, 0.35), (1811, 0.75))


def uniform(seed, counter):
    z = (seed + 0x9E3779B97F4A7C15 * (counter + 1)) & MASK
    z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & MASK
    z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & MASK
    return ((z ^ (z >> 31)) >> 11) / 9007199254740992.0


def u64(x):
    return struct.pack("<Q", x & MASK)


def text(s):
    b = s.encode()
    return u64(len(b)) + b


def ints(v):
    return u64(len(v)) + b"".join(u64(x) for x in v)


def tasks(seed, n):
    out = []
    for i in range(n):
        b = 64 * i
        kind = int(3 * uniform(seed, b))
        size = 4 + int(13 * uniform(seed, b + 1))
        out.append((kind, [int(10 * uniform(seed, b + 2 + j)) for j in range(size)]))
    return out


def solve(task):
    kind, v = task
    if kind == SORT:
        return sorted(v)
    if kind == DEDUPE:
        out = []
        for x in v:
            if x not in out:
                out.append(x)
        return out
    out, t = [], 0
    for x in v:
        t += x
        out.append(t)
    return out


def check(checker, task, work):
    want = solve(task)
    if checker == ANYTHING:
        return True
    if checker == LENGTH_ONLY:
        return len(work) == len(want)
    return work == want


def env_digest(env):
    task, checker, attempts, reference = env
    return hashlib.sha256(text("psionic.env.v1") + bytes([task[0]]) + ints(task[1]) + bytes([checker]) + u64(attempts)
                          + ints(reference)).digest()


def admit(env):
    task, checker, _, reference = env
    if check(checker, task, []):
        return EMPTY_PASSES
    if not check(checker, task, reference):
        return REFERENCE_FAILS
    near = list(reference)
    near[-1] += 1
    if check(checker, task, near):
        return NEAR_MISS_PASSES
    return ADMITTED


def attempt(seed, skill, task, r, a):
    b = (r * 8 + a) * 4
    want = solve(task)
    if int(uniform(seed, b) + skill) > 0:
        return want
    mistake = int(3 * uniform(seed, b + 1))
    if mistake == 0:
        out = list(want)
        out[int(len(out) * uniform(seed, b + 2))] += 1
        return out
    if mistake == 1:
        return want[:-1]
    return list(task[1])


def rollout(env, seed, skill, r):
    task, checker, attempts, _ = env
    h = hashlib.sha256(text("psionic.rollout.v1") + env_digest(env) + u64(seed) + struct.pack("<d", skill) + u64(r)).digest()
    for a in range(attempts):
        work = attempt(seed, skill, task, r, a)
        ok = check(checker, task, work)
        h = hashlib.sha256(h + ints(work) + bytes([ok])).digest()
        if ok:
            return True, a + 1, h
    return False, attempts, h


def broken(task):
    want = solve(task)
    wrong = list(want)
    wrong[0] += 1
    return [(task, ANYTHING, BUDGET, want), (task, LENGTH_ONLY, BUDGET, want), (task, EXACT, BUDGET, wrong)]


def experiment():
    ts = tasks(ENV_SEED, ENVS)
    envs = [(t, EXACT, BUDGET, solve(t)) for t in ts]
    admitted = [admit(e) for e in envs]
    refused = [admit(e) for e in broken(ts[0])]
    results = []
    for seed, skill in AGENTS:
        rs = [rollout(e, seed, skill, r) for r, e in enumerate(envs)]
        results.append(dict(solved=sum(x[0] for x in rs), first=sum(x[0] and x[1] == 1 for x in rs),
                            attempts=sum(x[1] for x in rs), rs=rs))
    return dict(tasks=ts, envs=envs, admitted=admitted, refused=refused, results=results)


def show(r):
    print("kinds", [sum(t[0] == k for t in r["tasks"]) for k in range(3)], "admitted", r["admitted"].count(ADMITTED), "refused", r["refused"])
    for (seed, skill), x in zip(AGENTS, r["results"]):
        print("agent", skill, "solved", x["solved"], "first", x["first"], "attempts", x["attempts"], x["rs"][0][2].hex()[:16])
    print("env0", env_digest(r["envs"][0]).hex()[:16])


def tests():
    ts = tasks(5, 3)
    print("tasks", ts)
    for t in ts:
        print("solve", solve(t))
    e = (ts[0], EXACT, 2, solve(ts[0]))
    print("env digest", env_digest(e).hex())
    for r in range(4):
        print("rollout", r, [attempt(9, 0.5, ts[0], r, a) for a in range(2)], rollout(e, 9, 0.5, r)[0:2], rollout(e, 9, 0.5, r)[2].hex())


if __name__ == "__main__":
    tests() if "--tests" in sys.argv else show(experiment())
