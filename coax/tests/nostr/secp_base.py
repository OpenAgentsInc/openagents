#!/usr/bin/env python3
"""Fixed-base Coax correctness and build-budget evidence; test tooling only.

The two real tables must be evaluated by the compiler using the explicit
bounded build input. No runtime table generator is substituted for #run.
Public deterministic scalars, never identities/production key material.
"""
import ctypes as C
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import random
import re
import subprocess
import tempfile
import time
import unittest
import secp_field as base
import secp_group as group
from scalar_candidate import Scalar,scalar
from secp_field_mul import file_sha
from oracle import G,N,point_mul,NativeOracle,DEFAULT_LIBRARY

ROOT=base.ROOT
SOURCE=ROOT/'coax/tests/nostr/secp_base_adapter.coax'
COUNT=int(os.environ.get('COAX_BASE_COUNT','100000'))
if not 1<=COUNT<=100000:raise ValueError('random scalar budget is1..100000')
RUN_STEPS=250000000
RUN_ARGS=['--run-steps',str(RUN_STEPS)]
EDGES=sorted(set([0,1,2,3,7,8,15,16,17,N-2,N-1,N,N+1,2**256-1]+[1<<i for i in range(256)]+[(1<<i)-1 for i in range(1,256)]))

class FixedBase(unittest.TestCase):
 @classmethod
 def setUpClass(cls):
  cls.temp=tempfile.TemporaryDirectory(prefix='coax-secp-base-');cls.dir=Path(cls.temp.name);cls.libs=[];cls.start=time.monotonic()
  cls.native=NativeOracle(DEFAULT_LIBRARY)
  cls.parse=cls.native.lib.secp256k1_ec_pubkey_parse;cls.parse.argtypes=[C.c_void_p,C.c_void_p,C.c_void_p,C.c_size_t];cls.parse.restype=C.c_int
  cls.serialize=cls.native.lib.secp256k1_ec_pubkey_serialize;cls.serialize.argtypes=[C.c_void_p,C.c_void_p,C.POINTER(C.c_size_t),C.c_void_p,C.c_uint];cls.serialize.restype=C.c_int
  paths=[ROOT/'coax/lib/crypto'/name for name in ['secp_mult_base.coax','secp_mult_base_web.coax','secp_mult_base_native.coax','secp_group.coax','secp_field.coax','secp_scalar.coax','bytes.coax']]
  paths += [SOURCE,Path(__file__)]+[ROOT/'coax/tests/nostr'/name for name in ['secp_base_stream.c','secp_base_wasm.c','secp_base_wasm.cjs','secp_base_web_probe.coax','secp_base_native_probe.coax','secp_field.py','secp_field_mul.py','secp_group.py','scalar_candidate.py']]
  paths += [ROOT/'tools/coax-nostr/fuzz'/name for name in ['oracle.py','UPSTREAM.json']]
  cls.report={'schema':'coax.secp-fixed-base.v1','scope':'fixed-work/full-scan arithmetic correctness and explicitly bounded actual compile-time table generation; not discharged authored guards, object/JIT constant-time, independent security review, erasure or production-secret approval','seed':20261029,'compiler':base.run(base.COMPILER,'version').stdout.strip(),'compiler_sha256':file_sha(base.COMPILER),'native_compiler':base.run('cc','--version').stdout.splitlines()[0],'native_target':base.run('cc','-dumpmachine').stdout.strip(),'build_inputs':{'run_steps':RUN_STEPS,'default_run_steps':100000,'maximum_run_steps':250000000,'command_suffix':RUN_ARGS},'budgets':{'random_scalars_per_profile':COUNT,'maximum_random_scalars_per_profile':100000,'source_steps_per_check':20000000,'maximum_temporary_bytes':268435456,'timeout_seconds_per_profile':1800},'tables':{'web':{'width':7,'spacing':37,'entries':128,'point_bytes':240,'table_bytes':30720,'cap_bytes':32768,'generation_probe_steps_including_assertions':63698174},'native':{'width':8,'spacing':32,'entries':256,'point_bytes':240,'table_bytes':61440,'cap_bytes':65536,'generation_probe_steps_including_assertions':103614101}},'sources':{str(p.relative_to(ROOT)):file_sha(p) for p in paths},'libsecp_artifact_sha256':file_sha(DEFAULT_LIBRARY),'libsecp_source_commit':json.loads((ROOT/'tools/coax-nostr/fuzz/UPSTREAM.json').read_text())['commit'],'modes':[]}
  for label,mode in [('checked',[]),('release',['--release'])]:
   c=cls.dir/(label+'.c');timings=cls.dir/(label+'-timings.json');c.write_text(base.run(base.COMPILER,'c',SOURCE,'--library','--header',cls.dir/'secp_base.h','--timings',timings,*mode,*RUN_ARGS).stdout)
   measurements=json.loads(timings.read_text())
   if (measurements.get('run_steps')!=RUN_STEPS or not 0<measurements.get('evaluated_steps',0)<=RUN_STEPS or
       measurements.get('aggregate_peak_slots',0)<=0 or measurements.get('aggregate_peak_containers',0)<=0):
    raise AssertionError('missing or inconsistent actual compile-time resource measurements')
   libpath=cls.dir/(label+'.dylib');base.run('cc','-std=c11','-O2','-shared','-fPIC',c,'-lm','-o',libpath);lib=C.CDLL(str(libpath))
   for variant in ['web','native']:getattr(lib,'smb_multiply_'+variant).argtypes=[Scalar];getattr(lib,'smb_multiply_'+variant).restype=group.Point
   lib.smb_table_entry_public.argtypes=[C.c_bool,C.c_int64];lib.smb_table_entry_public.restype=group.Point
   stream=cls.dir/(label+'_stream');base.run('cc','-std=c11','-O2','-I'+str(cls.dir),c,ROOT/'coax/tests/nostr/secp_base_stream.c','-lm','-o',stream)
   obj=cls.dir/(label+'.o');base.run('cc','-std=c11','-O2','-fstack-usage','-c',c,'-o',obj)
   stack=cls.dir/(label+'.su');stack_text=stack.read_text();rows=[]
   for line in stack_text.splitlines():
    if 'smb_multiply_' in line or 'smb_comb' in line:rows.append(line)
   if not rows:raise AssertionError('missing actual stack-usage records')
   cls.libs.append((label,lib));cls.report['modes'].append({'mode':label,'optimization':'O2','actual_run_tables':True,'compiler_timings':measurements,'generated_c_bytes':c.stat().st_size,'generated_c_sha256':file_sha(c),'artifact_sha256':file_sha(libpath),'stream_artifact_sha256':file_sha(stream),'object_bytes':obj.stat().st_size,'object_sections':base.run('size','-m',obj).stdout,'object_sha256':file_sha(obj),'stack_usage_records':rows,'stack_report_sha256':file_sha(stack)})
 @classmethod
 def tearDownClass(cls):
  cls.native.close();cls.report['temporary_bytes']=sum(p.stat().st_size for p in cls.dir.rglob('*') if p.is_file());cls.report['elapsed_seconds']=time.monotonic()-cls.start
  if os.environ.get('COAX_BASE_REPORT'):Path(os.environ['COAX_BASE_REPORT']).write_text(json.dumps(cls.report,indent=2,sort_keys=True)+'\n')
  if cls.report['temporary_bytes']>cls.report['budgets']['maximum_temporary_bytes']:raise AssertionError('temporary budget exceeded')
  cls.temp.cleanup()
 @classmethod
 def native_point(cls,k):
  if k%N==0:return None
  encoded=cls.native.point((k%N).to_bytes(32,'big'));pub=(C.c_uint8*64)()
  if encoded is None or cls.parse(cls.native.context,pub,C.c_char_p(encoded),len(encoded))!=1:raise ValueError('public oracle point creation')
  out=(C.c_uint8*65)();n=C.c_size_t(65)
  if cls.serialize(cls.native.context,out,C.byref(n),pub,2)!=1 or n.value!=65:raise ValueError('public oracle serialization')
  return int.from_bytes(bytes(out)[1:33],'big'),int.from_bytes(bytes(out)[33:],'big')
 def assert_point(self,value,wanted):
  f=[base.number(v) for v in [value.x,value.y,value.z]]
  for v in [value.x,value.y,value.z]:
   self.assertLess(base.number(v),base.P)
   for i,limb in enumerate(v.limb):self.assertLessEqual(limb,base.M22 if i==9 else base.M26)
  if wanted is None:self.assertEqual(f,[1,1,0]);return
  x,y,z=f;self.assertNotEqual(z,0);self.assertEqual(x,wanted[0]*z*z%base.P);self.assertEqual(y,wanted[1]*z*z*z%base.P)
 def test_a_actual_generated_tables_and_scalar_boundaries(self):
  comparisons=0
  for label,lib in self.libs:
   for variant,width,spacing in [('web',7,37),('native',8,32)]:
    for index in range(1<<width):
     coefficient=sum(1<<(tooth*spacing) for tooth in range(width) if index>>tooth&1)
     self.assert_point(lib.smb_table_entry_public(variant=='native',index),self.native_point(coefficient));comparisons+=1
    for index in [-2**63,-1,1<<width,2**63-1]:self.assert_point(lib.smb_table_entry_public(variant=='native',index),None)
    for k in EDGES:self.assert_point(getattr(lib,'smb_multiply_'+variant)(scalar(k)),self.native_point(k))
  self.report['table_entries_compared_to_independent_oracle']=comparisons;self.report['boundary_scalars_per_variant_mode']=len(EDGES)
 def test_b_hundred_thousand_scalars_on_both_native_profiles(self):
  inputs=self.dir/'scalars.bin';expected=self.dir/'expected.bin';rng=random.Random(20261029);start=time.monotonic()
  with inputs.open('wb') as stream,expected.open('wb') as oracle:
   for i in range(COUNT):
    k=rng.getrandbits(256);want=self.native_point(k)
    if i<32:self.assertEqual(want,point_mul(k%N))
    stream.write(k.to_bytes(32,'big'));oracle.write(b'\0'+bytes(64) if want is None else b'\1'+want[0].to_bytes(32,'big')+want[1].to_bytes(32,'big'))
  self.report.update({'input_sha256':file_sha(inputs),'expected_sha256':file_sha(expected),'independent_affine_cross_checks':min(32,COUNT),'oracle_generation_seconds':time.monotonic()-start})
  def run_profile(row):
   mode,index,variant=row;outputs=self.dir/(mode+'_'+variant+'.bin');start=time.monotonic()
   with inputs.open('rb') as stream,outputs.open('wb') as target:r=subprocess.run([str(self.dir/(mode+'_stream')),str(index)],stdin=stream,stdout=target,stderr=subprocess.PIPE,timeout=1800,check=False)
   self.assertEqual(r.returncode,0,r.stderr.decode());self.assertEqual(r.stderr.decode(),f'{COUNT} fixed-base multiplications\n');self.assertEqual(outputs.stat().st_size,96*COUNT)
   with expected.open('rb') as oracle,outputs.open('rb') as target:
    for i in range(COUNT):
     want=oracle.read(65);got=target.read(96);x,y,z=[int.from_bytes(got[j:j+32],'big') for j in [0,32,64]]
     self.assertLess(x,base.P);self.assertLess(y,base.P);self.assertLess(z,base.P)
     if want[0]==0:self.assertEqual((x,y,z),(1,1,0),(mode,variant,i))
     else:
      ex=int.from_bytes(want[1:33],'big');ey=int.from_bytes(want[33:],'big');self.assertNotEqual(z,0,(mode,variant,i));self.assertEqual(x,ex*z*z%base.P,(mode,variant,i));self.assertEqual(y,ey*z*z*z%base.P,(mode,variant,i))
    self.assertEqual(target.read(1),b'')
   return {'mode':mode,'variant':variant,'scalars':COUNT,'output_sha256':file_sha(outputs),'elapsed_seconds':time.monotonic()-start,'failures':0}
  with ThreadPoolExecutor(max_workers=2) as pool:self.report['native_random']=list(pool.map(run_profile,[(mode,index,variant) for mode,_ in self.libs for index,variant in enumerate(['web','native'])]))
 def test_c_actual_wasm_both_profiles_and_guard_bytes(self):
  rng=random.Random(20261029);values=EDGES+[rng.getrandbits(256) for _ in range(COUNT)];rows=[]
  for k in values:
   want=self.native_point(k);rows.append({'scalar':f'{k:064x}','expected':None if want is None else [f'{v:064x}' for v in want]})
  fixtures=self.dir/'wasm.json';fixtures.write_text(json.dumps(rows,separators=(',',':')));reports=[]
  for label,_ in self.libs:
   wasm=self.dir/(label+'.wasm');base.run('/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-I'+str(self.dir),self.dir/(label+'.c'),ROOT/'coax/tests/nostr/secp_base_wasm.c','-mexec-model=reactor','-Wl,--import-memory,--export-memory,--max-memory=67108864,--export=__heap_base,-z,stack-size=1048576','-lm','-o',wasm)
   reports.append({'mode':label,'stack_bytes':1048576,'artifact_sha256':file_sha(wasm),'wasm_compiler':base.run('/opt/homebrew/opt/llvm/bin/clang','--version').stdout.splitlines()[0],**json.loads(base.run('node',ROOT/'coax/tests/nostr/secp_base_wasm.cjs',wasm,fixtures,timeout=1800).stdout)})
  self.report['wasm']=reports
 def test_d_sanitizers_truncation_and_frozen_public_module(self):
  probe=self.dir/'sanitized.c';probe.write_text('#include "checked.c"\n#include <assert.h>\nint main(void){_Static_assert(sizeof(coax_t_SgPoint)==240,"point layout");coax_t_ScScalar s={0};for(unsigned i=0;i<256;i++){s.limbs.e[i/32]=UINT32_C(1)<<(i%32);coax_t_SgPoint a=smb_multiply_web(s),b=smb_multiply_native(s);assert(sg_valid_mask(a)==UINT64_MAX&&sg_equal_mask(a,b)==UINT64_MAX);s.limbs.e[i/32]=0;}return0;}\n'.replace('return0','return 0'))
  exe=self.dir/'sanitized';base.run('cc','-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer',probe,'-lm','-o',exe);base.run(exe,timeout=1800)
  for label,_ in self.libs:
   result=subprocess.run([str(self.dir/(label+'_stream')),'0'],input=bytes(31),capture_output=True);self.assertEqual(result.returncode,5);self.assertEqual(result.stdout,b'')
  # The public variable-time module is a separate verification dependency.
  self.report['separate_public_mult_sha256']=file_sha(ROOT/'coax/lib/crypto/secp_mult.coax');self.report['sanitizer_single_bits']=256;self.report['truncated_stream_refusals']=2
 def test_e_no_secret_index_or_branch_in_online_source(self):
  source=(ROOT/'coax/lib/crypto/secp_mult_base.coax').read_text();body=source.split('smb_comb ::',1)[1]
  self.assertNotRegex(body,r'\bif\b');self.assertIn('smb_pick_generated(table,smb_index(',body);self.assertNotIn('table[',body)
  self.assertNotIn('secp_mult.coax',source);self.report['structural_only']='online helper has public loop bounds and full-scan group selection; this is source inspection, not guard discharge, object timing or production admission'

if __name__=='__main__':unittest.main()
