#!/usr/bin/env python3
"""Bounded streamed product oracle for generated Coax, never production tooling."""
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

ROOT=base.ROOT
COUNT=int(os.environ.get('COAX_FIELD_MUL_COUNT','1000000'))
if not 1<=COUNT<=1000000:raise ValueError('product budget is1..1000000')
DRIVER='''#include "MODE.c"
int main(void){
 uint8_t in[64],out[32];unsigned count=0;
 while(fread(in,1,64,stdin)==64){
  coax_t_SecpFieldResult a=sf_load((coax_v_u8){in,32}),b=sf_load((coax_v_u8){in+32,32});
  coax_t_SecpField result=sf_multiply(a.value,b.value);
  if(!sf_store(result,(coax_v_u8){out,32})||fwrite(out,1,32,stdout)!=32)return 3;
  if((count%256)==0){result=sf_square(a.value);if(!sf_store(result,(coax_v_u8){out,32})||fwrite(out,1,32,stdout)!=32)return 4;}
  count++;if(count>1000000)return 5;
 }
 if(ferror(stdin)||ferror(stdout))return 6;
 fprintf(stderr,"%u products\\n",count);return 0;
}'''
def file_sha(path):
 with path.open('rb') as stream:return hashlib.file_digest(stream,'sha256').hexdigest()

class FieldMultiply(unittest.TestCase):
 @classmethod
 def setUpClass(cls):
  base.SecpField.setUpClass();cls.dir=base.SecpField.dir;cls.libs=base.SecpField.libs
  for label,lib in cls.libs:
   lib.sf_multiply.argtypes=[base.Field,base.Field];lib.sf_multiply.restype=base.Field
   lib.sf_square.argtypes=[base.Field];lib.sf_square.restype=base.Field
  cls.report={**base.SecpField.evidence,'schema':'coax.secp-field-multiply.v1','scope':'bounded multiplication/square correctness only; no constant-time, device, secure-erasure, one-hour or security-review claim','seed':20261028,'budgets':{'random_products_per_mode':COUNT,'maximum_products_per_mode':1000000,'stream_record_bytes':64,'maximum_temporary_bytes':167772160,'driver_timeout_seconds':120,'source_check_steps_per_check':20000000},'modes':[]}
  for source in [Path(__file__),ROOT/'coax/lib/crypto/secp_field.coax']:
   cls.report['sources'][str(source.relative_to(ROOT))]=hashlib.sha256(source.read_bytes()).hexdigest()
 @classmethod
 def tearDownClass(cls):
  cls.report['temporary_bytes']=sum(p.stat().st_size for p in cls.dir.rglob('*') if p.is_file())
  if cls.report['temporary_bytes']>cls.report['budgets']['maximum_temporary_bytes']:raise AssertionError('temporary fixture budget exceeded')
  if os.environ.get('COAX_FIELD_MUL_REPORT'):
   Path(os.environ['COAX_FIELD_MUL_REPORT']).write_text(json.dumps(cls.report,indent=2,sort_keys=True)+'\n')
  base.SecpField.tearDownClass()
 def test_seeded_product_stream_matches_independent_pinned_integer_oracle(self):
  inputs=self.dir/'million.bin';rng=random.Random(20261028)
  with inputs.open('wb') as stream:
   for _ in range(COUNT):stream.write(rng.randbytes(64))
  self.assertEqual(inputs.stat().st_size,COUNT*64)
  self.report['stream_sha256']=file_sha(inputs)
  for label,lib in self.libs:
   driver=self.dir/(label+'_stream.c');exe=self.dir/(label+'_stream');outputs=self.dir/(label+'_products.bin')
   driver.write_text(DRIVER.replace('MODE',label))
   base.run('cc','-std=c11','-O2',driver,'-lm','-o',exe)
   start=time.monotonic()
   with inputs.open('rb') as source,outputs.open('wb') as target:
    r=subprocess.run([str(exe)],stdin=source,stdout=target,stderr=subprocess.PIPE,timeout=120,check=False)
   self.assertEqual(r.returncode,0,r.stderr.decode());self.assertEqual(r.stderr.decode(),f'{COUNT} products\n')
   squares=0
   with inputs.open('rb') as source,outputs.open('rb') as target:
    for index in range(COUNT):
     record=source.read(64);a=int.from_bytes(record[:32],'big');b=int.from_bytes(record[32:],'big')
     expected=base.reference_field('mul',a,b).to_bytes(32,'big')
     if target.read(32)!=expected:self.fail(f'{label} product mismatch at seed20261028 index{index}')
     if index%256==0:
      if target.read(32)!=base.reference_field('mul',a,a).to_bytes(32,'big'):self.fail(f'{label} square mismatch at index{index}')
      squares+=1
    self.assertEqual(target.read(1),b'')
   self.report['modes'].append({'mode':label,'products':COUNT,'squares':squares,'elapsed_seconds':time.monotonic()-start,'generated_c_sha256':file_sha(self.dir/(label+'.c')),'driver_sha256':file_sha(driver),'output_sha256':file_sha(outputs),'output_bytes':outputs.stat().st_size,'failures':0})
 def test_raw_magnitude_and_carry_edges(self):
  values=[0,1,2,base.P-1,base.P-2,base.P-(2**32+977),base.P//2,2**128,2**256-1]
  raw=list(base.raw_cases(count=256));count=0
  for label,lib in self.libs:
   for a in values:
    for b in values:
     self.assertEqual(base.number(lib.sf_multiply(base.digits(a),base.digits(b))),base.reference_field('mul',a,b));count+=1
   for i,(mag,limbs) in enumerate(raw):
    x=base.Field((C.c_uint64*10)(*limbs));other=raw[(i*13+7)%len(raw)][1];y=base.Field((C.c_uint64*10)(*other))
    self.assertEqual(base.number(lib.sf_multiply(x,y)),base.number(x)*base.number(y)%base.P)
    self.assertEqual(base.number(lib.sf_square(x)),base.number(x)**2%base.P);count+=2
  self.report['raw_and_edge_comparisons']=count
 def test_exceeding_compile_time_bounds_fail_in_both_compiler_modes(self):
  cases=['BAD :: #run sf_bound_product(4294967296,4294967296);',
   'sf_invalid_bound_fixture :: () -> bool { b := SF_MUL_BOUNDS; b.facts[5] = CB_MAX64; return sf_mul_budget(b); }\n#assert #run sf_invalid_bound_fixture();']
  count=0
  for mode in [[],['--release']]:
   for i,source in enumerate(cases):
    path=self.dir/f'negative{i}.coax';path.write_text(f'#load "{base.SOURCE}";\n'+source+'\n')
    result=subprocess.run([str(base.COMPILER),'c',str(path),'--library',*mode],capture_output=True,text=True)
    self.assertNotEqual(result.returncode,0);self.assertIn('assertion failed',result.stderr);count+=1
  self.report['compile_time_refusals']=count
 def test_actual_wasm_product_and_square_fixtures(self):
  rng=random.Random(20261028);values=[0,1,base.P-1,base.P-(2**32+977),2**256-1]+[rng.getrandbits(256) for _ in range(256)]
  operations=[]
  for i,a in enumerate(values):
   b=values[(i*13+7)%len(values)]
   for op,bvalue in [('mul',b),('square',a)]:operations.append({'op':op,'a':f'{a:064x}','b':f'{bvalue:064x}','expected':f'{base.reference_field("mul",a,bvalue):064x}'})
  rows=list(base.raw_cases(count=128));raw=[]
  for i,(m,a) in enumerate(rows):
   b=rows[(i*7+11)%len(rows)][1];expected=sum(n<<(26*j) for j,n in enumerate(a))*sum(n<<(26*j) for j,n in enumerate(b))%base.P
   raw.append({'a':[str(n) for n in a],'b':[str(n) for n in b],'expected':f'{expected:064x}'})
  fixtures=self.dir/'mul-fixtures.json';fixtures.write_text(json.dumps({'operations':operations,'raw':[],'raw_products':raw}))
  reports=[]
  for label,lib in self.libs:
   wasm=self.dir/(label+'_mul.wasm')
   base.run('/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-I'+str(self.dir),self.dir/(label+'.c'),ROOT/'coax/tests/nostr/secp_field_wasm.c','-mexec-model=reactor','-Wl,--import-memory,--export-memory,--max-memory=67108864,--export=__heap_base','-lm','-o',wasm)
   report=json.loads(base.run('node',ROOT/'coax/tests/nostr/secp_field_wasm.cjs',wasm,fixtures).stdout)
   reports.append({'mode':label,'artifact_sha256':file_sha(wasm),'wasm_compiler':base.run('/opt/homebrew/opt/llvm/bin/clang','--version').stdout.splitlines()[0],**report})
  self.report['wasm']=reports
 def test_sanitized_raw_operands_and_actual_runtime_faults(self):
  probe=self.dir/'sanitized_mul.c';exe=self.dir/'sanitized_mul'
  probe.write_text('''#include "checked.c"
#include <assert.h>
int main(void){uint64_t seed=20261028;
 for(int t=0;t<8192;t++){coax_t_SecpField a={0},b={0};
  for(int i=0;i<10;i++){uint64_t limit=8*(i==9?UINT64_C(4194303):UINT64_C(67108863));
   seed=seed*UINT64_C(6364136223846793005)+1;a.limb.e[i]=seed%(limit+1);
   seed=seed*UINT64_C(6364136223846793005)+1;b.limb.e[i]=seed%(limit+1);}
  coax_t_SecpField x=sf_multiply(a,b),y=sf_multiply(b,a),s=sf_square(a);
  assert(sf_equal_mask(x,y)==UINT64_MAX);assert(sf_equal_mask(s,sf_multiply(a,a))==UINT64_MAX);
 }return 0;}''')
  base.run('cc','-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer',probe,'-lm','-o',exe);base.run(exe)
  original=(self.dir/'checked.c').read_text();match=re.search(r'coax_t_SecpField sf_multiply\([^;{}]*\)\s*\{',original);self.assertIsNotNone(match)
  begin=match.end();end=original.index('\n}',begin);body=original[begin:end]
  carry_match=re.search(r'_Bool (?:coax_p_)?sf_radix_carry\([^;{}]*\)\s*\{',original);self.assertIsNotNone(carry_match)
  carry_begin=carry_match.end();carry_end=original.index('\n}',carry_begin);carry_body=original[carry_begin:carry_end]
  self.assertIn('INT64_C(26)',carry_body);self.assertIn('UINT64_C(977)',body)
  faults={'lost_product_carry':original[:carry_begin]+carry_body.replace('INT64_C(26)','INT64_C(27)',1)+original[carry_end:],
   'wrong_final_fold':original[:begin]+body[:body.rfind('UINT64_C(977)')]+body[body.rfind('UINT64_C(977)'):].replace('UINT64_C(977)','UINT64_C(976)',1)+original[end:]}
  detected=[]
  for name,changed in faults.items():
   path=self.dir/(name+'.c');path.write_text(changed);libpath=self.dir/(name+'.dylib')
   base.run('cc','-std=c11','-O2','-shared','-fPIC',path,'-lm','-o',libpath)
   lib=C.CDLL(str(libpath));lib.sf_multiply.argtypes=[base.Field,base.Field];lib.sf_multiply.restype=base.Field
   a=base.P-1;b=base.P-(2**32+977)
   self.assertNotEqual(base.number(lib.sf_multiply(base.digits(a),base.digits(b))),base.reference_field('mul',a,b));detected.append(name)
  self.report['sanitized_raw_cases']=8192;self.report['detected_emitted_faults']=detected

if __name__=='__main__':unittest.main()
