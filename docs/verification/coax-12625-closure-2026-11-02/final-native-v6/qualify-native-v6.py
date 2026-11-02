# Firm reason: 2 (independent native ABI, diagnostic, effect and sanitizer oracles).
import concurrent.futures,hashlib,json,os,signal,subprocess,time
from pathlib import Path
ROOT=Path("/tmp/openagents-12625-complete-20261101").resolve();OUT=Path(os.environ.get("COAX_CLOSURE_NATIVE_OUT","/tmp/coax-12625-closure/native-v6"));COMP=OUT/"coax";DIR=OUT/"qualification";DIR.mkdir(exist_ok=False)
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def run(task):
    name,argv,extra=task;env={**os.environ,**extra};t=time.monotonic();so=DIR/(name+".stdout");se=DIR/(name+".stderr")
    with so.open("wb") as o,se.open("wb") as e:
        p=subprocess.Popen(list(map(str,argv)),cwd=ROOT,env=env,stdout=o,stderr=e,start_new_session=True)
        timed_out=False
        try:rc=p.wait(timeout=840)
        except subprocess.TimeoutExpired:timed_out=True;os.killpg(p.pid,signal.SIGKILL);p.wait();rc=p.returncode
    row={"name":name,"argv":list(map(str,argv)),"environment":extra,"returncode":rc,"timed_out":timed_out,"elapsed_seconds":time.monotonic()-t,"stdout_sha256":sha(so),"stderr_sha256":sha(se)}
    (DIR/(name+".json")).write_text(json.dumps(row,indent=2)+"\n");print(name,rc,row["elapsed_seconds"],flush=True)
    return row
pins=json.loads((OUT/"source-pins.json").read_text())["source_pins"];before={p:sha(ROOT/p) for p in pins};assert before==pins,"source drift before qualification"
test_paths=[ROOT/p for p in ["coax/tests/results.py","coax/tests/results_profiles.py","coax/tests/results-sum-native.c","coax/tests/parser.py","coax/tests/lexer.py","coax/tests/quiet_crash.py","coax/tools/bounded_process.py","coax/tests/sys/read-file.py","coax/tests/sys/read-file-fixture.coax","coax/tests/sys/read-file-native.c","coax/tests/sys/read-file-faults.h","coax/platform/read-file-native.c","coax/platform/read-file-native.h"]]
for directory in ["coax/tests/results","verse","proto"]:
    test_paths.extend(p for p in (ROOT/directory).rglob("*") if p.is_file() and p.suffix in (".coax",".json",".proto",".pb",".c",".h"))
test_pins={str(p.relative_to(ROOT)):sha(p) for p in sorted(set(test_paths))}
tests=[("outcomes",["python3",ROOT/"coax/tests/results.py","--compiler",COMP,"--output",OUT/"outcomes","--budget-s","840"],{}),("profiles",["python3",ROOT/"coax/tests/results_profiles.py"],{"COAX_TEST_COMPILER":str(COMP),"COAX_RESULTS_PROFILE_DIR":str(OUT/"profiles"),"COAX_TEST_ARTIFACT_DIR":str(OUT)}),("read-file",["python3",ROOT/"coax/tests/sys/read-file.py","--compiler",COMP,"--output",OUT/"read-file"],{}),("native-bridge",["python3",Path("/tmp/coax-12625-closure/bridge-native-v6.py")],{"COAX_TEST_COMPILER":str(COMP),"COAX_TEST_ARTIFACT_DIR":str(OUT),"COAX_NATIVE_BRIDGE_DIR":str(OUT/"bridge"),"ASAN_OPTIONS":"detect_leaks=0","UBSAN_OPTIONS":"halt_on_error=1"})]
with concurrent.futures.ThreadPoolExecutor(max_workers=4) as ex:rows=list(ex.map(run,tests))
after={p:sha(ROOT/p) for p in pins};record={"schema":"coax.12625.native.qualification.v1","compiler":str(COMP),"compiler_sha256":sha(COMP),"source_pins":pins,"test_input_pins":test_pins,"native_bridge_wall_override_script_sha256":sha(Path("/tmp/coax-12625-closure/bridge-native-v6.py")),"inputs_unchanged":before==after and all(sha(ROOT/p)==h for p,h in test_pins.items()),"commands":rows,"passed":all(r["returncode"]==0 for r in rows) and before==after and all(sha(ROOT/p)==h for p,h in test_pins.items())}
(DIR/"record.json").write_text(json.dumps(record,indent=2)+"\n");print("PASSED",record["passed"],flush=True)
