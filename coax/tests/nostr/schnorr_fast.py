#!/usr/bin/env python3
"""Differential public BIP-340 verification; no secret timing claim."""
import ctypes as C
import hashlib,json,random,tempfile
from pathlib import Path
import schnorr_support as support
from schnorr import official
from oracle import NativeOracle,DEFAULT_LIBRARY,N,P,schnorr_verify

ROOT=support.ROOT
SOURCE=ROOT/'coax/lib/crypto/schnorr_fast.coax'
base=support.base

def checked(lib,name,message,key,sig):
 arrays=[base.array(x) for x in (message,key,sig)]
 result=bool(getattr(lib,name)(*[base.view(x) for x in arrays]))
 if any(bytes(a)!=v for a,v in zip(arrays,(message,key,sig))):raise AssertionError('input changed')
 return result

def main():
 oracle=NativeOracle(DEFAULT_LIBRARY);rows=[]
 def add(name,message,key,sig):
  expected=oracle.verify(message,key,sig)
  if schnorr_verify(message,key,sig)!=expected:raise AssertionError('independent oracle disagreement '+name)
  rows.append({'name':name,'message':message.hex(),'public':key.hex(),'signature':sig.hex(),'expected':expected,'nostr':False})
 for row in official():
  add('official-'+str(row['index']),row['message'],row['public'],row['signature'])
  if rows[-1]['expected']!=row['expected']:raise AssertionError('official expected mismatch')
 rng=random.Random(3402026)
 for i in range(128):
  message=rng.randbytes(rng.choice([0,1,17,32,100,255,1024]));secret=rng.randrange(1,N).to_bytes(32,'big');key=oracle.point(secret)[1:];sig=oracle.sign(message,secret,bytes(32))
  add('valid-'+str(i),message,key,sig)
  changed=bytearray(sig);changed[rng.randrange(64)]^=1<<rng.randrange(8);add('signature-'+str(i),message,key,bytes(changed))
  add('r-range-'+str(i),message,key,P.to_bytes(32,'big')+sig[32:]);add('s-range-'+str(i),message,key,sig[:32]+N.to_bytes(32,'big'))
  bad=bytearray(message) if message else bytearray(b'\0');bad[0]^=1;add('message-'+str(i),bytes(bad),key,sig)
  bad=bytearray(key);bad[rng.randrange(32)]^=1<<rng.randrange(8);add('key-'+str(i),message,bytes(bad),sig)
 secret=(3).to_bytes(32,'big');key=oracle.point(secret)[1:];alias_sig=oracle.sign(key,secret,bytes(32));add('readonly-alias',key,key,alias_sig)
 message=bytes(range(256))*4096;sig=oracle.sign(message,secret,bytes(32));add('maximum-message',message,key,sig);add('over-maximum',message+b'\0',key,sig)
 with tempfile.TemporaryDirectory(prefix='coax-schnorr-fast-') as td:
  d=Path(td);cases=d/'cases.json';cases.write_text(json.dumps(rows));report=[]
  for label,mode in [('checked',[]),('release',['--release'])]:
   c=d/(label+'.c');h=d/'schnorr-fast.h';mapping=d/(label+'-map.json');c.write_text(base.run(base.COMPILER,'c',SOURCE,'--library','--header',h,'--source-map',mapping,*mode).stdout)
   hashes=json.loads(mapping.read_text())['source_hashes']
   library=d/(label+'.dylib');base.run('cc','-std=c11','-O2','-shared','-fPIC',c,'-lm','-o',library);lib=C.CDLL(str(library))
   for name in ['schnorr_fast_verify_vartime','nostr_fast_verify_vartime','schnorr_verify_vartime']:
    getattr(lib,name).argtypes=[base.View,base.View,base.View];getattr(lib,name).restype=C.c_bool
   for row in rows:
    args=[bytes.fromhex(row[k]) for k in ('message','public','signature')]
    fast=checked(lib,'schnorr_fast_verify_vartime',*args);old=checked(lib,'schnorr_verify_vartime',*args)
    nostr=checked(lib,'nostr_fast_verify_vartime',*args)
    if fast!=row['expected'] or old!=row['expected'] or nostr!=(row['expected'] and len(args[0])==32):raise AssertionError((label,row['name'],fast,old,nostr,row['expected']))
   good=official()[0];arrays=[base.array(good[k]) for k in ('message','public','signature')]
   for counts in [(-1,32,64),(1048577,32,64),(2**63-1,32,64),(32,-1,64),(32,31,64),(32,33,64),(32,32,63),(32,32,65)]:
    views=[base.view(a) for a in arrays]
    for view,count in zip(views,counts):view.count=count
    if lib.schnorr_fast_verify_vartime(*views) or lib.nostr_fast_verify_vartime(*views):raise AssertionError(('metadata accepted',label,counts))
   same=base.array(key);alias=base.view(same);sig_array=base.array(alias_sig);sig_view=base.view(sig_array)
   if not lib.schnorr_fast_verify_vartime(alias,alias,sig_view) or not lib.nostr_fast_verify_vartime(alias,alias,sig_view) or bytes(same)!=key:raise AssertionError(('readonly alias',label))
   wasm=d/(label+'.wasm');base.run('/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-I'+str(d),c,ROOT/'coax/tests/nostr/schnorr_fast_wasm.c','-mexec-model=reactor','-Wl,--import-memory,--export-memory,--max-memory=67108864,--export=__heap_base','-lm','-o',wasm)
   observed=json.loads(base.run('node',ROOT/'coax/tests/nostr/schnorr_fast_wasm.cjs',wasm,cases).stdout)

   if any(support.sha(path)!=digest for path,digest in hashes.items()):raise AssertionError('source closure changed during test')
   report.append({'mode':label,'native_cases':len(rows),'wasm_cases':observed['cases'],'source_sha256':hashes,'generated_c_sha256':support.sha(c),'native_sha256':support.sha(library),'wasm_sha256':support.sha(wasm)})
  print(json.dumps({'official':len(official()),'cases':len(rows),'modes':report,'oracle_sha256':support.sha(DEFAULT_LIBRARY),'compiler_sha256':support.sha(base.COMPILER)},indent=2))
 oracle.close()
if __name__=='__main__':main()
