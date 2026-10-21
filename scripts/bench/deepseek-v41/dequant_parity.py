#!/usr/bin/env python3
"""Write reference dequantized weight values for Psionic's V4.1 loader parity check.

Retained acceptance tooling for the V0 oracle (#12371). It reads slices of
the original Hugging Face checkpoint with `safetensors` and dequantizes them
the way the pinned reference does, then writes a
`psionic.deepseek_v41.dequant_parity.v1` file that
`cargo run -p psionic-models --example deepseek_v41_weights_check -- DIR --oracle FILE`
compares against (#12373).

The reference's arithmetic, as `weight.float() * scale.float()`:
  - FP8 weights: one E8M0 scale per 32x32 block (`inference/model.py` lines
    225-233; `convert.py` dequantizes `wo_a` the same way, lines 170-178).
  - FP4 experts: the I8 bytes viewed as `float4_e2m1fn_x2`, the low nibble
    first, decoded through `convert.py`'s `FP4_TABLE` (lines 13-15, 31-34),
    times one E8M0 scale per 32 elements along K.
  - Engram rows: `values.float().unflatten(-1, (-1, 32)) * scales.float()`,
    as `ParallelEngramEmbedding.forward` (`model.py` lines 317-319) does
    before it casts to BF16.
  - BF16 and F32 weights: `.float()`.

    dequant_parity.py --hf-dir DIR --reference-dir DIR/inference --out FILE.json
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import sys
from pathlib import Path

import torch
from safetensors import safe_open

REVISION = "1bae0e1088993b9b82de08f59de5be5a34e42170"
SCHEMA = "psionic.deepseek_v41.dequant_parity.v1"
BLOCK = 32

# (kind, tensor or layer, coordinates). Covers FP8 attention and shared-expert
# weights, FP4 routed experts (w1, w2, w3), BF16 and F32 weights, and both
# Engram tables (first, middle, and last rows).
CASES = [
    ("fp8_block", "layers.0.attn.wq_a.weight", 0, 0),
    ("fp8_block", "layers.0.attn.wq_a.weight", 39, 159),
    ("fp8_block", "layers.20.attn.wkv.weight", 3, 77),
    ("fp8_block", "layers.39.attn.wo_b.weight", 100, 200),
    ("fp8_block", "layers.14.ffn.shared_experts.w2.weight", 5, 9),
    ("row", "layers.2.attn.wq_b.weight", 4095),
    ("row", "layers.20.ffn.experts.17.w1.weight", 0),
    ("row", "layers.20.ffn.experts.17.w1.weight", 2303),
    ("row", "layers.20.ffn.experts.17.w2.weight", 100),
    ("row", "layers.2.ffn.experts.383.w3.weight", 1234),
    ("row", "layers.39.ffn.experts.0.w2.weight", 5119),
    ("row", "layers.20.attn.compressor.wkv.weight", 7),
    ("row", "layers.20.ffn.gate.weight", 42),
    ("engram_row", 1, 0),
    ("engram_row", 1, 192_003_084),
    ("engram_row", 1, 384_006_167),
    ("engram_row", 14, 0),
    ("engram_row", 14, 123_456_789),
    ("engram_row", 14, 384_016_681),
]


def load_fp4_table(reference_dir: Path) -> torch.Tensor:
    spec = importlib.util.spec_from_file_location("reference_convert", reference_dir / "convert.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.FP4_TABLE


class Checkpoint:
    def __init__(self, hf_dir: Path):
        self.dir = hf_dir
        self.weight_map = json.loads((hf_dir / "model.safetensors.index.json").read_text())["weight_map"]
        self.handles = {}

    def slice(self, name: str):
        shard = self.weight_map[name]
        if shard not in self.handles:
            self.handles[shard] = safe_open(self.dir / shard, framework="pt", device="cpu")
        return self.handles[shard].get_slice(name)


def bits(values: torch.Tensor) -> list[int]:
    return [int(b) & 0xFFFFFFFF for b in values.contiguous().view(torch.int32).flatten().tolist()]


def fp4_row(packed: torch.Tensor, scale: torch.Tensor, table: torch.Tensor) -> torch.Tensor:
    raw = packed.view(torch.uint8)
    low, high = raw & 0x0F, (raw >> 4) & 0x0F
    values = torch.stack([table[low.long()], table[high.long()]], dim=-1).flatten()
    return (values.unflatten(0, (-1, BLOCK)) * scale.float().unsqueeze(-1)).flatten()


def dequant_row(ck: Checkpoint, name: str, row: int, table: torch.Tensor) -> torch.Tensor:
    weight = ck.slice(name)
    dtype = weight.get_dtype()
    w = weight[row : row + 1][0]
    if dtype == "I8":
        scale = ck.slice(name.replace(".weight", ".scale"))[row : row + 1][0]
        return fp4_row(w, scale, table)
    if dtype == "F8_E4M3":
        scale_row = ck.slice(name.replace(".weight", ".scale"))[row // BLOCK : row // BLOCK + 1][0]
        return (w.float().unflatten(0, (-1, BLOCK)) * scale_row.float().unsqueeze(-1)).flatten()
    return w.float()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--hf-dir", required=True, type=Path)
    parser.add_argument("--reference-dir", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()

    table = load_fp4_table(args.reference_dir)
    ck = Checkpoint(args.hf_dir)
    cases = []
    for case in CASES:
        kind = case[0]
        if kind == "fp8_block":
            _, name, br, bc = case
            weight = ck.slice(name)[br * BLOCK : (br + 1) * BLOCK, bc * BLOCK : (bc + 1) * BLOCK]
            scale = ck.slice(name.replace(".weight", ".scale"))[br : br + 1, bc : bc + 1]
            assert weight.dtype == torch.float8_e4m3fn and scale.dtype == torch.float8_e8m0fnu
            values = weight.float() * scale.float()
            cases.append({"kind": kind, "tensor": name, "block_row": br, "block_col": bc, "f32_bits": bits(values)})
        elif kind == "row":
            _, name, row = case
            values = dequant_row(ck, name, row, table)
            cases.append({"kind": kind, "tensor": name, "row": row, "f32_bits": bits(values)})
        else:
            _, layer, row = case
            values = ck.slice(f"layers.{layer}.engram.embed.weight")[row : row + 1]
            scales = ck.slice(f"layers.{layer}.engram.embed.scale")[row : row + 1]
            assert values.dtype == torch.float8_e4m3fn and scales.dtype == torch.float8_e8m0fnu
            out = (values.float().unflatten(-1, (-1, BLOCK)) * scales.float().unsqueeze(-1)).flatten()
            cases.append({"kind": kind, "layer": layer, "row": row, "f32_bits": bits(out)})
        print(f"{kind} {case[1:]}: {len(cases[-1]['f32_bits'])} values", file=sys.stderr)

    args.out.write_text(json.dumps({"schema": SCHEMA, "revision": REVISION, "cases": cases}) + "\n")
    print(f"wrote {len(cases)} cases to {args.out}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
