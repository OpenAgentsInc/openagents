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
def atomic_text(path, text):
    temporary = path.with_suffix(path.suffix + ".part")
    temporary.write_text(text)
    temporary.replace(path)


records = [json.loads(line) for line in (args.output / "runs.jsonl").read_text().splitlines()] if (args.output / "runs.jsonl").exists() else []
binary_hash = hashlib.sha256(args.binary.read_bytes()).hexdigest()
if any(r["package"]["executable_sha256"] != binary_hash or r["statistics"]["samples"] != args.samples
       or r["cpu_affinity"] != args.cpus or r["round"] > args.rounds
       or (r["engine"]["engine"] == "ort" and r["engine"]["runtime_sha256"] != library_hash) for r in records):
    parser.error("Existing results use different binaries or run settings; choose a new output directory")
completed = {(r["round"], r["engine"]["engine"], r["total_threads"], r["concurrency"], r["bucket"], r["mode"]) for r in records}
for round_index in range(args.rounds):
    engines = ["ort", "native"] if round_index % 2 == 0 else ["native", "ort"]
    for threads, concurrency, bucket, mode in matrix:
        for engine in engines:
            key = (round_index + 1, engine, threads, concurrency, bucket.replace("-", "_"), mode)
            if key in completed:
                continue
            command = [str(args.binary.resolve()), "--engine", engine, "--model-dir", str(args.model_dir.resolve()),
                       "--threads", str(threads), "--concurrency", str(concurrency), "--samples", str(args.samples),
                       "--bucket", bucket, "--mode", mode]
            if engine == "ort":
                command += ["--ort-library", str(args.ort_library.resolve()), "--ort-sha256", library_hash]
            if args.cpus:
                command = ["taskset", "-c", args.cpus] + command
            name = f"{round_index + 1}-{engine}-{threads}-{concurrency}-{bucket}-{mode}"
            result = subprocess.run(command, text=True, capture_output=True)
            atomic_text(args.output / f"{name}.stderr", result.stderr)
            if result.returncode:
                sys.stderr.write(result.stderr)
                raise SystemExit(f"Failed benchmark {name}: exit {result.returncode}")
            record = json.loads(result.stdout)
            record["round"] = round_index + 1
            record["cpu_affinity"] = args.cpus
            record["launch_command"] = command
            atomic_text(args.output / f"{name}.json", json.dumps(record, sort_keys=True) + "\n")
            records.append(record)
            atomic_text(args.output / "runs.jsonl", "".join(json.dumps(r, sort_keys=True) + "\n" for r in records))
            stats = record["statistics"]
            print(f"{name}: p50={stats['p50_ms']:.3f} ms p99={stats['p99_ms']:.3f} ms {stats['requests_per_second']:.1f} requests/s", flush=True)

def binary(path):
    return {"bytes": path.stat().st_size, "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}

summary = {"schema": "psionic.model.benchmark.study.v1", "rounds": args.rounds, "samples_per_run": args.samples,
           "native_only_binary": binary(args.native_binary), "both_engines_binary": binary(args.binary),
           "ort_library": binary(args.ort_library),
           "remaining_coverage": ["Apple Silicon CPU", "cold filesystem caches", "accelerators", "HTTP"]}
summary["incremental_both_engines_bytes"] = summary["both_engines_binary"]["bytes"] + summary["ort_library"]["bytes"] - summary["native_only_binary"]["bytes"]
atomic_text(args.output / "study.json", json.dumps(summary, indent=2) + "\n")
