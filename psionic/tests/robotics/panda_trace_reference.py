#!/usr/bin/env python3
"""Firm reason2: independent struct-layout/SHA oracle; Python stdlib only."""
import argparse
import hashlib
import json
from pathlib import Path
import struct


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("prefix", type=Path)
    parser.add_argument("--record", type=Path)
    args = parser.parse_args()
    raw = Path(str(args.prefix)+".inputs").read_bytes()
    trace = Path(str(args.prefix)+".trace").read_bytes()
    chains = Path(str(args.prefix)+".chains").read_bytes()
    source = struct.Struct("<5q7I54Q")
    assert source.size == 500 and len(raw) == 513*500
    assert len(trace) == 64+513*512 and len(chains) == 513*32
    previous = bytes(range(1, 33))
    assert trace[:32] == previous
    for i in range(513):
        values = source.unpack_from(raw, i*500)
        epoch, serial, tick, substep, flags = values[:5]
        expected = (b"PTR1"+struct.pack("<HBBQQQQQ", 1, flags, substep,
                    epoch, serial, tick, 25*tick+substep, 0)+
                    struct.pack("<7I54Q", *values[5:])+b"END1")
        assert len(expected) == 512
        actual = trace[32+i*512:32+(i+1)*512]
        assert actual == expected, f"canonical record{i}"
        previous = hashlib.sha256(previous+expected).digest()
        assert chains[i*32:(i+1)*32] == previous, f"SHA link{i}"
    assert trace[-32:] == previous
    record = {"schema": "psionic-panda-trace-reference-v1", "passed": True,
              "canonical_records": 513, "canonical_bytes": 513*512,
              "sha_chain_steps": 513, "trace_bytes": len(trace),
              "trace_sha256": hashlib.sha256(trace).hexdigest(),
              "final_chain_sha256": previous.hex(),
              "source_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              "scope": "Owned codec fixtures, not a simulator trajectory or authenticated remote receipt."}
    if args.record:
        args.record.write_text(json.dumps(record, indent=2)+"\n")
    print(json.dumps(record, sort_keys=True))


if __name__ == "__main__":
    main()
