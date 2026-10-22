#!/usr/bin/env python3
"""Layer 0–1 router components on common inputs (#12405, second review step 3).

    g1_router_components.py RECORD ORACLE WEIGHTS CASE OUT

RECORD is a `qualify --observe` record folder for CASE, ORACLE the oracle
dump captured with `--router-tops`, WEIGHTS the checkpoint folder. For each
of layers 0 and 1, in decode and replay:

- local: the runtime's selected experts against an F64 evaluation of the
  selection function, sqrt(softplus(W x)) + bias, on the runtime's own
  router input; a difference is a local gate or routing arithmetic error.
- propagated: that F64 selection against the oracle's; a difference comes
  from the router input, which differs upstream.
- connected score error: F64 scores of the runtime's input against the
  oracle's retained scores for the same experts.
- changed rows: the oracle's boundary gap and the regret of each changed
  set, resolved only when every removed and added expert is retained.
- common input: at the oracle's labeled rows, the oracle's own router input,
  the F64 scores on it against the oracle's (the oracle's local error), and
  the BF16 values where the runtime's input differs (rounding flips).
"""
import json
import struct
import sys
from pathlib import Path

import numpy as np

record, oracle, weights, case, out = (Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3]),
                                      sys.argv[4], Path(sys.argv[5]))


class Safetensors:
    """Raw safetensors reader: BF16 becomes F32 without a numpy BF16 type."""

    def __init__(self, path):
        self.path = path
        with open(path, "rb") as f:
            n = struct.unpack("<Q", f.read(8))[0]
            self.header = json.loads(f.read(n))
        self.base = 8 + n

    def keys(self):
        return [k for k in self.header if k != "__metadata__"]

    def get(self, name):
        meta = self.header[name]
        a, b = meta["data_offsets"]
        with open(self.path, "rb") as f:
            f.seek(self.base + a)
            raw = f.read(b - a)
        dt = meta["dtype"]
        if dt == "BF16":
            v = (np.frombuffer(raw, "<u2").astype(np.uint32) << 16).view(np.float32)
        elif dt == "F32":
            v = np.frombuffer(raw, "<f4")
        elif dt == "I32":
            v = np.frombuffer(raw, "<i4")
        elif dt == "I64":
            v = np.frombuffer(raw, "<i8")
        else:
            raise ValueError(f"{name}: {dt}")
        return v.reshape(meta["shape"])


run = json.loads((oracle / case / "run.json").read_text())
prompt = run["prompt_tokens"]
labels = run["labels"]
ofiles = [Safetensors(p) for p in sorted((oracle / case).glob("tensors-rank*.safetensors"))]


def otensor(name):
    for f in ofiles:
        if name in f.header:
            return f.get(name)
    return None


index = json.loads((weights / "model.safetensors.index.json").read_text())["weight_map"]


def weight(name):
    return Safetensors(weights / index[name]).get(name)


def site(prefix, layer, name, per):
    hits = sorted(record.glob(f"{prefix}.stage*.L{layer}.{name}.u32"))
    if not hits:
        return None
    words = np.concatenate([np.fromfile(h, dtype="<u4") for h in hits])
    return words.reshape(-1, per)


def runtime_rows(mode, layer, name, per):
    """The runtime's rows for a mode: replay is one step; decode is the
    prompt step followed by one row per forced step."""
    if mode == "replay":
        return site(f"{case}-replay-0-0", layer, name, per)
    rows = [site(f"{case}-0-0", layer, name, per)]
    k = 1
    while (r := site(f"{case}-0-{k}", layer, name, per)) is not None:
        rows.append(r)
        k += 1
    return np.concatenate(rows)


def scores64(w, b, x):
    z = x.astype(np.float64) @ w.T
    return np.sqrt(np.logaddexp(0.0, z)) + b


def pct(v):
    v = np.asarray(v, dtype=np.float64)
    if v.size == 0:
        return None
    return {"n": int(v.size), "p50": float(np.percentile(v, 50)), "p99": float(np.percentile(v, 99)),
            "max": float(v.max())}


report = {"schema": "psionic.deepseek_v41.g1_router_components.v1", "case": case, "layers": []}
for layer in (0, 1):
    w = weight(f"layers.{layer}.ffn.gate.weight").astype(np.float64)
    b = weight(f"layers.{layer}.ffn.gate.bias").astype(np.float64)
    for mode in ("decode", "replay"):
        x = runtime_rows(mode, layer, "FfnIn", w.shape[1])
        ids = runtime_rows(mode, layer, "ExpertIds", 6)
        if x is None or ids is None:
            continue
        x = x.view(np.float32)
        ids = ids.astype(np.int64)
        oids = otensor(f"{case}.{mode}.router_ids.layers.{layer}")
        tops = otensor(f"{case}.{mode}.router_tops.layers.{layer}.scores")
        tids = otensor(f"{case}.{mode}.router_tops.layers.{layer}.ids")
        if tops is None:
            tops = otensor(f"{case}.replay.router_tops.layers.{layer}.scores")
            tids = otensor(f"{case}.replay.router_tops.layers.{layer}.ids")
        n = min(len(x), len(oids))
        s = scores64(w, b, x[:n])
        f64_top = np.argsort(-s, axis=1)[:, :6]
        srt = -np.sort(-s, axis=1)
        f64_gap = srt[:, 5] - srt[:, 6]
        same = lambda a, c: np.array([set(a[i]) == set(c[i]) for i in range(n)])
        local_same = same(ids[:n], f64_top)
        prop_same = same(f64_top, oids[:n])
        run_same = same(ids[:n], oids[:n])
        # Connected score error on the retained experts.
        err = np.abs(np.take_along_axis(s, tids[:n].astype(np.int64), 1) - tops[:n].astype(np.float64)).max(1)
        changed = []
        for i in np.nonzero(~run_same)[0]:
            o, c = set(oids[i].tolist()), set(ids[i].tolist())
            removed, added = sorted(o - c), sorted(c - o)
            ret = dict(zip(tids[i].tolist(), tops[i].astype(np.float64).tolist()))
            gap = float(tops[i][5] - tops[i][6])
            if all(e in ret for e in removed + added):
                regret = sum(ret[e] for e in removed) - sum(ret[e] for e in added)
            else:
                regret = None
            changed.append({"row": int(i), "removed": removed, "added": added, "oracle_gap": gap,
                            "regret": regret, "row_score_error": float(err[i]),
                            "local_same": bool(local_same[i]), "f64_gap": float(f64_gap[i])})
        resolved = [c for c in changed if c["regret"] is not None]
        entry = {
            "layer": layer, "mode": mode, "rows": int(n),
            "runtime_vs_oracle_changed": int((~run_same).sum()),
            "local_changed": int((~local_same).sum()),
            "local_changed_f64_gap": pct(f64_gap[~local_same]),
            "propagated_changed": int((~prop_same).sum()),
            "connected_score_error": pct(err),
            "changed_rows_resolved": len(resolved),
            "changed_rows_outside_retained": len(changed) - len(resolved),
            "changed_oracle_gap": pct([c["oracle_gap"] for c in changed]),
            "changed_regret": pct([c["regret"] for c in resolved]),
            "changed_regret_over_2_score_error": int(sum(
                1 for c in resolved if c["regret"] > 2 * c["row_score_error"] * len(c["removed"]))),
            "examples": changed[:8],
        }
        # Common input at the oracle's labeled rows.
        common = []
        for label, row in labels.items():
            xo = otensor(f"{case}.{mode}.{label}.layers.{layer}.ffn.gate.in")
            bo = otensor(f"{case}.{mode}.{label}.layers.{layer}.ffn.gate.biased_scores")
            if xo is None or bo is None or row >= n:
                continue
            so = scores64(w, b, xo.reshape(1, -1))[0]
            sr = s[row]
            diff = xo.reshape(-1) != x[row]
            common.append({
                "label": label, "row": int(row),
                "oracle_local_score_error": float(np.abs(so - bo.astype(np.float64)).max()),
                "input_values_differing": int(diff.sum()),
                "input_max_abs_diff": float(np.abs(xo.reshape(-1) - x[row]).max()),
                "propagated_score_error": float(np.abs(sr - so).max()),
                "oracle_ids_from_common_input_f64": sorted(np.argsort(-so)[:6].tolist()) == sorted(oids[row].tolist()),
                "runtime_same_as_oracle": bool(run_same[row]),
            })
        entry["common_input"] = common
        report["layers"].append(entry)
        print(json.dumps({k: entry[k] for k in entry if k not in ("examples", "common_input")}), flush=True)

out.write_text(json.dumps(report, indent=1) + "\n")
