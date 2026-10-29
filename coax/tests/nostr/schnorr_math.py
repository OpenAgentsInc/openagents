#!/usr/bin/env python3
"""Integer oracle for the isolated BIP-340 scalar equation, no real keys."""
if not __debug__: raise SystemExit('tests require Python assertions')
import ctypes as C,hashlib,json,os,random,subprocess,tempfile
from pathlib import Path
R=Path(__file__).resolve().parents[3];compiler=R/'coax/bin/coax';platform=R/'coax/lib/crypto/platform'
n=0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141;rng=random.Random(3402);cases=[]
values=[0,1,2,n-1,n,n+1,(1<<256)-1]
for index in range(507):
 d=values[index] if index<len(values) else rng.getrandbits(256);k=values[(index*3)%len(values)] if index<len(values) else rng.getrandbits(256);e=rng.getrandbits(256)%n
 dv=0<d<n;kr=k%n;valid=int(dv and kr!=0);ds=d if dv else 1;ks=kr if kr else 1
 expected=[list((((-ks if np else ks)+e*(-ds if kp else ds))%n).to_bytes(32,'big')) for kp in [0,1] for np in [0,1]]
 cases.append({'key':list(d.to_bytes(32,'big')),'nonce':list(k.to_bytes(32,'big')),'challenge':list(e.to_bytes(32,'big')),'valid':valid,'expected':expected})
with tempfile.TemporaryDirectory(prefix='coax-sign-math-') as temp:
 d=Path(temp);fixtures=d/'cases.json';fixtures.write_text(json.dumps(cases))
 for mode,flags in [('checked',[]),('release',['--release'])]:
  c=d/(mode+'.c');z=d/(mode+'-zero.c')
  c.write_bytes(subprocess.run([str(compiler),'c',str(R/'coax/lib/crypto/schnorr_math_workspace.coax'),'--library','--header',str(d/'math-owned.h'),*flags],capture_output=True,check=True).stdout)
  z.write_bytes(subprocess.run([str(compiler),'c',str(R/'coax/lib/crypto/zeroize.coax'),'--library','--header',str(d/'zeroize-policy.h'),*flags],capture_output=True,check=True).stdout)
  common=['-I'+str(d),'-I'+str(platform),str(c),str(z),str(platform/'zeroize.c'),str(R/'coax/tests/nostr/schnorr_math_fixture.c')]
  lib=d/(mode+'.dylib');subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC',*common,'-o',str(lib)],check=True);obj=C.CDLL(str(lib))
  for name in ['sm_key','sm_nonce','sm_challenge','sm_output']:getattr(obj,name).restype=C.POINTER(C.c_uint8)
  obj.sm_valid.restype=C.c_uint64;assert obj.sm_refusal()==1
  for case in cases:
   for name in ['key','nonce','challenge']:
    for i,b in enumerate(case[name]):getattr(obj,'sm_'+name)()[i]=b
   for kp in [0,1]:
    for np in [0,1]:assert obj.sm_run(kp,np)==1 and obj.sm_valid()==case['valid'] and list(obj.sm_output()[:32])==case['expected'][kp*2+np]
  main=d/'main.c';main.write_text('int sm_run(int,int),sm_refusal(void);int main(void){return !sm_refusal()||!sm_run(0,0)||!sm_run(1,1);}\n')
  exe=d/'asan';subprocess.run(['cc','-std=c11','-O1','-fsanitize=address,undefined',*common,str(main),'-o',str(exe)],check=True);subprocess.run([str(exe)],check=True)
  sim=os.environ.get('COAX_IOS_SIMULATOR');sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip()
  if sim:
   ios=d/(mode+'-ios');subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2',*common,str(R/'coax/tests/nostr/schnorr_math_ios.c'),'-o',str(ios)],check=True)
   subprocess.run(['xcrun','simctl','spawn',sim,str(ios)],check=True);print(mode,'iOS Simulator equation/refusal',hashlib.sha256(ios.read_bytes()).hexdigest(),flush=True)
  wasm=d/(mode+'.wasm');exports='-Wl,--export=sm_key,--export=sm_nonce,--export=sm_challenge,--export=sm_output,--export=sm_valid,--export=sm_run,--export=sm_refusal'
  subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-mexec-model=reactor',*common,exports,'-o',str(wasm)],check=True)
  subprocess.run(['node',str(R/'coax/tests/nostr/schnorr_math_wasm.cjs'),str(wasm),str(fixtures)],check=True)
  print(mode,len(cases),'independent cases, four parities and workspace wipes; C',hashlib.sha256(c.read_bytes()).hexdigest(),'Wasm',hashlib.sha256(wasm.read_bytes()).hexdigest(),flush=True)
