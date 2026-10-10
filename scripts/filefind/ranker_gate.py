#!/usr/bin/env python3
"""The filefind ranker's activation gate (LEARN-01, audit A12).

`retrain.sh` never writes the shipped model. It trains a candidate into an
immutable, digest-named file; `file-finding-bench.py compare` evaluates the
active model and the candidate on the same held-out cases under the frozen
plan below and writes a receipt; only `promote` changes the active model, and
only when that receipt passed, names this exact candidate, and was measured
against the model that is active now.

    python3 scripts/filefind/ranker_gate.py promote --candidate C --receipt R
    python3 scripts/filefind/ranker_gate.py status

Overlap: a model whose training cases include any evaluation case is refused
as a candidate (`compare` and `eval` stop; `eval --allow-overlap` labels its
numbers as development). A baseline that overlaps is recorded on the receipt:
its numbers are optimistic, which only makes the gate harder to pass.
"""
import argparse
import hashlib
import json
import math
import os
import re
import shutil
import statistics
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ACTIVE = os.path.join(HERE, "model.json")
RECEIPT_SCHEMA = "openagents.filefind.ranker-compare.v1"
# The frozen comparison plan. Changing it changes PLAN_DIGEST, and a receipt
# measured under another plan cannot promote.
PLAN = {
    "primary": {"metric": "recall", "k": 50, "min_gain_se": 2.0},
    "guards": [{"metric": "recall", "k": 20, "max_loss_se": 2.0},
               {"metric": "recall", "k": 100, "max_loss_se": 2.0},
               {"metric": "brier_top100", "max_rise_se": 2.0}],
    "pairing": "per evaluation case, the same features for both models",
}
PLAN_DIGEST = "sha256:" + hashlib.sha256(json.dumps(PLAN, sort_keys=True).encode()).hexdigest()


def digest_file(path):
    with open(path, "rb") as f:
        return "sha256:" + hashlib.sha256(f.read()).hexdigest()


def trained_issues(model):
    """The issue numbers a model was trained on: its list, or for a model card
    with only a range (`#1..#11208`), every number in the range."""
    t = model.get("trained_on") or {}
    if t.get("issue_list") is not None:
        return set(int(n) for n in t["issue_list"])
    m = re.match(r"^#(\d+)\.\.#(\d+)$", str(t.get("issues", "")))
    if m:
        return range(int(m.group(1)), int(m.group(2)) + 1)
    return None  # unknown: treated as overlapping everything


def overlap(model, issues):
    """Evaluation issues the model was trained on (all of them when unknown)."""
    seen = trained_issues(model)
    if seen is None:
        return sorted(issues)
    return sorted(n for n in issues if n in seen)


def _paired(base, cand):
    d = [c - b for b, c in zip(base, cand)]
    mean = statistics.mean(d) if d else 0.0
    se = statistics.stdev(d) / math.sqrt(len(d)) if len(d) > 1 else float("inf")
    return {"baseline": statistics.mean(base) if base else 0.0, "candidate": statistics.mean(cand) if cand else 0.0,
            "diff": mean, "se": se}


def decide(base_rows, cand_rows):
    """The plan's verdict on paired per-case rows ({issue, n, r20, r50, r100,
    brier_top100}). Pass needs the primary gain >= 2 SE and every guard held."""
    by = {r["issue"]: r for r in base_rows}
    pairs = [(by[r["issue"]], r) for r in cand_rows if r["issue"] in by and r["n"]]
    if len(pairs) != len(base_rows) or len(pairs) != len(cand_rows):
        return {"pass": False, "why": ["the two models were not measured on the same cases"], "metrics": {}}

    def series(key):
        if key.startswith("brier"):
            return [b[key] for b, _ in pairs], [c[key] for _, c in pairs]
        return [b[key] / b["n"] for b, _ in pairs], [c[key] / c["n"] for _, c in pairs]
    metrics, why = {}, []
    p = PLAN["primary"]
    m = metrics[f"recall@{p['k']}"] = _paired(*series(f"r{p['k']}"))
    if not (m["diff"] > 0 and m["diff"] >= p["min_gain_se"] * m["se"]):
        why.append(f"recall@{p['k']} gain {m['diff']:+.4f} is under {p['min_gain_se']} SE ({m['se']:.4f})")
    for g in PLAN["guards"]:
        if g["metric"] == "recall":
            m = metrics[f"recall@{g['k']}"] = _paired(*series(f"r{g['k']}"))
            if m["diff"] < -g["max_loss_se"] * m["se"]:
                why.append(f"recall@{g['k']} fell {m['diff']:+.4f} (> {g['max_loss_se']} SE)")
        else:
            m = metrics[g["metric"]] = _paired(*series(g["metric"]))
            if m["diff"] > g["max_rise_se"] * m["se"]:
                why.append(f"{g['metric']} rose {m['diff']:+.4f} (> {g['max_rise_se']} SE): calibration regressed")
    return {"pass": not why, "why": why, "metrics": metrics, "cases": len(pairs)}


def receipt(baseline_path, candidate_path, eval_issues, base_rows, cand_rows, baseline_overlap):
    verdict = decide(base_rows, cand_rows)
    return {
        "v": RECEIPT_SCHEMA, "at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "plan": PLAN, "plan_digest": PLAN_DIGEST,
        "baseline": {"path": os.path.abspath(baseline_path), "digest": digest_file(baseline_path),
                     "overlap_with_eval": len(baseline_overlap)},
        "candidate": {"path": os.path.abspath(candidate_path), "digest": digest_file(candidate_path),
                      "overlap_with_eval": 0},
        "eval_issues": sorted(eval_issues),
        "eval_digest": "sha256:" + hashlib.sha256(json.dumps(sorted(eval_issues)).encode()).hexdigest(),
        **verdict,
    }


def immutable_copy(model_path, dest_dir):
    """Copy a trained model to dest_dir/model-<digest12>.json, never overwriting
    another file under that name. Returns the path."""
    d = digest_file(model_path)
    os.makedirs(dest_dir, exist_ok=True)
    dest = os.path.join(dest_dir, f"model-{d.split(':')[1][:12]}.json")
    if os.path.exists(dest):
        if digest_file(dest) != d:
            raise SystemExit(f"{dest} exists with other content; refusing to overwrite a candidate")
        return dest
    tmp = f"{dest}.{os.getpid()}.tmp"
    shutil.copyfile(model_path, tmp)
    os.chmod(tmp, 0o444)
    os.replace(tmp, dest)
    return dest


class Refused(SystemExit):
    pass


def promote(candidate_path, receipt_path, active_path=ACTIVE):
    """Make the candidate the active model, atomically, when its receipt allows
    it. Anything else leaves the active model byte-for-byte unchanged."""
    with open(receipt_path) as f:
        r = json.load(f)
    problems = []
    if r.get("v") != RECEIPT_SCHEMA:
        problems.append("not a comparison receipt")
    if r.get("plan_digest") != PLAN_DIGEST:
        problems.append("the receipt was measured under another plan")
    if not r.get("pass"):
        problems.append("the comparison did not pass: " + "; ".join(r.get("why") or []))
    if r.get("candidate", {}).get("digest") != digest_file(candidate_path):
        problems.append("the receipt names another candidate")
    if r.get("candidate", {}).get("overlap_with_eval"):
        problems.append("the candidate was trained on evaluation cases")
    if os.path.exists(active_path) and r.get("baseline", {}).get("digest") != digest_file(active_path):
        problems.append("the receipt compared against a model that is no longer active")
    if problems:
        raise Refused("promote refused: " + "; ".join(problems))
    d = os.path.dirname(os.path.abspath(active_path))
    tmp = os.path.join(d, f".model.{os.getpid()}.tmp")
    shutil.copyfile(candidate_path, tmp)
    os.chmod(tmp, 0o644)
    side = os.path.splitext(active_path)[0] + ".receipt.json"
    stmp = side + f".{os.getpid()}.tmp"
    with open(stmp, "w") as f:
        json.dump(dict(r, promoted_at=time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())), f, indent=1, sort_keys=True)
    os.replace(tmp, active_path)
    os.replace(stmp, side)
    return r


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("promote")
    p.add_argument("--candidate", required=True)
    p.add_argument("--receipt", required=True)
    p.add_argument("--model", default=ACTIVE, help="the active model path")
    s = sub.add_parser("status")
    s.add_argument("--model", default=ACTIVE)
    a = ap.parse_args()
    if a.cmd == "promote":
        r = promote(a.candidate, a.receipt, a.model)
        key = "recall@%d" % PLAN["primary"]["k"]
        print(f"promoted {r['candidate']['digest']} -> {a.model} ({key} {r['metrics'][key]['diff']:+.4f})")
    else:
        with open(a.model) as f:
            m = json.load(f)
        print(f"active {digest_file(a.model)}  trained on {m.get('trained_on')}")
        side = os.path.splitext(a.model)[0] + ".receipt.json"
        print(f"receipt {side}" if os.path.exists(side) else "no promotion receipt (model predates the gate)")


if __name__ == "__main__":
    sys.exit(main())
