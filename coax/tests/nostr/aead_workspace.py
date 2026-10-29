#!/usr/bin/env python3
"""Staged AEAD workspace against independent HChaCha+OpenSSL vectors."""
if not __debug__: raise SystemExit('tests require Python assertions')
import ctypes as C,hashlib,importlib.util,json,os,random,subprocess,tempfile
from pathlib import Path
R=Path(__file__).resolve().parents[3];compiler=R/'coax/bin/coax';platform=R/'coax/lib/crypto/platform'
spec=importlib.util.spec_from_file_location('reference',R/'coax/tests/nostr/hchacha20.py');ref=importlib.util.module_from_spec(spec);spec.loader.exec_module(ref)
rng=random.Random(130520);cases=[]
lengths=[0,1,15,16,17,31,32,33,63,64,65,1023,1024,4095,4096]+[rng.randrange(4097) for _ in range(115)]+[0,4096]
for index,n in enumerate(lengths):
 key=rng.randbytes(32);nonce=rng.randbytes(24);an=0 if index==0 else 1024 if index>=len(lengths)-2 else rng.randrange(1025)
 cases.append({'key':list(key),'nonce':list(nonce),'aad':list(rng.randbytes(an)),'message':list(rng.randbytes(n)),'subkey':list(ref.reference(key,nonce[:16]))})
with tempfile.TemporaryDirectory(prefix='coax-aead-workspace-') as temp:
 d=Path(temp);fixtures=d/'cases.json';fixtures.write_text(json.dumps(cases));js=R/'coax/tests/nostr/aead_workspace_wasm.cjs'
 oracle=json.loads(subprocess.check_output(['node',str(js),'oracle',str(fixtures)]))
 for c,e in zip(cases,oracle,strict=True):c['expected']=e
 fixtures.write_text(json.dumps(cases))
 for mode,flags in [('checked',[]),('release',['--release'])]:
  c=d/(mode+'.c');z=d/(mode+'-zero.c')
  c.write_bytes(subprocess.run([str(compiler),'c',str(R/'coax/lib/crypto/aead_workspace.coax'),'--library','--header',str(d/'aead-workspace.h'),*flags],capture_output=True,check=True).stdout)
  z.write_bytes(subprocess.run([str(compiler),'c',str(R/'coax/lib/crypto/zeroize.coax'),'--library','--header',str(d/'zeroize-policy.h'),*flags],capture_output=True,check=True).stdout)
  common=['-I'+str(d),'-I'+str(platform),str(c),str(z),str(platform/'zeroize.c'),str(R/'coax/tests/nostr/aead_workspace_fixture.c')]
  lib=d/(mode+'.dylib');subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC',*common,'-o',str(lib)],check=True);obj=C.CDLL(str(lib))
  for name in ['aead_key','aead_nonce','aead_aad','aead_input','aead_output']:getattr(obj,name).restype=C.POINTER(C.c_uint8)
  for case in cases:
   for name,key in [('key','key'),('nonce','nonce'),('aad','aad'),('input','message')]:
    for i,b in enumerate(case[key]):getattr(obj,'aead_'+name)()[i]=b
   an=len(case['aad']);n=len(case['message']);assert obj.aead_seal(an,n)==1 and list(obj.aead_output()[:n+16])==case['expected']
   for i,b in enumerate(case['expected']):obj.aead_input()[i]=b
   for i in range(n,n+16):assert obj.aead_tamper(an,n+16,i)==1
   if n:assert obj.aead_tamper(an,n+16,0)==1
   assert obj.aead_nonce_tamper(an,n+16)==1
   if an:assert obj.aead_aad_tamper(an,n+16)==1
   assert obj.aead_open(an,n+16)==1 and list(obj.aead_output()[:n])==case['message']
  main=d/'main.c';main.write_text('int aead_seal(int,int);int main(void){return !aead_seal(1024,4096);}\n')
  exe=d/'asan';subprocess.run(['cc','-std=c11','-O1','-fsanitize=address,undefined',*common,str(main),'-o',str(exe)],check=True);subprocess.run([str(exe)],check=True)
  sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip();subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2','-c',str(c),'-o',str(d/'ios.o')],check=True)
  sim=os.environ.get('COAX_IOS_SIMULATOR')
  if sim:
   ios=d/(mode+'-ios');subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2',*common,str(R/'coax/tests/nostr/aead_workspace_ios.c'),'-o',str(ios)],check=True)
   subprocess.run(['xcrun','simctl','spawn',sim,str(ios)],check=True);print(mode,'iOS simulator draft vector and tag tampering',hashlib.sha256(ios.read_bytes()).hexdigest(),flush=True)
  wasm=d/(mode+'.wasm');exports='-Wl,-z,stack-size=1048576,--export=aead_key,--export=aead_nonce,--export=aead_aad,--export=aead_input,--export=aead_output,--export=aead_seal,--export=aead_open,--export=aead_tamper,--export=aead_nonce_tamper,--export=aead_aad_tamper'
  subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-mexec-model=reactor',*common,exports,'-o',str(wasm)],check=True)
  subprocess.run(['node',str(js),str(wasm),str(fixtures)],check=True)
  print(mode,len(cases),'independent cases, real workspace erasure; C',hashlib.sha256(c.read_bytes()).hexdigest(),'Wasm',hashlib.sha256(wasm.read_bytes()).hexdigest(),flush=True)
