#!/usr/bin/env python3
"""Replay the exact published sixteen-candidate supplement for issue 9628."""

import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import time

HERE = Path(__file__).resolve().parent
BINARY_SHA256 = "cdbf781be1c00814ba61bfddb6d69a581f6e3c345215b97afa4bba42b656bcd7"
TASKS = {
    "distributed-dedup", "formal-crypto", "freecad-impeller",
    "freecad-spring-clip", "math-eval-grader", "pretrain-shard-corruption",
    "shadow-relay", "vpp-loss-divergence",
}


def read(path):
    return json.loads(Path(path).read_text())


def digest(path):
    value = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


def write(path, value):
    Path(path).write_text(json.dumps(value, indent=2) + "\n")


def cohort(source):
    rows = read(source)["predictions"]
    keys = {(r["job"], r["trial"]) for r in rows}
    assert len(rows) == len(keys) == 16, "expected sixteen distinct candidates"
    assert {r["task"].split("/")[-1] for r in rows} == TASKS
    assert all(sum(r["task"].endswith("/" + task) and r["executor"] == executor
                   for r in rows) == 1 for task in TASKS for executor in ("luna", "astra"))
    assert all("9584-r" in r["job"] and "artifact" not in r["job"] for r in rows)
    return rows


def prepare(args):
    source = args.source / "prospective-measurement.json"
    rows = cohort(source)
    assert digest(args.binary) == BINARY_SHA256, "binary differs from protocol"
    args.out.mkdir(parents=True, exist_ok=False)
    staging = args.out / "jobs"
    staging.mkdir()
    references = {}
    for name in ("prospective-trace-files.json", "prospective-astra-trace-files.json"):
        references.update({r["path"]: r for r in read(args.source / name)["files"]})
    verified = []
    for row in rows:
        prefix = row["job"] + "/" + row["trial"] + "/"
        files = [r for key, r in references.items() if key.startswith(prefix)]
        assert files, f"no retained evidence for {prefix}"
        for entry in files:
            path = args.jobs / entry["path"]
            assert path.stat().st_size == entry["bytes"], f"size changed: {path}"
            assert digest(path) == entry["sha256"], f"digest changed: {path}"
        job = staging / row["job"]
        job.mkdir(exist_ok=True)
        (job / row["trial"]).symlink_to((args.jobs / row["job"] / row["trial"]).resolve())
        verified.extend(files)
    images = {}
    for task in sorted(TASKS):
        tag = f"truth9584-review/{task}:public-environment"
        image = json.loads(subprocess.check_output(["docker", "image", "inspect", tag]))[0]
        images[task] = {"tag": tag, "id": image["Id"],
                        "instruction_sha256": digest(args.tasks / task / "instruction.md")}
    write(args.out / "inputs.json", {
        "schema": "openagents.contract-supplement-inputs.v1",
        "created_utc": datetime.now(timezone.utc).isoformat(),
        "binary_sha256": BINARY_SHA256,
        "measurement_sha256": digest(source),
        "trace_manifests": {name: digest(args.source / name) for name in (
            "prospective-trace-files.json", "prospective-astra-trace-files.json")},
        "images": images, "candidates": rows, "verified_files": verified,
    })
    print(f"Verified {len(verified)} retained files for sixteen candidates", flush=True)


def run(args):
    inputs = read(args.out / "inputs.json")
    assert digest(args.binary) == inputs["binary_sha256"] == BINARY_SHA256
    records = args.out / "records"
    records.mkdir(exist_ok=True)
    for task, image in sorted(inputs["images"].items()):
        assert digest(args.tasks / task / "instruction.md") == image["instruction_sha256"]
        receipt = args.out / f"{task}.process.json"
        if receipt.exists():
            print(f"Retaining existing invocation: {task}", flush=True)
            continue
        trial_ids = [r["trial"] for r in inputs["candidates"] if r["task"].endswith("/" + task)]
        if args.plans:
            destination = records / task
            destination.mkdir(exist_ok=True)
            shutil.copy2(args.plans / task / "plan.json", destination / "plan.json")
        command = [str(args.binary), "checks", "contract", "offline", task,
                   "--jobs", str(args.out / "jobs"), "--tasks", str(args.tasks),
                   "--out", str(records), "--image", image["id"],
                   "--trials", ",".join(trial_ids), "--kinds", "snapshot,final",
                   "--workers", "1", "--jev", "recorded" if args.plans else "live"]
        if args.plans:
            command.append("--reuse-plan")
        start = time.monotonic()
        write(receipt, {"state": "started", "command": command})
        with (args.out / f"{task}.log").open("w") as log:
            completed = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, check=False)
        write(receipt, {"state": "finished", "command": command,
                        "exit": completed.returncode, "seconds": time.monotonic() - start})
        print(f"{task}: exit {completed.returncode}, {time.monotonic() - start:.1f}s", flush=True)
        tokens = sum(call.get("input_tokens") or 0
                     for path in records.glob("*/plan.json") for call in read(path).get("jev", []))
        # Jev's retained list-price rate is $0.042 per million input tokens.
        assert tokens * 0.042 / 1_000_000 < 0.05, "Jev spend ceiling reached"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("prepare", "run"))
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--jobs", type=Path, required=True)
    parser.add_argument("--tasks", type=Path, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--plans", type=Path, help="Replay saved plans without model calls")
    args = parser.parse_args()
    (prepare if args.command == "prepare" else run)(args)


if __name__ == "__main__":
    main()
