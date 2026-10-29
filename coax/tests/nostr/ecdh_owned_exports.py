#!/usr/bin/env python3
"""Build bounded public owner artifacts and check native/Wasm export surfaces."""
import hashlib,json,pathlib,subprocess,tempfile
ROOT=pathlib.Path(__file__).resolve().parents[3];C=ROOT/'coax/bin/coax';P=ROOT/'coax/lib/crypto/platform'
API=['coax_ecdh_owned_'+x for x in ('acquire','start','collect','retry','cancel','close')]
def run(*args):return subprocess.run([str(x) for x in args],check=True,capture_output=True,text=True).stdout
def sha(p):return hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
inputs=[C,P/'ecdh_owned.c',P/'ecdh_owned.h',P/'zeroize.c',pathlib.Path(__file__)]
hashes={str(p):sha(p) for p in inputs};result={'schema':'coax.ecdh-owner-export-probe.v1','scope':'export surface only; no target timing, ownership or production approval','modes':[]}
with tempfile.TemporaryDirectory(prefix='coax-ecdh-exports-') as td:
 d=pathlib.Path(td)
 for mode,flags in [('checked',[]),('release',['--release'])]:
  sources=[]
  for name,src in [('ecdh','ecdh_owned_adapter.coax'),('zeroize-policy','zeroize.coax'),('ecdh_lifecycle','ecdh_lifecycle.coax')]:
   out=d/(name+'.c');mapping=d/(name+'.map.json');run(C,'c',ROOT/'coax/lib/crypto'/src,'--library','--header',d/(name+'.h'),'--source-map',mapping,'-o',out,*flags);sources.append(out)
   for path,digest in json.loads(mapping.read_text())['source_hashes'].items():
    if sha(path)!=digest or path in hashes and hashes[path]!=digest:raise RuntimeError('source map drift')
    hashes[path]=digest
  public=(P/'ecdh_owned.h').read_text();private=(d/'ecdh.h').read_text()
  if 'ecdh_stage_bridge' not in private or 'ecdh_stage_bridge' in public or 'ecdh_stage(' in public or 'coax_ecdh_raw(' in public:raise RuntimeError('header surface')
  common=['-std=c11','-O2','-I'+str(d),'-I'+str(P)];sources += [P/'zeroize.c',P/'ecdh_owned.c']
  native=d/'libcoax_ecdh_owned.dylib';run('cc',*common,'-shared','-fPIC','-fvisibility=hidden','-Wl,-install_name,@rpath/libcoax_ecdh_owned.dylib',*sources,'-lm','-o',native)
  symbols={line.split()[-1].removeprefix('_') for line in run('nm','-gU',native).splitlines() if line.split()}
  if symbols!=set(API):raise RuntimeError(('native symbols',sorted(symbols)))
  consumer=d/(mode+'-consumer.cpp');consumer.write_text('#include "ecdh_owned.h"\nint main(){auto h=coax_ecdh_owned_acquire();return h && !coax_ecdh_owned_close(h) ? 0 : 1;}\n')
  executable=d/(mode+'-consumer');run('c++','-std=c++17','-I'+str(P),consumer,native,'-Wl,-rpath,'+str(d),'-o',executable);run(executable)
  negative=d/(mode+'-exposed.dylib');run('cc',*common,'-shared','-fPIC',*sources,'-lm','-o',negative)
  unsafe={line.split()[-1].removeprefix('_') for line in run('nm','-gU',negative).splitlines() if line.split()}
  if 'ecdh_stage_bridge' not in unsafe:raise RuntimeError('native negative control failed')
  wasi=['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes',*common,*sources,'-mexec-model=reactor','-Wl,--export-memory,--export=malloc,--export=free,-z,stack-size=1048576',*[f'-Wl,--export={name}' for name in API],'-lm']
  wasm=d/(mode+'.wasm');run(*wasi,'-o',wasm)
  js='const fs=require("fs");let m=new WebAssembly.Module(fs.readFileSync(process.argv[1]));console.log(JSON.stringify(WebAssembly.Module.exports(m).map(x=>x.name)))'
  exports=set(json.loads(run('node','-e',js,wasm)))
  if exports!=set(API)|{'memory','_initialize','malloc','free'}:raise RuntimeError(('Wasm exports',sorted(exports)))
  negative_wasm=d/(mode+'-exposed.wasm');run(*wasi,'-Wl,--export=ecdh_stage_bridge','-o',negative_wasm)
  if 'ecdh_stage_bridge' not in json.loads(run('node','-e',js,negative_wasm)):raise RuntimeError('Wasm negative control failed')
  result['modes'].append({'mode':mode,'native_symbols':sorted(symbols),'wasm_exports':sorted(exports),'native_sha256':sha(native),'wasm_sha256':sha(wasm),'negative_controls':'raw bridge exposed when protection is removed'})
for path,digest in hashes.items():
 if sha(path)!=digest:raise RuntimeError('source or tool drift')
result['inputs']=hashes;print(json.dumps(result,indent=2))
