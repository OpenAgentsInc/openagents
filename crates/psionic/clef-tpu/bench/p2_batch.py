"""P2/P3: batched prefill throughput, B requests of one length advanced together (#11459)."""
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


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("model")
    ap.add_argument("--lengths", default="1536,16384")
    ap.add_argument("--batches", default="1,2,4,8")
    ap.add_argument("--chunk", type=int, default=512)
    a = ap.parse_args()
    path = Path(a.model)
    cfg = M.Config.load(path)
    params, embed, _ = M.load_backbone(path, cfg)
    rng = np.random.default_rng(0)
    C = a.chunk
    for L in [int(x) for x in a.lengths.split(",")]:
        n = -(-L // C)
        cache = max(2048, n * C)
        for B in [int(x) for x in a.batches.split(",")]:
            ids = rng.integers(1000, 200000, (B, n * C))
            emb = jnp.asarray(np.asarray(embed[ids], np.float32))

            def run():
                st = jax.tree.map(lambda z: jnp.broadcast_to(z, (B,) + z.shape).copy(), M.init_state(cfg, cache))
                out = None
                for c in range(n):
                    st, out = M.prefill_chunk_batch(cfg, params, st, emb[:, c * C:(c + 1) * C],
                                                    jnp.full((B,), c * C, jnp.int32), jnp.full((B,), L, jnp.int32))
                return out

            jax.block_until_ready(run())
            ts = []
            for _ in range(3):
                t = time.perf_counter()
                jax.block_until_ready(run())
                ts.append(time.perf_counter() - t)
            s = float(np.median(ts))
            print(json.dumps({"tokens": L, "batch": B, "chunk": C, "seconds": round(s, 4),
                              "tok_s": round(B * L / s)}), flush=True)


if __name__ == "__main__":
    main()
