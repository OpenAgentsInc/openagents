#!/usr/bin/env python3
"""Run the frozen offline suites on Microluna's graded final workspaces.

No model runs. For each task with a frozen suite from `coder-one accept
offline`, this runs `coder-one accept offline TASK --reuse --kinds
final,reconstruction`, which rebuilds each Microluna trial's final
workspace, the task image's `/app` with the trial's collected deliverables
copied over it, and runs the suite in a fresh networkless container of that
image. It then records whether the suite was green beside the trial's
official reward, in the shape `measure_finals.py` reads.

The trials are the published set: every trial of a `tb4--*microluna*` job
with a verifier reward, plus the reconstructed v12 candidate before its
editing review. `accept offline` without `--trials` also finds Microluna
trials of jobs with other names, and the candidates a lean loop retained.

Usage:
  run_finals.py --coder-one BIN --suites DIR --jobs DIR --scratch DIR

`--suites` holds `pass2-calibrated/<task>/suite.accept.json` and
`pass3-facts/<task>/suite.accept.json` with their `suite/` directories, as
`accept offline --out` wrote them; `records/` here holds them. They're
copied to `--scratch` first. The result is `--scratch/microluna-finals.json`.
"""
import argparse
import glob
import json
import os
import shutil
import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parent
TASKS = {
    "embedding-drift-monitor": "accept-env/embedding-drift-monitor:latest",
    "sound-change-cascade": "sound-change-cascade__avnavjz__env-main:latest",
    "interleaved-vigenere": "tbench-warm/interleaved-vigenere:environment-37a3e35da105ad0d814a",
    "fin-saccr-rwa": "accept-env/fin-saccr-rwa:latest",
}
PASSES = ["pass2-calibrated", "pass3-facts"]
# The reconstructed workspace before v12 r1's editing review, from
# docs/terminal-bench/2026-09-24-microluna-candidate-evidence.md: 10 of 11.
RECONSTRUCTION = (
    HERE.parent / "2026-09-24-candidate-evidence" / "records"
    / "reconstructed-v12-embedding-r1-before-review"
)


def published(jobs):
    """The trials of `tb4--*microluna*` jobs on the four tasks with a reward."""
    out = []
    for job in sorted(glob.glob(f"{jobs}/tb4--*microluna*")):
        for task in TASKS:
            for trial in sorted(glob.glob(f"{job}/{task}__*")):
                try:
                    float((Path(trial) / "verifier" / "reward.txt").read_text().strip())
                except (OSError, ValueError):
                    continue
                out.append(os.path.basename(trial))
    return out


def ctrf(trial_dir):
    path = Path(trial_dir) / "verifier" / "ctrf.json"
    if not path.is_file():
        return None
    tests = json.loads(path.read_text())["results"]["tests"]
    return [sum(t["status"] == "passed" for t in tests), len(tests)]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--coder-one", required=True)
    parser.add_argument("--suites", required=True)
    parser.add_argument("--jobs", required=True)
    parser.add_argument("--scratch", required=True)
    parser.add_argument("--workers", type=int, default=4)
    args = parser.parse_args()
    scratch = Path(args.scratch)
    trials = ",".join(published(args.jobs))
    runs = []
    for suite_pass in PASSES:
        for task, image in TASKS.items():
            source = Path(args.suites) / suite_pass / task
            if not (source / "suite.accept.json").is_file():
                continue
            out = scratch / suite_pass
            shutil.rmtree(out / task, ignore_errors=True)
            (out / task).mkdir(parents=True)
            shutil.copy2(source / "suite.accept.json", out / task)
            shutil.copytree(source / "suite", out / task / "suite")
            command = [
                args.coder_one, "accept", "offline", task, "--reuse", "--jev", "off",
                "--jobs", args.jobs, "--out", str(out), "--image", image,
                "--kinds", "final,reconstruction", "--reconstruction", str(RECONSTRUCTION),
                "--trials", trials, "--workers", str(args.workers),
            ]
            subprocess.run(command, check=True, stdout=subprocess.DEVNULL)
            record = json.loads((out / task / "validity.json").read_text())
            for entry in record["trials"]:
                trial, run = entry["trial"], entry.get("run")
                row = {
                    "task": task,
                    "job": trial["job"],
                    "trial": trial["trial"],
                    "kind": trial["kind"],
                    "reward": trial["reward"],
                    "verifier_tests": ctrf(Path(trial["episode"]).parent.parent),
                    "suite": suite_pass,
                }
                if run is None:
                    row["error"] = entry.get("error")
                else:
                    row.update({
                        "suite_digest": record["suite"]["digest"],
                        "green": run["green"],
                        "tests_green": run["passed"],
                        "tests": run["total"],
                        "red": [t["id"] for t in run["tests"] if not t["green"]],
                    })
                if trial["kind"] == "reconstruction":
                    verified = sorted(RECONSTRUCTION.glob("verification/verify__*"))
                    row["verifier_tests"] = ctrf(verified[0]) if verified else None
                runs.append(row)
                print(suite_pass, row["trial"], row["reward"], row.get("green"),
                      row.get("tests_green"), row.get("tests"), row.get("error") or "", flush=True)
    out = {
        "schema": "openagents.acceptance-first.microluna-finals.v1",
        "note": "Each workspace is the task image's /app with the trial's collected deliverables over it, as `coder-one accept offline --kinds final` builds it.",
        "runs": runs,
    }
    (scratch / "microluna-finals.json").write_text(json.dumps(out, indent=1) + "\n")


if __name__ == "__main__":
    main()
