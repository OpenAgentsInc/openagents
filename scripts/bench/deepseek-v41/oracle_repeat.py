#!/usr/bin/env python3
"""Compare a second oracle capture with the original dump, same mode against same mode (#12404).

Reads two `oracle_manifest.py` manifests and the `run.json` of each case, and reports, for
every name both runs recorded: tensor SHA-256 equality, per-layer residual hashes, logits
hashes, and generated tokens. A diagnostic capture adds tensors and rows; those have no
counterpart and are only counted. Nothing compares one mode with another.

    oracle_repeat.py --original ORIG_DIR --repeat NEW_DIR --out REPORT.json

Each DIR holds manifest.json and CASE/run.json, as in the bucket.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--original", type=Path, required=True)
    ap.add_argument("--repeat", type=Path, required=True)
    ap.add_argument("--out", type=Path, required=True)
    a = ap.parse_args()
    orig = {t["name"]: t for t in json.loads((a.original / "manifest.json").read_text())["tensors"]}
    new = {t["name"]: t for t in json.loads((a.repeat / "manifest.json").read_text())["tensors"]}
    common = sorted(set(orig) & set(new))
    unequal = [n for n in common if orig[n]["sha256"] != new[n]["sha256"]]
    report = {
        "schema": "openagents.deepseek_v41.oracle_repeat.v1",
        "tensors": {"original": len(orig), "repeat": len(new), "common": len(common),
                    "equal": len(common) - len(unequal), "unequal": unequal[:2000],
                    "unequal_count": len(unequal), "missing_from_repeat": sorted(set(orig) - set(new))[:2000],
                    "missing_from_repeat_count": len(set(orig) - set(new)),
                    "added_by_repeat": len(set(new) - set(orig))},
        "cases": {},
    }
    for case in sorted(p.name for p in a.original.iterdir() if (p / "run.json").exists()):
        r0 = json.loads((a.original / case / "run.json").read_text())
        path = a.repeat / case / "run.json"
        if not path.exists():
            report["cases"][case] = {"repeated": False}
            continue
        r1 = json.loads(path.read_text())
        h0, h1 = r0.get("residual_sha256", {}), r1.get("residual_sha256", {})
        both = sorted(set(h0) & set(h1))
        bad_hash = [k for k in both if h0[k] != h1[k]]
        s0 = {s["step"]: s for s in r0.get("steps", [])}
        s1 = {s["step"]: s for s in r1.get("steps", [])}
        bad_logits = [k for k in sorted(set(s0) & set(s1)) if s0[k]["logits_sha256"] != s1[k]["logits_sha256"]]
        report["cases"][case] = {
            "repeated": True,
            "generated_ids_equal": r0.get("generated_ids") == r1.get("generated_ids"),
            "replay_last_argmax_equal": r0.get("replay_last_argmax") == r1.get("replay_last_argmax"),
            "residual_hashes": {"common": len(both), "unequal": len(bad_hash),
                                "first_unequal": sorted(bad_hash, key=lambda k: (k.split(".")[1], k))[:20]},
            "decode_logits": {"common": len(set(s0) & set(s1)), "unequal_steps": bad_logits},
            "router_ids_equal": all(orig[n]["sha256"] == new[n]["sha256"] for n in common
                                    if n.startswith(case + ".") and ".router_ids." in n),
            "near_ties_equal": r0.get("near_ties") == r1.get("near_ties"),
        }
    a.out.write_text(json.dumps(report, indent=1) + "\n")
    t = report["tensors"]
    print(f"tensors: {t['equal']} of {t['common']} common names equal; {t['unequal_count']} unequal; "
          f"{t['missing_from_repeat_count']} missing from repeat; {t['added_by_repeat']} added")
    for case, c in report["cases"].items():
        if c.get("repeated"):
            print(case, "tokens", c["generated_ids_equal"], "residual unequal",
                  c["residual_hashes"]["unequal"], "of", c["residual_hashes"]["common"],
                  "logit steps unequal", c["decode_logits"]["unequal_steps"])
        else:
            print(case, "not repeated")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
