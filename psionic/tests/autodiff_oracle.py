#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
"""Independent oracle for psionic/examples/autodiff-mlp.coax (chapter 71b).

Standard library only. It does not read the Coax sources. It rebuilds the
example's output line for line from its own model of the network:
- the tape: which nodes need a gradient, which values are saved, and where
  first-fit placement puts every value, staging buffer and gradient;
- the forward pass in f32 with chapter 26a's operation order;
- the backward pass written out by hand for this network, accumulated in f64
  in the order chapter 71b documents, with the squared ReLU's VJP in f32;
- the finite-difference check, gradient descent and the predictions.
"""
import math
from decimal import Decimal
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / "psionic/fixtures/autodiff-mlp.out"


def f32(x):
    return struct.unpack("<f", struct.pack("<f", x))[0]


def r6(v):
    return math.floor(v * 1e6 + 0.5) / 1e6


def num(v):
    """Coax prints an f64 in its shortest digits, always positional."""
    s = repr(v)
    return format(Decimal(s), "f") if "e" in s else s


# The program: (name, op, a, b, shape, extra). Inputs carry their base.
INPUTS = {0: ((8, 2), 0), 1: ((2, 8), 16), 2: ((8,), 32), 3: ((8, 1), 40), 4: ((1,), 48), 5: ((8, 1), 49), 6: ((1,), 57)}
NODES = {
    7: ("matmul", 0, 1, (8, 8)),
    8: ("add", 7, 2, (8, 8)),
    9: ("registered", 8, 8, (8, 8)),
    10: ("matmul", 9, 3, (8, 1)),
    11: ("add", 10, 4, (8, 1)),
    12: ("multiply", 5, 6, (8, 1)),
    13: ("add", 11, 12, (8, 1)),
    14: ("multiply", 13, 13, (8, 1)),
    15: ("sum", 14, None, (1,)),
    16: ("sum", 15, None, ()),
}
PARAMS = [1, 2, 3, 4]
NAMES = ["W1", "b1", "W2", "b2"]
LOSS = 16


def size(shape):
    n = 1
    for d in shape:
        n *= d
    return n


def shape_of(i):
    return INPUTS[i][0] if i in INPUTS else NODES[i][3]


def tape(grad):
    """Steps, value and gradient lifetimes, and first-fit placement."""
    wants = {i: (i in PARAMS and grad) for i in INPUTS}
    for i, (op, a, b, _) in NODES.items():
        wants[i] = wants[a] or (op not in ("registered", "sum", "relu") and b is not None and wants[b])
    reaches = {LOSS}
    for i in sorted(NODES, reverse=True):
        if i in reaches:
            op, a, b, _ = NODES[i]
            reaches.add(a)
            if b is not None:
                reaches.add(b)
    forward = sorted(NODES)
    backward = [i for i in sorted(NODES, reverse=True) if wants[i] and i in reaches]
    steps = [(i, False) for i in forward] + [(i, True) for i in backward]
    count = len(steps)
    born = {i: s for s, (i, back) in enumerate(steps) if not back}
    dies = dict(born)
    saved = set()
    for s, (i, back) in enumerate(steps):
        op, a, b, _ = NODES[i]
        if not back:
            reads = [a] + ([b] if b is not None else [])
        elif op == "registered":
            reads = [a]
        elif op in ("matmul", "multiply"):
            reads = ([a] if wants[b] else []) + ([b] if wants[a] else [])
        else:
            reads = []
        for x in reads:
            if x in INPUTS:
                continue
            if back:
                saved.add(x)
            dies[x] = s
    dies[LOSS] = count
    blocks = []

    def place(n, start, end):
        best, moved = 0, True
        while moved:
            moved = False
            for at, s0, s1, sz in blocks:
                if s1 < start or s0 > end:
                    continue
                if at < best + n and best < at + sz:
                    best, moved = at + sz, True
        blocks.append((best, start, end, n))
        return best

    kept = 0
    for s, (i, back) in enumerate(steps):
        n = size(shape_of(i))
        if not back:
            kept += n
            place(n, born[i], dies[i])
        elif NODES[i][0] == "registered":
            place(n, s, s)  # the output's gradient, staged in f32
            place(size(shape_of(NODES[i][1])), s, s)  # the operand's gradient
    values = max(at + sz for at, _, _, sz in blocks)
    value_blocks = len(blocks)
    gborn, gdies = {}, {}
    if backward:
        gborn[LOSS] = count - len(backward)
    for s, (i, back) in enumerate(steps):
        if not back:
            continue
        gdies[i] = s
        op, a, b, _ = NODES[i]
        operands = [a] if op in ("registered", "sum") else [a, b]
        for x in operands:
            if not wants[x]:
                continue
            gborn.setdefault(x, s)
            gdies[x] = count if x in INPUTS else s
    blocks = []
    grads = 0
    for i in sorted(gborn):
        n = size(shape_of(i))
        at = place(n, gborn[i], gdies[i])
        grads = max(grads, at + n)
    return {"forward": len(forward), "backward": len(backward), "values": values, "value_blocks": value_blocks, "kept": kept,
            "grads": grads, "grad_blocks": len(blocks), "saved": sorted(saved)}


def matmul(a, b, m, k, n):
    out = []
    for r in range(m):
        for c in range(n):
            v = 0.0
            for j in range(k):
                v = f32(v + f32(a[r * k + j] * b[j * n + c]))
            out.append(v)
    return out


def forward(inp, upto=LOSS):
    x = inp[0:16]
    w1, b1, w2, b2 = inp[16:32], inp[32:40], inp[40:48], inp[48]
    t, neg = inp[49:57], inp[57]
    v = {}
    v[7] = matmul(x, w1, 8, 2, 8)
    v[8] = [f32(v[7][j] + b1[j % 8]) for j in range(64)]
    v[9] = [f32(0.5 * f32(max(z, 0.0) * max(z, 0.0))) if z > 0 else 0.0 for z in v[8]]
    v[10] = matmul(v[9], w2, 8, 8, 1)
    v[11] = [f32(v[10][j] + b2) for j in range(8)]
    if upto == 11:
        return v
    v[12] = [f32(t[j] * neg) for j in range(8)]
    v[13] = [f32(v[11][j] + v[12][j]) for j in range(8)]
    v[14] = [f32(d * d) for d in v[13]]
    s = 0.0
    for e in v[14]:
        s = f32(s + e)
    v[15] = [s]
    v[16] = [f32(0.0 + s)]
    return v


def backward(inp, v):
    """Reverse mode for this network, one rule per node, as the tape orders it."""
    x, w2 = inp[0:16], inp[40:48]
    g = {}
    g[16] = [1.0]
    g[15] = [0.0]
    g[15][0] += g[16][0]
    g[14] = [0.0] * 8
    for k in range(8):
        g[14][k] += g[15][0]
    g[13] = [0.0] * 8
    for j in range(8):
        g[13][j] += g[14][j] * v[13][j]
        g[13][j] += g[14][j] * v[13][j]
    g[11] = [0.0] * 8
    for j in range(8):
        g[11][j] += g[13][j]
    g[10] = [0.0] * 8
    gb2 = [0.0]
    for j in range(8):
        g[10][j] += g[11][j]
        gb2[0] += g[11][j]
    g[9] = [0.0] * 64
    gw2 = [0.0] * 8
    for r in range(8):
        dy = g[10][r]
        for k in range(8):
            g[9][r * 8 + k] += dy * w2[k]
            gw2[k] += dy * v[9][r * 8 + k]
    g[8] = [0.0] * 64
    for j in range(64):
        z = v[8][j]
        positive = z if z > 0 else 0.0
        g[8][j] += f32(f32(1.0 * positive) * f32(g[9][j]))
    g[7] = [0.0] * 64
    gb1 = [0.0] * 8
    for j in range(64):
        g[7][j] += g[8][j]
        gb1[j % 8] += g[8][j]
    gw1 = [0.0] * 16
    for r in range(8):
        for c in range(8):
            dy = g[7][r * 8 + c]
            for k in range(2):
                gw1[k * 8 + c] += dy * x[r * 2 + k]
    return [[f32(e) for e in gw1], [f32(e) for e in gb1], [f32(e) for e in gw2], [f32(gb2[0])]]


BASES = [16, 32, 40, 48]


def main():
    out = []
    t, e = tape(True), tape(False)
    out.append(f"tape: {t['forward']} forward steps, {t['backward']} backward steps; statuses ok and ok")
    out.append(f"values arena: {t['values']} floats in {t['value_blocks']} blocks while training, "
               f"{e['values']} floats in {e['value_blocks']} blocks forward only, {t['kept']} if every value were kept")
    out.append(f"gradient arena: {t['grads']} doubles in {t['grad_blocks']} blocks")
    out.append(f"values saved for the backward pass: {len(t['saved'])};" + "".join(f" {i}" for i in t["saved"]))

    inp = [0.0] * 58
    for i in range(8):
        x0 = (i % 4 - 1.5) / 2
        x1 = i // 4 - 0.5
        inp[2 * i], inp[2 * i + 1], inp[49 + i] = f32(x0), f32(x1), f32(4 * x0 * x1)
    inp[57] = -1.0
    state = 12345
    for j in range(16, 49):
        state = (state * 1103515245 + 12345) % 2**32
        inp[j] = f32(((state >> 16) % 33 - 16) / 32)

    out.append("")
    out.append("gradient check at the start, central differences with h = 1/64:")
    grads = backward(inp, forward(inp))
    worst = 0.0
    for q, gq in enumerate(grads):
        largest, norm = 0.0, 0.0
        for j, gj in enumerate(gq):
            at = BASES[q] + j
            keep = inp[at]
            inp[at] = f32(keep + 0.015625)
            up = forward(inp)[LOSS][0]
            inp[at] = f32(keep - 0.015625)
            down = forward(inp)[LOSS][0]
            inp[at] = keep
            largest = max(largest, abs((up - down) / 0.03125 - gj))
            norm += gj * gj
        worst = max(worst, largest)
        n = len(gq)
        out.append(f"  {NAMES[q]}: {n} element{'' if n == 1 else 's'}, ok, gradient norm {num(r6(math.sqrt(norm)))}, largest difference {num(r6(largest))}")
    out.append(f"  worst difference {num(r6(worst))}: {'within 0.01' if worst < 0.01 else 'too large'}")

    out.append("")
    out.append("training, learning rate 0.01:")
    for step in range(801):
        v = forward(inp)
        if step % 100 == 0:
            out.append(f"  step {step}: loss {num(r6(v[LOSS][0]))}")
        for q, gq in enumerate(backward(inp, v)):
            for j, gj in enumerate(gq):
                inp[BASES[q] + j] = f32(inp[BASES[q] + j] - 0.01 * gj)
    y = forward(inp, 11)[11]
    out.append("")
    out.append("predictions after training (ok), against the targets:")
    for i in range(8):
        out.append(f"  ({num(inp[2 * i])} {num(inp[2 * i + 1])}): {num(r6(y[i]))} for {num(inp[49 + i])}")

    want = FIXTURE.read_text().splitlines()
    if out != want:
        for k, (a, b) in enumerate(zip(out, want)):
            if a != b:
                print(f"line {k + 1}:\n  oracle  {a}\n  fixture {b}")
                break
        else:
            print(f"line counts differ: oracle {len(out)}, fixture {len(want)}")
        return 1
    print(f"oracle: all {len(out)} lines of {FIXTURE.relative_to(ROOT)} rebuilt independently")
    return 0


if __name__ == "__main__":
    sys.exit(main())
