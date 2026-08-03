#!/usr/bin/env python3
"""Extract a frozen, content-addressed historical block bundle from our own
archival Bitcoin Core node in the exact canonical shape required by
`openagents.historical_block_bundle.v1`.

Read-only. Uses only getblockhash / getblock / getblockchaininfo. Never touches
a wallet RPC, never writes to the node, and never emits node credentials.

Exact integer satoshi arithmetic throughout: BTC amounts are parsed as Decimal
from raw JSON and converted to integer satoshis; no float is ever used.
"""
import hashlib
import json
import os
import subprocess
import sys
from decimal import Decimal

DATADIR = "/var/lib/bitcoin"
CONF = "/etc/bitcoin/bitcoin.conf"

IN_TYPE = {
    "witness_v0_keyhash": "p2wpkh",
    "scripthash": "p2sh_p2wpkh",
    "pubkeyhash": "p2pkh",
    "witness_v1_taproot": "p2tr",
}
OUT_TYPE = {
    "witness_v0_keyhash": "p2wpkh",
    "scripthash": "p2sh",
    "pubkeyhash": "p2pkh",
    "witness_v1_taproot": "p2tr",
}


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


def canonical(value):
    """Byte-for-byte match of forensicCanonicalJson in packages/forensic-contract."""
    if value is None:
        return "null"
    if value is True:
        return "true"
    if value is False:
        return "false"
    if isinstance(value, str):
        return json.dumps(value, ensure_ascii=False)
    if isinstance(value, int):
        return json.dumps(value)
    if isinstance(value, list):
        return "[" + ",".join(canonical(v) for v in value) + "]"
    if isinstance(value, dict):
        parts = []
        for k in sorted(value.keys()):
            v = value[k]
            if v is None:
                continue
            parts.append(json.dumps(k, ensure_ascii=False) + ":" + canonical(v))
        return "{" + ",".join(parts) + "}"
    raise TypeError("unsupported %r" % type(value))


def digest(value):
    return "sha256:" + hashlib.sha256(canonical(value).encode("utf-8")).hexdigest()


def iso(epoch):
    import datetime

    return (
        datetime.datetime.fromtimestamp(epoch, datetime.timezone.utc)
        .strftime("%Y-%m-%dT%H:%M:%S.")
        + "000Z"
    )


def build_transaction(tx):
    vin = tx["vin"]
    if any("coinbase" in v for v in vin):
        return None
    inputs = []
    complete = True
    in_total = 0
    for index, v in enumerate(vin):
        pv = v.get("prevout")
        if pv is None:
            complete = False
            in_type = "other"
        else:
            in_type = IN_TYPE.get(pv["scriptPubKey"]["type"], "other")
            in_total += sats(pv["value"])
        entry = {
            "inputRef": "input.%s.%d" % (tx["txid"], index),
            "inputType": in_type,
            "sequence": v["sequence"],
        }
        witness = v.get("txinwitness") or []
        if len(witness) >= 1 and witness[0]:
            der_len = len(witness[0]) // 2
            if der_len == 72:
                entry["signatureRWidthBytes"] = 33
            elif der_len == 71:
                entry["signatureRWidthBytes"] = 32
        if len(witness) >= 2 and witness[1]:
            entry["pubkeyDigest"] = "sha256:" + hashlib.sha256(
                bytes.fromhex(witness[1])
            ).hexdigest()
        inputs.append(entry)

    out_total = sum(sats(o["value"]) for o in tx["vout"])
    out_types = {OUT_TYPE.get(o["scriptPubKey"]["type"], "other") for o in tx["vout"]}
    record = {
        "inputs": inputs,
        "locktime": tx["locktime"],
        "outputCount": len(tx["vout"]),
        "outputScriptType": out_types.pop() if len(out_types) == 1 else "other",
        "outputValueSats": str(out_total),
        "prevoutDataStatus": "complete" if complete else "missing_required",
        "transactionRef": "transaction.%s" % tx["txid"],
        "txid": "sha256:%s" % tx["txid"],
        "version": tx["version"],
        "vsize": tx["vsize"],
    }
    if complete:
        fee = in_total - out_total
        if fee >= 0:
            record["feeSats"] = str(fee)
        else:
            record["prevoutDataStatus"] = "missing_required"
    return record


def main():
    lo, hi = int(sys.argv[1]), int(sys.argv[2])
    bundle_ref = sys.argv[3]
    out_path = sys.argv[4]

    info = cli("getblockchaininfo")
    genesis = cli("getblockhash", "0")
    network_info = cli("getnetworkinfo")
    # Node identity digest: binds the exact node posture without exposing any
    # address, credential, or cookie.
    node_identity = digest(
        {
            "chain": info["chain"],
            "genesis": genesis,
            "instance": "oa-bitcoind",
            "pruned": info["pruned"],
            "subversion": network_info["subversion"],
        }
    )

    blocks = []
    for height in range(lo, hi + 1):
        bh = cli("getblockhash", str(height))
        blk = cli("getblock", bh, "3")
        transactions = []
        for tx in blk["tx"]:
            record = build_transaction(tx)
            if record is not None:
                transactions.append(record)
        blocks.append(
            {
                "blockHash": "sha256:%s" % bh,
                "blockTime": iso(blk["time"]),
                "height": height,
                "rawBlockDigest": "sha256:%s"
                % hashlib.sha256(bytes.fromhex(cli("getblock", bh, "0"))).hexdigest(),
                "transactions": transactions,
            }
        )
        print("  block %d: %d non-coinbase tx" % (height, len(transactions)), file=sys.stderr)

    body = {
        "blocks": blocks,
        "bundleRef": bundle_ref,
        "endHeight": hi,
        "genesisHash": "sha256:%s" % genesis,
        "network": "mainnet",
        "sourceIdentityDigest": node_identity,
        "startHeight": lo,
    }
    import datetime

    bundle = {
        "schema": "openagents.historical_block_bundle.v1",
        "blocks": blocks,
        "bundleRef": bundle_ref,
        "capturedAt": datetime.datetime.now(datetime.timezone.utc).strftime(
            "%Y-%m-%dT%H:%M:%S."
        )
        + "000Z",
        "contentDigest": digest(body),
        "endHeight": hi,
        "genesisHash": "sha256:%s" % genesis,
        "network": "mainnet",
        "sourceIdentityDigest": node_identity,
        "startHeight": lo,
    }
    with open(out_path, "w") as fh:
        fh.write(canonical(bundle))
    print(
        "bundle %s: %d blocks, %d tx, %d bytes, digest %s"
        % (
            bundle_ref,
            len(blocks),
            sum(len(b["transactions"]) for b in blocks),
            os.path.getsize(out_path),
            bundle["contentDigest"],
        ),
        file=sys.stderr,
    )


if __name__ == "__main__":
    main()
