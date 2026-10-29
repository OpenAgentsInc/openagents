#!/usr/bin/env python3
"""Exercise caller-owned Poly1305 against bigint and OpenSSL oracles."""
if not __debug__: raise SystemExit('tests require Python assertions')
import ctypes as C,hashlib,importlib.util,json,os,random,subprocess,tempfile
from pathlib import Path
R=Path(__file__).resolve().parents[3]; compiler=R/'coax/bin/coax'; platform=R/'coax/lib/crypto/platform'
spec=importlib.util.spec_from_file_location('reference',R/'coax/tests/nostr/poly1305.py');ref=importlib.util.module_from_spec(spec);spec.loader.exec_module(ref)
rng=random.Random(843920);cases=[]
for n in [0,1,2,15,16,17,31,32,33,63,64,65,255,256,257,1024]+[rng.randrange(257) for _ in range(1000)]+[1048576]:
 key=rng.randbytes(32);msg=rng.randbytes(n);cases.append((key,msg,ref.reference(key,msg)))
key=bytes([1]+[0]*31);msg=bytes([255]*16+[252]+[255]*15);cases.append((key,msg,ref.reference(key,msg)))
for key,msg,want in cases[:100]+cases[-2:]:
 oracle=subprocess.run(['openssl','mac','-macopt','hexkey:'+key.hex(),'POLY1305'],input=msg,capture_output=True,check=True)
 assert bytes.fromhex(oracle.stdout.decode().strip())==want
with tempfile.TemporaryDirectory(prefix='coax-poly-workspace-') as temp:
 d=Path(temp);fixtures=d/'cases.json';fixtures.write_text(json.dumps([{'key':list(k),'message':list(m),'expected':list(e)} for k,m,e in cases]))
 for mode,flags in [('checked',[]),('release',['--release'])]:
  c=d/(mode+'.c');z=d/(mode+'-zero.c')
  c.write_bytes(subprocess.run([str(compiler),'c',str(R/'coax/lib/crypto/poly1305_workspace.coax'),'--library','--header',str(d/'poly-owned.h'),*flags],capture_output=True,check=True).stdout)
  z.write_bytes(subprocess.run([str(compiler),'c',str(R/'coax/lib/crypto/zeroize.coax'),'--library','--header',str(d/'zeroize-policy.h'),*flags],capture_output=True,check=True).stdout)
  common=['-I'+str(d),'-I'+str(platform),str(c),str(z),str(platform/'zeroize.c'),str(R/'coax/tests/nostr/poly1305_workspace_fixture.c')]
  lib=d/(mode+'.dylib');subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC',*common,'-o',str(lib)],check=True);obj=C.CDLL(str(lib))
  for name in ['poly_key','poly_message','poly_output']:getattr(obj,name).restype=C.POINTER(C.c_uint8)
  for key,msg,want in cases:
   for i,b in enumerate(key):obj.poly_key()[i]=b
   for i,b in enumerate(msg):obj.poly_message()[i]=b
   assert obj.poly_run(len(msg))==1 and bytes(obj.poly_output()[:16])==want
  for i in range(4):assert obj.poly_refusal(i)==1
  main=d/'main.c';main.write_text('int poly_run(int),poly_refusal(int);int main(void){for(int i=0;i<4;i++)if(!poly_refusal(i))return 1;return !poly_run(1024);}\n')
  exe=d/'asan';subprocess.run(['cc','-std=c11','-O1','-fsanitize=address,undefined',*common,str(main),'-o',str(exe)],check=True);subprocess.run([str(exe)],check=True)
  sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip()
  sim=os.environ.get('COAX_IOS_SIMULATOR')
  if sim:
   ios=d/(mode+'-ios');subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2',*common,str(R/'coax/tests/nostr/poly1305_workspace_ios.c'),'-o',str(ios)],check=True)
   subprocess.run(['xcrun','simctl','spawn',sim,str(ios)],check=True);print(mode,'iOS simulator vector and refusal',hashlib.sha256(ios.read_bytes()).hexdigest(),flush=True)
  wasm=d/(mode+'.wasm');exports='-Wl,--export=poly_key,--export=poly_message,--export=poly_output,--export=poly_run,--export=poly_refusal'
  subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-mexec-model=reactor',*common,exports,'-o',str(wasm)],check=True)
  subprocess.run(['node',str(R/'coax/tests/nostr/poly1305_wasm.cjs'),str(wasm),str(fixtures)],check=True)
  print(mode,len(cases),'independent cases and workspace wipes; C',hashlib.sha256(c.read_bytes()).hexdigest(),'Wasm',hashlib.sha256(wasm.read_bytes()).hexdigest(),flush=True)
