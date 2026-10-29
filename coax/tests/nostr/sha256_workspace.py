#!/usr/bin/env python3
"""Pinned NIST and hashlib oracle for owned streaming SHA-256."""
if not __debug__: raise SystemExit('tests require Python assertions')
import ctypes as C,hashlib,importlib.util,json,os,random,subprocess,tempfile
from pathlib import Path
R=Path(__file__).resolve().parents[3];compiler=R/'coax/bin/coax';platform=R/'coax/lib/crypto/platform'
spec=importlib.util.spec_from_file_location('reference',R/'coax/tests/nostr/sha256.py');ref=importlib.util.module_from_spec(spec);spec.loader.exec_module(ref)
rng=random.Random(2560);cases=list(ref.nist_vectors())
for n in [0,1,55,56,63,64,65,119,120,127,128,129,255,256,1000,4097,16384]:
 b=rng.randbytes(n);cases.append((b,hashlib.sha256(b).digest()))
assert all(len(b)<=16384 and hashlib.sha256(b).digest()==want for b,want in cases)
with tempfile.TemporaryDirectory(prefix='coax-sha-owned-') as temp:
 d=Path(temp);fixtures=d/'cases.json';fixtures.write_text(json.dumps([{'input':list(b),'expected':list(e)} for b,e in cases]))
 for mode,flags in [('checked',[]),('release',['--release'])]:
  c=d/(mode+'.c');z=d/(mode+'-zero.c')
  c.write_bytes(subprocess.run([str(compiler),'c',str(R/'coax/lib/crypto/sha256_workspace.coax'),'--library','--header',str(d/'sha-owned.h'),*flags],capture_output=True,check=True).stdout)
  z.write_bytes(subprocess.run([str(compiler),'c',str(R/'coax/lib/crypto/zeroize.coax'),'--library','--header',str(d/'zeroize-policy.h'),*flags],capture_output=True,check=True).stdout)
  common=['-I'+str(d),'-I'+str(platform),str(c),str(z),str(platform/'zeroize.c'),str(R/'coax/tests/nostr/sha256_workspace_fixture.c')]
  lib=d/(mode+'.dylib');subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC',*common,'-o',str(lib)],check=True);obj=C.CDLL(str(lib))
  for name in ['so_input','so_output']:getattr(obj,name).restype=C.POINTER(C.c_uint8)
  for data,want in cases:
   for i,b in enumerate(data):obj.so_input()[i]=b
   for chunk in [1,55,64,129,4096]:assert obj.so_run(len(data),chunk)==1 and bytes(obj.so_output()[:32])==want
  assert obj.so_refusal()==1
  main=d/'main.c';main.write_text('int so_run(int,int),so_refusal(void);int main(void){return !so_refusal()||!so_run(16384,55);}\n')
  exe=d/'asan';subprocess.run(['cc','-std=c11','-O1','-fsanitize=address,undefined',*common,str(main),'-o',str(exe)],check=True);subprocess.run([str(exe)],check=True)
  sim=os.environ.get('COAX_IOS_SIMULATOR');sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip()
  if sim:
   ios=d/(mode+'-ios');subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2',*common,str(R/'coax/tests/nostr/sha256_workspace_ios.c'),'-o',str(ios)],check=True)
   subprocess.run(['xcrun','simctl','spawn',sim,str(ios)],check=True);print(mode,'iOS Simulator abc and refusal',hashlib.sha256(ios.read_bytes()).hexdigest(),flush=True)
  wasm=d/(mode+'.wasm');exports='-Wl,--export=so_input,--export=so_output,--export=so_run,--export=so_refusal'
  subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-mexec-model=reactor',*common,exports,'-o',str(wasm)],check=True)
  subprocess.run(['node',str(R/'coax/tests/nostr/sha256_workspace_wasm.cjs'),str(wasm),str(fixtures)],check=True)
  print(mode,len(cases),'NIST/hashlib cases with streamed chunks and wipes; C',hashlib.sha256(c.read_bytes()).hexdigest(),'Wasm',hashlib.sha256(wasm.read_bytes()).hexdigest(),flush=True)
