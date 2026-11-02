#!/usr/bin/env python3
"""Independent definition of chapter 113's tiny decoder (pure Python, binary64).

Written from the chapter's equations, not from the Coax code: it rebuilds the
parameter tree by name, runs every position with full causal attention over
the whole prefix, and prints the logits and mean next-token loss that
psionic/lib/transformer/decoder.coax must reproduce.
"""
import argparse
import math

V, D, H, K, E, F, L = 5, 4, 2, 1, 2, 3, 2
TOKENS = [1, 3, 0, 2]
EPS, THETA = 1e-6, 10000.0


def count():
    return 2 * V * D + L * (2 * D + H * E * D + 2 * K * E * D + D * H * E + 2 * F * D) + D


def use_split():
    """Chapter 183's splittable shape: 4 heads over 2 key-value heads, 8 FFN columns."""
    global V, D, H, K, E, F, L, TOKENS
    V, D, H, K, E, F, L = 6, 8, 4, 2, 2, 8, 2
    TOKENS = [1, 3, 0, 2, 5, 4]


def parameters():
    n = count()
    flat = [0.5 * math.sin(0.7 * i + 0.3) for i in range(n)]
    at = 0

    def take(rows, cols=None):
        nonlocal at
        n = rows * (cols or 1)
        block = flat[at:at + n]
        at += n
        return block if cols is None else [block[r * cols:(r + 1) * cols] for r in range(rows)]

    embed = take(V, D)
    layers = []
    for _ in range(L):
        layers.append(dict(norm1=take(D), wq=take(H * E, D), wk=take(K * E, D), wv=take(K * E, D),
                           wo=take(D, H * E), norm2=take(D), w1=take(F, D), w2=take(D, F)))
    final, unembed = take(D), take(V, D)
    assert at == n
    return embed, layers, final, unembed


def mv(w, x):
    return [sum(a * b for a, b in zip(row, x)) for row in w]


def norm(x, g):
    r = 1 / math.sqrt(sum(v * v for v in x) / len(x) + EPS)
    return [gi * xi * r for gi, xi in zip(g, x)]


def rope(v, heads, pos):
    out = list(v)
    for h in range(heads):
        for i in range(E // 2):
            ang = pos * THETA ** (-2 * i / E)
            a, b = v[h * E + 2 * i], v[h * E + 2 * i + 1]
            out[h * E + 2 * i] = a * math.cos(ang) - b * math.sin(ang)
            out[h * E + 2 * i + 1] = a * math.sin(ang) + b * math.cos(ang)
    return out


def logits(tokens):
    embed, layers, final, unembed = parameters()
    xs = [list(embed[t]) for t in tokens]
    for layer in layers:
        normed = [norm(x, layer["norm1"]) for x in xs]
        qs = [rope(mv(layer["wq"], x), H, t) for t, x in enumerate(normed)]
        ks = [rope(mv(layer["wk"], x), K, t) for t, x in enumerate(normed)]
        vs = [mv(layer["wv"], x) for x in normed]
        out = []
        for t in range(len(tokens)):
            context = []
            for h in range(H):
                g = h // (H // K)
                q = qs[t][h * E:(h + 1) * E]
                scores = [sum(a * b for a, b in zip(q, ks[j][g * E:(g + 1) * E])) / math.sqrt(E) for j in range(t + 1)]
                top = max(scores)
                weights = [math.exp(s - top) for s in scores]
                total = sum(weights)
                context += [sum(weights[j] / total * vs[j][g * E + x] for j in range(t + 1)) for x in range(E)]
            mid = [a + b for a, b in zip(xs[t], mv(layer["wo"], context))]
            hidden = [max(z, 0.0) ** 2 for z in mv(layer["w1"], norm(mid, layer["norm2"]))]
            out.append([a + b for a, b in zip(mid, mv(layer["w2"], hidden))])
        xs = out
    return [mv(unembed, norm(x, final)) for x in xs]


def loss(z, tokens):
    terms = []
    for t in range(len(tokens) - 1):
        top = max(z[t])
        terms.append(top + math.log(sum(math.exp(v - top) for v in z[t])) - z[t][tokens[t + 1]])
    return sum(terms) / len(terms)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--coax", action="store_true", help="print the Coax constants")
    parser.add_argument("--header", action="store_true", help="print the native harness goldens")
    parser.add_argument("--split", action="store_true", help="print chapter 183's Coax constants for its splittable shape")
    args = parser.parse_args()
    if args.split:
        use_split()
        z = logits(TOKENS)
        flat = [v for row in z for v in row]
        print("TP_ORACLE_LOGITS :: [%d]f64.[%s];" % (len(flat), ",".join(repr(v) for v in flat)))
        return
    z = logits(TOKENS)
    flat = [v for row in z for v in row]
    if args.header:
        print("static const double DECODER_LOSS = %s;" % repr(loss(z, TOKENS)))
        print("static const double DECODER_LOGITS[20] = {%s};" % ",".join(repr(v) for v in flat))
        return
    if args.coax:
        print("TD_ORACLE_LOSS :: %s;" % repr(loss(z, TOKENS)))
        print("TD_ORACLE_LOGITS :: [20]f64.[%s];" % ",".join(repr(v) for v in flat))
        return
    for t in range(1, len(TOKENS)):
        assert logits(TOKENS[:t]) == z[:t], "a prefix changed an earlier logit"
    print("decoder oracle: %d logits, loss %r, prefixes causal" % (len(flat), loss(z, TOKENS)))


if __name__ == "__main__":
    main()
