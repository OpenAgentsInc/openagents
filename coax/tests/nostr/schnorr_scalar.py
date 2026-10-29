#!/usr/bin/env python3
"""Independent integer model for private scalar admission and parity."""
if not __debug__: raise SystemExit('tests require Python assertions')
import ctypes as C,hashlib,json,os,random,subprocess,tempfile
from pathlib import Path
R=Path(__file__).resolve().parents[3];compiler=R/'coax/bin/coax';platform=R/'coax/lib/crypto/platform'
order=0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141;rng=random.Random(3401)
values=[0,1,2,order-1,order,order+1,(1<<256)-1]+[rng.getrandbits(256) for _ in range(1000)];cases=[]
for value in values:
 valid=0<value<order;safe=value if valid else 1
 cases.append({'key':list(value.to_bytes(32,'big')),'valid':int(valid),'even':list(safe.to_bytes(32,'big')),'odd':list(((order-safe)%order).to_bytes(32,'big'))})
with tempfile.TemporaryDirectory(prefix='coax-sign-scalar-') as temp:
 d=Path(temp);fixtures=d/'cases.json';fixtures.write_text(json.dumps(cases))
 for mode,flags in [('checked',[]),('release',['--release'])]:
  c=d/(mode+'.c');z=d/(mode+'-zero.c')
  c.write_bytes(subprocess.run([str(compiler),'c',str(R/'coax/tests/nostr/schnorr_scalar_adapter.coax'),'--library','--header',str(d/'scalar-owned.h'),*flags],capture_output=True,check=True).stdout)
  z.write_bytes(subprocess.run([str(compiler),'c',str(R/'coax/lib/crypto/zeroize.coax'),'--library','--header',str(d/'zeroize-policy.h'),*flags],capture_output=True,check=True).stdout)
  common=['-I'+str(d),'-I'+str(platform),str(c),str(z),str(platform/'zeroize.c'),str(R/'coax/tests/nostr/schnorr_scalar_fixture.c')]
  lib=d/(mode+'.dylib');subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC',*common,'-o',str(lib)],check=True);obj=C.CDLL(str(lib))
  for name in ['ss_input','ss_output']:getattr(obj,name).restype=C.POINTER(C.c_uint8)
  obj.ss_valid.restype=C.c_uint64;assert obj.ss_refusal()==1
  for case in cases:
   for i,b in enumerate(case['key']):obj.ss_input()[i]=b
   for odd in [0,1]:assert obj.ss_run(odd)==1 and obj.ss_valid()==case['valid'] and list(obj.ss_output()[:32])==case['odd' if odd else 'even']
  main=d/'main.c';main.write_text('int ss_run(int),ss_refusal(void);int main(void){return !ss_refusal()||!ss_run(0)||!ss_run(1);}\n')
  exe=d/'asan';subprocess.run(['cc','-std=c11','-O1','-fsanitize=address,undefined',*common,str(main),'-o',str(exe)],check=True);subprocess.run([str(exe)],check=True)
  sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip();subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2','-c',str(c),'-o',str(d/'ios.o')],check=True)
  sim=os.environ.get('COAX_IOS_SIMULATOR')
  if sim:
   ios=d/(mode+'-ios');subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2',*common,str(R/'coax/tests/nostr/schnorr_scalar_ios.c'),'-o',str(ios)],check=True)
   subprocess.run(['xcrun','simctl','spawn',sim,str(ios)],check=True);print(mode,'iOS Simulator scalar/parity/refusal',hashlib.sha256(ios.read_bytes()).hexdigest(),flush=True)
  wasm=d/(mode+'.wasm');exports='-Wl,--export=ss_input,--export=ss_output,--export=ss_valid,--export=ss_run,--export=ss_refusal'
  subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-mexec-model=reactor',*common,exports,'-o',str(wasm)],check=True)
  subprocess.run(['node',str(R/'coax/tests/nostr/schnorr_scalar_wasm.cjs'),str(wasm),str(fixtures)],check=True)
  print(mode,len(cases),'independent cases, both parities and workspace wipes; C',hashlib.sha256(c.read_bytes()).hexdigest(),'Wasm',hashlib.sha256(wasm.read_bytes()).hexdigest(),flush=True)
