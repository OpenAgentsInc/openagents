"""Does a bf16 recurrent-state pool drift over chunked prefill? Run one long sequence through
the gdn/v3 kernel in chunks of C tokens (state carried in the pool between calls, as a
chunked-prefill server does), with the pool in f32 and in bf16, and compare each against the
single-call output (whose carry is f32 in VMEM) and the token-step reference."""
import os, sys, numpy as np, jax, jax.numpy as jnp
from common import env, write
from bench_gdn import make, fused_fn, conv_then, CONFIGS, DK, DV, KS
from sgl_jax.srt.kernels.gdn import ragged_gated_delta_rule_ref

def run_chunked(d, n_kq, n_v, C, sd):
    f = fused_fn(n_kq, n_v); T = d["T"]; D = d["qkv"].shape[1]
    conv = jnp.zeros((2, D, KS - 1), jnp.bfloat16); rec = jnp.zeros((2, n_v, DK, DV), sd)
    outs = []
    for s in range(0, T, C):
        e = min(T, s + C)
        o, conv, rec = f(d["qkv"][s:e], d["b"][s:e], d["a"][s:e], conv, rec, d["convw"], d["a_log"], d["dt_bias"],
                         jnp.asarray([0, e - s], jnp.int32), jnp.asarray([1], jnp.int32),
                         jnp.asarray([s > 0]), jnp.asarray([e], jnp.int32))
        outs.append(np.asarray(o, np.float32).reshape(e - s, -1))
    return np.concatenate(outs)

def main():
    e = env(); rows = []
    T = int(os.environ.get("T", "16384"))
    for cfg in sys.argv[1:] or ["flash", "27b"]:
        n_kq, n_v = CONFIGS[cfg]
        d = make([T], n_kq, n_v, jnp.float32)
        whole = run_chunked(d, n_kq, n_v, T, jnp.float32)
        ref = None
        if T <= int(os.environ.get("REF_MAX", "16384")):
            refj = conv_then(ragged_gated_delta_rule_ref, n_kq, n_v)
            ro, _ = refj(d["qkv"], d["b"], d["a"], jnp.zeros_like(d["rec"]), d["convw"], d["a_log"], d["dt_bias"],
                         d["cu"], d["idx"], d["has0"], d["conv_state"])
            ref = np.asarray(ro, np.float32).reshape(T, -1)
        for C in (512, 2048, 8192):
            for name, sd in (("f32", jnp.float32), ("bf16", jnp.bfloat16)):
                o = run_chunked(d, n_kq, n_v, C, sd)
                r = {"config": cfg, "T": T, "chunk": C, "state_pool": name,
                     "vs_single_call_max": float(np.abs(o - whole).max()),
                     "vs_single_call_mean_rel": float(np.abs(o - whole).mean() / np.abs(whole).mean())}
                if ref is not None:
                    r.update(vs_ref_max=float(np.abs(o - ref).max()),
                             vs_ref_mean_rel=float(np.abs(o - ref).mean() / np.abs(ref).mean()),
                             last_chunk_vs_ref_mean_rel=float(np.abs(o[-C:] - ref[-C:]).mean() / np.abs(ref[-C:]).mean()))
                r["gate"] = os.environ.get("GATE", "fast"); rows.append(r); print(r, flush=True)
    write(f"gdn-state-{os.environ.get('GATE', 'fast')}-{e['chip']}-{e['host']}.json", {"env": e, "rows": rows})

if __name__ == "__main__":
    main()
