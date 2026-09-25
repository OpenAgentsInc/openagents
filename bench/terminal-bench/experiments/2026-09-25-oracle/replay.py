#!/usr/bin/env python3
"""Replays the `checks.oracle` tier 0 measurement with no model call.

Usage: replay.py --coder-one BIN --scratch DIR [--records DIR] [--tasks A,B]

For each task in the retained records, copies the frozen spec and the
written oracle into DIR, reruns `coder-one checks oracle offline` with Jev
answering from the recorded answers and writing off, and compares every
workspace's case verdicts with the retained results. Containers are named
oracle-9656-*. Needs Docker and the task images.
"""

import argparse
import json
import os
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
GRADES = os.path.join(
    HERE, "..", "2026-09-24-candidate-evidence", "records", "candidate-grades"
)


def verdicts(results):
    out = {}
    for t in results.get("trials", []):
        r = t.get("result") or {}
        out[t["trial"]] = [c["verdict"] for c in r.get("cases", [])] if r else t.get("error")
    u = results.get("untouched") or {}
    out["untouched"] = [c["verdict"] for c in u.get("cases", [])]
    return out


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--coder-one", required=True)
    parser.add_argument("--scratch", required=True)
    parser.add_argument("--records", default=os.path.join(HERE, "records"))
    parser.add_argument("--tasks", default="")
    args = parser.parse_args()
    tasks = [t for t in args.tasks.split(",") if t] or sorted(
        d for d in os.listdir(args.records)
        if os.path.isfile(os.path.join(args.records, d, "results.json"))
    )
    os.makedirs(args.scratch, exist_ok=True)
    shutil.copy(
        os.path.join(args.records, "jev-recorded.json"),
        os.path.join(args.scratch, "jev-recorded.json"),
    )
    same = 0
    differ = []
    for task in tasks:
        src = os.path.join(args.records, task)
        dst = os.path.join(args.scratch, task)
        os.makedirs(dst, exist_ok=True)
        for name in ("spec.json", "oracle.json"):
            if os.path.exists(os.path.join(src, name)):
                shutil.copy(os.path.join(src, name), os.path.join(dst, name))
        image = json.load(open(os.path.join(src, "results.json"))).get("image")
        subprocess.run(
            [
                args.coder_one, "checks", "oracle", "offline", task, "--image", image,
                "--out", args.scratch, "--jev", "recorded", "--write", "off",
                "--reuse-spec", "--grades", GRADES,
                "--exclude-job", "9584", "--exclude-job", "truth-confirmation",
                "--exclude-job", "truth-control", "--workers", "3",
            ],
            check=False,
        )
        before = verdicts(json.load(open(os.path.join(src, "results.json"))))
        after_path = os.path.join(dst, "results.json")
        after = verdicts(json.load(open(after_path))) if os.path.exists(after_path) else {}
        for trial, v in before.items():
            if after.get(trial) == v:
                same += 1
            else:
                differ.append({"task": task, "trial": trial, "retained": v, "replayed": after.get(trial)})
    print(json.dumps({"identical": same, "different": differ}, indent=2))
    return 0 if not differ else 1


if __name__ == "__main__":
    sys.exit(main())
