#!/usr/bin/env python3
"""Independent variable-base arithmetic evidence; public test inputs only."""
import ctypes as C
from concurrent.futures import ThreadPoolExecutor
import json
import os
from pathlib import Path
import random
import subprocess
import tempfile
import time
import unittest
import secp_field as base
import secp_group as group
from scalar_candidate import Scalar,scalar
from secp_field_mul import file_sha
from oracle import N,NativeOracle,DEFAULT_LIBRARY,point_mul
ROOT=base.ROOT
SOURCE=ROOT/'coax/lib/crypto/secp_mult_const.coax'
COUNT=int(os.environ.get('COAX_CONST_COUNT','100000'))
if not 1<=COUNT<=100000:raise ValueError('random-pair budget is 1..100000')
EDGES=sorted(set([0,1,2,3,7,8,15,16,17,N-2,N-1,N,N+1,2**256-1]+[1<<i for i in range(256)]+[(1<<i)-1 for i in range(1,256)]))
class VariableBase(unittest.TestCase):
 @classmethod
 def setUpClass(cls):
  cls.temp=tempfile.TemporaryDirectory(prefix='coax-secp-const-');cls.dir=Path(cls.temp.name);cls.libs=[];cls.started=time.monotonic();cls.native=NativeOracle(DEFAULT_LIBRARY)
  cls.parse=cls.native.lib.secp256k1_ec_pubkey_parse;cls.parse.argtypes=[C.c_void_p,C.c_void_p,C.c_void_p,C.c_size_t];cls.parse.restype=C.c_int
  cls.serialize=cls.native.lib.secp256k1_ec_pubkey_serialize;cls.serialize.argtypes=[C.c_void_p,C.c_void_p,C.POINTER(C.c_size_t),C.c_void_p,C.c_uint];cls.serialize.restype=C.c_int
  paths=[SOURCE,ROOT/'coax/lib/crypto/secp_group32.coax',ROOT/'coax/lib/crypto/secp_field32.coax',ROOT/'coax/lib/crypto/secp_group.coax',ROOT/'coax/lib/crypto/secp_field.coax',ROOT/'coax/lib/crypto/secp_scalar.coax',ROOT/'coax/lib/crypto/bytes.coax',Path(__file__)]+[ROOT/'coax/tests/nostr'/x for x in ['secp_mult_const_stream.c','secp_mult_const_wasm.c','secp_mult_const_wasm.cjs','secp_field32.py','secp_field32_adapter.coax','secp_field.py','secp_field_mul.py','secp_group.py','scalar_candidate.py']]+[ROOT/'tools/coax-nostr/fuzz'/x for x in ['oracle.py','UPSTREAM.json']]
  cls.report={'schema':'coax.secp-mult-const.v1','scope':'public-fixture arithmetic and source schedule only; no discharged secret guards, object/JIT review, device, erasure or production-secret approval','seed':20261029,'random_pairs':COUNT,'compiler':base.run(base.COMPILER,'version').stdout.strip(),'compiler_sha256':file_sha(base.COMPILER),'native_compiler':base.run('cc','--version').stdout.splitlines()[0],'native_target':base.run('cc','-dumpmachine').stdout.strip(),'libsecp_artifact_sha256':file_sha(DEFAULT_LIBRARY),'libsecp_source_commit':json.loads((ROOT/'tools/coax-nostr/fuzz/UPSTREAM.json').read_text())['commit'],'sources':{str(p.relative_to(ROOT)):file_sha(p) for p in paths},'modes':[]}
  for name,mode in [('checked',[]),('release',['--release'])]:
   c=cls.dir/(name+'.c');c.write_text(base.run(base.COMPILER,'c',SOURCE,'--library','--header',cls.dir/'secp_mult_const.h',*mode).stdout)
   libpath=cls.dir/(name+'.dylib');base.run('cc','-std=c11','-O2','-shared','-fPIC',c,'-lm','-o',libpath);lib=C.CDLL(str(libpath));lib.smc_multiply.argtypes=[Scalar,group.Point];lib.smc_multiply.restype=group.Result
   stream=cls.dir/(name+'_stream');base.run('cc','-std=c11','-O2','-I'+str(cls.dir),c,ROOT/'coax/tests/nostr/secp_mult_const_stream.c','-lm','-o',stream)
   obj=cls.dir/(name+'.o');base.run('cc','-std=c11','-O2','-fstack-usage','-c',c,'-o',obj);stack=cls.dir/(name+'.su');rows=[x for x in stack.read_text().splitlines() if 'smc_multiply' in x]
   if not rows:raise AssertionError('missing stack record')
   cls.libs.append((name,lib));cls.report['modes'].append({'mode':name,'generated_c_sha256':file_sha(c),'library_sha256':file_sha(libpath),'stream_sha256':file_sha(stream),'object_sha256':file_sha(obj),'object_bytes':obj.stat().st_size,'stack_usage_records':rows})
 @classmethod
 def tearDownClass(cls):
  cls.native.close();cls.report['temporary_bytes']=sum(p.stat().st_size for p in cls.dir.rglob('*') if p.is_file());cls.report['elapsed_seconds']=time.monotonic()-cls.started
  if os.environ.get('COAX_CONST_REPORT'):Path(os.environ['COAX_CONST_REPORT']).write_text(json.dumps(cls.report,indent=2,sort_keys=True)+'\n')
  if cls.report['temporary_bytes']>256*1024*1024:raise AssertionError('temporary budget exceeded')
  cls.temp.cleanup()
 @classmethod
 def affine(cls,k):
  k%=N
  if not k:return None
  encoded=cls.native.point(k.to_bytes(32,'big'));pub=(C.c_uint8*64)()
  if encoded is None or cls.parse(cls.native.context,pub,C.c_char_p(encoded),len(encoded))!=1:raise ValueError('oracle parse')
  out=(C.c_uint8*65)();length=C.c_size_t(65)
  if cls.serialize(cls.native.context,out,C.byref(length),pub,2)!=1 or length.value!=65:raise ValueError('oracle serialize')
  return int.from_bytes(bytes(out)[1:33],'big'),int.from_bytes(bytes(out)[33:],'big')
 def check_point(self,value,want):
  vals=[base.number(v) for v in (value.x,value.y,value.z)]
  for v in (value.x,value.y,value.z):
   self.assertLess(base.number(v),base.P)
   for i,n in enumerate(v.limb):self.assertLessEqual(n,base.M22 if i==9 else base.M26)
  if want is None:self.assertEqual(vals,[1,1,0]);return
  x,y,z=vals;self.assertNotEqual(z,0);self.assertEqual(x,want[0]*z*z%base.P);self.assertEqual(y,want[1]*z*z*z%base.P)
 def test_a_boundaries_and_exceptional_peers(self):
  g=self.affine(1)
  for label,lib in self.libs:
   for i,k in enumerate(EDGES):
    got=lib.smc_multiply(scalar(k),group.point(g,z=1+i%11));self.assertTrue(got.ok);self.check_point(got.value,self.affine(k))
   for k in [0,1,N-1,2**256-1]:
    got=lib.smc_multiply(scalar(k),group.point(None));self.assertTrue(got.ok);self.check_point(got.value,None)
   for bad in [group.Point(),group.Point(base.digits(0),base.digits(1),base.digits(1))]:
    got=lib.smc_multiply(scalar(1),bad);self.assertFalse(got.ok);self.check_point(got.value,None)
  self.report['boundary_scalars_per_mode']=len(EDGES);self.report['exceptional_cases_per_mode']=6
 def test_b_native_random_pairs(self):
  inputs=self.dir/'inputs.bin';expected=self.dir/'expected.bin';rng=random.Random(20261029)
  with inputs.open('wb') as source,expected.open('wb') as target:
   for i in range(COUNT):
    k=rng.getrandbits(256);q=rng.randrange(1,N);peer=self.affine(q);want=self.affine(k*q)
    if i<32:self.assertEqual(want,point_mul(k*q%N))
    source.write(k.to_bytes(32,'big')+peer[0].to_bytes(32,'big')+peer[1].to_bytes(32,'big'))
    target.write(b'\0'+bytes(64) if want is None else b'\1'+want[0].to_bytes(32,'big')+want[1].to_bytes(32,'big'))
  self.report.update({'input_sha256':file_sha(inputs),'expected_sha256':file_sha(expected),'independent_affine_cross_checks':min(32,COUNT)})
  def run_mode(row):
   name,_=row;output=self.dir/(name+'_output.bin');start=time.monotonic()
   with inputs.open('rb') as src,output.open('wb') as dst:r=subprocess.run([str(self.dir/(name+'_stream'))],stdin=src,stdout=dst,stderr=subprocess.PIPE,timeout=1800,check=False)
   self.assertEqual(r.returncode,0,r.stderr.decode());self.assertEqual(r.stderr.decode(),f'{COUNT} variable-base multiplications\n');self.assertEqual(output.stat().st_size,96*COUNT)
   with expected.open('rb') as oracle,output.open('rb') as target:
    for i in range(COUNT):
     want=oracle.read(65);got=target.read(96);x,y,z=[int.from_bytes(got[j:j+32],'big') for j in [0,32,64]]
     self.assertLess(max(x,y,z),base.P)
     if not want[0]:self.assertEqual((x,y,z),(1,1,0),(name,i))
     else:
      ex=int.from_bytes(want[1:33],'big');ey=int.from_bytes(want[33:],'big');self.assertNotEqual(z,0);self.assertEqual(x,ex*z*z%base.P,(name,i));self.assertEqual(y,ey*z*z*z%base.P,(name,i))
   return {'mode':name,'pairs':COUNT,'output_sha256':file_sha(output),'elapsed_seconds':time.monotonic()-start,'failures':0}
  with ThreadPoolExecutor(max_workers=2) as pool:self.report['native_random']=list(pool.map(run_mode,self.libs))
 def test_c_wasm_random_pairs_and_guards(self):
  rows=[];g=self.affine(1)
  for k in EDGES:rows.append((k,g,self.affine(k)))
  inputs=self.dir/'wasm_inputs.bin';expected=self.dir/'wasm_expected.bin'
  with inputs.open('wb') as source,expected.open('wb') as target:
   for k,peer,want in rows:
    source.write(k.to_bytes(32,'big')+peer[0].to_bytes(32,'big')+peer[1].to_bytes(32,'big'));target.write(b'\0'+bytes(64) if want is None else b'\1'+want[0].to_bytes(32,'big')+want[1].to_bytes(32,'big'))
   source.write((self.dir/'inputs.bin').read_bytes());target.write((self.dir/'expected.bin').read_bytes())
  reports=[]
  for name,_ in self.libs:
   wasm=self.dir/(name+'.wasm');base.run('/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-I'+str(self.dir),self.dir/(name+'.c'),ROOT/'coax/tests/nostr/secp_mult_const_wasm.c','-mexec-model=reactor','-Wl,--import-memory,--export-memory,--max-memory=67108864,--export=__heap_base,-z,stack-size=1048576','-lm','-o',wasm)
   reports.append({'mode':name,'artifact_sha256':file_sha(wasm),'stack_bytes':1048576,**json.loads(base.run('node',ROOT/'coax/tests/nostr/secp_mult_const_wasm.cjs',wasm,inputs,expected,timeout=1800).stdout)})
  self.report['wasm']=reports
 def test_d_sanitizers_and_short_input(self):
  probe=self.dir/'sanitized.c';probe.write_text('#include "checked.c"\n#include <assert.h>\nint main(void){coax_t_SgPoint g=sg_generator();coax_t_ScScalar s={0};for(unsigned i=0;i<256;i++){s.limbs.e[i/32]=UINT32_C(1)<<(i%32);coax_t_SgResult r=smc_multiply(s,g);assert(r.ok&&sg_valid_mask(r.value)==UINT64_MAX);s.limbs.e[i/32]=0;}return 0;}\n')
  exe=self.dir/'sanitized';base.run('cc','-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer',probe,'-lm','-o',exe);base.run(exe,timeout=1800)
  for name,_ in self.libs:
   result=subprocess.run([str(self.dir/(name+'_stream'))],input=bytes(95),capture_output=True);self.assertEqual(result.returncode,7);self.assertEqual(result.stdout,b'')
  self.report['sanitizer_single_bits']=256;self.report['truncated_stream_refusals']=2
if __name__=='__main__':unittest.main()
