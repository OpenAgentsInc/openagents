# Firm reason: 2 (independent raw-block and retained Core/Electrs oracle for HTTP projections).
"""Compare the loopback adapter with authenticated raw bytes, without Coax codecs."""
import json
import sys
import urllib.error
import urllib.parse
import urllib.request

from oracle import FIXTURE, list_path, read_blocks


def check(base):
    raw = read_blocks()
    ticket = 0
    requests = 0

    def get(route, source="fixture", phase=0, cursor="latest", key=None, offset=0, status=200):
        nonlocal ticket, requests
        ticket += 1
        query = dict(source=source, phase=str(phase), ticket=str(ticket), cursor=cursor, offset=str(offset))
        if source == "own-node":
            del query["phase"]
        if key is not None:
            query["key"] = key
        req = urllib.request.Request(base + "/api/lab/v1/" + route + "?" + urllib.parse.urlencode(query))
        try:
            response = urllib.request.urlopen(req, timeout=15)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            assert response.status == status, (route, key, response.status, status)
            assert response.headers["Cache-Control"] == "no-store"
            data = response.read(2097153)
            assert len(data) <= 2097152
        requests += 1
        result = json.loads(data)
        if status == 200:
            assert result["ticket"] == str(ticket)
            sn = result["snapshot"]
            assert sn["source"] == source and sn["profile"] == "validated-v1"
            assert sn["mode"] == ("fixture" if source == "fixture" else "live")
            assert sn["observed_ms"] is None and sn["background_height"] is None
            assert sn["available"] and sn["complete"] and sn["validated_height"] == sn["height"]
            if cursor != "latest":
                assert sn["cursor"] == cursor
        return result

    def pages(route, field, cap, **kwargs):
        result = []
        offset = 0
        while True:
            answer = get(route, offset=offset, **kwargs)
            page = answer["page"]
            rows = answer[field]
            assert page["returned"] == len(rows) <= cap
            assert page["omitted"] == page["total"] - len(rows)
            result.extend(rows)
            offset += len(rows)
            assert page["next"] == (answer["snapshot"]["cursor"] + "~" + str(offset) if offset < page["total"] else "")
            if not page["next"]:
                assert len(result) == page["total"]
                return result

    for source, phases in (("fixture", range(3)), ("own-node", (2,))):
        for phase in phases:
            ref = json.loads((FIXTURE / f"reference-{'abc'[phase]}.json").read_text())
            snapshot = get("snapshot", source=source, phase=phase)["snapshot"]
            assert snapshot["hash"] == ref["snapshot"]["hash"]
            cursor = snapshot["cursor"]
            active_path = list(reversed(list_path(snapshot["hash"], raw)))
            active = set(active_path)
            heights = {h: i for i, h in enumerate(active_path)}
            blocks = pages("window", "blocks", 24, source=source, phase=phase)
            assert len(blocks) == (103, 106, 108)[phase]
            assert len({b["hash"] for b in blocks}) == len(blocks)
            if source == "own-node":
                assert blocks[0]["height"] == 0
            for block in blocks:
                expected = raw[block["hash"]]
                for field in ("hash", "previous", "time", "size", "weight"):
                    assert block[field] == expected[field], (source, phase, field)
                assert block["active"] == (block["hash"] in active)
                assert block["subsidy_sat"] == 5000000000 >> (block["height"] // 150)
                txs = pages("transactions", "transactions", 32, source=source, phase=phase, cursor=cursor, key=block["hash"])
                for tx, expected_tx in zip(txs, expected["transactions"], strict=True):
                    for field in ("txid", "size", "weight", "order"):
                        assert tx[field] == expected_tx[field]
                    assert tx["block"] == block["hash"] and tx["active"] == block["active"]
                    exact = get("transaction", source=source, phase=phase, cursor=cursor, key=tx["txid"] + "-" + block["hash"])["transaction"]
                    assert exact == tx
                    for output, expected_out in zip(tx["outputs"], expected_tx["outputs"], strict=True):
                        for field in ("vout", "value_sat", "script_hex"):
                            assert output[field] == expected_out[field]
                        if not block["active"]:
                            assert output["status"] == "stale"
                        elif tx["order"] == 0 and block["height"] and block["height"] + 100 > snapshot["height"]:
                            assert output["status"] in ("immature", "spent", "unspendable")
            # Independent exact-script balances retained from both reference implementations.
            for address in ref["addresses"]:
                script = address["script_hex"]
                history = pages("script", "transactions", 32, source=source, phase=phase, cursor=cursor, key=script)
                outputs = pages("outputs", "outputs", 16, source=source, phase=phase, cursor=cursor, key=script)
                answer = get("script", source=source, phase=phase, cursor=cursor, key=script)
                assert answer["balance_sat"] == address["balance_sat"]
                assert len({(t["txid"], t["block"]) for t in history}) == len(history)
                for occurrence in outputs:
                    original = next(t for t in raw[occurrence["block"]]["transactions"] if t["txid"] == occurrence["txid"])
                    output = occurrence["output"]
                    expected = original["outputs"][output["vout"]]
                    assert output["script_hex"] == script and output["value_sat"] == expected["value_sat"]
            get("window", source=source, phase=phase, cursor="wrong", status=409)
            print(f"PASS {source} {'abc'[phase]}: {len(blocks)} blocks, raw transactions/weights/outputs, exact occurrences, paged script histories and Core/Electrs balances")
    print(f"PASS {requests} bounded no-store HTTP responses; no Core/Electrs process required")


if __name__ == "__main__":
    check(sys.argv[1])
