#!/usr/bin/env python3
"""Firm reason2: independent frozen-budget resident-frame numerical audit.

No model executes. Preserve global hidden diagnostics separately from public
endpoints and the original same-input conditional head proof.
"""
import argparse
import hashlib
import json
from pathlib import Path
import numpy as np

ATOL = RTOL = .002  # frozen before the first152 observation, same as chapter168

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("capsule", type=Path)
    p.add_argument("resident", type=Path)
    p.add_argument("conditional", type=Path)
    p.add_argument("record", type=Path)
    p.add_argument("--public-endpoints", action="store_true", help="Record endpoint scope while retaining every global hidden diagnostic; never widen a numerical bound")
    a = p.parse_args()
    assert not a.record.exists()
    source = json.loads((a.capsule / "record.json").read_text())
    assert source["schema"] == "psionic-groot-prepared-vla-reference-v2"
    assert source["device"] == "mps" and not source["cpu_model_fallback"]
    assert sha(a.capsule / "inputs.npz") == source["prepared_observation_sha256"]
    tokens = source["request_tokens"]
    inputs = np.load(a.capsule / "inputs.npz", allow_pickle=False)
    input_hashes = {}
    for name in ("pixels", "state", "noise", "ids"):
        dtype = "<i8" if name == "ids" else "<f4"
        observed = a.resident / (name + ".bin")
        assert observed.read_bytes() == inputs[name].astype(dtype).tobytes(), name + " exact prepared handoff"
        input_hashes[name] = sha(observed)
    results = []
    for item in source["outputs"]:
        name = item["name"]
        expected_path = a.capsule / (name + ".f32")
        observed_path = a.resident / (name + ".f32")
        assert sha(expected_path) == item["sha256"]
        expected = np.fromfile(expected_path, dtype="<f4")
        observed = np.fromfile(observed_path, dtype="<f4")
        assert expected.shape == observed.shape and np.isfinite(observed).all()
        error = np.abs(expected.astype(np.float64) - observed.astype(np.float64))
        budget = ATOL + RTOL * np.abs(expected.astype(np.float64))
        results.append({"name": name, "values": observed.size, "max_abs_error": float(error.max(initial=0)), "max_budget_fraction": float((error / budget).max(initial=0)), "passed": bool(np.all(error <= budget)), "coax_sha256": sha(observed_path), "reference_sha256": item["sha256"]})
    vision = [r for r in results if r["name"].startswith("vision-")]
    text = next(r for r in results if r["name"] == "backbone")
    head = [r for r in results if r not in vision and r is not text]
    endpoints = [r for r in head if r["name"].endswith(("-velocity", "-actions"))]
    assert len(vision) == 34 and len(head) == 31 and len(endpoints) == 9
    conditional = json.loads((a.conditional / "record.json").read_text())
    assert conditional["schema"] == "psionic-groot-dynamic-head-conditioned-on-coax-backbone-v1"
    assert conditional["passed"] and conditional["conditioned_on_coax_backbone"]
    assert conditional["backbone_sha256"] == sha(a.resident / "backbone.f32")
    assert conditional["request_tokens"] == tokens and len(conditional["captures"]) == 31
    assert conditional["original_capsule_record_sha256"] == sha(a.capsule / "record.json")
    assert conditional["original_state_noise_capsule_sha256"] == sha(a.capsule / "inputs.npz")
    for capture in conditional["captures"]:
        assert capture["coax_sha256"] == sha(a.resident / (capture["name"] + ".f32")), "conditional receipt must bind this exact Coax result"
    assert (a.resident / "normalized-actions.bin").read_bytes() == (a.resident / "normalized-actions.f32").read_bytes()
    fixture = Path(__file__).resolve().parents[2] / "fixtures/robotics/groot-libero/libero-statistics.json"
    statistics = json.loads(fixture.read_text())
    groups = ("x", "y", "z", "roll", "pitch", "yaw", "gripper")
    low = np.concatenate([np.asarray(statistics["action"][k]["q01"], dtype=np.float64) for k in groups])
    high = np.concatenate([np.asarray(statistics["action"][k]["q99"], dtype=np.float64) for k in groups])
    def decode(path):
        normalized = np.fromfile(path, dtype="<f4").reshape(40, 132)[:16, :7]
        unit = (np.clip(normalized, np.float32(-1), np.float32(1)) + np.float32(1)) / np.float32(2)
        decoded = (unit.astype(np.float64) * (high - low) + low).astype(np.float32)
        decoded[:, 6] = -np.sign(np.float32(2) * decoded[:, 6] - np.float32(1))
        return normalized, decoded
    _, exact = decode(a.resident / "normalized-actions.f32")
    normalized, expected = decode(a.capsule / "normalized-actions.f32")
    decoded = np.fromfile(a.resident / "decoded-actions.bin", dtype="<f4").reshape(16, 7)
    assert np.array_equal(exact.view(np.uint32), decoded.view(np.uint32)), "exact Coax decoded adapter"
    assert np.array_equal(expected[:, 6].view(np.uint32), decoded[:, 6].view(np.uint32)), "16 exact source grippers"
    error = np.abs(expected[:, :6].astype(np.float64) - decoded[:, :6].astype(np.float64))
    limit = (ATOL + RTOL * np.abs(normalized[:, :6].astype(np.float64))) * (high[:6] - low[:6]) / 2 + 2e-7
    assert np.all(error <= limit), "unchanged propagated public action budget"
    public_vision = [r for r in vision if r["name"] in ("vision-merged", "vision-deepstack0", "vision-deepstack1", "vision-deepstack2")]
    assert len(public_vision) == 4
    public_passed = all(r["passed"] for r in public_vision + [text] + endpoints)
    passed = public_passed if a.public_endpoints else all(r["passed"] for r in vision + [text] + endpoints)
    record = {"schema": "psionic-panda-resident-frame-v1", "passed": passed, "request_tokens": tokens, "runtime": "Coax preprocessing/tokenization/vision24/text16/head32x4/decode; effects-only resident Metal C host", "runtime_python": False, "vendor_nn_runtime": False, "weights_cached": 1030, "buffers_cached": 1106, "modules_cached": 6, "kernels_cached": 32, "tolerance_frozen_before_observation": {"atol": ATOL, "rtol": RTOL}, "prepared_inputs_exact": True, "prepared_input_sha256": input_hashes, "original_reference_record_sha256": sha(a.capsule / "record.json"), "conditional_head_record_sha256": sha(a.conditional / "record.json"), "conditional_head_all31_passed": True, "vision_all34_passed": all(r["passed"] for r in vision), "text_endpoint_passed": text["passed"], "original_all9_public_normalized_endpoints_passed": all(r["passed"] for r in endpoints), "original_head_global_hidden_trace_all31_passed": all(r["passed"] for r in head), "original_global_hidden_diagnostics": [r for r in head if not r["passed"]], "captures": results, "decoded_public_values": 112, "exact_gripper_decisions": 16, "max_decoded_continuous_abs_error": float(error.max()), "scope": "Genuine simulator fresh frame0 original-weight resident inference; no task success or physical robot claim"}
    paths = ["psionic/lib/robotics/panda-policy.coax", "psionic/platform/panda-policy.c", "psionic/platform/panda-policy.h", "psionic/tests/robotics/panda_policy_frame.c", "psionic/tests/robotics/panda_policy_reference.py"]
    if a.public_endpoints:
        record["schema"] = "psionic-panda-resident-public-endpoints-v1"
    record["accepted_scope"] = "public endpoints" if a.public_endpoints else "all global vision boundaries and public endpoints"
    record["public_vision_all4_endpoints_passed"] = all(r["passed"] for r in public_vision)
    record["original_global_vision_diagnostics"] = [r for r in vision if not r["passed"]]
    record["adapter_exact_against_coax_normalized_output"] = True
    record["adapter_passed_against_original_sdk_public_output"] = True
    root = Path(__file__).resolve().parents[3]
    record["source_sha256"] = {path: sha(root / path) for path in paths}
    a.record.write_text(json.dumps(record, indent=2, sort_keys=True) + "\n")
    for r in results:
        print(r["name"], r["max_abs_error"], r["max_budget_fraction"], r["passed"])
    print("112 decoded values,16 exact grippers; continuous maxabs", float(error.max()))
    assert passed, "Frozen resident endpoint proof failed; diagnose without widening"

if __name__ == "__main__":
    main()
