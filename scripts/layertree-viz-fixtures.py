#!/usr/bin/env python3
"""Generate labelled simulation logs for the three-level viewer acceptance check."""
import argparse
import hashlib
import json
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("output", type=Path)
args = parser.parse_args()


def digest(value):
    return hashlib.sha256(value.encode()).hexdigest()


count = 16_385
amount = 1_000
leaves = [
    {
        "pos": i,
        "leaf": {
            "amount": amount,
            "exit_key": digest(f"exit {i}"),
            "owner_key": digest(f"owner {i}"),
            "evictor_key": digest("operator"),
            "evict_after": 1_000,
            "nonce": i,
            "seq": 0,
        },
    }
    for i in range(count)
]
seq = 0
args.output.parent.mkdir(parents=True, exist_ok=True)
with args.output.open("w") as output:
    def emit(kind, height, **fields):
        global seq
        output.write(json.dumps(dict(seq=seq, height=height, network="lab", kind=kind,
                                     synthetic=True, **fields), separators=(",", ":")) + "\n")
        seq += 1

    emit("tree_open", 100, tree_id=digest("simulated three-level tree"), depth=15,
         dust_min=330, unroll_delay=12, enclave_key=digest("simulated enclave"),
         outpoint=digest("simulated opening") + ":0", leaves=leaves,
         root=dict(hash=digest("illustrative root; not a Merkle commitment"), sum=count * amount))
    emit("enclave_down", 100)
    for height in range(101, 112):
        emit("block", height, spent=[])
    # Fanout 128: one root, two intermediate groups, 129 leaf groups.
    for level, groups in enumerate([1, 2, 129]):
        for group in range(groups):
            txid = digest(f"simulated level {level} group {group}")
            emit("unroll_broadcast", 111 + level, level=level, txid=txid, cpfp_fee=1_000)
            emit("unroll_confirmed", 112 + level, level=level, txid=txid)
    for item in leaves:
        emit("payout", 114, exit_key=item["leaf"]["exit_key"], amount=amount)
print(f"Wrote {args.output}: {count} simulated leaves, three levels, {seq} events")
