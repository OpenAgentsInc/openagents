#!/usr/bin/env python3
"""Recompute lifecycle component results from retained public artifacts."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import statistics


def load(path):
    return json.loads(path.read_text())


def reviews(root, manifest, focused=False, directory_name=None):
    rows = []
    for case in manifest["review_cases"]:
        directory = root / "runs" / (directory_name or ("review-focused" if focused else "review")) / case["id"]
        if not directory.exists():
            continue
        receipt = load(directory / "receipt.json")
        response = load(directory / "response.json")
        answers = response.get("answers", {}) if receipt.get("answers_valid") else {}
        if focused:
            probabilities = {k: v["noul"] for k, v in answers.items()}
            flag = any(p < .5 for p in probabilities.values()) if probabilities else None
            prediction = "revise" if flag else "verify" if flag is not None else "invalid"
        else:
            probabilities = {"material_defect": answers["material_defect"]["noul"]} if answers else {}
            prediction = answers["next_action"]["choice"] if answers else "invalid"
            flag = probabilities.get("material_defect", 0) >= .5 if answers else None
        rows.append({"id": case["id"], "family": case["family"], "label": case["label"],
                     "action": prediction, "probability_flag": flag, "probabilities": probabilities,
                     "api_wall_s": receipt["wall_s"], "cost_usd": receipt["cost_usd"]})
    if not rows:
        return None
    defective = [r for r in rows if r["label"] == "defective"]
    accepted = [r for r in rows if r["label"] == "accepted_under_retained_checks"]
    return {"cases": rows, "defective_cases": len(defective), "accepted_cases": len(accepted),
            "defects_flagged": sum(r["action"] == "revise" for r in defective),
            "accepted_flagged": sum(r["action"] == "revise" for r in accepted),
            "abstentions": sum(r["action"] in ("inspect", "invalid") for r in rows),
            "probability_defects_flagged": sum(r["probability_flag"] is True for r in defective),
            "probability_accepted_flagged": sum(r["probability_flag"] is True for r in accepted),
            "api_wall_s": sum(r["api_wall_s"] for r in rows),
            "median_api_wall_s": statistics.median(r["api_wall_s"] for r in rows),
            "cost_usd": sum(r["cost_usd"] for r in rows) if all(r["cost_usd"] is not None for r in rows) else None}


def compute(root):
    manifest = load(root / "review-cases.json")
    receipts, providers, provider_attempts, internal_failovers = [], Counter(), [], []
    for path in sorted(root.rglob("receipt.json")):
        receipt = load(path)
        for name, field in (("request.json", "request_sha256"), ("response.json", "response_sha256")):
            if receipt.get(field):
                assert hashlib.sha256((path.parent / name).read_bytes()).hexdigest() == receipt[field], str(path)
        receipts.append({"path": str(path.relative_to(root)), **receipt})
        meta = receipt.get("gateway_metadata", receipt.get("gateway", {})) or {}
        providers[meta.get("routing", {}).get("finalProvider", "unknown")] += 1
        attempts = meta.get("routing", {}).get("totalProviderAttemptCount")
        provider_attempts.append(attempts)
        if isinstance(attempts, int) and attempts > 1:
            internal_failovers.append(str(path.relative_to(root)))
    next_rows = []
    for case in manifest["next_read_cases"]:
        directory = root / "runs/next-read" / case["id"]
        response = load(directory / "response.json")
        answer = response["answers"]["next_read"]["choice"]
        next_rows.append({"id": case["id"], "expected": case["expected_choice"], "actual": answer,
                          "correct": answer == case["expected_choice"]})
    batching = load(root / "runs/batching/result.json")
    totals = {condition: {field: sum(b["conditions"][condition][field] for b in batching["blocks"])
                         for field in ("cost_usd", "api_wall_s")}
              for condition in ("batch", "sequential")}
    ratios = {field: totals["batch"][field] / totals["sequential"][field]
              for field in ("cost_usd", "api_wall_s")}
    return {
        "schema": "openagents.jev.lifecycle-results.v1", "scored_native_sessions": 0,
        "calls": len(receipts), "http_attempts": sum(r["attempts"] for r in receipts),
        "valid_calls": sum(r.get("answers_valid", r.get("answer_identity_and_types_match", False)) for r in receipts),
        "reported_cost_usd": sum(r["cost_usd"] for r in receipts) if all(r["cost_usd"] is not None for r in receipts) else None,
        "unknown_cost_calls": [r["path"] for r in receipts if r["cost_usd"] is None],
        "summed_api_wall_s": sum(r["wall_s"] for r in receipts), "final_providers": dict(providers),
        "provider_attempts": sum(provider_attempts) if all(type(x) is int for x in provider_attempts) else None,
        "gateway_internal_failovers": internal_failovers,
        "review": reviews(root, manifest), "focused_review": reviews(root, manifest, focused=True),
        "corrected_order_review": reviews(root, manifest, focused=True, directory_name="review-order-corrected"),
        "next_read": {"cases": next_rows, "correct": sum(r["correct"] for r in next_rows),
                      "count": len(next_rows), "label_kind": "expert public-contract relevance, not coding acceptance"},
        "batching": {"totals": totals, "batch_over_sequential": ratios,
                     "exact_answer_agreement": [b["exact_answer_agreement"] for b in batching["blocks"]]},
        "receipt_paths": [r["path"] for r in receipts],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--artifacts", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    result = compute(args.artifacts)
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({k: result[k] for k in ("calls", "valid_calls", "reported_cost_usd", "scored_native_sessions")}))


if __name__ == "__main__":
    main()
