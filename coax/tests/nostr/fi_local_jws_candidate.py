#!/usr/bin/env python3
"""Original JWS, semantic candidate and caller-supplied JWKS; never FI authority."""
import base64
import ctypes as c
import json
import os
from pathlib import Path
import subprocess
import tempfile
ROOT=Path(__file__).resolve().parents[3];SOURCE=ROOT/'coax/lib/nostr/block/fi-local-jws-candidate.coax';FIXTURE=ROOT/'coax/tests/nostr/fi_local_jws_fixture.c';COMPILER=Path(os.environ.get('COAX_COMPILER',ROOT/'coax/bin/coax'))
enc=lambda b:base64.urlsafe_b64encode(b).rstrip(b'=')
dump=lambda v:json.dumps(v,separators=(',',':')).encode()
with tempfile.TemporaryDirectory(prefix='fi-local-jws-') as td:
 d=Path(td);priv=d/'private.pem';pub=d/'public.pem';sig=d/'sig';message=d/'msg'
 subprocess.run(['openssl','genpkey','-algorithm','RSA','-pkeyopt','rsa_keygen_bits:2048','-out',str(priv)],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
 subprocess.run(['openssl','pkey','-in',str(priv),'-pubout','-out',str(pub)],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
 raw=subprocess.run(['openssl','rsa','-in',str(priv),'-modulus','-noout'],check=True,capture_output=True,text=True).stdout.strip();modulus=bytes.fromhex(raw.split('=',1)[1]);assert len(modulus)==256
 key={'kid':'test-key','kty':'RSA','alg':'RS256','use':'sig','n':enc(modulus).decode(),'e':'AQAB'};jwks=dump({'keys':[key]})
 header={'alg':'RS256','typ':'at+jwt','kid':'test-key'};payload={'iss':'https://issuer.test','sub':'subject','aud':'wss://relay.test','nostr_pubkey':'0'*64,'iat':100,'exp':200,'client_id':'mobile'}
 def sign(h,p):
  text=enc(dump(h))+b'.'+enc(dump(p) if isinstance(p,dict) else p);message.write_bytes(text)
  subprocess.run(['openssl','dgst','-sha256','-sign',str(priv),'-out',str(sig),str(message)],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
  subprocess.run(['openssl','dgst','-sha256','-verify',str(pub),'-signature',str(sig),str(message)],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
  return text+b'.'+enc(sig.read_bytes())
 good=sign(header,payload);parts=good.split(b'.');cases=[]
 def add(t=good,j=jwks,h='relay.test',now=101,w=0,klass=2):cases.append((t,j,h,now,w,klass))
 add(w=1);add(h='other.test');add(now=110);add(now=200);add(now=99)
 for patch in [{'iss':'https://other.test'},{'sub':''},{'aud':'wss://other.test'},{'nostr_pubkey':'A'*64},{'client_id':''},{'nbf':102},{'nostr_owner_pubkey':'1'*64}]:add(t=sign(header,dict(payload,**patch)))
 add(t=sign(header,dict(payload,aud=['wss://other.test','wss://relay.test'])),w=1)
 add(t=sign(dict(header,typ='nip-fi+jwt'),dict(payload,client_id={'ignored':True})),w=1,klass=1)
 agent_header=dict(header,typ='nip-fi-agent+jwt')
 add(t=sign(agent_header,dict(payload,nostr_owner_pubkey='1'*64)),w=1,klass=1)
 add(t=sign(agent_header,dict(payload,nostr_owner_pubkey='0'*64)),w=0,klass=1)
 add(t=sign(agent_header,payload),w=0,klass=1)
 add(t=sign(dict(header,typ='nip-fi+jwt'),dict(payload,nostr_owner_pubkey='1'*64)),w=0,klass=1)
 add(t=sign(header,dict(payload,client_id={'ignored':True})),w=0,klass=2)
 for patch in [{'alg':'HS256'},{'typ':'nip-fi+jwt'},{'kid':'unknown'},{'jku':'https://evil.test'}]:add(t=sign(dict(header,**patch),payload))
 add(t=sign(header,dump(payload)[:-1]+b',"\\u0069ss":"https://issuer.test"}'))
 add(t=parts[0]+b'.'+enc(dump(dict(payload,sub='other')))+b'.'+parts[2])
 spaced=json.dumps(payload).encode();assert json.loads(spaced)==payload and spaced!=dump(payload)
 add(t=parts[0]+b'.'+enc(spaced)+b'.'+parts[2]);add(t=sign(header,spaced),w=1)
 signature=base64.urlsafe_b64decode(parts[2]+b'='*(-len(parts[2])%4));add(t=parts[0]+b'.'+parts[1]+b'.'+enc(bytes([signature[0]^1])+signature[1:]))
 add(j=dump({'keys':[dict(key,kid='other')]}));add(j=dump({'keys':[key,key]}));add(j=dump({'keys':[dict(key,d='AQ')]}));add(j=b'{}');add(t=b'')
 rows=[dict(token=base64.b64encode(t).decode(),jwks=base64.b64encode(j).decode(),host=base64.b64encode(h.encode()).decode(),now=n,want=w,klass=k) for t,j,h,n,w,k in cases];(d/'vectors.json').write_text(json.dumps(rows))
 for mode in ('checked','release'):
  gen=d/'generated.c';lib=d/(mode+'.dylib');wasm=d/(mode+'.wasm');cmd=[str(COMPILER),'c',str(SOURCE),'--library','-o',str(gen)]
  if mode=='release':cmd.append('--release')
  subprocess.run(cmd,check=True);subprocess.run(['cc','-std=c11','-O2','-shared','-fPIC','-I'+str(d),str(FIXTURE),'-lm','-o',str(lib)],check=True)
  f=c.CDLL(str(lib)).fi_test_local_class;ptr=c.POINTER(c.c_uint8);f.argtypes=[ptr,c.c_int64,c.c_char_p,c.c_int64,c.c_char_p,c.c_int64,c.c_int64,c.c_int64];f.restype=c.c_int
  for i,(t,j,h,n,w,k) in enumerate(cases):
   tb=(c.c_uint8*max(1,len(t)))();tb[:len(t)]=t;hb=h.encode();got=f(tb,len(t),j,len(j),hb,len(hb),n,k);assert got==w,(mode,i,got,w)
  driver=d/'sanitized.c';lines=['#include "'+str(FIXTURE)+'"','#include <assert.h>','int main(void){']
  for i,(t,j,h,n,w,k) in enumerate(cases):
   def data(v):return ','.join(map(str,v)) or '0'
   hb=h.encode();lines.append(f'const uint8_t t{i}[]={{ {data(t)} }};const char j{i}[]={{ {data(j)} }},h{i}[]={{ {data(hb)} }};')
   lines.append(f'assert(fi_test_local_class(t{i},{len(t)},j{i},{len(j)},h{i},{len(hb)},{n},{k})=={w});')
  lines.append('return 0;}');driver.write_text('\n'.join(lines));exe=d/(mode+'-sanitized')
  subprocess.run(['cc','-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer','-I'+str(d),str(driver),'-lm','-o',str(exe)],check=True);subprocess.run([str(exe)],check=True,timeout=60)
  # An unreadable but allocated JWKS region confirms semantic refusals do not touch it.
  guard=d/'guard.c';tb=','.join(map(str,good));wrong_issuer=','.join(map(str,cases[5][0]))
  guard.write_text('#include "'+str(FIXTURE)+'"\n#include <assert.h>\n#include <sys/mman.h>\nint main(void){const uint8_t t[]={'+tb+'},w[]={'+wrong_issuer+'};void *p=mmap(0,4096,PROT_NONE,MAP_PRIVATE|MAP_ANON,-1,0);assert(p!=MAP_FAILED);assert(!fi_test_local(t,sizeof(t),p,4096,"other.test",10,101));assert(!fi_test_local(t,sizeof(t),p,4096,"relay.test",10,110));assert(!fi_test_local(w,sizeof(w),p,4096,"relay.test",10,101));munmap(p,4096);return 0;}')
  guard_exe=d/(mode+'-guard')
  subprocess.run(['cc','-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer','-I'+str(d),str(guard),'-lm','-o',str(guard_exe)],check=True);subprocess.run([str(guard_exe)],check=True,timeout=60)
  subprocess.run(['/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-DFI_LOCAL_WASM','-I'+str(d),str(FIXTURE),'-mexec-model=reactor','-Wl,-z,stack-size=4194304,--import-memory,--export-memory,--max-memory=67108864,--export=__heap_base','-lm','-o',str(wasm)],check=True)
  subprocess.run(['node',str(ROOT/'coax/tests/nostr/fi_local_jws_wasm.cjs'),str(wasm),str(d/'vectors.json')],check=True)
  print(f'{mode}: {len(cases)} local signed candidate cases passed native, Wasm and sanitizer')
