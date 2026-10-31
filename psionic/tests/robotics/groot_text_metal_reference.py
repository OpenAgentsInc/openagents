#!/usr/bin/env python3
"""Firm reason2: replay original GPU reference; no model inference here."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import numpy as np

ATOL,RTOL=.002,.002  # Frozen before first Coax text-device comparison.
p=argparse.ArgumentParser(description=__doc__)
p.add_argument("capsule",type=Path)
p.add_argument("output",type=Path)
p.add_argument("--prepare",action="store_true")
p.add_argument("--vision",type=Path)
a=p.parse_args()
if a.prepare:
    a.output.mkdir(parents=True,exist_ok=False)
    inputs=np.load(a.capsule/"inputs.npz")
    (a.output/"ids.i64").write_bytes(inputs["ids"].astype("<i8").tobytes())
    vision=a.vision or a.capsule
    for name in ["vision-merged","vision-deepstack0","vision-deepstack1","vision-deepstack2"]:
        shutil.copyfile(vision/(name+".f32"),a.output/(name+".f32"))
    print("private original text inputs prepared; no model execution")
else:
    actual=np.frombuffer((a.output/"backbone.f32").read_bytes(),dtype="<f4")
    expected=np.frombuffer((a.capsule/"backbone.f32").read_bytes(),dtype="<f4")
    assert actual.shape==expected.shape and np.isfinite(actual).all()
    difference=np.abs(actual.astype(np.float64)-expected.astype(np.float64))
    budget=ATOL+RTOL*np.abs(expected.astype(np.float64))
    device=(a.output/"device.txt").read_text().splitlines()
    record={"schema":"psionic-groot-text-metal-reference-v1","passed":bool(np.all(difference<=budget)),
            "device":device[0],"apple_family":int(device[1]),"command_gpu_seconds":float(device[2]),
            "launches":int(device[3]),"values":int(actual.size),"atol":ATOL,"rtol":RTOL,
            "max_abs_error":float(difference.max()),"max_tolerance_fraction":float((difference/budget).max()),
            "actual_sha256":hashlib.sha256((a.output/"backbone.f32").read_bytes()).hexdigest(),
            "reference_sha256":hashlib.sha256((a.capsule/"backbone.f32").read_bytes()).hexdigest(),
            "scope":"Complete original16-layer Cosmos text prefix, precomputed camera/deepstack inputs; no action decoder or simulated task claim."}
    assert not (a.output/"reference.json").exists()
    (a.output/"reference.json").write_text(json.dumps(record,indent=2,sort_keys=True)+"\n")
    print(json.dumps(record,sort_keys=True))
    assert record["passed"],"text prefix comparison failed; keep frozen tolerance"
