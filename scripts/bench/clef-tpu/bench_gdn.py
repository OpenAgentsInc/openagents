"""Gated DeltaNet prefill: tpu-inference gdn/v3 Pallas kernel (vendored in SGL-JAX
as fused_chunk_parallel), SGL-JAX pure-JAX chunked, and the token-step reference.

Head configs: Flash 16 key / 32 value heads, 27B 16 / 48, head dim 128, conv 4.
Sequence lengths 512..32k, batch of 1 and packed batches; recurrent state f32 vs bf16.
Error: max/mean |out - ref| against the token-by-token lax.scan reference
(conv1d + SiLU applied with the same weights, f32 state), for T <= REF_MAX.
"""
import os, sys, time, functools, numpy as np, jax, jax.numpy as jnp
from common import env, bench, write, hbm
from sgl_jax.srt.kernels.gdn import (ragged_gated_delta_rule_ref, chunked_gated_delta_rule_jax,
                                     jax_causal_conv1d_prefill)
from sgl_jax.srt.kernels.gdn.fused_chunk_parallel_adapter import _fused_chunk_parallel_prefill_local

CONFIGS = {"flash": (16, 32), "27b": (16, 48)}
DK = DV = 128; KS = 4
SEQS = [int(s) for s in os.environ.get("SEQS", "512,1024,2048,4096,8192,16384,32768").split(",")]
BATCHES = [int(s) for s in os.environ.get("BATCHES", "1,4,16").split(",")]
REF_MAX = int(os.environ.get("REF_MAX", "4096"))
MAX_TOKENS = int(os.environ.get("MAX_TOKENS", "65536"))

def make(seq_lens, n_kq, n_v, state_dtype, seed=0):
    rng = np.random.default_rng(seed)
    T = sum(seq_lens); B = len(seq_lens); D = 2 * n_kq * DK + n_v * DV
    qkv = jnp.asarray(rng.standard_normal((T, D)) * 0.5, jnp.bfloat16)
    b = jnp.asarray(rng.standard_normal((T, n_v)), jnp.bfloat16)
    a = jnp.asarray(rng.standard_normal((T, n_v)), jnp.bfloat16)
    convw = jnp.asarray(rng.standard_normal((D, KS)) * 0.3, jnp.bfloat16)
    a_log = jnp.asarray(rng.uniform(-1.0, 0.0, (n_v,)), jnp.float32)
    # GATE=long: slow decay (state remembers ~1e3 tokens, like trained Qwen3.5 heads); default: the SGL-JAX test fixture (fast decay)
    lo, hi = (-8.0, -4.0) if os.environ.get("GATE") == "long" else (-0.5, 0.5)
    dt_bias = jnp.asarray(rng.uniform(lo, hi, (n_v,)), jnp.float32)
    cu = jnp.asarray([0] + np.cumsum(seq_lens).tolist(), jnp.int32)
    idx = jnp.arange(1, B + 1, dtype=jnp.int32)
    has0 = jnp.zeros((B,), bool)
    lens = jnp.asarray(seq_lens, jnp.int32)
    conv_state = jnp.zeros((B + 1, D, KS - 1), jnp.bfloat16)
    rec = jnp.zeros((B + 1, n_v, DK, DV), state_dtype)
    return dict(qkv=qkv, b=b, a=a, convw=convw, a_log=a_log, dt_bias=dt_bias, cu=cu, idx=idx,
                has0=has0, lens=lens, conv_state=conv_state, rec=rec, T=T)

def fused_fn(n_kq, n_v):
    def f(qkv, b, a, conv_state, rec, convw, a_log, dt_bias, cu, idx, has0, lens):
        return _fused_chunk_parallel_prefill_local(qkv, b, a, conv_state, rec, convw, a_log, dt_bias,
            cu, idx, None, has0, lens, n_kq=n_kq, n_v=n_v, d_k=DK, d_v=DV, kernel_size=KS)
    return jax.jit(f)

def conv_then(fn, n_kq, n_v, **kw):
    def f(qkv, b, a, rec, convw, a_log, dt_bias, cu, idx, has0, conv_state):
        y, _ = jax_causal_conv1d_prefill(qkv.T, convw, None, cu, conv_state, idx, has0, activation="silu")
        new_rec, out = fn(y.T.astype(qkv.dtype), b.astype(jnp.float32), a.astype(jnp.float32), rec, a_log, dt_bias,
                          cu, idx, has0, n_kq=n_kq, n_v=n_v, d_k=DK, d_v=DV, **kw)
        return out, new_rec
    return jax.jit(f)

def main():
    e = env(); rows = []
    for cfg in sys.argv[1:] or list(CONFIGS):
        n_kq, n_v = CONFIGS[cfg]
        ref = conv_then(ragged_gated_delta_rule_ref, n_kq, n_v)
        chk = conv_then(chunked_gated_delta_rule_jax, n_kq, n_v, chunk_size=64)
        fused = fused_fn(n_kq, n_v)
        for B in BATCHES:
            for L in SEQS:
                if B * L > MAX_TOKENS: continue
                for sd_name, sd in (("f32", jnp.float32), ("bf16", jnp.bfloat16)):
                    d = make([L] * B, n_kq, n_v, sd)
                    r = {"config": cfg, "n_kq": n_kq, "n_v": n_v, "batch": B, "seq": L, "tokens": d["T"], "state": sd_name}
                    fargs = (d["qkv"], d["b"], d["a"], d["conv_state"], d["rec"], d["convw"], d["a_log"],
                             d["dt_bias"], d["cu"], d["idx"], d["has0"], d["lens"])
                    try:
                        # the kernel donates/aliases pools; pass fresh copies each call
                        t, out = bench(fused, *fargs)
                        r.update(fused_s=t, fused_tok_s=d["T"] / t, fused_us_per_tok_24layers=t / d["T"] * 24e6)
                        fused_out = np.asarray(out[0], np.float32).reshape(d["T"], -1)
                    except Exception as ex:
                        r["fused_error"] = str(ex)[:400]; fused_out = None
                    if sd_name == "f32":
                        rargs = (d["qkv"], d["b"], d["a"], jnp.zeros_like(d["rec"], jnp.float32), d["convw"], d["a_log"],
                                 d["dt_bias"], d["cu"], d["idx"], d["has0"], d["conv_state"])
                        try:
                            t, o = bench(chk, *rargs, min_time=0.2)
                            r.update(chunked_jax_s=t, chunked_jax_tok_s=d["T"] / t)
                        except Exception as ex:
                            r["chunked_error"] = str(ex)[:300]
                    if L <= REF_MAX and B == 1:
                        rargs = (d["qkv"], d["b"], d["a"], jnp.zeros_like(d["rec"], jnp.float32), d["convw"], d["a_log"],
                                 d["dt_bias"], d["cu"], d["idx"], d["has0"], d["conv_state"])
                        t0 = time.perf_counter(); ro, _ = ref(*rargs); jax.block_until_ready(ro)
                        r["ref_s_incl_compile"] = time.perf_counter() - t0
                        ro = np.asarray(ro, np.float32).reshape(d["T"], -1)
                        if fused_out is not None:
                            diff = np.abs(fused_out - ro)
                            r.update(err_max=float(diff.max()), err_mean=float(diff.mean()),
                                     ref_absmean=float(np.abs(ro).mean()),
                                     err_rel=float(diff.mean() / (np.abs(ro).mean() + 1e-12)))
                    r["hbm"] = hbm()
                    rows.append(r); print(r, flush=True)
    write(f"gdn-{os.environ.get('GATE', 'fast')}-{e['chip']}-{e['host']}.json", {"env": e, "rows": rows})

if __name__ == "__main__":
    main()
