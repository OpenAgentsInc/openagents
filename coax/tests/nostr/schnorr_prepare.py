#!/usr/bin/env python3
"""Key preparation against pinned libsecp and integer parity normalization."""
if not __debug__: raise SystemExit('tests require Python assertions')
import ctypes as C,hashlib,json,os,random,subprocess,sys,tempfile
from pathlib import Path
R=Path(__file__).resolve().parents[3];sys.path.insert(0,str(R/'tools/coax-nostr/fuzz'))
from oracle import N,NativeOracle,DEFAULT_LIBRARY,point_mul,encode_point
compiler=R/'coax/bin/coax';platform=R/'coax/lib/crypto/platform';rng=random.Random(3404)
values=[0,1,2,3,N-3,N-1,N,N+1,(1<<256)-1]+[rng.getrandbits(256) for _ in range(500)];cases=[];ref=NativeOracle(DEFAULT_LIBRARY)
try:
 for i,v in enumerate(values):
  valid=0<v<N;safe=v if valid else 1;encoded=ref.point(safe.to_bytes(32,'big'));assert encoded
  if i<32:assert encoded==encode_point(point_mul(safe))
  normalized=N-safe if encoded[0]&1 else safe
  cases.append({'key':list(v.to_bytes(32,'big')),'valid':int(valid),'public':list(encoded[1:]),'normalized':list(normalized.to_bytes(32,'big'))})
finally:ref.close()
with tempfile.TemporaryDirectory(prefix='coax-sign-prepare-') as temp:
 d=Path(temp);fixtures=d/'cases.json';fixtures.write_text(json.dumps(cases))
 for mode,flags in [('checked',[]),('release',['--release'])]:
  c=d/(mode+'.c');z=d/(mode+'-zero.c')
  c.write_bytes(subprocess.run([str(compiler),'c',str(R/'coax/lib/crypto/schnorr_prepare_workspace.coax'),'--library','--header',str(d/'prepare-owned.h'),'--run-steps','250000000',*flags],capture_output=True,check=True).stdout)
  z.write_bytes(subprocess.run([str(compiler),'c',str(R/'coax/lib/crypto/zeroize.coax'),'--library','--header',str(d/'zeroize-policy.h'),*flags],capture_output=True,check=True).stdout)
  common=['-I'+str(d),'-I'+str(platform),str(c),str(z),str(platform/'zeroize.c'),str(R/'coax/tests/nostr/schnorr_prepare_fixture.c')]
  lib=d/(mode+'.dylib');subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC',*common,'-o',str(lib)],check=True);obj=C.CDLL(str(lib))
  for name in ['sk_key','sk_public','sk_normalized']:getattr(obj,name).restype=C.POINTER(C.c_uint8)
  obj.sk_valid.restype=C.c_uint64;assert obj.sk_refusal()==1
  for case in cases:
   for i,b in enumerate(case['key']):obj.sk_key()[i]=b
   assert obj.sk_run()==1 and obj.sk_valid()==case['valid'] and list(obj.sk_public()[:32])==case['public'] and list(obj.sk_normalized()[:32])==case['normalized']
  main=d/'main.c';main.write_text('int sk_run(void),sk_refusal(void);int main(void){return !sk_refusal()||!sk_run();}\n')
  exe=d/'asan';subprocess.run(['cc','-std=c11','-O1','-fsanitize=address,undefined',*common,str(main),'-o',str(exe)],check=True);subprocess.run([str(exe)],check=True)
  sim=os.environ.get('COAX_IOS_SIMULATOR');sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip()
  if sim:
   ios=d/(mode+'-ios');subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2',*common,str(R/'coax/tests/nostr/schnorr_prepare_ios.c'),'-o',str(ios)],check=True)
   subprocess.run(['xcrun','simctl','spawn',sim,str(ios)],check=True);print(mode,'iOS Simulator key/invalid',hashlib.sha256(ios.read_bytes()).hexdigest(),flush=True)
  wasm=d/(mode+'.wasm');exports='-Wl,-z,stack-size=1048576,--export=sk_key,--export=sk_public,--export=sk_normalized,--export=sk_valid,--export=sk_run,--export=sk_refusal'
  subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-mexec-model=reactor',*common,exports,'-o',str(wasm)],check=True)
  subprocess.run(['node',str(R/'coax/tests/nostr/schnorr_prepare_wasm.cjs'),str(wasm),str(fixtures)],check=True)
  print(mode,len(cases),'pinned-libsecp and integer cases, test-host workspace wipes; C',hashlib.sha256(c.read_bytes()).hexdigest(),'Wasm',hashlib.sha256(wasm.read_bytes()).hexdigest(),flush=True)
