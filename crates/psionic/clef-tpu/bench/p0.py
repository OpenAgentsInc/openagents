"""P0 feasibility bench for Clef on Cloud TPU (#11459).

1. bf16 matmul throughput at Clef-Flash / Clef 27B projection shapes.
2. Chunked gated delta rule (Qwen3.5 linear attention) at Flash sizes:
   a plain-JAX chunked form and a Pallas kernel for the sequential part,
   both checked against a token-step reference.

Run on a TPU VM: python -I p0.py [--peak 918] > p0.json
"""

import argparse
import functools
import json
import time

import jax
import jax.numpy as jnp
import numpy as np
from jax.experimental import pallas as pl
from jax.experimental.pallas import tpu as pltpu


def bench(fn, *args, iters=20):
    out = fn(*args)
    jax.block_until_ready(out)
    out = fn(*args)
    jax.block_until_ready(out)
    t = time.perf_counter()
    for _ in range(iters):
        out = fn(*args)
    jax.block_until_ready(out)
    return (time.perf_counter() - t) / iters


def matmuls(peak):
    shapes = {
        "flash.deltanet.in_proj_qkvz": (4096, 12288),
        "flash.deltanet.out_proj": (4096, 4096),
        "flash.attn.q_gate": (4096, 8192),
        "flash.attn.kv": (4096, 2048),
        "flash.mlp.gate_up": (4096, 24576),
        "flash.mlp.down": (12288, 4096),
        "flash.head.mem": (4096, 1024),
        "27b.deltanet.in_proj_qkvz": (5120, 16384),
        "27b.mlp.gate_up": (5120, 34816),
        "27b.mlp.down": (17408, 5120),
    }
    rows = []
    key = jax.random.PRNGKey(0)
    for m in (512, 2048, 8192):
        for name, (k, n) in shapes.items():
            a = jax.random.normal(key, (m, k), jnp.bfloat16)
            b = jax.random.normal(key, (k, n), jnp.bfloat16)
            f = jax.jit(lambda a, b: jnp.dot(a, b, preferred_element_type=jnp.float32).astype(jnp.bfloat16))
            s = bench(f, a, b)
            tf = 2 * m * k * n / s / 1e12
            rows.append({"m": m, "shape": name, "k": k, "n": n, "us": round(s * 1e6, 1),
                         "tflops": round(tf, 1), "pct_peak": round(100 * tf / peak, 1)})
    # whole-layer proxy: Flash MLP + deltanet projections back to back at m=2048
    return rows


# ---------------------------------------------------------------- delta rule

def l2n(x):
    return x * jax.lax.rsqrt(jnp.sum(x * x, -1, keepdims=True) + 1e-6)


def step_ref(q, k, v, g, beta):
    """Token-step gated delta rule. q,k [H,T,dk] v [H,T,dv] g,beta [H,T]. f32."""
    dk = q.shape[-1]
    q = l2n(q) / np.sqrt(dk)
    k = l2n(k)

    def one(S, xs):
        qt, kt, vt, gt, bt = xs
        S = S * jnp.exp(gt)[:, None, None]
        kv = jnp.einsum("hk,hkv->hv", kt, S)
        d = (vt - kv) * bt[:, None]
        S = S + kt[:, :, None] * d[:, None, :]
        return S, jnp.einsum("hk,hkv->hv", qt, S)

    H, T, dv = v.shape
    S0 = jnp.zeros((H, dk, dv), jnp.float32)
    xs = (q.transpose(1, 0, 2), k.transpose(1, 0, 2), v.transpose(1, 0, 2), g.T, beta.T)
    S, o = jax.lax.scan(one, S0, xs)
    return o.transpose(1, 0, 2), S


def inv_unit_lower(L, C):
    """(I + L)^-1 for strictly lower L via the nilpotent product: matmuls only."""
    n = -L
    eye = jnp.eye(C, dtype=L.dtype)
    acc = eye + n
    p = n
    j = 2
    while j < C:
        p = p @ p
        acc = acc @ (eye + p)
        j *= 2
    return acc


def chunk_prep(q, k, v, g, beta, C):
    """Intra-chunk work, parallel over chunks and heads (all matmuls)."""
    H, T, dk = q.shape
    dv = v.shape[-1]
    N = T // C
    q = l2n(q) / np.sqrt(dk)
    k = l2n(k)
    q, k, v = (x.reshape(H, N, C, -1) for x in (q, k, v))
    g = g.reshape(H, N, C)
    beta = beta.reshape(H, N, C)
    gc = jnp.cumsum(g, -1)
    diff = gc[..., :, None] - gc[..., None, :]
    tril = jnp.tril(jnp.ones((C, C), bool))
    decay = jnp.where(tril, jnp.exp(jnp.where(tril, diff, 0.0)), 0.0)
    kb = k * beta[..., None]
    vb = v * beta[..., None]
    strict = jnp.tril(jnp.ones((C, C), bool), -1)
    Lm = jnp.where(strict, jnp.einsum("hnik,hnjk->hnij", kb, k) * decay, 0.0)
    Tm = inv_unit_lower(Lm, C)
    u = Tm @ vb                                   # [H,N,C,dv]
    w = Tm @ (kb * jnp.exp(gc)[..., None])        # k_cumdecay
    A = jnp.where(tril, jnp.einsum("hnik,hnjk->hnij", q, k) * decay, 0.0)
    qd = q * jnp.exp(gc)[..., None]
    glast = gc[..., -1]                           # [H,N]
    kt = k * jnp.exp(glast[..., None] - gc)[..., None]
    return qd, A, u, w, kt, jnp.exp(glast)


def chunk_scan_jax(qd, A, u, w, kt, dl):
    H, N, C, dk = qd.shape
    dv = u.shape[-1]

    def one(S, xs):
        qd_i, A_i, u_i, w_i, kt_i, dl_i = xs
        vn = u_i - w_i @ S
        o = qd_i @ S + A_i @ vn
        S = S * dl_i[:, None, None] + jnp.swapaxes(kt_i, -1, -2) @ vn
        return S, o

    S0 = jnp.zeros((H, dk, dv), jnp.float32)
    xs = tuple(jnp.moveaxis(x, 1, 0) for x in (qd, A, u, w, kt, dl))
    S, o = jax.lax.scan(one, S0, xs)
    return jnp.moveaxis(o, 0, 1).reshape(H, N * C, dv), S


def _scan_kernel(qd_ref, a_ref, u_ref, w_ref, kt_ref, dl_ref, o_ref, s_out_ref, s_ref):
    n = pl.program_id(1)

    @pl.when(n == 0)
    def _():
        s_ref[...] = jnp.zeros_like(s_ref)

    S = s_ref[...]
    vn = u_ref[0, 0] - jnp.dot(w_ref[0, 0], S, preferred_element_type=jnp.float32)
    o = jnp.dot(qd_ref[0, 0], S, preferred_element_type=jnp.float32) + jnp.dot(
        a_ref[0, 0], vn, preferred_element_type=jnp.float32)
    o_ref[0, 0] = o
    S = S * dl_ref[0, 0] + jax.lax.dot_general(
        kt_ref[0, 0], vn, (((0,), (0,)), ((), ())), preferred_element_type=jnp.float32)
    s_ref[...] = S
    s_out_ref[0] = S


def chunk_scan_pallas(qd, A, u, w, kt, dl):
    H, N, C, dk = qd.shape
    dv = u.shape[-1]
    dlb = jnp.broadcast_to(dl[:, :, None, None], (H, N, 8, 128))[:, :, :1, :1]
    dlb = jnp.broadcast_to(dl[:, :, None, None], (H, N, dk, dv))  # simple: full block
    blk = lambda d: pl.BlockSpec((1, 1, C, d), lambda h, n: (h, n, 0, 0))
    o, S = pl.pallas_call(
        _scan_kernel,
        grid=(H, N),
        in_specs=[blk(dk), blk(C), blk(dv), blk(dk), blk(dk),
                  pl.BlockSpec((1, 1, dk, dv), lambda h, n: (h, n, 0, 0))],
        out_specs=[blk(dv), pl.BlockSpec((1, dk, dv), lambda h, n: (h, 0, 0))],
        out_shape=[jax.ShapeDtypeStruct((H, N, C, dv), jnp.float32),
                   jax.ShapeDtypeStruct((H, dk, dv), jnp.float32)],
        scratch_shapes=[pltpu.VMEM((dk, dv), jnp.float32)],
        compiler_params=pltpu.CompilerParams(dimension_semantics=("parallel", "arbitrary")),
    )(qd, A, u, w, kt, dlb)
    return o.reshape(H, N * C, dv), S


def delta_flops(H, T, C, dk, dv):
    N = T // C
    per_chunk = 2 * (C * C * dk * 2          # kb k^T, q k^T
                     + C * C * C * 2 * 3     # ~log2 C inverse products (approx 6)
                     + C * C * dv + C * C * dk  # T vb, T kb
                     + C * dk * dv * 3       # w S, qd S, kt^T vn
                     + C * C * dv)           # A vn
    return H * N * per_chunk


def deltanet(peak):
    H, dk, dv = 32, 128, 128   # Flash: 16 key heads repeated to 32 value heads
    rows = []
    rng = np.random.default_rng(0)
    for T in (2048, 16384):
        for C in (64, 128):
            q = jnp.asarray(rng.standard_normal((H, T, dk)), jnp.float32)
            k = jnp.asarray(rng.standard_normal((H, T, dk)), jnp.float32)
            v = jnp.asarray(rng.standard_normal((H, T, dv)), jnp.float32)
            g = -jnp.asarray(np.abs(rng.standard_normal((H, T))) * 0.1, jnp.float32)
            beta = jax.nn.sigmoid(jnp.asarray(rng.standard_normal((H, T)), jnp.float32))
            prep = jax.jit(functools.partial(chunk_prep, C=C))
            sj = jax.jit(chunk_scan_jax)
            sp = jax.jit(chunk_scan_pallas)
            full_j = jax.jit(lambda *a: chunk_scan_jax(*chunk_prep(*a, C=C)))
            full_p = jax.jit(lambda *a: chunk_scan_pallas(*chunk_prep(*a, C=C)))
            row = {"T": T, "C": C, "H": H}
            if T == 2048:
                ref_o, ref_S = jax.jit(step_ref)(q, k, v, g, beta)
                for nm, f in (("jax", full_j), ("pallas", full_p)):
                    o, S = f(q, k, v, g, beta)
                    row[f"{nm}_max_abs_err_out"] = float(jnp.max(jnp.abs(o - ref_o)))
                    row[f"{nm}_max_abs_err_state"] = float(jnp.max(jnp.abs(S - ref_S)))
                row["ref_out_max_abs"] = float(jnp.max(jnp.abs(ref_o)))
            parts = prep(q, k, v, g, beta)
            t_prep = bench(prep, q, k, v, g, beta)
            t_sj = bench(sj, *parts)
            t_sp = bench(sp, *parts)
            t_fj = bench(full_j, q, k, v, g, beta)
            t_fp = bench(full_p, q, k, v, g, beta)
            fl = delta_flops(H, T, C, dk, dv)
            row.update({
                "prep_ms": round(t_prep * 1e3, 3), "scan_jax_ms": round(t_sj * 1e3, 3),
                "scan_pallas_ms": round(t_sp * 1e3, 3),
                "full_jax_ms": round(t_fj * 1e3, 3), "full_pallas_ms": round(t_fp * 1e3, 3),
                "gflop": round(fl / 1e9, 2),
                "full_best_pct_peak_f32_flops_vs_bf16_peak": round(100 * fl / min(t_fj, t_fp) / 1e12 / peak, 2),
                "us_per_token_per_layer": round(min(t_fj, t_fp) / T * 1e6, 4),
            })
            rows.append(row)
            print(json.dumps(row), flush=True)
    return rows


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--peak", type=float, default=918.0)
    ap.add_argument("--skip-matmul", action="store_true")
    a = ap.parse_args()
    out = {"devices": [str(d) for d in jax.devices()], "jax": jax.__version__, "peak_tflops": a.peak}
    if not a.skip_matmul:
        out["matmul"] = matmuls(a.peak)
        for r in out["matmul"]:
            print(json.dumps(r), flush=True)
    out["deltanet"] = deltanet(a.peak)
    print("RESULT " + json.dumps(out))


if __name__ == "__main__":
    main()
