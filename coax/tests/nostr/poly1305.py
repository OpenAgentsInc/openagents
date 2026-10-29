#!/usr/bin/env python3
"""Compare bounded Poly1305 with an arbitrary-precision RFC model."""
if not __debug__: raise SystemExit('tests require Python assertions')
import ctypes as C
import hashlib
import json
import random
import subprocess
import tempfile
from pathlib import Path

ROOT=Path(__file__).resolve().parents[3]
SOURCE=ROOT/'coax/lib/crypto/poly1305.coax'
COMPILER=ROOT/'coax/bin/coax'


def reference(key,message):
    r=int.from_bytes(key[:16],'little') & 0x0ffffffc0ffffffc0ffffffc0fffffff
    s=int.from_bytes(key[16:],'little');a=0;p=(1<<130)-5
    for i in range(0,len(message),16):
        block=message[i:i+16]
        a=((a+int.from_bytes(block+b'\x01','little'))*r)%p
    return ((a+s)% (1<<128)).to_bytes(16,'little')


class View(C.Structure): _fields_=[('data',C.POINTER(C.c_uint8)),('count',C.c_int64)]


def main():
    rng=random.Random(8439);cases=[]
    for n in [0,1,2,15,16,17,31,32,33,63,64,65,255,256,257,1024]+[rng.randrange(257) for _ in range(1000)]:
        key=rng.randbytes(32);message=rng.randbytes(n);cases.append((key,message,reference(key,message)))
    for n in [0,1,15,16,17,31,32,33,1024]:
        key=bytes([255]*32);message=bytes([255]*n);cases.append((key,message,reference(key,message)))
    key=bytes([1]+[0]*31);message=bytes([255]*16+[252]+[255]*15);cases.append((key,message,reference(key,message)))
    key=rng.randbytes(32);message=rng.randbytes(1048576);cases.append((key,message,reference(key,message)))
    for key,message,expected in cases[:100]+cases[-10:]:
        oracle=subprocess.run(['openssl','mac','-macopt','hexkey:'+key.hex(),'POLY1305'],input=message,capture_output=True,check=True)
        assert bytes.fromhex(oracle.stdout.decode().strip())==expected
    with tempfile.TemporaryDirectory(prefix='coax-poly1305-') as temp:
        d=Path(temp)
        fixtures=d/'cases.json';fixtures.write_text(json.dumps([{'key':list(k),'message':list(m),'expected':list(e)} for k,m,e in cases]))
        for mode,flags in [('checked',[]),('release',['--release'])]:
            cfile=d/(mode+'.c');lib=d/(mode+'.dylib')
            generated=subprocess.run([str(COMPILER),'c',str(SOURCE),'--library','--header',str(d/'poly1305.h'),*flags],capture_output=True,check=True)
            cfile.write_bytes(generated.stdout)
            subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC',str(cfile),'-o',str(lib)],check=True)
            fn=C.CDLL(str(lib)).poly1305_checked;fn.argtypes=[View,View,View,C.c_bool];fn.restype=C.c_bool
            for key,message,expected in cases:
                a=(C.c_uint8*32).from_buffer_copy(key);b=(C.c_uint8*len(message)).from_buffer_copy(message);out=(C.c_uint8*17)();out[16]=165
                assert fn(View(a,32),View(b,len(message)),View(out,17),False)
                assert bytes(out[:16])==expected and out[16]==165
            mainfile=d/'main.c';mainfile.write_text('int poly_run(int); int poly_refusal(int); int main(void){for(int i=0;i<4;i++)if(!poly_refusal(i))return 1;for(int n=0;n<=1024;n+=16)if(!poly_run(n))return 1;return 0;}\n')
            subprocess.run(['cc','-std=c11','-O1','-fsanitize=address,undefined','-I'+str(d),str(cfile),str(ROOT/'coax/tests/nostr/poly1305_wasm.c'),str(mainfile),'-o',str(d/'asan')],check=True)
            subprocess.run([str(d/'asan')],check=True)
            sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip()
            subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2','-c',str(cfile),'-o',str(d/'ios.o')],check=True)
            module=d/(mode+'.wasm')
            subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-I'+str(d),'-mexec-model=reactor',str(cfile),str(ROOT/'coax/tests/nostr/poly1305_wasm.c'),'-Wl,--export=poly_key,--export=poly_message,--export=poly_output,--export=poly_run,--export=poly_refusal','-o',str(module)],check=True)
            subprocess.run(['node',str(ROOT/'coax/tests/nostr/poly1305_wasm.cjs'),str(module),str(fixtures)],check=True)
            print(mode,len(cases),'independent cases; C sha256',hashlib.sha256(cfile.read_bytes()).hexdigest(),'Wasm sha256',hashlib.sha256(module.read_bytes()).hexdigest(),flush=True)


if __name__=='__main__': main()
