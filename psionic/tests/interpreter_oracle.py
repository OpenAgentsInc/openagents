#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
"""Chapter 229's independent reference, written from the chapter's rules.

It shares no code with psionic/lib/interpreter-weights.coax. It implements
the five-instruction register machine directly, regenerates the example's
seeded programs from the same linear congruential generator, runs each from
the example's start state, and derives the triangle line and the seeded
summary. From the universal graph's construction it derives the node count
for every length. Each derived line must appear in the example's output.
"""
import pathlib
import sys

ADDI, ADDR, JNZ, HALT = range(4)
VALUE_MAX = 2 ** 24


def nxt(x):
    return (x * 1103515245 + 12345) % 2147483648


def random_program(seed, n):
    p, x = [], seed
    for _ in range(n - 1):
        x = nxt(x)
        op = (x >> 8) % 3
        x = nxt(x)
        arg = (x >> 8) % 7 - 3
        if op == JNZ:
            arg = (x >> 8) % n
        p.append((op, arg))
    p.append((HALT, (seed >> 4) % 5))
    return p


def run(p, ip, acc, r, budget):
    """Returns (status, steps, ip, acc, r). A step that leaves the ip in place halts."""
    for steps in range(budget):
        op, arg = p[ip]
        nip, nacc, nr = ip + 1, acc, r
        if op == ADDI:
            nacc = acc + arg
        elif op == ADDR:
            nr = r + acc
        elif op == JNZ and acc != 0:
            nip = arg
        elif op == HALT:
            nip = ip
        if abs(nacc) > VALUE_MAX or abs(nr) > VALUE_MAX:
            return "range", steps, ip, acc, r
        if nip == ip:
            return "ok", steps, ip, acc, r
        ip, acc, r = nip, nacc, nr
    return "budget", budget, ip, acc, r


def nodes(slots):
    # inputs, constants 0..5, eq0(acc), the slot tests, 14 per slot, the three chains
    return 3 + 2 * slots + 6 + 1 + 2 * (slots - 1) + 14 * slots + 3 * (slots - 1)


def main():
    root = pathlib.Path(__file__).resolve().parents[2]
    text = (root / "psionic/fixtures/interpreter-weights.out").read_text()
    lines = []
    lines.append("universal interpreter, any program of 1 to 5 instructions: %d nodes" % nodes(5))
    tri = [(ADDR, 0), (ADDI, -1), (JNZ, 0), (HALT, 0)]
    _, steps, _, _, r = run(tri, 0, 100, 0, 1000)
    assert r == 100 * 101 // 2
    lines.append("triangle from 100: native %d steps, r %d; universal block %d steps, r %d; own block %d steps, r %d"
                 % (steps, r, steps, r, steps, r))
    total = halted = budget = check = 0
    for seed in range(200):
        k = 1 + seed % 5
        status, steps, ip, acc, r = run(random_program(seed, k), 0, (seed * 7) % 9 - 4, seed % 11, 40)
        total += steps
        halted += status == "ok"
        budget += status == "budget"
        check = (check * 31 + steps * 1000003 + ip * 101 + acc * 7 + r) % 1000000007
    lines.append("200 seeded programs agree three ways: %d steps, %d halted, %d out of budget; checksum %d"
                 % (total, halted, budget, check))
    for line in lines:
        assert line in text, line
    print("interpreter oracle:\n" + "\n".join(lines))
    for seed in (0, 7, 12):
        print("seed", seed, random_program(seed, 5))
    return 0


if __name__ == "__main__":
    sys.exit(main())
