#!/usr/bin/env python3
"""Six-span signing admission against independent integer intersections."""
if not __debug__: raise SystemExit('tests require Python assertions')
import ctypes as C,hashlib,json,random,subprocess,tempfile
from pathlib import Path
R=Path(__file__).resolve().parents[3];rng=random.Random(3407);U=(1<<64)-1
class Span(C.Structure):_fields_=[('address',C.c_uint64),('length',C.c_int64)]
class View(C.Structure):_fields_=[('data',C.POINTER(Span)),('count',C.c_int64)]
def model(s,narrow):
 for a,n in s:
  if n<0 or n>1048576 or (n and (not a or a+n>U or (narrow and a+n>0xffffffff))):return 0
 if s[0][1]<1 or s[1][1]<1 or s[2][1]!=32 or s[3][1]!=32 or s[5][1]<64:return 0
 for i in range(6):
  for j in range(i):
   a,n=s[i];b,m=s[j]
   if n and m and a<b+m and b<a+n:return 0
 return 1
cases=[];base=[[10000000,100],[20000000,200],[30000000,32],[40000000,32],[50000000,17],[60000000,64]]
for i in range(12000):
 s=[p[:] for p in base];narrow=i%2;region=rng.randrange(6)
 if rng.randrange(2):s[region][0]=rng.choice([0,1,10000000,20000000,30000000,40000000,50000000,60000000,4294967280,4294967295,4294967296,U-32,U-16,U])
 else:s[region][1]=rng.choice([-1,0,1,31,32,33,63,64,65,1048575,1048576,1048577])
 cases.append((s,narrow,model(s,narrow)))
with tempfile.TemporaryDirectory(prefix='coax-sign-admission-') as temp:
 d=Path(temp);fixtures=d/'cases.json';fixtures.write_text(json.dumps([[[[str(a),str(n)] for a,n in s],narrow,e] for s,narrow,e in cases]))
 for mode,flags in [('checked',[]),('release',['--release'])]:
  c=d/(mode+'.c');c.write_bytes(subprocess.run([str(R/'coax/bin/coax'),'c',str(R/'coax/lib/crypto/schnorr_admission.coax'),'--library','--header',str(d/'sign-admission.h'),*flags],capture_output=True,check=True).stdout)
  common=['-I'+str(d),str(c),str(R/'coax/tests/nostr/schnorr_admission_fixture.c')];lib=d/(mode+'.dylib');subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC',*common,'-o',str(lib)],check=True)
  obj=C.CDLL(str(lib));obj.sig_admit.argtypes=[View,C.c_bool];obj.sig_admit.restype=C.c_bool
  for s,narrow,want in cases:
   entries=(Span*6)(*(Span(a,n) for a,n in s));assert obj.sig_admit(View(entries,6),narrow)==want
  sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip();subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2','-c',str(c),'-o',str(d/'ios.o')],check=True)
  wasm=d/(mode+'.wasm');subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-mexec-model=reactor',*common,'-Wl,--export=sig_set,--export=sig_check','-o',str(wasm)],check=True)
  subprocess.run(['node',str(R/'coax/tests/nostr/schnorr_admission_wasm.cjs'),str(wasm),str(fixtures)],check=True)
  print(mode,len(cases),'independent address cases, native/Wasm; simulator compile; C',hashlib.sha256(c.read_bytes()).hexdigest(),'Wasm',hashlib.sha256(wasm.read_bytes()).hexdigest(),flush=True)
