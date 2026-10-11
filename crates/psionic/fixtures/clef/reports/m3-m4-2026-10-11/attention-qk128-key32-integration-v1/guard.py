"""Bound and clean up only this candidate build process group."""
from pathlib import Path
import hashlib,json,os,signal,subprocess,sys,time
root=Path(__file__).resolve().parent
repo=Path('<WORKTREE>')
sys.path.insert(0,str(repo/'crates/psionic/fixtures/clef/tools'))
from mac_quiet_bench import OwnedProcessGuard,available_memory,stop_owned_group
leases=set(os.environ.get('OPENAGENTS_LEASES','').split(','))
if not {'build','memory','disk'}<=leases or 'quiet' in leases:raise SystemExit('requires build, memory and disk leases without quiet')
if (root/'guard.json').exists():raise SystemExit('refuse replacing a build attempt')
before=available_memory()
record={'status':'preflight','preflight':before,'cap_bytes':24*1024**3,'minimum_start_bytes':36*1024**3,'minimum_available_bytes':12*1024**3,'deadline_seconds':7200,'cargo_jobs':2,'rust_toolchain':'1.97.1','target':'<OWNER_HOME>/work/openagents-target-agent1','source_manifest_sha256':hashlib.sha256((root/'source-manifest.json').read_bytes()).hexdigest(),'script_sha256':{n:hashlib.sha256((root/n).read_bytes()).hexdigest() for n in ['guard.py','checks.py']}}
def save():(root/'guard.json').write_text(json.dumps(record,indent=2)+'\n')
save()
if before['available_memory_bytes']<record['minimum_start_bytes']:
 record.update(status='refused',reason='need36GiB available');save();raise SystemExit(1)
process=guard=None;start=time.monotonic()
def interrupted(signum,frame):raise KeyboardInterrupt('signal '+str(signum))
signal.signal(signal.SIGTERM,interrupted);signal.signal(signal.SIGINT,interrupted)
try:
 process=subprocess.Popen([sys.executable,str(root/'checks.py')],cwd=repo,start_new_session=True)
 guard=OwnedProcessGuard(process,record['cap_bytes'],root/'memory.jsonl',minimum_available_memory_bytes=record['minimum_available_bytes'])
 record['status']='running';save()
 while process.poll() is None:
  guard.guard()
  if time.monotonic()-start>7200:raise TimeoutError('candidate build reached7200-second deadline')
  time.sleep(.5)
 guard.guard();record.update(status='complete' if process.returncode==0 else 'failed',exit=process.returncode)
except BaseException as e:record.update(status='failed',error_type=type(e).__name__,error=str(e))
finally:
 if guard:
  try:guard.close()
  except BaseException as e:record.update(status='failed',cleanup_error=str(e))
  record['memory']=guard.receipt()
 elif process:
  try:stop_owned_group(process)
  except BaseException as e:record.update(status='failed',cleanup_error=str(e))
 record['seconds']=time.monotonic()-start;save()
print(json.dumps(record),flush=True)
raise SystemExit(0 if record['status']=='complete' else 1)
