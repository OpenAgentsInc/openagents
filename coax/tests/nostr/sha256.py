#!/usr/bin/env python3
"""Focused SHA-256 stream/tag/oracle tests. Explicit tooling, no app dependency."""
import ctypes as C
import hashlib
import json
import os
from pathlib import Path
import platform
import random
import re
import subprocess
import sys
import tempfile
import unittest

ROOT=Path(__file__).resolve().parents[3]
COMPILER=Path(os.environ.get('COAX_TEST_COMPILER',ROOT/'coax/bin/coax'))
PURE=ROOT/'coax/lib/crypto/sha256.coax'
BRIDGE=ROOT/'coax/lib/crypto/sha256-memory.coax'
FINGERPRINT_PIN='94142391e7e8122c4314443a878d65e61a1318cc5de2430f7d1ac826d620728a'

def run(*args,**kw):
 r=subprocess.run([str(a) for a in args],capture_output=True,text=True,**kw)
 if r.returncode:raise RuntimeError(f'{args[0]} failed({r.returncode}): {r.stderr or r.stdout}')
 return r
class View(C.Structure):_fields_=[('data',C.POINTER(C.c_uint8)),('count',C.c_int64)]
class String(C.Structure):_fields_=[('data',C.c_char_p),('count',C.c_int64)]
class Info(C.Structure):_fields_=[('error',C.c_uint32),('phase',C.c_uint32),('total',C.c_uint64),('buffered',C.c_uint32)]
def array(data):return(C.c_uint8*len(data))(*data)
def view(data,at=0,count=None):return View(C.cast(C.byref(data,at),C.POINTER(C.c_uint8)),len(data)-at if count is None else count)
def nist_vectors():
 for name in ['SHA256ShortMsg.rsp','SHA256LongMsg.rsp']:
  text=(ROOT/'coax/tests/vectors/upstream/nist'/name).read_text()
  for bits,msg,digest in re.findall(r'Len\s*=\s*(\d+)\s+Msg\s*=\s*([0-9a-fA-F]+)\s+MD\s*=\s*([0-9a-fA-F]+)',text):
   assert int(bits)%8==0
   yield bytes.fromhex(msg)[:int(bits)//8],bytes.fromhex(digest)

class Sha256(unittest.TestCase):
 @classmethod
 def setUpClass(cls):
  cls.temp=tempfile.TemporaryDirectory(prefix='coax-sha256-');cls.dir=Path(cls.temp.name);cls.libs=[]
  cpu=run('sysctl','-n','machdep.cpu.brand_string').stdout.strip() if platform.system()=='Darwin' else platform.processor()
  cls.evidence={'compiler':run(COMPILER,'version').stdout.strip(),'compiler_sha256':hashlib.sha256(COMPILER.read_bytes()).hexdigest(),'platform':platform.platform(),'machine':platform.machine(),'cpu':cpu,'method':'Warm kernel-only throughput; 7 samples, native32x1MiB and Wasm8x1MiB per sample; input first byte varies each iteration and digest bytes feed an observable sink. MB=1000000 bytes. No UI/network or cold-start measurement.','native_compiler':run('cc','--version').stdout.splitlines()[0],'sources':{str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in [PURE,BRIDGE,ROOT/'coax/lib/proto/fingerprint.coax',ROOT/'coax/lib/crypto/bytes.coax',ROOT/'coax/tests/nostr/sha256_wasm.c',ROOT/'coax/tests/nostr/sha256_wasm.cjs',ROOT/'coax/tests/nostr/sha256_adapter.coax']},'nist_sources':{name:hashlib.sha256((ROOT/'coax/tests/vectors/upstream/nist'/name).read_bytes()).hexdigest() for name in ['SHA256ShortMsg.rsp','SHA256LongMsg.rsp']},'measurements':[]}
  for label,mode in [('checked',[]),('release',['--release'])]:
   c=cls.dir/(label+'.c');c.write_text(run(COMPILER,'c',BRIDGE,'--library','--header',cls.dir/'sha256.h',*mode).stdout)
   run('cc','-std=c11','-O2','-shared','-fPIC',c,'-lm','-o',cls.dir/(label+'.dylib'))
   lib=C.CDLL(str(cls.dir/(label+'.dylib')))
   lib.sha256_state_size.argtypes=[];lib.sha256_state_size.restype=C.c_int64
   lib.sha256_init.argtypes=[View];lib.sha256_init.restype=C.c_uint32
   lib.sha256_info.argtypes=[View];lib.sha256_info.restype=Info
   for name in ['sha256_update','sha256_final','sha256_hash']:
    f=getattr(lib,name);f.argtypes=[View,View];f.restype=C.c_uint32
   lib.sha256_tagged.argtypes=[String,View,View];lib.sha256_tagged.restype=C.c_uint32
   lib.sha256_bip340.argtypes=[C.c_int64,View,View];lib.sha256_bip340.restype=C.c_uint32
   cls.libs.append(lib)
 @classmethod
 def tearDownClass(cls):cls.temp.cleanup()
 def digest(self,lib,data):
  src=array(data);out=array([0xa5]*40);self.assertEqual(lib.sha256_hash(view(src),view(out)),0)
  self.assertEqual(bytes(out)[32:],b'\xa5'*8);return bytes(out)[:32]
 def stream(self,lib,data,chunk):
  state=array([0]*lib.sha256_state_size());self.assertEqual(lib.sha256_init(view(state)),0)
  source=array(data)
  for at in range(0,len(data),chunk):self.assertEqual(lib.sha256_update(view(state),view(source,at,min(chunk,len(data)-at))),0)
  before=lib.sha256_info(view(state));self.assertEqual((before.error,before.phase,before.total,before.buffered),(0,1,len(data),len(data)%64))
  out=array([0xa5]*32);self.assertEqual(lib.sha256_final(view(state),view(out)),0)
  snapshot=bytes(state);self.assertEqual(lib.sha256_final(view(state),view(out)),0);self.assertEqual(bytes(state),snapshot)
  info=lib.sha256_info(view(state));self.assertEqual((info.error,info.phase,info.total,info.buffered),(0,2,len(data),0))
  return bytes(out)
 def test_official_nist_vectors_and_independent_hashlib(self):
  vectors=list(nist_vectors());self.assertGreater(len(vectors),100)
  for data,digest in vectors:
   self.assertEqual(hashlib.sha256(data).digest(),digest)
   for lib in self.libs:self.assertEqual(self.digest(lib,data),digest)
 def test_stream_boundaries_against_hashlib(self):
  rng=random.Random(256)
  for size in [0,1,2,31,32,55,56,57,63,64,65,119,120,127,128,129,255,256,1000,4097]:
   data=rng.randbytes(size);expected=hashlib.sha256(data).digest()
   for lib in self.libs:
    for chunk in [1,7,55,56,63,64,65,127,129,4096]:self.assertEqual(self.stream(lib,data,chunk),expected)
 def test_tagged_hash_and_bip340_names(self):
  rng=random.Random(340)
  for tag in [b'',b'example',b'BIP0340/aux',b'BIP0340/nonce',b'BIP0340/challenge']:
   for size in [0,1,17,32,100,129]:
    data=rng.randbytes(size);src=array(data);th=hashlib.sha256(tag).digest();expected=hashlib.sha256(th+th+data).digest()
    for lib in self.libs:
     out=array([0xa5]*40);self.assertEqual(lib.sha256_tagged(String(tag,len(tag)),view(src),view(out)),0)
     self.assertEqual(bytes(out)[:32],expected);self.assertEqual(bytes(out)[32:],b'\xa5'*8)
     if tag.startswith(b'BIP0340/'):
      which=[b'BIP0340/aux',b'BIP0340/nonce',b'BIP0340/challenge'].index(tag)
      self.assertEqual(lib.sha256_bip340(which,view(src),view(out)),0);self.assertEqual(bytes(out)[:32],expected)
 def test_errors_preserve_state_and_output(self):
  for lib in self.libs:
   state=array([0xa5]*112);out=array([0xa5]*40);source=array([1,2,3])
   self.assertNotEqual(lib.sha256_update(view(state),view(source)),0);self.assertEqual(bytes(state),b'\xa5'*112)
   for count in [-1,0,111,113]:
    self.assertNotEqual(lib.sha256_init(view(state,count=count)),0);self.assertEqual(bytes(state),b'\xa5'*112)
   self.assertEqual(lib.sha256_init(view(state)),0)
   for operation in [lambda:lib.sha256_update(view(state),view(source,count=-1)),lambda:lib.sha256_update(view(state),view(source,count=1048577)),lambda:lib.sha256_final(view(state),view(out,count=31)),lambda:lib.sha256_update(view(state),view(state,48,3)),lambda:lib.sha256_final(view(state),view(state,0,32))]:
    before=bytes(state);self.assertNotEqual(operation(),0);self.assertEqual(bytes(state),before);self.assertEqual(bytes(out),b'\xa5'*40)
   self.assertEqual(lib.sha256_final(view(state),view(out)),0);before=bytes(state)
   self.assertNotEqual(lib.sha256_update(view(state),view(source)),0);self.assertEqual(bytes(state),before)
   for index,value in [(0,0),(4,2),(5,0),(6,1),(7,64),(15,1)]:
    self.assertEqual(lib.sha256_init(view(state)),0);state[index]=value;before=bytes(state)
    self.assertNotEqual(lib.sha256_update(view(state),view(source)),0);self.assertEqual(bytes(state),before)
 def test_one_shot_alias_and_tag_refusal(self):
  for lib in self.libs:
   data=array(range(64));before=bytes(data)
   self.assertNotEqual(lib.sha256_hash(view(data),view(data,0,32)),0);self.assertEqual(bytes(data),before)
   for tag in [String(b'x',-1),String(b'x',1025)]:
    out=array([0xa5]*32);self.assertNotEqual(lib.sha256_tagged(tag,view(data),view(out)),0);self.assertEqual(bytes(out),b'\xa5'*32)
   for tag in [-1,3,2**63-1]:
    out=array([0xa5]*32);self.assertNotEqual(lib.sha256_bip340(tag,view(data),view(out)),0);self.assertEqual(bytes(out),b'\xa5'*32)
 def test_frozen_fingerprint_source_and_digest_evidence(self):
  self.assertEqual(hashlib.sha256((ROOT/'coax/lib/proto/fingerprint.coax').read_bytes()).hexdigest(),FINGERPRINT_PIN)

 def test_one_mib_all_chunk_sizes_native(self):
  expected=','.join(str(x) for x in hashlib.sha256(bytes(range(256))*4096).digest())
  for label in ['checked','release']:
   host=self.dir/(label+'_chunks.c')
   host.write_text(f'''#include "{label}.c"
#include <assert.h>
#include <stdio.h>
#include <time.h>
static uint8_t data[1048576];
int main(void){{
 uint8_t state[112],out[32],expected[32]={{{expected}}};
 for(size_t i=0;i<sizeof(data);i++)data[i]=(uint8_t)i;
 for(int chunk=1;chunk<=129;chunk++){{
  assert(sha256_init((coax_v_u8){{state,112}})==0);
  for(int at=0;at<1048576;at+=chunk){{int n=1048576-at<chunk?1048576-at:chunk;assert(sha256_update((coax_v_u8){{state,112}},(coax_v_u8){{data+at,n}})==0);}}
  assert(sha256_final((coax_v_u8){{state,112}},(coax_v_u8){{out,32}})==0);
  for(int i=0;i<32;i++)assert(out[i]==expected[i]);
 }}
 double rates[7];volatile unsigned sink=0;
 for(int sample=0;sample<7;sample++){{
  struct timespec a,b;clock_gettime(CLOCK_MONOTONIC,&a);
  for(int i=0;i<32;i++){{data[0]=(uint8_t)i;assert(sha256_hash((coax_v_u8){{data,1048576}},(coax_v_u8){{out,32}})==0);sink^=out[i&31];}}
  clock_gettime(CLOCK_MONOTONIC,&b);double seconds=(b.tv_sec-a.tv_sec)+(b.tv_nsec-a.tv_nsec)*1e-9;
  rates[sample]=(32.0*1048576.0/1e6)/seconds;
 }}
 printf("{{\\\"target\\\":\\\"native-c11\\\",\\\"profile\\\":\\\"{label}\\\",\\\"optimization\\\":\\\"O2\\\",\\\"chunk_sizes\\\":129,\\\"payload_bytes\\\":1048576,\\\"mb_per_second\\\":[");
 for(int i=0;i<7;i++)printf("%s%.6f",i?",":"",rates[i]);printf("],\\\"digest_sink\\\":%u}}\\n",sink);return 0;
}}
''')
   run('cc','-std=c11','-O2',host,'-lm','-o',self.dir/(label+'_chunks'))
   self.evidence['measurements'].append(json.loads(run(self.dir/(label+'_chunks')).stdout))

 def test_native_state_and_alias_sanitizers(self):
  host=self.dir/'sanitized.c'
  host.write_text('''#include "sha256.h"
#include <assert.h>
int main(void){
 uint8_t state[112],input[129],out[40];
 for(int i=0;i<129;i++)input[i]=(uint8_t)i;
 assert(sha256_init((coax_v_u8){state,112})==0);
 assert(sha256_update((coax_v_u8){state,112},(coax_v_u8){input,63})==0);
 assert(sha256_update((coax_v_u8){state,112},(coax_v_u8){input+63,66})==0);
 assert(sha256_final((coax_v_u8){state,112},(coax_v_u8){out,40})==0);
 assert(sha256_update((coax_v_u8){state,112},(coax_v_u8){input,1})!=0);
 assert(sha256_init((coax_v_u8){state,-1})!=0);
 assert(sha256_hash((coax_v_u8){input,-1},(coax_v_u8){out,32})!=0);
 assert(sha256_init((coax_v_u8){state,112})==0);
 assert(sha256_update((coax_v_u8){state,112},(coax_v_u8){state+48,1})!=0);
 assert(sha256_final((coax_v_u8){state,112},(coax_v_u8){state,32})!=0);
 return 0;
}''')
  run('cc','-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer',self.dir/'checked.c',host,'-lm','-o',self.dir/'sanitized')
  run(self.dir/'sanitized')

 def test_actual_wasm_vectors_streams_tags_and_throughput(self):
  clang=Path(os.environ.get('COAXIAL_CLANG','/opt/homebrew/opt/llvm/bin/clang'))
  sysroot=Path(os.environ.get('COAXIAL_SYSROOT','/opt/homebrew/opt/wasi-libc/share/wasi-sysroot'))
  resources=Path(os.environ.get('COAXIAL_RESOURCES','/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes'))
  self.assertTrue(clang.exists() and sysroot.exists() and resources.exists(),'actual Wasm toolchain is required; see coax/web/wasm.sh')
  vectors=[{'input':data.hex(),'digest':digest.hex()} for data,digest in nist_vectors()]
  streams=[{'input':bytes(i%256 for i in range(n)).hex(),'digest':hashlib.sha256(bytes(i%256 for i in range(n))).hexdigest(),'chunk':chunk} for n in [0,55,56,63,64,65,120,129,4097] for chunk in [1,7,64,129]]
  tags=[]
  for tag_id,tag in enumerate([b'BIP0340/aux',b'BIP0340/nonce',b'BIP0340/challenge']):
   for data in [b'',b'a',b'abc',bytes(range(100))]:
    th=hashlib.sha256(tag).digest();tags.append({'id':tag_id,'input':data.hex(),'digest':hashlib.sha256(th+th+data).hexdigest()})
  fixture=self.dir/'wasm_vectors.json';fixture.write_text(json.dumps({'vectors':vectors,'streams':streams,'tags':tags,'large_digest':hashlib.sha256(bytes(range(256))*4096).hexdigest()}))
  face=ROOT/'coax/tests/nostr/sha256_wasm.c'
  environment=os.environ.copy();environment['PATH']='/opt/homebrew/opt/lld/bin:'+environment.get('PATH','')
  for label in ['checked','release']:
   module=self.dir/(label+'.wasm')
   run(clang,'--target=wasm32-wasip1','--sysroot='+str(sysroot),'-resource-dir='+str(resources),'-std=c11','-O2','-ffp-contract=off','-fno-fast-math','-I'+str(self.dir),'-mexec-model=reactor',self.dir/(label+'.c'),face,'-Wl,--import-memory,--export-memory,--export=__heap_base,--max-memory=67108864,-z,stack-size=1048576,--gc-sections','-o',module,env=environment)
   measurement=json.loads(run('node',ROOT/'coax/tests/nostr/sha256_wasm.cjs',module,fixture).stdout)
   measurement.update(profile=label,optimization='O2',module_sha256=hashlib.sha256(module.read_bytes()).hexdigest(),wasm_compiler=run(clang,'--version').stdout.splitlines()[0]);self.evidence['measurements'].append(measurement)

if __name__=='__main__':
 evidence=None
 if '--evidence' in sys.argv:
  at=sys.argv.index('--evidence');evidence=Path(sys.argv[at+1]);del sys.argv[at:at+2]
 result=unittest.main(exit=False)
 if result.result.wasSuccessful() and evidence:
  evidence.write_text(json.dumps(Sha256.evidence,indent=2)+'\n')
 sys.exit(not result.result.wasSuccessful())
