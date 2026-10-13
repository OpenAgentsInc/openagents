"""Spend so far from the watcher's state log: each VM bills from READY until it leaves
READY (DELETING / PREEMPTED / gone) or now, at the catalog spot/on-demand price of its region.
usage: python3 spend.py ops/states.jsonl [--held]"""
import json, sys, os, datetime as dt
P = json.load(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "prices.json")))
FAM = {"v6e": "v6e", "v5litepod": "v5e", "v5p": "v5p"}
def chips(t):
    n = int(t.split("-")[-1]); return n // 2 if t.startswith("v5p") else n
def price(t, zone, spot):
    fam = FAM[t.split("-")[0]]; reg = zone.rsplit("-", 1)[0]
    return P[fam][reg]["spot" if spot else "ondemand"]
ts = lambda s: dt.datetime.strptime(s, "%Y-%m-%dT%H:%M:%SZ")
now = dt.datetime.utcnow()
vms = {}
for l in open(sys.argv[1]):
    d = json.loads(l)
    if d["kind"] != "vm" or not d["name"].startswith(("cd-", "cq-")): continue
    v = vms.setdefault(d["name"], {"zone": d["zone"], "type": d["type"], "spot": d.get("spot") == "True", "start": None, "end": None, "last": None, "done_h": 0.0})
    if d["state"] == "READY" and (v["start"] is None or v["end"] is not None):
        if v["end"] is not None: v["done_h"] += (v["end"] - v["start"]).total_seconds() / 3600  # a new incarnation (queued resource re-provisioned)
        v["start"] = ts(d["ts"]); v["end"] = None
    if d["state"] in ("DELETING", "PREEMPTED", "TERMINATED", "CREATING") and v["start"] and not v["end"]: v["end"] = ts(d["ts"])
    v["last"] = d["state"]
total = 0; rate = 0; rows = []
for n, v in sorted(vms.items()):
    if not v["start"]: continue
    end = v["end"] or now; h = (end - v["start"]).total_seconds() / 3600 + v["done_h"]
    p = price(v["type"], v["zone"], v["spot"]) * chips(v["type"])
    cost = h * p; total += cost
    held = v["end"] is None
    if held: rate += p
    rows.append((n, v["zone"], v["type"], "spot" if v["spot"] else "on-demand", p, h, cost, v["last"], held))
print("| VM | zone | type | mode | $/h | hours | $ so far | last state |"); print("| --- | --- | --- | --- | ---: | ---: | ---: | --- |")
for r in rows: print(f"| {r[0]} | {r[1]} | {r[2]} | {r[3]} | {r[4]:.2f} | {r[5]:.2f} | {r[6]:.2f} | {r[7]}{' (held)' if r[8] else ''} |")
print(f"\nTotal so far ${total:.2f}; current burn ${rate:.2f}/h = ${rate*24:.0f}/day (this campaign only)")
