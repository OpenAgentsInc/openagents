# Firm reason: 2 (independent raw-block, transaction, UTXO and retained Core/Electrs oracle).
"""Check record.coax output without using any Coax decoder or formatter."""
import hashlib
import json
import pathlib
import struct
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
FIXTURE = ROOT / "bitcoin/fixtures/explorer-core-reorg"


def sha(data):
    return hashlib.sha256(hashlib.sha256(data).digest()).digest()


def compact(raw, at):
    prefix = raw[at]
    if prefix < 253:
        return prefix, at + 1
    size = {253: 2, 254: 4, 255: 8}[prefix]
    return int.from_bytes(raw[at + 1:at + 1 + size], "little"), at + 1 + size


def transaction(raw, at):
    start = at
    version = raw[at:at + 4]
    at += 4
    witness = raw[at:at + 2] == b"\0\1"
    if witness:
        at += 2
    body_start = at
    ni, at = compact(raw, at)
    inputs = []
    for _ in range(ni):
        txid = raw[at:at + 32][::-1].hex()
        vout = int.from_bytes(raw[at + 32:at + 36], "little")
        at += 36
        n, at = compact(raw, at)
        at += n + 4
        inputs.append(dict(txid=txid, vout=vout, coinbase=txid == "0" * 64 and vout == 0xffffffff))
    no, at = compact(raw, at)
    outputs = []
    for i in range(no):
        value = int.from_bytes(raw[at:at + 8], "little")
        n, at = compact(raw, at + 8)
        script = raw[at:at + n]
        at += n
        outputs.append(dict(vout=i, value_sat=value, script_hex=script.hex()))
    body_end = at
    if witness:
        for _ in range(ni):
            items, at = compact(raw, at)
            for _ in range(items):
                n, at = compact(raw, at)
                at += n
    locktime = raw[at:at + 4]
    at += 4
    stripped = version + raw[body_start:body_end] + locktime
    return dict(txid=sha(stripped)[::-1].hex(), size=at-start,
                weight=3*len(stripped)+at-start, inputs=inputs, outputs=outputs), at


def read_blocks():
    raw = (FIXTURE / "blk00000.dat").read_bytes()
    blocks = {}
    at = 0
    while at < len(raw):
        assert raw[at:at + 4] == bytes.fromhex("fabfb5da")
        n, = struct.unpack_from("<I", raw, at + 4)
        body = raw[at + 8:at + 8 + n]
        assert len(body) == n
        blockhash = sha(body[:80])[::-1].hex()
        count, pos = compact(body, 80)
        prefix = pos
        txs = []
        for i in range(count):
            tx, pos = transaction(body, pos)
            tx["order"] = i
            txs.append(tx)
        assert pos == len(body)
        leaves = [bytes.fromhex(t["txid"])[::-1] for t in txs]
        while len(leaves) > 1:
            if len(leaves) % 2:
                leaves.append(leaves[-1])
            leaves = [sha(leaves[i]+leaves[i+1]) for i in range(0, len(leaves), 2)]
        assert leaves[0] == body[36:68]
        blocks[blockhash] = dict(hash=blockhash, previous=body[4:36][::-1].hex(),
                                 time=int.from_bytes(body[68:72], "little"), size=n,
                                 weight=4*prefix+sum(t["weight"] for t in txs), transactions=txs)
        at += 8 + n
    assert len(blocks) == 108
    return blocks


def check(directory):
    blocks = read_blocks()
    for phase in "abc":
        actual = json.loads((directory / f"{phase}.json").read_text())
        reference = json.loads((FIXTURE / f"reference-{phase}.json").read_text())
        snapshot = actual["snapshot"]
        assert snapshot["profile"] == "validated-v1" and snapshot["mode"] == "fixture"
        assert snapshot["hash"] == reference["snapshot"]["hash"]
        assert snapshot["height"] == reference["snapshot"]["height"]
        assert snapshot["complete"] is True and snapshot["observation_ms"] is None
        active = set()
        cursor = snapshot["hash"]
        while cursor != "0" * 64:
            assert cursor not in active
            active.add(cursor)
            cursor = blocks[cursor]["previous"]
        coins = {}
        spent = {}
        for height, blockhash in enumerate(reversed(list_path(snapshot["hash"], blocks))):
            block = blocks[blockhash]
            for tx in block["transactions"]:
                if not tx["inputs"][0]["coinbase"]:
                    for i, inp in enumerate(tx["inputs"]):
                        key = (inp["txid"], inp["vout"])
                        assert key in coins
                        del coins[key]
                        spent[key] = dict(txid=tx["txid"], block=blockhash, height=height, vin=i)
                if height:
                    for out in tx["outputs"]:
                        if not out["script_hex"].startswith("6a"):
                            coins[(tx["txid"], out["vout"])] = out
        occurrences = {}
        for block in actual["blocks"]:
            raw = blocks[block["hash"]]
            for key in ("hash", "previous", "time", "size", "weight"):
                assert block[key] == raw[key], (phase, key, block["hash"])
            assert block["active"] == (block["hash"] in active)
            assert len(block["transactions"]) == len(raw["transactions"])
            assert int(block["chainwork"], 16) == 2 * (block["height"] + 1)
            for tx, rawtx in zip(block["transactions"], raw["transactions"]):
                for key in ("txid", "size", "weight", "order", "inputs"):
                    assert tx[key] == rawtx[key], (phase, key)
                occurrences[(block["hash"], tx["txid"])] = tx
                assert len(tx["outputs"]) == len(rawtx["outputs"])
                for out, rawout in zip(tx["outputs"], rawtx["outputs"]):
                    for key in ("vout", "value_sat", "script_hex"):
                        assert out[key] == rawout[key]
                    point = (tx["txid"], out["vout"])
                    status = ("stale" if not block["active"] else "unspendable" if block["height"] == 0 or out["script_hex"].startswith("6a")
                              else "unspent" if point in coins else "spent")
                    assert out["status"] == status, (phase, point, out["status"], status)
                    if status == "spent":
                        assert out["spend"] == spent[point]
        indexed = {b["hash"]: b for b in actual["blocks"]}
        for block in reference["blocks"]:
            for key in ("height", "previous", "time", "size", "active"):
                assert indexed[block["hash"]][key] == block[key]
        for tx in reference["transactions"]:
            occurrence = tx["occurrence"]
            projected = occurrences[(occurrence["block"], occurrence["txid"])]["outputs"]
            assert len(projected) == len(tx["outputs"])
            for out, expected in zip(projected, tx["outputs"]):
                for key, value in expected.items():
                    assert out[key] == value, (phase, key, out[key], value)
        for address in reference["addresses"]:
            script = address["script_hex"]
            raw_balance = sum(out["value_sat"] for out in coins.values() if out["script_hex"] == script)
            projected_balance = sum(out["value_sat"] for block in actual["blocks"] if block["active"]
                                    for tx in block["transactions"] for out in tx["outputs"]
                                    if out["status"] == "unspent" and out["script_hex"] == script)
            assert raw_balance == projected_balance == address["balance_sat"]
            for entry in address["outputs"]:
                occurrence = entry["occurrence"]
                output = entry["output"]
                projected = occurrences[(occurrence["block"], occurrence["txid"])]["outputs"][output["vout"]]
                for key, value in output.items():
                    assert projected[key] == value
        balances = [row["balance_sat"] for row in reference["addresses"]]
        print(f"{phase}: {len(indexed)} blocks; all raw txids/weights/outpoints/output states; retained Core/Electrs answers and exact-script balances {balances} match")


def list_path(tip, blocks):
    result = []
    while tip != "0" * 64:
        result.append(tip)
        tip = blocks[tip]["previous"]
    return result


if __name__ == "__main__":
    check(pathlib.Path(sys.argv[1]))
