#!/usr/bin/env python3
"""Tallies the microluna-v18 family trials from their records (run on the host)."""
import glob
import json
import os

HOME = os.path.expanduser("~")
JOBS = f"{HOME}/.openagents/terminal-bench/jobs"
DIGEST = "05aac15cefa419cfc3dc2db9225ace213bda351c252a9590d5d50de6b717c445"
rows = []
for job in sorted(glob.glob(f"{JOBS}/tb4--coder-one-microluna-v18--*--family-*")):
    name = os.path.basename(job)
    task = name.split("--")[2]
    label = name.split("--family-")[1].split("-")[0]
    row = {"job": name, "task": task, "label": label}
    att = glob.glob(f"{job}/tbench/attempts/*.json")
    if att:
        d = json.load(open(att[0]))
        row["trial"] = d["attempt"]["trial"]
        row["reward"] = d["outcome"]["reward"]
        exc = d["outcome"].get("exception")
        row["exception"] = (exc.get("type") if isinstance(exc, dict) else exc)
        t = d["timing"]
        row.update(started_at=t["started_at"], finished_at=t["finished_at"],
                   trial_sec=(t["total_ms"] or 0) / 1000, agent_sec=(t["agent_execution_ms"] or 0) / 1000)
    usage = glob.glob(f"{job}/*/agent/episode/evaluation/usage.json")
    if usage:
        u = json.load(open(usage[0]))
        c = u["cost"]
        comp = u.get("components", {})
        jev = (comp.get("jev") or {}).get("cost_lower_bound_usd") or 0.0
        luna = ((comp.get("delegate") or {}).get("cost_lower_bound_usd"))
        row["recorded_usd"] = c.get("lower_bound_usd") or 0.0
        row["jev_usd"] = jev
        row["luna_usd"] = luna
        row["unknown_calls"] = c.get("unknown_calls", 0)
        row["cost_lower_bound"] = c.get("amount_usd") is None
        row["threshold_usd"] = max(row["recorded_usd"], 0.09 + jev) if row["cost_lower_bound"] else row["recorded_usd"]
    else:
        row["recorded_usd"] = 0.0
        row["threshold_usd"] = 0.0
    tests = glob.glob(f"{job}/*/verifier/ctrf.json") + glob.glob(f"{job}/*/verifier/*/ctrf.json")
    if tests:
        s = json.load(open(tests[0])).get("results", {}).get("summary", {})
        row["tests_passed"] = s.get("passed")
        row["tests_total"] = s.get("tests")
    doctor = glob.glob(f"{job}/*/agent/episode-doctor.txt")
    row["digest_ok"] = (DIGEST in open(doctor[0]).read()) if doctor else None
    rows.append(row)
print(json.dumps(rows, indent=1))
