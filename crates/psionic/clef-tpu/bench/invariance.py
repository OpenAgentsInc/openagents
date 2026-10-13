"""Where chunk-size differences enter (#11502, gate 5): runs one encoded
fixture record at two prefill chunk sizes and reports, layer by layer, the
largest difference of every projection input (and the final rows).

  python invariance.py <clef-flash dir> <fixtures/clef> [--record 0] [--chunks 64,2048] [--scan fused] [--precision bf16]
"""
import argparse
import json
import sys
from pathlib import Path

import jax
import jax.numpy as jnp
import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from clef_tpu import model as M  # noqa: E402
from clef_tpu.runner import Runner  # noqa: E402


def taps(r, ids, C, cache):
    T = len(ids)
    Tp = -(-T // C) * C
    ids_p = np.zeros(Tp, np.int64)
    ids_p[:T] = ids
    ids_dev = jnp.asarray(ids_p, jnp.int32)
    state = M.init_state(r.cfg, cache)
    out, rows = {}, []
    for c0 in range(0, Tp, C):
        x = jnp.asarray(r.embed_dev[jax.lax.dynamic_slice_in_dim(ids_dev, c0, C)], jnp.float32)
        state, h, xs = M.prefill_chunk(r.cfg, r.params, state, x, jnp.int32(c0), jnp.int32(T),
                                       precision=r.precision, scan=r.scan, calib="inputs")
        rows.append(np.asarray(h))
        for k, v in xs.items():
            out.setdefault(k, []).append(np.asarray(v.astype(jnp.float32)))
    out = {k: np.concatenate(v, 0)[:T] for k, v in out.items()}
    out["final"] = np.concatenate(rows, 0)[:T]
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("model")
    ap.add_argument("fixtures")
    ap.add_argument("--record", type=int, default=0)
    ap.add_argument("--chunks", default="64,2048")
    ap.add_argument("--scan", default="fused")
    ap.add_argument("--precision", default="bf16")
    a = ap.parse_args()
    r = Runner(a.model, precision=a.precision, scan=a.scan)
    exp = [json.loads(l) for l in open(Path(a.fixtures) / "encoder" / "expected-000.jsonl", encoding="utf-8")]
    ids = exp[a.record]["input_ids"]
    c1, c2 = [int(x) for x in a.chunks.split(",")]
    cache = 2048 if len(ids) <= 2048 else -(-len(ids) // 2048) * 2048
    A, B = taps(r, ids, c1, cache), taps(r, ids, c2, cache)
    order = sorted((k for k in A if k != "final"), key=lambda k: (int(k.split(".")[0]), k))
    first = None
    for k in order + ["final"]:
        d = float(np.max(np.abs(A[k] - B[k])))
        rel = d / max(float(np.max(np.abs(A[k]))), 1e-30)
        if d > 0 and first is None:
            first = k
        print(json.dumps({"tap": k, "max_abs": d, "rel": rel, "rows_differing": int(np.sum(np.any(A[k] != B[k], -1)))}))
    print(json.dumps({"tokens": len(ids), "chunks": [c1, c2], "first_difference": first}))


if __name__ == "__main__":
    main()
