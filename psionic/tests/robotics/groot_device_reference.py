#!/usr/bin/env python3
"""Firm reason: 2. Independent Torch equations; tooling only, never shipped."""
import argparse
import hashlib
import json
from pathlib import Path
import numpy as np
import torch
import torch.nn.functional as F

# Frozen before observing either CPU or GPU results; do not widen on failure.
ATOL = 2e-5
RTOL = 2e-5

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    args = parser.parse_args()
    assert torch.__version__.split("+")[0] == "2.9.0"
    torch.set_num_threads(2)
    results = []
    count = 0
    for line in (args.directory / "cases.jsonl").read_text().splitlines():
        case = json.loads(line)
        prefix = args.directory / f"{case['index']:03d}"
        def read(tag, dtype="<f4"):
            return np.frombuffer(Path(str(prefix) + f"-{tag}.bin").read_bytes(), dtype=dtype).copy()
        def tensor(tag):
            return torch.from_numpy(read(tag))
        x = tensor("x")
        if case["op"] == "norm":
            w, kind = case["width"], case["kind"]
            x = x.reshape(case["rows"], w)
            a, b = tensor("a"), tensor("b")
            if kind == 2:
                expected = x * torch.rsqrt(torch.mean(x * x, dim=-1, keepdim=True) + case["epsilon"]) * a
            else:
                expected = F.layer_norm(x, (w,), a if kind else None, b if kind else None, case["epsilon"])
        elif case["op"] == "activation":
            expected = F.silu(x) if case["kind"] == 0 else F.gelu(x, approximate="tanh")
        elif case["op"] == "modulate":
            w = case["width"]
            first, second = tensor("a").reshape(2, w)
            scale, shift = (first, second) if case["kind"] == 0 else (second, first)
            expected = x.reshape(-1, w) * (1 + scale) + shift
        elif case["op"] == "split":
            source = x.reshape(case["rows"], 3, case["width"])
            expected = torch.cat([source[:, i, :].reshape(-1) for i in range(3)])
        else:
            qn, kn, h, kv, d, mode = [case[key] for key in ("queries", "keys", "heads", "kv_heads", "dim", "mode")]
            q = x.reshape(qn, h, d).transpose(0, 1)
            k = tensor("a").reshape(kn, kv, d).transpose(0, 1).repeat_interleave(h // kv, dim=0)
            v = tensor("b").reshape(kn, kv, d).transpose(0, 1).repeat_interleave(h // kv, dim=0)
            mask = torch.ones(qn, kn, dtype=torch.bool)
            if mode & 1:
                mask &= torch.from_numpy(read("mask", "u1")).bool()[None, :]
            if mode & 2:
                mask &= torch.arange(kn)[None, :] <= torch.arange(qn)[:, None] + case["offset"]
            if mode & 4:
                camera = torch.from_numpy(read("camera", "<i8"))
                mask &= camera[:, None] == camera[None, :]
            scores = q @ k.transpose(-2, -1) / (d ** 0.5)
            probabilities = torch.softmax(scores.masked_fill(~mask, -torch.inf), dim=-1).nan_to_num(0)
            context = F.scaled_dot_product_attention(q, k, v, attn_mask=mask)
            expected = torch.cat([scores.masked_fill(~mask, 0).transpose(0, 1).reshape(-1), probabilities.transpose(0, 1).reshape(-1), context.transpose(0, 1).reshape(-1)])
        expected = expected.reshape(-1).numpy()
        assert np.isfinite(expected).all()
        for backend in ("debug", "release", "sanitized", "metal"):
            observed = read(backend)
            assert observed.shape == expected.shape and np.isfinite(observed).all()
            error = np.abs(observed.astype(np.float64) - expected.astype(np.float64))
            budget = ATOL + RTOL * np.abs(expected.astype(np.float64))
            passed = bool(np.all(error <= budget))
            results.append({"index": case["index"], "op": case["op"], "backend": backend, "values": len(expected), "max_abs_error": float(error.max(initial=0)), "max_budget_fraction": float((error / budget).max(initial=0)), "passed": passed})
            count += len(expected)
    record = {"schema": "psionic-groot-device-f32-reference-v1", "passed": all(r["passed"] for r in results), "torch": torch.__version__, "numpy": np.__version__, "profile": "Coax F32 products, corrected F32 reductions, F32 private stages; Metal safe math; source-default BF16 parity is separate", "tolerance_frozen_before_observation": {"atol": ATOL, "rtol": RTOL}, "values_compared": count, "scope": "Owned component fixtures only; no original model, external inference or robot", "results": results}
    root = Path(__file__).resolve().parents[3]
    record["source_sha256"] = {name: hashlib.sha256((root / name).read_bytes()).hexdigest() for name in ("psionic/lib/robotics/groot-device-kernels.coax", "psionic/tests/robotics/groot-device-native.c", "psionic/tests/robotics/groot_device_reference.py")}
    with (args.directory / "reference.json").open("x") as stream:
        json.dump(record, stream, indent=2, sort_keys=True)
        stream.write("\n")
    assert record["passed"], "Frozen tolerance failed; inspect reference.json without widening it"
    print(f"GR00T device Torch{torch.__version__}: {len(results)} comparisons, {count} values; max budget fraction {max(r['max_budget_fraction'] for r in results):.6g}; CPU+realMetal passed")

if __name__ == "__main__":
    main()
