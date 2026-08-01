#!/usr/bin/env python3
"""Isolate the operator signal from the base-rate noise.

The wide scan proved the exact-integer test alone is far too weak. This
narrows on the discriminators that actually separate the documented sweeps
from ordinary traffic, and reports what survives.
"""
import json, glob
from collections import Counter, defaultdict

KNOWN = set(range(960183, 960192)) | {960359, 960367}
hits, stats = [], []
for f in glob.glob("/var/tmp/ccscan/cc-hits-*.jsonl"):
    t = f.split("cc-hits-")[1][:-6]
    for l in open(f):
        h = json.loads(l); h["tag"] = t; hits.append(h)
for f in glob.glob("/var/tmp/ccscan/cc-stats-*.jsonl"):
    t = f.split("cc-stats-")[1][:-6]
    for l in open(f):
        s = json.loads(l); s["tag"] = t; stats.append(s)

elig_by_tag = Counter()
for s in stats:
    elig_by_tag[s["tag"]] += s["eligible"]

print("=" * 78)
print("A. FALSE-POSITIVE RATE BY ERA AND RATE  (the unpublished denominator)")
print("=" * 78)
print("   %-12s %10s %12s %12s %12s" % ("era", "eligible", "rate=2 /1M", "rate=30 /1M", "any-int /1M"))
for tag in sorted(elig_by_tag):
    e = elig_by_tag[tag]
    sub = [h for h in hits if h["tag"] == tag]
    r2 = sum(1 for h in sub if h["rate"] == 2)
    r30 = sum(1 for h in sub if h["rate"] == 30)
    print("   %-12s %10d %12.0f %12.0f %12.0f" %
          (tag, e, 1e6 * r2 / e, 1e6 * r30 / e, 1e6 * len(sub) / e))

# The documented sweeps are consolidations: many inputs, one output, real value.
def sweepish(h, minin=2, minval=1_000_000):
    return (h["n_out"] == 1 and list(h["in_types"]) == ["witness_v0_keyhash"]
            and h["lo_r"] == 0 and h["n_in"] >= minin and h["in_total"] >= minval)

print()
print("=" * 78)
print("B. RATE-30 CANDIDATES WITH CONSOLIDATION SHAPE AND >=0.01 BTC")
print("   (>=2 inputs, 1 output, all-P2WPKH, no low-R grinding)")
print("=" * 78)
r30 = [h for h in hits if h["rate"] == 30 and sweepish(h)]
byblk = defaultdict(list)
for h in r30:
    byblk[h["height"]].append(h)
print("   surviving transactions: %d in %d blocks" % (len(r30), len(byblk)))
print()
print("   %-9s %4s %14s %7s  %s" % ("height", "n", "BTC", "maxIn", "status"))
for ht in sorted(byblk):
    g = byblk[ht]
    btc = sum(x["in_total"] for x in g) / 1e8
    mx = max(x["n_in"] for x in g)
    st = "DOCUMENTED WAVE" if ht in KNOWN else ""
    print("   %-9d %4d %14.8f %7d  %s" % (ht, len(g), btc, mx, st))

print()
print("=" * 78)
print("C. SAME FILTER, THE 31-JULY RATE (2 sat/vB), INCIDENT ERA ONLY")
print("=" * 78)
r2 = [h for h in hits if h["rate"] == 2 and sweepish(h, minin=2, minval=10_000_000)
      and h["height"] >= 959200]
byblk2 = defaultdict(list)
for h in r2:
    byblk2[h["height"]].append(h)
print("   >=0.1 BTC consolidations at 2 sat/vB: %d tx in %d blocks" % (len(r2), len(byblk2)))
for ht in sorted(byblk2):
    g = byblk2[ht]
    btc = sum(x["in_total"] for x in g) / 1e8
    st = "DOCUMENTED WAVE" if ht in KNOWN else ""
    print("   %-9d %4d %14.8f  %s" % (ht, len(g), btc, st))

print()
print("=" * 78)
print("D. OVERSHOOT, STRATIFIED  (published claim: estimate ALWAYS exceeds real)")
print("=" * 78)
for label, sub in [("documented-wave tx", [h for h in hits if h["height"] in KNOWN and sweepish(h, 1, 0)]),
                   ("all rate-30 sweepish", r30),
                   ("2025 control era", [h for h in hits if h["tag"] == "control2025" and sweepish(h, 1, 0)])]:
    c = Counter("exact" if h["overshoot"] == 0 else ("over" if h["overshoot"] > 0 else "under") for h in sub)
    print("   %-22s n=%-6d %s" % (label, len(sub), dict(c)))

print()
print("=" * 78)
print("E. LARGEST UNDOCUMENTED CANDIDATES, INCIDENT ERA, EITHER OPERATOR RATE")
print("=" * 78)
cand = [h for h in hits if h["height"] >= 959200 and h["height"] not in KNOWN
        and h["rate"] in (2, 30) and sweepish(h, minin=3, minval=50_000_000)]
for h in sorted(cand, key=lambda x: -x["in_total"])[:30]:
    print("   %s h=%d r=%d in=%3d %14.8f BTC" %
          (h["txid"][:24], h["height"], h["rate"], h["n_in"], h["in_total"] / 1e8))
print("   (%d candidates >=0.5 BTC)" % len(cand))
