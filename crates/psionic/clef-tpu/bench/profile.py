"""Where a prefill's device time goes (#11502): traces a few prompts with the
JAX profiler and prints XLA op self-time grouped by category.

  python profile.py <clef-flash dir> [--tokens 4096] [--precision bf16] [--top 40]

Needs `xprof` in the venv.
"""
import argparse
import collections
import glob
import json
import shutil
import sys
import tempfile
import time
from pathlib import Path

import jax
import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from clef_tpu.runner import Runner  # noqa: E402


def op_table(trace_dir):
    from xprof.convert import raw_to_tool_data as r
    f = sorted(glob.glob(f"{trace_dir}/**/*.xplane.pb", recursive=True))[-1]
    data, _ = r.xspace_to_tool_data([f], "framework_op_stats", {})
    d = json.loads(data) if isinstance(data, (str, bytes)) else data
    tbl = d[1] if isinstance(d, list) and len(d) > 1 else d
    cols = [c["label"] if isinstance(c, dict) else c for c in tbl["cols"]]
    rows = [[(v.get("v") if isinstance(v, dict) else v) for v in row["c"]] for row in tbl["rows"]]
    return cols, rows


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("model")
    ap.add_argument("--tokens", type=int, default=4096)
    ap.add_argument("--precision", default="bf16")
    ap.add_argument("--chunk", type=int, default=0)
    ap.add_argument("--scan", default="jax")
    ap.add_argument("--calib", default=None)
    ap.add_argument("--reps", type=int, default=3)
    ap.add_argument("--top", type=int, default=40)
    a = ap.parse_args()
    r = Runner(a.model, chunk=a.chunk, precision=a.precision, scan=a.scan, calib=a.calib)
    rng = np.random.default_rng(0)
    L = a.tokens
    ids = rng.integers(1000, 200000, L).tolist()
    q = [{"type": 1, "question_span": [L - 40, L - 30], "option_spans": [[L - 30, L - 20], [L - 20, L - 10]]}]
    r.logits(ids, q)
    r.logits(ids, q)
    d = tempfile.mkdtemp()
    t = time.perf_counter()
    with jax.profiler.trace(d):
        for _ in range(a.reps):
            r.logits(ids, q)
    wall = (time.perf_counter() - t) / a.reps
    keep = Path.home() / "trace-last"
    shutil.rmtree(keep, ignore_errors=True)
    shutil.move(d, keep)
    cols, rows = op_table(keep)
    host = [i for i, c in enumerate(cols) if c.lower().startswith("host/device")][0]
    typ = [i for i, c in enumerate(cols) if c.lower() in ("type", "operation type")][0]
    name = [i for i, c in enumerate(cols) if c.lower() in ("operation", "operation name")][0]
    st = [i for i, c in enumerate(cols) if "total self-time (us)" in c.lower()][0]
    dev = [x for x in rows if x[host] == "Device"]
    total = sum(x[st] for x in dev)
    by = collections.Counter()
    for x in dev:
        by[x[typ]] += x[st]
    print(json.dumps({"tokens": L, "precision": a.precision, "wall_ms": round(wall * 1e3, 2),
                      "device_ms": round(total / a.reps / 1e3, 2),
                      "tok_s": round(L / wall)}))
    for k, v in by.most_common(25):
        print(f"{v / total * 100:6.2f}%  {v / a.reps / 1e3:8.3f} ms  {k}")
    print("--- top ops")
    dev.sort(key=lambda x: -x[st])
    for x in dev[:a.top]:
        print(f"{x[st] / total * 100:6.2f}%  {x[st] / a.reps / 1e3:8.3f} ms  {x[typ]:<24} {str(x[name])[:110]}")


if __name__ == "__main__":
    main()
