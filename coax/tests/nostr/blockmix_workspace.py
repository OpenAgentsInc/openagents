#!/usr/bin/env python3
"""Independent Salsa/BlockMix oracle for the owned workspace."""
if not __debug__: raise SystemExit('tests require Python assertions')
import ctypes as C,hashlib,importlib.util,json,os,random,subprocess,tempfile
from pathlib import Path
R=Path(__file__).resolve().parents[3];compiler=R/'coax/bin/coax';platform=R/'coax/lib/crypto/platform'
spec=importlib.util.spec_from_file_location('reference',R/'coax/tests/nostr/scrypt_blockmix.py');ref=importlib.util.module_from_spec(spec);spec.loader.exec_module(ref)
rng=random.Random(791490);cases=[]
for r in range(1,9):
 for n in range(16):
  data=bytes(128*r) if n==0 else rng.randbytes(128*r);cases.append((r,data,ref.reference(data,r)))
with tempfile.TemporaryDirectory(prefix='coax-block-owned-') as temp:
 d=Path(temp);fixtures=d/'cases.json';fixtures.write_text(json.dumps([{'r':r,'input':list(b),'expected':list(e)} for r,b,e in cases]))
 for mode,flags in [('checked',[]),('release',['--release'])]:
  c=d/(mode+'.c');z=d/(mode+'-zero.c')
  c.write_bytes(subprocess.run([str(compiler),'c',str(R/'coax/lib/crypto/scrypt_blockmix_workspace.coax'),'--library','--header',str(d/'block-owned.h'),*flags],capture_output=True,check=True).stdout)
  z.write_bytes(subprocess.run([str(compiler),'c',str(R/'coax/lib/crypto/zeroize.coax'),'--library','--header',str(d/'zeroize-policy.h'),*flags],capture_output=True,check=True).stdout)
  common=['-I'+str(d),'-I'+str(platform),str(c),str(z),str(platform/'zeroize.c'),str(R/'coax/tests/nostr/blockmix_workspace_fixture.c')]
  lib=d/(mode+'.dylib');subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC',*common,'-o',str(lib)],check=True);obj=C.CDLL(str(lib))
  for name in ['bm_input','bm_output']:getattr(obj,name).restype=C.POINTER(C.c_uint8)
  for r,data,want in cases:
   for i,b in enumerate(data):obj.bm_input()[i]=b
   assert obj.bm_run(r)==1 and bytes(obj.bm_output()[:len(data)])==want
  for i in range(5):assert obj.bm_refusal(i)==1
  main=d/'main.c';main.write_text('int bm_run(int),bm_refusal(int);int main(void){for(int i=0;i<5;i++)if(!bm_refusal(i))return 1;for(int i=1;i<=8;i++)if(!bm_run(i))return 2;return 0;}\n')
  exe=d/'asan';subprocess.run(['cc','-std=c11','-O1','-fsanitize=address,undefined',*common,str(main),'-o',str(exe)],check=True);subprocess.run([str(exe)],check=True)
  sim=os.environ.get('COAX_IOS_SIMULATOR');sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip()
  if sim:
   ios=d/(mode+'-ios');subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2',*common,str(R/'coax/tests/nostr/blockmix_workspace_ios.c'),'-o',str(ios)],check=True)
   subprocess.run(['xcrun','simctl','spawn',sim,str(ios)],check=True);print(mode,'iOS Simulator independent vector and refusal',hashlib.sha256(ios.read_bytes()).hexdigest(),flush=True)
  wasm=d/(mode+'.wasm');exports='-Wl,--export=bm_input,--export=bm_output,--export=bm_run,--export=bm_refusal'
  subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-mexec-model=reactor',*common,exports,'-o',str(wasm)],check=True)
  subprocess.run(['node',str(R/'coax/tests/nostr/scrypt_blockmix_wasm.cjs'),str(wasm),str(fixtures)],check=True)
  print(mode,len(cases),'independent cases and workspace wipes; C',hashlib.sha256(c.read_bytes()).hexdigest(),'Wasm',hashlib.sha256(wasm.read_bytes()).hexdigest(),flush=True)
