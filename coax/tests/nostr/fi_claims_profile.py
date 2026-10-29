#!/usr/bin/env python3
"""Issuer-only and selected-class claim projection; no authorization."""
import base64
import ctypes as c
import json
import os
import random
from pathlib import Path
import subprocess
import tempfile
ROOT=Path(__file__).resolve().parents[3];SOURCE=ROOT/'coax/lib/nostr/block/fi-claims-parse.coax';FIXTURE=ROOT/'coax/tests/nostr/fi_claims_profile_fixture.c';COMPILER=Path(os.environ.get('COAX_COMPILER',ROOT/'coax/bin/coax'))
base={'iss':'https://issuer.test','sub':'subject','aud':'wss://relay.test','nostr_pubkey':'0'*64,'iat':1,'exp':20,'client_id':'mobile'};dump=lambda x:json.dumps(x,separators=(',',':')).encode()
cases=[(b'{"iss":"https://issuer.test","sub":null,"client_id":{}}','',0,1),
 (b'{"iss":"https://issuer.test","\\u0069ss":"https://issuer.test"}','',0,0),
 (b'{}','',0,0),(dump(base),'x',0,0),(dump(base),'',3,0),(dump(base),'',-1,0),
 (dump(base),'',1,1),(dump(dict(base,client_id={'ignored':True})),'',1,1),
 (dump(dict(base,client_id=None)),'',1,1),(dump(dict(base,client_id='x'*300)),'',1,1),
 (dump(base),'iss',2,7),(dump(dict(base,client_id={'ignored':True})),'iss',2,0),
 (dump(dict(base,client_id=None)),'iss',2,0),(dump(base),'client_id',2,7),
 (dump(base),'nonexistent',2,3),(dump(base),'iat',2,0),
 (dump(dict(base,sub=None)),'',1,0),(dump(dict(base,sub=None)),'',0,1)]
# Independently derive deterministic mixed-field cases from the strict oracle.
oracle_path=ROOT/'coax/tests/nostr/fi_claims_parse.py';oracle_source=oracle_path.read_text().split("with tempfile.TemporaryDirectory(prefix='fi-claims-')")[0]
namespace={'__file__':str(oracle_path)};exec(compile(oracle_source,str(oracle_path),'exec'),namespace)
ref=namespace['oracle'];visible=namespace['visible'];rng=random.Random(42601)
choices={'iss':['https://issuer.test','',None], 'sub':['subject','x','',None],
 'aud':['wss://relay.test',['wss://relay.test'],[],['x','x']],
 'nostr_pubkey':['0'*64,'A'*64,'x',None], 'iat':[1,0,-1,1.0], 'exp':[20,0,-1,None],
 'client_id':['mobile','x','',None,{'unverified':True}], 'type':['human','service',None]}
for _ in range(60):
 obj=dict(base)
 for k in rng.sample(list(choices),rng.randint(1,4)):obj[k]=rng.choice(choices[k])
 text=dump(obj)
 for n in (0,1,2):
  marker='' if n!=2 else rng.choice(['iss','client_id','aud','iat','type','missing'])
  if n==0:ok=visible(obj.get('iss'),2048)
  elif n==1:
   dedicated=dict(obj);dedicated.pop('client_id',None);ok=ref(dump(dedicated),'') is not None
  else:ok=ref(text,marker) is not None
  expected=(1|(2 if n==2 and 'client_id' in obj else 0)|(4 if n==2 and marker in obj else 0)) if ok else 0
  cases.append((text,marker,n,expected))
rows=[dict(text=base64.b64encode(t).decode(),marker=base64.b64encode(m.encode()).decode(),mode=n,want=w) for t,m,n,w in cases]
with tempfile.TemporaryDirectory(prefix='fi-claims-profile-') as td:
 d=Path(td);(d/'vectors.json').write_text(json.dumps(rows))
 for mode in ('checked','release'):
  gen=d/'generated.c';lib=d/(mode+'.dylib');wasm=d/(mode+'.wasm');cmd=[str(COMPILER),'c',str(SOURCE),'--library','-o',str(gen)]
  if mode=='release':cmd.append('--release')
  subprocess.run(cmd,check=True);subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC','-I'+str(d),str(FIXTURE),'-lm','-o',str(lib)],check=True)
  f=c.CDLL(str(lib)).fi_test_profile;f.argtypes=[c.c_char_p,c.c_int64,c.c_char_p,c.c_int64,c.c_int64];f.restype=c.c_int
  for i,(t,m,n,w) in enumerate(cases):
   mb=m.encode();got=f(t,len(t),mb,len(mb),n);assert got==w,(mode,i,got,w)
  driver=d/'sanitized.c';lines=['#include "'+str(FIXTURE)+'"','#include <assert.h>','int main(void){']
  for i,(t,m,n,w) in enumerate(cases):
   def data(v):return ','.join(map(str,v)) or '0'
   mb=m.encode();lines.append(f'const char t{i}[]={{ {data(t)} }},m{i}[]={{ {data(mb)} }};assert(fi_test_profile(t{i},{len(t)},m{i},{len(mb)},{n})=={w});')
  lines.append('return 0;}');driver.write_text('\n'.join(lines));exe=d/(mode+'-sanitized')
  subprocess.run(['cc','-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer','-I'+str(d),str(driver),'-lm','-o',str(exe)],check=True);subprocess.run([str(exe)],check=True,timeout=60)
  subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-DFI_CLAIMS_WASM','-I'+str(d),str(FIXTURE),'-mexec-model=reactor','-Wl,-z,stack-size=1048576,--import-memory,--export-memory,--max-memory=67108864,--export=__heap_base','-lm','-o',str(wasm)],check=True)
  subprocess.run(['node',str(ROOT/'coax/tests/nostr/fi_claims_profile_wasm.cjs'),str(wasm),str(d/'vectors.json')],check=True)
  print(f'{mode}: {len(cases)} class-aware native, Wasm and sanitizer cases passed')
