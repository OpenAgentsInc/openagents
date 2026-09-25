#!/usr/bin/env python3
"""Precision, recall, Wilson intervals, and a task bootstrap for issue #9627.

This script consumes the Rust-produced rows (features, Jev answers, and
calls) and the label files the same replay wrote. It never reads a trace
and never asks Jev. With --select it reads calibration labels only and
picks the thresholds by the rule the protocol froze. Without it, it scores
the frozen thresholds on every labels file it is given.
"""

import argparse
import json
import math
import random
from collections import defaultdict

PROGRESS_GRID = [0.1, 0.2, 0.3, 0.4, 0.5]
REPEATING_GRID = [0.5, 0.6, 0.7, 0.8, 0.9, 1.01]  # 1.01: never
MIN_PRECISION = 0.80
BOOTSTRAP = 10_000
SEED = 9627


def wilson(k, n, z=1.959964):
    if n == 0:
        return None
    p = k / n
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return [round(max(0.0, c - h), 4), round(min(1.0, c + h), 4)]


def load(path):
    with open(path) as f:
        return [json.loads(line) for line in f if line.strip()]


def key(row):
    return (row["trial"], row["session"], row["turn"], row["at"])


def join(rows, labels):
    by = {key(r): r for r in rows}
    out = []
    for label in labels:
        row = by.get(key(label))
        if row is None or not label["hindsight"]["labeled"]:
            continue
        out.append((row, label))
    return out


def jev_call(answers, progress_below, repeating_at):
    p, r = answers.get("progress"), answers.get("repeating")
    if p is None and r is None:
        return None
    return (p is not None and p < progress_below) or (r is not None and r >= repeating_at)


def predictors(row, params):
    j = jev_call(row["answers"], params["progress_below"], params["repeating_at"])
    cascade = row["suspect"] and (j if j is not None else row["strong"])
    return {
        "code_suspect": row["suspect"],
        "code_strong": row["strong"],
        "jev_alone": bool(j),
        "cascade": bool(cascade),
    }


def confusion(pairs, name, params):
    tp = fp = fn = tn = 0
    for row, label in pairs:
        call = predictors(row, params)[name]
        stall = label["hindsight"]["stall"]
        if call and stall:
            tp += 1
        elif call:
            fp += 1
        elif stall:
            fn += 1
        else:
            tn += 1
    return tp, fp, fn, tn


def summarize(tp, fp, fn, tn):
    calls, stalls = tp + fp, tp + fn
    return {
        "calls": calls,
        "correct_calls": tp,
        "stalls": stalls,
        "checkpoints": tp + fp + fn + tn,
        "precision": round(tp / calls, 4) if calls else None,
        "precision_wilson": wilson(tp, calls),
        "recall": round(tp / stalls, 4) if stalls else None,
        "recall_wilson": wilson(tp, stalls),
    }


def select(pairs):
    grid = []
    for pb in PROGRESS_GRID:
        for ra in REPEATING_GRID:
            params = {"progress_below": pb, "repeating_at": ra}
            s = summarize(*confusion(pairs, "cascade", params))
            grid.append({"params": params, **s})
    ok = [g for g in grid if g["precision"] is not None and g["precision"] >= MIN_PRECISION]
    if ok:
        best = sorted(ok, key=lambda g: (-g["recall"], -g["precision"],
                                         g["params"]["progress_below"], -g["params"]["repeating_at"]))[0]
        rule = f"highest recall with precision >= {MIN_PRECISION}"
    else:
        cand = [g for g in grid if g["calls"] >= 5]
        best = sorted(cand, key=lambda g: (-(g["precision"] or 0), -(g["recall"] or 0),
                                           g["params"]["progress_below"], -g["params"]["repeating_at"]))[0]
        rule = f"no setting reached precision {MIN_PRECISION}; highest precision with at least 5 calls"
    return best, rule, grid


def bootstrap(pairs, params, names=("cascade", "code_suspect")):
    tasks = sorted({label["task"] for _, label in pairs})
    by_task = defaultdict(list)
    for pair in pairs:
        by_task[pair[1]["task"]].append(pair)
    rng = random.Random(SEED)
    stats = defaultdict(list)
    undefined = defaultdict(int)
    for _ in range(BOOTSTRAP):
        sample = [p for t in (rng.choice(tasks) for _ in tasks) for p in by_task[t]]
        values = {}
        for name in names:
            tp, fp, fn, _ = confusion(sample, name, params)
            values[name] = (tp / (tp + fp) if tp + fp else None, tp / (tp + fn) if tp + fn else None)
        for i, metric in enumerate(["precision", "recall"]):
            a, b = values[names[0]][i], values[names[1]][i]
            if a is None:
                undefined[f"{names[0]}_{metric}"] += 1
            else:
                stats[f"{names[0]}_{metric}"].append(a)
            if a is None or b is None:
                undefined[f"difference_{metric}"] += 1
            else:
                stats[f"difference_{metric}"].append(a - b)

    def interval(xs):
        xs = sorted(xs)
        if not xs:
            return None
        return [round(xs[int(0.025 * (len(xs) - 1))], 4), round(xs[int(0.975 * (len(xs) - 1))], 4)]

    return {
        "resamples": BOOTSTRAP,
        "seed": SEED,
        "unit": "task",
        "tasks": len(tasks),
        "compared": f"{names[0]} minus {names[1]}",
        "intervals": {k: interval(v) for k, v in stats.items()},
        "undefined": dict(undefined),
    }


def next_step(pairs):
    """Whether Jev's next-step pick agrees with a productive next step."""
    agree = [0, 0]  # productive, total
    disagree = [0, 0]
    productive_total = 0
    productive_agree = 0
    picks = defaultdict(int)
    for row, label in pairs:
        pick = row["answers"].get("next")
        if pick is None:
            continue
        picks[pick] += 1
        h = label["hindsight"]
        if pick == "continue" or h["next_kind"] == "none":
            continue
        productive = h["productive_next"]
        same = pick == h["next_kind"]
        bucket = agree if same else disagree
        bucket[0] += int(productive)
        bucket[1] += 1
        if productive:
            productive_total += 1
            productive_agree += int(same)
    return {
        "picks": dict(picks),
        "pick_matches_next_step": {"productive": agree[0], "of": agree[1],
                                   "rate": round(agree[0] / agree[1], 4) if agree[1] else None,
                                   "wilson": wilson(*agree)},
        "pick_differs_from_next_step": {"productive": disagree[0], "of": disagree[1],
                                        "rate": round(disagree[0] / disagree[1], 4) if disagree[1] else None,
                                        "wilson": wilson(*disagree)},
        "precision": {"value": round(agree[0] / agree[1], 4) if agree[1] else None, "wilson": wilson(*agree),
                      "meaning": "of next steps that matched Jev's pick, the share followed by a score gain"},
        "recall": {"value": round(productive_agree / productive_total, 4) if productive_total else None,
                   "wilson": wilson(productive_agree, productive_total),
                   "meaning": "of productive next steps, the share Jev had picked"},
    }


def report(pairs, params):
    out = {}
    for name in ["code_suspect", "code_strong", "jev_alone", "cascade"]:
        out[name] = summarize(*confusion(pairs, name, params))
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--rows", required=True)
    ap.add_argument("--labels", nargs="+", required=True)
    ap.add_argument("--select", action="store_true")
    ap.add_argument("--params", help="a selection.json whose chosen params to score")
    ap.add_argument("--out")
    a = ap.parse_args()
    rows = load(a.rows)
    labels = [l for path in a.labels for l in load(path)]
    if a.select:
        labels = [l for l in labels if l["partition"] == "calibration"]
        pairs = join(rows, labels)
        best, rule, grid = select(pairs)
        result = {"rule": rule, "chosen": best, "grid": grid,
                  "labeled_checkpoints": len(pairs)}
    else:
        params = json.load(open(a.params))["chosen"]["params"]
        result = {"params": params, "partitions": {}}
        for part in sorted({l["partition"] for l in labels}):
            pairs = join(rows, [l for l in labels if l["partition"] == part])
            if not pairs:
                continue
            section = {"labeled_checkpoints": len(pairs), "all": report(pairs, params)}
            for at in ["session_end", "in_session"]:
                sub = [p for p in pairs if p[1]["at"] == at]
                section[at] = report(sub, params)
            section["by_task"] = {}
            for task in sorted({p[1]["task"] for p in pairs}):
                sub = [p for p in pairs if p[1]["task"] == task]
                section["by_task"][task] = {
                    "trials": len({p[1]["trial"] for p in sub}),
                    "passed_trials": len({p[1]["trial"] for p in sub if p[1]["passed"]}),
                    **report(sub, params),
                }
            section["bootstrap"] = bootstrap(pairs, params)
            section["next_step"] = next_step(pairs)
            section["done_report_only"] = done_report(pairs)
            result["partitions"][part] = section
    text = json.dumps(result, indent=2)
    if a.out:
        with open(a.out, "w") as f:
            f.write(text + "\n")
    print(text)


def done_report(pairs):
    """The done Noul against passing trials, reported and never acted on."""
    high = [0, 0]
    for row, label in pairs:
        p = row["answers"].get("done")
        if p is None or p < 0.8:
            continue
        high[1] += 1
        high[0] += int(bool(label["passed"]))
    return {"done_at_least_0_8": high[1], "of_which_trial_passed": high[0],
            "pass_rate_wilson": wilson(*high)}


if __name__ == "__main__":
    main()
