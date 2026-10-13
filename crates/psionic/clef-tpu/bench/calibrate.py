"""int8 calibration map for the Clef TPU worker (#11502).

Runs the BF16 backbone over the encoder fixture records that the e2e gate
does not use (160 records, ~164k tokens) and records, per layer and
projection, the largest |x| of each input channel of the activation the
projection multiplies. `load_backbone(..., calib=<map>)` turns it into
SmoothQuant scales.

  python calibrate.py <clef-flash dir> <fixtures/clef> <out.npz> [--limit 160] [--search 40]

With `--search K`, a second pass over K records picks the SmoothQuant
alpha of each layer's projection that minimizes that projection's int8
output error (squared, summed over real rows, against BF16) and stores it
as `alpha.<layer>.<projection>`.
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
from clef_tpu import model as M  # noqa: E402
from clef_tpu.runner import Runner  # noqa: E402


def calibration_records(fx, max_tokens=1100, e2e_count=40):
    exp = []
    for f in ("expected-000.jsonl", "expected-100.jsonl"):
        exp += [json.loads(l) for l in open(fx / "encoder" / f, encoding="utf-8")]
    e2e = {i for i, e in enumerate(exp) if len(e["input_ids"]) <= max_tokens}
    e2e = set(sorted(e2e)[:e2e_count])
    return [e for i, e in enumerate(exp) if i not in e2e]


def chunks(r, ids):
    T = len(ids)
    C = r.auto_chunk(T)
    Tp = -(-T // C) * C
    cache = next((b for b in r.cache_buckets if b >= Tp), -(-Tp // 2048) * 2048)
    ids_p = np.zeros(Tp, np.int64)
    ids_p[:T] = ids
    return T, C, Tp, cache, jnp.asarray(ids_p, jnp.int32)


@jax.jit
def _errors(x, w, amax, alphas):
    """Squared int8 output error of x @ w for each SmoothQuant alpha (x [R, in], pad rows 0)."""
    xf = x.astype(jnp.float32)
    wf = w.astype(jnp.float32)
    ref = jnp.dot(x.astype(jnp.bfloat16), w, preferred_element_type=jnp.float32)
    wmax = jnp.maximum(jnp.max(jnp.abs(wf), 1), 1e-5)
    am = jnp.maximum(amax, 1e-5)

    def one(alpha):
        sm = jnp.clip(am ** alpha / wmax ** (1 - alpha), 1e-4, 1e4)
        ws = wf * sm[:, None]
        sc = jnp.maximum(jnp.max(jnp.abs(ws), 0), 1e-12) / 127.0
        wq = jnp.clip(jnp.round(ws / sc), -127, 127).astype(jnp.int8)
        xs = xf / sm[None, :]
        ax = jnp.maximum(jnp.max(jnp.abs(xs), -1, keepdims=True), 1e-12) / 127.0
        xq = jnp.clip(jnp.round(xs / ax), -127, 127).astype(jnp.int8)
        y = jnp.dot(xq, wq, preferred_element_type=jnp.int32).astype(jnp.float32) * ax * sc[None, :]
        return jnp.sum((y - ref) ** 2)

    return jax.lax.map(one, alphas), jnp.sum(ref ** 2)


def search(r, recs, amax, alphas):
    al = jnp.asarray(alphas, jnp.float32)
    err, energy = {}, {}
    t = time.time()
    for n, rec in enumerate(recs):
        T, C, Tp, cache, ids_dev = chunks(r, rec["input_ids"])
        state = M.init_state(r.cfg, cache)
        for c0 in range(0, Tp, C):
            x = jnp.asarray(r.embed_dev[jax.lax.dynamic_slice_in_dim(ids_dev, c0, C)], jnp.float32)
            state, _, xs = M.prefill_chunk(r.cfg, r.params, state, x, jnp.int32(c0), jnp.int32(T), calib="inputs")
            for k, xin in xs.items():
                i, name = k.split(".")
                e, en = _errors(xin, r.params["layers"][int(i)][name], amax[k], al)
                err[k] = e if k not in err else err[k] + e
                energy[k] = en if k not in energy else energy[k] + en
        print("search", n, T, round(time.time() - t, 1), file=sys.stderr, flush=True)
    out, picks = {}, {}
    for k, e in err.items():
        e = np.asarray(e)
        best = alphas[int(np.argmin(e))]
        out["alpha." + k] = np.float32(best)
        picks[k] = (best, round(float(np.min(e) / max(float(energy[k]), 1e-30)), 6))
    print(json.dumps({"alphas": picks}), flush=True)
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("model")
    ap.add_argument("fixtures")
    ap.add_argument("out")
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--search", type=int, default=0)
    ap.add_argument("--alphas", default="0.3,0.4,0.5,0.6,0.7,0.8")
    ap.add_argument("--amax", default=None, help="reuse the |x| maxima of an earlier map (skips the first pass)")
    a = ap.parse_args()
    r = Runner(a.model)
    recs = calibration_records(Path(a.fixtures))
    if a.limit:
        recs = recs[:a.limit]
    acc = {}
    if a.amax:
        acc = {k: jnp.asarray(v) for k, v in np.load(a.amax).items() if not k.startswith("alpha.")}
    t = time.time()
    for n, rec in enumerate([] if a.amax else recs):
        ids = rec["input_ids"]
        T = len(ids)
        C = r.auto_chunk(T)
        Tp = -(-T // C) * C
        cache = next((b for b in r.cache_buckets if b >= Tp), -(-Tp // 2048) * 2048)
        ids_p = np.zeros(Tp, np.int64)
        ids_p[:T] = ids
        ids_dev = jnp.asarray(ids_p, jnp.int32)
        state = M.init_state(r.cfg, cache)
        for c0 in range(0, Tp, C):
            x = jnp.asarray(r.embed_dev[jax.lax.dynamic_slice_in_dim(ids_dev, c0, C)], jnp.float32)
            state, _, st = M.prefill_chunk(r.cfg, r.params, state, x, jnp.int32(c0), jnp.int32(T), calib=True)
            for k, v in st.items():
                acc[k] = v if k not in acc else jnp.maximum(acc[k], v)
        print(n, T, round(time.time() - t, 1), file=sys.stderr, flush=True)
    out = {k: np.asarray(v) for k, v in acc.items()}
    if a.search:
        out.update(search(r, recs[:a.search], acc, [float(x) for x in a.alphas.split(",")]))
    np.savez(a.out, **out)
    worst = sorted(((float(np.max(v) / max(np.median(np.asarray(v)), 1e-6)), k) for k, v in acc.items()), reverse=True)[:8]
    print(json.dumps({"records": len(recs), "tokens": sum(len(x["input_ids"]) for x in recs), "keys": len(acc),
                      "max_over_median": [(k, round(v, 1)) for v, k in worst]}))


if __name__ == "__main__":
    main()
