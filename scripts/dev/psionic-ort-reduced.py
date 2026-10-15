#!/usr/bin/env python3
"""Prepare and build an explicit, model-restricted ORT CPU deployment package.

Requires the pinned reference environment and an external ORT source checkout.
This tooling can download build dependencies; model inference stays offline.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

ORT_REVISION="da9b5e364c465de65c49d91e696cd6485270757f"  # v1.28.0
REPO=Path(__file__).resolve().parents[2]
FIXTURES=REPO/"crates/psionic/crates/psionic-inference/fixtures"

def artifact(path):
    h=hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda:stream.read(1024*1024),b""):h.update(chunk)
    return {"sha256":h.hexdigest(),"bytes":path.stat().st_size}

def prepare(args):
    import onnxruntime
    if onnxruntime.__version__!="1.28.0":raise ValueError("Use ONNX Runtime 1.28.0 for conversion")
    sources=args.output/"source-models";sources.mkdir(parents=True,exist_ok=True)
    pins={}
    for family,directory,filename in [("router","s1-router","model.gq.onnx"),("potion","potion","onnx/model.onnx"),("mnist","mnist","model.onnx")]:
        spec=json.loads((FIXTURES/("s1-router-manifest.json" if family=="router" else f"{family}-manifest.json")).read_text())
        source=(args.models/directory/filename).resolve()
        pin=artifact(source)
        if pin["sha256"]!=spec["files"][filename]["sha256"]:raise ValueError(f"Source checksum mismatch: {family}")
        pins[family]=pin
        link=sources/f"{family}.onnx"
        if link.is_symlink():link.unlink()
        link.symlink_to(source)
    subprocess.run([sys.executable,"-m","onnxruntime.tools.convert_onnx_models_to_ort",str(sources),"--output_dir",str(args.output/"optimized"),"--optimization_style","Fixed","--enable_type_reduction"],check=True)
    config=args.output/"optimized/required_operators_and_types.config"
    config.write_text("# Psionic router, Potion, and MNIST; ORT 1.28.0; fixed CPU optimization.\n"+"\n".join(s for s in config.read_text().splitlines() if not s.startswith("#"))+"\n")
    receipt={"schema":"psionic.ort-reduced.package.v1","ort_source_revision":ORT_REVISION,"conversion_runtime":"1.28.0","source_models":pins,
        "coverage":"Fixed CPU ORT-format artifacts only; no ONNX parser, CUDA, Core ML, or arbitrary model coverage.","files":{p.name:artifact(p) for p in sorted((args.output/"optimized").iterdir()) if p.is_file()}}
    (args.output/"conversion.json").write_text(json.dumps(receipt,indent=2)+"\n")

def build(args):
    revision=subprocess.check_output(["git","-C",str(args.source),"rev-parse","HEAD"],text=True).strip()
    if revision!=ORT_REVISION:raise ValueError("ORT source checkout must be pinned to v1.28.0")
    command=[sys.executable,str(args.source/"tools/ci_build/build.py"),"--config","MinSizeRel","--build_dir",str(args.build),"--build_shared_lib","--minimal_build","--include_ops_by_config",str(args.output/"optimized/required_operators_and_types.config"),"--enable_reduced_operator_type_support","--disable_ml_ops","--disable_rtti","--skip_tests","--parallel",str(args.jobs),"--cmake_generator","Ninja","--skip_submodule_sync","--compile_no_warning_as_error"]
    subprocess.run(command,cwd=args.source,check=True)
    (args.output/"build-command.json").write_text(json.dumps(command,indent=2)+"\n")

if __name__=="__main__":
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action",choices=["prepare","build"])
    parser.add_argument("--models",type=Path)
    parser.add_argument("--output",type=Path,required=True)
    parser.add_argument("--source",type=Path)
    parser.add_argument("--build",type=Path)
    parser.add_argument("--jobs",type=int,default=8)
    args=parser.parse_args()
    args.output=args.output.resolve();args.output.mkdir(parents=True,exist_ok=True)
    if args.action=="prepare":
        if args.models is None:parser.error("prepare requires --models")
        prepare(args)
    else:
        if args.source is None or args.build is None or args.jobs<1:parser.error("build requires --source, --build, and a positive job count")
        args.source=args.source.resolve();args.build=args.build.resolve();build(args)
