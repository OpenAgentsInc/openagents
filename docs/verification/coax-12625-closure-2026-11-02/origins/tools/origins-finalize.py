# Firm reason: 2 (validate stock shard merge and publish byte-bound receipts).
from pathlib import Path
import hashlib,json,subprocess,time,sys,shutil,signal,os
D=Path('/tmp/coax-12625-origins-closure-scopes/CB_query-v6');C=Path('/tmp/coax-12625-native/final-v4/coax');PRIVATE=Path('/tmp/openagents-12625-origins-closure-20261102');SHARED=Path('/tmp/openagents-12625-complete-20261101');OUT=PRIVATE/'docs/verification/coax-12625-closure-2026-11-02/origins'
def sha(b):return hashlib.sha256(b).hexdigest()
s=json.loads((D/'setup.json').read_text());assert all(sha((D/f).read_bytes())==h for f,h in s['pins'].items());assert sha(C.read_bytes())==s['compiler_sha256']
shards=[]
for i in range(1,5):
 p=D/('shard-'+str(i));e=json.loads((p/'evidence.json').read_text());r=json.loads((p/'process.json').read_text());m=e['mutation'];assert r['exit_status']==0 and not r['timeout'] and r['pins_unchanged'] and r['compiler_unchanged'];assert m['mutants']==371 and m['shard']==[i,4] and m['killed']==m['evaluated'] and not m['survived'] and m['marked_equivalent']==0;assert e['sources']==json.loads((D/'shard-1/evidence.json').read_text())['sources'];shards.append(r)
cmd=[str(C),'mutate',str(D/'scope.coax'),'--root-only']
for i in range(1,5):cmd+=['--merge',str(D/('shard-'+str(i))/'stdout')]
cmd+=['--evidence',str(D/'merge-evidence.json')];start=time.monotonic();timeout=False
with (D/'merge-stdout').open('wb') as so,(D/'merge-stderr').open('wb') as se:
 p=subprocess.Popen(cmd,cwd=D,stdout=so,stderr=se,start_new_session=True)
 try:rc=p.wait(timeout=840)
 except subprocess.TimeoutExpired:
  timeout=True;os.killpg(p.pid,signal.SIGTERM)
  try:rc=p.wait(timeout=5)
  except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGKILL);rc=p.wait()
r={'argv':cmd,'exit_status':rc,'timeout':timeout,'seconds':time.monotonic()-start,'pins_unchanged':all(sha((D/f).read_bytes())==h for f,h in s['pins'].items()),'compiler_unchanged':sha(C.read_bytes())==s['compiler_sha256'],'stdout_sha256':sha((D/'merge-stdout').read_bytes()),'stderr_sha256':sha((D/'merge-stderr').read_bytes())};(D/'merge-process.json').write_text(json.dumps(r,indent=2)+'\n');assert rc==0 and not timeout and r['pins_unchanged'] and r['compiler_unchanged']
e=json.loads((D/'merge-evidence.json').read_text());m=e['mutation'];assert m['mutants']==m['evaluated']==m['killed']==371 and not m['survived'] and m['marked_equivalent']==0 and m['merged_records']==4
subprocess.run([sys.executable,'/tmp/origins-archive.py'],check=True)
record=json.loads((OUT/'record.json').read_text());baseline=json.loads((D/'shard-1/evidence.json').read_text())['claims'];record['final_query']={'scope':'CB_query-v6','mutants':371,'evaluated':371,'killed':371,'survived':0,'marked_equivalent':0,'stock_merged_records':4,'scope_source_sha256':sha((D/'scope.coax').read_bytes()),'production_body_sha256':s['production_body_sha256'],'baseline_claims':len(baseline),'baseline_cases':sum(c.get('cases',0) for c in baseline),'baseline_outcomes':sorted(set(c['outcome'] for c in baseline)),'merge_seconds':r['seconds'],'shard_seconds':[x['seconds'] for x in shards]};record['combined_focused_sites']=478+371;record['combined_focused_killed']=478+371;record['combined_focused_survived']=0;record['final_cache_modules']={n:sha((D/'coax/lib/compiler'/n).read_bytes()) for n in ['query-index.coax','literal-projection-cache.coax','query-projection-cache.coax']};(OUT/'record.json').write_text(json.dumps(record,indent=2)+'\n')
f=OUT/'README.md';txt=f.read_text().replace('| `CB_query` | Previously unrun; first complete inventory had 7 | pending | pending | pending |','| `CB_query` | Previously unrun; first complete inventory had 7 | 371 | 371 | 0 |');txt=txt.replace('The first four bodies are unchanged.','All five focused procedures have 849 sites killed and zero survivors. The final query result is a complete stock four-shard merge; every shard used one source identity and all source/compiler pins remained unchanged.\n\nThe first four bodies are unchanged.');f.write_text(txt)
shutil.copyfile('/tmp/origins-finalize.py',OUT/'tools/origins-finalize.py')
pins={str(p.relative_to(OUT)):{'bytes':p.stat().st_size,'sha256':sha(p.read_bytes())} for p in sorted(OUT.rglob('*')) if p.is_file() and p.name!='package-manifest.json'};(OUT/'package-manifest.json').write_text(json.dumps({'schema':1,'files':pins},indent=2)+'\n')
dst=SHARED/'docs/verification/coax-12625-closure-2026-11-02/origins';assert not dst.exists(),'Stop rather than overwrite another origins archive';shutil.copytree(OUT,dst)
assert all(sha((dst/f).read_bytes())==v['sha256'] for f,v in pins.items())
print(json.dumps({'published':str(dst),'manifest_sha256':sha((dst/'package-manifest.json').read_bytes()),'final_query':record['final_query'],'focused_total':849,'files':len(pins),'bytes':sum(v['bytes'] for v in pins.values())},indent=2));print((D/'merge-stdout').read_text())
