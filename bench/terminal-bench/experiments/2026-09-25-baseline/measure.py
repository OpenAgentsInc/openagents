#!/usr/bin/env python3
"""Measure evidence.baseline offline (issue #9633).

Reads the component's results on the anatomy's 18 untouched workspaces
(written by `coder-one component run evidence.baseline --json`, one fixture
per task) and the retained Microluna session logs under
bench/terminal-bench/traces, and writes records/measure.json:

- per task: the entry points found, run, and refused, with each run's
  command, exit, and time;
- per session log: the commands before the first edit, and how many of
  them duplicate a baseline command the host could have run, by three
  rules stated in protocol.md.

Usage: measure.py RESULTS.json [--traces DIR] [--out FILE]
"""

import argparse
import glob
import json
import os
import re

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "../../../.."))


def collapse(text):
    return " ".join(text.split())


def first_edit(calls):
    """Index of the first completed edit of the solution: an apply_patch or
    write_file outside /tmp that isn't the evaluation script."""
    for i, call in enumerate(calls):
        if call["name"] not in ("apply_patch", "write_file"):
            continue
        if call.get("outcome") != "Completed":
            continue
        args = call["arguments"]
        path = args.get("path") or ""
        if call["name"] == "apply_patch":
            files = re.findall(r"\*\*\* (?:Update|Add|Delete) File: (\S+)", args.get("patch", ""))
            path = files[0] if files else ""
        if path.startswith("/tmp/") or path.endswith("score.sh"):
            continue
        return i
    return None


def classify(command, baseline):
    """Which duplicate rules a session command meets against one baseline
    run:

    - exact: it contains the baseline command, spaces collapsed (the
      finish rule's match, #9638);
    - same_run: it runs the same entry point on the same set of data files,
      in any order and by any path;
    - entry: it runs the same entry point, with any arguments;
    - purpose: it runs the entry point, or imports its package, and names
      one of the baseline's data files.

    A command that only writes a file with a here-document (the evaluation
    script, for instance) doesn't run anything it writes, so it meets no
    rule unless it also runs the entry point outside the here-document."""
    out = set()
    if not baseline:
        return out
    text = collapse(command)
    base = collapse(baseline["command"])
    if base in text:
        out.add("exact")
    # Drop here-documents written to a file: `cat > FILE <<'X' ... X`.
    ran = re.sub(r"cat\s*>\s*\S+\s*<<\s*'?(\w+)'?\n.*?\n\1(\n|$)", "", command, flags=re.S)
    words = base.split()
    data = [os.path.basename(w) for w in words[1:] if "." in os.path.basename(w) and "/" in w]
    stems = [d.rsplit(".", 1)[0] for d in data]
    package = None
    if baseline["kind"] == "module" and "-m" in words:
        package = words[words.index("-m") + 1]
    if package:
        entry = re.compile(r"-m\s+" + re.escape(package) + r"\b|" + re.escape(package) + r"/__main__\.py")
        if entry.search(ran):
            out.add("entry")
            if data and all(d in ran for d in data):
                out.add("same_run")
        imports = re.search(r"\b(from|import)\s+" + re.escape(package) + r"\b", ran)
        if (imports or "entry" in out) and any(s in ran for s in stems):
            out.add("purpose")
    return out


def sessions(traces, known):
    for path in sorted(glob.glob(os.path.join(traces, "*", "*", "artifacts", "microluna-*-*.atif.jsonl"))):
        run = path.split(os.sep)[-4]
        task = next((t for t in sorted(known, key=len, reverse=True) if f"--{t}" in run), None)
        calls, start = [], None
        for line in open(path):
            record = json.loads(line)
            if record.get("record") == "session":
                start = record.get("at")
            step = record.get("step") or {}
            call = step.get("call")
            if call:
                call = dict(call)
                call["at"] = step.get("at")
                calls.append(call)
        yield path, run, task, start, calls


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("results")
    parser.add_argument("--traces", default=os.path.join(REPO, "bench/terminal-bench/traces"))
    parser.add_argument("--out", default=os.path.join(HERE, "records/measure.json"))
    args = parser.parse_args()
    results = json.load(open(args.results))
    tasks = {}
    for task, result in sorted(results.items()):
        b = result["output"]["baseline"]
        tasks[task] = {
            "runs": [
                dict({k: r.get(k) for k in ("kind", "command", "stated", "exit", "timed_out", "ms", "failed", "why")},
                     stdout_head=r["stdout_head"][:2000], stderr_head=r["stderr_head"][:2000])
                for r in b["runs"]
            ],
            "refused": b["refused"],
            "none": b.get("none"),
            "untouched": b.get("untouched"),
            "commands": result["output"]["commands"],
        }
    logs = []
    for path, run, task, start, calls in sessions(args.traces, tasks):
        session = int(re.search(r"microluna-\d+-(\d+)\.atif", path).group(1))
        cut = first_edit(calls)
        before = calls if cut is None else calls[:cut]
        commands = [c for c in before if c["name"] == "run_command"]
        base = tasks.get(task, {}).get("runs", [])
        rows = []
        for c in commands:
            hits = set()
            for b in base:
                hits |= classify(c["arguments"]["command"], b)
            rows.append({
                "command": c["arguments"]["command"][:300],
                "exit": (c.get("extra") or {}).get("exit"),
                "rules": sorted(hits),
            })
        logs.append({
            "log": os.path.relpath(path, REPO),
            "run": run,
            "task": task,
            "session": session,
            "calls": len(calls),
            "first_edit_call": cut,
            "first_edit_s": None if cut is None or start is None else round((calls[cut]["at"] - start) / 1000, 1),
            "commands_before_edit": len(commands),
            "exact": sum("exact" in r["rules"] for r in rows),
            "same_run": sum("same_run" in r["rules"] for r in rows),
            "entry": sum("entry" in r["rules"] for r in rows),
            "purpose": sum("purpose" in r["rules"] for r in rows),
            "baseline_commands": len(tasks.get(task, {}).get("commands", [])),
            "rows": rows,
        })
    summary = {}
    for log in logs:
        key = f"{log['task']} session {log['session']}"
        s = summary.setdefault(key, {"logs": 0, "commands_before_edit": 0, "exact": 0, "same_run": 0,
                                     "entry": 0, "purpose": 0, "logs_with_same_run": 0,
                                     "logs_with_purpose": 0, "duplicates_not_found": 0,
                                     "first_edit_s": []})
        s["logs"] += 1
        for k in ("commands_before_edit", "exact", "same_run", "entry", "purpose"):
            s[k] += log[k]
        s["logs_with_same_run"] += log["same_run"] > 0
        s["logs_with_purpose"] += log["purpose"] > 0
        s["duplicates_not_found"] += sum(1 for r in log["rows"] if r["rules"] and r["exit"] == 127)
        if log["first_edit_s"] is not None:
            s["first_edit_s"].append(log["first_edit_s"])
    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    json.dump({"schema": "openagents.tbench.baseline-offline.v1", "tasks": tasks, "summary": summary,
               "logs": logs}, open(args.out, "w"), indent=1)
    print(json.dumps(summary, indent=1))
    for log in logs:
        print(log["task"], log["session"], log["run"][-40:], "before-edit", log["commands_before_edit"],
              "exact", log["exact"], "same_run", log["same_run"], "entry", log["entry"], "purpose", log["purpose"], "first_edit_s", log["first_edit_s"])


if __name__ == "__main__":
    main()
