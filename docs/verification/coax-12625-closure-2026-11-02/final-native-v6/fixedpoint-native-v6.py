# Firm reason: 2 (independent exact C/header compiler fixedpoint oracle).
import concurrent.futures,hashlib,json,signal,subprocess,os,time
from pathlib import Path
ROOT=Path("/tmp/openagents-12625-complete-20261101").resolve();BASE=Path(os.environ.get("COAX_CLOSURE_NATIVE_OUT","/tmp/coax-12625-closure/native-v6"));OUT=BASE/"fixedpoints";OUT.mkdir(exist_ok=False)
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def run(m):
 t=time.monotonic();argv=[str(BASE/"coax"),"c","lib/compiler/"+m+".coax","--library","--header",str(OUT/(m+".h"))];c=OUT/(m+".c");err=OUT/(m+".stderr")
 with c.open("wb") as so,err.open("wb") as se:
  p=subprocess.Popen(argv,cwd=ROOT/"coax",stdout=so,stderr=se,start_new_session=True)
  timed_out=False
  try:rc=p.wait(timeout=840)
  except subprocess.TimeoutExpired:timed_out=True;os.killpg(p.pid,signal.SIGKILL);p.wait();rc=p.returncode
 stem="proto_importer" if m=="compiler" else m
 row={"module":m,"argv":argv,"returncode":rc,"timed_out":timed_out,"elapsed_seconds":time.monotonic()-t,"stderr_empty":err.stat().st_size==0,"c_identical":rc==0 and c.read_bytes()==(BASE/(stem+".c")).read_bytes(),"header_identical":rc==0 and (OUT/(m+".h")).read_bytes()==(BASE/(stem+".h")).read_bytes()}
 (OUT/(m+".json")).write_text(json.dumps(row,indent=2)+"\n");print(m,row,flush=True);return row
pins=json.loads((BASE/"source-pins.json").read_text())["source_pins"];assert all(sha(ROOT/p)==h for p,h in pins.items()),"source drift before fixedpoint"
mods=["budget","check-domains","profiles","mutation-schedule","lexer","parser","checker","sources","compiler"]
with concurrent.futures.ThreadPoolExecutor(max_workers=4) as ex:rows=list(ex.map(run,mods))
record={"schema":"coax.r2.native.fixedpoint.v1","compiler":str(BASE/"coax"),"compiler_sha256":sha(BASE/"coax"),"commands":rows,"inputs_unchanged":all(sha(ROOT/p)==h for p,h in pins.items()),"passed":all(r["returncode"]==0 and r["stderr_empty"] and r["c_identical"] and r["header_identical"] for r in rows)}
record["passed"]=record["passed"] and record["inputs_unchanged"]
(OUT/"record.json").write_text(json.dumps(record,indent=2)+"\n");print("PASSED",record["passed"])
