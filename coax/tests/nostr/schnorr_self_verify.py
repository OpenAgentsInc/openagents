#!/usr/bin/env python3
"""Fixed-work signer self-verification (#89db): checked and release, native and Wasm.

Explicit test tooling only. Every case is decided three ways: the Coax
fixed-work ssv_verify, the existing Coax variable-time schnorr_verify_vartime
(oracle on public data, tests only) and the independent Python reference in
tools/coax-nostr/fuzz/oracle.py. The candidates are public test fixtures.
"""
import ctypes as C
import json
import os
import platform
import random
import sys
import tempfile
from pathlib import Path
import secp_field as base
import schnorr_support as support
from schnorr import official
from oracle import N,P,G,lift_x,point_mul,point_add,tagged,schnorr_verify

ROOT=support.ROOT
SOURCE=ROOT/'coax/lib/crypto/schnorr_self_verify.coax'
DRIVER=ROOT/'coax/tests/nostr/schnorr_self_verify_driver.c'
RUNNER=ROOT/'coax/tests/nostr/schnorr_self_verify_wasm.cjs'
GCC=os.environ.get('SSV_CC','gcc')

def xonly(d):
 point=point_mul(d);return point,point[0].to_bytes(32,'big')
def challenge(r,key,message):return int.from_bytes(tagged('BIP0340/challenge',r+key+message),'big')%N
def sign(message,secret,k,odd_nonce=False):
 """BIP-340 with an explicit nonce; odd_nonce skips the R parity fix."""
 point,key=xonly(secret);d=secret if point[1]%2==0 else N-secret
 R=point_mul(k)
 if not odd_nonce and R[1]%2:k=N-k
 r=R[0].to_bytes(32,'big');e=challenge(r,key,message)
 return key,r+((k+e*d)%N).to_bytes(32,'big')

def cases():
 rows=[]
 def add(name,message,key,sig,nostr_too=True):
  rows.append({'name':name,'message':message.hex(),'public':key.hex(),'signature':sig.hex(),'expected':schnorr_verify(message,key,sig)})
 for row in official():
  add('official-'+str(row['index']),row['message'],row['public'],row['signature'])
  if rows[-1]['expected']!=row['expected']:raise AssertionError('reference disagrees with official vector')
 rng=random.Random(890260)
 for i in range(24):
  message=rng.randbytes(rng.choice([0,1,17,32,32,100,255]));secret=rng.randrange(1,N);k=rng.randrange(1,N)
  key,sig=sign(message,secret,k);add('valid-'+str(i),message,key,sig)
  for part,lo,hi in [('r',0,32),('s',32,64)]:
   bad=bytearray(sig);bad[rng.randrange(lo,hi)]^=1<<rng.randrange(8);add('tamper-'+part+'-'+str(i),message,key,bytes(bad))
  bad=bytearray(key);bad[rng.randrange(32)]^=1<<rng.randrange(8);add('tamper-key-'+str(i),message,bytes(bad),sig)
  bad=bytearray(message) if message else bytearray(b'\1');bad[0]^=1;add('tamper-message-'+str(i),bytes(bad),key,sig)
  other=rng.randbytes(32);add('mismatched-message-'+str(i),other,key,sig)
  s=int.from_bytes(sig[32:],'big')
  add('s-plus-n-'+str(i),message,key,sig[:32]+((s+N).to_bytes(32,'big') if s+N<2**256 else N.to_bytes(32,'big')))
  add('s-equals-n-'+str(i),message,key,sig[:32]+N.to_bytes(32,'big'))
  add('r-equals-p-'+str(i),message,key,P.to_bytes(32,'big')+sig[32:])
  add('r-all-ones-'+str(i),message,key,b'\xff'*32+sig[32:])
  # R' at infinity: pick any R bytes, then s = e*d makes s*G - e*P = O.
  point,key2=xonly(secret);d=secret if point[1]%2==0 else N-secret
  r=rng.randrange(1,P).to_bytes(32,'big');e=challenge(r,key2,message)
  add('infinity-'+str(i),message,key2,r+(e*d%N).to_bytes(32,'big'))
  # Odd y(R'): the right x with the wrong parity must refuse.
  while True:
   k=rng.randrange(1,N)
   if point_mul(k)[1]%2:break
  add('odd-r-'+str(i),message,*sign(message,secret,k,odd_nonce=True)[::1])
  add('zero-s-'+str(i),message,key,sig[:32]+bytes(32))
  # Invalid key: an x with no curve point, and x >= p.
  while True:
   x=rng.randrange(P)
   if lift_x(x) is None:break
  add('key-off-curve-'+str(i),message,x.to_bytes(32,'big'),sig)
  add('key-at-p-'+str(i),message,P.to_bytes(32,'big'),sig)
 # Reused-workspace and readonly alias: message is the key itself.
 secret=3;key,sig=sign(xonly(secret)[1],secret,7);add('readonly-alias',key,key,sig)
 big=bytes(range(256))*4096;key,sig=sign(big,secret,11);add('maximum-message',big,key,sig)
 for row in rows:
  if row['name'].startswith(('valid-','readonly','maximum')) and not row['expected']:raise AssertionError('fixture not valid:'+row['name'])
  if row['name'].startswith(('infinity-','odd-r-','s-','r-','zero-s-','key-','tamper-','mismatched-')) and row['expected']:raise AssertionError('fixture unexpectedly valid:'+row['name'])
 return rows

def wasm_env():
 cache=Path.home()/'.cache/coax-wasm-toolchain'
 return [os.environ.get('COAXIAL_CLANG',str(cache/'bin/clang')),'--sysroot='+os.environ.get('COAXIAL_SYSROOT',str(cache/'sysroot')),'-resource-dir='+os.environ.get('COAXIAL_RESOURCES',str(cache/'resource'))]

def main():
 rows=cases();report=[]
 with tempfile.TemporaryDirectory(prefix='coax-ssv-') as td:
  d=Path(td);casefile=d/'cases.json';casefile.write_text(json.dumps(rows))
  for label,mode in [('checked',[]),('release',['--release'])]:
   c=d/(label+'.c');h=d/'schnorr-self-verify.h';mapping=d/(label+'-map.json')
   c.write_text(base.run(base.COMPILER,'c',SOURCE,'--library','--header',h,'--source-map',mapping,'--run-steps','250000000',*mode).stdout)
   hashes=json.loads(mapping.read_text())['source_hashes']
   lib_path=d/(label+'.so');base.run(GCC,'-std=c11','-O2','-shared','-fPIC','-I'+str(d),c,DRIVER,'-lm','-o',lib_path);lib=C.CDLL(str(lib_path))
   lib.ssv_case.argtypes=[C.c_char_p,C.c_int64,C.c_char_p,C.c_int64,C.c_char_p,C.c_int64,C.c_int,C.c_int,C.c_int64,C.c_int];lib.ssv_case.restype=C.c_int
   lib.old_verify.argtypes=[C.c_char_p,C.c_int64,C.c_char_p,C.c_int64,C.c_char_p,C.c_int64];lib.old_verify.restype=C.c_int
   for row in rows:
    m,p,s=[bytes.fromhex(row[k]) for k in ('message','public','signature')]
    for nostr in (0,1):
     want=row['expected'] and (not nostr or len(m)==32);refused_view=nostr and len(m)!=32
     got=lib.ssv_case(m,len(m),p,len(p),s,len(s),nostr,0,1,0)
     if got<0 or bool(got&1)!=want:raise AssertionError((label,row['name'],nostr,got,want))
     if refused_view and not got&4:raise AssertionError(('refused view touched work',label,row['name']))
     if not refused_view and not got&2:raise AssertionError(('work not cleared',label,row['name']))
    if bool(lib.old_verify(m,len(m),p,len(p),s,len(s)))!=row['expected']:raise AssertionError(('oracle disagreement',label,row['name']))
   good=official()[0];m,p,s=good['message'],good['public'],good['signature']
   for counts in [(-1,32,64,1),(1048577,32,64,1),(2**63-1,32,64,1),(32,-1,64,1),(32,31,64,1),(32,33,64,1),(32,32,63,1),(32,32,65,1),(32,32,64,0),(32,32,64,2)]:
    got=lib.ssv_case(m,counts[0],p,counts[1],s,counts[2],0,0,counts[3],0)
    if got!=4:raise AssertionError(('metadata',label,counts,got))
   if lib.ssv_case(m,32,p,32,s,64,0,1,1,0)!=4:raise AssertionError(('overlap',label))
   if lib.ssv_case(m,32,p,32,s,64,0,0,1,1)!=2 or lib.ssv_case(m,32,p,32,s,64,1,0,1,1)!=2:raise AssertionError(('faulted stage accepted or not cleared',label))
   wasm=d/(label+'.wasm')
   base.run(*wasm_env(),'--target=wasm32-wasip1','-std=c11','-O2','-I'+str(d),c,DRIVER,'-mexec-model=reactor','-Wl,--import-memory,--export-memory,--max-memory=67108864,--export=__heap_base','-lm','-o',wasm)
   observed=json.loads(base.run('node',RUNNER,wasm,casefile).stdout)
   if any(support.sha(ROOT/path)!=digest for path,digest in hashes.items()):raise AssertionError('source closure changed during test')
   report.append({'mode':label,'native_cases':len(rows),'native_entries':2,'wasm':observed,'source_sha256':hashes,'generated_c_sha256':support.sha(c),'native_sha256':support.sha(lib_path),'wasm_sha256':support.sha(wasm)})
 print(json.dumps({'official':len(official()),'cases':len(rows),'valid':sum(r['expected'] for r in rows),'compiler':base.run(base.COMPILER,'version').stdout.strip(),'compiler_sha256':support.sha(base.COMPILER),'native_cc':base.run(GCC,'--version').stdout.splitlines()[0],'wasm_cc':base.run(wasm_env()[0],'--version').stdout.splitlines()[0],'node':base.run('node','--version').stdout.strip(),'target':platform.machine()+' '+platform.system()+' '+platform.release(),'driver_sha256':support.sha(DRIVER),'runner_sha256':support.sha(RUNNER),'harness_sha256':support.sha(__file__),'modes':report},indent=2))
if __name__=='__main__':main()
