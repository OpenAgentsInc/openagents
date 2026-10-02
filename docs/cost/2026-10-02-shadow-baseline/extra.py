import json,glob,os,collections,statistics
rows=[json.loads(l) for l in open("collected.jsonl")]
agg=collections.defaultdict(collections.Counter)
for r in rows:
    a=agg[r["arm"]]
    for k in ("input_tokens","cache_read","cache_write","output_tokens"):
        a[k]+= r.get(k) or 0
    a["engine"]+= r.get("engine_usd") or 0
    a["jev"]+= r.get("jev_usd") or 0
for arm,a in sorted(agg.items()):
    print(arm, "in",a["input_tokens"], "cache_read %.0f%%"%(100*a["cache_read"]/max(1,a["input_tokens"])), "cache_write %.0f%%"%(100*a["cache_write"]/max(1,a["input_tokens"])), "out",a["output_tokens"], "engine $%.2f jev $%.4f"%(a["engine"],a["jev"]))
def rd(r): return "runs/%s/%s/%s" % (r["task"], r["arm"], r["trial"])
print("FAILS")
for r in rows:
    if not r.get("passed"):
        d=json.load(open(rd(r)+"/result.json"))
        print(r["arm"], r["task"], r["trial"], r.get("ending"), d.get("check_detail","")[-250:].replace("\n"," "))
print("ESCAPES")
for r in rows:
    if not r["arm"].startswith("routed"): continue
    d=rd(r); repo=os.path.abspath(d+"/repo"); n=0
    for l in open(d+"/out.ndjson"):
        try: e=json.loads(l)
        except Exception: continue
        if e.get("event")=="step" and e.get("kind")=="command" and repo in e.get("text","") and "worktrees" not in e.get("text",""): n+=1
    if n: print("escape", r["arm"], r["task"], r["trial"], n)
for arm in ["routed-claude-on","routed-claude-off","routed-codex-on","routed-codex-off"]:
    rs=[r for r in rows if r["arm"]==arm and r.get("route_wall_s")]
    print(arm, "median e2e-minus-run s %.1f" % statistics.median([r["wall_s"]-r["route_wall_s"] for r in rs]), "median steps", statistics.median([r.get("steps") or 0 for r in rs]))
rs=[r for r in rows if r["arm"]=="raw-claude"]
print("raw models", collections.Counter(tuple(r.get("models") or []) for r in rs), "median turns", statistics.median([r.get("turns") or 0 for r in rs]))
