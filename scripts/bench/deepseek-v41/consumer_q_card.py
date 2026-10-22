#!/usr/bin/env python3
"""Score teacher-forced logits against the oracle's (#12465, the consumer-q
quality card).

    consumer_q_card.py ORACLE_DIR OUT CASE=CAPTURE_DIR[,LABEL] ...

ORACLE_DIR holds the oracle dump's `manifest.json`, each case's `run.json`
(as CASE/run.json), and the safetensors files the manifest names for the
cases' `CASE.decode.gN.logits` tensors. Each CAPTURE_DIR is a
`psionic-v41-stage chain --capture` folder from a run that took the
oracle's prompt ids (`--prompt-ids`) and was forced through the oracle's
generated ids (`--force-ids`), so its k-th answer step lines up with the
oracle's `g(k+1)`. LABEL names the runtime (for example `consumer-q-q2q3` or
`exact`); runs with the same label are pooled.

For every position the card reports the KL divergence from the oracle's
distribution (temperature 1, natural log), the total variation, whether
the top token agrees, and the log-probability the run gives the oracle's
own next token minus the oracle's. It is a quality card for a lossy,
unqualified profile, not a G1 v2 verdict.
"""
import json
import math
import struct
import sys
from pathlib import Path

import numpy as np


def safetensor(path, name):
    with open(path, "rb") as f:
        n = struct.unpack("<Q", f.read(8))[0]
        header = json.loads(f.read(n))
        meta = header[name]
        start, end = meta["data_offsets"]
        f.seek(8 + n + start)
        return np.frombuffer(f.read(end - start), dtype=np.float32).astype(np.float64)


def log_softmax(x):
    if x.ndim != 1 or not x.size or not np.isfinite(x).all():
        raise ValueError("Logits must be a nonempty finite vector")
    m = np.max(x)
    return x - m - math.log(np.sum(np.exp(x - m)))


def captured(folder):
    """The run's answer-step logits in step order."""
    steps = []
    for path in Path(folder).glob("step-*.logits.f32"):
        steps.append((int(path.name.split("-")[1].split(".")[0]), path))
    steps.sort()
    if not steps or [step for step, _ in steps] != list(range(len(steps))):
        raise ValueError(f"{folder}: captures must start at step 0 and have no gaps")
    return [np.fromfile(p, dtype=np.float32).astype(np.float64) for _, p in steps]


def main():
    oracle_dir = Path(sys.argv[1])
    out = Path(sys.argv[2])
    manifest = json.loads((oracle_dir / "manifest.json").read_text())
    files = {t["name"]: t["file"] for t in manifest["tensors"]}
    pooled = {}
    rows = []
    for spec in sys.argv[3:]:
        case, rest = spec.split("=", 1)
        folder, _, label = rest.partition(",")
        label = label or "run"
        run = json.loads((oracle_dir / case / "run.json").read_text())
        generated = run["generated_ids"]
        logits = captured(folder)
        for k, q in enumerate(logits):
            name = f"{case}.decode.g{k + 1}.logits"
            if name not in files or k >= len(generated):
                raise ValueError(f"{case}: no matching oracle position for capture step {k}")
            o = safetensor(oracle_dir / files[name], name)
            if o.shape != q.shape:
                raise ValueError(f"{case}: oracle and capture vocabulary sizes differ")
            lo, lq = log_softmax(o), log_softmax(q)
            po, pq = np.exp(lo), np.exp(lq)
            kl = float(np.sum(po * (lo - lq)))
            tv = float(0.5 * np.sum(np.abs(po - pq)))
            ref = generated[k]
            if not isinstance(ref, int) or not 0 <= ref < len(o):
                raise ValueError(f"{case}: oracle token is outside the vocabulary")
            row = {
                "label": label,
                "case": case,
                "position": k + 1,
                "kl": kl,
                "tv": tv,
                "top1_agrees": int(np.argmax(o)) == int(np.argmax(q)),
                "ref_logprob_error": float(lq[ref] - lo[ref]),
            }
            rows.append(row)
            pooled.setdefault(label, []).append(row)
    summary = {}
    for label, rs in pooled.items():
        kls = np.array([r["kl"] for r in rs])
        summary[label] = {
            "positions": len(rs),
            "kl_mean": float(kls.mean()),
            "kl_p90": float(np.quantile(kls, 0.9)),
            "kl_max": float(kls.max()),
            "tv_mean": float(np.mean([r["tv"] for r in rs])),
            "top1_agreement": float(np.mean([r["top1_agrees"] for r in rs])),
            "ref_logprob_error_mean": float(np.mean([r["ref_logprob_error"] for r in rs])),
            "cases": sorted({r["case"] for r in rs}),
        }
    card = {
        "schema": "openagents.deepseek_v41.consumer_q_card.v1",
        "issue": 12465,
        "lossy": True,
        "qualified": False,
        "note": "Teacher-forced decode logits against the oracle's at the same positions. consumer-q is lossy and unqualified.",
        "summary": summary,
        "rows": rows,
    }
    out.write_text(json.dumps(card, indent=1) + "\n")
    print(json.dumps(summary, indent=1))


if __name__ == "__main__":
    main()
