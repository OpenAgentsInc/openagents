#!/usr/bin/env python3
"""Independent canonical configuration byte oracle, not a full policy ID."""
import base64
import ctypes as c
import json
import os
from pathlib import Path
import struct
import subprocess
import tempfile
ROOT=Path(__file__).resolve().parents[3];SOURCE=ROOT/'coax/lib/nostr/block/fi-policy-encoding.coax';FIXTURE=ROOT/'coax/tests/nostr/fi_policy_encoding_fixture.c';COMPILER=Path(os.environ.get('COAX_COMPILER',ROOT/'coax/bin/coax'))
def reference(i):
 if i in (19,20):return None
 if i==21:
  issuer='https://issuer.test/'+'x'*(2048-20);aud='wss://relay.test/'+'x'*(2048-17);uri='https://keys.test/'+'x'*(2048-18)
  names=[issuer]+[chr(ord('A')+j)+'x'*2047 for j in range(1,16)];owners=[chr(ord('A')+j)+'y'*127 for j in range(8)];clients=[chr(ord('K')+j)+'z'*127 for j in range(8)]
  def text(s):b=s.encode();return struct.pack('>H',len(b))+b
  def items(a):return bytes([len(a)])+b''.join(text(x) for x in sorted(a))
  return text('openagents.fi-config-encoding.v1')+text('relay.test')+text(aud)+items(names)+text(issuer)+text(uri)+b''.join(struct.pack('>Q',v) for v in (1,2,10,0,10,1,30))+text('kind')+items(owners)+items(clients)+bytes([0,0])
 host='relay.test';aud='wss://relay.test';names=['https://issuer.test','https://other.test'];issuer=names[0];uri='https://keys.test';transport=1;klass=2;age=10;skew=1;lifetime=10;refresh=1;hard=30;marker='kind';owners=['human','person'];clients=['service','worker'];allow=0;noncollision=1
 if i==1:names.reverse();owners.reverse();clients.reverse()
 if i==2:host='other.test'
 if i==3:aud='wss://other.test'
 if i==4:names.append('https://third.test')
 if i==5:issuer='https://other.test'
 if i==6:uri='https://else.test'
 if i==7:transport=2;uri='http://keys.local'
 if i==8:klass=1;marker='';owners=[];clients=[];noncollision=0
 if i==9:age=11
 if i==10:skew=2
 if i==11:lifetime=11
 if i==12:refresh=2
 if i==13:hard=31
 if i==14:marker='type'
 if i==15:owners[1]='other'
 if i==16:clients[1]='daemon'
 if i==17:allow=1
 if i==18:noncollision=0
 def text(s):b=s.encode();return struct.pack('>H',len(b))+b
 def items(a):return bytes([len(a)])+b''.join(text(x) for x in sorted(a))
 return text('openagents.fi-config-encoding.v1')+text(host)+text(aud)+items(names)+text(issuer)+text(uri)+b''.join(struct.pack('>Q',v) for v in (transport,klass,age,skew,lifetime,refresh,hard))+text(marker)+items(owners)+items(clients)+bytes([allow,noncollision])
rows=[dict(id=i,expect=None if reference(i) is None else base64.b64encode(reference(i)).decode()) for i in range(22)]
assert reference(0)==reference(1) and all(reference(i)!=reference(0) for i in range(2,19))
with tempfile.TemporaryDirectory(prefix='fi-policy-encoding-') as td:
 d=Path(td);(d/'vectors.json').write_text(json.dumps(rows))
 for mode in ('checked','release'):
  gen=d/'generated.c';lib=d/(mode+'.dylib');wasm=d/(mode+'.wasm');cmd=[str(COMPILER),'c',str(SOURCE),'--library','-o',str(gen)]
  if mode=='release':cmd.append('--release')
  subprocess.run(cmd,check=True);subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC','-I'+str(d),str(FIXTURE),'-lm','-o',str(lib)],check=True)
  f=c.CDLL(str(lib)).fi_test_policy_encoding;f.argtypes=[c.c_int,c.POINTER(c.c_uint8),c.c_int64];f.restype=c.c_int
  for i in range(22):
   out=(c.c_uint8*65536)();n=f(i,out,len(out));want=reference(i);assert (n==-1 if want is None else n==len(want) and bytes(out[:n])==want),(mode,i,n,want)
  driver=d/'sanitized.c';lines=['#include "'+str(FIXTURE)+'"','#include <assert.h>','int main(void){uint8_t out[65536];']
  for i in range(22):
   want=reference(i)
   if want is None:lines.append(f'assert(fi_test_policy_encoding({i},out,sizeof(out))==-1);')
   else:
    lines.append(f'const uint8_t e{i}[]={{ '+','.join(map(str,want))+' };')
    lines.append(f'assert(fi_test_policy_encoding({i},out,sizeof(out))=={len(want)} && memcmp(out,e{i},{len(want)})==0);')
  lines.append('return 0;}');driver.write_text('\n'.join(lines));exe=d/(mode+'-sanitized')
  subprocess.run(['cc','-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer','-I'+str(d),str(driver),'-lm','-o',str(exe)],check=True);subprocess.run([str(exe)],check=True,timeout=60)
  subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-DFI_POLICY_WASM','-I'+str(d),str(FIXTURE),'-mexec-model=reactor','-Wl,-z,stack-size=1048576,--import-memory,--export-memory,--max-memory=67108864,--export=__heap_base','-lm','-o',str(wasm)],check=True)
  subprocess.run(['node',str(ROOT/'coax/tests/nostr/fi_policy_encoding_wasm.cjs'),str(wasm),str(d/'vectors.json')],check=True)
  print(f'{mode}: {len(rows)} native/Wasm/sanitizer encoding cases passed')
