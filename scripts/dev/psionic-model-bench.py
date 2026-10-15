#!/usr/bin/env python3
"""Run repeated ORT/native pairs serially and retain benchmark evidence."""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--binary", type=Path, required=True)
parser.add_argument("--native-binary", type=Path, required=True)
parser.add_argument("--model-dir", type=Path, required=True)
parser.add_argument("--ort-library", type=Path, required=True)
parser.add_argument("--output", type=Path, required=True)
parser.add_argument("--samples", type=int, default=1000)
parser.add_argument("--rounds", type=int, default=3)
parser.add_argument("--cpus", help="Optional taskset CPU list; recorded with each run")
args = parser.parse_args()
if args.samples < 1 or args.rounds < 2:
    parser.error("Use positive samples and at least two A/B rounds")
args.output.mkdir(parents=True, exist_ok=True)
library_hash = hashlib.sha256(args.ort_library.read_bytes()).hexdigest()
matrix = [
    (1, 1, "route-v1", "engine"),
    (4, 1, "route-v1", "engine"),
    (4, 1, "short", "engine"),
    (4, 1, "medium", "engine"),
    (4, 1, "long", "engine"),
    (4, 1, "route-v1", "text"),
    (4, 4, "route-v1", "engine"),
]
records = []
for round_index in range(args.rounds):
    engines = ["ort", "native"] if round_index % 2 == 0 else ["native", "ort"]
    for threads, concurrency, bucket, mode in matrix:
        for engine in engines:
            command = [str(args.binary.resolve()), "--engine", engine, "--model-dir", str(args.model_dir.resolve()),
                       "--threads", str(threads), "--concurrency", str(concurrency), "--samples", str(args.samples),
                       "--bucket", bucket, "--mode", mode]
            if engine == "ort":
                command += ["--ort-library", str(args.ort_library.resolve()), "--ort-sha256", library_hash]
            if args.cpus:
                command = ["taskset", "-c", args.cpus] + command
            name = f"{round_index + 1}-{engine}-{threads}-{concurrency}-{bucket}-{mode}"
            result = subprocess.run(command, text=True, capture_output=True)
            (args.output / f"{name}.stderr").write_text(result.stderr)
            if result.returncode:
                sys.stderr.write(result.stderr)
                raise SystemExit(f"Failed benchmark {name}: exit {result.returncode}")
            record = json.loads(result.stdout)
            record["round"] = round_index + 1
            record["cpu_affinity"] = args.cpus
            record["launch_command"] = command
            records.append(record)
            (args.output / "runs.jsonl").write_text("".join(json.dumps(r, sort_keys=True) + "\n" for r in records))
            stats = record["statistics"]
            print(f"{name}: p50={stats['p50_ms']:.3f} ms p99={stats['p99_ms']:.3f} ms {stats['requests_per_second']:.1f} requests/s", flush=True)

def binary(path):
    return {"bytes": path.stat().st_size, "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}

summary = {"schema": "psionic.model.benchmark.study.v1", "rounds": args.rounds, "samples_per_run": args.samples,
           "native_only_binary": binary(args.native_binary), "both_engines_binary": binary(args.binary),
           "ort_library": binary(args.ort_library),
           "remaining_coverage": ["Apple Silicon CPU", "cold filesystem caches", "accelerators", "HTTP"]}
summary["incremental_both_engines_bytes"] = summary["both_engines_binary"]["bytes"] + summary["ort_library"]["bytes"] - summary["native_only_binary"]["bytes"]
(args.output / "study.json").write_text(json.dumps(summary, indent=2) + "\n")
