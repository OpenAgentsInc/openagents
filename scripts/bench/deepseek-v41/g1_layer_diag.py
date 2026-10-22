#!/usr/bin/env python3
"""Where the served runtime's early layers first differ from the oracle (#12405).

    g1_layer_diag.py RECORD ORACLE CASE OUT

RECORD is a `qualify --observe` record folder for CASE; ORACLE is the oracle
dump. For each captured label in layers 0 and 1, compares the runtime's
observed sites with the oracle's tensors: the attention mixes, the attention
input and output, the expanded stream, the FFN mixes, and the router input.
Reports, per site, how many values differ and by how much.
"""
import json
import sys
from pathlib import Path

import numpy as np
from safetensors import safe_open

record, oracle, case, out = Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3], Path(sys.argv[4])
run = json.loads((oracle / case / "run.json").read_text())
prompt = run["prompt_tokens"]
files = [safe_open(str(oracle / case / f"tensors-rank{r}.safetensors"), "np") for r in range(8)]
keys = [set(f.keys()) for f in files]


def oracle_tensor(name):
    for f, k in zip(files, keys):
        if name in k:
            t = f.get_tensor(name)
            return t.astype(np.float32).reshape(-1)
    return None


def site(step, layer, name, row, tokens):
    hits = sorted(record.glob(f"{case}-0-{step}*.L{layer}.{name}.u32"))
    if not hits:
        return None
    words = np.concatenate([np.fromfile(h, dtype="<u4") for h in hits])
    per = words.size // tokens
    return words[row * per:(row + 1) * per].view(np.float32)


SITES = [
    ("AttnMixes", ["hc.attn.pre", "hc.attn.post", "hc.attn.comb"]),
    ("AttnIn", ["attn.in"]),
    ("AttnOut", ["attn.out"]),
    ("AttnExpanded", ["hc.attn.expanded"]),
    ("FfnMixes", ["hc.ffn.pre", "hc.ffn.post", "hc.ffn.comb"]),
    ("FfnIn", ["ffn.gate.in"]),
]
rows = []
for label, position in run["labels"].items():
    step, row, tokens = (0, position, prompt) if position < prompt else (position - prompt + 1, 0, 1)
    for layer in (0, 1):
        for name, parts in SITES:
            ref = [oracle_tensor(f"{case}.decode.{label}.layers.{layer}.{p}") for p in parts]
            if any(r is None for r in ref):
                continue
            ref = np.concatenate(ref)
            got = site(step, layer, name, row, tokens)
            if got is None or got.size != ref.size:
                rows.append({"label": label, "layer": layer, "site": name, "missing": True})
                continue
            diff = np.abs(got.astype(np.float64) - ref.astype(np.float64))
            rows.append({"label": label, "layer": layer, "site": name, "values": int(ref.size),
                         "unequal": int((got != ref).sum()), "max_abs": float(diff.max()),
                         "max_rel": float((diff / np.maximum(np.abs(ref), 1e-30)).max())})
out.write_text(json.dumps(rows, indent=1) + "\n")
for r in rows:
    print(r)
