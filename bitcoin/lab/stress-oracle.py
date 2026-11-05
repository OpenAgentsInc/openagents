# Firm reason: 2 (independent raw-block oracle for fully validated large-page fixtures).
"""Check paging against hashes, Merkle roots, amounts and spend links from raw bytes."""
import json
import sys
import urllib.parse
import urllib.request
from oracle import read_blocks, list_path


def check(base, archive):
    blocks = read_blocks(archive)
    ticket = 0
    def get(route, key=None, offset=0):
        nonlocal ticket
        ticket += 1
        query = dict(source="own-node", ticket=ticket, cursor="latest" if route in ("snapshot", "window") else cursor, offset=offset)
        if key is not None:
            query["key"] = key
        with urllib.request.urlopen(base + "/api/lab/v1/" + route + "?" + urllib.parse.urlencode(query), timeout=20) as response:
            assert response.status == 200 and response.headers["Cache-Control"] == "no-store"
            data = response.read(2097153)
            assert len(data) <= 2097152
        result = json.loads(data)
        assert result["ticket"] == str(ticket) and result["snapshot"]["complete"]
        assert result["snapshot"]["profile"] == "validated-v1"
        return result
    snap = get("snapshot")["snapshot"]
    cursor = snap["cursor"]
    assert snap["height"] == 105 and len(blocks) == 109
    block = blocks[snap["hash"]]
    assert len(block["transactions"]) == 72
    # Regtest compact target, checked independently of Coax's PoW code.
    assert int(snap["hash"], 16) <= 0x7fffff << (8 * (0x20 - 3))
    def pages(route, field, cap, key=None):
        rows = []
        while True:
            result = get(route, key, len(rows))
            page = result["page"]
            assert page["returned"] == len(result[field]) <= cap
            assert page["omitted"] == page["total"] - page["returned"]
            rows.extend(result[field])
            assert page["next"] == (cursor + "~" + str(len(rows)) if len(rows) < page["total"] else "")
            if not page["next"]:
                assert len(rows) == page["total"]
                return rows
    window = pages("window", "blocks", 24)
    assert len(window) == 109
    txs = pages("transactions", "transactions", 32, snap["hash"])
    assert len(txs) == 72
    for actual, expected in zip(txs, block["transactions"], strict=True):
        assert all(actual[key] == expected[key] for key in ("txid", "weight", "size", "order"))
        assert len(actual["inputs"]) <= 16 and len(actual["outputs"]) <= 16
    wide = txs[-1]
    inputs, outputs = [], []
    for offset in (0, 16, 32):
        result = get("transaction", wide["txid"] + "-" + snap["hash"], offset)
        actual = result["transaction"]
        info = result["block_info"]
        assert info["hash"] == snap["hash"] and info["tx_count"] == 72
        assert info["size"] == block["size"] and info["weight"] == block["weight"]
        for field in ("inputs", "outputs"):
            page = actual[field[:-1] + "_page"]
            assert page["total"] == 40 and page["returned"] == min(16, 40 - offset)
            assert page["omitted"] == 40 - len(actual[field])
        inputs.extend(actual["inputs"])
        outputs.extend(actual["outputs"])
    expected = block["transactions"][-1]
    for actual, wanted in zip(inputs, expected["inputs"], strict=True):
        assert all(actual[key] == wanted[key] for key in ("txid", "vout", "coinbase"))
    for actual, wanted in zip(outputs, expected["outputs"], strict=True):
        assert all(actual[key] == wanted[key] for key in ("vout", "value_sat", "script_hex"))
    active = list(reversed(list_path(snap["hash"], blocks)))
    expected_outputs = {}
    spends = {}
    balance = 0
    for height, blockhash in enumerate(active):
        for tx in blocks[blockhash]["transactions"]:
            for vin, item in enumerate(tx["inputs"]):
                if not item["coinbase"]:
                    spends[item["txid"], item["vout"]] = (tx["txid"], vin, blockhash)
            for output in tx["outputs"]:
                if output["script_hex"] == "51":
                    expected_outputs[tx["txid"], output["vout"]] = (output["value_sat"], blockhash, tx["order"] == 0 and height + 100 > 105)
    occurrences = pages("outputs", "outputs", 16, "51")
    assert len(occurrences) == len(expected_outputs)
    for occurrence in occurrences:
        output = occurrence["output"]
        key = occurrence["txid"], output["vout"]
        value, blockhash, immature = expected_outputs.pop(key)
        assert output["value_sat"] == value and occurrence["block"] == blockhash
        if key in spends:
            spender, vin, spend_block = spends[key]
            assert (output["status"], output["spender"], output["vin"], output["spend_block"]) == ("spent", spender, vin, spend_block)
        else:
            balance += value
            assert output["status"] == ("immature" if immature else "unspent")
    assert not expected_outputs and get("script", "51")["balance_sat"] == balance
    print(f"PASS {ticket} real responses: 109 blocks, 72 transactions, 40 exact inputs/outputs over three pages, {len(occurrences)} script outputs, exact spend links and balance {balance}", flush=True)


if __name__ == "__main__":
    check(sys.argv[1], sys.argv[2])
