#!/usr/bin/env python3
"""Run the frozen offline suites on Microluna's graded final workspaces.

No model runs. For each task with a frozen suite from `coder-one accept
offline`, this rebuilds each Microluna trial's final workspace as the task
image's `/app` with the trial's collected deliverables copied over it, runs
`coder-one accept run` in a fresh networkless container of that image, and
records whether the suite was green beside the trial's official reward.

Usage:
  run_finals.py --coder-one BIN --suites DIR --jobs DIR --scratch DIR

`--suites` holds `pass2-calibrated/<task>/suite.accept.json` and
`pass3-facts/<task>/suite.accept.json` with their `suite/` directories, as
`accept offline --out` wrote them. The result is `records/microluna-finals.json`.
"""
import argparse
import concurrent.futures
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
    / "reconstructed-v12-embedding-r1-before-review" / "source-files"
)


def base_app(image, into):
    """Copies the image's /app to `into`/app."""
    if (into / "app").is_dir():
        return
    into.mkdir(parents=True, exist_ok=True)
    cid = subprocess.run(["docker", "create", image], check=True, capture_output=True, text=True).stdout.strip()
    try:
        subprocess.run(["docker", "cp", f"{cid}:/app", str(into)], check=True, capture_output=True)
    finally:
        subprocess.run(["docker", "rm", "-f", cid], capture_output=True)


def overlay(src, dst):
    for root, dirs, files in os.walk(src):
        dirs[:] = [d for d in dirs if d != "__pycache__"]
        rel = Path(root).relative_to(src)
        (dst / rel).mkdir(parents=True, exist_ok=True)
        for name in files:
            shutil.copy2(Path(root) / name, dst / rel / name)


def trials(jobs):
    out = []
    for job in sorted(glob.glob(f"{jobs}/tb4--*microluna*")):
        for task in TASKS:
            for trial in sorted(glob.glob(f"{job}/{task}__*")):
                reward_file = Path(trial) / "verifier" / "reward.txt"
                if not reward_file.is_file():
                    continue
                try:
                    reward = float(reward_file.read_text().strip())
                except ValueError:
                    continue
                ctrf = Path(trial) / "verifier" / "ctrf.json"
                tests = None
                if ctrf.is_file():
                    results = json.loads(ctrf.read_text())["results"]["tests"]
                    tests = [sum(t["status"] == "passed" for t in results), len(results)]
                out.append({
                    "task": task,
                    "job": os.path.basename(job),
                    "trial": os.path.basename(trial),
                    "reward": reward,
                    "verifier_tests": tests,
                    "artifacts": str(Path(trial) / "artifacts" / "app"),
                })
    return out


def run_one(args, row, suite_pass):
    record = Path(args.suites) / suite_pass / row["task"] / "suite.accept.json"
    if not record.is_file():
        return None
    candidate = Path(args.scratch) / "candidates" / row["trial"]
    command = [
        args.coder_one, "accept", "run", str(record), "/app",
        "--docker", TASKS[row["task"]], "--workdir", "/app",
        "--candidate", str(candidate), "--json",
    ]
    done = subprocess.run(command, capture_output=True, text=True, timeout=1800)
    try:
        result = json.loads(done.stdout)
    except json.JSONDecodeError:
        return {**row, "suite": suite_pass, "error": (done.stderr or done.stdout)[-400:]}
    return {
        **row,
        "suite": suite_pass,
        "suite_digest": result.get("digest"),
        "green": result["green"],
        "tests_green": result["passed"],
        "tests": result["total"],
        "red": [t["id"] for t in result["tests"] if not t["green"]],
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--coder-one", required=True)
    parser.add_argument("--suites", required=True)
    parser.add_argument("--jobs", required=True)
    parser.add_argument("--scratch", required=True)
    parser.add_argument("--workers", type=int, default=4)
    args = parser.parse_args()
    scratch = Path(args.scratch)
    rows = trials(args.jobs)
    v12_r1 = next((r for r in rows if r["trial"] == "embedding-drift-monitor__uU3ZNb9"), None)
    if v12_r1 and RECONSTRUCTION.is_dir():
        rows.append({
            **v12_r1,
            "trial": "embedding-drift-monitor__uU3ZNb9.before-review",
            "reward": 0.0,
            "verifier_tests": [10, 11],
            "reconstruction": str(RECONSTRUCTION),
        })
    for row in rows:
        base = scratch / "base" / row["task"]
        base_app(TASKS[row["task"]], base)
        candidate = scratch / "candidates" / row["trial"]
        shutil.rmtree(candidate, ignore_errors=True)
        shutil.copytree(base / "app", candidate / "app", symlinks=True)
        if Path(row["artifacts"]).is_dir():
            overlay(Path(row["artifacts"]), candidate / "app")
        if row.get("reconstruction"):
            overlay(Path(row["reconstruction"]), candidate / "app")
    work = [(row, p) for row in rows for p in PASSES]
    results = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.workers) as pool:
        for result in pool.map(lambda w: run_one(args, *w), work):
            if result is not None:
                results.append(result)
                print(result["suite"], result["trial"], result["reward"],
                      result.get("green"), result.get("tests_green"), result.get("tests"),
                      result.get("error", ""), flush=True)
    for r in results:
        r.pop("artifacts", None)
    out = {
        "schema": "openagents.acceptance-first.microluna-finals.v1",
        "note": "Each workspace is the task image's /app with the trial's collected deliverables over it.",
        "runs": results,
    }
    (HERE / "records" / "microluna-finals.json").write_text(json.dumps(out, indent=1) + "\n")


if __name__ == "__main__":
    main()
