import json,sys,random,statistics as st,glob,collections
rows=[json.loads(l) for f in sys.argv[1:] for l in open(f)]
d=collections.defaultdict(lambda: collections.defaultdict(list))
for r in rows: d[r["task"]][r["arm"]].append(r)
R=random.Random(1)
def ci(a,b,key):
  xs=[]
  for _ in range(4000):
    A=[R.choice(a) for _ in a]; B=[R.choice(b) for _ in b]
    xs.append(st.mean(x[key] for x in B)/st.mean(x[key] for x in A))
  xs.sort(); return xs[int(.025*len(xs))], xs[int(.975*len(xs))]
tasks=sorted(d)
for t in tasks+["all"]:
  a=[r for r in rows if r["arm"]=="raw-claude" and (t=="all" or r["task"]==t)]
  b=[r for r in rows if r["arm"]=="routed-lean" and (t=="all" or r["task"]==t)]
  if t=="all":
    # ratio of sum of per-task means, bootstrap within task
    def stat(A,B,key): return sum(st.mean(x[key] for x in B[k]) for k in tasks)/sum(st.mean(x[key] for x in A[k]) for k in tasks)
    A0={k:[r for r in a if r["task"]==k] for k in tasks}; B0={k:[r for r in b if r["task"]==k] for k in tasks}
    out=[]
    for key in ("wall_s","cost_usd"):
      xs=sorted(stat({k:[R.choice(v) for _ in v] for k,v in A0.items()},{k:[R.choice(v) for _ in v] for k,v in B0.items()},key) for _ in range(4000))
      out.append((stat(A0,B0,key),xs[100],xs[3899]))
    tw,cw=out
  else:
    tw=(st.mean(x["wall_s"] for x in b)/st.mean(x["wall_s"] for x in a),)+ci(a,b,"wall_s")
    cw=(st.mean(x["cost_usd"] for x in b)/st.mean(x["cost_usd"] for x in a),)+ci(a,b,"cost_usd")
  print(f"{t:24} n={len(a)}/{len(b)} pass {sum(x['passed'] for x in b)}/{len(b)} vs {sum(x['passed'] for x in a)}/{len(a)}  time {tw[0]:.2f} ({tw[1]:.2f}-{tw[2]:.2f})  cost {cw[0]:.2f} ({cw[1]:.2f}-{cw[2]:.2f})  raw {st.mean(x['wall_s'] for x in a):.0f}s lean {st.mean(x['wall_s'] for x in b):.0f}s")
