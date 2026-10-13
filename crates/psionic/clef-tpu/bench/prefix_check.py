"""Prefix reuse on the TPU worker: exactness and speed (#11459 P3).

A prompt is answered three ways: whole (no snapshot), split at P with a
snapshot taken (miss), and restored from that snapshot (hit). The logits of
miss and hit must match the whole run to the chunk-invariance tolerance.
"""
import json
import sys
import time
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from clef_tpu.runner import Runner  # noqa: E402


def main():
    r = Runner(sys.argv[1], prefix_budget=4 << 30)
    rng = np.random.default_rng(1)
    for T, P in ((1800, 1200), (4096, 3500), (8192, 7600), (16000, 15000)):
        ids = rng.integers(1000, 200000, T).tolist()
        q = [{"type": 1, "question_span": [T - 60, T - 40],
              "option_spans": [[T - 40, T - 30], [T - 30, T - 20], [T - 20, T - 5]]},
             {"type": 0, "question_span": [P - 50, P - 10], "option_spans": [[T - 4, T - 3], [T - 3, T - 2]]}]
        whole = r.logits(ids, q)
        r._prefixes.clear(); r._prefix_bytes.clear()
        times = {}
        t = time.perf_counter(); miss = r.logits(ids, q, prefix_tokens=P); times["miss"] = time.perf_counter() - t
        t = time.perf_counter(); hit = r.logits(ids, q, prefix_tokens=P); times["hit"] = time.perf_counter() - t
        assert r.last_restored == P
        t = time.perf_counter(); r.logits(ids, q); times["whole"] = time.perf_counter() - t
        t = time.perf_counter(); hit = r.logits(ids, q, prefix_tokens=P); times["hit"] = time.perf_counter() - t
        d = lambda a, b: max(abs(x - y) for qa, qb in zip(a, b) for x, y in zip(qa, qb))
        print(json.dumps({"tokens": T, "prefix": P, "max_dlogit_miss": round(d(whole, miss), 5),
                          "max_dlogit_hit": round(d(whole, hit), 5), "hit_vs_miss": round(d(miss, hit), 6),
                          **{k + "_ms": round(v * 1e3, 1) for k, v in times.items()},
                          "speedup": round(times["whole"] / times["hit"], 2)}), flush=True)

    # fresh prompts with a new prefix length each time (live traffic): every
    # miss must run at prefill speed, with no compile for the new length
    for base_T, base_P in ((1100, 800), (4096, 3500)):
        fresh = []
        for j in range(6):
            T, P = base_T + 3 * j, base_P + 7 * j
            ids = rng.integers(1000, 200000, T).tolist()
            q = [{"type": 0, "question_span": [P - 50, P - 10], "option_spans": [[T - 4, T - 3], [T - 3, T - 2]]}]
            t = time.perf_counter()
            r.logits(ids, q, prefix_tokens=P)
            fresh.append(round((time.perf_counter() - t) * 1e3, 1))
        print(json.dumps({"fresh_misses_ms": fresh, "tokens_from": base_T, "prefix_from": base_P}), flush=True)


if __name__ == "__main__":
    main()
