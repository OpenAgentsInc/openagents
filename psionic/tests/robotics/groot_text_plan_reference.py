#!/usr/bin/env python3
"""Independent original-header comparison for new text graph's native ABI facts."""
import argparse
import hashlib
import json
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("fixture", type=Path)
parser.add_argument("facts", nargs="+", type=Path)
parser.add_argument("--record", required=True, type=Path)
args = parser.parse_args()
inventory = json.loads((args.fixture / "tensor-inventory.json").read_text())
actual = {}
for shard, info in enumerate(inventory["shards"]):
    raw = (args.fixture / info["header_file"]).read_bytes()
    assert hashlib.sha256(raw).hexdigest() == info["header_sha256"]
    header = json.loads(raw)
    for name, value in header.items():
        if name == "__metadata__":
            continue
        shape = value["shape"]
        actual[name] = (shard, shape, value["dtype"], info["data_start"] + value["data_offsets"][0], info["data_start"] + value["data_offsets"][1])
root = "backbone.model.model.language_model."
names = [root + "embed_tokens.weight"]
slots = ["input_layernorm.weight", "post_attention_layernorm.weight", "self_attn.q_norm.weight", "self_attn.k_norm.weight", "self_attn.q_proj.weight", "self_attn.k_proj.weight", "self_attn.v_proj.weight", "self_attn.o_proj.weight", "mlp.gate_proj.weight", "mlp.up_proj.weight", "mlp.down_proj.weight"]
for layer in range(16):
    names.extend(root + f"layers.{layer}." + slot for slot in slots)
names.append(root + "norm.weight")
records = []
for path in args.facts:
    rows = [line.split("\t") for line in path.read_text().splitlines()]
    assert len(rows) == 178
    for index, row in enumerate(rows):
        assert len(row) == 17 and int(row[0]) == index and row[1] == names[index]
        shard, shape, dtype, begin, end = actual[row[1]]
        assert dtype == "BF16"
        rank, *fields = map(int, row[2:])
        dims, fields = fields[:5], fields[5:]
        size, target, decode, nr, nc, selected_shard, selected_begin, selected_end, count = fields
        vector = len(shape) == 1
        assert rank == len(shape) and dims == shape + [0] * (5 - len(shape))
        assert size == end - begin and target == size * (2 if vector else 1)
        assert bool(decode) == vector and nr == (0 if vector else shape[0])
        assert nc == (shape[0] if vector else shape[1]) == count
        assert (selected_shard, selected_begin, selected_end) == (shard, begin, end)
    records.append({"mode": path.stem, "bindings": len(rows), "sha256": hashlib.sha256(path.read_bytes()).hexdigest()})
assert len({r["sha256"] for r in records}) == 1
record = {"schema": "psionic-groot-text-plan-native-reference-v1", "passed": True,
          "scope": "Original committed header identities, all178active text binding metadata, native descriptor/ABI/private cursor/copy checks only; no model inference.",
          "native_modes_bit_identical": True, "modes": records}
with args.record.open("x") as stream:
    json.dump(record, stream, indent=2, sort_keys=True)
    stream.write("\n")
print(f"Original text-plan reference: {len(records)} native modes ×178original bindings passed; identical facts")
