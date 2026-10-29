#!/usr/bin/env python3
"""Experimental owned ECDH host: checked/release, native/sanitized/Wasm."""
import hashlib,json,pathlib,subprocess,sys,tempfile
ROOT=pathlib.Path(__file__).resolve().parents[3];C=ROOT/'coax/bin/coax';P=ROOT/'coax/lib/crypto/platform';T=ROOT/'coax/tests/nostr'
def run(*args):subprocess.run([str(x) for x in args],check=True)
if sys.argv[1:] not in ([],['--direct']):raise SystemExit('usage: ecdh_owned.py [--direct]')
direct=sys.argv[1:]==['--direct']
inputs=[C,ROOT/'coax/lib/crypto/ecdh.coax',ROOT/'coax/lib/crypto/ecdh_owned_adapter.coax',ROOT/'coax/lib/crypto/zeroize.coax',ROOT/'coax/lib/crypto/ecdh_lifecycle.coax',P/'zeroize.c',P/'ecdh_owned.c',P/'ecdh_owned.h',T/'ecdh_owned_fixture.c',T/'ecdh_host_wasm.cjs']
hashes={str(p):hashlib.sha256(p.read_bytes()).hexdigest() for p in inputs}
with tempfile.TemporaryDirectory(prefix='coax-ecdh-owned-') as tmp:
 d=pathlib.Path(tmp)
 for mode,flags in [('checked',[]),('release',['--release'])]:
  sources=[]
  for name,src in [('ecdh','ecdh_owned_adapter.coax'),('zeroize-policy','zeroize.coax'),('ecdh_lifecycle','ecdh_lifecycle.coax')]:
   out=d/(name+'.c');mapping=d/(name+'-map.json');run(C,'c',ROOT/'coax/lib/crypto'/src,'--library','--header',d/(name+'.h'),'--source-map',mapping,'-o',out,*flags);sources.append(out)
   for path,digest in json.loads(mapping.read_text())['source_hashes'].items():
    if hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()!=digest:raise RuntimeError('compiler source-map mismatch')
    if path in hashes and hashes[path]!=digest:raise RuntimeError('source differs between compilations')
    hashes[path]=digest
  sources += [P/'zeroize.c',P/'ecdh_owned.c',T/'ecdh_owned_fixture.c'];common=['-std=c11',*([] if direct else ['-DCOAX_ECDH_OWNED_TEST_EFFECT']),'-I'+str(d),'-I'+str(P)]
  for name,opts in [('native',['-O2']),('asan',['-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer'])]:
   exe=d/(mode+'-'+name);run('cc',*common,*opts,*sources,'-pthread','-lm','-o',exe);run(exe);print(mode,name,'owned host fixture passed',flush=True)
  wasm=d/(mode+'.wasm');run('/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes',*common,'-O2',*sources,'-Wl,-z,stack-size=1048576','-lm','-o',wasm);run('node',T/'ecdh_host_wasm.cjs',wasm);print(mode,'Wasm owned host fixture passed',flush=True)
if any(hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()!=digest for p,digest in hashes.items()):raise RuntimeError('test input drift')
print('input hashes',hashes,flush=True)
