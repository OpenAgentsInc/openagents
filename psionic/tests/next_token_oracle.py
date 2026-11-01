"""Independent reference values for chapter 53 (psionic/lib/next-token.coax).

Tooling only. Recomputes the counter RNG words, the uniforms and the
truncated distributions that the unit's #test blocks hard-code, from the
written rules and nothing else: SplitMix64 on seed + golden * (counter + 1),
the high 53 bits as a uniform, logits sorted by value descending then ID
ascending, weights exp((l - max) / T) summed in that order, top-k then top-p.
"""
import math

M = (1 << 64) - 1
GOLDEN = 0x9E3779B97F4A7C15


def bits(seed, counter):
    z = (seed + GOLDEN * (counter + 1)) & M
    z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & M
    z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & M
    return z ^ (z >> 31)


def uniform(seed, counter):
    return (bits(seed, counter) >> 11) / 9007199254740992.0


def distribution(logits, mask, temperature, top_k, top_p):
    allowed = [i for i in range(len(logits)) if mask is None or mask[i]]
    order = sorted(allowed, key=lambda i: (-logits[i], i))
    p = [0.0] * len(logits)
    if temperature == 0:
        p[order[0]] = 1.0
        return p
    kept = order[:top_k] if top_k else order
    top = logits[order[0]]
    w = {i: math.exp((logits[i] - top) / temperature) for i in kept}
    total = 0.0
    for i in kept:
        total += w[i]
    mass, take = 0.0, 0
    while mass < top_p * total:
        mass += w[kept[take]]
        take += 1
    for i in kept[:take]:
        p[i] = w[i] / mass
    return p


def main():
    for seed, counter in [(0, 0), (45001, 0), (45001, 1), (45001, 2), (53, 7)]:
        print(f"bits({seed},{counter}) = 0x{bits(seed, counter):016x}  uniform = {uniform(seed, counter)!r}")
    logits = [2.0, 1.0, 2.0, 0.0, -1.0]
    for t, k, p in [(1, 0, 1), (0.5, 0, 1), (1, 2, 1), (1, 1, 1), (1, 0, 0.5), (1, 0, 0.3), (2, 3, 0.9), (0, 0, 1)]:
        print(f"T={t} k={k} p={p}: {[repr(x) for x in distribution(logits, None, t, k, p)]}")
    mask = [0, 1, 1, 1, 0]
    print("masked T=1:", [repr(x) for x in distribution(logits, mask, 1, 0, 1)])


if __name__ == "__main__":
    main()
