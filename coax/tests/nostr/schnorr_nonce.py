#!/usr/bin/env python3
"""Independent BIP-340 tagged-hash nonce oracle; no real keys are used."""
if not __debug__: raise SystemExit('tests require Python assertions')
import ctypes as C,hashlib,json,os,random,subprocess,tempfile
from pathlib import Path
R=Path(__file__).resolve().parents[3];compiler=R/'coax/bin/coax';platform=R/'coax/lib/crypto/platform'
def tagged(tag,msg):
 h=hashlib.sha256(tag).digest();return hashlib.sha256(h+h+msg).digest()
def nonce(key,pub,aux,msg):
 t=bytes(a^b for a,b in zip(key,tagged(b'BIP0340/aux',aux)));return tagged(b'BIP0340/nonce',t+pub+msg)
rng=random.Random(3402026);cases=[]
for n in [0,1,31,32,33,55,56,63,64,65,127,128,129,1024,1048576]+[rng.randrange(1025) for _ in range(100)]:
 key=rng.randbytes(32);pub=rng.randbytes(32);aux=rng.randbytes(32);msg=rng.randbytes(n);cases.append({'key':list(key),'pub':list(pub),'aux':list(aux),'message':list(msg),'expected':list(nonce(key,pub,aux,msg))})
with tempfile.TemporaryDirectory(prefix='coax-sn-nonce-') as temp:
 d=Path(temp);fixtures=d/'cases.json';fixtures.write_text(json.dumps(cases))
 for mode,flags in [('checked',[]),('release',['--release'])]:
  c=d/(mode+'.c');z=d/(mode+'-zero.c')
  c.write_bytes(subprocess.run([str(compiler),'c',str(R/'coax/lib/crypto/schnorr_nonce_workspace.coax'),'--library','--header',str(d/'nonce-owned.h'),*flags],capture_output=True,check=True).stdout)
  z.write_bytes(subprocess.run([str(compiler),'c',str(R/'coax/lib/crypto/zeroize.coax'),'--library','--header',str(d/'zeroize-policy.h'),*flags],capture_output=True,check=True).stdout)
  common=['-I'+str(d),'-I'+str(platform),str(c),str(z),str(platform/'zeroize.c'),str(R/'coax/tests/nostr/schnorr_nonce_fixture.c')]
  lib=d/(mode+'.dylib');subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC',*common,'-o',str(lib)],check=True);obj=C.CDLL(str(lib))
  for name in ['sn_key','sn_pub','sn_aux','sn_message','sn_output']:getattr(obj,name).restype=C.POINTER(C.c_uint8)
  for case in cases:
   for name in ['key','pub','aux','message']:
    for i,b in enumerate(case[name]):getattr(obj,'sn_'+name)()[i]=b
   assert obj.sn_run(len(case['message']))==1 and list(obj.sn_output()[:32])==case['expected']
  for i in range(5):assert obj.sn_refusal(i)==1
  main=d/'main.c';main.write_text('int sn_run(int),sn_refusal(int);int main(void){for(int i=0;i<5;i++)if(!sn_refusal(i))return 1;return !sn_run(1048576);}\n')
  exe=d/'asan';subprocess.run(['cc','-std=c11','-O1','-fsanitize=address,undefined',*common,str(main),'-o',str(exe)],check=True);subprocess.run([str(exe)],check=True)
  sim=os.environ.get('COAX_IOS_SIMULATOR');sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip()
  if sim:
   ios=d/(mode+'-ios');subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2',*common,str(R/'coax/tests/nostr/schnorr_nonce_ios.c'),'-o',str(ios)],check=True)
   subprocess.run(['xcrun','simctl','spawn',sim,str(ios)],check=True);print(mode,'iOS Simulator vector and refusals',hashlib.sha256(ios.read_bytes()).hexdigest(),flush=True)
  wasm=d/(mode+'.wasm');exports='-Wl,--export=sn_key,--export=sn_pub,--export=sn_aux,--export=sn_message,--export=sn_output,--export=sn_run,--export=sn_refusal'
  subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-mexec-model=reactor',*common,exports,'-o',str(wasm)],check=True)
  subprocess.run(['node',str(R/'coax/tests/nostr/schnorr_nonce_wasm.cjs'),str(wasm),str(fixtures)],check=True)
  print(mode,len(cases),'independent cases and workspace wipes; C',hashlib.sha256(c.read_bytes()).hexdigest(),'Wasm',hashlib.sha256(wasm.read_bytes()).hexdigest(),flush=True)
