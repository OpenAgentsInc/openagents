from pathlib import Path
import concurrent.futures,hashlib,json,subprocess,os,signal,time,shutil
ROOT=Path('/Users/christopherdavid/work/openagents-aethyr-book');OLD=Path('/tmp/aethyr-ch11-grid-qualified');OUT=Path('/tmp/aethyr-ch11-grid-stack-4m')
OUT.mkdir(exist_ok=False);(OUT/'admitted').mkdir();(OUT/'objects').mkdir()
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
old=json.load(open(OLD/'qualification.json'));orig_sources=old['inputs']
assert all(sha(OLD/'admitted'/p)==h for p,h in old['emitted'].items())
before={p:sha(ROOT/p) for p in orig_sources if p!='coax/web/project/build-home.sh'}
assert all(h==orig_sources[p] for p,h in before.items()),[p for p,h in before.items() if h!=orig_sources[p]]
before['coax/web/project/build-home.sh']=sha(ROOT/'coax/web/project/build-home.sh')
for p,h in old['emitted'].items():shutil.copyfile(OLD/'admitted'/p,OUT/'admitted'/p)
query=['bash','-c','source "$1"; printf "%s\\0%s\\0%s\\0%s\\0" "$COAXIAL_CLANG" "$COAXIAL_SYSROOT" "$COAXIAL_RESOURCES" "$PATH"','--',str(ROOT/'coax/web/toolchain.sh')]
cc,sysroot,resources,task_path=subprocess.check_output(query,cwd=ROOT,timeout=10).decode().split('\0')[:4]
platform=['--target=wasm32-wasip1','--sysroot='+sysroot,'-resource-dir='+resources]
cmds=[];begin=time.monotonic();deadline=begin+120
def run(cmd,name):
 log=OUT/(name+'.log');start=time.monotonic()
 with log.open('wb') as f:
  p=subprocess.Popen(cmd,cwd=ROOT,stdout=f,stderr=f,start_new_session=True,env=dict(os.environ,PATH=task_path))
  try:code=p.wait(timeout=max(.1,min(60,deadline-time.monotonic())))
  except BaseException:os.killpg(p.pid,signal.SIGKILL);p.wait();raise
 row={'command':cmd,'returncode':code,'elapsed_seconds':time.monotonic()-start,'log':str(log),'log_sha256':sha(log)};cmds.append(row)
 assert code==0,row
units=[('newland',OUT/'admitted/newland.c',False),('landing-browser',OUT/'admitted/landing-browser.c',False),('landing-fold',OUT/'admitted/landing-fold.c',False),('Gridwalk',OUT/'admitted/Gridwalk.c',False)]
units += [('newland-face',ROOT/'apps/openagents-web/newland-face.c',True)]
units += [(n+'-face',ROOT/'coax/web/project'/(n+'-face.c'),True) for n in ['landing-reader','landing-fold','landing-cache']]
units += [('Render-face',ROOT/'coax/web/render-face.c',True)]
def build(u):
 n,src,face=u;obj=OUT/'objects'/(n+'.o');flags=['-std=c11','-O2']
 flags+=['-I'+str(OUT/'admitted'),'-I'+str(src.parent)] if face else ['-ffp-contract=off','-fno-fast-math','-Wno-unused-function','-Wno-unused-variable']
 run([cc,*platform,*flags,'-c',str(src),'-o',str(obj)],'compile-'+n);return str(obj)
status='failed';error=None
try:
 with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:objects=list(pool.map(build,units))
 link=[cc,*platform,'-O2','-mexec-model=reactor',*objects,'-Wl,--import-memory,--max-memory=67108864,-z,stack-size=4194304,--export=__heap_base,--gc-sections,--strip-debug','-o',str(OUT/'newland.wasm')]
 run(link,'link')
 changed=[p for p,h in before.items() if sha(ROOT/p)!=h];assert not changed,changed
 assert all(sha(OUT/'admitted'/p)==h for p,h in old['emitted'].items())
 status='passed'
except Exception as e:error=str(e)
record={'schema':'aethyr.grid.retained-coax-stack-relink.v1','status':status,'error':error,'elapsed_seconds':time.monotonic()-begin,
 'original_build_receipt':str(OLD/'qualification.json'),'original_build_receipt_sha256':sha(OLD/'qualification.json'),'original_wasm_sha256':old['wasm_sha256'],
 'compiler_sha256':old['compiler_sha256'],'coax_reemitted':False,'coax_checks_repeated':False,'generated_sha256':old['emitted'],'current_inputs':before,
 'source_changes':[p for p,h in before.items() if sha(ROOT/p)!=h],
 'stack_bytes':4194304,'memory_max_bytes':67108864,'commands':cmds,'clang':cc,'clang_sha256':sha(cc),'wasm_sha256':sha(OUT/'newland.wasm') if status=='passed' else None,
 'scope':'Original build-home ordinary C flags and object order, exact retained Coax C/headers and current source-fenced effects; only the Grid/homepage stack reservation changes8MiB→4MiB. Source remains320rows;512growth is a separately labelled prediction.'}
(OUT/'qualification.json').write_text(json.dumps(record,indent=2)+'\n');print(json.dumps({k:record[k] for k in ['status','error','elapsed_seconds','source_changes','wasm_sha256']}))
if status!='passed':raise SystemExit(1)
