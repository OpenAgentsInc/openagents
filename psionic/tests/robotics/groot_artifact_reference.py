#!/usr/bin/env python3
"""Firm reason: 2 (independent oracle). Original JSON versus emitted native facts."""
import hashlib
import json
from pathlib import Path
import sys

root = Path(__file__).resolve().parents[2] / "fixtures/robotics/groot-libero"
provenance = json.loads((root / "provenance.json").read_text())
for item in provenance["files"]:
    assert hashlib.sha256((root / item["path"]).read_bytes()).hexdigest() == item["sha256"], item["path"]
inventory = json.loads((root / "tensor-inventory.json").read_text())
expected = {}
headers = []
for shard, spec in enumerate(inventory["shards"]):
    raw = (root / spec["header_file"]).read_bytes()
    header = json.loads(raw)
    headers.append(header)
    for name, tensor in header.items():
        if name == "__metadata__":
            continue
        shape = tensor["shape"]
        elements = 1
        for d in shape:
            elements *= d
        expected[(shard, name)] = [0, len(shape), *shape, *([0] * (5-len(shape))), elements,
                                  8+len(raw)+tensor["data_offsets"][0],
                                  8+len(raw)+tensor["data_offsets"][1]]
names = ["action_head.state_encoder.layer1.W", "action_head.state_encoder.layer1.b",
         "action_head.state_encoder.layer2.W", "action_head.state_encoder.layer2.b"]
expected_ranges = {}
for part, name in enumerate(names):
    t = headers[1][name]
    start, end = t["data_offsets"]
    stride = (end-start)//32
    inputs = t["shape"][1] if len(t["shape"]) == 3 else 0
    outputs = t["shape"][-1]
    for category in range(32):
        begin = 8 + len((root / inventory["shards"][1]["header_file"]).read_bytes()) + start + category*stride
        expected_ranges[(part, category)] = [name, begin, begin+stride, stride, inputs, outputs]
assert sys.argv[1:], "expected native fact files"
for filename in sys.argv[1:]:
    got, ranges = {}, {}
    for line in Path(filename).read_text().splitlines():
        p = line.split("|")
        if p[0] == "tensor":
            key = int(p[1]), p[2]
            assert key not in got
            got[key] = list(map(int, p[3:]))
        else:
            assert p[0] == "range"
            key = int(p[1]), int(p[2])
            assert key not in ranges
            ranges[key] = [p[3], *map(int, p[4:])]
    assert got == expected, filename
    assert ranges == expected_ranges, filename
print("GR00T original-header reference: all1031 typed tensors and128 category ranges exact in each mode")
