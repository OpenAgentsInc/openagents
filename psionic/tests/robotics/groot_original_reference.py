#!/usr/bin/env python3
"""Independent original-weight state-encoder reference; no model repo script.

Firm reason: 2 (independent numerical/artifact oracle). NumPy is an existing
reference dependency only. Neither this script nor NumPy is a shipped runtime.
"""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import sys

import numpy as np

# Frozen before any native/reference comparison, not chosen from observed error.
ATOL = 2e-5
RTOL = 1e-5
REVISION = "2ea293aa20ba7cf5bbf3ba17a5fbcb1a01cbfe21"
PARTS = {
    "state-w1": (270336, "2153820ab3384da5c97cde122b91771a00c82a5dc7704905d997f841ba3164e6", (132, 1024)),
    "state-b1": (2048, "434177fe7b3a856b5d3e81ea63562e814f3ad96b9c35cdc39c8e1e468d1317c7", (1024,)),
    "state-w2": (3145728, "e17d06407e544c362b4fff549df257a1fa108ae38b3bd604553b2d6e40c9f39d", (1024, 1536)),
    "state-b2": (3072, "adfcba5960705125ba54d79a1991b9fb9093a952141aed213d85cc23334e0b0d", (1536,)),
}
CASE_NAMES = ("authored-eight-values-and-padding", "numeric-ramp-all132")


def require(condition, text):
    if not condition:
        raise SystemExit("GR00T original reference: " + text)


def load_parts(root):
    arrays, records = {}, []
    for name, (size, expected, shape) in PARTS.items():
        raw = (root / (name + ".bf16")).read_bytes()
        digest = hashlib.sha256(raw).hexdigest()
        require(len(raw) == size and digest == expected, "selected tensor size/hash: " + name)
        # Decode source input-major BF16 directly; never use Coax's packed bytes.
        words = np.frombuffer(raw, dtype="<u2").astype(np.uint32) << np.uint32(16)
        values = words.view(np.float32).reshape(shape)
        require(np.isfinite(values).all(), "nonfinite source: " + name)
        arrays[name] = values
        records.append({"name": name, "bytes": size, "sha256": digest, "shape": list(shape)})
    return arrays, records


def affine(values, weight, bias):
    # Independent wide reduction, then the same two F32 storage boundaries.
    # On platforms where longdouble is binary64, report that fact explicitly.
    projection = (values.astype(np.longdouble) @ weight.astype(np.longdouble)).astype(np.float32)
    return (projection.astype(np.longdouble) + bias.astype(np.longdouble)).astype(np.float32)


def expected_outputs(arrays):
    first = np.zeros(132, dtype=np.float32)
    first[:8] = [-0.75, -0.5, -0.25, 0, 0.25, 0.5, 0.75, 1]
    second = np.array([(i % 17 - 8) / 16 for i in range(132)], dtype=np.float32)
    results = []
    for values in (first, second):
        hidden = np.maximum(affine(values, arrays["state-w1"], arrays["state-b1"]), np.float32(0))
        output = affine(hidden, arrays["state-w2"], arrays["state-b2"])
        require(np.isfinite(hidden).all() and np.isfinite(output).all(), "nonfinite reference")
        results.append((hidden, output))
    return results


def metric(observed, expected):
    difference = np.abs(observed.astype(np.float64) - expected.astype(np.float64))
    tolerance = ATOL + RTOL * np.abs(expected.astype(np.float64))
    return {"values": int(expected.size), "passed": bool(np.all(difference <= tolerance)),
            "max_abs_error": float(difference.max(initial=0)),
            "max_relative_error": float((difference / np.maximum(np.abs(expected.astype(np.float64)), 1e-30)).max(initial=0)),
            "max_fraction_of_tolerance": float((difference / tolerance).max(initial=0))}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("weights", type=Path)
    parser.add_argument("outputs", nargs="+", type=Path)
    parser.add_argument("--record", required=True, type=Path)
    args = parser.parse_args()
    require(not args.record.exists(), "record already exists")
    arrays, parts = load_parts(args.weights)
    expected = expected_outputs(arrays)
    records, raw_outputs = [], []
    passed = True
    for path in args.outputs:
        raw = path.read_bytes()
        require(len(raw) == 20480, "native output size: " + path.name)
        observed = np.frombuffer(raw, dtype="<f4").reshape(2, 2560)
        require(np.isfinite(observed).all(), "nonfinite native output: " + path.name)
        cases = []
        for index, (hidden, output) in enumerate(expected):
            h = metric(observed[index, :1024], hidden)
            o = metric(observed[index, 1024:], output)
            passed = passed and h["passed"] and o["passed"]
            cases.append({"name": CASE_NAMES[index], "captured_robot_observation": False,
                          "relu_hidden": h, "state_features": o})
        raw_outputs.append(raw)
        records.append({"mode": path.stem, "sha256": hashlib.sha256(raw).hexdigest(), "cases": cases})
    identical = all(raw == raw_outputs[0] for raw in raw_outputs)
    passed = passed and identical
    fp = np.finfo(np.longdouble)
    record = {"schema": "psionic-groot-original-state-reference-v1", "passed": passed,
              "model": "nvidia/GR00T-N1.7-LIBERO", "revision": REVISION,
              "subfolder": "libero_10", "category": 2, "whole_shard_hash_verified": False,
              "scope": "Original category-selected state encoder only; no vision/language/action flow or simulated task success.",
              "execution_profile": "F32 storage, compensated F64 Coax projection, separate F32 bias, ReLU",
              "reference": {"implementation": "independently authored NumPy longdouble reduction with F32 boundaries",
                            "interpreter": sys.executable, "python": platform.python_version(), "numpy": np.__version__,
                            "machine": platform.machine(), "longdouble_bits": int(fp.bits), "longdouble_fraction_bits": int(fp.nmant)},
              "tolerance_frozen_before_comparison": {"atol": ATOL, "rtol": RTOL},
              "source_parts": parts, "native_modes_bit_identical": identical, "results": records}
    with args.record.open("x") as stream:
        json.dump(record, stream, indent=2, sort_keys=True)
        stream.write("\n")
    require(passed, "numerical comparison failed; inspect record")
    print(f"GR00T original reference: {len(records)} native modes, 5120 values/mode passed; NumPy {np.__version__}, longdouble {fp.bits} bits/{fp.nmant + 1} significant bits")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
