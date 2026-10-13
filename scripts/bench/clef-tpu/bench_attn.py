"""Causal attention at Clef's full-attention shape: head_dim 256, GQA 16q/4kv (Flash)
and 24q/4kv (27B). Splash attention (Pallas, MQA-per-kv-head form, a few block
sizes), Pallas flash_attention (kv repeated) and XLA dot_product_attention.
Sequence lengths 1k..32k, batch 1. TFLOPS counts causal work (half the square)."""
import os, sys, jax, jax.numpy as jnp
from common import env, bench, write, hbm
from jax.experimental.pallas.ops.tpu.splash_attention import splash_attention_kernel as sk
from jax.experimental.pallas.ops.tpu.splash_attention import splash_attention_mask as sm
try:
    from jax.experimental.pallas.ops.tpu.flash_attention import flash_attention as pallas_flash
except Exception:
    pallas_flash = None

CONFIGS = {"flash": (16, 4), "27b": (24, 4)}
D = 256
SEQS = [int(s) for s in os.environ.get("SEQS", "1024,2048,4096,8192,16384,32768").split(",")]

def splash_gqa(nq, nkv, L, blk):
    g = nq // nkv
    mask = sm.MultiHeadMask([sm.CausalMask((L, L)) for _ in range(g)])
    bs = sk.BlockSizes(block_q=blk, block_kv_compute=blk, block_kv=blk,
                       block_q_dkv=blk, block_kv_dkv=blk, block_kv_dkv_compute=blk,
                       block_q_dq=blk, block_kv_dq=blk)
    kern = sk.make_splash_mqa_single_device(mask, block_sizes=bs)
    def f(q, k, v):  # q [nq, L, D], k/v [nkv, L, D]
        qg = q.reshape(nkv, g, L, D) * (D ** -0.5)
        return jax.vmap(kern)(qg, k, v).reshape(nq, L, D)
    return jax.jit(f)

def main():
    e = env(); rows = []
    key = jax.random.PRNGKey(0)
    for cfg in sys.argv[1:] or list(CONFIGS):
        nq, nkv = CONFIGS[cfg]
        for L in SEQS:
            q = jax.random.normal(key, (nq, L, D), jnp.bfloat16)
            k = jax.random.normal(key, (nkv, L, D), jnp.bfloat16)
            v = jax.random.normal(key, (nkv, L, D), jnp.bfloat16)
            flop = 4.0 * L * L * D * nq / 2
            impls = {}
            for blk in (256, 512, 1024):
                if blk <= L: impls[f"splash_b{blk}"] = lambda blk=blk: (splash_gqa(nq, nkv, L, blk), (q, k, v))
            if pallas_flash is not None:
                def pf():
                    f = jax.jit(lambda q, k, v: pallas_flash(q[None], jnp.repeat(k, nq // nkv, 0)[None],
                                jnp.repeat(v, nq // nkv, 0)[None], causal=True, sm_scale=D ** -0.5)[0])
                    return f, (q, k, v)
                impls["pallas_flash"] = pf
            if L <= 16384:
                def xla():
                    f = jax.jit(lambda q, k, v: jax.nn.dot_product_attention(
                        q.transpose(1, 0, 2)[None], k.transpose(1, 0, 2)[None], v.transpose(1, 0, 2)[None],
                        is_causal=True, implementation="xla")[0])
                    return f, (q, k, v)
                impls["xla_dpa"] = xla
            for name, mk in impls.items():
                r = {"config": cfg, "nq": nq, "nkv": nkv, "head_dim": D, "seq": L, "impl": name}
                try:
                    fn, args = mk()
                    t, _ = bench(fn, *args)
                    r.update(seconds=t, tflops=flop / t / 1e12, us_per_token=t / L * 1e6)
                except Exception as ex:
                    r["error"] = str(ex)[:300]
                rows.append(r); print(r, flush=True)
    write(f"attn-{e['chip']}-{e['host']}.json", {"env": e, "rows": rows})

if __name__ == "__main__":
    main()
