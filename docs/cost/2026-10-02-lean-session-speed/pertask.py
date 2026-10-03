import json,sys,collections,statistics as st
rows=[json.loads(l) for f in sys.argv[1:] for l in open(f)]
d=collections.defaultdict(lambda: collections.defaultdict(list))
for r in rows: d[r["task"]][r["arm"]].append(r)
tot=collections.Counter()
print(f"{'task':24}{'raw s':>8}{'lean s':>8}{'t':>6}{'raw $':>8}{'lean $':>8}{'c':>6}  raw turns / lean turns  pass")
for t in sorted(d):
  a=d[t]
  if "raw-claude" not in a or "routed-lean" not in a: continue
  rw=st.mean(x["wall_s"] for x in a["raw-claude"]); lw=st.mean(x["wall_s"] for x in a["routed-lean"])
  rc=st.mean(x["cost_usd"] for x in a["raw-claude"]); lc=st.mean(x["cost_usd"] for x in a["routed-lean"])
  tot["rw"]+=rw; tot["lw"]+=lw; tot["rc"]+=rc; tot["lc"]+=lc
  print(f"{t:24}{rw:8.0f}{lw:8.0f}{lw/rw:6.2f}{rc:8.3f}{lc:8.3f}{lc/rc:6.2f}  {[x.get('turns') for x in a['raw-claude']]} / {[x.get('steps') for x in a['routed-lean']]}  {sum(x['passed'] for x in a['raw-claude'])}/{len(a['raw-claude'])} {sum(x['passed'] for x in a['routed-lean'])}/{len(a['routed-lean'])}")
print(f"{'sum of means':24}{tot['rw']:8.0f}{tot['lw']:8.0f}{tot['lw']/tot['rw']:6.2f}{tot['rc']:8.3f}{tot['lc']:8.3f}{tot['lc']/tot['rc']:6.2f}")
