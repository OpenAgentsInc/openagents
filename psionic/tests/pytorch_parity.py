#!/usr/bin/env python3
"""Chapter 250's opt-in PyTorch comparator. Not a check, never needed at run time.

Loads the two retained MNIST models of chapter 118 (the MLP and the CNN,
psionic/fixtures/{mnist,cnn}/run-v1/model.safetensors), recomputes their
scores for the 2,048-image official test slice in PyTorch float32, and
compares them with the scores Psionic recorded during the run
(test-scores.f32). It writes one JSON record.

PyTorch is not a repository dependency. Run it only through an ephemeral Nix
shell, with MNIST acquired by psionic/tests/mnist_reference.py acquire:

  nix shell --impure --expr 'let p = (builtins.getFlake "nixpkgs").legacyPackages.${builtins.currentSystem}; in p.python3.withPackages (ps: [ps.torch])' \\
    -c python3 -I psionic/tests/pytorch_parity.py DATA_DIR OUT.json
"""
import json
import struct
import sys
from pathlib import Path

import torch

ROOT = Path(__file__).resolve().parents[2]
ROWS = 2048


def safetensors(path):
    raw = path.read_bytes()
    n = struct.unpack("<Q", raw[:8])[0]
    header = json.loads(raw[8:8 + n])
    out = {}
    for name, item in header.items():
        if name == "__metadata__":
            continue
        assert item["dtype"] == "F32"
        a, b = item["data_offsets"]
        values = torch.frombuffer(bytearray(raw[8 + n + a:8 + n + b]), dtype=torch.float32)
        out[name] = values.reshape(item["shape"])
    return out


def images(data_dir):
    raw = (data_dir / "t10k-images-idx3-ubyte").read_bytes()
    labels = (data_dir / "t10k-labels-idx1-ubyte").read_bytes()[8:8 + ROWS]
    pixels = torch.tensor(list(raw[16:16 + ROWS * 784]), dtype=torch.float64).reshape(ROWS, 28, 28)
    # Chapter 118's preprocessing: each 2 by 2 block's byte sum over 1,020,
    # in binary64, rounded once to binary32.
    pooled = pixels.reshape(ROWS, 14, 2, 14, 2).sum(dim=(2, 4)) / 1020
    return pooled.to(torch.float32), torch.tensor(list(labels))


def compare(name, scores, recorded, labels):
    gap = (scores - recorded).abs()
    agree = int((scores.argmax(1) == recorded.argmax(1)).sum())
    correct = int((scores.argmax(1) == labels).sum())
    return {"model": name, "rows": ROWS, "max_abs_gap": float(gap.max()),
            "exact_scores": int((gap == 0).sum()), "scores": ROWS * 10,
            "argmax_agree": agree, "torch_correct": correct,
            "psionic_correct": int((recorded.argmax(1) == labels).sum())}


def main():
    data_dir, out = Path(sys.argv[1]), Path(sys.argv[2])
    torch.use_deterministic_algorithms(True)
    torch.set_num_threads(1)
    x, labels = images(data_dir)
    results = []
    with torch.no_grad():
        p = safetensors(ROOT / "psionic/fixtures/mnist/run-v1/model.safetensors")
        h = torch.relu(x.reshape(ROWS, 196) @ p["fc1.weight"] + p["fc1.bias"])
        s = h @ p["fc2.weight"] + p["fc2.bias"]
        recorded = torch.frombuffer(bytearray((ROOT / "psionic/fixtures/mnist/run-v1/test-scores.f32").read_bytes()), dtype=torch.float32).reshape(ROWS, 10)
        results.append(compare("chapter 118 MNIST MLP", s, recorded, labels))
        p = safetensors(ROOT / "psionic/fixtures/cnn/run-v1/model.safetensors")
        a = torch.relu(torch.nn.functional.conv2d(x.reshape(ROWS, 1, 14, 14), p["conv.weight"]) + p["conv.bias"])
        s = a.reshape(ROWS, 288) @ p["fc.weight"] + p["fc.bias"]
        recorded = torch.frombuffer(bytearray((ROOT / "psionic/fixtures/cnn/run-v1/test-scores.f32").read_bytes()), dtype=torch.float32).reshape(ROWS, 10)
        results.append(compare("chapter 118 MNIST CNN", s, recorded, labels))
    record = {"schema": 1, "comparator": "PyTorch " + torch.__version__ + ", CPU float32, 1 thread, deterministic",
              "inputs": "official MNIST test images 0-2047, chapter 118's 2x2 pooling", "results": results}
    out.write_text(json.dumps(record, indent=2) + "\n")
    print(json.dumps(record, indent=2))


if __name__ == "__main__":
    main()
