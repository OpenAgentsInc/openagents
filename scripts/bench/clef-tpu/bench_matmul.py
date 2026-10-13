"""Matmul TFLOPS at Clef's projection shapes, bf16 / int8 (W8A8 as Qwix lowers it) / fp8.

Shapes are [tokens, K] @ [K, N] for each projection of Clef-Flash (Qwen3.5-9B
backbone) and Clef 27B. Token counts 128..32k. One chip.
"""
import itertools, sys, jax, jax.numpy as jnp
from common import env, bench, write, chip_family

SHAPES = {  # name: (K, N)
    "flash.gdn_qkv": (4096, 8192), "flash.gdn_z": (4096, 4096), "flash.gdn_out": (4096, 4096),
    "flash.attn_q_gate": (4096, 8192), "flash.attn_kv": (4096, 2048), "flash.attn_o": (4096, 4096),
    "flash.mlp_gate_up": (4096, 24576), "flash.mlp_down": (12288, 4096),
    "27b.gdn_qkv": (5120, 10240), "27b.gdn_z": (5120, 6144), "27b.gdn_out": (6144, 5120),
    "27b.attn_q_gate": (5120, 12288), "27b.attn_kv": (5120, 2048), "27b.attn_o": (6144, 5120),
    "27b.mlp_gate_up": (5120, 34816), "27b.mlp_down": (17408, 5120),
}
TOKENS = [128, 512, 1024, 2048, 4096, 8192, 16384, 32768]

def mm_bf16(x, w):
    return jnp.dot(x, w, preferred_element_type=jnp.float32).astype(jnp.bfloat16)

def mm_int8_static(xq, wq, sx, sw):
    # both operands already int8 (activation scale known / precomputed)
    acc = jax.lax.dot_general(xq, wq, (((1,), (0,)), ((), ())), preferred_element_type=jnp.int32)
    return (acc.astype(jnp.float32) * sx * sw).astype(jnp.bfloat16)

def mm_int8_dyn(x, wq, sw):
    # W8A8 with dynamic per-token activation quantization (what Qwix PTQ emits)
    sx = jnp.max(jnp.abs(x.astype(jnp.float32)), axis=1, keepdims=True) / 127.0 + 1e-8
    xq = jnp.round(x.astype(jnp.float32) / sx).astype(jnp.int8)
    acc = jax.lax.dot_general(xq, wq, (((1,), (0,)), ((), ())), preferred_element_type=jnp.int32)
    return (acc.astype(jnp.float32) * sx * sw).astype(jnp.bfloat16)

def mm_w8a16(x, wq, sw):
    # weight-only int8: dequantize in the matmul
    return (jnp.dot(x, wq.astype(jnp.bfloat16), preferred_element_type=jnp.float32) * sw).astype(jnp.bfloat16)

def mm_fp8(x8, w8):
    return jax.lax.dot_general(x8, w8, (((1,), (0,)), ((), ())), preferred_element_type=jnp.float32).astype(jnp.bfloat16)

def main():
    e = env(); rows = []
    sel = sys.argv[1:] or list(SHAPES)
    key = jax.random.PRNGKey(0)
    for name in sel:
        K, N = SHAPES[name]
        w = (jax.random.normal(key, (K, N), jnp.float32) * 0.02).astype(jnp.bfloat16)
        sw = (jnp.max(jnp.abs(w.astype(jnp.float32)), axis=0, keepdims=True) / 127.0)
        wq = jnp.round(w.astype(jnp.float32) / sw).astype(jnp.int8)
        try: w8 = w.astype(jnp.float8_e4m3fn)
        except Exception: w8 = None
        for M in TOKENS:
            x = (jax.random.normal(key, (M, K), jnp.float32)).astype(jnp.bfloat16)
            flop = 2.0 * M * K * N
            variants = {
                "bf16": (jax.jit(mm_bf16), (x, w)),
                "int8_w8a8_static": (jax.jit(mm_int8_static), (x.astype(jnp.int8), wq, jnp.float32(0.01), sw)),
                "int8_w8a8_dynamic": (jax.jit(mm_int8_dyn), (x, wq, sw)),
                "int8_weight_only": (jax.jit(mm_w8a16), (x, wq, sw)),
            }
            if w8 is not None:
                variants["fp8_e4m3"] = (jax.jit(mm_fp8), (x.astype(jnp.float8_e4m3fn), w8))
            for v, (fn, args) in variants.items():
                try:
                    t, _ = bench(fn, *args)
                    r = {"shape": name, "M": M, "K": K, "N": N, "dtype": v, "seconds": t, "tflops": flop / t / 1e12}
                except Exception as ex:
                    r = {"shape": name, "M": M, "K": K, "N": N, "dtype": v, "error": str(ex)[:300]}
                rows.append(r); print(r, flush=True)
    write(f"matmul-{e['chip']}-{e['host']}.json", {"env": e, "rows": rows})

if __name__ == "__main__":
    main()
