#!/usr/bin/env python3
"""Extract address-bearing chain transactions for a bounded incident window.

Read-only against our own archival node. Given a seed address set and a block
range, it walks the range once per traversal generation and keeps every
transaction that spends a currently-known address, recording exact integer
satoshi values and real addresses.

It never guesses. A transaction is kept because the chain says one of its
prevouts belongs to a known address, not because it looked like a sweep.
"""
import json
import subprocess
import sys
from decimal import Decimal

DATADIR = "/var/lib/bitcoin"
CONF = "/etc/bitcoin/bitcoin.conf"


def cli(*args):
    p = subprocess.run(
        ["bitcoin-cli", "-conf=" + CONF, "-datadir=" + DATADIR, *args],
        capture_output=True,
        text=True,
    )
    if p.returncode != 0:
        raise RuntimeError(p.stderr.strip()[:300])
    s = p.stdout.strip()
    try:
        return json.loads(s, parse_float=Decimal)
    except json.JSONDecodeError:
        return s


def sats(v):
    return int((Decimal(v) * 100000000).to_integral_value())


def main():
    lo, hi, depth, seed_path, out_path = (
        int(sys.argv[1]),
        int(sys.argv[2]),
        int(sys.argv[3]),
        sys.argv[4],
        sys.argv[5],
    )
    seeds = set(json.load(open(seed_path)))

    blocks = []
    for height in range(lo, hi + 1):
        bh = cli("getblockhash", str(height))
        blk = cli("getblock", bh, "3")
        blocks.append((height, bh, blk))
        print("  loaded %d (%d tx)" % (height, len(blk["tx"])), file=sys.stderr)

    known = set(seeds)
    kept = {}
    for generation in range(depth + 1):
        added = set()
        for height, bh, blk in blocks:
            for tx in blk["tx"]:
                if tx["txid"] in kept:
                    continue
                if any("coinbase" in v for v in tx["vin"]):
                    continue
                inputs = []
                complete = True
                for v in tx["vin"]:
                    pv = v.get("prevout")
                    if pv is None:
                        complete = False
                        continue
                    inputs.append(
                        {
                            "address": pv["scriptPubKey"].get("address"),
                            "prevTxid": v["txid"],
                            "prevVout": v["vout"],
                            "scriptType": pv["scriptPubKey"]["type"],
                            "valueSats": sats(pv["value"]),
                        }
                    )
                if not complete:
                    continue
                if not any(i["address"] in known for i in inputs):
                    continue
                outputs = [
                    {
                        "address": o["scriptPubKey"].get("address"),
                        "n": o["n"],
                        "scriptType": o["scriptPubKey"]["type"],
                        "valueSats": sats(o["value"]),
                    }
                    for o in tx["vout"]
                ]
                kept[tx["txid"]] = {
                    "blockHash": bh,
                    "blockHeight": height,
                    "blockTime": blk["time"],
                    "generation": generation,
                    "inputs": inputs,
                    "outputs": outputs,
                    "txid": tx["txid"],
                    "vsize": tx["vsize"],
                }
                for o in outputs:
                    if o["address"] is not None:
                        added.add(o["address"])
        print(
            "  generation %d: %d transactions, %d new addresses"
            % (generation, len(kept), len(added - known)),
            file=sys.stderr,
        )
        if not (added - known):
            break
        known |= added

    with open(out_path, "w") as fh:
        json.dump(
            {
                "endHeight": hi,
                "seedAddresses": sorted(seeds),
                "startHeight": lo,
                "traversalDepth": depth,
                "transactions": [kept[k] for k in sorted(kept)],
            },
            fh,
            sort_keys=True,
        )
    print("wrote %s with %d transactions" % (out_path, len(kept)), file=sys.stderr)


if __name__ == "__main__":
    main()
