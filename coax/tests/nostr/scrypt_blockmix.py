#!/usr/bin/env python3
"""Independent byte/block model for bounded RFC 7914 BlockMix."""
import ctypes as C
import hashlib
import importlib.util
import json
import random
import subprocess
import tempfile
from pathlib import Path

ROOT=Path(__file__).resolve().parents[3]
SOURCE=ROOT/'coax/lib/crypto/scrypt_blockmix.coax'
COMPILER=ROOT/'coax/bin/coax'
spec=importlib.util.spec_from_file_location('salsa_reference',ROOT/'coax/tests/nostr/salsa20_8.py')
salsa=importlib.util.module_from_spec(spec);spec.loader.exec_module(salsa)


def reference(data,r):
    x=data[-64:];parts=[]
    for i in range(2*r):
        mixed=bytes(a^b for a,b in zip(x,data[64*i:64*(i+1)]))
        words=[int.from_bytes(mixed[j:j+4],'little') for j in range(0,64,4)]
        x=b''.join(v.to_bytes(4,'little') for v in salsa.reference(words))
        parts.append(x)
    return b''.join(parts[::2]+parts[1::2])


class View(C.Structure): _fields_=[('data',C.POINTER(C.c_uint8)),('count',C.c_int64)]


def main():
    rng=random.Random(79149);cases=[]
    for r in range(1,9):
        for n in range(16):
            data=bytes(128*r) if n==0 else rng.randbytes(128*r)
            cases.append((r,data,reference(data,r)))
    with tempfile.TemporaryDirectory(prefix='coax-blockmix-') as temp:
        d=Path(temp);header=d/'scrypt_blockmix.h'
        fixtures=d/'cases.json';fixtures.write_text(json.dumps([{'r':r,'input':list(b),'expected':list(e)} for r,b,e in cases]))
        for mode,flags in [('checked',[]),('release',['--release'])]:
            cfile=d/(mode+'.c');lib=d/(mode+'.dylib')
            generated=subprocess.run([str(COMPILER),'c',str(SOURCE),'--library','--header',str(header),*flags],capture_output=True,check=True)
            cfile.write_bytes(generated.stdout)
            subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC',str(cfile),'-o',str(lib)],check=True)
            fn=C.CDLL(str(lib)).scrypt_blockmix;fn.argtypes=[View,View,C.c_int64,C.c_bool];fn.restype=C.c_bool
            for r,data,expected in cases:
                n=len(data);a=(C.c_uint8*n).from_buffer_copy(data);out=(C.c_uint8*(n+1))();out[n]=165
                assert fn(View(a,n),View(out,n+1),r,False)
                assert bytes(out[:n])==expected and out[n]==165
            mainfile=d/'main.c';mainfile.write_text('int bm_run(int); int bm_refusal(int); int main(void){for(int i=0;i<5;i++)if(!bm_refusal(i))return 1;for(int r=1;r<=8;r++)if(!bm_run(r))return 1;return 0;}\n')
            subprocess.run(['cc','-std=c11','-O1','-fsanitize=address,undefined','-I'+str(d),str(cfile),str(ROOT/'coax/tests/nostr/scrypt_blockmix_wasm.c'),str(mainfile),'-o',str(d/'asan')],check=True)
            subprocess.run([str(d/'asan')],check=True)
            sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip()
            subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2','-c',str(cfile),'-o',str(d/'ios.o')],check=True)
            module=d/(mode+'.wasm')
            subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-I'+str(d),'-mexec-model=reactor',str(cfile),str(ROOT/'coax/tests/nostr/scrypt_blockmix_wasm.c'),'-Wl,--export=bm_input,--export=bm_output,--export=bm_run,--export=bm_refusal','-o',str(module)],check=True)
            subprocess.run(['node',str(ROOT/'coax/tests/nostr/scrypt_blockmix_wasm.cjs'),str(module),str(fixtures)],check=True)
            print(mode,'native:',len(cases),'passed; C sha256',hashlib.sha256(cfile.read_bytes()).hexdigest(),'Wasm sha256',hashlib.sha256(module.read_bytes()).hexdigest(),flush=True)


if __name__=='__main__': main()
