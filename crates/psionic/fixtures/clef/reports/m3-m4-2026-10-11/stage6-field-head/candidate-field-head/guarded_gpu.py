import json,os,subprocess,sys,time
from pathlib import Path
REPO=Path('workspace/openagents')
ROOT=Path(__file__).parent
sys.path.insert(0,str(REPO/'crates/psionic/fixtures/clef/tools'))
from mac_quiet_bench import OwnedProcessGuard,available_memory,file_sha
assert {'gpu','memory'} <= set(os.environ.get('OPENAGENTS_LEASES','').split(','))
record={'status':'refused','preflight':available_memory(),'process_budget_bytes':4*1024**3,'host_floor_bytes':12*1024**3,'deadline_seconds':300}
if record['preflight']['available_memory_bytes']<16*1024**3:
    (ROOT/'gpu.json').write_text(json.dumps(record,indent=2)+'\n');raise SystemExit(1)
exe=ROOT/'psionic-backend-metal-tests'
record['binary_sha256']=file_sha(exe)
record['command']=[str(exe),'clef_prefill::head_blocks::tests','--ignored','--nocapture','--test-threads=1']
began=time.monotonic()
env={k:v for k,v in os.environ.items() if not k.startswith('PSIONIC_CLEF_')}
env.update(RAYON_NUM_THREADS='2',OMP_NUM_THREADS='2',VECLIB_MAXIMUM_THREADS='2')
with (ROOT/'gpu.log').open('w') as log:
    proc=subprocess.Popen(record['command'],cwd=REPO,env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
    guard=OwnedProcessGuard(proc,4*1024**3,ROOT/'gpu-memory.jsonl',minimum_available_memory_bytes=12*1024**3)
    try:
        while proc.poll() is None:
            guard.guard()
            if time.monotonic()-began>300:raise TimeoutError('GPU tests exceeded300seconds')
            time.sleep(.25)
        guard.guard()
        record.update(status='complete' if proc.returncode==0 else 'failed',exit=proc.returncode)
    except BaseException as error:record.update(status='failed',error_type=type(error).__name__,error=str(error))
    finally:
        try:guard.close()
        except BaseException as error:record.update(status='failed',cleanup_error=str(error))
        record['memory']=guard.receipt();record['seconds']=time.monotonic()-began
(ROOT/'gpu.json').write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps(record),flush=True)
raise SystemExit(0 if record['status']=='complete' else 1)
