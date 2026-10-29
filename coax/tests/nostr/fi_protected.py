#!/usr/bin/env python3
"""Independent closed-header Python oracle for the unintegrated prototype."""
import ctypes as c
import base64
import json
import hashlib
import os
from pathlib import Path
import random
import subprocess
import tempfile

ROOT=Path(__file__).resolve().parents[3]
SOURCE=ROOT/'coax/lib/nostr/block/fi-protected.coax'
COMPILER=Path(os.environ.get('COAX_COMPILER',ROOT/'coax/bin/coax'))
class String(c.Structure): _fields_=[('data',c.c_char_p),('count',c.c_int64)]
class Header(c.Structure): _fields_=[('ok',c.c_bool),('typ',c.c_int64),('kid',c.c_uint8*128),('kid_count',c.c_int64)]

def reference(text):
 def pairs(items):
  result={}
  for key,value in items:
   if key in result: raise ValueError('duplicate')
   result[key]=value
  return result
 try: value=json.loads(text,object_pairs_hook=pairs)
 except (ValueError,UnicodeError): return None
 if not isinstance(value,dict) or set(value)!={'alg','typ','kid'}: return None
 if value['alg']!='RS256' or value['typ'] not in ('nip-fi+jwt','nip-fi-agent+jwt','at+jwt'): return None
 kid=value['kid']
 if not isinstance(kid,str) or not 1<=len(kid)<=128 or not all(33<=ord(b)<=126 for b in kid): return None
 return (('nip-fi+jwt','nip-fi-agent+jwt','at+jwt').index(value['typ'])+1,kid.encode())

def main():
 rng=random.Random(20261028)
 cases=[b'{}',b'[]',b'null',b'{"alg":"RS256","\\u0061lg":"RS256","typ":"at+jwt","kid":"x"}',b'{"alg":"RS256","typ":"at+jwt","kid":"x","crit":[]}',b'{"alg":"RS256","typ":"at+jwt","kid":"x"} true']
 for _ in range(500):
  obj={'alg':rng.choice(['RS256','HS256','none','']), 'typ':rng.choice(['nip-fi+jwt','nip-fi-agent+jwt','at+jwt','JWT']), 'kid':rng.choice(['x','x'*128,'x'*129,'','x y',1])}
  if rng.randrange(5)==0: obj[rng.choice(['jku','jwk','x5u','crit','other'])]='bad'
  if rng.randrange(7)==0: del obj[rng.choice(['alg','typ','kid'])]
  cases.append(json.dumps(obj,separators=(',',':')).encode())
 with tempfile.TemporaryDirectory(prefix='fi-header-') as tmp:
  fixtures=Path(tmp)/'vectors.json'
  fixtures.write_text(json.dumps([{'text':base64.b64encode(t).decode(),'typ':reference(t)[0] if reference(t) else 0,'kid':reference(t)[1].hex() if reference(t) else ''} for t in cases]))
  for mode in ('checked','release'):
   generated=Path(tmp)/'generated.c';lib=Path(tmp)/(mode+'.dylib')
   command=[str(COMPILER),'c',str(SOURCE),'--library','-o',str(generated)]
   if mode=='release':command.append('--release')
   subprocess.run(command,check=True)
   subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC',str(generated),'-lm','-o',str(lib)],check=True)
   parse=c.CDLL(str(lib)).fi_protected_parse;parse.argtypes=[String];parse.restype=Header
   for text in cases:
    got=parse(String(text,len(text)));want=reference(text)
    assert got.ok==(want is not None),(mode,text)
    if want is not None: assert (got.typ,bytes(got.kid[:got.kid_count]))==want,(mode,text)
    else: assert got.typ==0 and got.kid_count==0,(mode,text)
   print(f'{mode}: {len(cases)} closed-header cases match independent Python oracle',flush=True)
   wasm=Path(tmp)/(mode+'.wasm')
   subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1',
    '--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot',
    '-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-I'+tmp,
    str(ROOT/'coax/tests/nostr/fi_protected_wasm.c'),'-mexec-model=reactor',
    '-Wl,--import-memory,--export-memory,--max-memory=67108864,--export=__heap_base','-lm','-o',str(wasm)],check=True)
   print(f'{mode} generated_c_sha256={hashlib.sha256(generated.read_bytes()).hexdigest()} native_sha256={hashlib.sha256(lib.read_bytes()).hexdigest()} wasm_sha256={hashlib.sha256(wasm.read_bytes()).hexdigest()}',flush=True)
   subprocess.run(['node',str(ROOT/'coax/tests/nostr/fi_protected_wasm.cjs'),str(wasm),str(fixtures)],check=True)

if __name__=='__main__':main()
