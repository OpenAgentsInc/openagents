#!/usr/bin/env python3
"""AEAD seven-span policy against independent integer intersections."""
if not __debug__: raise SystemExit('tests require Python assertions')
import ctypes as C,hashlib,json,random,subprocess,tempfile
from pathlib import Path
R=Path(__file__).resolve().parents[3];rng=random.Random(99049);U=(1<<64)-1
class Span(C.Structure):_fields_=[('address',C.c_uint64),('length',C.c_int64)]
class View(C.Structure):_fields_=[('data',C.POINTER(Span)),('count',C.c_int64)]
def model(s,seal,narrow):
 for a,n in s:
  if n<0 or n>1048576 or (n and (not a or a+n>U or (narrow and a+n>0xffffffff))):return 0
 if s[0][1]<1 or s[1][1]<1 or s[2][1]!=32 or s[3][1]!=24 or s[4][1]>1024:return 0
 n=s[5][1];capacity=s[6][1]
 if seal:
  if n>4096 or capacity<n+16:return 0
 elif n<16 or n>4112 or capacity<n-16:return 0
 for i in range(7):
  for j in range(i):
   a,n=s[i];b,m=s[j]
   if n and m and a<b+m and b<a+n:return 0
 return 1
cases=[];baseline=[[10000000,100],[20000000,200],[30000000,32],[40000000,24],[50000000,5],[60000000,17],[70000000,33]]
for i in range(12000):
 s=[p[:] for p in baseline];seal=i%2;narrow=(i//2)%2
 if not seal:s[5][1]=33;s[6][1]=17
 region=rng.randrange(7)
 if rng.randrange(2):s[region][0]=rng.choice([0,1,10000000,20000000,30000000,40000000,50000000,60000000,70000000,4294967280,4294967295,4294967296,U-32,U-16,U])
 else:s[region][1]=rng.choice([-1,0,1,15,16,17,23,24,25,31,32,33,1023,1024,1025,4095,4096,4097,4111,4112,4113,1048576,1048577])
 cases.append((s,seal,narrow,model(s,seal,narrow)))
with tempfile.TemporaryDirectory(prefix='coax-aead-admission-') as temp:
 d=Path(temp);casesfile=d/'cases.json';casesfile.write_text(json.dumps([[[[str(a),str(n)] for a,n in s],seal,narrow,e] for s,seal,narrow,e in cases]))
 for mode,flags in [('checked',[]),('release',['--release'])]:
  c=d/(mode+'.c');c.write_bytes(subprocess.run([str(R/'coax/bin/coax'),'c',str(R/'coax/lib/crypto/aead_admission.coax'),'--library','--header',str(d/'aead-admission.h'),*flags],capture_output=True,check=True).stdout)
  common=['-I'+str(d),str(c),str(R/'coax/tests/nostr/aead_admission_fixture.c')];lib=d/(mode+'.dylib')
  subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC',*common,'-o',str(lib)],check=True);obj=C.CDLL(str(lib));obj.aa_admit.argtypes=[View,C.c_bool,C.c_bool];obj.aa_admit.restype=C.c_bool
  for s,seal,narrow,e in cases:
   entries=(Span*7)(*(Span(a,n) for a,n in s));assert obj.aa_admit(View(entries,7),seal,narrow)==e
  sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip();subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2','-c',str(c),'-o',str(d/'ios.o')],check=True)
  wasm=d/(mode+'.wasm');subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-mexec-model=reactor',*common,'-Wl,--export=aa_set,--export=aa_check','-o',str(wasm)],check=True)
  subprocess.run(['node',str(R/'coax/tests/nostr/aead_admission_wasm.cjs'),str(wasm),str(casesfile)],check=True)
  print(mode,len(cases),'independent address cases, native and Wasm; simulator compile; C',hashlib.sha256(c.read_bytes()).hexdigest(),'Wasm',hashlib.sha256(wasm.read_bytes()).hexdigest(),flush=True)
