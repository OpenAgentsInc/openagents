#!/usr/bin/env python3
"""Independent gradients of the pinned nanochat GPT (pure Python, binary64).

Chapter 127's reference. It shares no code with psionic/lib/nanochat/backward.coax:
it runs chapter 125's independent forward pass (psionic/tests/nanochat_oracle.py,
written from karpathy/nanochat 92d63d4e gpt.py, read and never executed) on
scalar values that record every arithmetic operation, then walks that record
backwards once. No vector-Jacobian product is written by hand here: each
primitive (+, -, *, /, power, exp, log, sqrt, tanh) knows its own local
derivative, and the chain rule does the rest. Tooling only; never shipped.

It prints psionic/fixtures/nanochat/grad-goldens.coax: per tensor three
digests of the gradient, every 13th gradient value, the loss, and three
plain gradient-descent steps (losses and weight digests).
"""
import importlib.util
import math
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("nanochat_oracle", ROOT / "psionic/tests/nanochat_oracle.py")
oracle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(oracle)

TAPE = []


class V:
    """A binary64 scalar that records its parents and local derivatives."""
    __slots__ = ("data", "grad", "parents")

    def __init__(self, data, parents=()):
        self.data = float(data)
        self.grad = 0.0
        self.parents = parents
        if parents:
            TAPE.append(self)

    def __add__(self, o):
        if isinstance(o, V):
            return V(self.data + o.data, ((self, 1.0), (o, 1.0)))
        return V(self.data + o, ((self, 1.0),))

    __radd__ = __add__

    def __sub__(self, o):
        if isinstance(o, V):
            return V(self.data - o.data, ((self, 1.0), (o, -1.0)))
        return V(self.data - o, ((self, 1.0),))

    def __rsub__(self, o):
        return V(o - self.data, ((self, -1.0),))

    def __mul__(self, o):
        if isinstance(o, V):
            return V(self.data * o.data, ((self, o.data), (o, self.data)))
        return V(self.data * o, ((self, float(o)),))

    __rmul__ = __mul__

    def __truediv__(self, o):
        if isinstance(o, V):
            return V(self.data / o.data, ((self, 1.0 / o.data), (o, -self.data / (o.data * o.data))))
        return V(self.data / o, ((self, 1.0 / o),))

    def __rtruediv__(self, o):
        return V(o / self.data, ((self, -o / (self.data * self.data)),))

    def __neg__(self):
        return V(-self.data, ((self, -1.0),))

    def __pow__(self, k):
        assert k == 2
        return V(self.data * self.data, ((self, 2.0 * self.data),))

    def __lt__(self, o):
        return self.data < (o.data if isinstance(o, V) else o)

    def __gt__(self, o):
        return self.data > (o.data if isinstance(o, V) else o)

    def __le__(self, o):
        return self.data <= (o.data if isinstance(o, V) else o)

    def __ge__(self, o):
        return self.data >= (o.data if isinstance(o, V) else o)


class Math:
    """math for the oracle's forward pass, recording on V and plain on floats."""
    prod = staticmethod(math.prod)
    sin = staticmethod(math.sin)
    cos = staticmethod(math.cos)

    @staticmethod
    def exp(x):
        if not isinstance(x, V):
            return math.exp(x)
        e = math.exp(x.data)
        return V(e, ((x, e),))

    @staticmethod
    def log(x):
        if not isinstance(x, V):
            return math.log(x)
        return V(math.log(x.data), ((x, 1.0 / x.data),))

    @staticmethod
    def sqrt(x):
        if not isinstance(x, V):
            return math.sqrt(x)
        r = math.sqrt(x.data)
        return V(r, ((x, 0.5 / r),))

    @staticmethod
    def tanh(x):
        if not isinstance(x, V):
            return math.tanh(x)
        t = math.tanh(x.data)
        return V(t, ((x, 1.0 - t * t),))


oracle.math = Math


def flat_names(c):
    return [(name, math.prod(shape)) for name, _, shape in oracle.inventory(c)]


def gradient(c, values, tokens):
    """Loss and the flat gradient in state_dict order for flat parameter values."""
    del TAPE[:]
    params, at, leaves = {}, 0, []
    for name, _, shape in oracle.inventory(c):
        n = math.prod(shape)
        vs = [V(x) for x in values[at:at + n]]
        leaves += vs
        params[name] = (shape, vs)
        at += n
    z, _ = oracle.Model(c, params).full(tokens)
    loss = oracle.loss(z, tokens)
    loss.grad = 1.0
    for node in reversed(TAPE):
        for parent, local in node.parents:
            parent.grad += local * node.grad
    return loss.data, [v.grad for v in leaves]


CASES = {
    # NE_SMALL from chapter 126: one block, 2 query heads over 1 key-value head,
    # a value embedding on layer 0 and backout at layer 0.
    "small": dict(sequence_len=8, vocab_size=12, n_layer=1, n_head=2, n_kv_head=1, n_embd=24, window_pattern="L",
                  head=0.5, tokens=[3, 4, 5, 6, 9, 3], rounding=None),
    # Three blocks, 4 query heads over 2 key-value heads: value embeddings on
    # layers 0 and 2, none on 1, backout taken after layer 1.
    "deep": dict(sequence_len=8, vocab_size=7, n_layer=3, n_head=4, n_kv_head=2, n_embd=24, window_pattern="L",
                 head=0.5, tokens=[1, 3, 0, 2, 6, 5, 1], rounding=None),
}
STEPS, RATE, STRIDE = 3, 0.02, 13


def flat_weights(c):
    p = oracle.weights(c)
    return [x for name, _, _ in oracle.inventory(c) for x in p[name][1]]


def digests(c, g):
    """Per tensor: sum, sum of squares and a cosine-weighted sum of its values."""
    out, at = [], 0
    for _, n in flat_names(c):
        part = g[at:at + n]
        out += [math.fsum(part), math.fsum(x * x for x in part), math.fsum(x * math.cos(0.37 * e) for e, x in enumerate(part))]
        at += n
    return out


def coax_list(vals):
    return ",".join(repr(float(v)) for v in vals)


def emit():
    print("// Generated by psionic/tests/nanochat_grad_oracle.py; do not edit.")
    for name, c in CASES.items():
        up = name.upper()
        w = flat_weights(c)
        loss, g = gradient(c, w, c["tokens"])
        d = digests(c, g)
        print(f"NB_{up}_TOKENS :: [{len(c['tokens'])}]int.[{','.join(map(str, c['tokens']))}];")
        print(f"NB_{up}_LOSS :: {loss!r};")
        print(f"NB_{up}_DIGESTS :: [{len(d)}]f64.[{coax_list(d)}];")
        s = g[::STRIDE]
        print(f"NB_{up}_SAMPLES :: [{len(s)}]f64.[{coax_list(s)}];")
        print(f"// {up}: {len(w)} parameters, {len(TAPE)} recorded operations", file=sys.stderr)
    c = CASES["small"]
    w = flat_weights(c)
    losses = []
    for _ in range(STEPS):
        loss, g = gradient(c, w, c["tokens"])
        losses.append(loss)
        w = [a - RATE * b for a, b in zip(w, g)]
    losses.append(gradient(c, w, c["tokens"])[0])
    print(f"NB_DESCENT_LOSSES :: [{len(losses)}]f64.[{coax_list(losses)}];")
    d = digests(c, w)
    print(f"NB_DESCENT_DIGESTS :: [{len(d)}]f64.[{coax_list(d)}];")


if __name__ == "__main__":
    emit()
