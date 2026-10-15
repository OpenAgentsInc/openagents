#!/usr/bin/env python3
"""Run alternating, matched CPU comparisons for the pinned small models."""
import argparse
import json
import os
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--families",nargs="+",choices=["router","potion","mnist"],default=["potion","mnist"])
parser.add_argument("--threads",nargs="+",type=int,default=[1,4])
parser.add_argument("--min-tokens",type=int,default=1)
parser.add_argument("--max-tokens",type=int,default=512)
parser.add_argument("--binary", type=Path, required=True)
parser.add_argument("--models", type=Path, required=True)
parser.add_argument("--ort-library", type=Path, required=True)
parser.add_argument("--ort-sha256", required=True)
parser.add_argument("--output", type=Path, required=True)
parser.add_argument("--cpus", help="Linux taskset CPU list")
parser.add_argument("--samples", type=int, default=1000)
parser.add_argument("--rounds", type=int, default=2)
args = parser.parse_args()
if args.rounds < 2 or args.samples < 1000:
    parser.error("Use at least two rounds and 1,000 samples")
args.output.parent.mkdir(parents=True, exist_ok=True)
with args.output.open("x") as output:
    for family in args.families:
        for threads in args.threads:
            for repetition in range(args.rounds):
                engines = ["ort", "native"] if repetition % 2 == 0 else ["native", "ort"]
                for engine in engines:
                    command = [str(args.binary.resolve()), "--family", family, "--engine", engine,
                               "--model-dir", str((args.models / ("s1-router" if family=="router" else family)).resolve()), "--threads", str(threads),
                               "--ort-library", str(args.ort_library.resolve()), "--ort-sha256", args.ort_sha256,
                               "--samples", str(args.samples),"--min-tokens",str(args.min_tokens),"--max-tokens",str(args.max_tokens)]
                    if args.cpus:
                        command = ["taskset", "-c", args.cpus] + command
                    record = json.loads(subprocess.check_output(command, text=True))
                    record.update(repetition=repetition, cpu_affinity=args.cpus, driver_command=command,
                                  other_load=os.getloadavg())
                    output.write(json.dumps(record, separators=(",", ":")) + "\n")
                    output.flush()
                    print(f"{family} {engine} threads={threads} round={repetition}: p50={record['p50_ms']:.4f} ms", flush=True)
