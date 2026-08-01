#!/usr/bin/env python3
"""
Coldcard sweep-fingerprint scanner — OpenAgents independent implementation.

Recomputes the transaction-builder fingerprint from raw chain data on our own
archival node. Does not import or copy the reference tool's code or its
generated data; the arithmetic is reimplemented from the documented rule so a
disagreement is meaningful.

Fingerprint (all three must hold):
  1. fee is an exact whole number of sat per vbyte of an ESTIMATED size
       est = 42 + sum(per-input vbytes by prevout script type)
       P2WPKH 68 | P2SH 91 | P2PKH 148
  2. nLockTime == 0
  3. (recorded, not required) share of 33-byte-R signatures

Exact integer satoshi arithmetic throughout: amounts parsed as Decimal from
raw JSON, never float.

Outputs (append-only, resumable):
  cc-hits.jsonl    one JSON line per match
  cc-done.txt      completed block heights + block hash checkpoints
  cc-stats.jsonl   per-block eligibility counts (base-rate denominator)
"""
import json, os, subprocess, sys, time
from decimal import Decimal

DATADIR = "/var/lib/bitcoin"
CONF = "/etc/bitcoin/bitcoin.conf"
OUT = os.environ.get("CC_OUT", "/var/tmp/ccscan")

# per-input vbyte estimate by prevout script type (the documented table)
SZ = {"witness_v0_keyhash": 68, "scripthash": 91, "pubkeyhash": 148}
OVERHEAD = 42
PLAUSIBLE_RATES = range(1, 1001)

SELFTEST = [
    ("44e622cb32d9e65a99b727f87744ac6d0a5d0c1f4cfdd8af399771502e7ad3f3", 960359, 2),
    ("04ebdc774df620036326142769e7ab79d3af33fc52537daf88daeece225e50ed", 960367, 2),
    ("528e31f9279d3c938d7caf36b8760628d394c334e85371c843b213387c0ed08d", 960367, 2),
    ("10f1fefdfa767d3bf191a0f67cd8374171b3f45f532348d1ac0b5f1247145931", 960367, 2),
    ("d317749b5734a5d484e9d97f3df1067945a09ea52abcc3f3284a0c88bde9b848", 960367, 2),
    ("ac2bcaced5ea0cab2c30fbff5f96d90958cbe8453f5811af35fe42dce02958bb", 960367, 2),
    ("5f2231199a86a3ebb07187a568ecdd0f57973aec71bf5a0c0da974f650bce17c", 960367, 2),
    ("2fe075cf0ec799f3529ed6a28e0a08b45fe1fc9bd93c3f33bdbc42d5bff4f736", 960189, 30),
]


def cli(*args):
    p = subprocess.run(["bitcoin-cli", "-conf=" + CONF, "-datadir=" + DATADIR, *args],
                       capture_output=True, text=True)
    if p.returncode != 0:
        raise RuntimeError(p.stderr.strip()[:300])
    s = p.stdout.strip()
    try:
        return json.loads(s, parse_float=Decimal)
    except json.JSONDecodeError:
        return s          # bitcoin-cli prints bare scalars (e.g. getblockhash)


def sats(v):
    """Decimal BTC -> exact integer satoshis."""
    return int((Decimal(v) * 100000000).to_integral_value())


def examine(tx):
    """Return a hit dict if the tx matches, else None. Raises on missing data."""
    if any("coinbase" in vin for vin in tx["vin"]):
        return None
    if tx["locktime"] != 0:
        return None

    est = OVERHEAD
    in_total = 0
    types = {}
    for vin in tx["vin"]:
        pv = vin.get("prevout")
        if pv is None:
            raise KeyError("missing prevout")           # fail loudly, never silently skip
        t = pv["scriptPubKey"]["type"]
        if t not in SZ:
            return None                                 # off-table script type
        types[t] = types.get(t, 0) + 1
        est += SZ[t]
        in_total += sats(pv["value"])

    out_total = sum(sats(o["value"]) for o in tx["vout"])
    fee = in_total - out_total
    if fee <= 0 or est <= 0:
        return None
    if fee % est != 0:                                  # THE discriminating test
        return None
    rate = fee // est
    if rate not in PLAUSIBLE_RATES:
        return None

    # supporting mark: 33-byte R share (72-byte DER sig)
    hi_r = lo_r = 0
    for vin in tx["vin"]:
        w = vin.get("txinwitness") or []
        if w:
            n = len(w[0]) // 2
            if n == 72:
                hi_r += 1
            elif n == 71:
                lo_r += 1
    return {
        "txid": tx["txid"], "rate": rate, "fee": fee, "est_vsize": est,
        "real_vsize": tx["vsize"], "overshoot": est - tx["vsize"],
        "n_in": len(tx["vin"]), "n_out": len(tx["vout"]),
        "in_types": types, "in_total": in_total, "out_total": out_total,
        "hi_r": hi_r, "lo_r": lo_r,
    }


def scan_block(h):
    bh = cli("getblockhash", str(h))
    blk = cli("getblock", bh, "3")
    hits, eligible, errs = [], 0, 0
    for tx in blk["tx"]:
        if any("coinbase" in v for v in tx["vin"]):
            continue
        eligible += 1
        try:
            r = examine(tx)
        except KeyError:
            errs += 1
            continue
        if r:
            r["height"] = h
            r["blockhash"] = bh
            r["time"] = blk["time"]
            hits.append(r)
    return bh, hits, eligible, errs, blk["time"]


def selftest():
    print("SELF-TEST — 8 known positives", flush=True)
    blocks, ok = {}, True
    for txid, h, want in SELFTEST:
        if h not in blocks:
            blocks[h] = cli("getblock", cli("getblockhash", str(h)), "3")
        tx = next((t for t in blocks[h]["tx"] if t.get("txid") == txid), None)
        if tx is None:
            print("  FAIL %s not in block %d" % (txid[:16], h)); ok = False; continue
        r = examine(tx)
        got = r["rate"] if r else None
        mark = "PASS" if got == want else "FAIL"
        if got != want:
            ok = False
        ovr = r["overshoot"] if r else "-"
        print("  %s %s block %d rate=%s want=%d overshoot=%s" %
              (mark, txid[:16], h, got, want, ovr), flush=True)
    print("SELF-TEST %s\n" % ("PASSED" if ok else "FAILED"), flush=True)
    return ok


def main():
    os.makedirs(OUT, exist_ok=True)
    lo, hi = int(sys.argv[1]), int(sys.argv[2])
    tag = sys.argv[3] if len(sys.argv) > 3 else "scan"

    if os.environ.get("CC_SELFTEST", "1") == "1":
        if not selftest():
            print("refusing wide scan: self-test failed"); sys.exit(1)
    if os.environ.get("CC_SELFTEST_ONLY") == "1":
        return

    donef = os.path.join(OUT, "cc-done-%s.txt" % tag)
    done = set()
    if os.path.exists(donef):
        done = {int(l.split()[0]) for l in open(donef) if l.strip()}

    hitf = open(os.path.join(OUT, "cc-hits-%s.jsonl" % tag), "a")
    statf = open(os.path.join(OUT, "cc-stats-%s.jsonl" % tag), "a")
    donefh = open(donef, "a")

    t0 = time.time()
    tot_hits = tot_elig = tot_err = 0
    for h in range(lo, hi + 1):
        if h in done:
            continue
        try:
            bh, hits, elig, errs, bt = scan_block(h)
        except Exception as e:
            print("ERROR block %d: %s" % (h, e), flush=True)
            continue
        for r in hits:
            hitf.write(json.dumps(r) + "\n")
        statf.write(json.dumps({"height": h, "hash": bh, "eligible": elig,
                                "hits": len(hits), "prevout_errors": errs,
                                "time": bt}) + "\n")
        donefh.write("%d %s\n" % (h, bh))
        hitf.flush(); statf.flush(); donefh.flush()
        tot_hits += len(hits); tot_elig += elig; tot_err += errs
        if (h - lo) % 25 == 0:
            el = time.time() - t0
            print("  block %d  hits=%d elig=%d  %.1fs  %.2f blk/s" %
                  (h, tot_hits, tot_elig, el, (h - lo + 1) / max(el, .001)), flush=True)
    print("DONE %s: %d hits / %d eligible tx / %d prevout errors" %
          (tag, tot_hits, tot_elig, tot_err), flush=True)


if __name__ == "__main__":
    main()
