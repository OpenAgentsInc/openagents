# Firm reason: 4 (compiler bootstrap production of a matching native image).
import concurrent.futures,hashlib,json,os,signal,subprocess,time,sys
from pathlib import Path
ROOT=Path("/tmp/openagents-12625-complete-20261101").resolve();OUT=Path(os.environ.get("COAX_CLOSURE_NATIVE_OUT","/tmp/coax-12625-closure/native-v5"));GEN=Path(os.environ.get("COAX_CLOSURE_GENERATOR","/tmp/coax-12625-native/final-v4/coax")); records=[]; BUILD_START=time.monotonic()
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def run(name,args,timeout=840):
    t=time.monotonic();so=OUT/(name+".stdout");se=OUT/(name+".stderr")
    with so.open("wb") as o,se.open("wb") as e:
        p=subprocess.Popen(list(map(str,args)),cwd=ROOT/"coax",stdout=o,stderr=e,start_new_session=True)
        timed_out=False
        try:rc=p.wait(timeout=min(timeout,max(1,840-(time.monotonic()-BUILD_START))))
        except subprocess.TimeoutExpired:timed_out=True;os.killpg(p.pid,signal.SIGKILL);p.wait();rc=p.returncode
    row={"name":name,"argv":list(map(str,args)),"returncode":rc,"timed_out":timed_out,"elapsed_seconds":time.monotonic()-t,"stdout_sha256":sha(so),"stderr_sha256":sha(se)}
    (OUT/(name+".json")).write_text(json.dumps(row,indent=2)+"\n");print(name,rc,row["elapsed_seconds"],flush=True)
    if rc:raise RuntimeError(name+" failed "+se.read_text()[-3000:])
    return row
if __name__=="__main__":
    freeze_path=Path(os.environ["COAX_CLOSURE_FREEZE"])
    freeze=json.loads(freeze_path.read_text());assert freeze["authorized_all_source_frozen"] is True
    assert all(sha(ROOT/p)==h for p,h in freeze["source_pins"].items()),"source drift after coordinated freeze"
    assert sha(GEN)=="710c4f5708d4c7ccbbe3207f89b9e9bb141860988ec9d98e71eca7a14712ac4b"
    OUT.mkdir(exist_ok=False)
    paths=sorted([p for d in ["coax/src","coax/lib"] for p in (ROOT/d).rglob("*") if p.is_file()])
    pins={str(p.relative_to(ROOT)):sha(p) for p in paths}
    (OUT/"source-pins.json").write_text(json.dumps({"root":str(ROOT),"source_pins":pins,"generator":str(GEN),"generator_sha256":sha(GEN)},indent=2)+"\n")

    mods=["budget","check-domains","profiles","mutation-schedule","lexer","parser","checker","sources"]
    def emit(m):
        r=run("emit-"+m,[GEN,"c","lib/compiler/"+m+".coax","--library","--header",OUT/(m+".h")])
        (OUT/(m+".c")).write_bytes((OUT/("emit-"+m+".stdout")).read_bytes());return r
    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as ex:records.extend(ex.map(emit,mods))
    def obj(m):return run("object-"+m,["cc","-std=c11","-O1","-g","-c",OUT/(m+".c"),"-o",OUT/(m+".o")])
    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as ex:records.extend(ex.map(obj,mods))
    proto=[("CORE","coax/lib/proto/wire.coax"),("MEMORY","coax/lib/proto/memory.coax"),("IMPORTER","coax/lib/proto/importer.coax")]
    rt="".join("static constexpr char PROTO_"+n+'_SOURCE[] = R"COAX_'+n+'('+ (ROOT/p).read_text()+')COAX_'+n+'";\n' for n,p in proto)
    rt+='static constexpr char PROTO_FINGERPRINT_SOURCE[] = R"COAX_FP('+(ROOT/"coax/lib/crypto/sha256-kernel.coax").read_text()+(ROOT/"coax/lib/proto/fingerprint.coax").read_text()+')COAX_FP";\n'
    (OUT/"proto_runtime.inc").write_text(rt)
    records.append(run("compile-stage0",["c++","-std=c++20","-O1","-g","-Wall","-Wextra","-Wno-missing-field-initializers","-DCOAX_BOOTSTRAP_IMPORTER","-I",OUT,"-o",OUT/"stage0",ROOT/"coax/src/main.cpp",*[OUT/(m+".o") for m in mods]]))
    records.append(run("emit-importer",[OUT/"stage0","c","lib/compiler/compiler.coax","--library","--header",OUT/"proto_importer.h"]))
    (OUT/"proto_importer.c").write_bytes((OUT/"emit-importer.stdout").read_bytes())
    records.append(run("compile-importer",["cc","-std=c11","-O1","-g","-c",OUT/"proto_importer.c","-o",OUT/"proto_importer.o"]))
    records.append(run("compile-native",["c++","-std=c++20","-O1","-g","-Wall","-Wextra","-Wno-missing-field-initializers","-I",OUT,"-o",OUT/"coax",ROOT/"coax/src/main.cpp",OUT/"proto_importer.o"]))
    assert all(sha(ROOT/p)==v for p,v in pins.items()),"source drift during build"
    (OUT/"build.json").write_text(json.dumps({"schema":"coax.12625.closure.native.build.v5","freeze_authorization_sha256":sha(freeze_path),"normal_provenance":False,"generator":str(GEN),"generator_sha256":sha(GEN),"compiler_sha256":sha(OUT/"coax"),"commands":records},indent=2)+"\n")
