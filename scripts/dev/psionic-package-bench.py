#!/usr/bin/env python3
"""Compare native CPU, full ORT, and reduced ORT using identical model fixtures."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument("--binary",type=Path,required=True)
parser.add_argument("--models",type=Path,required=True)
parser.add_argument("--full-runtime",type=Path,required=True)
parser.add_argument("--reduced-runtime",type=Path,required=True)
parser.add_argument("--converted",type=Path,required=True)
parser.add_argument("--output",type=Path,required=True)
parser.add_argument("--cpus")
parser.add_argument("--samples",type=int,default=1000)
args=parser.parse_args()
if args.samples<1000:parser.error("Use at least 1,000 samples")
def checksum(path):
    h=hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda:stream.read(1024*1024),b""):h.update(block)
    return h.hexdigest()
full=args.full_runtime.resolve();reduced=args.reduced_runtime.resolve()
hashes={str(p):checksum(p) for p in [full,reduced]}
converted={name:(args.converted/f"{name}.ort").resolve() for name in ["router","potion","mnist"]}
converted_hashes={name:checksum(path) for name,path in converted.items()}
args.output.parent.mkdir(parents=True,exist_ok=True)
with args.output.open("x") as output:
 for family in ["potion","mnist","router"]:
  for repetition in range(2):
   choices=["native","full","reduced"] if repetition==0 else ["reduced","full","native"]
   for package in choices:
    command=[str(args.binary.resolve()),"--family",family,"--engine","native" if package=="native" else "ort",
      "--model-dir",str((args.models/("s1-router" if family=="router" else family)).resolve()),"--threads","4","--samples",str(args.samples)]
    if package!="native":
     library=full if package=="full" else reduced
     command += ["--ort-library",str(library),"--ort-sha256",hashes[str(library)]]
    if package=="reduced":command += ["--ort-model",str(converted[family]),"--ort-model-sha256",converted_hashes[family]]
    if args.cpus:command=["taskset","-c",args.cpus]+command
    row=json.loads(subprocess.check_output(command,text=True))
    row.update(package=package,repetition=repetition,cpu_affinity=args.cpus,other_load=os.getloadavg(),driver_command=command)
    output.write(json.dumps(row,separators=(",",":"))+"\n");output.flush()
    print(f"{family}/{package}: p50 {row['p50_ms']:.4f} ms; p99 {row['p99_ms']:.4f} ms",flush=True)
