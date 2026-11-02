#!/usr/bin/env python3
import argparse,concurrent.futures,hashlib,json,os,pathlib,signal,subprocess,threading,time
p=argparse.ArgumentParser();p.add_argument('first',type=int);p.add_argument('last',type=int);p.add_argument('--parallel',type=int,default=8);p.add_argument('--jobs',type=int,default=2);a=p.parse_args();assert 1<=a.parallel<=8 and 1<=a.jobs<=2
out=pathlib.Path('/tmp/aethyr-authority-budget-current-checker-shards');qpath=out/'qualification.json';q=json.loads(qpath.read_text());cwd=q['cwd'];binary=q['compiler'];active={};lock=threading.Lock();began=time.monotonic()
def sha(p):return hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
def stop(proc):
 if proc.poll() is not None:return
 try:os.killpg(proc.pid,signal.SIGTERM)
 except ProcessLookupError:return
 try:proc.wait(timeout=3)
 except subprocess.TimeoutExpired:
  try:os.killpg(proc.pid,signal.SIGKILL)
  except ProcessLookupError:pass
  proc.wait(timeout=3)
def interrupted(sig,frame):raise KeyboardInterrupt('signal '+str(sig))
for sig in (signal.SIGINT,signal.SIGTERM,signal.SIGHUP):signal.signal(sig,interrupted)
def work(i):
 command=[binary,'mutate',q['entry'],'--shard',f'{i}/{q["total_shards"]}','--jobs',str(a.jobs)];record=out/f'{i:03}.rec';error=out/f'{i:03}.stderr';started=time.monotonic()
 with record.open('wb') as stdout,error.open('wb') as stderr:
  proc=subprocess.Popen(command,cwd=cwd,stdout=stdout,stderr=stderr,start_new_session=True)
  with lock:active[i]=proc
  try:
   try:code=proc.wait(timeout=780)
   except subprocess.TimeoutExpired:stop(proc);code='timeout'
  finally:
   with lock:active.pop(i,None)
 result={'part':i,'command':command,'exit_code':code,'seconds':round(time.monotonic()-started,3),'record':str(record),'record_sha256':sha(record),'stderr':str(error),'stderr_sha256':sha(error)}
 print(json.dumps(result),flush=True);return result
existing={r['part'] for r in q['shards'] if r['exit_code']==0}
parts=[i for i in range(a.first,a.last+1) if i not in existing];assert len(parts)==a.last-a.first+1,'refusing repeated passed or partial overlapping batch'
assert sha(binary)==q['compiler_sha256'];assert all(sha(path)==h for path,h in q['source_inputs'].items())
results=[];pool=concurrent.futures.ThreadPoolExecutor(max_workers=a.parallel)
try:
 futures=[pool.submit(work,i) for i in parts]
 for future in concurrent.futures.as_completed(futures,timeout=810):results.append(future.result())
except BaseException as error:
 print('batch interruption: '+repr(error),flush=True)
 for future in futures:future.cancel()
 with lock:processes=list(active.values())
 for proc in processes:stop(proc)
 raise
finally:
 pool.shutdown(wait=True,cancel_futures=True)
 q=json.loads(qpath.read_text());q['shards'] += results;q['shards'].sort(key=lambda r:r['part']);q['source_changes']=[path for path,h in q['source_inputs'].items() if sha(path)!=h]
 q['compiler_changed']=sha(binary)!=q['compiler_sha256'];q['last_batch']={'first':a.first,'last':a.last,'seconds':round(time.monotonic()-began,3),'parallel':a.parallel,'jobs':a.jobs}
 if q['source_changes'] or q['compiler_changed'] or any(r['exit_code']!=0 for r in results):q['status']='failed'
 temp=qpath.with_suffix('.tmp');temp.write_text(json.dumps(q,indent=2)+'\n');temp.replace(qpath)
assert len(results)==len(parts) and all(r['exit_code']==0 for r in results), 'incomplete or failed shard batch'
assert not q['source_changes'] and not q['compiler_changed'],'source/compiler changed'
