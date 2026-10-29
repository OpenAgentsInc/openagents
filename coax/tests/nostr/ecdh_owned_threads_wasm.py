#!/usr/bin/env python3
"""Test-only shared-memory Wasm owner contention, checked and release."""
import hashlib,json,pathlib,platform,shutil,subprocess,sys,tempfile
ROOT=pathlib.Path(__file__).resolve().parents[3];C=ROOT/'coax/bin/coax';P=ROOT/'coax/lib/crypto/platform';T=ROOT/'coax/tests/nostr'
def run(*args):
 result=subprocess.run([str(x) for x in args],capture_output=True,text=True,timeout=60)
 if result.returncode:raise RuntimeError('probe failed: '+str(args[0])+': '+result.stderr[-3000:])
 return result.stdout
def sha(p):return hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
clang=pathlib.Path('/opt/homebrew/opt/llvm/bin/clang');linker=pathlib.Path(shutil.which('wasm-ld'));node=pathlib.Path(shutil.which('node'))
inputs=[C,clang,linker,node,pathlib.Path(sys.executable),P/'zeroize.c',P/'ecdh_owned.c',P/'ecdh_owned.h',T/'ecdh_owned_threads_wasm.c',T/'ecdh_owned_threads_wasm.cjs',pathlib.Path(__file__).resolve()];hashes={str(p):sha(p) for p in inputs};results=[]
if sys.argv[1:] not in ([],['--negative']):raise SystemExit('usage: ecdh_owned_threads_wasm.py [--negative]')
negative=sys.argv[1:]==['--negative']
with tempfile.TemporaryDirectory(prefix='coax-owned-wasm-threads-') as td:
 d=pathlib.Path(td)
 host=P/'ecdh_owned.c';negative_host_sha=None
 if negative:
  text=host.read_text();before='static int lock_slot(void){return !atomic_flag_test_and_set_explicit(&mutex,memory_order_acquire);}'
  assert text.count(before)==1;host=d/'unlocked.c';host.write_text(text.replace(before,'static int lock_slot(void){return 1;}'));negative_host_sha=sha(host)
 for mode,flags in [('checked',[]),('release',['--release'])]:
  sources=[]
  for name,src in [('ecdh','ecdh_owned_adapter.coax'),('zeroize-policy','zeroize.coax'),('ecdh_lifecycle','ecdh_lifecycle.coax')]:
   out=d/(name+'.c');mapping=d/(name+'.map.json');run(C,'c',ROOT/'coax/lib/crypto'/src,'--library','--header',d/(name+'.h'),'--source-map',mapping,'-o',out,*flags);sources.append(out)
   for path,digest in json.loads(mapping.read_text())['source_hashes'].items():
    if sha(path)!=digest or path in hashes and hashes[path]!=digest:raise RuntimeError('source identity drift')
    hashes[path]=digest
  wasm=d/(mode+'.wasm');sources += [P/'zeroize.c',host,T/'ecdh_owned_threads_wasm.c'];exports=['coax_ecdh_owned_'+n for n in ('acquire','start','collect','retry','cancel','close')]+['gate_address','__stack_pointer','__heap_base']
  run(clang,'--target=wasm32-wasip1-threads','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-pthread','-matomics','-mbulk-memory','-DCOAX_ECDH_OWNED_TEST_EFFECT','-I'+str(d),'-I'+str(P),*sources,'-mexec-model=reactor','-Wl,--import-memory,--export-memory,--shared-memory,--initial-memory=16777216,--max-memory=16777216,-z,stack-size=1048576',*[f'-Wl,--export={x}' for x in exports],'-lm','-o',wasm)
  if negative:
   result=subprocess.run(['node',str(T/'ecdh_owned_threads_wasm.cjs'),str(wasm)],capture_output=True,text=True,timeout=30)
   if result.returncode==0 or 'concurrent operation did not report busy' not in result.stderr:raise RuntimeError('unlocked negative control did not trigger expected contention refusal: '+result.stderr[-3000:])
   observed={'unlocked_control':'detected expected contention violation'}
  else:observed=json.loads(run('node',T/'ecdh_owned_threads_wasm.cjs',wasm))
  results.append({'mode':mode,'wasm_sha256':sha(wasm),'observation':observed})
for path,digest in hashes.items():
 if sha(path)!=digest:raise RuntimeError('input changed during probe')
print(json.dumps({'schema':'coax.ecdh-shared-wasm-contention.v1','scope':'test-only shared-memory target; public artifact is unshared and target/production approval is not implied','negative_control':negative,'negative_host_sha256':negative_host_sha,'platform':platform.platform(),'compiler_version':run(C,'version').strip(),'clang_version':run(clang,'--version').splitlines()[0],'inputs':hashes,'modes':results},indent=2))
