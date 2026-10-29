#!/usr/bin/env python3
"""Bounded AEAD oracle with independent HChaCha and OpenSSL ChaCha20-Poly1305."""
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
SOURCE=ROOT/'coax/lib/crypto/xchacha20_poly1305.coax';COMPILER=ROOT/'coax/bin/coax'
spec=importlib.util.spec_from_file_location('hchacha_reference',ROOT/'coax/tests/nostr/hchacha20.py')
hchacha=importlib.util.module_from_spec(spec);spec.loader.exec_module(hchacha)
class View(C.Structure): _fields_=[('data',C.POINTER(C.c_uint8)),('count',C.c_int64)]


def main():
    rng=random.Random(130520);cases=[]
    lengths=[0,1,15,16,17,31,32,33,63,64,65,1023,1024,4095,4096]+[rng.randrange(4097) for _ in range(115)]+[0,4096]
    for index,n in enumerate(lengths):
        key=rng.randbytes(32);nonce=rng.randbytes(24);a=0 if index==0 else 1024 if index>=len(lengths)-2 else rng.randrange(1025)
        cases.append({'key':list(key),'nonce':list(nonce),'aad':list(rng.randbytes(a)),
                      'message':list(rng.randbytes(n)),'subkey':list(hchacha.reference(key,nonce[:16]))})
    with tempfile.TemporaryDirectory(prefix='coax-xcp-') as temp:
        d=Path(temp);fixtures=d/'cases.json';fixtures.write_text(json.dumps(cases));js=ROOT/'coax/tests/nostr/xchacha20_poly1305_wasm.cjs'
        oracle=json.loads(subprocess.check_output(['node',str(js),'oracle',str(fixtures)]))
        for case,expected in zip(cases,oracle,strict=True):case['expected']=expected
        fixtures.write_text(json.dumps(cases))
        for mode,flags in [('checked',[]),('release',['--release'])]:
            cfile=d/(mode+'.c');lib=d/(mode+'.dylib')
            generated=subprocess.run([str(COMPILER),'c',str(SOURCE),'--library','--header',str(d/'xchacha20_poly1305.h'),*flags],capture_output=True,check=True)
            cfile.write_bytes(generated.stdout)
            subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC',str(cfile),'-o',str(lib)],check=True)
            library=C.CDLL(str(lib));seal=library.xcp_seal;opened=library.xcp_open
            for f in (seal,opened):f.argtypes=[View,View,View,View,View,C.c_bool];f.restype=C.c_bool
            for c in cases:
                key=(C.c_uint8*32)(*c['key']);nonce=(C.c_uint8*24)(*c['nonce']);a=len(c['aad']);aad=(C.c_uint8*a)(*c['aad'])
                n=len(c['message']);msg=(C.c_uint8*n)(*c['message']);box=(C.c_uint8*(n+17))();box[n+16]=165
                assert seal(View(key,32),View(nonce,24),View(aad,a),View(msg,n),View(box,n+17),False)
                assert list(box[:n+16])==c['expected'] and box[n+16]==165
                plain=(C.c_uint8*(n+1))();plain[n]=165
                assert opened(View(key,32),View(nonce,24),View(aad,a),View(box,n+16),View(plain,n+1),False)
                assert list(plain[:n])==c['message'] and plain[n]==165
                for i in range(n+1):plain[i]=165
                for i in range(n,n+16):
                    box[i]^=1
                    assert not opened(View(key,32),View(nonce,24),View(aad,a),View(box,n+16),View(plain,n+1),False)
                    assert all(v==165 for v in plain)
                    box[i]^=1
                if n:
                    box[0]^=1
                    assert not opened(View(key,32),View(nonce,24),View(aad,a),View(box,n+16),View(plain,n+1),False)
                    assert all(v==165 for v in plain)
                    box[0]^=1
                nonce[0]^=1
                assert not opened(View(key,32),View(nonce,24),View(aad,a),View(box,n+16),View(plain,n+1),False)
                assert all(v==165 for v in plain)
                nonce[0]^=1
                if a:
                    aad[0]^=1
                    assert not opened(View(key,32),View(nonce,24),View(aad,a),View(box,n+16),View(plain,n+1),False)
                    assert all(v==165 for v in plain)
            mainfile=d/'main.c';mainfile.write_text('#include <string.h>\nunsigned char *aead_input(void); unsigned char *aead_output(void); int aead_seal(int,int); int aead_tamper(int,int,int); int aead_open(int,int); int main(void){if(!aead_seal(1024,4096))return 1;memcpy(aead_input(),aead_output(),4112);if(!aead_tamper(1024,4112,4111))return 1;return !aead_open(1024,4112);}\n')
            subprocess.run(['cc','-std=c11','-O1','-fsanitize=address,undefined','-I'+str(d),str(cfile),str(ROOT/'coax/tests/nostr/xchacha20_poly1305_wasm.c'),str(mainfile),'-o',str(d/'asan')],check=True)
            subprocess.run([str(d/'asan')],check=True)
            sdk=subprocess.check_output(['xcrun','--sdk','iphonesimulator','--show-sdk-path'],text=True).strip()
            subprocess.run(['xcrun','--sdk','iphonesimulator','clang','-arch','arm64','-isysroot',sdk,'-std=c11','-O2','-c',str(cfile),'-o',str(d/'ios.o')],check=True)
            module=d/(mode+'.wasm')
            subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-I'+str(d),'-mexec-model=reactor',str(cfile),str(ROOT/'coax/tests/nostr/xchacha20_poly1305_wasm.c'),'-Wl,--export=aead_key,--export=aead_nonce,--export=aead_aad,--export=aead_input,--export=aead_output,--export=aead_seal,--export=aead_open,--export=aead_tamper','-o',str(module)],check=True)
            subprocess.run(['node',str(js),str(module),str(fixtures)],check=True)
            print(mode,len(cases),'independent cases; C sha256',hashlib.sha256(cfile.read_bytes()).hexdigest(),'Wasm sha256',hashlib.sha256(module.read_bytes()).hexdigest(),flush=True)


if __name__=='__main__': main()
