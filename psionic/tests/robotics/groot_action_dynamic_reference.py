#!/usr/bin/env python3
"""Firm reason: 2. Authenticate/replay a prior original-GPU capsule; no model run."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import struct
import numpy as np

REVISION = "2ea293aa20ba7cf5bbf3ba17a5fbcb1a01cbfe21"
ATOL = 0.002
RTOL = 0.002

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("capsule", type=Path)
    p.add_argument("work", type=Path)
    p.add_argument("--prepare", action="store_true")
    p.add_argument("--weights", type=Path)
    p.add_argument("--backbone", type=Path)
    args = p.parse_args()
    root = Path(__file__).resolve().parents[3]
    reference = json.loads((args.capsule / "record.json").read_text())
    assert reference["schema"] == "psionic-groot-prepared-vla-reference-v2"
    assert reference["model_revision"] == REVISION and reference["category"] == 2
    tokens = reference["request_tokens"]
    assert 137 <= tokens <= 512
    assert reference["dtype"] == "float32" and reference["device"] == "mps"
    assert not reference["cpu_model_fallback"] and not reference["source_checkout_executed"]
    names = ["vl-conditioning", "state-features"]
    names += [f"flow{step}-{suffix}" for step in range(4) for suffix in ("block0", "block1", "block2", "block3", "block31", "velocity", "actions")]
    names += ["normalized-actions"]
    inventory = {entry["name"]: entry for entry in reference["outputs"]}
    for name in names + ["backbone"]:
        path = args.capsule / f"{name}.f32"
        assert path.stat().st_size == inventory[name]["bytes"] and digest(path) == inventory[name]["sha256"]
    if args.prepare:
        assert args.weights is not None
        fixture = root / "psionic/fixtures/robotics/groot-libero"
        tensors = json.loads((fixture / "tensor-inventory.json").read_text())
        for shard in tensors["shards"]:
            path = args.weights / shard["file"]
            assert path.stat().st_size == shard["file_bytes"]
            with path.open("rb") as stream:
                size = struct.unpack("<Q", stream.read(8))[0]
                assert size == shard["header_bytes"]
                header = stream.read(size)
                assert hashlib.sha256(header).hexdigest() == shard["header_sha256"]
        args.work.mkdir(parents=True, exist_ok=True)
        with np.load(args.capsule / "inputs.npz", allow_pickle=False) as source:
            assert source["state"].shape == (1, 1, 132) and source["noise"].shape == (1, 40, 132) and source["ids"].shape == (tokens,)
            for key in ("state", "noise"):
                value = source[key].astype("<f4")
                assert np.isfinite(value).all()
                (args.work / f"{key}.f32").write_bytes(value.tobytes())
            (args.work / "ids.i64").write_bytes(source["ids"].astype("<i8").tobytes())
        backbone = args.backbone if args.backbone is not None else args.capsule / "backbone.f32"
        value = np.fromfile(backbone, dtype="<f4")
        assert value.shape == (tokens * 2048,) and np.isfinite(value).all()
        shutil.copyfile(backbone, args.work / "backbone.f32")
        print("GR00T action replay inputs and both original shard headers authenticated; whole-shard hashes reuse prior cache verification")
        return
    observed_root = args.work / "output"
    results = []
    total = 0
    for name in names:
        expected = np.fromfile(args.capsule / f"{name}.f32", dtype="<f4")
        path = observed_root / f"{name}.f32"
        observed = np.fromfile(path, dtype="<f4")
        assert expected.shape == observed.shape and np.isfinite(observed).all()
        error = np.abs(expected.astype(np.float64) - observed.astype(np.float64))
        budget = ATOL + RTOL * np.abs(expected.astype(np.float64))
        passed = bool(np.all(error <= budget))
        results.append({"name": name, "shape": inventory[name]["shape"], "values": len(observed), "max_abs_error": float(error.max(initial=0)), "max_budget_fraction": float((error / budget).max(initial=0)), "passed": passed, "coax_sha256": digest(path), "reference_sha256": inventory[name]["sha256"]})
        total += len(observed)
    assert (observed_root / "normalized-actions.f32").read_bytes() == (observed_root / "published-actions.f32").read_bytes()
    device = (args.work / "device.txt").read_text().splitlines()
    record = {"schema": "psionic-groot-dynamic-action-metal-reference-v1", "passed": all(r["passed"] for r in results), "model_revision": REVISION, "category": 2, "profile": "Original BF16 weights decoded exactly; F32 storage/products/affine stages, wide F64 normalization statistics (Metal float-float); source-default BF16 identity separate", "tolerance_frozen_before_observation": {"atol": ATOL, "rtol": RTOL}, "source_reference": {"record_sha256": digest(args.capsule / "record.json"), "inputs_npz_sha256": digest(args.capsule / "inputs.npz"), "torch": reference["torch"], "transformers": reference["transformers"], "diffusers": reference["diffusers"], "cpu_fallback": False}, "device": {"name": device[0], "apple_family": int(device[1]), "stage_compute_seconds": float(device[2])}, "input_backbone_sha256": digest(args.work / "backbone.f32"), "input_backbone_matches_sdk_capsule": digest(args.work / "backbone.f32") == inventory["backbone"]["sha256"], "request_tokens": tokens, "values_compared": total, "captures": results, "scope": "Variable-length learned original category2 action head; input_backbone_matches_sdk_capsule states whether its input is the original SDK output or an actual Coax output. Global trace diagnostics and public endpoint comparisons are retained separately; no simulator success or physical control", "whole_shard_hash_rechecked_here": False}
    paths = ["psionic/lib/robotics/groot-action-dynamic-plan.coax", "psionic/lib/robotics/groot-action-metal-plan.coax", "psionic/lib/robotics/groot-action-metal-types.coax", "psionic/lib/robotics/groot-action-metal-kernels.coax", "psionic/lib/robotics/groot-action-wide-kernels.coax", "psionic/lib/robotics/groot-action-metadata-fixtures.coax", "psionic/lib/robotics/groot-device-kernels.coax", "psionic/lib/device/tile.coax", "psionic/tests/robotics/groot_action_dynamic_metal.c", "psionic/tests/robotics/groot_action_dynamic_reference.py", "psionic/tests/robotics/groot_gpu_dispatch.h"]
    record["source_sha256"] = {name: digest(root / name) for name in paths}
    with (args.work / "reference.json").open("x") as stream:
        json.dump(record, stream, indent=2, sort_keys=True)
        stream.write("\n")
    for result in results:
        print(result["name"], "max_abs", result["max_abs_error"], "budget_fraction", result["max_budget_fraction"], "passed", result["passed"])
    assert record["passed"], "Frozen integration tolerance failed; diagnose without widening"
    print(f"GR00T full action head Metal: {len(results)} original captures, {total} values passed; no CPU model execution")

if __name__ == "__main__":
    main()
