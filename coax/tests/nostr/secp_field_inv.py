#!/usr/bin/env python3
"""Fixed-chain field tests against bigint and pinned public libsecp points.

Explicit test tooling: no production keys, downloads or native oracle rebuilds.
"""
import ctypes as C
import hashlib
import json
import os
from pathlib import Path
import random
import re
import subprocess
import time
import unittest
import secp_field as base
from secp_field_mul import file_sha
from oracle import NativeOracle,DEFAULT_LIBRARY,N

ROOT=base.ROOT
COUNT=int(os.environ.get('COAX_FIELD_INV_COUNT','100000'))
if not 1<=COUNT<=100000:raise ValueError('identity budget is1..100000')
def root_reference(value):
 n=value%base.P;r=pow(n,(base.P+1)//4,base.P)
 if r*r%base.P!=n:return False,0
 return True,base.P-r if r&1 else r
def inverse_reference(value):
 n=value%base.P
 return (False,0) if n==0 else (True,base.reference_field('inverse',n))
DRIVER='''#include "MODE.c"
int main(void){uint8_t in[32],out[32];unsigned count=0;
 while(fread(in,1,32,stdin)==32){
  coax_t_SecpFieldResult a=sf_load((coax_v_u8){in,32}),inv=sf_inverse(a.value),root=sf_sqrt(sf_square(a.value));
  if(!inv.ok||!root.ok)return 2;
  if(!sf_store(inv.value,(coax_v_u8){out,32})||fwrite(out,1,32,stdout)!=32)return 3;
  if(!sf_store(root.value,(coax_v_u8){out,32})||fwrite(out,1,32,stdout)!=32)return 4;
  if(++count>100000)return 5;
 }if(ferror(stdin)||ferror(stdout))return 6;
 fprintf(stderr,"%u identities\\n",count);return 0;}'''

class FieldPowers(unittest.TestCase):
 @classmethod
 def setUpClass(cls):
  base.SecpField.setUpClass();cls.dir=base.SecpField.dir;cls.libs=base.SecpField.libs
  for label,lib in cls.libs:
   for name in ['sf_inverse','sf_inverse_vartime','sf_sqrt']:
    getattr(lib,name).argtypes=[base.Field];getattr(lib,name).restype=base.Result
  cls.report={**base.SecpField.evidence,'schema':'coax.secp-field-powers.v1','scope':'fixed public chain correctness/refusals only; no cross-target constant-time, secret-erasure, device or independent security approval claim','seed':20261028,'budgets':{'identities_per_mode':COUNT,'maximum_identities_per_mode':100000,'stream_record_bytes':32,'driver_timeout_seconds':180,'maximum_temporary_bytes':33554432,'compile_time_steps_aggregate':100000,'source_steps_per_check':20000000},'modes':[]}
  for source in [Path(__file__),ROOT/'coax/tests/nostr/secp_field_mul.py',ROOT/'tools/coax-nostr/fuzz/UPSTREAM.json']:
   cls.report['sources'][str(source.relative_to(ROOT))]=file_sha(source)
 @classmethod
 def tearDownClass(cls):
  cls.report['temporary_bytes']=sum(p.stat().st_size for p in cls.dir.rglob('*') if p.is_file())
  if cls.report['temporary_bytes']>cls.report['budgets']['maximum_temporary_bytes']:raise AssertionError('fixture budget exceeded')
  if os.environ.get('COAX_FIELD_INV_REPORT'):
   Path(os.environ['COAX_FIELD_INV_REPORT']).write_text(json.dumps(cls.report,indent=2,sort_keys=True)+'\n')
  base.SecpField.tearDownClass()
 def assert_result(self,result,expected):
  self.assertEqual(result.ok,expected[0]);self.assertEqual(base.number(result.value),expected[1]);self.assertLess(base.number(result.value),base.P)
 def test_seeded_inverse_and_square_root_stream(self):
  inputs=self.dir/'identities.bin';rng=random.Random(20261028)
  with inputs.open('wb') as stream:
   for _ in range(COUNT):stream.write((int.from_bytes(rng.randbytes(32),'big')%base.P or 1).to_bytes(32,'big'))
  self.report['input_sha256']=file_sha(inputs)
  for label,lib in self.libs:
   source=self.dir/(label+'_powers.c');exe=self.dir/(label+'_powers');outputs=self.dir/(label+'_identities.bin')
   source.write_text(DRIVER.replace('MODE',label));base.run('cc','-std=c11','-O2',source,'-lm','-o',exe)
   start=time.monotonic()
   with inputs.open('rb') as src,outputs.open('wb') as dst:
    result=subprocess.run([str(exe)],stdin=src,stdout=dst,stderr=subprocess.PIPE,timeout=180)
   self.assertEqual(result.returncode,0,result.stderr.decode());self.assertEqual(result.stderr.decode(),f'{COUNT} identities\n')
   with inputs.open('rb') as src,outputs.open('rb') as out:
    for i in range(COUNT):
     a=int.from_bytes(src.read(32),'big');inverse=int.from_bytes(out.read(32),'big');root=int.from_bytes(out.read(32),'big')
     if inverse!=base.reference_field('inverse',a) or a*inverse%base.P!=1:self.fail(f'{label} inverse mismatch at seed20261028 index{i}')
     expected=a if not a&1 else base.P-a
     if root!=expected or root*root%base.P!=a*a%base.P:self.fail(f'{label} square-root mismatch at index{i}')
    self.assertEqual(out.read(1),b'')
   self.report['modes'].append({'mode':label,'inverses':COUNT,'square_roots':COUNT,'elapsed_seconds':time.monotonic()-start,'output_sha256':file_sha(outputs),'generated_c_sha256':file_sha(self.dir/(label+'.c')),'driver_sha256':file_sha(source),'failures':0})
 def test_zero_nonresidues_aliases_and_magnitude_edges(self):
  rng=random.Random(3406076);values=[0,1,2,3,4,9,base.P-1,base.P-2,base.P,base.P+1,2**256-1]+[rng.getrandbits(256) for _ in range(128)]
  rows=[base.digits(a) for a in values]+[base.Field((C.c_uint64*10)(*d)) for _,d in base.raw_cases(count=64)]
  failures=0;count=0
  for label,lib in self.libs:
   for a in rows:
    number=base.number(a);want_root=root_reference(number);want_inverse=inverse_reference(number)
    self.assert_result(lib.sf_inverse(a),want_inverse);self.assert_result(lib.sf_inverse_vartime(a),want_inverse)
    self.assert_result(lib.sf_sqrt(a),want_root);failures+=not want_root[0];count+=3
  self.report['edge_comparisons']=count;self.report['nonresidue_refusals']=failures
 def test_roots_match_uncompressed_public_points_from_pinned_libsecp(self):
  native=NativeOracle(DEFAULT_LIBRARY)
  try:
   parse=native.lib.secp256k1_ec_pubkey_parse;parse.argtypes=[C.c_void_p,C.c_void_p,C.c_void_p,C.c_size_t];parse.restype=C.c_int
   serialize=native.lib.secp256k1_ec_pubkey_serialize;serialize.argtypes=[C.c_void_p,C.c_void_p,C.POINTER(C.c_size_t),C.c_void_p,C.c_uint];serialize.restype=C.c_int
   rng=random.Random(6076);scalars=[1,2,3,N-1]+[1+rng.randrange(N-1) for _ in range(32)]
   for k in scalars:
    compressed=native.point(k.to_bytes(32,'big'));self.assertIsNotNone(compressed)
    pub=(C.c_uint8*64)();out=(C.c_uint8*65)();length=C.c_size_t(65)
    self.assertEqual(parse(native.context,pub,C.c_char_p(compressed),33),1)
    # Official pinned header: EC_UNCOMPRESSED = FLAGS_TYPE_COMPRESSION =2.
    self.assertEqual(serialize(native.context,out,C.byref(length),pub,2),1);self.assertEqual(length.value,65);self.assertEqual(out[0],4)
    x=int.from_bytes(bytes(out)[1:33],'big');y=int.from_bytes(bytes(out)[33:],'big');even=base.P-y if y&1 else y
    for label,lib in self.libs:self.assert_result(lib.sf_sqrt(base.digits((x*x*x+7)%base.P)),(True,even))
   self.report['libsecp_public_point_cases_per_mode']=len(scalars)
   self.report['libsecp_artifact_sha256']=file_sha(DEFAULT_LIBRARY)
   self.report['libsecp_source_commit']=json.loads((ROOT/'tools/coax-nostr/fuzz/UPSTREAM.json').read_text())['commit']
  finally:native.close()
 def test_actual_wasm_roots_inverse_and_refusal_fixtures(self):
  rng=random.Random(6076);values=[0,1,2,3,4,9,base.P-1,base.P,base.P+1,2**256-1]+[rng.getrandbits(256) for _ in range(32)]
  operations=[];raw=[]
  for a in values:
   for op,reference in [('inverse',inverse_reference),('sqrt',root_reference)]:
    ok,result=reference(a);operations.append({'op':op,'a':f'{a:064x}','b':'0'*64,'status':0 if ok else 1,'expected':f'{result:064x}'})
  for m,d in base.raw_cases(count=16):
   a=sum(n<<(26*i) for i,n in enumerate(d))
   for op,reference in [('inverse',inverse_reference),('sqrt',root_reference)]:
    ok,result=reference(a);raw.append({'op':op,'a':[str(n) for n in d],'status':0 if ok else 1,'expected':f'{result:064x}'})
  fixture=self.dir/'powers.json';fixture.write_text(json.dumps({'operations':operations,'raw':[],'raw_powers':raw}));reports=[]
  for label,lib in self.libs:
   wasm=self.dir/(label+'_powers.wasm')
   base.run('/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-I'+str(self.dir),self.dir/(label+'.c'),ROOT/'coax/tests/nostr/secp_field_wasm.c','-mexec-model=reactor','-Wl,--import-memory,--export-memory,--max-memory=67108864,--export=__heap_base','-lm','-o',wasm)
   report=json.loads(base.run('node',ROOT/'coax/tests/nostr/secp_field_wasm.cjs',wasm,fixture).stdout)
   reports.append({'mode':label,'artifact_sha256':file_sha(wasm),'wasm_compiler':base.run('/opt/homebrew/opt/llvm/bin/clang','--version').stdout.splitlines()[0],**report})
  self.report['wasm']=reports
 def test_exponent_overflow_refusals_and_emitted_sanitizer(self):
  templates=[
   'a: SfExponent; z: SfExponent; for i: 0..<8 { a.word[i] = 4294967295; } return sf_exponent_step(a,1,z);',
   'a: SfExponent; b: SfExponent; b.word[0] = 1; for i: 0..<8 { a.word[i] = 4294967295; } return sf_exponent_step(a,0,b);']
  refusals=0
  for mode in [[],['--release']]:
   for i,body in enumerate(templates):
    path=self.dir/f'exp-negative{i}.coax';path.write_text(f'#load "{base.SOURCE}";\nsf_bad_exponent_fixture :: () -> SfExponent {{ {body} }}\nBAD_EXPONENT :: #run sf_bad_exponent_fixture();\n')
    result=subprocess.run([str(base.COMPILER),'c',str(path),'--library',*mode],capture_output=True,text=True)
    self.assertNotEqual(result.returncode,0);self.assertIn('assertion failed',result.stderr);refusals+=1
  self.report['compile_time_exponent_refusals']=refusals
  c=self.dir/'sanitized_power.c';exe=self.dir/'sanitized_power'
  c.write_text('''#include "checked.c"
#include <assert.h>
int main(void){uint64_t seed=6076;coax_t_SecpField one={0};one.limb.e[0]=1;
 for(int t=0;t<256;t++){coax_t_SecpField a={0};
  for(int i=0;i<10;i++){uint64_t limit=8*(i==9?UINT64_C(4194303):UINT64_C(67108863));
   seed=seed*UINT64_C(6364136223846793005)+1;a.limb.e[i]=seed%(limit+1);}
  coax_t_SecpFieldResult inv=sf_inverse(a),root=sf_sqrt(sf_square(a));
  assert(inv.ok);assert(sf_equal_mask(sf_multiply(a,inv.value),one)==UINT64_MAX);
  assert(root.ok);assert(sf_equal_mask(sf_square(root.value),sf_square(a))==UINT64_MAX);assert((root.value.limb.e[0]&1)==0);
 }return 0;}''')
  base.run('cc','-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer',c,'-lm','-o',exe);base.run(exe)
  self.report['sanitized_raw_power_cases']=256
 def test_actual_emitted_faults_are_detected(self):
  original=(self.dir/'checked.c').read_text();faults={}
  for name in ['sf_inverse','sf_sqrt']:
   match=re.search(r'coax_t_SecpFieldResult '+name+r'\([^;{}]*\)\s*\{',original);self.assertIsNotNone(match)
   begin=match.end();end=original.index('\n}',begin);body=original[begin:end]
   if name=='sf_inverse':
    self.assertIn('INT64_C(10)',body);faults['wrong_inverse_exponent']=original[:begin]+body.replace('INT64_C(10)','INT64_C(9)',1)+original[end:]
   else:
    self.assertIn('UINT64_C(1)',body);self.assertIn('& l_valid',body)
    faults['wrong_root_parity']=original[:begin]+body.replace('UINT64_C(1)','UINT64_C(0)',1)+original[end:]
    faults['partial_nonresidue_value']=original[:begin]+body.replace('& l_valid','| UINT64_C(0)',1)+original[end:]
  detected=[]
  for name,source in faults.items():
   path=self.dir/(name+'.c');libpath=self.dir/(name+'.dylib');path.write_text(source)
   base.run('cc','-std=c11','-O2','-shared','-fPIC',path,'-lm','-o',libpath)
   lib=C.CDLL(str(libpath));fn=lib.sf_inverse if name=='wrong_inverse_exponent' else lib.sf_sqrt;fn.argtypes=[base.Field];fn.restype=base.Result
   a=2 if name=='wrong_inverse_exponent' else 1 if name=='wrong_root_parity' else 3
   result=fn(base.digits(a));expected=inverse_reference(a) if name=='wrong_inverse_exponent' else root_reference(a)
   self.assertNotEqual((result.ok,base.number(result.value)),expected);detected.append(name)
  self.report['detected_emitted_faults']=detected

if __name__=='__main__':unittest.main()
