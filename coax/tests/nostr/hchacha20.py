#!/usr/bin/env python3
"""Compare emitted HChaCha20 with an independent round-state model."""
import ctypes as C
import hashlib
import json
import random
import subprocess
import tempfile
from pathlib import Path

ROOT=Path(__file__).resolve().parents[3]
SOURCE=ROOT/'coax/lib/crypto/hchacha20.coax'
COMPILER=ROOT/'coax/bin/coax'
MASK=(1<<32)-1


def reference(key,nonce):
    words=lambda b:[int.from_bytes(b[i:i+4],'little') for i in range(0,len(b),4)]
    x=words(b'expand 32-byte k'+key+nonce)
    def quarter(a,b,c,d):
        for left,right,dest,shift in ((a,b,d,16),(c,d,b,12),(a,b,d,8),(c,d,b,7)):
            x[left]=(x[left]+x[right])&MASK
            v=x[dest]^x[left]
            x[dest]=((v<<shift)|(v>>(32-shift)))&MASK
    for _ in range(10):
        for indices in ((0,4,8,12),(1,5,9,13),(2,6,10,14),(3,7,11,15),
                        (0,5,10,15),(1,6,11,12),(2,7,8,13),(3,4,9,14)):
            quarter(*indices)
    return b''.join(x[i].to_bytes(4,'little') for i in (0,1,2,3,12,13,14,15))


class View(C.Structure): _fields_=[('data',C.POINTER(C.c_uint8)),('count',C.c_int64)]
class Result(C.Structure): _fields_=[('written',C.c_int64),('error',C.c_uint32)]


def main():
    key=bytes(range(32));nonce=bytes.fromhex('000000090000004a0000000031415927')
    assert reference(key,nonce).hex()=='82413b4227b27bfed30e42508a877d73a0f9e4d58a74a853c12ec41326d3ecdc'
    rng=random.Random(2020)
    cases=[(key,nonce),(bytes(32),bytes(16))]+[(rng.randbytes(32),rng.randbytes(16)) for _ in range(1000)]
    cases=[(k,n,reference(k,n)) for k,n in cases]
    with tempfile.TemporaryDirectory(prefix='coax-hchacha-') as temp:
        d=Path(temp);header=d/'hchacha20.h'
        fixtures=d/'cases.json';fixtures.write_text(json.dumps([{'key':list(k),'nonce':list(n),'expected':list(e)} for k,n,e in cases]))
        for mode,flags in [('checked',[]),('release',['--release'])]:
            cfile=d/(mode+'.c');lib=d/(mode+'.dylib')
            generated=subprocess.run([str(COMPILER),'c',str(SOURCE),'--library','--header',str(header),*flags],capture_output=True,check=True)
            cfile.write_bytes(generated.stdout)
            subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC',str(cfile),'-o',str(lib)],check=True)
            fn=C.CDLL(str(lib)).hchacha20_checked;fn.argtypes=[View,View,View,C.c_bool];fn.restype=Result
            for key,nonce,expected in cases:
                a=(C.c_uint8*32).from_buffer_copy(key);b=(C.c_uint8*16).from_buffer_copy(nonce);out=(C.c_uint8*33)();out[32]=165
                r=fn(View(a,32),View(b,16),View(out,33),False)
                assert r.error==0 and r.written==32 and bytes(out[:32])==expected and out[32]==165
            mainfile=d/'main.c';mainfile.write_text('int hc_run(void); int hc_refusal(int); int main(void){for(int i=0;i<4;i++)if(!hc_refusal(i))return 1;return !hc_run();}\n')
            subprocess.run(['cc','-std=c11','-O1','-fsanitize=address,undefined','-I'+str(d),str(cfile),str(ROOT/'coax/tests/nostr/hchacha20_wasm.c'),str(mainfile),'-o',str(d/'asan')],check=True)
            subprocess.run([str(d/'asan')],check=True)
            sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip()
            subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2','-c',str(cfile),'-o',str(d/'ios.o')],check=True)
            module=d/(mode+'.wasm')
            subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-I'+str(d),'-mexec-model=reactor',str(cfile),str(ROOT/'coax/tests/nostr/hchacha20_wasm.c'),'-Wl,--export=hc_key,--export=hc_nonce,--export=hc_output,--export=hc_run,--export=hc_refusal','-o',str(module)],check=True)
            subprocess.run(['node',str(ROOT/'coax/tests/nostr/hchacha20_wasm.cjs'),str(module),str(fixtures)],check=True)
            print(mode,'native:',len(cases),'passed; C sha256',hashlib.sha256(cfile.read_bytes()).hexdigest(),'Wasm sha256',hashlib.sha256(module.read_bytes()).hexdigest(),flush=True)


if __name__=='__main__': main()
