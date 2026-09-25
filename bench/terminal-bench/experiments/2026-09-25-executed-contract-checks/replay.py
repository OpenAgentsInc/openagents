#!/usr/bin/env python3
"""Replay the executed contract checks from the retained records, with no model calls.

For each task under `records/`, this copies the retained `plan.json` and
`jev-recorded.json` to a scratch directory, runs

    coder-one checks contract offline TASK --reuse-plan --jev recorded ...

which restores every retained workspace into its own networkless container
of the task's image and runs the frozen plan, then compares each
workspace's item outcomes, call, and score with the retained
`contract.json`. It prints every difference and exits 1 if there is one.

Usage: replay.py --coder-one BIN --scratch DIR [--tasks TASK,...] [--workers N]

The images are the ones `protocol.md` names: `tbench-warm/<task>:environment-*`
or `accept-env/<task>:latest`, and for two tasks the image IMAGES names.
The retained workspaces are read from ~/.openagents/terminal-bench/jobs.
"""
import argparse
import json
import shutil
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
EXPERIMENTS = HERE.parent
IMAGES = {
    "sound-change-cascade": "sound-change-cascade__avnavjz__env-main:latest",
    "uefi-bootkit": "uefi-bootkit__al5vpch__env-main:latest",
}


def outcomes(record):
    out = {}
    for trial in record["trials"]:
        report = trial.get("report")
        if report is None:
            out[trial["trial"]] = ("error", trial.get("error"))
            continue
        out[trial["trial"]] = (
            report["call"],
            report["score"],
            tuple((i["id"], i["outcome"]) for i in report["items"]),
        )
    return out


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--coder-one", required=True)
    parser.add_argument("--scratch", required=True)
    parser.add_argument("--tasks")
    parser.add_argument("--workers", type=int, default=3)
    args = parser.parse_args()
    records = HERE / "records"
    scratch = Path(args.scratch)
    tasks = args.tasks.split(",") if args.tasks else sorted(
        p.parent.name for p in records.glob("*/plan.json"))
    scratch.mkdir(parents=True, exist_ok=True)
    shutil.copy2(records / "jev-recorded.json", scratch / "jev-recorded.json")
    differences = 0
    for task in tasks:
        (scratch / task).mkdir(exist_ok=True)
        shutil.copy2(records / task / "plan.json", scratch / task / "plan.json")
        command = [
            args.coder_one, "checks", "contract", "offline", task, "--reuse-plan",
            "--jev", "recorded", "--out", str(scratch), "--workers", str(args.workers),
            "--grades", str(EXPERIMENTS / "2026-09-24-candidate-evidence/records/candidate-grades"),
            "--grades", str(EXPERIMENTS / "2026-09-24-iteration-speed/records/grading-parallel"),
            "--reconstruction", str(EXPERIMENTS / "2026-09-24-candidate-evidence/records/reconstructed-v12-embedding-r1-before-review"),
            "--exclude-job", "truth-confirmation", "--exclude-job", "truth-control",
        ]
        if task in IMAGES:
            command += ["--image", IMAGES[task]]
        subprocess.run(command, check=False, stdout=subprocess.DEVNULL)
        want = outcomes(json.loads((records / task / "contract.json").read_text()))
        got_path = scratch / task / "contract.json"
        got = outcomes(json.loads(got_path.read_text())) if got_path.is_file() else {}
        same = 0
        for trial in sorted(set(want) | set(got)):
            if want.get(trial) == got.get(trial):
                same += 1
            else:
                differences += 1
                print(f"DIFFERS {task} {trial}: retained {want.get(trial)} replayed {got.get(trial)}")
        print(f"{task}: {same} of {len(set(want) | set(got))} workspaces identical", flush=True)
    print(f"{differences} differences")
    sys.exit(1 if differences else 0)


if __name__ == "__main__":
    main()
