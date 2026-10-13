"""Budget guard: every 2 min, total the hourly list price of every READY/CREATING TPU VM in the
project (ours plus anything else, e.g. the P1 worker) and, while it exceeds CAP_PER_DAY/24,
release our least useful VM: v5e first, then v5p, then our v6e-1s, then extra v6e-4/-8 beyond
one of each (never another agent's VM). Syncs the VM's results to GCS before deleting. Logs to ops/guard.jsonl."""
import json, os, subprocess, time, datetime as dt
HERE = os.path.dirname(os.path.abspath(__file__))
P = json.load(open(os.path.join(HERE, "prices.json")))
CAP = float(os.environ.get("CAP_PER_DAY", "500")) / 24
LOG = os.path.join(os.environ.get("CLEF_TPU_LOGDIR", "ops"), "guard.jsonl")
ENV = dict(os.environ, CLOUDSDK_CONFIG=os.environ.get("CLOUDSDK_CONFIG", "/Users/christopherdavid/work/.secrets/gcloud-sa-config"))
FAM = {"v6e": "v6e", "v5litepod": "v5e", "v5p": "v5p"}
ORDER = {"v5e": 0, "v5p": 1, "v6e1": 2}
def vms():
    out = subprocess.run(["gcloud", "compute", "tpus", "tpu-vm", "list", "--zone=-", "--project", "openagentsgemini",
        "--format=json(name,acceleratorType,state,schedulingConfig)"], capture_output=True, text=True, env=ENV).stdout
    res = []
    for v in json.loads(out or "[]"):
        name = v["name"].split("/")[-1]; zone = v["name"].split("/")[3]; t = v["acceleratorType"].split("/")[-1]
        if v.get("state") != "READY": continue  # CREATING attempts often fail; count only billing VMs
        n = int(t.split("-")[-1]); chips = n // 2 if t.startswith("v5p") else n
        spot = bool((v.get("schedulingConfig") or {}).get("spot") or (v.get("schedulingConfig") or {}).get("preemptible"))
        fam = FAM[t.split("-")[0]]
        price = P[fam][zone.rsplit("-", 1)[0]]["spot" if spot else "ondemand"] * chips
        res.append(dict(name=name, zone=zone, type=t, fam=fam, chips=chips, price=price))
    return res
while True:
    v = vms(); rate = sum(x["price"] for x in v)
    rec = {"ts": dt.datetime.utcnow().strftime("%FT%TZ"), "rate_h": round(rate, 2), "day": round(rate * 24), "n": len(v)}
    if rate > CAP:
        ours = [x for x in v if x["name"].startswith(("cd-", "cq-"))]
        # release order: v5e, v5p, our v6e-1s, then any second-or-later v6e-4 / v6e-8 (keep one of each)
        cand = sorted([x for x in ours if x["fam"] in ("v5e", "v5p")], key=lambda x: (ORDER[x["fam"]], -x["price"]))
        cand += sorted([x for x in ours if x["type"] == "v6e-1"], key=lambda x: -x["price"])
        for t in ("v6e-4", "v6e-8"):
            cand += sorted([x for x in ours if x["type"] == t], key=lambda x: -x["price"])[:-1]
        if cand:
            x = cand[0]
            subprocess.run([os.path.join(HERE, "vm.sh"), "sync", x["name"], x["zone"]], env=dict(ENV, DATE="2026-10-13"), capture_output=True, timeout=180)
            subprocess.run(["gcloud", "compute", "tpus", "tpu-vm", "delete", x["name"], "--zone", x["zone"], "--project", "openagentsgemini", "--quiet", "--async"], env=ENV, capture_output=True)
            rec["released"] = x
    open(LOG, "a").write(json.dumps(rec) + "\n")
    time.sleep(120)
