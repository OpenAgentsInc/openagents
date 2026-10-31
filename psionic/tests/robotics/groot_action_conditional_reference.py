#!/usr/bin/env python3
"""Firm reason2: same-input original-head MPS oracle; never a Psionic runtime.

This isolates the head on an actual Coax backbone. It neither replaces the
independent full-network capsule nor changes its failed hidden-trace metric.
"""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import time
import numpy as np
from safetensors import safe_open
import torch
import diffusers

ATOL = 0.002
RTOL = 0.002

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("weights", type=Path)
    p.add_argument("original_capsule", type=Path)
    p.add_argument("coax_backbone", type=Path)
    p.add_argument("coax_head_output", type=Path)
    p.add_argument("new_reference", type=Path)
    args = p.parse_args()
    assert torch.__version__.split("+")[0] == "2.9.0" and diffusers.__version__ == "0.35.1"
    assert torch.backends.mps.is_available()
    assert os.environ.get("PYTORCH_ENABLE_MPS_FALLBACK", "0") == "0"
    assert os.environ.get("PYTORCH_MPS_FAST_MATH", "0") == "0"
    source = Path(__file__).with_name("groot_vla_reference.py")
    spec = importlib.util.spec_from_file_location("independent_groot_vla_reference", source)
    reference = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(reference)
    original = json.loads((args.original_capsule / "record.json").read_text())
    assert original["model_revision"] == reference.REVISION and original["category"] == 2
    assert original["dtype"] == "float32" and original["device"] == "mps" and not original["cpu_model_fallback"]
    backbone = np.fromfile(args.coax_backbone, dtype="<f4")
    assert backbone.shape == (140 * 2048,) and np.isfinite(backbone).all()
    with np.load(args.original_capsule / "inputs.npz", allow_pickle=False) as data:
        state, noise, ids = [data[key].copy() for key in ("state", "noise", "ids")]
    assert state.shape == (1, 1, 132) and noise.shape == (1, 40, 132) and ids.shape == (140,)
    capture = reference.Capture(args.new_reference)
    weights = {}
    started = time.monotonic()
    for shard, size in reference.SIZES.items():
        path = args.weights / shard
        assert path.stat().st_size == size
        with safe_open(path, framework="pt", device="cpu") as tensors:
            for name in tensors.keys():
                if not name.startswith("action_head."):
                    continue
                if name.endswith((".W", ".b")):
                    assert tensors.get_slice(name).get_shape()[0] == 32
                    value = tensors.get_slice(name)[2]
                else:
                    value = tensors.get_tensor(name)
                weights[name.removeprefix("action_head.")] = value.to(device="mps", dtype=torch.float32)
    assert len(weights) == 537 and all(value.device.type == "mps" for value in weights.values())
    loaded = time.monotonic()
    print("same-input conditional head: all537 original tensors loaded onMPS", flush=True)
    with torch.inference_mode():
        output = reference.ActionHead(weights, capture)(
            torch.from_numpy(backbone.reshape(1, 140, 2048)).to("mps"),
            torch.from_numpy(state).to("mps"), torch.from_numpy(noise).to("mps"),
            torch.from_numpy(ids[None]).to("mps"))
        capture("normalized-actions", output)
    torch.mps.synchronize()
    finished = time.monotonic()
    results = []
    total = 0
    for item in capture.records:
        name = item["name"]
        expected = np.fromfile(args.new_reference / f"{name}.f32", dtype="<f4")
        observed_path = args.coax_head_output / f"{name}.f32"
        observed = np.fromfile(observed_path, dtype="<f4")
        assert observed.shape == expected.shape and np.isfinite(observed).all()
        error = np.abs(observed.astype(np.float64) - expected.astype(np.float64))
        budget = ATOL + RTOL * np.abs(expected.astype(np.float64))
        result = {"name": name, "shape": item["shape"], "values": len(expected), "max_abs_error": float(error.max(initial=0)), "max_budget_fraction": float((error / budget).max(initial=0)), "passed": bool(np.all(error <= budget)), "conditional_reference_sha256": item["sha256"], "coax_sha256": digest(observed_path)}
        results.append(result)
        total += len(expected)
        print(name, "max_abs", result["max_abs_error"], "budget_fraction", result["max_budget_fraction"], "passed", result["passed"], flush=True)
    assert len(results) == 31
    record = {"schema": "psionic-groot-head-conditioned-on-coax-backbone-v1", "passed": all(r["passed"] for r in results), "conditioned_on_coax_backbone": True, "input_backbone_sha256": digest(args.coax_backbone), "backbone_sha256": digest(args.coax_backbone), "model_revision": reference.REVISION, "category": 2, "torch": torch.__version__, "diffusers": diffusers.__version__, "reference_device": "mps", "reference_dtype": "float32", "cpu_model_fallback": False, "source_checkout_executed": False, "original_full_network_reference_replaced": False, "global_hidden_trace_failure_relabelled": False, "tolerance_frozen_before_observation": {"atol": ATOL, "rtol": RTOL}, "values_compared": total, "load_seconds": loaded - started, "execution_and_capture_seconds": finished - loaded, "captures": results, "outputs": capture.records, "source_sha256": {source.name: digest(source), Path(__file__).name: digest(Path(__file__))}, "original_capsule_record_sha256": digest(args.original_capsule / "record.json"), "original_state_noise_capsule_sha256": digest(args.original_capsule / "inputs.npz"), "scope": "Same-input component fidelity of the original learned head on an actual Coax backbone; independent full-network final outputs and global hidden-trace diagnostics remain separate."}
    (args.new_reference / "record.json").write_text(json.dumps(record, indent=2, sort_keys=True) + "\n")
    assert record["passed"], "Conditional frozen head budget failed; do not widen it"
    print(f"same-input original head: all31captures/{total}values passed; fullnetwork hidden-trace diagnostic unchanged", flush=True)

if __name__ == "__main__":
    main()
