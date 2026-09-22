"""Emit deterministic demo artifacts: a LoRA adapter dir, a pointer-head
dir, and a tokenizer config, all derived from the seed.

This is a demonstration driver, not a trainer. It proves the
training-to-candidate contract — digests, the trials ledger, the seal —
with artifacts a reader can byte-for-byte reproduce. Real adapter
training runs through `training/lev-adapter` (Apple's toolkit) or the
kev port's own pipeline; this script stands in for whatever produced
`adapter_model.safetensors` and `head.safetensors` so the flow can be
rehearsed without paid compute.

Usage:
    python3 make_artifacts.py --seed 11 --out artifacts/
"""

import argparse
import hashlib
import json
import struct
from pathlib import Path


def stream(seed: int, name: str, length: int) -> bytes:
    """Deterministic bytes: sha256(seed||name||counter) chained."""
    out = bytearray()
    counter = 0
    while len(out) < length:
        block = hashlib.sha256(f"{seed}:{name}:{counter}".encode()).digest()
        out.extend(block)
        counter += 1
    return bytes(out[:length])


def safetensors(tensors: dict) -> bytes:
    """A valid safetensors file: LE u64 header length, JSON header, raw
    F32 tensor bytes in header order."""
    header = {}
    offset = 0
    body = bytearray()
    for name, (shape, data) in tensors.items():
        header[name] = {
            "dtype": "F32",
            "shape": shape,
            "data_offsets": [offset, offset + len(data)],
        }
        offset += len(data)
        body.extend(data)
    header_bytes = json.dumps(header).encode()
    return struct.pack("<Q", len(header_bytes)) + header_bytes + bytes(body)


def f32_tensor(seed: int, name: str, count: int) -> bytes:
    """`count` deterministic f32 values in [-0.02, 0.02] — init-scale
    noise, honestly labeled."""
    raw = stream(seed, name, count * 4)
    return b"".join(
        struct.pack("<f", (b / 255.0 - 0.5) * 0.04)
        for b in raw[::4][:count]
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--seed", type=int, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()

    adapter = args.out / "adapter"
    head = args.out / "head"
    tokenizer = args.out / "tokenizer"
    for directory in (adapter, head, tokenizer):
        directory.mkdir(parents=True, exist_ok=True)

    rank, dim = 8, 16  # demo shape; a real adapter matches the backbone
    adapter_config = {
        "peft_type": "LORA",
        "r": rank,
        "lora_alpha": 16,
        "target_modules": ["q_proj", "v_proj"],
        "base_model_name_or_path": "kev-3b-base",
    }
    (adapter / "adapter_config.json").write_text(json.dumps(adapter_config, indent=2))
    (adapter / "adapter_model.safetensors").write_bytes(
        safetensors(
            {
                f"base_model.model.{module}.lora_A.weight": (
                    [rank, dim],
                    f32_tensor(args.seed, f"{module}.a", rank * dim),
                )
                for module in ("q_proj", "v_proj")
            }
            | {
                f"base_model.model.{module}.lora_B.weight": (
                    [dim, rank],
                    f32_tensor(args.seed, f"{module}.b", dim * rank),
                )
                for module in ("q_proj", "v_proj")
            }
        )
    )

    dp = 256
    (head / "head.safetensors").write_bytes(
        safetensors(
            {
                "q.weight": ([dp, dim], f32_tensor(args.seed, "q.w", dp * dim)),
                "q.bias": ([dp], f32_tensor(args.seed, "q.b", dp)),
                "k.weight": ([dp, dim], f32_tensor(args.seed, "k.w", dp * dim)),
                "k.bias": ([dp], f32_tensor(args.seed, "k.b", dp)),
            }
        )
    )

    (tokenizer / "config.json").write_text(
        json.dumps({"render": "kev-blocks-v1", "max_state_bytes": 65536}, indent=2)
    )

    for name, path in (
        ("adapter", adapter / "adapter_model.safetensors"),
        ("head", head / "head.safetensors"),
        ("tokenizer", tokenizer / "config.json"),
    ):
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        print(f"{name} sha256:{digest}")


if __name__ == "__main__":
    main()
