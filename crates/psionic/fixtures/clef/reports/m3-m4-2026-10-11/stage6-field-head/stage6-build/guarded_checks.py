import json, os, pathlib, subprocess, sys, time
sys.path.insert(0, str(pathlib.Path.cwd()/"crates/psionic/fixtures/clef/tools"))
from mac_quiet_bench import OwnedProcessGuard, available_memory
root=pathlib.Path(__file__).parent
before=available_memory()
if before["available_memory_bytes"] < 36 * 1024**3:
    (root/"guard-preflight.json").write_text(json.dumps({"status":"refused","reason":"Need 36 GiB available for bounded build","preflight":before},indent=2)+"\n")
    raise SystemExit("Need 36 GiB available for bounded build")
env=dict(os.environ,CARGO_TARGET_DIR="user-home/work/openagents-target-agent1",CARGO_BUILD_JOBS="2")
process=subprocess.Popen([sys.executable,str(root/"checks.py")],env=env,start_new_session=True)
guard=OwnedProcessGuard(process,24*1024**3,root/"build-memory.jsonl",minimum_available_memory_bytes=12*1024**3)
record={"preflight":before,"status":"running","exit":None}
start=time.monotonic()
try:
    while process.poll() is None:
        guard.guard()
        if time.monotonic()-start > 7200:
            raise TimeoutError("Build deadline exceeded")
        time.sleep(.5)
    guard.guard()
    record.update(status="complete" if process.returncode==0 else "failed",exit=process.returncode)
except BaseException as error:
    record.update(status="failed",error=str(error))
finally:
    try:
        guard.close()
    except Exception as error:
        record.update(status="failed",cleanup_error=str(error))
    record["memory"]=guard.receipt()
    record["seconds"]=time.monotonic()-start
    (root/"guard.json").write_text(json.dumps(record,indent=2)+"\n")
print(json.dumps(record),flush=True)
raise SystemExit(0 if record["status"]=="complete" else 1)
