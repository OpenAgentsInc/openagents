#!/usr/bin/env python3
"""Recompute retained component observations; make no executor-quality claim."""
import hashlib
import json
from pathlib import Path
import statistics


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    here = Path(__file__).resolve().parent
    root = here.parents[3]
    paths = {
        "timings": here / "reserve-preparation-timings.json",
        "freeze": here / "treatment-freeze.json",
        "preflight": root / "bench/delegation-study/infrastructure-preflight.json",
        "provider_calls": root / "bench/delegation-study/infrastructure-provider-receipts.jsonl",
        "jev_refusal": here / "jev-capability-refusal.json",
        "prior_ledger": here.parent / "experiment-costs.json",
    }
    freeze = json.loads(paths["freeze"].read_text())
    for name, expected in freeze["hashes"].items():
        if digest(root / name) != expected:
            raise ValueError("Frozen component changed: " + name)
    tasks, times = [], []
    for task in json.loads(paths["timings"].read_text()):
        preview = here / "previews" / task["task_id"]
        prep = json.loads((preview / "preparation.json").read_text())
        output_hash = digest(preview / "briefing.md")
        pool_hash = digest(preview / "candidates.json")
        if (prep["source_commit"] != task["source_commit"]
                or prep["script_sha256"] != freeze["hashes"]["bench/delegation-study/prepare.py"]
                or output_hash != prep["briefing_sha256"] or pool_hash != prep["candidate_sha256"]
                or task["index_exit_code"] != 0):
            raise ValueError("Preview identity or index outcome mismatch")
        samples = []
        for row in task["timings"]:
            if (row["exit_code"] != 0 or row["briefing_sha256"] != output_hash
                    or row["candidate_sha256"] != pool_hash
                    or row["briefing_bytes"] != (preview / "briefing.md").stat().st_size):
                raise ValueError("Retained repetition output mismatch")
            samples.append(row["process_wall_s"])
        times.extend(samples)
        tasks.append({"task_id": task["task_id"], "source_commit": task["source_commit"],
                      "samples": len(samples), "min_s": min(samples),
                      "median_s": statistics.median(samples), "max_s": max(samples),
                      "under_one_second": sum(v < 1 for v in samples),
                      "index_wall_s": task["index_wall_s"], "briefing_sha256": output_hash,
                      "candidate_sha256": pool_hash, "briefing_bytes": prep["briefing_bytes"]})
    preflight = json.loads(paths["preflight"].read_text())["authenticated_capability_probe"]
    ledger = [json.loads(line) for line in paths["provider_calls"].read_text().splitlines()]
    starts = {r["call_id"]: r for r in ledger if r["phase"] == "admitted"}
    ends = {r["call_id"]: r for r in ledger if r["phase"] == "finished"}
    if len(ledger) != len(starts) + len(ends) or starts.keys() != ends.keys():
        raise ValueError("Duplicate or incomplete capability ledger")
    cost = 0.0
    for identity, row in ends.items():
        if (starts[identity]["model"] != "claude-opus-5-5"
                or row["served_model"] != "claude-opus-5-5"
                or row["usage_status"] != "reported" or row["http_status"] != 200):
            raise ValueError("Unexpected capability identity or usage")
        u = row["usage"]
        if u["cache_creation_input_tokens"] != u["cache_write_5m_tokens"] + u["cache_write_1h_tokens"]:
            raise ValueError("Unknown cache lifetime")
        value = (u["input_tokens"] * 4 + u["output_tokens"] * 20
                 + u["cache_read_input_tokens"] * .2 + u["cache_write_5m_tokens"] * 5
                 + u["cache_write_1h_tokens"] * 8) / 1e6
        if abs(value - row["cost_usd"]) > 1e-9:
            raise ValueError("Provider cost mismatch")
        cost += value
    if abs(cost - preflight["native_cumulative_cost_usd"]) > 1e-9:
        raise ValueError("Native cumulative cost mismatch")
    refusal = json.loads(paths["jev_refusal"].read_text())
    result = {
        "schema": "openagents.delegation.component-results.v1",
        "input_sha256": {name: digest(path) for name, path in paths.items()},
        "scored_executor_sessions": 0,
        "reserve_preparation": {"samples": len(times), "under_one_second": sum(v < 1 for v in times),
                                "same_output_within_each_task": True, "tasks": tasks},
        "capability": {"provider_requests": len(ends), "repriced_known_usd": cost,
                       "model_completed": preflight["model_completed"],
                       "native_result": preflight["native_result"],
                       "pricing_source": "https://platform.claude.com/docs/en/models/opus-5-5/overview",
                       "pricing_checked": "2026-10-03"},
        "jev": {"http_status": refusal["http_status"], "cost_usd": refusal["cost_usd"],
                "cost_status": refusal["cost_status"]},
        "limits": ["Component measurements do not measure executor improvement or quality.",
                   "Warm preview excludes cold indexing, issue fetch, successful inference, and checks.",
                   "Linux reserve timings may include contention from trusted compilation.",
                   "Costs are list-price equivalents, not invoices; audit and machine costs are unmeasured."],
    }
    (here / "component-results.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"preview_samples": len(times), "under_one_second": sum(v < 1 for v in times),
                      "known_capability_usd": cost, "scored_executor_sessions": 0}))


if __name__ == "__main__":
    main()
