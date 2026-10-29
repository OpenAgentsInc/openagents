#!/usr/bin/env python3
"""Independent closed-JWKS Python oracle; no transport or authorization."""
import base64
import copy
import ctypes as c
import json
import hashlib
import os
from pathlib import Path
import random
import subprocess
import tempfile

ROOT=Path(__file__).resolve().parents[3]
SOURCE=ROOT/'coax/lib/nostr/block/fi-jwks-parse.coax'
COMPILER=Path(os.environ.get('COAX_COMPILER',ROOT/'coax/bin/coax'))
class String(c.Structure): _fields_=[('data',c.c_char_p),('count',c.c_int64)]
class Key(c.Structure): _fields_=[('ok',c.c_bool),('has_key_ops',c.c_bool),('kid',c.c_uint8*128),('kid_count',c.c_int64),('modulus',c.c_uint8*512),('modulus_count',c.c_int64)]
class Snapshot(c.Structure): _fields_=[('ok',c.c_bool),('count',c.c_int64),('keys',Key*16)]

def reference(text):
 def pairs(items):
  out={}
  for k,v in items:
   if k in out: raise ValueError('duplicate')
   out[k]=v
  return out
 def uint(value):
  if not isinstance(value,str): raise ValueError('string required')
  raw=value.encode('ascii')
  if b'+' in raw or b'/' in raw: raise ValueError('alphabet')
  data=base64.b64decode(raw+b'='*(-len(raw)%4),altchars=b'-_',validate=True)
  if base64.urlsafe_b64encode(data).rstrip(b'=')!=raw or not data or data[0]==0 or len(data)>512: raise ValueError('uint')
  return data
 try:
  obj=json.loads(text,object_pairs_hook=pairs)
  if not isinstance(obj,dict) or set(obj)!={'keys'} or not isinstance(obj['keys'],list) or not 1<=len(obj['keys'])<=16: return None
  result=[]
  for key in obj['keys']:
   if not isinstance(key,dict) or not {'kid','kty','alg','use','n','e'}<=set(key) or set(key)-{'kid','kty','alg','use','n','e','key_ops'}: return None
   if key['kty']!='RSA' or key['alg']!='RS256' or key['use']!='sig' or ('key_ops' in key and key['key_ops']!=['verify']): return None
   kid=key['kid']
   if not isinstance(kid,str) or not 1<=len(kid)<=128 or not all(33<=ord(x)<=126 for x in kid): return None
   n=uint(key['n']);e=uint(key['e'])
   if not 256<=len(n)<=512 or (len(n)==256 and n[0]<128) or n[-1]%2==0 or e!=b'\x01\x00\x01': return None
   if any(prior[0]==kid.encode() for prior in result): return None
   result.append((kid.encode(),n,'key_ops' in key))
  return result
 except (ValueError,UnicodeError,TypeError): return None

def main():
 rng=random.Random(20261028);n=bytes([128])+bytes(254)+b'\x01'
 key={'kid':'k','kty':'RSA','alg':'RS256','use':'sig','n':base64.urlsafe_b64encode(n).rstrip(b'=').decode(),'e':'AQAB'}
 cases=[b'{"keys":[]}',b'{"keys":[],"keys":[]}',b'{}']
 for _ in range(350):
  k=copy.deepcopy(key);choice=rng.randrange(12)
  if choice==1:k['alg']='HS256'
  if choice==2:k['use']='enc'
  if choice==3:k['e']='Aw'
  if choice==4:k['n']='AA'+k['n']
  if choice==5:k['kid']='x'*129
  if choice==6:k['d']='AA'
  if choice==7:k['key_ops']=['sign']
  if choice==8:k['key_ops']=['verify']
  if choice==9:del k['use']
  if choice==10:k['kty']='EC'
  if choice==11:k['kid']='x y'
  cases.append(json.dumps({'keys':[k]},separators=(',',':')).encode())
 cases.extend([json.dumps({'keys':[key,key]}).encode(),json.dumps({'keys':[key,{**key,'kid':'m'}]}).encode()])
 cases.extend([json.dumps({'keys':[{**key,'kid':str(i)} for i in range(count)]}).encode() for count in (16,17)])
 for count in (255,256,257,511,512,513):
  for first in (1,127,128,255):
   raw=bytes([first])+bytes(count-2)+b'\x01'
   cases.append(json.dumps({'keys':[{**key,'n':base64.urlsafe_b64encode(raw).rstrip(b'=').decode()}]}).encode())
 cases.extend([json.dumps({'keys':[{**key,'key_ops':ops}]}).encode() for ops in ([],['verify'],['verify','sign'],'verify',None)])
 cases.append(json.dumps({'keys':[key],'other':1}).encode())
 cases.append(json.dumps({'keys':[{**key,'e':'AQAB='}]}).encode())
 encoded=json.dumps({'keys':[key]},separators=(',',':')).encode()
 cases.append(encoded.replace(b'"kid":"k"',b'"\\u006bid":"k"'))
 cases.append(encoded.replace(b'"kid":"k"',b'"kid":"k","\\u006bid":"other"'))
 cases.append(encoded.replace(b'"keys":',b'"\\u006beys":'))
 cases.append(encoded.replace(b'"keys":',b'"keys":[],"\\u006beys":'))
 with tempfile.TemporaryDirectory(prefix='fi-jwks-') as tmp:
  fixtures=Path(tmp)/'vectors.json'
  fixtures.write_text(json.dumps([{'text':base64.b64encode(t).decode(),'expected':None if reference(t) is None else [[kid.hex(),n.hex(),has] for kid,n,has in reference(t)]} for t in cases]))
  for mode in ('checked','release'):
   generated=Path(tmp)/'generated.c';lib=Path(tmp)/(mode+'.dylib')
   command=[str(COMPILER),'c',str(SOURCE),'--library','-o',str(generated)]
   if mode=='release':command.append('--release')
   subprocess.run(command,check=True);subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC',str(generated),'-lm','-o',str(lib)],check=True)
   parse=c.CDLL(str(lib)).fi_jwks_parse;parse.argtypes=[String];parse.restype=Snapshot
   for text in cases:
    got=parse(String(text,len(text)));want=reference(text)
    assert got.ok==(want is not None),(mode,text)
    if want is None: assert got.count==0,(mode,text)
    else:
     assert got.count==len(want),(mode,text)
     for i,(kid,modulus,has) in enumerate(want):
      assert bytes(got.keys[i].kid[:got.keys[i].kid_count])==kid and bytes(got.keys[i].modulus[:got.keys[i].modulus_count])==modulus and got.keys[i].has_key_ops==has,(mode,text)
   print(f'{mode}: {len(cases)} JWKS cases match independent Python oracle',flush=True)
   wasm=Path(tmp)/(mode+'.wasm')
   subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1',
    '--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot',
    '-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-I'+tmp,
    str(ROOT/'coax/tests/nostr/fi_jwks_wasm.c'),'-mexec-model=reactor',
    '-Wl,--import-memory,--export-memory,--max-memory=67108864,--export=__heap_base','-lm','-o',str(wasm)],check=True)
   print(f'{mode} generated_c_sha256={hashlib.sha256(generated.read_bytes()).hexdigest()} native_sha256={hashlib.sha256(lib.read_bytes()).hexdigest()} wasm_sha256={hashlib.sha256(wasm.read_bytes()).hexdigest()}',flush=True)
   subprocess.run(['node',str(ROOT/'coax/tests/nostr/fi_jwks_wasm.cjs'),str(wasm),str(fixtures)],check=True)
if __name__=='__main__':main()
