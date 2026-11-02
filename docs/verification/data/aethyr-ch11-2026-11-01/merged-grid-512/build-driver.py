from pathlib import Path
import json,hashlib,subprocess,os,time,signal,re,concurrent.futures
ROOT=Path('/Users/christopherdavid/work/openagents-aethyr-book-main');OUT=Path('/tmp/aethyr-ch11-grid-main-512-budget-100m');DRIVER=Path('/tmp/aethyr-authority-budget-qualified-1fac/coax')
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
EXPECTED='1fac3750bcffbb735ef26f4a065d16414b911068fd7d68bc3ca112565d2432aa'
assert sha(DRIVER)==EXPECTED
OUT.mkdir(exist_ok=True);start=time.monotonic();deadline=start+810;commands=[]
def run(cmd,name,timeout=90,env=None):
 begin=time.monotonic();log=OUT/(name+'.log')
 with log.open('wb') as f:
  p=subprocess.Popen([str(x) for x in cmd],cwd=ROOT,stdout=f,stderr=f,start_new_session=True,env=env)
  try:code=p.wait(timeout=max(.1,min(timeout,deadline-time.monotonic())))
  except subprocess.TimeoutExpired:
   os.killpg(p.pid,signal.SIGTERM)
   try:p.wait(timeout=5)
   except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGKILL);p.wait()
   commands.append({'command':list(map(str,cmd)),'elapsed_seconds':time.monotonic()-begin,'returncode':p.returncode,'deadline_exceeded':True,'log':str(log)});return p.returncode,True
  except BaseException:
   os.killpg(p.pid,signal.SIGKILL);p.wait();raise
 commands.append({'command':list(map(str,cmd)),'elapsed_seconds':time.monotonic()-begin,'returncode':code,'deadline_exceeded':False,'log':str(log)});return code,False

entries=[('newland','apps/openagents-web/newland.coax'),('landing-browser','aethyr/render/carriers/landing-browser.coax'),('landing-fold','coax/web/project/landing-fold.coax'),('Gridwalk','verse/gridwalk.coax')]
def query(entry):
 name,src=entry;profile=OUT/(name+'-query.json')
 if profile.exists():
  d=json.loads(profile.read_text());assert d['success'] and all(sha(Path(s['path']))==s['sha256'] for s in d['sources'])
  return d
 code,timed=run([DRIVER,'query',src,'exports','--library','--profile-report',profile],name+'-query',90)
 assert code==0 and not timed,(name,code)
 d=json.loads(profile.read_text());assert d['success'];return d
with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:queries=list(pool.map(query,entries))
before={}
for q in queries:
 for s in q['sources']+q['descriptors']:
  path=Path(s['path']);key=str(path.relative_to(ROOT));assert sha(path)==s['sha256'];before[key]=s['sha256']
# The finite C-effect inputs are observed before the build. Actual emitted
# source tables and Clang include graphs refine this conservative list later.
cfiles=list((ROOT/'apps/openagents-web').glob('*.[ch]'))+list((ROOT/'coax/web/project').glob('*.[ch]'))+[ROOT/'coax/web/render-face.c',ROOT/'coax/web/toolchain.sh',ROOT/'coax/web/project/build-home.sh']
before.update({str(p.relative_to(ROOT)):sha(p) for p in cfiles})
compiler_folder=DRIVER.parent
compiler_provenance={str(p):sha(p) for p in compiler_folder.iterdir() if p.is_file() and p.name in ['coax','provenance-check','provenance.record','input.sha256','source.digest','source-digest']}
checkout_source_digest=subprocess.check_output([str(DRIVER),'source-digest',str(ROOT)],cwd=ROOT,text=True,timeout=30).strip()
admitted=OUT/'admitted';artifact=OUT/'newland.wasm';env=dict(os.environ,COAX=str(DRIVER),COAXIAL_ADMITTED_DIR=str(admitted));env.pop('COAXIAL_RENDER',None)
cmd=['coax/web/project/build-home.sh',str(artifact)]
code,timed=run(cmd,'execution',810,env)
changes=[p for p,h in before.items() if not (ROOT/p).is_file() or sha(ROOT/p)!=h]
compiler_changed=sha(DRIVER)!=EXPECTED or any(sha(Path(p))!=h for p,h in compiler_provenance.items())
emitted={p.name:sha(p) for p in admitted.glob('*') if p.is_file()}
meta={'schema':'aethyr.render.merged-grid-ordinary-build.v1','status':'passed' if code==0 and not timed and not changes and not compiler_changed else 'failed',
 'command':cmd,'returncode':code,'deadline_exceeded':timed,'elapsed_seconds':time.monotonic()-start,'whole_command_deadline_seconds':810,
 'worktree':str(ROOT),'head_at_build':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
 'compiler':{'path':str(DRIVER),'sha256':EXPECTED,'qualification_commit':'8008dcc0386f7ba819faa7b1bc8c4454ed460fae','source_digest':'b3f0e5a69265dbbbe2f7dc07ebff868b415e465658bf68c2436202260f951dc9','provenance_artifact_sha256':compiler_provenance,'checkout_source_digest':checkout_source_digest,'sources_match_checkout':False,'scope':'Explicit supported COAX override pins independently qualified B3/1fac. This artifact is not claimed to have been built from the newer integrated main compiler sources.'},
 'environment':{'COAX':str(DRIVER),'COAXIAL_RENDER':'default1(unset)','COAXIAL_ADMITTED_DIR':str(admitted)},
 'inputs':before,'source_changes':changes,'compiler_changed':compiler_changed,'emitted':emitted,'wasm_sha256':sha(artifact) if artifact.exists() else None,
 'commands':commands,'scope':'Fresh ordinary renderer-enabled merged512-row Grid/homepage build; mandatory Coax authority checks/emission, default checked C and strict platform math,4MiB stack/64MiB maximum/400k vertices. No old320 build is relabelled current.'}
(OUT/'qualification.json').write_text(json.dumps(meta,indent=2)+'\n');print(json.dumps({k:meta[k] for k in ['status','returncode','deadline_exceeded','elapsed_seconds','source_changes','compiler_changed','wasm_sha256']}))
if meta['status']!='passed':raise SystemExit(1)
