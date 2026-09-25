#!/usr/bin/env python3
"""Join only the published supplemental candidates, retaining every unknown."""

import argparse
import json
from pathlib import Path
import random
import statistics

import measure
from supplemental import cohort, digest, read, write


def load(root, published):
    expected = cohort(published)
    inputs = read(root / "inputs.json")
    if digest(published) != inputs["measurement_sha256"]:
        raise ValueError("published measurement differs from the input receipt")
    if inputs["candidates"] != expected:
        raise ValueError("candidate identities or labels changed")
    by_trial = {r["trial"]: r for r in expected}
    results, labels, plans = {}, {}, {}
    for path in sorted((root / "records").glob("*/contract.json")):
        task = path.parent.name
        record = read(path)
        plan = read(path.parent / "plan.json")
        if record["task"] != task or record["plan"] != plan["digest"]:
            raise ValueError(f"plan identity differs: {task}")
        plans[task] = {"plan": plan, "untouched": record.get("untouched")}
        for label in read(path.parent / "labels.json")["trials"]:
            if label["trial"] in labels:
                raise ValueError("duplicate trial label")
            labels[label["trial"]] = label
        for entry in record["trials"]:
            trial = entry["trial"]
            if trial not in by_trial or trial in results:
                raise ValueError(f"unexpected or duplicate candidate: {trial}")
            wanted = by_trial[trial]
            if wanted["task"].split("/")[-1] != task:
                raise ValueError(f"candidate moved to another task: {trial}")
            label = labels.get(trial, {})
            if label.get("job") != wanted["job"]:
                raise ValueError(f"candidate job identity differs: {trial}")
            report = entry.get("report")
            if report and (report.get("plan") != plan["digest"]
                           or report.get("task") != task or report.get("candidate") != trial):
                raise ValueError(f"report identity differs: {trial}")
            results[trial] = (entry, label, report)
    rows = []
    for original in expected:
        trial = original["trial"]
        entry, label, report = results.get(trial, ({}, {}, None))
        error = entry.get("error")
        if trial not in results:
            error = "candidate has no retained contract result"
        elif label.get("kind") == "snapshot" and not label.get("snapshot_graded"):
            error = "snapshot is not established as the graded workspace"
        elif label.get("kind") not in ("snapshot", "final"):
            error = "unsupported workspace kind"
        if label and label.get("reward") != original["original_outcome"]["reward"]:
            raise ValueError(f"original reward changed: {trial}")
        rows.append({
            "task": original["task"].split("/")[-1], "split": "post_label_supplement",
            "trial": trial, "job": original["job"], "executor": original["executor"],
            "kind": label.get("kind"), "snapshot_graded": label.get("snapshot_graded", False),
            "reward": original["reward"], "original_outcome": original["original_outcome"],
            "regrade": original.get("regrade"), "error": error,
            "raw_call": report.get("call") if report else None,
            "call": report.get("call") if report and not error else None,
            "score": report.get("score") if report and not error else None,
            "items": report.get("items", []) if report and not error else [],
            "seconds": entry.get("seconds"), "network": entry.get("network"),
        })
    return rows, plans


def mean_sampled_rates(sample, rates):
    values = [rates[task] for task in sample if task in rates]
    return sum(values) / len(values) if values else None


def concordance_interval(rows):
    """Resample whole tasks, preserving repeated clusters' multiplicity."""
    tasks = sorted({row["task"] for row in rows})
    rates = {task: result["concordance"] for task, result in measure.concordance(rows).items()}
    rng = random.Random(measure.SEED)
    values, undefined = [], 0
    for _ in range(measure.RESAMPLES):
        value = mean_sampled_rates([rng.choice(tasks) for _ in tasks], rates)
        if value is None:
            undefined += 1
        else:
            values.append(value)
    values.sort()
    interval = ([round(values[int(q * (len(values) - 1))], 4) for q in (0.025, 0.975)]
                if values else None)
    return {"interval_95": interval, "undefined": undefined}


def population_summary(rows):
    # Restoration failures are unknown calls, but their official failures
    # remain in recall. The original complete-case calculation is separate.
    result = measure.summarize([{**r, "error": None} for r in rows])
    result["errors"] = sum(r["error"] is not None for r in rows)
    result["within_task"]["legacy_task_bootstrap_95"] = result["within_task"]["task_bootstrap_95"]
    result["within_task"]["task_bootstrap_95"] = concordance_interval(rows)
    return result


def analyze(root, published):
    rows, plans = load(root, published)
    processes = [read(path) for path in sorted(root.glob("*.process.json"))]
    calls = [call for entry in plans.values() for call in entry["plan"].get("jev", [])]
    live = [call for call in calls if call.get("how") == "live"]
    input_tokens = sum(call.get("input_tokens") or 0 for call in live)
    unknown_usage = sum(call.get("input_tokens") is None for call in live)
    times = [r["seconds"] for r in rows if r["seconds"] is not None]
    return {
        "schema": "openagents.contract-supplement-summary.v1",
        "protocol": "supplemental-protocol.md", "post_label": True,
        "concordance_bootstrap_note": "Repeated sampled tasks retain their multiplicity. The original helper collapses repeated task names when averaging concordance; its interval is retained separately for comparison.",
        "measurement_sha256": digest(published), "inputs_sha256": digest(root / "inputs.json"),
        "population": population_summary(rows),
        "complete_case": measure.summarize(rows),
        "by_executor": {name: population_summary([r for r in rows if r["executor"] == name])
                        for name in ("luna", "astra")},
        "by_task": {task: measure.calls([r for r in rows if r["task"] == task])
                    for task in sorted({r["task"] for r in rows})},
        "coverage": measure.coverage(rows, plans),
        "timing": {"invocation_seconds": sum(p.get("seconds", 0) for p in processes),
                   "unfinished_invocations": sum(p.get("state") != "finished" for p in processes),
                   "candidate_seconds_sum": sum(times),
                   "candidate_seconds_median": statistics.median(times) if times else None,
                   "candidate_times_unknown": len(rows) - len(times)},
        "usage": {"jev_recorded_requests": len(calls), "jev_live_requests": len(live),
                  "jev_input_tokens": input_tokens, "jev_unknown_usage": unknown_usage,
                  "jev_list_price_per_million_input_tokens": 0.042,
                  "jev_known_list_price_usd": input_tokens * 0.042 / 1_000_000,
                  "candidate_generation_requests": 0},
        "rows": rows,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    parser.add_argument("--published", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    result = analyze(args.root, args.published)
    write(args.out, result)
    print(json.dumps({key: result[key] for key in ("timing", "usage")}, indent=2))
    print(json.dumps(result["population"]["calls"], indent=2))


if __name__ == "__main__":
    main()
