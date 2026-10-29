#!/usr/bin/env python3
"""Tagged hash and nonce point against hashlib, pinned libsecp and Python EC."""
if not __debug__: raise SystemExit('tests require Python assertions')
import ctypes as C,hashlib,json,os,random,subprocess,sys,tempfile
from pathlib import Path
R=Path(__file__).resolve().parents[3];sys.path.insert(0,str(R/'tools/coax-nostr/fuzz'))
from oracle import N,NativeOracle,DEFAULT_LIBRARY,point_mul,encode_point,tagged
compiler=R/'coax/bin/coax';platform=R/'coax/lib/crypto/platform';rng=random.Random(3405);cases=[];ref=NativeOracle(DEFAULT_LIBRARY)
try:
 for i in range(107):
  d=3 if i==0 else rng.randrange(1,N);point=ref.point(d.to_bytes(32,'big'));assert point
  key=(N-d if point[0]&1 else d).to_bytes(32,'big');pub=point[1:];aux=bytes(32) if i==0 else rng.randbytes(32);m=bytes(32) if i==0 else rng.randbytes(1048576 if i==106 else [0,1,32,64,129,1024][(i-1)%6]);
  a=tagged('BIP0340/aux',aux);t=bytes(x^y for x,y in zip(key,a));h=tagged('BIP0340/nonce',t+pub+m);k=int.from_bytes(h,'big')%N;assert k
  point=ref.point(k.to_bytes(32,'big'));assert point
  if i<16:assert point==encode_point(point_mul(k))
  cases.append({'key':list(key),'pub':list(pub),'aux':list(aux),'message':list(m),'hash':list(h),'r':list(point[1:]),'odd':point[0]&1,'valid':1})
finally:ref.close()
with tempfile.TemporaryDirectory(prefix='coax-nonce-point-') as temp:
 d=Path(temp);fixtures=d/'cases.json';fixtures.write_text(json.dumps(cases))
 for mode,flags in [('checked',[]),('release',['--release'])]:
  c=d/(mode+'.c');z=d/(mode+'-zero.c')
  c.write_bytes(subprocess.run([str(compiler),'c',str(R/'coax/lib/crypto/schnorr_nonce_point.coax'),'--library','--header',str(d/'nonce-point.h'),'--run-steps','250000000',*flags],capture_output=True,check=True).stdout)
  z.write_bytes(subprocess.run([str(compiler),'c',str(R/'coax/lib/crypto/zeroize.coax'),'--library','--header',str(d/'zeroize-policy.h'),*flags],capture_output=True,check=True).stdout)
  common=['-I'+str(d),'-I'+str(platform),str(c),str(z),str(platform/'zeroize.c'),str(R/'coax/tests/nostr/schnorr_nonce_point_fixture.c')]
  lib=d/(mode+'.dylib');subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC',*common,'-o',str(lib)],check=True);obj=C.CDLL(str(lib))
  for name in ['np_key','np_pub','np_aux','np_message','np_r','np_hash']:getattr(obj,name).restype=C.POINTER(C.c_uint8)
  for name in ['np_valid','np_odd']:getattr(obj,name).restype=C.c_uint64
  assert obj.np_refusal()==1
  for case in cases:
   for name in ['key','pub','aux','message']:
    data=bytes(case[name]);C.memmove(getattr(obj,'np_'+name)(),data,len(data))
   assert obj.np_run(len(case['message']))==1 and obj.np_valid()==case['valid'] and obj.np_odd()==case['odd'] and list(obj.np_r()[:32])==case['r'] and list(obj.np_hash()[:32])==case['hash']
  main=d/'main.c';main.write_text('int np_run(int),np_refusal(void);int main(void){return !np_refusal()||!np_run(1048576);}\n')
  exe=d/'asan';subprocess.run(['cc','-std=c11','-O1','-fsanitize=address,undefined',*common,str(main),'-o',str(exe)],check=True);subprocess.run([str(exe)],check=True)
  sim=os.environ.get('COAX_IOS_SIMULATOR');sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip()
  if sim:
   ios=d/(mode+'-ios');subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2',*common,str(R/'coax/tests/nostr/schnorr_nonce_point_ios.c'),'-o',str(ios)],check=True)
   subprocess.run(['xcrun','simctl','spawn',sim,str(ios)],check=True);print(mode,'iOS Simulator BIP-340 nonce point/refusal',hashlib.sha256(ios.read_bytes()).hexdigest(),flush=True)
  wasm=d/(mode+'.wasm');exports='-Wl,-z,stack-size=1048576,--export=np_key,--export=np_pub,--export=np_aux,--export=np_message,--export=np_r,--export=np_hash,--export=np_valid,--export=np_odd,--export=np_run,--export=np_refusal'
  subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-mexec-model=reactor',*common,exports,'-o',str(wasm)],check=True)
  subprocess.run(['node',str(R/'coax/tests/nostr/schnorr_nonce_point_wasm.cjs'),str(wasm),str(fixtures)],check=True)
  print(mode,len(cases),'independent tagged hashes and pinned-libsecp nonce points; C',hashlib.sha256(c.read_bytes()).hexdigest(),'Wasm',hashlib.sha256(wasm.read_bytes()).hexdigest(),flush=True)
