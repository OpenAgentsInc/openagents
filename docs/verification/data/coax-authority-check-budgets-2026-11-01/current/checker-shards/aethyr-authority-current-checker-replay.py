#!/usr/bin/env python3
import hashlib,json,os,pathlib,signal,subprocess,time
out=pathlib.Path('/tmp/aethyr-authority-budget-current-checker-shards');q=json.loads((out/'qualification.json').read_text());cwd=pathlib.Path(q['cwd']);manifest=out/'manifest.tsv';retained=pathlib.Path(q['compiler']).parent
sha=lambda p:hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
assert q['status']=='running' and len(q['shards'])==128 and sorted(r['part'] for r in q['shards'])==list(range(1,129))
assert all(r['exit_code']==0 and sha(r['record'])==r['record_sha256'] for r in q['shards'])
assert all(sha(p)==h for p,h in q['source_inputs'].items()) and sha(q['compiler'])==q['compiler_sha256']
rows=['coax-bootstrap-shards-v1','compiler\t'+q['compiler_sha256'],'closure\t'+q['closure'],'entry\t'+q['entry']]+['record\t'+r['record_sha256']+'\t'+r['record']for r in q['shards']]
manifest.write_text('\n'.join(rows)+'\n')
cache_key=hashlib.sha256(('coax-gate-v1\0'+q['retained_construction']['stage0_identity']+'\0'+q['closure']+'\0mutate\0'+q['entry']+'\0').encode()).hexdigest();cache=pathlib.Path.home()/'.cache/coax-gates'/cache_key
def interrupt(sig,frame):raise KeyboardInterrupt('signal '+str(sig))
for sig in (signal.SIGINT,signal.SIGTERM,signal.SIGHUP):signal.signal(sig,interrupt)
input_before=subprocess.check_output(['./provenance.sh','fingerprint',str(cwd.parent)],cwd=cwd,text=True,timeout=60).strip();assert input_before==q['retained_construction']['input_fingerprint']
before=set((cwd/'bin').glob('.replay-*'));gate_fd=None;work=None;command=['bash','./bootstrap-build.sh','--private-replay-checker-cache',str(retained),str(manifest)];started=time.monotonic()
log=out/'supported-replay.log';preexisting_cache=cache.exists();proc=None
try:
 with log.open('wb') as stream:
  proc=subprocess.Popen(command,cwd=cwd,stdout=stream,stderr=subprocess.STDOUT,start_new_session=True)
  while proc.poll() is None:
   new=set((cwd/'bin').glob('.replay-*'))-before
   if len(new)>1:raise RuntimeError('ambiguous replay observation directories')
   if new:work=next(iter(new))
   if gate_fd is None and work and (work/'gate.log').exists():gate_fd=os.open(work/'gate.log',os.O_RDONLY)
   if time.monotonic()-started>780:raise TimeoutError('supported replay exceeded780seconds')
   time.sleep(.01)
  code=proc.wait()
except BaseException:
 if proc and proc.poll() is None:
  os.killpg(proc.pid,signal.SIGTERM)
  try:proc.wait(timeout=3)
  except subprocess.TimeoutExpired:os.killpg(proc.pid,signal.SIGKILL);proc.wait(timeout=3)
 raise
finally:
 if gate_fd is not None:
  os.lseek(gate_fd,0,os.SEEK_SET);pieces=[]
  while True:
   piece=os.read(gate_fd,65536)
   if not piece:break
   pieces.append(piece)
  os.close(gate_fd);(out/'actual-merge.log').write_bytes(b''.join(pieces))
input_after=subprocess.check_output(['./provenance.sh','fingerprint',str(cwd.parent)],cwd=cwd,text=True,timeout=60).strip()
code=proc.returncode;changed=[p for p,h in q['source_inputs'].items() if sha(p)!=h];compiler_changed=sha(q['compiler'])!=q['compiler_sha256']
r={'schema':'coax.supported-checker-replay.v1','command':command,'cwd':str(cwd),'exit_code':code,'seconds':round(time.monotonic()-started,3),'manifest':str(manifest),'manifest_sha256':sha(manifest),'preexisting_cache':preexisting_cache,'cache_key':cache_key,'cache':str(cache),'cache_present_after':cache.exists(),'source_changes':changed,'compiler_changed':compiler_changed,'input_fingerprint_before':input_before,'input_fingerprint_after':input_after,'actual_merge_log':str(out/'actual-merge.log') if gate_fd is not None else None,'actual_merge_log_sha256':sha(out/'actual-merge.log') if gate_fd is not None else None}
(out/'supported-replay.json').write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(r))
assert code==0 and not changed and not compiler_changed and input_before==input_after and cache.exists(),'supported replay failed'
assert gate_fd is not None or preexisting_cache,'neither actualmerge nor priorqualifiedcache observed'
q['status']='passed';q['supported_replay']=r;(out/'qualification.json').write_text(json.dumps(q,indent=2)+'\n')
