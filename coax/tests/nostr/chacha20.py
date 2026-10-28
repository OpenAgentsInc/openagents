#!/usr/bin/env python3
"""Pinned RFC bytes, independent OpenSSL, real native/Wasm and benchmark gates."""
import ctypes as C
import hashlib
import json
import os
from pathlib import Path
import platform
import random
import re
import struct
import subprocess
import sys
import tempfile
import unittest

ROOT=Path(__file__).resolve().parents[3]
COMPILER=Path(os.environ.get('COAX_TEST_COMPILER',ROOT/'coax/bin/coax'))
SOURCE=ROOT/'coax/lib/crypto/chacha20.coax';BRIDGE=ROOT/'coax/lib/crypto/chacha20-memory.coax'
RFC=ROOT/'coax/tests/vectors/upstream/rfc/rfc8439.txt'
RFC_HASH='25bef70fbf7a07ff45c2fe4cb7c6ce954eac687413d8610603268b4e4415324c'
LIMIT=1048576
def run(*args,**kw):
 r=subprocess.run([str(a) for a in args],capture_output=True,text=True,**kw)
 if r.returncode:raise RuntimeError(f'{args[0]} failed({r.returncode}): {r.stderr or r.stdout}')
 return r
def rows(text):
 return bytes.fromhex(' '.join(m.group(1) for line in text.splitlines() if(m:=re.match(r'^\s*\d{3}  ((?:[0-9a-fA-F]{2}(?: |$)){1,16})',line))))
def rfc_vectors():
 assert hashlib.sha256(RFC.read_bytes()).hexdigest()==RFC_HASH
 text=RFC.read_text();vectors=[]
 block=text.split('\n2.3.2.  ',1)[1].split('\n2.4.  ',1)[0]
 vectors.append(dict(id='2.3.2',key=bytes(range(32)),nonce=bytes.fromhex('000000090000004a00000000'),counter=1,input=bytes(64),output=rows(block.split('Serialized Block:',1)[1]),block=True))
 cipher=text.split('\n2.4.2.  ',1)[1].split('\n2.5.  ',1)[0]
 vectors.append(dict(id='2.4.2',key=bytes(range(32)),nonce=bytes.fromhex('000000000000004a00000000'),counter=1,input=rows(cipher.split('Plaintext Sunscreen:',1)[1].split('The following figure',1)[0]),output=rows(cipher.split('Ciphertext Sunscreen:',1)[1]),block=False))
 for section,next_section,is_block in [('A.1.','A.2.',True),('A.2.','A.3.',False)]:
  body=text.split('\n'+section+'  ',1)[1].split('\n'+next_section+'  ',1)[0]
  parts=re.split(r'\n  Test Vector #(\d+):\n',body)
  for at in range(1,len(parts),2):
   number,case=parts[at:at+2];key=rows(case.split('Key:',1)[1].split('Nonce:',1)[0]);nonce=rows(case.split('Nonce:',1)[1].split('Counter =',1)[0]);counter=int(re.search(r'Counter = (\d+)',case).group(1))
   output=rows(case.split('Keystream:' if is_block else 'Ciphertext:',1)[1]);data=bytes(64) if is_block else rows(case.split('Plaintext:',1)[1].split('Ciphertext:',1)[0])
   assert len(key)==32 and len(nonce)==12 and len(data)==len(output)
   vectors.append(dict(id=section+'#'+number,key=key,nonce=nonce,counter=counter,input=data,output=output,block=is_block))
 assert len(vectors)==10 and all(len(v['input'])==len(v['output']) for v in vectors)
 return vectors
def wire(c):return{k:v.hex() if isinstance(v,bytes) else v for k,v in c.items()}
class View(C.Structure):_fields_=[('data',C.POINTER(C.c_uint8)),('count',C.c_int64)]
class Result(C.Structure):_fields_=[('written',C.c_int64),('error',C.c_uint32)]
def array(b):return(C.c_uint8*len(b))(*b)
def view(a,n=None,at=0):return View(C.cast(C.byref(a,at),C.POINTER(C.c_uint8)),len(a)-at if n is None else n)

class ChaCha(unittest.TestCase):
 @classmethod
 def setUpClass(cls):
  cls.temp=tempfile.TemporaryDirectory(prefix='coax-chacha20-');cls.dir=Path(cls.temp.name);cls.cases=rfc_vectors();rng=random.Random(8439)
  for name,n,counter in [('last-counter',64,0xffffffff),('last-two-blocks',128,0xfffffffe),('last-max-request',LIMIT,0xffffc000)]:
   cls.cases.append(dict(id=name,key=rng.randbytes(32),nonce=rng.randbytes(12),counter=counter,input=rng.randbytes(n),block=False))
  sizes=[0,1,2,3,4,31,32,63,64,65,127,128,129,255,256,257,4097,LIMIT-1,LIMIT]
  for i in range(256):
   n=sizes[i] if i<len(sizes) else rng.randrange(2049);blocks=(n+63)//64;counter=min(rng.getrandbits(32),0x100000000-blocks)
   if i==0:counter=0xffffffff
   cls.cases.append(dict(id='oracle-'+str(i),key=rng.randbytes(32),nonce=rng.randbytes(12),counter=counter,input=rng.randbytes(n),block=False))
  request=cls.dir/'request.json';request.write_text(json.dumps([wire(c) for c in cls.cases]))
  oracle=json.loads(run('node',ROOT/'coax/tests/nostr/chacha20_wasm.cjs','oracle',request).stdout)
  for c,expected in zip(cls.cases,oracle['outputs'],strict=True):
   if 'output'in c:assert c['output'].hex()==expected,c['id']
   c['output']=bytes.fromhex(expected)
  cls.fixture=cls.dir/'vectors.json';cls.fixture.write_text(json.dumps([wire(c) for c in cls.cases]))
  binary=bytearray()
  for c in cls.cases:binary.extend(struct.pack('<IIB',len(c['input']),c['counter'],c['block'])+c['key']+c['nonce']+c['input']+c['output'])
  cls.binary=bytes(binary);cls.libs=[]
  cls.evidence={'compiler':run(COMPILER,'version').stdout.strip(),'compiler_sha256':hashlib.sha256(COMPILER.read_bytes()).hexdigest(),'platform':platform.platform(),'machine':platform.machine(),'cpu':run('sysctl','-n','machdep.cpu.brand_string').stdout.strip() if platform.system()=='Darwin' else platform.processor(),'oracle':{k:v for k,v in oracle.items() if k!='outputs'},'rfc_sha256':RFC_HASH,'records':len(cls.cases),'method':'Warm scalar O2 Wasm kernel, 7 samples of32x1MiB; 5 warmups, input byte varies each iteration and output feeds observable sink. Decimal MB/s; no UI/network/cold-start timing. Test keys only; repeated fixture nonce/counters are not production usage.','sources':{str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in [SOURCE,BRIDGE,ROOT/'coax/lib/crypto/bytes.coax',Path(__file__),ROOT/'coax/tests/nostr/chacha20_wasm.c',ROOT/'coax/tests/nostr/chacha20_wasm.cjs',ROOT/'coax/tests/nostr/chacha20_native.c']},'measurements':[]}
  cls.evidence.update(seed=8439,fixture_sha256=hashlib.sha256(cls.fixture.read_bytes()).hexdigest(),native_compiler=run('cc','--version').stdout.splitlines()[0],generated_c={})
  for label,flags in [('checked',[]),('release',['--release'])]:
   c=cls.dir/(label+'.c');c.write_text(run(COMPILER,'c',BRIDGE,'--library','--header',cls.dir/'chacha20.h',*flags).stdout)
   cls.evidence['generated_c'][label]=hashlib.sha256(c.read_bytes()).hexdigest()
   libfile=cls.dir/(label+'.dylib');run('cc','-std=c11','-O2','-shared','-fPIC',c,'-lm','-o',libfile);lib=C.CDLL(str(libfile))
   lib.chacha20_block.argtypes=[View,View,C.c_uint32,View];lib.chacha20_block.restype=Result
   lib.chacha20_xor.argtypes=[View,View,C.c_uint32,View,View];lib.chacha20_xor.restype=Result;cls.libs.append(lib)
 @classmethod
 def tearDownClass(cls):cls.temp.cleanup()
 def test_native_all_rfc_and_openssl_records(self):
  for lib in self.libs:
   for c in self.cases:
    key=array(c['key']);nonce=array(c['nonce']);data=array(c['input']);n=len(data);out=array(b'\xa5'*(n+8));cap=min(LIMIT,n+8)
    r=lib.chacha20_block(view(key),view(nonce),c['counter'],view(out,cap)) if c['block'] else lib.chacha20_xor(view(key),view(nonce),c['counter'],view(data),view(out,cap))
    self.assertEqual((r.error,r.written),(0,n),c['id']);self.assertEqual(bytes(out)[:n],c['output'],c['id']);self.assertEqual(bytes(out)[n:],b'\xa5'*8)
 def test_checked_and_release_sanitized_native_boundary(self):
  for label in ['checked','release']:
   exe=self.dir/(label+'-sanitized');run('cc','-std=c11','-O2','-g','-fsanitize=address,undefined','-fno-sanitize-recover=all','-I'+str(self.dir),self.dir/(label+'.c'),ROOT/'coax/tests/nostr/chacha20_native.c','-lm','-o',exe)
   r=subprocess.run([str(exe)],input=self.binary,capture_output=True,env={**os.environ,'ASAN_OPTIONS':'detect_leaks=0'});self.assertEqual(r.returncode,0,r.stderr.decode());print(label+': '+r.stdout.decode().strip())
 def test_actual_wasm_oracle_and_performance_gate(self):
  clang=Path(os.environ.get('COAXIAL_CLANG','/opt/homebrew/opt/llvm/bin/clang'));sysroot=Path(os.environ.get('COAXIAL_SYSROOT','/opt/homebrew/opt/wasi-libc/share/wasi-sysroot'));resources=Path(os.environ.get('COAXIAL_RESOURCES','/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes'))
  environment={**os.environ,'PATH':'/opt/homebrew/opt/lld/bin:'+os.environ.get('PATH','')}
  for label in ['checked','release']:
   module=self.dir/(label+'.wasm');run(clang,'--target=wasm32-wasip1','--sysroot='+str(sysroot),'-resource-dir='+str(resources),'-std=c11','-O2','-ffp-contract=off','-fno-fast-math','-I'+str(self.dir),'-mexec-model=reactor',self.dir/(label+'.c'),ROOT/'coax/tests/nostr/chacha20_wasm.c','-Wl,--import-memory,--export=__heap_base,--max-memory=67108864,-z,stack-size=1048576,--gc-sections','-o',module,env=environment)
   evidence=json.loads(run('node',ROOT/'coax/tests/nostr/chacha20_wasm.cjs',module,self.fixture).stdout);evidence.update(profile=label,optimization='O2',simd=False,wasm_compiler=run(clang,'--version').stdout.splitlines()[0]);self.evidence['measurements'].append(evidence)
   print(f"{label}: {evidence['records']} Wasm oracle records, median {evidence['median_mb_per_second']:.2f} MB/s")
   if label=='release':self.assertGreaterEqual(evidence['median_mb_per_second'],250,'release Wasm throughput gate')

if __name__=='__main__':
 evidence=None
 if '--evidence'in sys.argv:
  at=sys.argv.index('--evidence');evidence=Path(sys.argv[at+1]);del sys.argv[at:at+2]
 result=unittest.main(exit=False)
 if result.result.wasSuccessful() and evidence:evidence.write_text(json.dumps(ChaCha.evidence,indent=2)+'\n')
 sys.exit(not result.result.wasSuccessful())
