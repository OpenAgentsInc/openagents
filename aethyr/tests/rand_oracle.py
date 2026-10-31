#!/usr/bin/env python3
"""The oracle for aethyr/rand.coax: lowbias32 and the counter-based draw, written
from the published hash, not from the Coax. Prints the values rand.coax pins.
  python3 aethyr/tests/rand_oracle.py
"""
M = 0xFFFFFFFF


def mix(x):
    x &= M
    x ^= x >> 16
    x = (x * 0x7FEB352D) & M
    x ^= x >> 15
    x = (x * 0x846CA68B) & M
    x ^= x >> 16
    return x


def halves(v):
    v &= (1 << 64) - 1  # two's complement, as Coax's int
    return v & M, (v >> 32) & M


def u32(seed, stream, counter):
    s0, s1 = halves(seed)
    t0, t1 = halves(stream)
    c0, c1 = halves(counter)
    h = mix(s0 ^ 0x9E3779B9)
    for part in (s1, t0, t1, c0, c1):
        h = mix(h ^ part)
    return h


def unit(seed, stream, counter):
    return (u32(seed, stream, counter) >> 8) / 16777216.0


def below(seed, stream, counter, n):
    return 0 if n < 1 else (u32(seed, stream, counter) * n) >> 32


if __name__ == "__main__":
    for x in (0, 1, M):
        print(f"rand_mix({x}) == {mix(x)}")
    for args in ((0, 0, 0), (2026, 1, 0), (2026, 1, 1), (2026, 2, 0), (-1, 7, 1 << 32), (1 << 40, 1 << 33, 5)):
        print(f"rand_u32{args} == {u32(*args)}")
    print(f"rand_below(2026, 3, 9, 6) == {below(2026, 3, 9, 6)}")
    print(f"rand_unit(2026, 1, 0) == {unit(2026, 1, 0)!r}")
