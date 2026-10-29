#!/usr/bin/env python3
"""Fixed-base affine x/parity against pinned libsecp and a Python model."""
if not __debug__: raise SystemExit('tests require Python assertions')
import ctypes as C,hashlib,json,os,random,subprocess,sys,tempfile
from pathlib import Path
R=Path(__file__).resolve().parents[3];sys.path.insert(0,str(R/'tools/coax-nostr/fuzz'))
from oracle import N,NativeOracle,DEFAULT_LIBRARY,point_mul,encode_point
compiler=R/'coax/bin/coax';platform=R/'coax/lib/crypto/platform';rng=random.Random(3403)
values=[0,1,2,3,7,N-1,N,N+1,(1<<256)-1]+[rng.getrandbits(256) for _ in range(1000)];cases=[]
ref=NativeOracle(DEFAULT_LIBRARY)
try:
 for i,v in enumerate(values):
  d=v%N;encoded=ref.point(d.to_bytes(32,'big')) if d else None
  if i<32:assert encoded==encode_point(point_mul(d))
  assert (encoded is None)==(d==0)
  cases.append({'scalar':list(v.to_bytes(32,'big')),'finite':int(encoded is not None),'odd':0 if encoded is None else encoded[0]&1,'x':[0]*32 if encoded is None else list(encoded[1:])})
finally:ref.close()
with tempfile.TemporaryDirectory(prefix='coax-sign-point-') as temp:
 d=Path(temp);fixtures=d/'cases.json';fixtures.write_text(json.dumps(cases))
 for mode,flags in [('checked',[]),('release',['--release'])]:
  c=d/(mode+'.c');z=d/(mode+'-zero.c')
  c.write_bytes(subprocess.run([str(compiler),'c',str(R/'coax/lib/crypto/schnorr_point_workspace.coax'),'--library','--header',str(d/'point-owned.h'),'--run-steps','250000000',*flags],capture_output=True,check=True).stdout)
  z.write_bytes(subprocess.run([str(compiler),'c',str(R/'coax/lib/crypto/zeroize.coax'),'--library','--header',str(d/'zeroize-policy.h'),*flags],capture_output=True,check=True).stdout)
  common=['-I'+str(d),'-I'+str(platform),str(c),str(z),str(platform/'zeroize.c'),str(R/'coax/tests/nostr/schnorr_point_fixture.c')]
  lib=d/(mode+'.dylib');subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC',*common,'-o',str(lib)],check=True);obj=C.CDLL(str(lib))
  for name in ['sp_input','sp_output']:getattr(obj,name).restype=C.POINTER(C.c_uint8)
  for name in ['sp_finite','sp_odd']:getattr(obj,name).restype=C.c_uint64
  for case in cases:
   for i,b in enumerate(case['scalar']):obj.sp_input()[i]=b
   assert obj.sp_run()==1 and obj.sp_finite()==case['finite'] and obj.sp_odd()==case['odd'] and list(obj.sp_output()[:32])==case['x']
  main=d/'main.c';main.write_text('int sp_run(void);int main(void){return !sp_run();}\n')
  exe=d/'asan';subprocess.run(['cc','-std=c11','-O1','-fsanitize=address,undefined',*common,str(main),'-o',str(exe)],check=True);subprocess.run([str(exe)],check=True)
  sim=os.environ.get('COAX_IOS_SIMULATOR');sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip()
  if sim:
   ios=d/(mode+'-ios');subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2',*common,str(R/'coax/tests/nostr/schnorr_point_ios.c'),'-o',str(ios)],check=True)
   subprocess.run(['xcrun','simctl','spawn',sim,str(ios)],check=True);print(mode,'iOS Simulator three/infinity',hashlib.sha256(ios.read_bytes()).hexdigest(),flush=True)
  wasm=d/(mode+'.wasm');exports='-Wl,-z,stack-size=1048576,--export=sp_input,--export=sp_output,--export=sp_finite,--export=sp_odd,--export=sp_run'
  subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-mexec-model=reactor',*common,exports,'-o',str(wasm)],check=True)
  subprocess.run(['node',str(R/'coax/tests/nostr/schnorr_point_wasm.cjs'),str(wasm),str(fixtures)],check=True)
  print(mode,len(cases),'pinned-libsecp inputs, first 32 independently modelled, workspace wipes; C',hashlib.sha256(c.read_bytes()).hexdigest(),'Wasm',hashlib.sha256(wasm.read_bytes()).hexdigest(),flush=True)
