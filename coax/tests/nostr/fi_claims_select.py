#!/usr/bin/env python3
"""Independent issuer/marker selection cases, not FI authorization."""
import base64
import ctypes as c
import json
import os
from pathlib import Path
import subprocess
import tempfile
ROOT=Path(__file__).resolve().parents[3]
SOURCE=ROOT/'coax/lib/nostr/block/fi-claims-select.coax'
FIXTURE=ROOT/'coax/tests/nostr/fi_claims_select_fixture.c'
COMPILER=Path(os.environ.get('COAX_COMPILER',ROOT/'coax/bin/coax'))
base={'iss':'https://issuer.test','sub':'subject','aud':'wss://relay.test','nostr_pubkey':'0'*64,'iat':1,'exp':2,'client_id':'mobile'}
dump=lambda x:json.dumps(x,separators=(',',':')).encode()
b=dump(base);esc=b.replace(b'"iss"',b'"\\u0069ss"')
cases=[(b,'relay.test','https://issuer.test','iss',1,'https://issuer.test'),
 (esc,'relay.test','https://issuer.test','iss',1,'https://issuer.test'),
 (b,'relay.test','https://issuer.test','sub',1,'subject'),
 (b,'relay.test','https://issuer.test','client_id',1,'mobile'),
 (b,'relay.test','https://issuer.test','aud',1,'wss://relay.test'),
 (b,'relay.test','https://issuer.test','absent',0,''),
 (b,'relay.test','https://issuer.test','iat',4,''),
 (b,'other.test','https://issuer.test','iss',2,''),
 (b,'relay.test','https://other.test','iss',4,''),
 (b[:-1]+b',"\\u0069ss":"https://issuer.test"}','relay.test','https://issuer.test','iss',4,''),
 (b'{}','relay.test','https://issuer.test','iss',4,''),
 (b'{}','other.test','https://issuer.test','iss',2,''),
 (b'{"iss":"https://issuer.test","sub":null}','relay.test','https://issuer.test','invalid_config',2,''),
 (dump(dict(base,aud=['wss://relay.test'])),'relay.test','https://issuer.test','aud',4,'')]
rows=[dict(text=base64.b64encode(t).decode(),host=base64.b64encode(h.encode()).decode(),allow=base64.b64encode(a.encode()).decode(),marker=base64.b64encode(m.encode()).decode(),want=w,output=o) for t,h,a,m,w,o in cases]
with tempfile.TemporaryDirectory(prefix='fi-claims-select-') as td:
 d=Path(td);(d/'vectors.json').write_text(json.dumps(rows))
 for mode in ('checked','release'):
  gen=d/'generated.c';lib=d/(mode+'.dylib');wasm=d/(mode+'.wasm');cmd=[str(COMPILER),'c',str(SOURCE),'--library','-o',str(gen)]
  if mode=='release':cmd.append('--release')
  subprocess.run(cmd,check=True);subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC','-I'+str(d),str(FIXTURE),'-lm','-o',str(lib)],check=True)
  f=c.CDLL(str(lib)).fi_test_select;ptr=c.POINTER(c.c_uint8);f.argtypes=[c.c_char_p,c.c_int64,c.c_char_p,c.c_int64,c.c_char_p,c.c_int64,c.c_char_p,c.c_int64,ptr];f.restype=c.c_int
  for i,(t,h,a,m,w,o) in enumerate(cases):
   out=(c.c_uint8*129)();hb=h.encode();ab=a.encode();mb=m.encode();result=f(t,len(t),hb,len(hb),ab,len(ab),mb,len(mb),out)
   assert result==w and bytes(out).split(b'\0')[0].decode()==o,(mode,i,result)
  driver=d/'sanitized.c';lines=['#include "'+str(FIXTURE)+'"','#include <assert.h>','int main(void){uint8_t out[129];']
  for i,(t,h,a,m,w,o) in enumerate(cases):
   def data(v):return ','.join(map(str,v)) or '0'
   hb=h.encode();ab=a.encode();mb=m.encode();expected=o.encode()
   lines.append(f'const char t{i}[]={{ {data(t)} }},h{i}[]={{ {data(hb)} }},a{i}[]={{ {data(ab)} }},m{i}[]={{ {data(mb)} }};')
   lines.append(f'memset(out,0,sizeof(out));assert(fi_test_select(t{i},{len(t)},h{i},{len(hb)},a{i},{len(ab)},m{i},{len(mb)},out)=={w});')
   if expected:
    lines.append(f'const uint8_t e{i}[]={{ {data(expected)} }};assert(memcmp(out,e{i},{len(expected)})==0 && out[{len(expected)}]==0);')
   else:lines.append('assert(out[0]==0);')
  lines.append('return 0;}');driver.write_text('\n'.join(lines));exe=d/(mode+'-sanitized')
  subprocess.run(['cc','-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer','-I'+str(d),str(driver),'-lm','-o',str(exe)],check=True)
  subprocess.run([str(exe)],check=True,timeout=60)
  # Unreadable issuer registry verifies unknown Host, malformed input, and
  # community-unauthorized issuer are handled before issuer-specific reads.
  wrong=dump(dict(base,iss='https://other.test'));nums=','.join(map(str,wrong));guard=d/'guard.c'
  guard.write_text('#include "generated.c"\n#include <assert.h>\n#include <sys/mman.h>\nint main(void){const char bad[]={'+nums+'};coax_string names[1]={{"https://issuer.test",19}};coax_t_FiCommunity c={0};c.host=(coax_string){"relay.test",10};c.audience=(coax_string){"wss://relay.test",16};c.issuers=(coax_v_string){names,1};void *p=mmap(0,4096,PROT_NONE,MAP_PRIVATE|MAP_ANON,-1,0);assert(p!=MAP_FAILED);coax_v_t_FiIssuer issuers={(coax_t_FiIssuer*)p,1};assert(fi_claims_select((coax_v_t_FiCommunity){&c,1},issuers,(coax_string){"other.test",10},(coax_string){"{}",2}).status==1);assert(fi_claims_select((coax_v_t_FiCommunity){&c,1},issuers,(coax_string){"relay.test",10},(coax_string){"{}",2}).status==2);assert(fi_claims_select((coax_v_t_FiCommunity){&c,1},issuers,(coax_string){"relay.test",10},(coax_string){bad,sizeof(bad)}).status==2);c.host=(coax_string){"bad_",4};assert(fi_claims_select((coax_v_t_FiCommunity){&c,1},issuers,(coax_string){"bad_",4},(coax_string){"{}",2}).status==1);munmap(p,4096);return 0;}')
  guard_exe=d/(mode+'-guard');subprocess.run(['cc','-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer','-I'+str(d),str(guard),'-lm','-o',str(guard_exe)],check=True);subprocess.run([str(guard_exe)],check=True,timeout=60)
  subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-DFI_CLAIMS_WASM','-I'+str(d),str(FIXTURE),'-mexec-model=reactor','-Wl,-z,stack-size=2097152,--import-memory,--export-memory,--max-memory=67108864,--export=__heap_base','-lm','-o',str(wasm)],check=True)
  subprocess.run(['node',str(ROOT/'coax/tests/nostr/fi_claims_select_wasm.cjs'),str(wasm),str(d/'vectors.json')],check=True)
  print(f'{mode}: {len(cases)} native and Wasm issuer/marker cases passed')
