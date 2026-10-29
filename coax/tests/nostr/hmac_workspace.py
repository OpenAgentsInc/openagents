#!/usr/bin/env python3
"""Compare owned HMAC with Python/OpenSSL's independent implementation."""
if not __debug__: raise SystemExit('tests require Python assertions')
import ctypes as C,hashlib,hmac,json,os,random,subprocess,tempfile
from pathlib import Path
R=Path(__file__).resolve().parents[3];compiler=R/'coax/bin/coax';platform=R/'coax/lib/crypto/platform'
rng=random.Random(42310);cases=[]
for k in [0,1,20,63,64,65,129,1024]:
 for n in [0,1,55,56,63,64,65,129,1024,4096]:
  key=rng.randbytes(k);data=rng.randbytes(n);cases.append((key,data,hmac.digest(key,data,'sha256')))
with tempfile.TemporaryDirectory(prefix='coax-hmac-owned-') as temp:
 d=Path(temp);fixtures=d/'cases.json';fixtures.write_text(json.dumps([{'key':list(k),'input':list(b),'expected':list(e)} for k,b,e in cases]))
 for mode,flags in [('checked',[]),('release',['--release'])]:
  c=d/(mode+'.c');z=d/(mode+'-zero.c')
  c.write_bytes(subprocess.run([str(compiler),'c',str(R/'coax/lib/crypto/hmac_workspace.coax'),'--library','--header',str(d/'hmac-owned.h'),*flags],capture_output=True,check=True).stdout)
  z.write_bytes(subprocess.run([str(compiler),'c',str(R/'coax/lib/crypto/zeroize.coax'),'--library','--header',str(d/'zeroize-policy.h'),*flags],capture_output=True,check=True).stdout)
  common=['-I'+str(d),'-I'+str(platform),str(c),str(z),str(platform/'zeroize.c'),str(R/'coax/tests/nostr/hmac_workspace_fixture.c')]
  lib=d/(mode+'.dylib');subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC',*common,'-o',str(lib)],check=True);obj=C.CDLL(str(lib))
  for name in ['hm_key','hm_input','hm_output']:getattr(obj,name).restype=C.POINTER(C.c_uint8)
  for key,data,want in cases:
   for i,b in enumerate(key):obj.hm_key()[i]=b
   for i,b in enumerate(data):obj.hm_input()[i]=b
   assert obj.hm_run(len(key),len(data))==1 and bytes(obj.hm_output()[:32])==want
  assert obj.hm_refusal()==1
  main=d/'main.c';main.write_text('int hm_run(int,int),hm_refusal(void);int main(void){return !hm_refusal()||!hm_run(1024,4096);}\n')
  exe=d/'asan';subprocess.run(['cc','-std=c11','-O1','-fsanitize=address,undefined',*common,str(main),'-o',str(exe)],check=True);subprocess.run([str(exe)],check=True)
  sim=os.environ.get('COAX_IOS_SIMULATOR');sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip()
  if sim:
   ios=d/(mode+'-ios');subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2',*common,str(R/'coax/tests/nostr/hmac_workspace_ios.c'),'-o',str(ios)],check=True)
   subprocess.run(['xcrun','simctl','spawn',sim,str(ios)],check=True);print(mode,'iOS Simulator RFC vector and refusal',hashlib.sha256(ios.read_bytes()).hexdigest(),flush=True)
  wasm=d/(mode+'.wasm');exports='-Wl,--export=hm_key,--export=hm_input,--export=hm_output,--export=hm_run,--export=hm_refusal'
  subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-mexec-model=reactor',*common,exports,'-o',str(wasm)],check=True)
  subprocess.run(['node',str(R/'coax/tests/nostr/hmac_workspace_wasm.cjs'),str(wasm),str(fixtures)],check=True)
  print(mode,len(cases),'independent cases and workspace wipes; C',hashlib.sha256(c.read_bytes()).hexdigest(),'Wasm',hashlib.sha256(wasm.read_bytes()).hexdigest(),flush=True)
