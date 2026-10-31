#!/usr/bin/env python3
"""Tooling: qualify original24-block Coax Metal vision against pinned SDK captures.
No model framework, inference fallback or downloaded project scripts execute here.
"""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import struct
import sys
import numpy as np

ATOL, RTOL = 2e-3, 2e-4  # Frozen before the first original vision comparison.
REVISION = "2ea293aa20ba7cf5bbf3ba17a5fbcb1a01cbfe21"
ENDPOINTS = {"vision-merged", "vision-deepstack0", "vision-deepstack1", "vision-deepstack2"}
REPO = Path(__file__).resolve().parents[3]
FIXTURE = REPO / "psionic/fixtures/robotics/groot-libero"


def require(ok, why):
    if not ok:
        raise RuntimeError(why)


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def prepare(shards, capsule, work):
    require(not (work / "preflight.json").exists(), "preflight already exists")
    inventory = json.loads((FIXTURE / "tensor-inventory.json").read_text())
    receipt_raw = (FIXTURE / "full-cache-verification.json").read_bytes()
    receipt = json.loads(receipt_raw)
    require(inventory["revision"] == receipt["revision"] == REVISION, "source revision")
    original = {entry["file"]: entry for entry in receipt["full_shards"]}
    headers = []
    for part in inventory["shards"]:
        path = shards / part["file"]
        previous = original[part["file"]]
        require(previous["bytes"] == path.stat().st_size == part["file_bytes"], "full cached shard size")
        require(previous["sha256"] == part["advertised_sha256"], "prior authenticated full-shard identity")
        with path.open("rb") as stream:
            n = struct.unpack("<Q", stream.read(8))[0]
            require(n == part["header_bytes"] and n <= 1048576, "bounded original header")
            header = stream.read(n)
        require(len(header) == n and digest(header) == part["header_sha256"], "original header identity")
        headers.append({"file": part["file"], "bytes": path.stat().st_size,
                        "header_sha256": digest(header), "previously_verified_full_sha256": previous["sha256"]})
    reference_raw = (capsule / "record.json").read_bytes()
    reference = json.loads(reference_raw)
    require(reference["model_revision"] == REVISION and reference["dtype"] == "float32"
            and reference["device"] == "mps" and not reference["cpu_model_fallback"], "original SDK reference profile")
    npz_raw = (capsule / "inputs.npz").read_bytes()
    with np.load(capsule / "inputs.npz", allow_pickle=False) as inputs:
        pixels = inputs["pixels"]
        require(pixels.shape == (512, 1536) and pixels.dtype == np.float32 and np.isfinite(pixels).all(), "packed two-camera pixel tensor")
        raw = pixels.astype("<f4", copy=False).tobytes()
    with (work / "pixels.f32").open("xb") as stream:
        stream.write(raw)
    record = {"schema": "psionic-groot-original-vision-preflight-v1", "revision": REVISION,
              "full_hashes_recomputed": False,
              "full_hash_receipt": "Reuses earlier completed full-shard authentication; sizes and exact headers rechecked for this run.",
              "full_hash_receipt_sha256": digest(receipt_raw), "source_shards": headers,
              "reference_record_sha256": digest(reference_raw), "reference_inputs_npz_sha256": digest(npz_raw),
              "packed_pixels_sha256": digest(raw), "packed_pixels_shape": [512, 1536]}
    with (work / "preflight.json").open("x") as stream:
        json.dump(record, stream, indent=2, sort_keys=True)
        stream.write("\n")
    print("Original vision preflight: pinned headers/prior full-shard receipt and SDK two-camera input accepted")


def compare(capsule, work, executed_pixels):
    require(not (work / "record.json").exists(), "comparison record already exists")
    preflight = json.loads((work / "preflight.json").read_text())
    reference_raw = (capsule / "record.json").read_bytes()
    require(digest(reference_raw) == preflight["reference_record_sha256"], "reference record changed")
    require(digest(executed_pixels.read_bytes()) == preflight["packed_pixels_sha256"], "executed packed input differs")
    reference = json.loads(reference_raw)
    records = []
    for target in reference["outputs"]:
        name = target["name"]
        if not name.startswith("vision-"):
            continue
        raw_reference = (capsule / (name + ".f32")).read_bytes()
        raw = (work / (name + ".f32")).read_bytes()
        require(digest(raw_reference) == target["sha256"] and len(raw_reference) == target["bytes"], "reference capture identity: " + name)
        require(len(raw) == len(raw_reference), "Coax capture size: " + name)
        expected = np.frombuffer(raw_reference, dtype="<f4").astype(np.float64)
        observed = np.frombuffer(raw, dtype="<f4").astype(np.float64)
        require(np.isfinite(expected).all() and np.isfinite(observed).all(), "finite capture: " + name)
        error = np.abs(observed - expected)
        bound = ATOL + RTOL * np.abs(expected)
        record = {"name": name, "shape": target["shape"], "values": int(expected.size),
                  "endpoint": name in ENDPOINTS, "passed": bool(np.all(error <= bound)),
                  "max_abs_error": float(error.max(initial=0)),
                  "max_fraction_of_tolerance": float((error / bound).max(initial=0)),
                  "output_sha256": digest(raw), "reference_sha256": target["sha256"]}
        records.append(record)
    require(len(records) == 34 and {x["name"] for x in records if x["endpoint"]} == ENDPOINTS, "complete visual boundary set")
    device = (work / "device.txt").read_text().splitlines()
    require(len(device) == 3, "device receipt")
    all_passed = all(r["passed"] for r in records)
    endpoint_passed = all(r["passed"] for r in records if r["endpoint"])
    sources = ["psionic/lib/robotics/groot-vision-device.coax", "psionic/lib/robotics/groot-vision-metal-plan.coax",
               "psionic/lib/robotics/groot-vision-metal-types.coax", "psionic/lib/robotics/groot-vision-metal-check-fixtures.coax",
               "psionic/lib/robotics/groot-device-kernels.coax", "psionic/lib/robotics/groot-action-wide-kernels.coax", "psionic/lib/device/tile.coax",
               "psionic/tests/robotics/groot_gpu_dispatch.h", "psionic/tests/robotics/groot_vision_metal.c",
               "psionic/tests/robotics/groot_vision_metal_reference.py", "psionic/platform/metal-host.m"]
    record = {"schema": "psionic-groot-original-metal-vision-reference-v1", "passed": all_passed,
              "endpoint_outputs_passed": endpoint_passed, "all_diagnostic_boundaries_passed": all_passed,
              "model": "nvidia/GR00T-N1.7-LIBERO", "revision": REVISION, "subfolder": "libero_10", "category": 2,
              "scope": "Complete original vision24 plus ordinary and three deepstack mergers only. Language, action flow, physical robot control and task success are separate.",
              "input_scope": "Original SDK-packed F32 pixels for two owned patterned still cameras; no captured real-world observation or preprocessor qualification claim.",
              "execution_profile": "Original BF16 learned weights; F32 Metal4 matrix accumulation, wide normalization statistics/centering/division (Metal float-float f64 profile), F32 affines/attention/tanh-GELU, independently qualified erf-polynomial merger GELU.",
              "cpu_model_fallback": False, "learned_tensors": 315, "stages": 29, "micro_op_launches": 359,
              "values_compared": sum(r["values"] for r in records),
              "tolerance_frozen_before_comparison": {"atol": ATOL, "rtol": RTOL},
              "reference": {"implementation": "Pinned original Transformers4.57.3 Qwen3VLModel on MPS/F32, independently captured earlier",
                            "capsule_record_sha256": digest(reference_raw), "cpu_model_fallback": False},
              "comparison_runtime": {"python": platform.python_version(), "numpy": np.__version__, "interpreter": sys.executable},
              "device": {"name": device[0], "apple_family": int(device[1]), "gpu_all_stage_command_seconds": float(device[2]),
                         "timing_scope": "GPU command batches; excludes compile, original-weight load/decode/upload, packed input preparation, waits/flag scans on CPU, captures and comparison."},
              "preflight": preflight, "results": records,
              "source_sha256": {name: digest((REPO / name).read_bytes()) for name in sources}}
    with (work / "record.json").open("x") as stream:
        json.dump(record, stream, indent=2, sort_keys=True)
        stream.write("\n")
    require(all_passed, "some original vision boundaries failed frozen limits; inspect record and do not widen tolerance")
    print(f"Original Coax Metal vision: all {len(records)} boundaries/{record['values_compared']:,} values passed; GPU {device[2]}s")
    for r in records:
        if r["endpoint"]:
            print(r["name"], "maxabs", r["max_abs_error"], "budget fraction", r["max_fraction_of_tolerance"])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("prepare", "compare"))
    parser.add_argument("capsule", type=Path)
    parser.add_argument("work", type=Path)
    parser.add_argument("--shards", type=Path)
    parser.add_argument("--executed-pixels", type=Path)
    args = parser.parse_args()
    if args.mode == "prepare":
        require(args.shards is not None, "--shards required")
        prepare(args.shards, args.capsule, args.work)
    else:
        compare(args.capsule, args.work, args.executed_pixels or args.work / "pixels.f32")


if __name__ == "__main__":
    main()
