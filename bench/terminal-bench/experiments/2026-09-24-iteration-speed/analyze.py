"""Summarize the completed suite and its post-run candidate grades."""
import json
from pathlib import Path
from statistics import mean


def analyze(records):
    results = json.loads((records / "results.json").read_text())
    grades = json.loads((records / "grading-fresh/batch.json").read_text())
    assert len(results["trials"]) == 6
    assert not grades["errors"] and not grades["invalid_candidates"]
    rows = []
    for trial in results["trials"]:
        candidates = [c for c in grades["candidates"] if Path(c["trial"]).name == trial["trial"]]
        assert candidates and trial["reward"] in (0, 1)
        candidates.sort(key=lambda c: (c["dispatch"], c["session"]))
        selections = [m for selection in trial["selection"] for m in selection["moves"]]
        copied = [m for m in selections if m.get("kind") == "lean"]
        submitted = [m for m in selections if m.get("kind") == "lean.submitted"]
        sessions = [s for loop in trial["loops"] for s in loop["sessions"]]
        assert len(trial["loops"]) == 1, "Inspect multiple dispatches before aggregating reviews"
        review_numbers = {c["after_session"] for c in copied if c.get("self_check")}
        reviews = [s for s in sessions if s["number"] in review_numbers]
        assert len(reviews) == 1, "This trial has no unique recorded review"
        review = reviews[0]
        changed = None
        if len(copied) == 2:
            a, b = copied[0]["workspace_files"], copied[1]["workspace_files"]
            changed = sorted(k for k in set(a) | set(b) if a.get(k) != b.get(k))
        rows.append({"job": trial["job"], "trial": trial["trial"], "task": trial["task"],
                     "attempt": trial["attempt"], "reward": trial["reward"],
                     "cost_usd": trial["cost_usd"], "agent_seconds": trial["agent_seconds"],
                     "trial_seconds": trial["trial_seconds"], "verifier": trial["verifier_summary"],
                     "failed_tests": trial["verifier_failures"],
                     "candidates": [{"session": c["session"], "grade": c["grade"]["reward"],
                                     "self_score": c["self_score"], "reused_from": c["reused_from"]}
                                    for c in candidates],
                     "snapshot_ms": [c["snapshot_ms"] for c in copied],
                     "submitted": submitted, "sessions": sessions,
                     "review_changed_files": changed,
                     "review_seconds": review["milliseconds"] / 1000,
                     "review_cost_usd": review["cost_usd"]})
    summary = []
    for task in sorted({r["task"] for r in rows}):
        group = [r for r in rows if r["task"] == task]
        passes = sum(r["reward"] == 1 for r in group)
        cost = sum(r["cost_usd"] for r in group)
        summary.append({"task": task, "n": len(group), "passes": passes,
                        "cost_usd": cost, "cost_per_pass_usd": cost / passes if passes else None,
                        "mean_agent_seconds": mean(r["agent_seconds"] for r in group),
                        "mean_trial_seconds": mean(r["trial_seconds"] for r in group),
                        "first_candidate_passes": sum(r["candidates"][0]["grade"] == 1 for r in group),
                        "oracle_passes": sum(any(c["grade"] == 1 for c in r["candidates"]) for r in group),
                        "review_cost_usd": sum(r["review_cost_usd"] for r in group),
                        "mean_review_seconds": mean(r["review_seconds"] for r in group)})
    return {"schema": "openagents.microluna.iteration-speed-analysis.v1", "summary": summary,
            "trials": rows, "total_cost_usd": sum(r["cost_usd"] for r in rows),
            "snapshot_ms": [ms for r in rows for ms in r["snapshot_ms"]],
            "limitations": "Selected development tasks; historical Fable reference on different hosts/models/harnesses; usage-valued costs, not invoices."}


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("records", type=Path)
    args = parser.parse_args()
    value = analyze(args.records)
    (args.records / "analysis.json").write_text(json.dumps(value, indent=2) + "\n")
    print(json.dumps(value["summary"], indent=2))
