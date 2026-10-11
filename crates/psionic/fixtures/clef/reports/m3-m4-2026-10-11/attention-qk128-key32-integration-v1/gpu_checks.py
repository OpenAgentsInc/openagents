"""Run only the four ignored backend attention tests under fixed bounds."""
import hashlib,json,os,signal,subprocess,sys,time
from pathlib import Path
import acceptance_helpers as helper
root=Path(__file__).resolve().parent
bench=helper.bench
leases=set(os.environ.get('OPENAGENTS_LEASES','').split(','))
if not {'gpu','memory'}<=leases or 'quiet' in leases:raise SystemExit('GPU and memory leases, without quiet, required')
out=root/'backend-attention-gpu';out.mkdir(exist_ok=False)
binary=root/'psionic-metal-tests'
built=json.loads((root/'binaries.json').read_text())['backend-build']
if bench.file_sha(binary)!=built['sha256']:raise SystemExit('frozen test binary changed')
if json.loads((root/'guard.json').read_text())['status']!='complete':raise SystemExit('build did not finish')
record={'status':'preflight','binary_sha256':built['sha256'],'script_sha256':bench.file_sha(__file__),
        'max_owned_group_bytes':4*1024**3,'minimum_start_bytes':16*1024**3,'minimum_available_bytes':12*1024**3,'deadline_seconds':180,
        'conditions':'shared-machine correctness only; four attention GPU tests'}
def save():(out/'receipt.json').write_text(json.dumps(record,indent=2)+'\n')
record['preflight']=bench.available_memory();save()
if record['preflight']['available_memory_bytes']<record['minimum_start_bytes']:
 record.update(status='refused',reason='insufficient available memory');save();raise SystemExit(1)
process=guard=None;began=time.monotonic()
def interrupted(signum,_):raise KeyboardInterrupt(f'signal {signum}')
signal.signal(signal.SIGTERM,interrupted);signal.signal(signal.SIGINT,interrupted)
try:
 command=[str(binary),'fused_attention_','--ignored','--test-threads=1','--nocapture'];record['command']=command
 env={k:v for k,v in os.environ.items() if not k.startswith('PSIONIC_CLEF_')};env.update(RAYON_NUM_THREADS='2',OMP_NUM_THREADS='2',VECLIB_MAXIMUM_THREADS='2')
 with (out/'output.log').open('w') as log:
  process=subprocess.Popen(command,cwd=helper.REPO,env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
  guard=bench.OwnedProcessGuard(process,record['max_owned_group_bytes'],out/'memory.jsonl',minimum_available_memory_bytes=record['minimum_available_bytes'])
  while process.poll() is None:
   guard.guard()
   if time.monotonic()-began>180:raise TimeoutError('GPU checks reached180 seconds')
   time.sleep(.2)
  guard.guard();text=(out/'output.log').read_text()
  record.update(status='complete' if process.returncode==0 and 'test result: ok. 4 passed; 0 failed;' in text else 'failed',exit=process.returncode)
except BaseException as e:record.update(status='failed',error_type=type(e).__name__,error=str(e))
finally:
 if guard:
  try:guard.close()
  except BaseException as e:record.update(status='failed',cleanup_error=str(e))
  record['memory']=guard.receipt()
 elif process:
  try:bench.stop_owned_group(process)
  except BaseException as e:record.update(status='failed',cleanup_error=str(e))
 record['seconds']=time.monotonic()-began;save()
print(json.dumps(record),flush=True)
raise SystemExit(0 if record['status']=='complete' else 1)
