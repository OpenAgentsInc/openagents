#!/usr/bin/env python3
"""Join executed contract checks with verifier rewards, as the protocol fixes.

Reads `<records>/<task>/contract.json` and `labels.json`, written by
`coder-one checks contract offline`, and writes `<records>/summary.json`.
It computes nothing the Rust component decides: every item outcome, call,
and score comes from the records. This script only joins them with the
labels, splits by task, and counts.

Usage: measure.py RECORDS [--out FILE]
"""
import argparse
import hashlib
import json
import math
import random
from pathlib import Path

KINDS = ["path", "format", "interface", "example", "exit_code", "command"]
EXCLUDED_TASKS = {
    "distributed-dedup", "formal-crypto", "freecad-impeller", "freecad-spring-clip",
    "math-eval-grader", "pretrain-shard-corruption", "shadow-relay", "vpp-loss-divergence",
}
RESAMPLES = 10_000
SEED = 9628


def split(task):
    digest = hashlib.sha256(("openagents-9628:" + task).encode()).hexdigest()
    return "development" if int(digest[:8], 16) % 2 == 0 else "held_out"


def wilson(k, n, z=1.959964):
    if n == 0:
        return None
    p = k / n
    centre = (p + z * z / (2 * n)) / (1 + z * z / n)
    half = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / (1 + z * z / n)
    return [round(max(0.0, centre - half), 4), round(min(1.0, centre + half), 4)]


def ratio(k, n):
    return {"k": k, "n": n, "value": round(k / n, 4) if n else None, "wilson_95": wilson(k, n)}


def load(records):
    rows, plans = [], {}
    for contract in sorted(Path(records).glob("*/contract.json")):
        task_dir = contract.parent
        record = json.loads(contract.read_text())
        task = record["task"]
        if task in EXCLUDED_TASKS:
            raise SystemExit(f"{task} is excluded by the protocol")
        plan = json.loads((task_dir / "plan.json").read_text())
        labels = json.loads((task_dir / "labels.json").read_text())
        by_trial = {t["trial"]: t for t in labels["trials"]}
        plans[task] = {"plan": plan, "untouched": record.get("untouched")}
        for entry in record["trials"]:
            label = by_trial.get(entry["trial"], {})
            if any(x in label.get("job", "") for x in ("truth-confirmation", "truth-control")):
                raise SystemExit(f"{entry['trial']} is from an excluded job")
            report = entry.get("report")
            rows.append({
                "task": task,
                "split": split(task),
                "trial": entry["trial"],
                "job": label.get("job"),
                "kind": label.get("kind"),
                "snapshot_graded": label.get("snapshot_graded", False),
                "reward": label.get("reward"),
                "reward_source": label.get("reward_source"),
                "error": entry.get("error"),
                "call": report["call"] if report else None,
                "score": report["score"] if report else None,
                "items": report["items"] if report else [],
            })
    return rows, plans


def graded(row):
    return row["kind"] in ("final", "candidate", "reconstruction") or (
        row["kind"] == "snapshot" and row["snapshot_graded"])


def kind_outcome(row, kind):
    outcomes = [i["outcome"] for i in row["items"] if i["kind"] == kind]
    if "differed" in outcomes:
        return "differed"
    if "matched" in outcomes:
        return "matched"
    return None


def passed(row):
    return row["reward"] >= 1.0


def per_kind(rows):
    out = {}
    for kind in KINDS:
        differed = [r for r in rows if kind_outcome(r, kind) == "differed"]
        matched = [r for r in rows if kind_outcome(r, kind) == "matched"]
        out[kind] = {
            "fail_given_differed": ratio(sum(not passed(r) for r in differed), len(differed)),
            "pass_given_matched": ratio(sum(passed(r) for r in matched), len(matched)),
            "tasks_differed": len({r["task"] for r in differed}),
            "tasks_matched": len({r["task"] for r in matched}),
        }
    return out


def calls(rows):
    failures = [r for r in rows if not passed(r)]
    passes = [r for r in rows if passed(r)]
    said_fail = [r for r in rows if r["call"] == "fail"]
    said_pass = [r for r in rows if r["call"] == "pass"]
    return {
        "candidates": len(rows),
        "failures": len(failures),
        "passes": len(passes),
        "no_call": sum(r["call"] is None for r in rows),
        "fail_precision": ratio(sum(not passed(r) for r in said_fail), len(said_fail)),
        "failure_recall": ratio(sum(r["call"] == "fail" for r in failures), len(failures)),
        "pass_precision": ratio(sum(passed(r) for r in said_pass), len(said_pass)),
        "pass_recall": ratio(sum(r["call"] == "pass" for r in passes), len(passes)),
    }


def concordance(rows):
    """Per task with a pass and a failure: P(score pass > score fail), ties half."""
    out = {}
    for task in sorted({r["task"] for r in rows}):
        group = [r for r in rows if r["task"] == task]
        good = [r for r in group if passed(r)]
        bad = [r for r in group if not passed(r)]
        if not good or not bad:
            continue
        total, undefined = 0.0, 0
        for g in good:
            for b in bad:
                if g["score"] is None or b["score"] is None:
                    undefined += 1
                    total += 0.5
                elif g["score"] > b["score"]:
                    total += 1
                elif g["score"] == b["score"]:
                    total += 0.5
        pairs = len(good) * len(bad)
        scores = {r["score"] for r in group}
        out[task] = {
            "passes": len(good),
            "failures": len(bad),
            "pairs": pairs,
            "pairs_with_undefined_score": undefined,
            "concordance": round(total / pairs, 4),
            "score_varies": len(scores) > 1,
        }
    return out


def bootstrap(rows, statistic):
    """A 95% percentile interval over resampled whole tasks."""
    tasks = sorted({r["task"] for r in rows})
    if not tasks:
        return None
    by_task = {t: [r for r in rows if r["task"] == t] for t in tasks}
    rng = random.Random(SEED)
    values, undefined = [], 0
    for _ in range(RESAMPLES):
        sample = [r for t in (rng.choice(tasks) for _ in tasks) for r in by_task[t]]
        value = statistic(sample)
        if value is None:
            undefined += 1
        else:
            values.append(value)
    if not values:
        return {"interval_95": None, "undefined": undefined}
    values.sort()
    lo = values[int(0.025 * (len(values) - 1))]
    hi = values[int(0.975 * (len(values) - 1))]
    return {"interval_95": [round(lo, 4), round(hi, 4)], "undefined": undefined}


def share(rows, want, given):
    chosen = [r for r in rows if given(r)]
    return sum(want(r) for r in chosen) / len(chosen) if chosen else None


def mean_concordance(rows):
    values = [v["concordance"] for v in concordance(rows).values()]
    return sum(values) / len(values) if values else None


def summarize(rows):
    known = [r for r in rows if r["reward"] is not None and r["error"] is None]
    kinds = per_kind(known)
    for kind in KINDS:
        kinds[kind]["fail_given_differed"]["task_bootstrap_95"] = bootstrap(
            known, lambda s, k=kind: share(s, lambda r: not passed(r), lambda r: kind_outcome(r, k) == "differed"))
        kinds[kind]["pass_given_matched"]["task_bootstrap_95"] = bootstrap(
            known, lambda s, k=kind: share(s, passed, lambda r: kind_outcome(r, k) == "matched"))
    by_task = concordance(known)
    result = {
        "candidates_with_reward": len(known),
        "candidates_without_reward": sum(r["reward"] is None for r in rows),
        "errors": sum(r["error"] is not None for r in rows),
        "tasks": len({r["task"] for r in known}),
        "per_kind": kinds,
        "calls": calls(known),
        "within_task": {
            "tasks": by_task,
            "mean_concordance": round(mean_concordance(known), 4) if by_task else None,
            "task_bootstrap_95": bootstrap(known, mean_concordance) if by_task else None,
            "tasks_where_score_varies": sum(v["score_varies"] for v in by_task.values()),
        },
    }
    result["calls"]["fail_precision"]["task_bootstrap_95"] = bootstrap(
        known, lambda s: share(s, lambda r: not passed(r), lambda r: r["call"] == "fail"))
    result["calls"]["pass_precision"]["task_bootstrap_95"] = bootstrap(
        known, lambda s: share(s, passed, lambda r: r["call"] == "pass"))
    return result


def coverage(rows, plans):
    out = {}
    for task, entry in sorted(plans.items()):
        plan = entry["plan"]
        items = {}
        for item in plan["items"]:
            state = "not_executable" if item.get("not_executable") else "check"
            items.setdefault(item["kind"], {}).setdefault(state, 0)
            items[item["kind"]][state] += 1
        outcomes = {}
        for row in (r for r in rows if r["task"] == task):
            for i in row["items"]:
                outcomes.setdefault(i["kind"], {}).setdefault(i["outcome"], 0)
                outcomes[i["kind"]][i["outcome"]] += 1
        out[task] = {
            "split": split(task),
            "plan": plan["digest"],
            "items": items,
            "untouched": (entry["untouched"] or {}).get("tally"),
            "outcomes_over_candidates": outcomes,
            "candidates": sum(r["task"] == task for r in rows),
            "no_call": sum(r["task"] == task and r["call"] is None for r in rows),
        }
    return out


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("records")
    parser.add_argument("--out")
    args = parser.parse_args()
    rows, plans = load(args.records)
    sets = {}
    for name, keep in [("primary", graded), ("all_snapshots", lambda r: r["kind"] == "snapshot")]:
        chosen = [r for r in rows if keep(r)]
        sets[name] = {
            part: summarize([r for r in chosen if part == "both" or r["split"] == part])
            for part in ("development", "held_out", "both")
        }
    summary = {
        "schema": "openagents.contract-checks.summary.v1",
        "protocol": "protocol.md",
        "resamples": RESAMPLES,
        "seed": SEED,
        "sets": sets,
        "coverage": coverage(rows, plans),
        "rows": [
            {k: r[k] for k in ("task", "split", "trial", "job", "kind", "snapshot_graded", "reward",
                               "reward_source", "call", "score", "error")}
            | {"kinds": {k: kind_outcome(r, k) for k in KINDS if kind_outcome(r, k)}}
            for r in rows
        ],
    }
    out = Path(args.out) if args.out else Path(args.records) / "summary.json"
    out.write_text(json.dumps(summary, indent=1) + "\n")
    for name, parts in sets.items():
        for part, s in parts.items():
            c = s["calls"]
            w = s["within_task"]
            print(f"{name}/{part}: {s['candidates_with_reward']} candidates on {s['tasks']} tasks; "
                  f"fail calls {c['fail_precision']['k']}/{c['fail_precision']['n']} right, "
                  f"recall {c['failure_recall']['k']}/{c['failure_recall']['n']}; "
                  f"pass calls {c['pass_precision']['k']}/{c['pass_precision']['n']} right; "
                  f"no call {c['no_call']}; within-task concordance {w['mean_concordance']} "
                  f"over {len(w['tasks'])} tasks {w['task_bootstrap_95']}")
    print(f"wrote {out}")


if __name__ == "__main__":
    main()
