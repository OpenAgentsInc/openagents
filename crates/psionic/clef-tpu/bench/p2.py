"""P2 throughput on one chip: prefill tokens/s per prompt length (#11459).

  python p2.py <clef-flash dir> [--lengths 1536,4096,16384] [--chunk 2048] [--scan jax] [--batch 1]
"""
import argparse
import json
import sys
import time
from pathlib import Path

import jax
import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from clef_tpu.runner import Runner  # noqa: E402


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("model")
    ap.add_argument("--lengths", default="1536,4096,16384")
    ap.add_argument("--chunk", type=int, default=0)
    ap.add_argument("--scan", default="jax")
    ap.add_argument("--precision", default="bf16")
    ap.add_argument("--reps", type=int, default=3)
    a = ap.parse_args()
    r = Runner(a.model, chunk=a.chunk, precision=a.precision, scan=a.scan)
    rng = np.random.default_rng(0)
    for L in [int(x) for x in a.lengths.split(",")]:
        ids = rng.integers(1000, 200000, L).tolist()
        q = [{"type": 1, "question_span": [L - 40, L - 30], "option_spans": [[L - 30, L - 20], [L - 20, L - 10], [L - 10, L - 1]]}]
        r.logits(ids, q)  # compile
        ts, tp = [], []
        for _ in range(a.reps):
            t = time.perf_counter()
            H = r.hidden_rows(ids)
            jax.block_until_ready(H)
            tp.append(time.perf_counter() - t)
            t = time.perf_counter()
            r.logits(ids, q)
            ts.append(time.perf_counter() - t)
        p, e = float(np.median(tp)), float(np.median(ts))
        print(json.dumps({"tokens": L, "chunk": a.chunk or r.auto_chunk(L), "scan": a.scan, "precision": a.precision,
                          "prefill_s": round(p, 4), "request_s": round(e, 4),
                          "prefill_tok_s": round(L / p), "request_tok_s": round(L / e)}), flush=True)


if __name__ == "__main__":
    main()
