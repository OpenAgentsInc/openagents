#!/usr/bin/env python3
"""Count what replay.py found, and join the hand labels.

Reads ~/.openagents/coder-one/data-profile-offline/results.json (or the
path given) and records/labels.json beside this script, and writes
records/measure.json: per task, the data files profiled and their findings,
the entry points named and wide discovery found, and each run's exit and
time, without any output text; and per set, the counts the report
publishes, with 95% Wilson intervals.

Usage: measure.py [RESULTS]
"""

import json
import math
import os
import re
import statistics
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
RESULTS = os.path.expanduser("~/.openagents/coder-one/data-profile-offline/results.json")
SETS = ["source", "anatomy", "family", "wider"]

# Finding categories, by the words the profile uses.
CATEGORIES = [
    ("all-zero rows", r"all-zero rows"),
    ("NaN values", r"NaN values"),
    ("infinite values", r"infinite values"),
    ("duplicate rows or records", r"duplicate (rows|records)"),
    ("empty fields or strings", r"empty (fields|strings)"),
    ("rows with another field count", r"field count other than"),
    ("lines that don't parse as JSON", r"don't parse as JSON|invalid JSON"),
    ("null values", r"null values"),
    ("missing keys", r"missing keys"),
    ("constant columns", r"constant columns"),
    ("mixed columns", r"mixes"),
]


def wilson(k, n):
    if n == 0:
        return [None, None]
    z = 1.959964
    p = k / n
    centre = (p + z * z / (2 * n)) / (1 + z * z / n)
    half = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / (1 + z * z / n)
    return [round(max(0.0, centre - half), 2), round(min(1.0, centre + half), 2)]


def observable(run):
    return (not run["timed_out"]) and run["exit"] is not None and run["exit"] not in (126, 127)


def task_record(r):
    out = {"set": r["set"], "image": r.get("image"), "workdir": r.get("workdir"),
           "reconstructed": r.get("reconstructed", False)}
    if "skipped" in r:
        out["skipped"] = r["skipped"]
        return out
    profile = r["profile"]["output"]["profile"]
    out["profile"] = {
        "files": [
            {"path": f["path"], "kind": f["kind"], "bytes": f["bytes"], "whole": f["whole"],
             "findings": f["findings"], "ms": f["ms"]}
            for f in profile["files"]
        ],
        "skipped": profile["skipped"],
        "ms": profile["ms"],
    }
    runs = {run["entry"]: run for run in r["runs"]}
    for mode in ("named", "wide"):
        entries = r[f"discover_{mode}"]["output"]["entries"]
        out[mode] = {
            "ms": r[f"discover_{mode}"]["output"]["ms"],
            "entries": [
                {
                    "kind": e["kind"],
                    "command": e["command"],
                    "refused": e.get("refused"),
                    "exit": runs[e["command"]]["exit"] if e["command"] in runs else None,
                    "timed_out": runs[e["command"]]["timed_out"] if e["command"] in runs else None,
                    "ms": runs[e["command"]]["ms"] if e["command"] in runs else None,
                    "observable": observable(runs[e["command"]]) if e["command"] in runs else None,
                }
                for e in entries
            ],
        }
    return out


def summarize(tasks, labels):
    sets = {}
    for name in SETS:
        members = {t: r for t, r in tasks.items() if r["set"] == name}
        replayed = {t: r for t, r in members.items() if "skipped" not in r}
        n = len(replayed)
        entry = {}
        for mode in ("named", "wide"):
            has = [t for t, r in replayed.items()
                   if any(e["refused"] is None for e in r[mode]["entries"])]
            ran = [t for t, r in replayed.items()
                   if any(e["observable"] for e in r[mode]["entries"])]
            kinds = {}
            for r in replayed.values():
                for e in r[mode]["entries"]:
                    if e["refused"] is None:
                        kinds[e["kind"]] = kinds.get(e["kind"], 0) + 1
            entry[mode] = {
                "tasks_with_entry_point": len(has),
                "share": round(len(has) / n, 2) if n else None,
                "wilson": wilson(len(has), n),
                "tasks_with_observable_run": len(ran),
                "entries_by_kind": kinds,
                "tasks": sorted(has),
            }
        gained = sorted(set(entry["wide"]["tasks"]) - set(entry["named"]["tasks"]))
        lost = sorted(set(entry["named"]["tasks"]) - set(entry["wide"]["tasks"]))
        with_data = [t for t, r in replayed.items() if r["profile"]["files"]]
        with_findings = [t for t, r in replayed.items()
                         if any(f["findings"] for f in r["profile"]["files"])]
        files = [f for r in replayed.values() for f in r["profile"]["files"]]
        categories = {}
        for label, pattern in CATEGORIES:
            hit = [f for f in files if any(re.search(pattern, x) for x in f["findings"])]
            if hit:
                categories[label] = {"files": len(hit),
                                     "tasks": len({t for t, r in replayed.items()
                                                   for f in r["profile"]["files"] if f in hit})}
        profile_ms = [r["profile"]["ms"] for r in replayed.values()]
        discover_ms = [r["wide"]["ms"] for r in replayed.values()]
        run_ms = [e["ms"] for r in replayed.values() for e in r["wide"]["entries"]
                  if e["ms"] is not None]
        sets[name] = {
            "tasks": len(members),
            "replayed": n,
            "not_replayed": {t: r["skipped"] for t, r in members.items() if "skipped" in r},
            "entry_points": entry,
            "gained": gained,
            "lost": lost,
            "profile": {
                "tasks_with_data_files": len(with_data),
                "tasks_with_findings": len(with_findings),
                "files": len(files),
                "files_with_findings": sum(1 for f in files if f["findings"]),
                "files_by_kind": {k: sum(1 for f in files if f["kind"] == k)
                                  for k in sorted({f["kind"] for f in files})},
                "findings_by_category": categories,
            },
            "time_ms": {
                "profile_median": statistics.median(profile_ms) if profile_ms else None,
                "profile_max": max(profile_ms) if profile_ms else None,
                "discovery_wide_median": statistics.median(discover_ms) if discover_ms else None,
                "discovery_wide_max": max(discover_ms) if discover_ms else None,
                "run_median": statistics.median(run_ms) if run_ms else None,
                "run_max": max(run_ms) if run_ms else None,
            },
        }
        if name in labels.get("sets", []):
            labeled = {t: labels["tasks"][t] for t in replayed if t in labels["tasks"]}
            total = sum(len(v["labels"]) for v in labeled.values())

            def exposed(source):
                return [l for v in labeled.values() for l in v["labels"] if source in l["exposed_by"]]

            def tasks_exposed(source):
                return sorted(t for t, v in labeled.items()
                              if any(source in l["exposed_by"] for l in v["labels"]))

            either = sorted(t for t, v in labeled.items() if any(l["exposed_by"] for l in v["labels"]))
            sets[name]["exposure"] = {
                "tasks_labeled": len(labeled),
                "labels": total,
                "profile": {"labels": len(exposed("profile")), "tasks": tasks_exposed("profile"),
                            "wilson_tasks": wilson(len(tasks_exposed("profile")), len(labeled))},
                "baseline_named": {"labels": len(exposed("baseline_named")),
                                   "tasks": tasks_exposed("baseline_named")},
                "baseline_wide": {"labels": len(exposed("baseline_wide")),
                                  "tasks": tasks_exposed("baseline_wide")},
                "either": {"tasks": either, "wilson": wilson(len(either), len(labeled))},
            }
    return sets


def main():
    results = json.load(open(sys.argv[1] if len(sys.argv) > 1 else RESULTS))
    labels_file = os.path.join(HERE, "records", "labels.json")
    labels = json.load(open(labels_file)) if os.path.exists(labels_file) else {}
    tasks = {t: task_record(r) for t, r in sorted(results.items())}
    out = {
        "schema": "openagents.data-profile-offline.v1",
        "issue": 9654,
        "sets": summarize(tasks, labels),
        "tasks": tasks,
        "cost_usd": 0.0,
    }
    os.makedirs(os.path.join(HERE, "records"), exist_ok=True)
    with open(os.path.join(HERE, "records", "measure.json"), "w") as handle:
        json.dump(out, handle, indent=1)
        handle.write("\n")
    for name, s in out["sets"].items():
        e = s["entry_points"]
        print(f"{name}: {s['replayed']}/{s['tasks']} replayed; entry point before "
              f"{e['named']['tasks_with_entry_point']}, after {e['wide']['tasks_with_entry_point']}; "
              f"observable run before {e['named']['tasks_with_observable_run']}, after "
              f"{e['wide']['tasks_with_observable_run']}; data files in "
              f"{s['profile']['tasks_with_data_files']}, findings in {s['profile']['tasks_with_findings']}")


if __name__ == "__main__":
    main()
