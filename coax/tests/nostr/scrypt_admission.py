#!/usr/bin/env python3
"""Generated pointer-range policy against an independent integer model."""
if not __debug__: raise SystemExit('tests require Python assertions')
import ctypes as C
import hashlib
import json
import random
import subprocess
import tempfile
from pathlib import Path
R=Path(__file__).resolve().parents[3];U=(1<<64)-1;rng=random.Random(4909)
def span(a,n,limit): return 0<=n<=limit and (n==0 or (a!=0 and a+n<=U))
def overlaps(a,n,b,m): return n>0 and m>0 and a<b+m and b<a+n
def start(q):
    s,sn,m,mn,p,pn,t,tn,log=q
    if sn<1 or not span(s,sn,1048576) or not span(m,mn,268435456) or not span(p,pn,1024):return False
    if tn!=16 or t==0 or t+16>U or not 1<=log<=18 or mn<(1<<log)*1024:return False
    spans=[(s,sn),(m,mn),(p,pn),(t,tn)]
    return not any(overlaps(*spans[i],*spans[j]) for i in range(4) for j in range(i+1,4))
def span32(a,n):return n==0 or (0<=a<=0xffffffff and a+n<=0xffffffff)
def start32(q):return start(q) and all(span32(q[i],q[i+1]) for i in (0,2,4,6))
def output32(q):return output(q) and all(span32(q[i],q[i+1]) for i in (0,2,4))
def output(q):
    s,sn,m,mn,o,on=q
    if sn<1 or not span(s,sn,1048576) or mn<2048 or not span(m,mn,268435456) or on<32 or not span(o,on,1048576):return False
    spans=[(s,sn),(m,mn),(o,on)]
    return not any(overlaps(*spans[i],*spans[j]) for i in range(3) for j in range(i+1,3))
starts=[];outs=[]
for i in range(12000):
    q=[65536,64,2097152,2048,300000000,5,400000000,16,1]
    j=rng.randrange(len(q));choices=[0,1,15,16,17,31,32,63,64,1023,1024,1025,2047,2048,2049,1048576,1048577,268435456,268435457,U-17,U-16,U-15,U,4294967279,4294967280,4294967295,4294967296,65536,65537,2097152,2097153,300000000,400000000]
    q[j]=rng.choice(choices if j in [0,2,4,6] else [-1,0,1,15,16,17,18,19,31,32,63,64,1023,1024,1025,2047,2048,2049,1048576,1048577,268435456,268435457])
    starts.append([q,int(start(q)),int(start32(q))])
    v=[65536,64,2097152,2048,300000000,32];j=rng.randrange(len(v));v[j]=rng.choice(choices if j in [0,2,4] else [-1,0,1,31,32,33,63,64,2047,2048,2049,1048576,1048577,268435456,268435457]);outs.append([v,int(output(v)),int(output32(v))])
with tempfile.TemporaryDirectory(prefix='coax-scrypt-admission-') as temp:
    d=Path(temp);cases=d/'cases.json';cases.write_text(json.dumps({'start':[[list(map(str,q)),e,e32] for q,e,e32 in starts],'output':[[list(map(str,q)),e,e32] for q,e,e32 in outs]}))
    for mode,flags in [('checked',[]),('release',['--release'])]:
        c=d/(mode+'.c');c.write_bytes(subprocess.run([str(R/'coax/bin/coax'),'c',str(R/'coax/lib/crypto/scrypt_admission.coax'),'--library',*flags],capture_output=True,check=True).stdout)
        lib=d/(mode+'.dylib');subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC',str(c),'-o',str(lib)],check=True)
        obj=C.CDLL(str(lib));obj.sa_start.argtypes=[C.c_uint64,C.c_int64,C.c_uint64,C.c_int64,C.c_uint64,C.c_int64,C.c_uint64,C.c_int64,C.c_int64];obj.sa_start.restype=C.c_bool
        obj.sa_output.argtypes=[C.c_uint64,C.c_int64,C.c_uint64,C.c_int64,C.c_uint64,C.c_int64];obj.sa_output.restype=C.c_bool
        obj.sa_start32.argtypes=obj.sa_start.argtypes;obj.sa_start32.restype=C.c_bool
        obj.sa_output32.argtypes=obj.sa_output.argtypes;obj.sa_output32.restype=C.c_bool
        for q,e,e32 in starts:assert obj.sa_start(*q)==e and obj.sa_start32(*q)==e32
        for q,e,e32 in outs:assert obj.sa_output(*q)==e and obj.sa_output32(*q)==e32
        sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip()
        subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2','-c',str(c),'-o',str(d/'ios.o')],check=True)
        wasm=d/(mode+'.wasm')
        subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-mexec-model=reactor',str(c),'-Wl,--export=sa_start,--export=sa_output,--export=sa_start32,--export=sa_output32','-o',str(wasm)],check=True)
        subprocess.run(['node',str(R/'coax/tests/nostr/scrypt_admission_wasm.cjs'),str(wasm),str(cases)],check=True)
        print(mode,'12000 start and 12000 output independent 32/64-bit address cases on native and Wasm; simulator compile; C',hashlib.sha256(c.read_bytes()).hexdigest(),'Wasm',hashlib.sha256(wasm.read_bytes()).hexdigest(),flush=True)
