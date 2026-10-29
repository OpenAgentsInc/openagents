#!/usr/bin/env python3
"""Independent JWKS lookup/composition checks; no provider authority is inferred."""
import base64
import ctypes
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[3]
DATA = ROOT / 'coax/tests/nostr/data/fi-jws'
SOURCE = ROOT / 'coax/lib/nostr/block/fi-jws-jwks.coax'
COMPILER = Path(os.environ.get('COAX_COMPILER', ROOT / 'coax/bin/coax'))

def enc(b): return base64.urlsafe_b64encode(b).rstrip(b'=').decode()
def dump(v): return json.dumps(v, separators=(',', ':')).encode()

def main():
    token = (DATA / 'token.txt').read_bytes().strip()
    modulus = bytes.fromhex((DATA / 'modulus.hex').read_text())
    h, p, s = token.split(b'.')
    signature = base64.urlsafe_b64decode(s + b'=' * (-len(s) % 4))
    original = {'kid':'test-key','kty':'RSA','alg':'RS256','use':'sig','n':enc(modulus),'e':'AQAB'}
    other = dict(original, kid='other')
    wrong = dict(original, n=enc(bytes([modulus[0] ^ 1]) + modulus[1:]))
    bad = dict(other, d='AQ')
    cases = [
        (token, dump({'keys':[original]}), 1),
        (token, dump({'keys':[other,original]}), 1),
        (token, dump({'keys':[original,other]}), 1),
        (token, dump({'keys':[other]}), 0),
        (token, dump({'keys':[wrong]}), 0),
        (token, dump({'keys':[original,original]}), 0),
        (token, dump({'keys':[original,bad]}), 0),
        (token, dump({'keys':[dict(original, e='Aw')]}), 0),
        (token, dump({'keys':[dict(original, key_ops=['sign'])]}), 0),
        (token, dump({'keys':[]}), 0),
        (h+b'.'+enc(b'{"sub":"fixture", "iat":1700000000}').encode()+b'.'+s, dump({'keys':[original]}), 0),
        (h+b'.'+p+b'.'+enc(bytes([signature[0]^1])+signature[1:]).encode(), dump({'keys':[original]}), 0),
        (enc(b'{"alg":"HS256","typ":"nip-fi+jwt","kid":"test-key"}').encode()+b'.'+p+b'.'+s, dump({'keys':[original]}), 0),
    ]
    with tempfile.TemporaryDirectory(prefix='fi-jws-jwks-') as td:
        tmp = Path(td)
        (tmp/'message').write_bytes(h+b'.'+p)
        (tmp/'signature').write_bytes(signature)
        subprocess.run(['openssl','dgst','-sha256','-verify',str(DATA/'public.pem'),'-signature',str(tmp/'signature'),str(tmp/'message')],check=True,stdout=subprocess.DEVNULL)
        (tmp/'message').write_bytes(h+b'.'+enc(b'{"sub":"fixture", "iat":1700000000}').encode())
        assert subprocess.run(['openssl','dgst','-sha256','-verify',str(DATA/'public.pem'),'-signature',str(tmp/'signature'),str(tmp/'message')],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL).returncode != 0
        (tmp/'message').write_bytes(h+b'.'+p)
        (tmp/'signature').write_bytes(bytes([signature[0]^1])+signature[1:])
        assert subprocess.run(['openssl','dgst','-sha256','-verify',str(DATA/'public.pem'),'-signature',str(tmp/'signature'),str(tmp/'message')],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL).returncode != 0
        vectors = tmp/'vectors.json'
        vectors.write_text(json.dumps([{'token':enc(t),'jwks':enc(j),'want':w} for t,j,w in cases]))
        for mode in ('checked','release'):
            generated = tmp/'generated.c'
            cmd = [str(COMPILER),'c',str(SOURCE),'--library','-o',str(generated)]
            if mode == 'release': cmd.append('--release')
            subprocess.run(cmd,check=True)
            wrapper = tmp/'wrapper.c'
            wrapper.write_text('#include "generated.c"\nint verify(const uint8_t *t,int64_t tn,const uint8_t *j,int64_t jn){return fi_jws_document_signature((coax_v_u8){(uint8_t*)t,tn},(coax_string){(const char*)j,jn});}\n')
            lib = tmp/(mode+'.dylib')
            subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC',str(wrapper),'-lm','-o',str(lib)],check=True)
            f = ctypes.CDLL(str(lib)).verify
            ptr = ctypes.POINTER(ctypes.c_uint8)
            f.argtypes = [ptr,ctypes.c_int64,ptr,ctypes.c_int64]; f.restype = ctypes.c_int
            for t,j,w in cases:
                tb=(ctypes.c_uint8*len(t)).from_buffer_copy(t); jb=(ctypes.c_uint8*len(j)).from_buffer_copy(j)
                assert f(tb,len(t),jb,len(j)) == w, (mode, t, j)
            # Exercise the same independently constructed rows with allocation and UB instrumentation.
            sanitizer = tmp/'sanitizer.c'
            lines = ['#include "generated.c"', '#include <assert.h>', 'int main(void) {']
            for i,(t,j,w) in enumerate(cases):
                tb=','.join(map(str,t)); jb=','.join(map(str,j))
                lines.append(f'uint8_t t{i}[]={{ {tb} }}; const char j{i}[]={{ {jb} }};')
                lines.append(f'assert(fi_jws_document_signature((coax_v_u8){{t{i},sizeof(t{i})}},(coax_string){{j{i},sizeof(j{i})}})=={w});')
            lines.append('return 0; }')
            sanitizer.write_text('\n'.join(lines))
            executable = tmp/(mode+'-sanitized')
            subprocess.run(['cc','-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer',str(sanitizer),'-lm','-o',str(executable)],check=True)
            subprocess.run([str(executable)],check=True,timeout=60)
            wasm = tmp/(mode+'.wasm')
            subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-I'+str(tmp),str(ROOT/'coax/tests/nostr/fi_jws_jwks_wasm.c'),'-mexec-model=reactor','-Wl,--import-memory,--export-memory,--max-memory=67108864,--export=__heap_base','-lm','-o',str(wasm)],check=True)
            subprocess.run(['node',str(ROOT/'coax/tests/nostr/fi_jws_jwks_wasm.cjs'),str(wasm),str(vectors)],check=True)
            print(f'{mode}: {len(cases)} native and Wasm composition cases passed',flush=True)
if __name__ == '__main__': main()
