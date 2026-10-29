#!/usr/bin/env python3
"""HChaCha bigint model and OpenSSL ChaCha oracle for owned state."""
if not __debug__: raise SystemExit('tests require Python assertions')
import ctypes as C,hashlib,importlib.util,json,os,random,subprocess,tempfile
from pathlib import Path
R=Path(__file__).resolve().parents[3];compiler=R/'coax/bin/coax';platform=R/'coax/lib/crypto/platform'
spec=importlib.util.spec_from_file_location('reference',R/'coax/tests/nostr/hchacha20.py');ref=importlib.util.module_from_spec(spec);spec.loader.exec_module(ref)
rng=random.Random(202025);cases=[]
for n in [0,1,15,16,31,32,63,64,65,127,128,129,1024]+[rng.randrange(1025) for _ in range(500)]:
 key=rng.randbytes(32);nonce=rng.randbytes(24);counter=min(rng.getrandbits(32),0x100000000-((n+63)//64))
 cases.append({'key':list(key),'nonce':list(nonce),'input':list(rng.randbytes(n)),'subkey':list(ref.reference(key,nonce[:16])),'counter':counter})
with tempfile.TemporaryDirectory(prefix='coax-xc-owned-') as temp:
 d=Path(temp);fixtures=d/'cases.json';fixtures.write_text(json.dumps(cases));js=R/'coax/tests/nostr/xchacha20_workspace_wasm.cjs'
 oracle=json.loads(subprocess.check_output(['node',str(js),'oracle',str(fixtures)]))
 for c,e in zip(cases,oracle,strict=True):c['expected']=e
 fixtures.write_text(json.dumps(cases))
 for mode,flags in [('checked',[]),('release',['--release'])]:
  c=d/(mode+'.c');z=d/(mode+'-zero.c')
  c.write_bytes(subprocess.run([str(compiler),'c',str(R/'coax/lib/crypto/xchacha20_workspace.coax'),'--library','--header',str(d/'xc-owned.h'),*flags],capture_output=True,check=True).stdout)
  z.write_bytes(subprocess.run([str(compiler),'c',str(R/'coax/lib/crypto/zeroize.coax'),'--library','--header',str(d/'zeroize-policy.h'),*flags],capture_output=True,check=True).stdout)
  common=['-I'+str(d),'-I'+str(platform),str(c),str(z),str(platform/'zeroize.c'),str(R/'coax/tests/nostr/xchacha20_workspace_fixture.c')]
  lib=d/(mode+'.dylib');subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC',*common,'-o',str(lib)],check=True);obj=C.CDLL(str(lib))
  for name in ['xc_key','xc_nonce','xc_input','xc_output','xc_subkey']:getattr(obj,name).restype=C.POINTER(C.c_uint8)
  obj.xc_run.argtypes=[C.c_int,C.c_uint32]
  for case in cases:
   for name in ['key','nonce','input']:
    for i,b in enumerate(case[name]):getattr(obj,'xc_'+name)()[i]=b
   assert obj.xc_derive()==1 and list(obj.xc_subkey()[:32])==case['subkey']
   assert obj.xc_run(len(case['input']),case['counter'])==1 and list(obj.xc_output()[:len(case['input'])])==case['expected']
  for i in range(5):assert obj.xc_refusal(i)==1
  main=d/'main.c';main.write_text('int xc_run(int,unsigned),xc_refusal(int),xc_derive(void);int main(void){for(int i=0;i<5;i++)if(!xc_refusal(i))return 1;return !xc_derive()||!xc_run(1024,0);}\n')
  exe=d/'asan';subprocess.run(['cc','-std=c11','-O1','-fsanitize=address,undefined',*common,str(main),'-o',str(exe)],check=True);subprocess.run([str(exe)],check=True)
  sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip();sim=os.environ.get('COAX_IOS_SIMULATOR')
  if sim:
   ios=d/(mode+'-ios');subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2',*common,str(R/'coax/tests/nostr/xchacha20_workspace_ios.c'),'-o',str(ios)],check=True)
   subprocess.run(['xcrun','simctl','spawn',sim,str(ios)],check=True);print(mode,'iOS Simulator vector and refusal',hashlib.sha256(ios.read_bytes()).hexdigest(),flush=True)
  wasm=d/(mode+'.wasm');exports='-Wl,--export=xc_key,--export=xc_nonce,--export=xc_input,--export=xc_output,--export=xc_subkey,--export=xc_derive,--export=xc_run,--export=xc_refusal'
  subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-mexec-model=reactor',*common,exports,'-o',str(wasm)],check=True)
  subprocess.run(['node',str(js),str(wasm),str(fixtures)],check=True)
  print(mode,len(cases),'independent cases and workspace wipes; C',hashlib.sha256(c.read_bytes()).hexdigest(),'Wasm',hashlib.sha256(wasm.read_bytes()).hexdigest(),flush=True)
