#!/usr/bin/env python3
"""Independent HChaCha model plus OpenSSL ChaCha20 cross-target oracle."""
if not __debug__: raise SystemExit('tests require Python assertions')
import ctypes as C
import hashlib
import importlib.util
import json
import random
import subprocess
import tempfile
from pathlib import Path

ROOT=Path(__file__).resolve().parents[3]
SOURCE=ROOT/'coax/lib/crypto/xchacha20.coax';COMPILER=ROOT/'coax/bin/coax'
spec=importlib.util.spec_from_file_location('hchacha_reference',ROOT/'coax/tests/nostr/hchacha20.py')
hchacha=importlib.util.module_from_spec(spec);spec.loader.exec_module(hchacha)
class View(C.Structure): _fields_=[('data',C.POINTER(C.c_uint8)),('count',C.c_int64)]
class Result(C.Structure): _fields_=[('written',C.c_int64),('error',C.c_uint32)]


def main():
    rng=random.Random(202024);cases=[]
    for n in [0,1,15,16,31,32,63,64,65,127,128,129,1024]+[rng.randrange(1025) for _ in range(500)]:
        key=rng.randbytes(32);nonce=rng.randbytes(24);counter=min(rng.getrandbits(32),0x100000000-((n+63)//64))
        cases.append({'key':list(key),'nonce':list(nonce),'input':list(rng.randbytes(n)),
                      'subkey':list(hchacha.reference(key,nonce[:16])),'counter':counter})
    with tempfile.TemporaryDirectory(prefix='coax-xchacha-') as temp:
        d=Path(temp);fixtures=d/'cases.json';fixtures.write_text(json.dumps(cases))
        js=ROOT/'coax/tests/nostr/xchacha20_wasm.cjs'
        oracle=json.loads(subprocess.check_output(['node',str(js),'oracle',str(fixtures)]))
        for case,expected in zip(cases,oracle,strict=True):case['expected']=expected
        fixtures.write_text(json.dumps(cases))
        for mode,flags in [('checked',[]),('release',['--release'])]:
            cfile=d/(mode+'.c');lib=d/(mode+'.dylib')
            generated=subprocess.run([str(COMPILER),'c',str(SOURCE),'--library','--header',str(d/'xchacha20.h'),*flags],capture_output=True,check=True)
            cfile.write_bytes(generated.stdout)
            subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC',str(cfile),'-o',str(lib)],check=True)
            fn=C.CDLL(str(lib)).xchacha20_xor_checked;fn.argtypes=[View,View,C.c_uint32,View,View,C.c_bool];fn.restype=Result
            for c in cases:
                key=(C.c_uint8*32)(*c['key']);nonce=(C.c_uint8*24)(*c['nonce']);n=len(c['input'])
                message=(C.c_uint8*n)(*c['input']);out=(C.c_uint8*(n+1))();out[n]=165
                result=fn(View(key,32),View(nonce,24),c['counter'],View(message,n),View(out,n+1),False)
                assert result.error==0 and result.written==n and list(out[:n])==c['expected'] and out[n]==165
            mainfile=d/'main.c';mainfile.write_text('int xc_run(int,unsigned); int xc_refusal(int); int main(void){for(int i=0;i<5;i++)if(!xc_refusal(i))return 1;for(int n=0;n<=1024;n+=16)if(!xc_run(n,0))return 1;return 0;}\n')
            subprocess.run(['cc','-std=c11','-O1','-fsanitize=address,undefined','-I'+str(d),str(cfile),str(ROOT/'coax/tests/nostr/xchacha20_wasm.c'),str(mainfile),'-o',str(d/'asan')],check=True)
            subprocess.run([str(d/'asan')],check=True)
            sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip()
            subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2','-c',str(cfile),'-o',str(d/'ios.o')],check=True)
            module=d/(mode+'.wasm')
            subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-I'+str(d),'-mexec-model=reactor',str(cfile),str(ROOT/'coax/tests/nostr/xchacha20_wasm.c'),'-Wl,--export=xc_key,--export=xc_nonce,--export=xc_input,--export=xc_output,--export=xc_run,--export=xc_refusal','-o',str(module)],check=True)
            subprocess.run(['node',str(js),str(module),str(fixtures)],check=True)
            print(mode,len(cases),'independent cases; C sha256',hashlib.sha256(cfile.read_bytes()).hexdigest(),'Wasm sha256',hashlib.sha256(module.read_bytes()).hexdigest(),flush=True)


if __name__=='__main__': main()
