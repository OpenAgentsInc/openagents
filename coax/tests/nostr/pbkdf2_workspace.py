#!/usr/bin/env python3
"""One-iteration owned PBKDF2 compared with OpenSSL-backed hashlib."""
if not __debug__: raise SystemExit('tests require Python assertions')
import ctypes as C,hashlib,json,os,random,subprocess,tempfile
from pathlib import Path
R=Path(__file__).resolve().parents[3];compiler=R/'coax/bin/coax';platform=R/'coax/lib/crypto/platform'
rng=random.Random(791412);cases=[];sizes=[0,1,31,32,33,63,64,65,1023,1024]
for i in range(100):
 pn=sizes[i%10] if i<20 else rng.randrange(1025);sn=sizes[(i//10)%10] if i<20 else rng.randrange(1025);n=max(1,sizes[(i+1)%10]) if i<20 else rng.randrange(1,1025)
 p=rng.randbytes(pn);s=rng.randbytes(sn);cases.append({'password':list(p),'salt':list(s),'expected':list(hashlib.pbkdf2_hmac('sha256',p,s,1,n))})
for pn,sn,n in [(0,0,1),(1024,1024,1024),(0,1024,1024),(1024,0,1024)]:
 p=rng.randbytes(pn);s=rng.randbytes(sn);cases.append({'password':list(p),'salt':list(s),'expected':list(hashlib.pbkdf2_hmac('sha256',p,s,1,n))})
with tempfile.TemporaryDirectory(prefix='coax-pbkdf-owned-') as temp:
 d=Path(temp);fixtures=d/'cases.json';fixtures.write_text(json.dumps(cases))
 for mode,flags in [('checked',[]),('release',['--release'])]:
  c=d/(mode+'.c');z=d/(mode+'-zero.c')
  c.write_bytes(subprocess.run([str(compiler),'c',str(R/'coax/lib/crypto/pbkdf2_workspace.coax'),'--library','--header',str(d/'pb-owned.h'),*flags],capture_output=True,check=True).stdout)
  z.write_bytes(subprocess.run([str(compiler),'c',str(R/'coax/lib/crypto/zeroize.coax'),'--library','--header',str(d/'zeroize-policy.h'),*flags],capture_output=True,check=True).stdout)
  common=['-I'+str(d),'-I'+str(platform),str(c),str(z),str(platform/'zeroize.c'),str(R/'coax/tests/nostr/pbkdf2_workspace_fixture.c')]
  lib=d/(mode+'.dylib');subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC',*common,'-o',str(lib)],check=True);obj=C.CDLL(str(lib))
  for name in ['pb_password','pb_salt','pb_output']:getattr(obj,name).restype=C.POINTER(C.c_uint8)
  for case in cases:
   for name in ['password','salt']:
    for i,b in enumerate(case[name]):getattr(obj,'pb_'+name)()[i]=b
   n=len(case['expected']);assert obj.pb_run(len(case['password']),len(case['salt']),n)==1 and list(obj.pb_output()[:n])==case['expected']
  for i in range(5):assert obj.pb_refusal(i)==1
  main=d/'main.c';main.write_text('int pb_run(int,int,int),pb_refusal(int);int main(void){for(int i=0;i<5;i++)if(!pb_refusal(i))return 1;return !pb_run(1024,1024,1024);}\n')
  exe=d/'asan';subprocess.run(['cc','-std=c11','-O1','-fsanitize=address,undefined',*common,str(main),'-o',str(exe)],check=True);subprocess.run([str(exe)],check=True)
  sim=os.environ.get('COAX_IOS_SIMULATOR');sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip()
  if sim:
   ios=d/(mode+'-ios');subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2',*common,str(R/'coax/tests/nostr/pbkdf2_workspace_ios.c'),'-o',str(ios)],check=True)
   subprocess.run(['xcrun','simctl','spawn',sim,str(ios)],check=True);print(mode,'iOS Simulator RFC vector and refusals',hashlib.sha256(ios.read_bytes()).hexdigest(),flush=True)
  wasm=d/(mode+'.wasm');exports='-Wl,--export=pb_password,--export=pb_salt,--export=pb_output,--export=pb_run,--export=pb_refusal'
  subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-mexec-model=reactor',*common,exports,'-o',str(wasm)],check=True)
  subprocess.run(['node',str(R/'coax/tests/nostr/scrypt_pbkdf2_wasm.cjs'),str(wasm),str(fixtures)],check=True)
  print(mode,len(cases),'independent cases and workspace wipes; C',hashlib.sha256(c.read_bytes()).hexdigest(),'Wasm',hashlib.sha256(wasm.read_bytes()).hexdigest(),flush=True)
