from pathlib import Path
import concurrent.futures,hashlib,json,os,signal,subprocess,threading,time
root=Path('/Users/christopherdavid/work/openagents-aethyr-book');binary=Path('/Users/christopherdavid/work/openagents-aethyr-check-budgets-current/coax/bin/coax');work=Path('/tmp/aethyr-ch11-grid-check-profiles');work.mkdir(exist_ok=True)
rows=json.loads(Path('/tmp/aethyr-ch11-grid-long-checks.json').read_text());sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();compiler=sha(binary);assert compiler=='1fac3750bcffbb735ef26f4a065d16414b911068fd7d68bc3ca112565d2432aa'
before={str(p.relative_to(root)):sha(p)for p in root.rglob('*.coax')if p.is_file()};(work/'sources-before').mkdir(exist_ok=True)
for rel in sorted({r['source']for r in rows}):(work/'sources-before'/Path(rel).name).write_bytes((root/rel).read_bytes())
active={};lock=threading.Lock();cancel=threading.Event();began=time.monotonic();results=[]
def stop(proc):
 if proc.poll()is not None:return
 try:os.killpg(proc.pid,signal.SIGTERM)
 except ProcessLookupError:return
 try:proc.wait(timeout=3)
 except subprocess.TimeoutExpired:
  try:os.killpg(proc.pid,signal.SIGKILL)
  except ProcessLookupError:pass
  proc.wait(timeout=3)
def interrupted(sig,frame):raise KeyboardInterrupt('signal '+str(sig))
for sig in (signal.SIGINT,signal.SIGTERM,signal.SIGHUP):signal.signal(sig,interrupted)
def run(row):
 i=row['index'];ev=work/f'{i:02}.evidence.json';command=[str(binary),'test','apps/openagents-web/newland.coax','--library','--only',row['name'],'--steps','400000000','--profile','steps','--evidence',str(ev)]
 assert not ev.exists(),'Refusing to overwrite a previous measurement.'
 start=time.monotonic();p=None;timed=False
 with lock:
  if cancel.is_set():raise RuntimeError('batch cancelled before spawn')
  p=subprocess.Popen(command,cwd=root,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,start_new_session=True);active[i]=p
 try:
  try:out,_=p.communicate(timeout=max(.1,min(780,805-(time.monotonic()-began))))
  except subprocess.TimeoutExpired:timed=True;stop(p);out,_=p.communicate(timeout=3)
 except BaseException:stop(p);raise
 finally:
  with lock:active.pop(i,None)
 (work/f'{i:02}.log').write_bytes(out);evidence=json.loads(ev.read_text())if ev.exists()else{};used={}
 for source in evidence.get('sources',[]):
  pth=Path(source['path']);rel=str(pth.relative_to(root));used[rel]=source['sha256']
 changes=[k for k,h in used.items()if before.get(k)!=h or sha(root/k)!=h]
 claims=[c for c in evidence.get('claims',[])if c.get('name')==row['name']and c.get('outcome')!='not-checked'];assert len(claims)==1,(row['name'],claims)
 claim=claims[0];steps=claim.get('steps');compiler_changed=sha(binary)!=compiler
 passed=p.returncode==0 and not timed and not changes and not compiler_changed and claim.get('outcome')=='passed'
 r={**row,'command':command,'compiler_sha256':compiler,'source_sha256':used,'source_changes':changes,'compiler_changed':compiler_changed,'returncode':p.returncode,'deadline_exceeded':timed,'elapsed_seconds':round(time.monotonic()-start,3),'steps':steps,'claim':claim,'status':'pass'if passed else'fail','scope':'Only this existing authored check is measured under the source-documented400M caller budget. No full authority-coverage claim.'}
 (work/f'{i:02}.json').write_text(json.dumps(r,indent=2)+'\n');print(json.dumps({k:r[k]for k in ['index','name','steps','returncode','elapsed_seconds','status']}),flush=True);return r
pool=concurrent.futures.ThreadPoolExecutor(max_workers=4)
try:
 futures=[pool.submit(run,row)for row in rows]
 for future in concurrent.futures.as_completed(futures,timeout=810):results.append(future.result())
except BaseException:
 cancel.set()
 for future in futures:future.cancel()
 with lock:processes=list(active.values())
 for proc in processes:stop(proc)
 raise
finally:pool.shutdown(wait=True,cancel_futures=True)
assert len(results)==13
result={'schema':'aethyr.grid.check-budget-measurement.v1','status':'pass'if all(r['status']=='pass'for r in results)else'fail','compiler_sha256':compiler,'entry':'apps/openagents-web/newland.coax','selection_count':13,'parallel_commands':4,'batch_deadline_seconds':810,'elapsed_seconds':round(time.monotonic()-began,3),'source_sha256_before':{r:before[r]for r in sorted({row['source']for row in rows})},'results':sorted(results,key=lambda r:r['index'])}
(work/'qualification.json').write_text(json.dumps(result,indent=2)+'\n');print('BATCH',result['status'],result['elapsed_seconds'],flush=True)
assert result['status']=='pass'
