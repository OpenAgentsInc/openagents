from pathlib import Path
import hashlib,json,os,signal,subprocess,concurrent.futures,time
root=Path('/Users/christopherdavid/work/openagents-aethyr-book');out=Path('/tmp/aethyr-ch11-beacon-alias');out.mkdir(exist_ok=True);name='the envelope refusals: kind, tags, marker, digest, expiration and auth';sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();before={str(p.relative_to(root)):sha(p)for p in root.rglob('*.coax')if p.is_file()};source=root/'psionic/lib/pylon/beacon.coax';(out/'beacon-before.coax').write_bytes(source.read_bytes());rows=[]
for label,binary in [('308',root/'coax/bin/coax'),('1fac',Path('/Users/christopherdavid/work/openagents-aethyr-check-budgets-current/coax/bin/coax'))]:
 for scope,entry in [('bare','psionic/lib/pylon/beacon.coax'),('grid','aethyr/render/carriers/landing-browser.coax')]:rows.append((label,scope,binary,entry))
def run(row):
 label,scope,binary,entry=row;tag=label+'-'+scope;evidence=out/(tag+'.evidence.json');command=[str(binary),'test',entry,'--library','--run-steps','128000','--only',name,'--steps','400000000','--evidence',str(evidence)];compiler=sha(binary);start=time.monotonic();p=subprocess.Popen(command,cwd=root,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,start_new_session=True)
 try:stdout,_=p.communicate(timeout=180)
 except BaseException:
  os.killpg(p.pid,signal.SIGTERM)
  try:p.wait(timeout=3)
  except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGKILL);p.wait(timeout=3)
  raise
 (out/(tag+'.log')).write_bytes(stdout);e=json.loads(evidence.read_text())if evidence.exists()else{};used={str(Path(v['path']).relative_to(root)):v['sha256']for v in e.get('sources',[])};changes=[k for k,h in used.items()if before.get(k)!=h or sha(root/k)!=h];r={'label':label,'scope':scope,'command':command,'compiler_sha256':compiler,'compiler_changed':sha(binary)!=compiler,'returncode':p.returncode,'seconds':round(time.monotonic()-start,3),'source_sha256':used,'source_changes':changes,'claims':[v for v in e.get('claims',[])if v.get('name')==name]};(out/(tag+'.json')).write_text(json.dumps(r,indent=2)+'\n');print(json.dumps({k:r[k]for k in ['label','scope','returncode','seconds','source_changes','claims']}),flush=True);return r
with concurrent.futures.ThreadPoolExecutor(max_workers=4)as pool:results=list(pool.map(run,rows))
(out/'diagnosis.json').write_text(json.dumps({'schema':'aethyr.beacon.original-check-diagnosis.v1','scope':'Original unchanged check under both qualified compiler artifacts and actual Grid SHA-owner carrier. Failures are retained as observations.','results':results},indent=2)+'\n');assert all(not r['source_changes']and not r['compiler_changed']for r in results)
