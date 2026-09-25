#!/usr/bin/env python3
"""Join verify.executed's offline records with official rewards, and count.

Reads `<records>/<set>/<task>/executed.json` and `labels.json`, written by
`coder-one checks contract executed`, and writes `<records>/summary.json`.
Every command, verdict, and rejection comes from the records; this script
only joins them with rewards and counts:

- precision and recall of "rejected" (a command that exited 0 on the
  untouched workspace fails on the candidate) as a fail signal, with 95%
  Wilson intervals, by task, by set, and pooled;
- keep-best decisions the rule would have changed, by replaying each
  retained lean loop's `selection.json` with and without the rule.

Rewards: the label file's reward; for #9584's fresh trials, whose
retained traces carry no verifier result, the official label in
`--fresh-labels`, given to the final workspace and to the candidate whose
files are the submitted workspace's.

Usage: measure.py RECORDS --jobs DIR [--jobs DIR]... --fresh-labels FILE
"""
import argparse
import json
import math
from collections import defaultdict
from pathlib import Path


def wilson(k, n, z=1.959964):
    if n == 0:
        return None
    p = k / n
    centre = (p + z * z / (2 * n)) / (1 + z * z / n)
    half = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / (1 + z * z / n)
    return [round(max(0.0, centre - half), 4), round(min(1.0, centre + half), 4)]


def ratio(k, n):
    return {"k": k, "n": n, "value": round(k / n, 4) if n else None, "wilson_95": wilson(k, n)}


def base_trial(trial):
    return trial.split(".")[0]


def load(records, fresh):
    rows = []
    for executed in sorted(Path(records).glob("*/*/executed.json")):
        record = json.loads(executed.read_text())
        labels = json.loads((executed.parent / "labels.json").read_text())
        by_trial = {t["trial"]: t for t in labels["trials"]}
        for entry in record["trials"]:
            label = by_trial.get(entry["trial"], {})
            reward = label.get("reward")
            source = label.get("reward_source")
            base = base_trial(entry["trial"])
            if reward is None and base in fresh:
                if label.get("kind") == "final" or source == "submitted":
                    reward = fresh[base]
                    source = "official label"
            rows.append({
                "set": executed.parent.parent.name,
                "task": record["task"],
                "trial": entry["trial"],
                "job": label.get("job"),
                "kind": label.get("kind"),
                "reward": reward,
                "reward_source": source,
                "error": entry.get("error"),
                "rejected": entry.get("rejected"),
                "regressed": [
                    {"command": r["command"], "rule": r["rule"]}
                    for r in entry.get("records", [])
                    if r["verdict"] == "regressed"
                ],
                "verdicts": [r["verdict"] for r in entry.get("records", [])],
            })
    return rows


def signal(rows):
    graded = [r for r in rows if r["error"] is None and r["reward"] is not None]
    fails = [r for r in graded if r["reward"] < 1.0]
    rejected = [r for r in graded if r["rejected"]]
    right = [r for r in rejected if r["reward"] < 1.0]
    return {
        "graded": len(graded),
        "failures": len(fails),
        "passes": len(graded) - len(fails),
        "rejected": len(rejected),
        "precision": ratio(len(right), len(rejected)),
        "recall": ratio(len(right), len(fails)),
        "ungraded": sum(1 for r in rows if r["reward"] is None and r["error"] is None),
        "errors": sum(1 for r in rows if r["error"] is not None),
    }


def fraction(score):
    if not score or score.get("total") in (None, 0) or score.get("passed") is None:
        return -1.0
    return score["passed"] / score["total"]


def replay(moves, rejected, protect):
    """The session the lean loop submits, by lean.rs's keep and restore rules."""
    best = None
    last = None
    for m in moves:
        if m.get("kind") != "lean" or m.get("lane") is not None:
            continue
        n = m["after_session"]
        flagged = isinstance(m.get("hardcoded"), dict) and m["hardcoded"].get("flagged") is True
        out = flagged or rejected.get(n, False)
        if not out:
            if best is None or (fraction(m.get("score")) > fraction(best[1]) if protect
                                else fraction(m.get("score")) >= fraction(best[1])):
                best = (n, m.get("score"))
        last = (n, m.get("score"), out)
    if last is None:
        return None
    if best is not None and best[0] != last[0] and (
            protect or last[2] or fraction(best[1]) > fraction(last[1])):
        return best[0]
    return last[0]


def keep_best(rows, jobs):
    by_base = defaultdict(dict)
    for r in rows:
        parts = r["trial"].split(".")
        if len(parts) == 3 and r["error"] is None:
            by_base[(r["task"], parts[0], parts[1])][int(parts[2].split("-")[1])] = r
    out = []
    for (task, base, lean), sessions in sorted(by_base.items()):
        selection = None
        for root in jobs:
            found = list(Path(root).glob(f"*/{base}/agent/episode/artifacts/{lean}/selection.json"))
            if found:
                selection = found[0]
                break
        if selection is None:
            continue
        moves = json.loads(selection.read_text())
        submitted = next((m for m in reversed(moves) if m.get("kind") == "lean.submitted"), {})
        protect = "output" in submitted
        without = replay(moves, {}, protect)
        with_rule = replay(moves, {n: bool(r["rejected"]) for n, r in sessions.items()}, protect)
        out.append({
            "task": task,
            "trial": base,
            "lean": lean,
            "recorded": submitted.get("selected_session"),
            "replayed_without_rule": without,
            "replay_matches_record": without == submitted.get("selected_session"),
            "with_rule": with_rule,
            "changed": with_rule != without,
            "rejected_sessions": sorted(n for n, r in sessions.items() if r["rejected"]),
            "sessions_measured": sorted(sessions),
            "rewards": {n: r["reward"] for n, r in sorted(sessions.items())},
        })
    return out


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("records")
    parser.add_argument("--jobs", action="append", default=[])
    parser.add_argument("--fresh-labels", required=True)
    parser.add_argument("--out")
    args = parser.parse_args()
    fresh = {
        label["trial"]: label["reward"]
        for label in json.loads(Path(args.fresh_labels).read_text())["labels"]
        if label.get("executor") == "luna" and label.get("reward") is not None
    }
    rows = load(args.records, fresh)
    by_task = defaultdict(list)
    by_set = defaultdict(list)
    for r in rows:
        by_task[(r["set"], r["task"])].append(r)
        by_set[r["set"]].append(r)
    decisions = keep_best(rows, args.jobs)
    summary = {
        "schema": "openagents.coder-one.verify-executed-summary.v1",
        "pooled": signal(rows),
        "by_set": {s: signal(v) for s, v in sorted(by_set.items())},
        "by_task": {f"{s}/{t}": signal(v) for (s, t), v in sorted(by_task.items())},
        "verdicts": {
            v: sum(r["verdicts"].count(v) for r in rows)
            for v in ("ok", "regressed", "not_a_regression", "unknown")
        },
        "rejected": [
            {k: r[k] for k in ("set", "task", "trial", "reward", "reward_source", "regressed")}
            for r in rows if r["rejected"]
        ],
        "keep_best": {
            "loops": len(decisions),
            "replays_matching_record": sum(d["replay_matches_record"] for d in decisions),
            "changed": sum(d["changed"] for d in decisions),
            "decisions": decisions,
        },
    }
    text = json.dumps(summary, indent=2) + "\n"
    Path(args.out or Path(args.records) / "summary.json").write_text(text)
    print(json.dumps({k: summary[k] for k in ("pooled", "by_set", "verdicts")}, indent=2))
    print("keep-best:", {k: v for k, v in summary["keep_best"].items() if k != "decisions"})


if __name__ == "__main__":
    main()
