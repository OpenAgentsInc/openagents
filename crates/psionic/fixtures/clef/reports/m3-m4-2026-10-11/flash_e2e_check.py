import sys,json,math,time,urllib.request,signal
from pathlib import Path
sys.path.insert(0,str(Path.cwd()/"crates/psionic/fixtures/clef/tools"))
import mac_quiet_bench as bench
import e2e
root=Path(__file__).parent
config=bench.load_config(root/"flash-latency-config.json")
output=root/"flash-e2e-stage2"
output.mkdir(exist_ok=False)
fixtures=Path("crates/psionic/fixtures/clef/e2e")
requests=[line for line in (fixtures/"requests.jsonl").read_text().split("\n") if line]
references=[json.loads(line) for line in (fixtures/"psionic-cpu-q4_k_m.jsonl").read_text().split("\n") if line]
assert len(requests)==len(references)==40
host=bench.Host(config,output);host.thread.start()
summary={"conditions":"shared machine; correctness only","binary_sha256":bench.file_sha(config["variants"][0]["binary"]),"runs":[]}
def interrupt(signum,frame): raise KeyboardInterrupt
signal.signal(signal.SIGTERM,interrupt)
try:
 host.settle()
 with (output/"records.jsonl").open("w") as records:
  for variant in config["variants"]:
   maximum=0.;questions=agree=0;rows=[]
   with bench.server(config,variant,0,output,records,host) as url:
    for index,(raw,reference) in enumerate(zip(requests,references)):
     host.guard();start=time.monotonic()
     request=urllib.request.Request(url,raw.encode(),headers={"Content-Type":"application/json"})
     with bench.HTTP.open(request,timeout=300) as response: code,body=response.status,json.load(response)
     bench.validate_uncached_response(code,body)
     row={"index":index,"status":code,"seconds":time.monotonic()-start,"body":body}
     rows.append(row);bench.emit(records,{"variant":variant["name"],**row})
     expected=reference["body"]
     assert reference["status"]==code==200
     assert body["psionic"]["backend"]=="metal"
     assert body["usage"]["input_tokens"]==expected["usage"]["input_tokens"]
     for field in ("artifact_digest","head_digest"):
      assert body["psionic"][field]==expected["psionic"][field]
     assert set(body["answers"])==set(expected["answers"])
     for key,a in body["answers"].items():
      b=expected["answers"][key]
      assert a["type"]==b["type"]
      ap,bp=e2e.probabilities(a),e2e.probabilities(b)
      assert set(ap)==set(bp) and ap
      assert all(math.isfinite(p) and 0<=p<=1 for p in ap.values())
      assert abs(sum(ap.values())-1)<1e-6
      maximum=max(maximum,max(abs(ap[k]-bp[k]) for k in ap))
      questions+=1;agree+=e2e.top(a)==e2e.top(b)
     print(variant["name"],index,code,round(row["seconds"],3),flush=True)
   (output/(variant["name"]+".jsonl")).write_text("".join(json.dumps(r)+"\n" for r in rows))
   result={"variant":variant,"requests":len(rows),"questions":questions,"top_agree":agree,"max_abs_dp":maximum,"gate":questions>0 and agree==questions and maximum<=.02}
   summary["runs"].append(result)
   (output/"summary.json").write_text(json.dumps(summary,indent=2)+"\n")
finally:
 host.stop.set();host.thread.join(timeout=6)
print(json.dumps(summary,indent=2),flush=True)
raise SystemExit(0 if all(row["gate"] for row in summary["runs"]) and len(summary["runs"])==len(config["variants"]) else 1)
