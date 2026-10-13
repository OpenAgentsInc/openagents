"""Regenerate docs/inference/clef-tpu-data.md from synced JSON + ops logs.
usage: python3 make_doc.py DATA_DIR OPS_DIR OUT.md HEADER.md"""
import subprocess, sys, json, os
data, ops, out, header = sys.argv[1:5]
here = os.path.dirname(os.path.abspath(__file__))
summ = subprocess.run([sys.executable, os.path.join(here, "summarize.py"), data], capture_output=True, text=True).stdout
spend = subprocess.run([sys.executable, os.path.join(here, "spend.py"), os.path.join(ops, "states.jsonl")], capture_output=True, text=True).stdout
cap = ["| time (UTC) | zone | type | mode | result | EID |", "| --- | --- | --- | --- | --- | --- |"]
def kind(r):
    m = r["msg"]
    return ("created" if r["ok"] else "no more capacity" if "no more capacity" in m else "insufficient capacity" if "Insufficient" in m
            else "permission denied (accelerator type)" if "permission" in m else "quota 0" if "Quota" in m
            else "internal error" if "internal error" in m else "reservation not found" if "Reservation" in m else "duplicate name (attempt already in flight)" if "resourceName" in m else m[:60])
try:
    rows = [r for r in json.load(open(os.path.join(ops, "capacity-rows.json"))) if "coordinator" in r["ts"]]
    for l in open(os.path.join(ops, "attempts.jsonl")):
        d = json.loads(l)
        if " " in d["name"]: continue
        rows.append(dict(ts=d["ts"], zone=d["zone"], type=d["type"], mode=d["mode"], ok=d["rc"] == 0, msg=d["message"], eid=d["eid"]))
    # first wave of v5e-8 / v5p-8 spot creates: client killed by a shell bug, outcomes from the watcher
    for z in ("europe-west4-b", "us-east1-c", "us-south1-a", "us-west1-c", "us-west4-a"):
        rows.append(dict(ts="2026-10-13T00:23:45Z", zone=z, type="v5litepod-8", mode="spot", ok=True, msg="created (watcher)", eid=""))
    for z, ok in (("europe-west4-b", True), ("us-east5-a", True), ("us-central1-a", False)):
        rows.append(dict(ts="2026-10-13T00:23:45Z", zone=z, type="v5p-8", mode="spot", ok=ok, msg="created (watcher)" if ok else "failed (outcome not captured)", eid=""))
    for r in rows:
        r["k"] = kind(r); r["mode"] = r["mode"].replace("ondemand", "on-demand")
    import collections
    c = collections.Counter((r["type"], r["mode"], "created" if r["ok"] else "failed") for r in rows)
    cap.insert(0, "Totals: " + "; ".join(f"{t} {m}: {c[(t, m, 'created')]} created / {c[(t, m, 'failed')]} failed" for t, m in sorted({(k[0], k[1]) for k in c})) + "\n")
    for r in sorted(rows, key=lambda r: (r["type"], r["zone"], r["ts"])):
        cap.append(f"| {r['ts'][:16].replace('T', ' ')} | {r['zone']} | {r['type']} | {r['mode']} | {r['k']} | {r['eid']} |")
except Exception as e:
    cap.append(f"(capacity rows unavailable: {e})")
ev = ["| time (UTC) | VM / queued resource | zone | type | state |", "| --- | --- | --- | --- | --- |"]
for l in open(os.path.join(ops, "states.jsonl")):
    d = json.loads(l)
    if d["kind"] == "qr" or d.get("state") in ("READY", "PREEMPTED", "DELETING", "SUSPENDED") or d.get("health") == "UNHEALTHY_MAINTENANCE":
        if d["name"].startswith("p6-"): continue
        ev.append(f"| {d['ts'][:19].replace('T', ' ')} | {d['name']} | {d['zone']} | {d.get('type', 'queued')} | {d['state']}{(' / ' + d['health']) if d.get('health') else ''} |")
# cold start: create call -> READY, setup (venv + SGL-JAX install), engine start (load + precompile)
import glob
cold = ["| VM | type | create call s | setup s | engine starts (s): model/TP/quant | first prefill answer after process start (s) |", "| --- | --- | ---: | ---: | --- | ---: |"]
att = {}
for l in open(os.path.join(ops, "attempts.jsonl")):
    d = json.loads(l)
    if d["rc"] == 0: att[d["name"]] = d["seconds"]
ar = {}
try:
    for l in open(os.path.join(ops, "autorun.jsonl")):
        d = json.loads(l); ar[d["name"]] = d["setup_s"]
except Exception: pass
vmdirs = sorted(glob.glob(os.path.join(data, "c*-*")))
for vd in vmdirs:
    vm = os.path.basename(vd); starts = []; first = []
    for f in sorted(glob.glob(os.path.join(vd, "prefill-*.json"))):
        try: j = json.load(open(f))
        except Exception: continue
        e = j["env"]; starts.append(f"{e.get('model','').split('/')[-1]}/TP{e.get('tp')}/{e.get('quant')}/{e.get('load')}: {e.get('engine_start_s', 0):.0f}")
        if j.get("first_answer_since_process_start_s"): first.append(f"{j['first_answer_since_process_start_s']:.0f}")
    setup = ar.get(vm)
    if setup is None:
        try: setup = json.load(open(os.path.join(vd, "setup.json"))).get("setup_seconds")
        except Exception: setup = None
    cold.append(f"| {vm} | {vm.split('-')[1]} | {att.get(vm, '—')} | {setup if setup is not None else '—'} | {'; '.join(starts) or '—'} | {', '.join(first) or '—'} |")
# queued resources: filed -> provisioning -> VM READY -> preempted (per incarnation)
qrt = ["| queued resource | zone | filed (UTC) | wait to PROVISIONING | READY | VM lifetime until preemption | outcome |", "| --- | --- | --- | ---: | --- | ---: | --- |"]
import datetime as dt
def T(x): return dt.datetime.strptime(x[:19], "%Y-%m-%dT%H:%M:%S")
filed = []
for l in open(os.path.join(ops, "queued.jsonl")):
    d = json.loads(l)
    if d.get("rc") == 0: filed.append(d)
evs = [json.loads(l) for l in open(os.path.join(ops, "states.jsonl"))]
for i, f in enumerate(filed):
    t0 = T(f["ts"]); nxt = [g for g in filed[i + 1:] if g["qr"] == f["qr"]]
    t1 = T(nxt[0]["ts"]) if nxt else dt.datetime(2100, 1, 1)
    mine = [e for e in evs if e["name"] == f["qr"] and t0 <= T(e["ts"]) < t1]
    prov = next((e for e in mine if e["kind"] == "qr" and e["state"] == "PROVISIONING"), None)
    ready = next((e for e in mine if e["kind"] == "vm" and e["state"] == "READY"), None)
    gone = next((e for e in mine if (e["kind"] == "vm" and e["state"] in ("PREEMPTED", "DELETING")) or (e["kind"] == "qr" and e["state"] in ("SUSPENDING", "SUSPENDED"))), None)
    failed = next((e for e in mine if e["kind"] == "qr" and e["state"] == "FAILED"), None)
    w = f"{(T(prov['ts']) - t0).total_seconds() / 60:.0f} min" if prov else "—"
    life = f"{(T(gone['ts']) - T(ready['ts'])).total_seconds() / 60:.0f} min" if ready and gone and T(gone["ts"]) >= T(ready["ts"]) else "—"
    res = "failed" if failed else ("preempted" if gone and ready else ("suspended before READY" if gone else ("held" if ready else ("waiting" if any(e["kind"] == "qr" for e in mine) else "refused at filing"))))
    qrt.append(f"| {f['qr']} | {f['zone']} | {f['ts'][11:16]} | {w} | {ready['ts'][11:16] if ready else '—'} | {life} | {res} |")
body = open(header).read()
body += "\n" + summ + "\n## Spend and machines (this campaign)\n\n" + spend + "\n## Capacity log\n\nEvery create attempt from this campaign (and the coordinator's on-demand probe just before it). " \
        "`resourceName` rows are duplicate-name retries of an attempt already in flight.\n\n" + "\n".join(cap) + \
        "\n\n## Queued resources\n\nEach row is one filing (a preempted or failed spot request is deleted and filed again under the same name).\n\n" + "\n".join(qrt) + "\n\n## Cold start\n\nCreate call = the `gcloud ... create` call until the VM is READY (successful attempts only). Setup = uv + Python 3.12 venv + SGL-JAX + CPU torch on the stock image. Engine start = SGL-JAX `Engine()` (weights, dummy or real, plus precompiling the token buckets). Real-weight loads are the `auto` rows.\n\n" + "\n".join(cold) + \
        "\n\n## Lifecycle events (watcher, 30 s poll)\n\n" + "\n".join(ev) + "\n"
open(out, "w").write(body)
print("wrote", out, len(body))
