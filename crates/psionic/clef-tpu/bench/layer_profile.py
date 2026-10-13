"""Times one layer's pieces at a fixed chunk on the TPU (#11459 P2)."""
import functools
import json
import sys
import time
from pathlib import Path

import jax
import jax.numpy as jnp
import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from clef_tpu import delta, model as M  # noqa: E402


def bench(f, *a, n=10):
    jax.block_until_ready(f(*a))
    t = time.perf_counter()
    for _ in range(n):
        o = f(*a)
    jax.block_until_ready(o)
    return (time.perf_counter() - t) / n * 1e3


def main():
    path = Path(sys.argv[1])
    C = int(sys.argv[2]) if len(sys.argv) > 2 else 2048
    cache = int(sys.argv[3]) if len(sys.argv) > 3 else 16384
    cfg = M.Config.load(path)
    params, _, _ = M.load_backbone(path, cfg)
    Ld, La = params["layers"][0], params["layers"][3]
    x = jax.random.normal(jax.random.PRNGKey(0), (C, cfg.hidden), jnp.float32)
    st = M.init_state(cfg, cache)
    out = {}
    out["mlp_ms"] = bench(jax.jit(lambda x: M.mm(jax.nn.silu(M.mm(x, Ld["gate_up"], "bf16")[:, :cfg.ffn]) * M.mm(x, Ld["gate_up"], "bf16")[:, cfg.ffn:], Ld["down"], "bf16")), x)
    out["deltanet_layer_ms"] = bench(jax.jit(lambda x, s: M.deltanet(cfg, Ld, s, x, "bf16", "jax")), x, st[0])
    out["deltanet_proj_ms"] = bench(jax.jit(lambda x: (M.mm(x, Ld["in_qkv"], "bf16"), M.mm(M.mm(x, Ld["in_z"], "bf16"), Ld["out"], "bf16"))), x)
    H = 32
    q = jax.random.normal(jax.random.PRNGKey(1), (H, C, 128), jnp.float32)
    g = -jnp.abs(jax.random.normal(jax.random.PRNGKey(2), (H, C))) * 0.1
    b = jax.nn.sigmoid(jax.random.normal(jax.random.PRNGKey(3), (H, C)))
    S0 = jnp.zeros((H, 128, 128))
    out["delta_prep_ms"] = bench(jax.jit(functools.partial(delta.prep, C=64)), q, q, q, g, b)
    parts = jax.jit(functools.partial(delta.prep, C=64))(q, q, q, g, b)
    out["delta_scan_jax_ms"] = bench(jax.jit(delta.scan_jax), *parts, S0)
    out["delta_scan_pallas_ms"] = bench(jax.jit(delta.scan_pallas), *parts, S0)
    for pos in (0, cache - C):
        out[f"attn_layer_pos{pos}_ms"] = bench(jax.jit(lambda x, s, p: M.attention(cfg, La, s, x, p, *M.rope_tables(cfg, p + jnp.arange(C)), "bf16")), x, st[3], jnp.int32(pos))
    out["full_chunk_ms"] = bench(jax.jit(lambda s, x: M.prefill_chunk(cfg, params, s, x, jnp.int32(0), jnp.int32(C))), st, x, n=3) if False else None
    print(json.dumps({"C": C, "cache": cache, **{k: (round(v, 3) if v else v) for k, v in out.items()}}))


if __name__ == "__main__":
    main()
