#!/usr/bin/env python3
"""Original GR00T selected state encoder: required Metal output vs independent reference.
Tooling only. Reuses the independently authored original-BF16 reference, never
Coax packed data or any CPU fallback result. Tolerances stay frozen there.
"""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import platform
import sys

spec = importlib.util.spec_from_file_location("groot_original_reference", Path(__file__).with_name("groot_original_reference.py"))
reference = importlib.util.module_from_spec(spec)
spec.loader.exec_module(reference)
np = reference.np


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("weights", type=Path)
    parser.add_argument("--authenticate-only", action="store_true")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--device", type=Path)
    parser.add_argument("--record", type=Path)
    args = parser.parse_args()
    arrays, parts = reference.load_parts(args.weights)
    if args.authenticate_only:
        print("GR00T Metal preflight: all 4 original BF16 parts authenticated")
        return 0
    reference.require(args.output and args.device and args.record, "output/device/record required")
    reference.require(not args.record.exists(), "record already exists")
    raw = args.output.read_bytes()
    reference.require(len(raw) == 20480, "GPU output size")
    observed = np.frombuffer(raw, dtype="<f4").reshape(2, 2560)
    reference.require(np.isfinite(observed).all(), "GPU output nonfinite")
    expected = reference.expected_outputs(arrays)
    results = []
    for index, (hidden, final) in enumerate(expected):
        results.append({"name": reference.CASE_NAMES[index], "captured_robot_observation": False,
                        "relu_hidden": reference.metric(observed[index, :1024], hidden),
                        "state_features": reference.metric(observed[index, 1024:], final)})
    passed = all(case[layer]["passed"] for case in results for layer in ("relu_hidden", "state_features"))
    device = args.device.read_text().splitlines()
    reference.require(len(device) == 4, "device record shape")
    fp = np.finfo(np.longdouble)
    repo = Path(__file__).resolve().parents[3]
    sources = ["psionic/lib/robotics/groot-metal-plan.coax", "psionic/lib/robotics/groot-metal-types.coax",
               "psionic/lib/robotics/groot-metal-kernels.coax", "psionic/lib/device/tile.coax",
               "psionic/tests/robotics/groot_metal.c", "psionic/tests/robotics/groot_metal_reference.py",
               "psionic/tests/robotics/groot_original_reference.py", "psionic/platform/metal-host.m"]
    record = {"schema": "psionic-groot-original-metal-state-reference-v1", "passed": passed,
              "model": "nvidia/GR00T-N1.7-LIBERO", "revision": reference.REVISION,
              "subfolder": "libero_10", "category": 2, "whole_shard_hash_verified": False,
              "scope": "Original category-selected state encoder only; no vision/language/action flow or simulated task success.",
              "execution_profile": "Metal4 BF16 weights, F32 inputs/matrix-unit accumulation, separate F32 bias and ReLU",
              "cpu_projection_fallback": False, "values_compared": 5120,
              "reference": {"implementation": "independently authored NumPy longdouble reduction with F32 boundaries",
                            "interpreter": sys.executable, "python": platform.python_version(), "numpy": np.__version__,
                            "machine": platform.machine(), "longdouble_bits": int(fp.bits), "longdouble_fraction_bits": int(fp.nmant)},
              "tolerance_frozen_before_comparison": {"atol": reference.ATOL, "rtol": reference.RTOL},
              "device": {"name": device[0], "apple_family": int(device[1]),
                         "gpu_four_launch_seconds_per_case": [float(x) for x in device[2:]],
                         "timing_scope": "One four-launch command batch per case; excludes compile, upload, publication and reference."},
              "source_parts": parts, "output_sha256": hashlib.sha256(raw).hexdigest(), "results": results,
              "source_sha256": {name: hashlib.sha256((repo / name).read_bytes()).hexdigest() for name in sources}}
    with args.record.open("x") as stream:
        json.dump(record, stream, indent=2, sort_keys=True)
        stream.write("\n")
    reference.require(passed, "GPU comparison failed; inspect record; do not widen tolerances")
    print(f"GR00T original Metal: 5120 values passed frozen atol={reference.ATOL}, rtol={reference.RTOL}; device={device[0]}")
    for case in results:
        print(case["name"], "hidden max_abs", case["relu_hidden"]["max_abs_error"],
              "final max_abs", case["state_features"]["max_abs_error"])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
