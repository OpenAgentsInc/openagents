from pathlib import Path
import hashlib,json,subprocess,concurrent.futures,time,os,signal
ROOT=Path('/Users/christopherdavid/work/openagents-aethyr-book')
ADMIT=Path('/tmp/aethyr-ch11-grid-qualified/admitted')
OUT=Path('/tmp/aethyr-ch11-grid-stack')
CC=Path('/opt/homebrew/opt/llvm/bin/clang')
LD=Path('/opt/homebrew/opt/lld/bin/wasm-ld')
OBJDUMP=Path('/opt/homebrew/opt/llvm/bin/llvm-objdump')
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
src=[(ADMIT/(n+'.c'),False) for n in ['newland','landing-browser','landing-fold','Gridwalk']]
src += [(ROOT/'apps/openagents-web/newland-face.c',True)]
src += [(ROOT/'coax/web/project'/(n+'-face.c'),True) for n in ['landing-reader','landing-fold','landing-cache']]
src += [(ROOT/'coax/web/render-face.c',True)]
rec=json.load(open('/tmp/aethyr-ch11-grid-qualified/qualification.json'))
for n,h in rec['emitted'].items():assert sha(ADMIT/n)==h,n
for p,_ in src:
 rp=str(p.relative_to(ROOT)) if p.is_relative_to(ROOT) else None
 if rp in rec['inputs']:assert sha(p)==rec['inputs'][rp],rp
flags=['--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2']
records=[];started=time.monotonic()
def run(cmd,name,stdout=None):
 t=time.monotonic();log=OUT/(name+'.log')
 with log.open('wb') as f:
  p=subprocess.Popen([str(x) for x in cmd],cwd=ROOT,stdout=f if stdout is None else stdout,stderr=f,start_new_session=True)
  try:code=p.wait(timeout=120)
  except BaseException:
   os.killpg(p.pid,signal.SIGKILL);p.wait();raise
 row={'command':[str(x) for x in cmd],'returncode':code,'elapsed_seconds':time.monotonic()-t,'log':str(log),'log_sha256':sha(log)}
 records.append(row)
 if code:raise RuntimeError(row)
 return row
def build(item):
 p,face=item;name=p.stem;obj=OUT/'objects'/(name+'.o');opts=[]
 if face:opts=['-I'+str(ADMIT),'-I'+str(p.parent)]
 else:opts=['-ffp-contract=off','-fno-fast-math','-Wno-unused-function','-Wno-unused-variable']
 # Reuse the already completed first diagnostic compile instead of repeating it.
 if not obj.exists():run([CC,*flags,*opts,'-fstack-usage','-MD','-MF',str(obj.with_suffix('.d')),'-c',p,'-o',obj],name)
 return obj
with concurrent.futures.ThreadPoolExecutor(max_workers=3) as ex:objects=list(ex.map(build,src))
link=[CC,*flags[:-2],'-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-O2','-mexec-model=reactor',*objects,'-Wl,--import-memory,--max-memory=67108864,-z,stack-size=8388608,--export=__heap_base,--gc-sections,--strip-debug,-Map='+str(OUT/'grid.map'),'-o',OUT/'grid-map.wasm']
run(link,'link')
with (OUT/'grid.disasm').open('wb') as f:run([OBJDUMP,'-d',OUT/'grid-map.wasm'],'disassemble',stdout=f)
for p,_ in src:
 rp=str(p.relative_to(ROOT)) if p.is_relative_to(ROOT) else None
 if rp in rec['inputs']:assert sha(p)==rec['inputs'][rp],rp
result={'schema':'aethyr.grid.stack-static-analysis.v1','scope':'C-only diagnostics using exact admitted Coax C/H and original production C compile/link flags plus stack-usage/map metadata; no Coax emission or checks rerun.','clang_sha256':sha(CC),'wasm_ld_sha256':sha(LD),'objdump_sha256':sha(OBJDUMP),'source_sha256':{str(p):sha(p) for p,_ in src},'admitted_sha256':rec['emitted'],'qualified_wasm_sha256':rec['wasm_sha256'],'diagnostic_wasm_sha256':sha(OUT/'grid-map.wasm'),'elapsed_seconds':time.monotonic()-started,'commands':records}
(OUT/'qualification.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:result[k] for k in ['elapsed_seconds','qualified_wasm_sha256','diagnostic_wasm_sha256']}))
