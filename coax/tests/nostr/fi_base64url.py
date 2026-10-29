#!/usr/bin/env python3
"""Test-only independent Python oracle for the FI base64url primitive."""
import base64
import ctypes as c
import json
import hashlib
import os
from pathlib import Path
import random
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[3]
SOURCE = ROOT / 'coax/lib/nostr/block/fi-base64url.coax'
COMPILER = Path(os.environ.get('COAX_COMPILER', ROOT / 'coax/bin/coax'))
WRAPPER = '''
#include "generated.c"
int decode(const uint8_t *input,int64_t n,uint8_t *out,int64_t cap,int64_t *written) {
 coax_t_NC_Result r=coax_p_fi_b64url_decode((coax_string){(const char *)input,n},(coax_v_u8){out,cap});
 *written=r.written;return r.error;
}
'''

def reference(text):
 if b'+' in text or b'/' in text: return None
 try:
  raw = base64.b64decode(text + b'=' * (-len(text) % 4), altchars=b'-_', validate=True)
  return raw if base64.urlsafe_b64encode(raw).rstrip(b'=') == text else None
 except ValueError:
  return None

def main():
 rng = random.Random(20261028)
 texts = [base64.urlsafe_b64encode(rng.randbytes(n)).rstrip(b'=') for n in range(256)]
 texts += [bytes([i]) + b'AAA' for i in range(256)]
 texts += [b'Zg=',b'Zh',b'Zm9',b'AAAA=',b'AA A',b'-_8',b'',b'!AAA']
 with tempfile.TemporaryDirectory(prefix='fi-base64url-') as tmp:
  tmp = Path(tmp)
  fixtures=tmp/'vectors.json'
  fixtures.write_text(json.dumps([{'text':base64.b64encode(t).decode(),'expected':reference(t).hex() if reference(t) is not None else None} for t in texts]))
  for mode in ('checked','release'):
   generated, wrapper, lib = tmp/'generated.c',tmp/'wrapper.c',tmp/(mode+'.dylib')
   command=[str(COMPILER),'c',str(SOURCE),'--library','-o',str(generated)]
   if mode=='release': command.append('--release')
   subprocess.run(command,check=True)
   wrapper.write_text(WRAPPER)
   subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC',str(wrapper),'-lm','-o',str(lib)],check=True)
   decode=c.CDLL(str(lib)).decode
   decode.argtypes=[c.POINTER(c.c_uint8),c.c_int64,c.POINTER(c.c_uint8),c.c_int64,c.POINTER(c.c_int64)]
   decode.restype=c.c_int
   for text in texts:
    data=(c.c_uint8*max(1,len(text)))();data[:len(text)]=text
    out=(c.c_uint8*512)(*([165]*512));written=c.c_int64(-1)
    status=decode(data,len(text),out,512,c.byref(written));want=reference(text)
    if want is None:
     assert status!=0 and written.value==0 and bytes(out)==bytes([165])*512,(mode,text)
    else:
     assert status==0 and written.value==len(want) and bytes(out[:len(want)])==want,(mode,text)
     assert bytes(out[len(want):])==bytes([165])*(512-len(want)),(mode,text)
   print(f'{mode}: {len(texts)} base64url cases match independent Python oracle',flush=True)
   wasm=tmp/(mode+'.wasm')
   subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1',
    '--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot',
    '-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-I'+str(tmp),
    str(ROOT/'coax/tests/nostr/fi_base64url_wasm.c'),'-mexec-model=reactor',
    '-Wl,--import-memory,--export-memory,--max-memory=67108864,--export=__heap_base','-lm','-o',str(wasm)],check=True)
   print(f'{mode} generated_c_sha256={hashlib.sha256(generated.read_bytes()).hexdigest()} native_sha256={hashlib.sha256(lib.read_bytes()).hexdigest()} wasm_sha256={hashlib.sha256(wasm.read_bytes()).hexdigest()}',flush=True)
   subprocess.run(['node',str(ROOT/'coax/tests/nostr/fi_base64url_wasm.cjs'),str(wasm),str(fixtures)],check=True)

if __name__=='__main__':main()
