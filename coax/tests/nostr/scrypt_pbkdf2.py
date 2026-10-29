#!/usr/bin/env python3
"""One-iteration PBKDF2 compared with OpenSSL-backed hashlib."""
if not __debug__: raise SystemExit('tests require Python assertions')
import ctypes as C
import hashlib
import json
import random
import subprocess
import tempfile
from pathlib import Path

ROOT=Path(__file__).resolve().parents[3]
SOURCE=ROOT/'coax/lib/crypto/scrypt_pbkdf2.coax';COMPILER=ROOT/'coax/bin/coax'
class View(C.Structure): _fields_=[('data',C.POINTER(C.c_uint8)),('count',C.c_int64)]


def main():
    rng=random.Random(791411);cases=[]
    sizes=[0,1,31,32,33,63,64,65,1023,1024]
    for i in range(100):
        pn=sizes[i%len(sizes)] if i<20 else rng.randrange(1025)
        sn=sizes[(i//len(sizes))%len(sizes)] if i<20 else rng.randrange(1025)
        n=sizes[(i+1)%len(sizes)] if i<20 else rng.randrange(1,1025)
        if n==0:n=1
        password=rng.randbytes(pn);salt=rng.randbytes(sn)
        cases.append({'password':list(password),'salt':list(salt),'expected':list(hashlib.pbkdf2_hmac('sha256',password,salt,1,n))})
    for pn,sn,n in [(0,0,1),(1024,1024,1024),(0,1024,1024),(1024,0,1024)]:
        password=rng.randbytes(pn);salt=rng.randbytes(sn)
        cases.append({'password':list(password),'salt':list(salt),'expected':list(hashlib.pbkdf2_hmac('sha256',password,salt,1,n))})
    with tempfile.TemporaryDirectory(prefix='coax-pb1-') as temp:
        d=Path(temp);fixtures=d/'cases.json';fixtures.write_text(json.dumps(cases))
        for mode,flags in [('checked',[]),('release',['--release'])]:
            cfile=d/(mode+'.c');lib=d/(mode+'.dylib')
            generated=subprocess.run([str(COMPILER),'c',str(SOURCE),'--library','--header',str(d/'scrypt_pbkdf2.h'),*flags],capture_output=True,check=True)
            cfile.write_bytes(generated.stdout)
            subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC',str(cfile),'-o',str(lib)],check=True)
            fn=C.CDLL(str(lib)).scrypt_pbkdf2_one;fn.argtypes=[View,View,C.c_int64,View,C.c_bool];fn.restype=C.c_bool
            for c in cases:
                pn=len(c['password']);sn=len(c['salt']);n=len(c['expected'])
                password=(C.c_uint8*pn)(*c['password']);salt=(C.c_uint8*sn)(*c['salt']);out=(C.c_uint8*(n+1))();out[n]=165
                assert fn(View(password,pn),View(salt,sn),n,View(out,n+1),False)
                assert list(out[:n])==c['expected'] and out[n]==165
            mainfile=d/'main.c';mainfile.write_text('int pb_run(int,int,int); int pb_refusal(int); int main(void){for(int i=0;i<5;i++)if(!pb_refusal(i))return 1;return !pb_run(1024,1024,1024);}\n')
            subprocess.run(['cc','-std=c11','-O1','-fsanitize=address,undefined','-I'+str(d),str(cfile),str(ROOT/'coax/tests/nostr/scrypt_pbkdf2_wasm.c'),str(mainfile),'-o',str(d/'asan')],check=True)
            subprocess.run([str(d/'asan')],check=True)
            sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip()
            subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2','-c',str(cfile),'-o',str(d/'ios.o')],check=True)
            module=d/(mode+'.wasm')
            subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-I'+str(d),'-mexec-model=reactor',str(cfile),str(ROOT/'coax/tests/nostr/scrypt_pbkdf2_wasm.c'),'-Wl,--export=pb_password,--export=pb_salt,--export=pb_output,--export=pb_run,--export=pb_refusal','-o',str(module)],check=True)
            subprocess.run(['node',str(ROOT/'coax/tests/nostr/scrypt_pbkdf2_wasm.cjs'),str(module),str(fixtures)],check=True)
            print(mode,len(cases),'independent cases; C sha256',hashlib.sha256(cfile.read_bytes()).hexdigest(),'Wasm sha256',hashlib.sha256(module.read_bytes()).hexdigest(),flush=True)


if __name__=='__main__': main()
