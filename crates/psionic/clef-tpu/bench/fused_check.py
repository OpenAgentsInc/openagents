"""The fused Pallas delta rule against the token-step f32 reference and the
XLA chunked path (#11502): accuracy on random and near-parallel keys with
long-memory decays, and time per 512-token layer-chunk.

  python fused_check.py [--T 512] [--chunks 4]
"""
import argparse
import json
import sys
import time
from pathlib import Path

import jax
import jax.numpy as jnp
import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from clef_tpu import delta  # noqa: E402

Hk, Hv, dk, dv = 16, 32, 128, 128


def inputs(key, T, kind):
    ks = jax.random.split(key, 6)
    conv = jax.random.normal(ks[0], (T, 2 * Hk * dk + Hv * dv), jnp.float32)
    if kind == "parallel":
        # keys nearly parallel within each head: one direction plus small noise
        base = jax.random.normal(ks[1], (1, 2 * Hk * dk), jnp.float32)
        conv = conv.at[:, :2 * Hk * dk].set(base + 0.02 * conv[:, :2 * Hk * dk])
    # long-memory decays (dt_bias -8..-4 regime) and large betas
    g = -jnp.exp(jax.random.uniform(ks[2], (T, Hv), minval=-8, maxval=-3))
    beta = jax.nn.sigmoid(jax.random.normal(ks[3], (T, Hv)) + 2.0)
    S0 = 0.1 * jax.random.normal(ks[4], (Hv, dk, dv), jnp.float32)
    return conv, g, beta, S0


def split(conv):
    q = conv[:, :Hk * dk].reshape(-1, Hk, dk)
    k = conv[:, Hk * dk:2 * Hk * dk].reshape(-1, Hk, dk)
    v = conv[:, 2 * Hk * dk:].reshape(-1, Hv, dv)
    rep = Hv // Hk
    q = jnp.repeat(q, rep, 1).transpose(1, 0, 2)
    k = jnp.repeat(k, rep, 1).transpose(1, 0, 2)
    return q, k, v.transpose(1, 0, 2)


def bench(f, *a, n=20):
    jax.block_until_ready(f(*a))
    t = time.perf_counter()
    for _ in range(n):
        o = f(*a)
    jax.block_until_ready(o)
    return (time.perf_counter() - t) / n * 1e3


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--T", type=int, default=512)
    ap.add_argument("--chunks", type=int, default=4, help="layer-chunks carried in sequence for the accuracy check")
    a = ap.parse_args()
    T = a.T
    fused = jax.jit(lambda c, g, b, s: delta.fused(c, g, b, s, Hk=Hk, Hv=Hv, dk=dk, dv=dv))

    def xla(c, g, b, s):
        q, k, v = split(c)
        o, S = delta.chunked(q, k, v, g.T, b.T, s)
        return o.transpose(1, 0, 2).reshape(T, Hv * dv), S

    xla = jax.jit(xla)

    def ref(c, g, b, s):
        q, k, v = split(c)
        o, S = delta.step_ref(q, k, v, g.T, b.T, s)
        return o.transpose(1, 0, 2).reshape(T, Hv * dv), S

    ref = jax.jit(ref)
    for kind in ("random", "parallel"):
        key = jax.random.PRNGKey(0)
        S_f = S_x = S_r = None
        err = {"fused": [0.0, 0.0], "xla": [0.0, 0.0]}
        for i in range(a.chunks):
            key, sub = jax.random.split(key)
            conv, g, b, S0 = inputs(sub, T, kind)
            if S_r is None:
                S_f = S_x = S_r = S0
            of, S_f = fused(conv, g, b, S_f)
            ox, S_x = xla(conv, g, b, S_x)
            orf, S_r = ref(conv, g, b, S_r)
            scale = float(jnp.max(jnp.abs(orf)))
            sscale = float(jnp.max(jnp.abs(S_r)))
            for name, o, S in (("fused", of, S_f), ("xla", ox, S_x)):
                err[name][0] = max(err[name][0], float(jnp.max(jnp.abs(o - orf))) / scale)
                err[name][1] = max(err[name][1], float(jnp.max(jnp.abs(S - S_r))) / sscale)
        finite = bool(jnp.all(jnp.isfinite(of)))
        print(json.dumps({"keys": kind, "T": T, "chunks": a.chunks, "finite": finite,
                          "fused_rel_err_o_S": err["fused"], "xla_rel_err_o_S": err["xla"]}), flush=True)
    conv, g, b, S0 = inputs(jax.random.PRNGKey(1), T, "random")
    print(json.dumps({"T": T, "fused_ms": round(bench(fused, conv, g, b, S0), 3),
                      "xla_ms": round(bench(xla, conv, g, b, S0), 3),
                      "precision": str(delta.FUSED_PREC)}), flush=True)


if __name__ == "__main__":
    main()
