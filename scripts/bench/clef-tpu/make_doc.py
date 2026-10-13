"""Regenerate docs/inference/clef-tpu-data.md from synced JSON + ops logs.
usage: python3 make_doc.py DATA_DIR OPS_DIR OUT.md HEADER.md"""
import subprocess, sys, json, os
data, ops, out, header = sys.argv[1:5]
here = os.path.dirname(os.path.abspath(__file__))
summ = subprocess.run([sys.executable, os.path.join(here, "summarize.py"), data], capture_output=True, text=True).stdout
spend = subprocess.run([sys.executable, os.path.join(here, "spend.py"), os.path.join(ops, "states.jsonl")], capture_output=True, text=True).stdout
cap = ["| time (UTC) | zone | type | mode | result | EID |", "| --- | --- | --- | --- | --- | --- |"]
try:
    rows = json.load(open(os.path.join(ops, "capacity-rows.json")))
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
body = open(header).read()
body += "\n" + summ + "\n## Spend and machines (this campaign)\n\n" + spend + "\n## Capacity log\n\nEvery create attempt from this campaign (and the coordinator's on-demand probe just before it). " \
        "`resourceName` rows are duplicate-name retries of an attempt already in flight.\n\n" + "\n".join(cap) + \
        "\n\n## Lifecycle events (watcher, 30 s poll)\n\n" + "\n".join(ev) + "\n"
open(out, "w").write(body)
print("wrote", out, len(body))
