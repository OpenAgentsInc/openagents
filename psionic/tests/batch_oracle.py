#!/usr/bin/env python3
"""Chapter 25's independent oracle: which batches one pass admits, where each
question's rows and logits sit, and how large the batch workspace is, derived
from the written rule and the two models' shapes rather than from plan.coax.
Checks the transcript of psionic/examples/many-questions.coax.
Usage: python3 -I psionic/tests/batch_oracle.py psionic/fixtures/many-questions.out"""
import re
import sys

MAX_TOKENS, MMA, BATCH = 512, 16, 8
MODELS = {  # width, feed-forward width, key heads, value heads, heads, kv heads
    0: dict(d=4096, f=12288, kh=16, vh=32, n=16, kvh=4),
    1: dict(d=5120, f=17408, kh=16, vh=48, n=24, kvh=4),
}
VOCAB = 248320


def admits(metal, start, lengths):
    if metal or start < 1 or not 1 <= len(lengths) <= BATCH:
        return False
    if any(t < 1 or start + t > MAX_TOKENS for t in lengths) or sum(lengths) > MAX_TOKENS:
        return False
    # One projection path for every question: all on the tensor cores, or none.
    short = [t < MMA for t in lengths]
    return not any(short) or sum(lengths) < MMA


def align(n):
    return -(-n // 256) * 256


def layout(m, t, keys):
    """Region sizes in bytes, in order (chapter 30's workspace)."""
    d, f, kh, vh, n, kvh = (m[k] for k in ("d", "f", "kh", "vh", "n", "kvh"))
    channels, values, queries, attended, kv = 256 * kh + 128 * vh, 128 * vh, 512 * n, 256 * n, 256 * kvh
    pad = -(-t // 88) * 88
    sizes = [
        ("x0", 4 * t * d), ("x1", 4 * t * d), ("norm", 4 * t * d), ("residual", 4 * t * d),
        ("qkv", 4 * t * max(channels, queries)), ("z", 4 * t * values), ("a", 4 * t * vh), ("b", 4 * t * vh),
        ("k", 4 * t * kv), ("v", 4 * t * kv), ("conv", 4 * t * channels),
        ("qn", 4 * t * 128 * kh), ("kn", 4 * t * 128 * kh), ("beta", 4 * t * vh), ("decay", 4 * t * vh),
        ("draft", 4 * t * values), ("mix", 4 * t * max(values, attended)), ("aq", 4 * t * attended), ("ak", 4 * t * kv),
        ("scores", 4 * t * n * keys), ("stats", 8 * t * n * 3), ("probs", 8 * t * n * keys), ("squares", 8 * t * 16),
        ("partial", 4 * 8 * f * t), ("hi", 4 * pad * f // 2), ("lo", 4 * pad * f // 2), ("out", 4 * t * d),
        ("gate", 4 * t * f), ("up", 4 * t * f), ("product", 4 * t * f), ("down", 4 * t * d),
        ("rope", 4 * t * 64), ("final", 4 * d), ("logits", 4 * VOCAB), ("lanes", 4 * 16 * VOCAB),
    ]
    at, offsets = 256, {}
    for name, size in sizes:
        offsets[name] = at
        at += align(size)
    offsets["total"] = at
    return offsets


def batch(kind, start, lengths):
    m = MODELS[kind]
    p = layout(m, sum(lengths), start + max(lengths))
    finals = p["total"]
    logits_base = finals + (len(lengths) - 1) * align(4 * m["d"])
    logits = [p["logits"]] + [logits_base + i * align(4 * VOCAB) for i in range(len(lengths) - 1)]
    return logits_base + (len(lengths) - 1) * align(4 * VOCAB), logits


def main():
    text = open(sys.argv[1]).read().splitlines()
    lab = [37, 38, 38, 38, 37]
    word = lambda b: "true" if b else "false"
    want = "admitted: lab %s, on Metal %s, 37 and 8 tokens %s, 7 and 8 %s, 8 and 8 %s" % tuple(
        word(x) for x in (admits(False, 49, lab), admits(True, 49, lab), admits(False, 49, [37, 8]), admits(False, 49, [7, 8]), admits(False, 49, [8, 8])))
    assert text[0] == want, (text[0], want)
    firsts = [sum(lab[:i]) for i in range(5)]
    assert text[1] == "batch tokens: %d; first rows: %s" % (sum(lab), " ".join(map(str, firsts))), text[1]
    for kind, line in ((0, text[2]), (1, text[4])):
        total, logits = batch(kind, 49, lab)
        assert line == "model %d: batch workspace %d bytes; logits at %s" % (kind, total, " ".join(map(str, logits))), (line, total, logits)
    # A batched pass copies what one suffix pass copies: a fifth of five passes.
    for line in (text[3], text[5]):
        seq, one = map(int, re.search(r"(\d+) bytes copied; one batched pass \d+ operations, (\d+) bytes", line).groups())
        assert seq == 5 * one and one > 0, line
    # The rule itself, exhaustively over two questions and small starts.
    for start in range(0, 4):
        for a in range(0, 20):
            for b in range(0, 20):
                ok = admits(False, start, [a, b])
                assert ok == (start >= 1 and a >= 1 and b >= 1 and ((a >= 16 and b >= 16) or a + b < 16))
    print("batch oracle: admission, rows, workspace and logits offsets agree for Elephant and Mammoth")


if __name__ == "__main__":
    main()
