#!/usr/bin/env python3
"""Recompute retained evidence without calling a model or reading credentials."""

import hashlib
import json
import math
from pathlib import Path
import statistics


ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent


def read(relative):
    data = (ROOT / relative).read_bytes()
    return data, [json.loads(line) for line in data.splitlines() if line.strip()]


def latency(rows):
    values = sorted(row["latency_ms"] for row in rows)
    return {
        "n": len(values),
        "mean_ms": statistics.mean(values),
        "median_ms": statistics.median(values),
        "p95_nearest_rank_ms": values[math.ceil(len(values) * 0.95) - 1],
        "max_ms": max(values),
    }


def evidence(relative, suite_path):
    data, records = read(relative)
    rows = [row for row in records if "item" in row]
    suite_bytes = (ROOT / suite_path).read_bytes()
    items = {item["id"]: item for item in json.loads(suite_bytes)["items"]}
    if len(rows) != len(items) or {row["item"] for row in rows} != set(items):
        raise ValueError("Evidence records do not cover the suite exactly once.")
    discrepancies = []
    rescored = []
    for row in rows:
        truth = items[row["item"]]["truth"]
        correct = row["chosen_path"] == truth["chosen"] and (
            truth["chosen"] is None or row["chosen_span"] == truth.get("span"))
        any_agree = (row["any_relevant_noul"] >= 0.5) == truth["any_relevant"]
        coverage_agree = (row["coverage_noul"] >= 0.5) == truth["coverage"]
        if (correct, any_agree, coverage_agree) != (
                row["correct"], row["any_relevant_agree"], row["coverage_agree"]):
            discrepancies.append({
                "item": row["item"], "recorded_correct": row["correct"],
                "recomputed_correct": correct, "current_truth": truth,
                "chosen_path": row["chosen_path"], "chosen_span": row["chosen_span"],
            })
        rescored.append({**row, "correct": correct,
                         "any_relevant_agree": any_agree, "coverage_agree": coverage_agree})
    errors = [
        {"item": row["item"], "choice": row["choice"],
         "selected_probability": row["probabilities"][row["choice"]]}
        for row in rescored if not row["correct"]
    ]
    return {
        "source": relative,
        "source_sha256": hashlib.sha256(data).hexdigest(),
        "suite": suite_path,
        "suite_sha256": hashlib.sha256(suite_bytes).hexdigest(),
        "recorded_grades_match_current_suite": not discrepancies,
        "grade_discrepancies": discrepancies,
        "items": len(rows),
        "recorded_correct": sum(row["correct"] for row in rows),
        "recomputed_correct": sum(row["correct"] for row in rescored),
        "recomputed_errors": errors,
        "recomputed_wrong_with_selected_probability_at_least_0_8": sum(
            row["selected_probability"] >= 0.8 for row in errors),
        "any_relevant_agreement": sum(row["any_relevant_agree"] for row in rows),
        "coverage_agreement": sum(row["coverage_agree"] for row in rows),
        "input_tokens": sum(row["input_tokens"] for row in rows),
        "output_tokens": sum(row["output_tokens"] for row in rows),
        "latency": latency(rows),
        "historical_billed_usd": None,
        "threshold_sweep_recomputed_descriptive_only": [
            {
                "floor": floor,
                "correct_kept": sum(row["correct"] and row["probabilities"][row["choice"]] >= floor for row in rescored),
                "correct_lost": sum(row["correct"] and row["probabilities"][row["choice"]] < floor for row in rescored),
                "errors_deferred": sum(not row["correct"] and row["probabilities"][row["choice"]] < floor for row in rescored),
                "errors_kept": sum(not row["correct"] and row["probabilities"][row["choice"]] >= floor for row in rescored),
            }
            for floor in (0.0, 0.5, 0.6, 0.7, 0.8, 0.9, 0.95)
        ],
    }


def review():
    relative = "docs/coder/measurements/2026-09-22-review-finding.raw.jsonl"
    data, records = read(relative)
    rows = [row for row in records if "item" in row]
    findings = [dict(item=row["item"], finding=name, **finding)
                for row in rows for name, finding in row["findings"].items()]
    suite_path = "crates/gym/suites/review-finding-v1.json"
    suite_bytes = (ROOT / suite_path).read_bytes()
    truths = {(item["id"], name): truth
              for item in json.loads(suite_bytes)["items"]
              for name, truth in item["truth"].items()}
    if len(findings) != len(truths):
        raise ValueError("Finding count does not match the suite.")
    for finding in findings:
        truth = truths[(finding["item"], finding["finding"])]
        if finding["truth"] != truth or finding["agree"] != ((finding["noul"] >= 0.5) == truth):
            raise ValueError("Recorded finding label or grade disagrees with current suite.")
    thresholds = []
    for cutoff in (0.1, 0.2, 0.3, 0.4, 0.5):
        dropped = [finding for finding in findings if finding["noul"] < cutoff]
        thresholds.append({
            "dismiss_below": cutoff,
            "dismissed_genuine": sum(finding["truth"] for finding in dropped),
            "dismissed_false_positive": sum(not finding["truth"] for finding in dropped),
        })
    return {
        "source": relative,
        "source_sha256": hashlib.sha256(data).hexdigest(),
        "suite": suite_path,
        "suite_sha256": hashlib.sha256(suite_bytes).hexdigest(),
        "recorded_grades_checked_against_current_suite": True,
        "items": len(rows), "findings": len(findings),
        "genuine": sum(finding["truth"] for finding in findings),
        "false_positive": sum(not finding["truth"] for finding in findings),
        "correct_at_0_5": sum((finding["noul"] >= 0.5) == finding["truth"]
                                for finding in findings),
        "errors": [finding for finding in findings if not finding["agree"]],
        "threshold_sweep_descriptive_only": thresholds,
        "latency": latency(rows),
        "historical_billed_usd": None,
    }


def fanout():
    relative = "crates/coderbench/goldens/devin-fan-out-six.atif.jsonl"
    data, records = read(relative)
    calls = [row["step"]["call"] for row in records
             if "call" in row.get("step", {})]
    decisions = [call for call in calls
                 if call.get("extra", {}).get("schema") == "openagents.decision-call.v1"]
    return {
        "source": relative,
        "source_sha256": hashlib.sha256(data).hexdigest(),
        "decision_durations_ms": {call["name"]: call["milliseconds"] for call in decisions},
        "decision_duration_sum_ms": sum(call["milliseconds"] for call in decisions),
        "capability_probe_ms": [call["milliseconds"] for call in calls
                                if call["name"] == "capability_probe"],
        "longest_delegate_ms": max(call["milliseconds"] for call in calls
                                   if call["name"] == "delegate"),
        "note": "Concurrent delegate durations must not be summed as elapsed time.",
    }


def main():
    report = {
        "kind": "offline_reanalysis_of_retained_records",
        "new_model_calls": 0,
        "quantiles": "Median averages the central pair; p95 uses nearest rank.",
        "evidence_heldout": evidence(
            "docs/coder/measurements/2026-09-21-evidence-select.raw.jsonl",
            "crates/gym/suites/evidence-select-v1.json"),
        "evidence_development": evidence(
            "docs/coder/measurements/2026-09-22-evidence-select-dev.raw.jsonl",
            "crates/gym/suites/evidence-select-dev-v1.json"),
        "review_findings": review(),
        "observed_fanout": fanout(),
        "economics_assumptions": {
            "jev_usd_per_million_input_tokens": 0.042,
            "source": "https://docs.typesafe.ai/models",
            "input_tokens_to_estimated_usd": {
                str(tokens): tokens * 0.042 / 1_000_000
                for tokens in (2000, 6000, 12000, 18000, 24000, 48000)
            },
            "note": "Current-price estimates, not observed charges.",
        },
    }
    (HERE / "audit.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
