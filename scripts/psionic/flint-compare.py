#!/usr/bin/env python3
"""Compare native and eager-reference Flint logits on a common token prefix.

Diagnostic limits fixed before reading the first full-checkpoint comparison:
top-1 agrees, TV <= 0.02, and centered-logit maximum error <= 0.25 at every
position. The logit allowance is two BF16 ULPs at magnitude 16; these are
short-prefix regression limits, not a broad model-quality qualification.
"""
import argparse
import json
import math
from pathlib import Path

p = argparse.ArgumentParser(description=__doc__)
p.add_argument("native", type=Path)
p.add_argument("reference", type=Path)
p.add_argument("output", type=Path)
args = p.parse_args()
rows = []
native = [json.loads(s) for s in args.native.read_text().splitlines()]
reference = [json.loads(s) for s in args.reference.read_text().splitlines()]
assert len(native) == len(reference) and len(native) > 0
for a, b in zip(native, reference):
    assert (a["position"], a["token"]) == (b["position"], b["token"])
    x, y = a["logits"], b["logits"]
    assert len(x) == len(y) == 150144 and all(math.isfinite(t) for t in x+y)
    mx, my = sum(x)/len(x), sum(y)/len(y)
    error = max(abs((i-mx)-(j-my)) for i, j in zip(x, y))
    def softmax(v):
        largest = max(v)
        e = [math.exp(t-largest) for t in v]
        total = sum(e)
        return [t/total for t in e]
    px, py = softmax(x), softmax(y)
    tv = sum(abs(i-j) for i, j in zip(px, py))*.5
    tx, ty = max(range(len(x)), key=x.__getitem__), max(range(len(y)), key=y.__getitem__)
    passed = tx == ty and tv <= .02 and error <= .25
    rows.append(dict(position=a["position"], token=a["token"], native_top1=tx, reference_top1=ty,
                     total_variation=tv, centered_logit_max_abs=error, passed=passed))
report = dict(policy=dict(top1="equal", total_variation_max=.02, centered_logit_max_abs=.25),
              positions=len(rows), passed=all(r["passed"] for r in rows), rows=rows)
args.output.write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps(report, indent=2))
assert report["passed"], "full-checkpoint diagnostic failed"
