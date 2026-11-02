from pathlib import Path
import hashlib,json,os,re,shlex,signal,subprocess,tarfile,time
ROOT=Path('/Users/christopherdavid/work/openagents-aethyr-book-main')
WORK=Path('/tmp/aethyr-ch11-icecream-doorway-main-7c9289b4-producer')
WORK.mkdir(exist_ok=False)
COAX=Path('/tmp/aethyr-authority-budget-qualified-1fac/coax')
ADMITTED=WORK/'admitted'
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
started=time.monotonic();child=None
record={'schema':'aethyr.icecream.doorway-main-producer.v1','status':'failed','checkout':str(ROOT),'product_head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),'build_deadline_seconds':810,'commands':[]}
def kill_group():
 global child
 if child is not None and child.poll()is None:
  try:os.killpg(child.pid,signal.SIGTERM)
  except ProcessLookupError:pass
  try:child.wait(timeout=5)
  except subprocess.TimeoutExpired:
   try:os.killpg(child.pid,signal.SIGKILL)
   except ProcessLookupError:pass
   child.wait()
def interrupt(sig,frame):raise KeyboardInterrupt(f'signal{sig}')
for sig in (signal.SIGINT,signal.SIGTERM):signal.signal(sig,interrupt)
def run(argv,name,limit=20,env=None):
 global child
 then=time.monotonic();p=WORK/name
 with p.open('wb')as log:
  child=subprocess.Popen(list(map(str,argv)),cwd=ROOT,env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
  try:code=child.wait(timeout=limit)
  finally:
   kill_group();record['commands'].append({'argv':list(map(str,argv)),'exit':child.returncode,'elapsed_seconds':round(time.monotonic()-then,6),'log':name,'log_sha256':sha(p)});child=None
 if code:raise RuntimeError(f'{name}:exit{code}')
 return p
try:
 artifact=json.loads(Path('/tmp/aethyr-authority-budget-current-final-artifact.json').read_text())
 assert sha(COAX)==artifact['binary_sha256']=='1fac3750bcffbb735ef26f4a065d16414b911068fd7d68bc3ca112565d2432aa'
 record['compiler']={'path':str(COAX),'sha256':sha(COAX),'qualified_source_root':artifact['root'],'qualified_source_digest':artifact['source_digest'],'qualified_full_input_fingerprint':artifact['full_input_fingerprint'],'matches_current_checkout':False,'scope':'Explicit previously qualified B3 compiler; this is not the current-main matching-source compiler.','prior_qualification':'docs/verification/data/coax-authority-check-budgets-2026-11-01/current/final-artifact.json'}
 (WORK/'compiler-qualified-source.json').write_text(json.dumps(artifact,indent=2)+'\n')
 env=dict(os.environ,COAX=str(COAX),COAXIAL_ADMITTED_DIR=str(ADMITTED));env.pop('COAXIAL_RENDER',None);env.pop('COAXIAL_MODE',None);env.pop('COAXIAL_RUN_STEPS',None)
 tc=run(['bash','-c','source "$1"; printf "%s\\0%s\\0%s\\0" "$COAXIAL_CLANG" "$COAXIAL_SYSROOT" "$COAXIAL_RESOURCES"','toolchain',str(ROOT/'coax/web/toolchain.sh')],'toolchain-paths.bin',env=env).read_bytes().decode().split('\0')
 clang,sysroot,resources=map(Path,tc[:3]);record['wasm_toolchain']={'clang':str(clang),'clang_sha256':sha(clang),'sysroot':str(sysroot),'resources':str(resources)}
 run([clang,'--version'],'clang-version.log')
 files=set(ROOT.rglob('*.coax'))
 files.update(p for p in ROOT.rglob('*')if p.is_file()and p.suffix in('.c','.h','.sh')and not any(part.startswith('.')for part in p.relative_to(ROOT).parts))
 before={str(p.relative_to(ROOT)):sha(p)for p in files}
 external={str(p.resolve()):sha(p)for base in [sysroot/'include',resources/'include']if base.exists()for p in base.rglob('*.h')}
 libraries=run(['verse/native/build.sh','--libraries','icecream'],'libraries.txt').read_text().splitlines();assert len(libraries)==1
 command=['coax/web/wasm.sh','apps/openagents-web/icecream.coax','apps/openagents-web/icecream-face.c',str(WORK/'icecream.wasm'),*libraries]
 record['build_command']=command;record['environment']={'COAX':str(COAX),'COAXIAL_ADMITTED_DIR':str(ADMITTED),'COAXIAL_RENDER':'auto(unset)','COAXIAL_MODE':'release(unset)','COAXIAL_RUN_STEPS':'default(unset)'}
 run(command,'build.log',810,env)
 emitted={p.name:sha(p)for p in ADMITTED.iterdir()if p.is_file()};record['emitted']=emitted
 by_unit={}
 for p in ADMITTED.glob('*.c'):
  text=p.read_text();end=text.index('};',text.index('coax_source_ranges[]'))
  by_unit[p.name]=list(dict.fromkeys(re.findall(r'\{\d+,\d+,\d+,\d+,"([^"]+)"\}',text[:end])))
 assert by_unit and all(by_unit.values());record['coax_emission_source_graph']=by_unit
 inputs=set(n for names in by_unit.values()for n in names)
 inputs.update(['coax/web/wasm.sh','coax/web/toolchain.sh','verse/native/build.sh','apps/openagents-web/build.sh'])
 c_graph={};external_used={}
 units=[*ADMITTED.glob('*.c'),ROOT/'apps/openagents-web/icecream-face.c',ROOT/'coax/web/render-face.c']
 for index,unit in enumerate(units):
  dep=run([clang,'--target=wasm32-wasip1','--sysroot='+str(sysroot),'-resource-dir',resources,'-std=c11','-O2','-ffp-contract=off','-fno-fast-math','-I',ADMITTED,'-I',ROOT/'apps/openagents-web','-M',unit],f'c-includes-{index}.d')
  paths=[Path(p).resolve()for p in shlex.split(dep.read_text().replace('\\\n',' ').split(':',1)[1])];c_graph[str(unit)]=[str(p)for p in paths]
  for p in paths:
   if p.is_relative_to(ROOT):inputs.add(str(p.relative_to(ROOT)))
   elif p.is_relative_to(ADMITTED):assert emitted[p.name]==sha(p)
   else:
    assert str(p)in external and external[str(p)]==sha(p),str(p)
    external_used[str(p)]=external[str(p)]
 record['c_include_graph']=c_graph;record['external_header_sha256']=external_used
 missing=sorted(n for n in inputs if n not in before);assert not missing,missing
 record['inputs']={n:before[n]for n in sorted(inputs)};record['source_changes']=[n for n,h in record['inputs'].items()if sha(ROOT/n)!=h]
 record['compiler_changed']=sha(COAX)!=record['compiler']['sha256'];assert not record['source_changes']and not record['compiler_changed']
 assert all(sha(ADMITTED/n)==h for n,h in emitted.items())
 record['wasm_sha256']=sha(WORK/'icecream.wasm');record['helper_sha256']=sha(Path(__file__))
 with tarfile.open(WORK/'source-closure.tar.gz','w:gz')as tar:
  for n in sorted(inputs):tar.add(ROOT/n,arcname=n)
 (WORK/'include-graph.json').write_text(json.dumps({'coax':by_unit,'c':c_graph,'external_headers':external_used},indent=2)+'\n')
 record['source_archive_sha256']=sha(WORK/'source-closure.tar.gz');record['status']='pass'
except BaseException as error:
 record['error']=f'{type(error).__name__}:{error}';raise
finally:
 kill_group();record['elapsed_seconds']=round(time.monotonic()-started,6)
 (WORK/'qualification.json').write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps({'status':record['status'],'wasm_sha256':record['wasm_sha256'],'elapsed_seconds':record['elapsed_seconds'],'work':str(WORK)}))
