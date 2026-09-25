#!/usr/bin/env python3
"""Joins `checks.oracle` results with verifier rewards (issue #9656).

Usage: measure.py RECORDS [--out FILE]

RECORDS holds one directory per task with `results.json` and
`labels.json` from `coder-one checks oracle offline`. The rules are the
frozen ones in protocol.md. Prints the tables and writes summary.json.
"""

import glob
import json
import math
import os
import sys


def wilson(k, n, z=1.96):
    if n == 0:
        return None
    p = k / n
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return [round(max(0.0, c - h), 3), round(min(1.0, c + h), 3)]


def rate(k, n):
    return {"k": k, "n": n, "share": round(k / n, 3) if n else None, "wilson95": wilson(k, n)}


def call_of(result):
    if not result:
        return None, None
    verdicts = [c["verdict"] for c in result.get("cases", [])]
    passed = verdicts.count("passed")
    failed = verdicts.count("failed")
    score = passed / (passed + failed) if passed + failed else None
    if failed:
        return "fail", score
    if passed:
        return "pass", score
    return None, score


def self_score(episode, trial):
    """The frozen self-score a lean run recorded for this workspace."""
    if not episode:
        return None
    records = sorted(glob.glob(os.path.join(episode, "artifacts", "microluna-*.json")))
    session = None
    if ".session-" in trial:
        try:
            session = int(trial.rsplit(".session-", 1)[1])
        except ValueError:
            return None
    for path in records:
        try:
            record = json.load(open(path))
        except (OSError, ValueError):
            continue
        moves = record.get("moves") or []
        if session is not None:
            for m in moves:
                if m.get("kind") == "lean" and m.get("after_session") == session and m.get("score"):
                    return m["score"]
            continue
        restore = [m for m in moves if m.get("kind") == "lean.restore" and m.get("score")]
        if restore:
            return restore[-1]["score"]
        scored = [m for m in moves if m.get("kind") == "lean" and m.get("score")]
        if scored:
            return scored[-1]["score"]
    return None


def graded(row, secondary):
    if row["reward"] is None:
        return False
    kind = row["kind"]
    if kind in ("final", "reconstruction"):
        return True
    if kind == "candidate":
        return row.get("reward_source") not in (None, "submitted")
    if kind == "snapshot":
        return row.get("snapshot_graded") or secondary
    return False


def discrimination(rows, key):
    """Keep, catch, and pair ordering within mixed tasks for call/score `key`."""
    by_task = {}
    for r in rows:
        by_task.setdefault(r["task"], []).append(r)
    keep = [0, 0]
    catch = [0, 0]
    pairs = [0, 0]
    ties = 0
    mixed = []
    for task, rs in sorted(by_task.items()):
        passes = [r for r in rs if r["reward"] >= 1.0 and r[key + "_call"] is not None]
        fails = [r for r in rs if r["reward"] < 1.0 and r[key + "_call"] is not None]
        if not passes or not fails:
            continue
        mixed.append(task)
        keep[1] += len(passes)
        keep[0] += sum(r[key + "_call"] == "pass" for r in passes)
        catch[1] += len(fails)
        catch[0] += sum(r[key + "_call"] == "fail" for r in fails)
        for p in passes:
            for f in fails:
                a, b = p[key + "_score"], f[key + "_score"]
                if a is None or b is None:
                    continue
                if a == b:
                    ties += 1
                    continue
                pairs[1] += 1
                pairs[0] += a > b
    return {
        "mixed_tasks": mixed,
        "passes_kept_green": rate(*keep),
        "failures_called_red": rate(*catch),
        "pairs_ordered_right": rate(*pairs),
        "pairs_tied": ties,
    }


def bar(d):
    keep = d["passes_kept_green"]["wilson95"]
    catch = d["failures_called_red"]["wilson95"]
    return bool(
        len(d["mixed_tasks"]) >= 2
        and keep
        and catch
        and keep[0] >= 0.8
        and catch[0] >= 0.2
    )


def main():
    root = sys.argv[1]
    out = sys.argv[sys.argv.index("--out") + 1] if "--out" in sys.argv else os.path.join(root, "summary.json")
    tasks = []
    rows = []
    for results_path in sorted(glob.glob(os.path.join(root, "*", "results.json"))):
        task_dir = os.path.dirname(results_path)
        results = json.load(open(results_path))
        labels = json.load(open(os.path.join(task_dir, "labels.json")))
        task = results["task"]
        untouched_call, _ = call_of(results.get("untouched"))
        source = results.get("source") if results.get("oracle") else None
        writer = {}
        if os.path.exists(os.path.join(task_dir, "writer.json")):
            writer = json.load(open(os.path.join(task_dir, "writer.json")))
        info = {
            "task": task,
            "oracle": source,
            "untouched_call": untouched_call,
            "trivially_passing": untouched_call == "pass",
            "usable": untouched_call == "fail",
            "writer_usd": writer.get("usd"),
            "writer_ending": writer.get("ending"),
            "workspaces": len(labels["trials"]),
            "answered": 0,
            "errors": 0,
        }
        by_trial = {t["trial"]: t for t in results.get("trials", [])}
        for label in labels["trials"]:
            ran = by_trial.get(label["trial"], {})
            if "error" in ran:
                info["errors"] += 1
            call, score = call_of(ran.get("result"))
            if call is not None:
                info["answered"] += 1
            ss = self_score(label.get("episode"), label["trial"])
            ss_call = None if ss is None else ("pass" if ss["passed"] >= ss["total"] else "fail")
            ss_score = None if ss is None or not ss["total"] else ss["passed"] / ss["total"]
            rows.append({
                "task": task,
                "trial": label["trial"],
                "kind": label["kind"],
                "reward": label["reward"],
                "reward_source": label.get("reward_source"),
                "snapshot_graded": label.get("snapshot_graded"),
                "usable": info["usable"],
                "oracle_call": call,
                "oracle_score": score,
                "self_call": ss_call,
                "self_score": ss_score,
            })
        tasks.append(info)
    found = sum(t["oracle"] == "found" for t in tasks)
    written = sum(t["oracle"] == "written" for t in tasks)
    with_oracle = [t for t in tasks if t["oracle"]]
    answered_untouched = [t for t in with_oracle if t["untouched_call"] is not None]
    summary = {
        "tasks": tasks,
        "coverage": {
            "tasks": len(tasks),
            "found": rate(found, len(tasks)),
            "written": rate(written, len(tasks)),
            "oracle": rate(len(with_oracle), len(tasks)),
            "trivially_passing": rate(sum(t["trivially_passing"] for t in answered_untouched), len(answered_untouched)),
            "untouched_no_answer": sum(t["untouched_call"] is None for t in with_oracle),
            "usable": rate(sum(t["usable"] for t in with_oracle), len(tasks)),
            "workspaces_answered": rate(sum(t["answered"] for t in with_oracle), sum(t["workspaces"] for t in with_oracle)),
        },
        "luna_usd": round(sum(t["writer_usd"] or 0 for t in tasks), 5),
    }
    for name, secondary in (("primary", False), ("secondary", True)):
        set_rows = [r for r in rows if graded(r, secondary)]
        summary[name] = {
            "workspaces": len(set_rows),
            "passes": sum(r["reward"] >= 1.0 for r in set_rows),
            "oracle_usable": discrimination([r for r in set_rows if r["usable"]], "oracle"),
            "oracle_all": discrimination(set_rows, "oracle"),
            "self_score": discrimination(set_rows, "self"),
            "self_score_workspaces": sum(r["self_call"] is not None for r in set_rows),
        }
        for k in ("oracle_usable", "oracle_all", "self_score"):
            summary[name][k]["meets_bar"] = bar(summary[name][k])
        # Agreement across tasks, for context only.
        called = [r for r in set_rows if r["usable"] and r["oracle_call"] is not None]
        summary[name]["oracle_usable_fail_calls_right"] = rate(
            sum(r["oracle_call"] == "fail" and r["reward"] < 1.0 for r in called),
            sum(r["oracle_call"] == "fail" for r in called),
        )
        summary[name]["oracle_usable_pass_calls_right"] = rate(
            sum(r["oracle_call"] == "pass" and r["reward"] >= 1.0 for r in called),
            sum(r["oracle_call"] == "pass" for r in called),
        )
    summary["rows"] = rows
    json.dump(summary, open(out, "w"), indent=2)
    print(json.dumps({k: v for k, v in summary.items() if k not in ("rows",)}, indent=2))


if __name__ == "__main__":
    main()
