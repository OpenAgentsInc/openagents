#!/usr/bin/env python3
"""Independent typed untrusted FI claims oracle; no identity authority."""
import base64
import ctypes as c
import json
import os
from pathlib import Path
import re
import struct
import subprocess
import tempfile
ROOT=Path(__file__).resolve().parents[3]
SOURCE=ROOT/'coax/lib/nostr/block/fi-claims-parse.coax'
COMPILER=Path(os.environ.get('COAX_COMPILER',ROOT/'coax/bin/coax'))
FIXTURE=ROOT/'coax/tests/nostr/fi_claims_fixture.c'
MAX=9007199254740991

def visible(v,n):
 return isinstance(v,str) and 1<=len(v.encode('utf8'))<=n and all(33<=x<=126 for x in v.encode('utf8'))
def key(v):return isinstance(v,str) and bool(re.fullmatch('[0-9a-f]{64}',v))
def oracle(raw,marker):
 try:
  if len(raw)>65536 or (marker and not visible(marker,128)):raise ValueError()
  class Pairs(list):pass
  raw_obj=json.loads(raw.decode('utf8'),object_pairs_hook=Pairs,parse_constant=lambda x: (_ for _ in ()).throw(ValueError(x)))
  if not isinstance(raw_obj,Pairs):raise ValueError()
  obj={}
  for k,v in raw_obj:
   if k in obj:raise ValueError('duplicate top-level name')
   obj[k]=v
  if len(obj)>64 or any(len(k.encode('utf8'))>128 for k in obj):raise ValueError()
  iss=obj['iss'];sub=obj['sub'];aud=obj['aud'];pub=obj['nostr_pubkey'];iat=obj['iat'];exp=obj['exp']
  if not visible(iss,2048) or not visible(sub,256) or not key(pub):raise ValueError()
  if isinstance(aud,str):aud=[aud]
  if not isinstance(aud,list) or not 1<=len(aud)<=16 or any(not visible(x,2048) for x in aud) or len(aud)!=len(set(aud)):raise ValueError()
  def number(v):return type(v) is int and 0<=v<=MAX
  if not number(iat) or not number(exp) or ('nbf' in obj and not number(obj['nbf'])):raise ValueError()
  if 'nostr_owner_pubkey' in obj and not key(obj['nostr_owner_pubkey']):raise ValueError()
  if 'client_id' in obj and not visible(obj['client_id'],256):raise ValueError()
  mark=obj.get(marker) if marker else None
  if marker and marker in obj and not visible(mark,128):raise ValueError()
  def field(v):
   b=v.encode('utf8');return struct.pack('>H',len(b))+b
  flags=3|(4 if 'nbf' in obj else 0)|(8 if 'nostr_owner_pubkey' in obj else 0)|(16 if 'client_id' in obj else 0)|(32 if marker and marker in obj else 0)
  return (field(iss)+field(sub)+bytes([len(aud)])+b''.join(map(field,aud))+field(pub)+bytes([flags])+
          struct.pack('<qqq',iat,exp,obj.get('nbf',0))+field(obj.get('nostr_owner_pubkey',''))+field(obj.get('client_id',''))+field(mark or ''))
 except (ValueError,KeyError,TypeError,UnicodeError):return None
base={'iss':'https://issuer.test','sub':'subject','aud':'wss://relay.test','nostr_pubkey':'0'*64,'iat':1,'exp':2}
dump=lambda x:json.dumps(x,separators=(',',':'),ensure_ascii=False).encode()
cases=[]
def add(obj,marker=''):cases.append((dump(obj) if isinstance(obj,dict) else obj,marker))
add(base);add(base,'iss');add(base,'sub');add(base,'aud');add(base,'iat');add(base,'absent')
for k,values in {
 'iss':['','https://other.test','a'*2048,'a'*2049,1,None],
 'sub':['','x'*256,'x'*257,'a b',None],
 'nostr_pubkey':['0'*63,'A'*64,'f'*64,'f'*65,None],
 'aud':['',[],[''],['x'*2048],['x'*2049],['wss://relay.test','wss://other.test'],['x']*2,[str(i) for i in range(16)],[str(i) for i in range(17)],None,1],
 'iat':[0,MAX,MAX+1,-1,1.0,True,None],
 'exp':[0,MAX,MAX+1,-1,1.0,None],
 'nbf':[0,MAX,MAX+1,-1,1.0,None],
 'client_id':['mobile','x'*256,'x'*257,'',None],
 'nostr_owner_pubkey':['f'*64,'f'*63,None],
 'type':['human','',1,None,{'a':1}],
}.items():
 for v in values:
  o=dict(base);o[k]=v;add(o,'type' if k=='type' else '')
for k in base:
 o=dict(base);del o[k];add(o)
valid=dump(base);add(valid[:-1]+b',"\\u0069ss":"https://issuer.test"}');add(valid[:-1]+b',"x":1,"\\u0078":2}')
escaped=valid.replace(b'"iss"',b'"\\u0069ss"').replace(b'https://issuer.test',b'\\u0068ttps://issuer.test')
add(escaped,'iss');assert oracle(escaped,'iss')==oracle(valid,'iss')
zero=valid.replace(b'"iat":1',b'"iat":-0');add(zero)
add(valid.replace(b'"iat":1',b'"iat":01'))
add(valid.replace(b'"sub":"subject"',b'"sub":"\\ud800"'))
add(valid[:-1]+b',"extra":{"nested":[true,1,null]}}')
add(valid[:-1]+b',"extra":{"x":1,"\\u0078":2}}') # Ignored nested object; top-level names remain unique.
for count in (58,59):
 o=dict(base);o.update({f'extra{i}':i for i in range(count)});add(o)
for count in (128,129):
 o=dict(base);o['x'*count]=1;add(o)
add(valid+b' '*(65536-len(valid)));add(valid+b' '*(65537-len(valid)))
add(valid[:-1]+b',"type":"human"}','type');add(valid[:-1]+b',"type":"human","\\u0074ype":"human"}','type')
add(b'{"iss":1}');add(b'[]');add(b'{} true');add(valid[:-1]+b',"nbf":1e0}')
for i in range(40):
 o=dict(base);o['unused'+str(i)]={'x':i,'v':[None,True]};add(o)
rows=[]
for raw,marker in cases:
 expected=oracle(raw,marker);rows.append({'text':base64.b64encode(raw).decode(),'marker':base64.b64encode(marker.encode()).decode(),'expect':None if expected is None else base64.b64encode(expected).decode()})
assert oracle(valid,'iss') is not None and oracle(valid,'iat') is None
with tempfile.TemporaryDirectory(prefix='fi-claims-') as td:
 d=Path(td);vectors=d/'vectors.json';vectors.write_text(json.dumps(rows))
 for mode in ('checked','release'):
  generated=d/'generated.c';lib=d/(mode+'.dylib');wasm=d/(mode+'.wasm')
  cmd=[str(COMPILER),'c',str(SOURCE),'--library','-o',str(generated)]
  if mode=='release':cmd.append('--release')
  subprocess.run(cmd,check=True)
  subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC','-I'+str(d),str(FIXTURE),'-lm','-o',str(lib)],check=True)
  f=c.CDLL(str(lib)).fi_test_claims;ptr=c.POINTER(c.c_uint8);f.argtypes=[c.c_char_p,c.c_int64,c.c_char_p,c.c_int64,ptr,c.c_int64];f.restype=c.c_int
  for i,((raw,marker),row) in enumerate(zip(cases,rows)):
   out=(c.c_uint8*40000)();n=f(raw,len(raw),marker.encode(),len(marker),out,len(out));expected=oracle(raw,marker)
   assert (n==-1 if expected is None else n==len(expected) and bytes(out[:n])==expected),(mode,i,raw,marker,n)
  # Exercise all independent rows under native ASan/UBSan as well.
  driver=d/'sanitized.c';lines=['#include "'+str(FIXTURE)+'"','#include <assert.h>','int main(void){uint8_t out[40000];']
  for i,((raw,marker),row) in enumerate(zip(cases,rows)):
   expected=oracle(raw,marker);rb=','.join(map(str,raw)) or '0';mb=','.join(map(str,marker.encode())) or '0'
   lines.append(f'const char t{i}[]={{ {rb} }},m{i}[]={{ {mb} }};')
   if expected is None:lines.append(f'assert(fi_test_claims(t{i},{len(raw)},m{i},{len(marker)},out,sizeof(out))==-1);')
   else:
    eb=','.join(map(str,expected)) or '0';lines.append(f'const uint8_t e{i}[]={{ {eb} }};')
    lines.append(f'assert(fi_test_claims(t{i},{len(raw)},m{i},{len(marker)},out,sizeof(out))=={len(expected)} && memcmp(out,e{i},{len(expected)})==0);')
  lines.append('return 0;}');driver.write_text('\n'.join(lines))
  exe=d/(mode+'-sanitized')
  subprocess.run(['cc','-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer','-I'+str(d),str(driver),'-lm','-o',str(exe)],check=True)
  subprocess.run([str(exe)],check=True,timeout=60)
  subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-DFI_CLAIMS_WASM','-I'+str(d),str(FIXTURE),'-mexec-model=reactor','-Wl,-z,stack-size=1048576,--import-memory,--export-memory,--max-memory=67108864,--export=__heap_base','-lm','-o',str(wasm)],check=True)
  subprocess.run(['node',str(ROOT/'coax/tests/nostr/fi_claims_wasm.cjs'),str(wasm),str(vectors)],check=True)
  print(f'{mode}: {len(rows)} native and Wasm claims cases passed',flush=True)
