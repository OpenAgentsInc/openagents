#!/usr/bin/env python3
"""Compare native CUDA/Metal with ORT CUDA/Core ML using shared fixtures."""
import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--binary",type=Path,required=True)
parser.add_argument("--models",type=Path,required=True)
parser.add_argument("--ort-library",type=Path,required=True)
parser.add_argument("--ort-sha256",required=True)
parser.add_argument("--platform",choices=["cuda","apple"],required=True)
parser.add_argument("--output",type=Path,required=True)
parser.add_argument("--profiles",type=Path,required=True)
parser.add_argument("--cpus")
parser.add_argument("--cudnn-library",type=Path,help="Explicit cuDNN 9.20.0 library for the pinned CUDA baseline")
parser.add_argument("--cudnn-sha256")
parser.add_argument("--samples",type=int,default=1000)
parser.add_argument("--rounds",type=int,default=2)
args=parser.parse_args()
if args.samples<1000 or args.rounds<2:parser.error("Use at least 1,000 samples and two rounds")
cuda_stack=None
if args.platform=="cuda":
 if args.cudnn_library is None or args.cudnn_sha256 is None:parser.error("CUDA comparison requires --cudnn-library and --cudnn-sha256")
 library=args.cudnn_library.resolve(strict=True)
 if hashlib.sha256(library.read_bytes()).hexdigest()!=args.cudnn_sha256:parser.error("cuDNN checksum mismatch")
 cudnn=ctypes.CDLL(str(library));cudnn.cudnnGetVersion.restype=ctypes.c_size_t
 version=cudnn.cudnnGetVersion()
 if version!=92000:parser.error("This CUDA reference comparison pins cuDNN 9.20.0; use the binary directly for other explicitly qualified stacks")
 driver=subprocess.check_output(["nvidia-smi","--query-gpu=driver_version","--format=csv,noheader"],text=True).splitlines()[0]
 if tuple(map(int,driver.split('.')))<(580,65,6):parser.error("cuDNN 9.20.0 with CUDA 13 requires NVIDIA driver 580.65.06 or newer")
 cuda_stack={"cudnn_version":version,"cudnn_library":str(library),"cudnn_sha256":args.cudnn_sha256,"driver":driver,"minimum_driver":"580.65.06"}
args.output.parent.mkdir(parents=True,exist_ok=True)
args.profiles.mkdir(parents=True,exist_ok=True)
env=os.environ.copy()
env["NVIDIA_TF32_OVERRIDE"]="0"
env["CUDA_MODULE_LOADING"]="EAGER"
env["CUDA_MODULE_DATA_LOADING"]="EAGER"
if cuda_stack:env["LD_LIBRARY_PATH"]=str(args.cudnn_library.resolve().parent)+":"+env.get("LD_LIBRARY_PATH","")
with args.output.open("x") as output:
 for family in ["mnist","router"]:
  for repetition in range(args.rounds):
   for engine in (["ort","native"] if repetition%2==0 else ["native","ort"]):
    device="cuda" if args.platform=="cuda" else ("metal" if engine=="native" else "coreml")
    command=[str(args.binary.resolve()),"--family",family,"--engine",engine,"--device",device,
       "--model-dir",str((args.models/("s1-router" if family=="router" else family)).resolve()),
       "--threads","4","--samples",str(args.samples),"--ort-library",str(args.ort_library.resolve()),
       "--ort-sha256",args.ort_sha256]
    if engine=="ort":command += ["--allow-cpu-fallback","--profile",str((args.profiles/f"{family}-{repetition}").resolve())]
    if args.cpus:command=["taskset","-c",args.cpus]+command
    row=json.loads(subprocess.check_output(command,env=env,text=True))
    row.update(repetition=repetition,cpu_affinity=args.cpus,driver_command=command,other_load=os.getloadavg(),NVIDIA_TF32_OVERRIDE="0",CUDA_MODULE_LOADING="EAGER",CUDA_MODULE_DATA_LOADING="EAGER",cuda_stack=cuda_stack)
    if args.platform=="cuda":row["gpu"]=subprocess.check_output(["nvidia-smi","--query-gpu=name,uuid,driver_version,memory.total,memory.used","--format=csv,noheader"],text=True).strip()
    else:row["apple_hardware"]=subprocess.check_output(["system_profiler","SPHardwareDataType","SPDisplaysDataType"],text=True)
    output.write(json.dumps(row,separators=(",",":"))+"\n");output.flush()
    print(f"{family}/{engine}/{device}: p50 {row['p50_ms']:.4f} ms; p99 {row['p99_ms']:.4f} ms",flush=True)
