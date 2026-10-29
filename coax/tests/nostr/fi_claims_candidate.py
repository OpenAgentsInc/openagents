#!/usr/bin/env python3
"""Typed FI semantic candidates; neither signature nor possession authority."""
import base64
import ctypes as c
import json
import os
from pathlib import Path
import subprocess
import tempfile
ROOT=Path(__file__).resolve().parents[3];SOURCE=ROOT/'coax/lib/nostr/block/fi-claims-candidate.coax';FIXTURE=ROOT/'coax/tests/nostr/fi_claims_candidate_fixture.c';COMPILER=Path(os.environ.get('COAX_COMPILER',ROOT/'coax/bin/coax'))
base={'iss':'https://issuer.test','sub':'subject','aud':'wss://relay.test','nostr_pubkey':'0'*64,'iat':1,'exp':20,'client_id':'mobile'}
dump=lambda x:json.dumps(x,separators=(',',':')).encode()
cases=[]
def add(o,host='relay.test',marker='iss',alg='RS256',typ='at+jwt',now=2,want=0):cases.append((dump(o) if isinstance(o,dict) else o,host,marker,alg,typ,now,want))
add(base);add(b'{}',host='other.test',want=1);add(base,host='other.test',want=1);add(base,alg='HS256',want=2);add(base,alg='none',want=2);add(base,typ='nip-fi+jwt',want=2);add(base,now=11,want=2);add(base,now=20,want=2);add(base,now=0,want=2);add(base,marker='absent',want=2);add(base,marker='iat',want=2)
for patch in [{'iss':'https://other.test'},{'sub':''},{'aud':'wss://other.test'},{'nostr_pubkey':'A'*64},{'client_id':''},{'nbf':3},{'nostr_owner_pubkey':'1'*64}]:
 add(dict(base,**patch),want=2)
add(dict(base,aud=['wss://other.test','wss://relay.test']));add(dump(base)[:-1]+b',"\\u0069ss":"https://issuer.test"}',want=2)
rows=[dict(text=base64.b64encode(t).decode(),host=base64.b64encode(h.encode()).decode(),marker=base64.b64encode(m.encode()).decode(),algorithm=base64.b64encode(a.encode()).decode(),typ=base64.b64encode(y.encode()).decode(),now=n,want=w) for t,h,m,a,y,n,w in cases]
with tempfile.TemporaryDirectory(prefix='fi-claims-candidate-') as td:
 d=Path(td);(d/'vectors.json').write_text(json.dumps(rows))
 for mode in ('checked','release'):
  gen=d/'generated.c';lib=d/(mode+'.dylib');wasm=d/(mode+'.wasm');cmd=[str(COMPILER),'c',str(SOURCE),'--library','-o',str(gen)]
  if mode=='release':cmd.append('--release')
  subprocess.run(cmd,check=True);subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC','-I'+str(d),str(FIXTURE),'-lm','-o',str(lib)],check=True)
  f=c.CDLL(str(lib)).fi_test_candidate;f.argtypes=[c.c_char_p,c.c_int64]*5+[c.c_int64];f.restype=c.c_int
  for i,(t,h,m,a,y,n,w) in enumerate(cases):
   args=[]
   for v in (t,h.encode(),m.encode(),a.encode(),y.encode()):args += [v,len(v)]
   got=f(*args,n);assert got==w,(mode,i,got,w)
  driver=d/'sanitized.c';lines=['#include "'+str(FIXTURE)+'"','#include <assert.h>','int main(void){']
  for i,(t,h,m,a,y,n,w) in enumerate(cases):
   def data(v):return ','.join(map(str,v)) or '0'
   fields=[t,h.encode(),m.encode(),a.encode(),y.encode()]
   for j,v in enumerate(fields):lines.append(f'const char v{i}_{j}[]={{ {data(v)} }};')
   args=','.join(f'v{i}_{j},{len(v)}' for j,v in enumerate(fields));lines.append(f'assert(fi_test_candidate({args},{n})=={w});')
  lines.append('return 0;}');driver.write_text('\n'.join(lines));exe=d/(mode+'-sanitized')
  subprocess.run(['cc','-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer','-I'+str(d),str(driver),'-lm','-o',str(exe)],check=True)
  subprocess.run([str(exe)],check=True,timeout=60)
  subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-DFI_CLAIMS_WASM','-I'+str(d),str(FIXTURE),'-mexec-model=reactor','-Wl,-z,stack-size=2097152,--import-memory,--export-memory,--max-memory=67108864,--export=__heap_base','-lm','-o',str(wasm)],check=True)
  subprocess.run(['node',str(ROOT/'coax/tests/nostr/fi_claims_candidate_wasm.cjs'),str(wasm),str(d/'vectors.json')],check=True)
  print(f'{mode}: {len(cases)} native and Wasm semantic candidate cases passed')
