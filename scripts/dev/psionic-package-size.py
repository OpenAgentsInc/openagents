#!/usr/bin/env python3
"""Inventory a Psionic deployment, including transitive Linux shared libraries.

Pass lazy-loaded accelerator libraries explicitly. The JSON separates system
prerequisites and reports both application bytes and the entire listed closure.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument("--name",required=True)
parser.add_argument("--binary",type=Path,required=True)
parser.add_argument("--library",type=Path,action="append",default=[])
parser.add_argument("--system-library",type=Path,action="append",default=[])
parser.add_argument("--asset",type=Path,action="append",default=[])
args=parser.parse_args()
files={}
system_names=("libc.so.","libm.so.","libdl.so.","librt.so.","libpthread.so.","libgcc_s.so.","libstdc++.so.","ld-linux-","libcuda.so.")

def add(path,role):
    path=path.resolve(strict=True)
    if path not in files or files[path]=="system prerequisite":files[path]=role

for path in [args.binary]+args.library:add(path,"application")
for path in args.system_library:add(path,"system prerequisite")
for path in args.asset:add(path,"model asset or license")
missing=[]
for path in [args.binary]+args.library+args.system_library:
    output=subprocess.run(["ldd",str(path.resolve())],capture_output=True,text=True,check=True).stdout
    for line in output.splitlines():
        if "not found" in line:missing.append(line.strip());continue
        match=re.search(r"(?:=>\s*)?(/\S+)\s+\(",line)
        if match:
            dependency=Path(match[1])
            add(dependency,"system prerequisite" if dependency.name.startswith(system_names) else "application")
if missing:raise RuntimeError("Missing dependencies: "+"; ".join(missing))
records=[]
for path,role in sorted(files.items()):
    h=hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda:stream.read(1024*1024),b""):h.update(block)
    records.append({"file":str(path),"name":path.name,"role":role,"bytes":path.stat().st_size,"sha256":h.hexdigest()})
json.dump({"schema":"psionic.package.size.v1","name":args.name,"files":records,
    "application_bytes":sum(r["bytes"] for r in records if r["role"]=="application"),
    "model_asset_bytes":sum(r["bytes"] for r in records if r["role"]=="model asset or license"),
    "system_prerequisite_bytes":sum(r["bytes"] for r in records if r["role"]=="system prerequisite"),
    "total_listed_bytes":sum(r["bytes"] for r in records),"command":sys.argv,
    "notes":["Uncompressed files; each resolved file counted once. No Python environment is part of the runtime.","CUDA driver and ordinary OS/C++ libraries are listed as system prerequisites; provision them separately or include them when packaging.","Pass every lazy-loaded library and required license explicitly; ldd cannot discover dlopen dependencies or license requirements."]},sys.stdout,indent=2)
print()
