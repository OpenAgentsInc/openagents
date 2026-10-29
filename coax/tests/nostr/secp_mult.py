#!/usr/bin/env python3
"""Public verification multiplication: pinned libsecp, bigint, native/Wasm.

Test-only effects, deterministic public scalars, bounded streamed corpora.
Never a production key handler or independent security approval.
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
from oracle import G,N,point_add,point_mul,NativeOracle,DEFAULT_LIBRARY

ROOT=base.ROOT
SOURCE=ROOT/'coax/lib/crypto/secp_mult.coax'
COUNT=int(os.environ.get('COAX_MULT_COUNT','100000'))
if not 1<=COUNT<=100000:raise ValueError('random triple budget is1..100000')
class Wnaf(C.Structure):_fields_=[('digits',C.c_int64*257),('length',C.c_int64),('ok',C.c_bool)]
def wnaf(value,width):
 if not 2<=width<=5:return [0]*257,0,False
 n=value;digits=[]
 while n:
  digit=n%(1<<width) if n&1 else 0
  if digit>=(1<<(width-1)):digit-=1<<width
  digits.append(digit);n=(n-digit)//2
 return digits+[0]*(257-len(digits)),len(digits),True

def suffix_policy(source):
 names=re.findall(r'^(\w+)\s*::\s*\([^\n]*\)\s*->',source,re.M)
 return bool(names) and all(name.endswith('_vartime') for name in names)

def function_body(source,name):
 m=re.search(r'\b(?:coax_p_)?'+name+r'\([^;{}]*\)\s*\{',source)
 if not m:raise ValueError('missing emitted '+name)
 start=m.end();end=source.index('\n}',start);return start,end,source[start:end]

class Mult(unittest.TestCase):
 @classmethod
 def setUpClass(cls):
  cls.temp=tempfile.TemporaryDirectory(prefix='coax-secp-mult-');cls.dir=Path(cls.temp.name);cls.libs=[];cls.start=time.monotonic()
  cls.native=NativeOracle(DEFAULT_LIBRARY)
  for name,args in [('secp256k1_ec_pubkey_parse',[C.c_void_p,C.c_void_p,C.c_void_p,C.c_size_t]),('secp256k1_ec_pubkey_serialize',[C.c_void_p,C.c_void_p,C.POINTER(C.c_size_t),C.c_void_p,C.c_uint]),('secp256k1_ec_pubkey_combine',[C.c_void_p,C.c_void_p,C.POINTER(C.c_void_p),C.c_size_t]),('secp256k1_ec_pubkey_tweak_mul',[C.c_void_p,C.c_void_p,C.c_void_p])]:
   fn=getattr(cls.native.lib,name);fn.argtypes=args;fn.restype=C.c_int
  cls.parse=cls.native.lib.secp256k1_ec_pubkey_parse;cls.serialize=cls.native.lib.secp256k1_ec_pubkey_serialize;cls.combine=cls.native.lib.secp256k1_ec_pubkey_combine;cls.tweak=cls.native.lib.secp256k1_ec_pubkey_tweak_mul
  paths=[SOURCE,ROOT/'coax/lib/crypto/secp_group.coax',ROOT/'coax/lib/crypto/secp_field.coax',ROOT/'coax/lib/crypto/secp_scalar.coax',ROOT/'coax/lib/crypto/bytes.coax',Path(__file__),ROOT/'coax/tests/nostr/secp_mult_stream.c',ROOT/'coax/tests/nostr/secp_mult_wasm.c',ROOT/'coax/tests/nostr/secp_mult_wasm.cjs',ROOT/'coax/tests/nostr/secp_field.py',ROOT/'coax/tests/nostr/secp_field_mul.py',ROOT/'coax/tests/nostr/secp_group.py',ROOT/'coax/tests/nostr/scalar_candidate.py',ROOT/'tools/coax-nostr/fuzz/oracle.py',ROOT/'tools/coax-nostr/fuzz/UPSTREAM.json']
  cls.report={'schema':'coax.secp-public-mult.v1','scope':'public-only variable-time verification arithmetic correctness; no secret signing/ECDH, constant-time, device, performance or security approval claim','seed':20261029,'compiler':base.run(base.COMPILER,'version').stdout.strip(),'compiler_sha256':file_sha(base.COMPILER),'native_compiler':base.run('cc','--version').stdout.splitlines()[0],'native_target':base.run('cc','-dumpmachine').stdout.strip(),'budgets':{'random_triples_per_mode':COUNT,'maximum_triples_per_mode':100000,'stream_record_bytes':160,'stream_output_bytes':97,'max_temporary_bytes':100663296,'timeout_seconds_per_profile':1200,'source_steps_per_check':20000000,'compile_time_aggregate_steps':100000,'scalar_bits':257,'point_window':5,'point_table':8,'generator_window':2,'generator_signed_table':2},'sources':{str(p.relative_to(ROOT)):file_sha(p) for p in paths},'libsecp_artifact_sha256':file_sha(DEFAULT_LIBRARY),'libsecp_source_commit':json.loads((ROOT/'tools/coax-nostr/fuzz/UPSTREAM.json').read_text())['commit'],'modes':[]}
  for label,mode in [('checked',[]),('release',['--release'])]:
   c=cls.dir/(label+'.c');c.write_text(base.run(base.COMPILER,'c',SOURCE,'--library','--header',cls.dir/'secp_mult.h',*mode).stdout)
   libpath=cls.dir/(label+'.dylib');base.run('cc','-std=c11','-O2','-shared','-fPIC',c,'-lm','-o',libpath);lib=C.CDLL(str(libpath))
   lib.ecmult_vartime.argtypes=[Scalar,group.Point,Scalar];lib.ecmult_vartime.restype=group.Result
   lib.sm_wnaf_vartime.argtypes=[Scalar,C.c_int64];lib.sm_wnaf_vartime.restype=Wnaf
   stream=cls.dir/(label+'_stream');base.run('cc','-std=c11','-O2','-I'+str(cls.dir),c,ROOT/'coax/tests/nostr/secp_mult_stream.c','-lm','-o',stream)
   cls.libs.append((label,lib));cls.report['modes'].append({'mode':label,'generated_c_sha256':file_sha(c),'artifact_sha256':file_sha(libpath),'stream_artifact_sha256':file_sha(stream)})
 @classmethod
 def tearDownClass(cls):
  cls.native.close();cls.report['temporary_bytes']=sum(p.stat().st_size for p in cls.dir.rglob('*') if p.is_file());cls.report['elapsed_seconds']=time.monotonic()-cls.start
  if cls.report['temporary_bytes']>cls.report['budgets']['max_temporary_bytes']:raise AssertionError('temporary budget exceeded')
  if os.environ.get('COAX_MULT_REPORT'):Path(os.environ['COAX_MULT_REPORT']).write_text(json.dumps(cls.report,indent=2,sort_keys=True)+'\n')
  cls.temp.cleanup()
 @classmethod
 def parse_point(cls,p):
  pub=(C.c_uint8*64)();data=group.compressed(p)
  if cls.parse(cls.native.context,pub,C.c_char_p(data),len(data))!=1:raise ValueError('public point parse')
  return pub
 @classmethod
 def affine(cls,pub):
  out=(C.c_uint8*65)();n=C.c_size_t(65)
  if cls.serialize(cls.native.context,out,C.byref(n),pub,2)!=1 or n.value!=65:raise ValueError('public point serialize')
  return int.from_bytes(bytes(out)[1:33],'big'),int.from_bytes(bytes(out)[33:],'big')
 @classmethod
 def native_point(cls,k):
  if k%N==0:return None
  encoded=cls.native.point((k%N).to_bytes(32,'big'));pub=(C.c_uint8*64)()
  if encoded is None or cls.parse(cls.native.context,pub,C.c_char_p(encoded),len(encoded))!=1:raise ValueError('public generator multiple')
  return cls.affine(pub)
 @classmethod
 def native_result(cls,a,p,b):
  a%=N;b%=N;pubs=[]
  if a:pubs.append(cls.parse_point(cls.native_point(a)))
  if b and p is not None:
   pub=cls.parse_point(p)
   if cls.tweak(cls.native.context,pub,C.c_char_p(b.to_bytes(32,'big')))!=1:raise ValueError('public tweak multiplication')
   pubs.append(pub)
  if not pubs:return None
  if len(pubs)==1:return cls.affine(pubs[0])
  pointers=(C.c_void_p*len(pubs))(*(C.cast(p,C.c_void_p).value for p in pubs));out=(C.c_uint8*64)()
  return cls.affine(out) if cls.combine(cls.native.context,out,pointers,len(pubs)) else None
 def assert_point(self,value,wanted):
  f=[base.number(v) for v in [value.x,value.y,value.z]]
  for v in [value.x,value.y,value.z]:
   self.assertLess(base.number(v),base.P)
   for i,limb in enumerate(v.limb):self.assertLessEqual(limb,base.M22 if i==9 else base.M26)
  if wanted is None:self.assertEqual(f,[1,1,0]);return
  x,y,z=f;self.assertNotEqual(z,0);self.assertEqual(x,wanted[0]*z*z%base.P);self.assertEqual(y,wanted[1]*z*z*z%base.P)
 def test_a_seeded_hundred_thousand_public_triples_against_pinned_libsecp(self):
  inputs=self.dir/'triples.bin';expected=self.dir/'expected.bin';rng=random.Random(20261029);start=time.monotonic();independent=0
  with inputs.open('wb') as stream,expected.open('wb') as oracle:
   for i in range(COUNT):
    a=rng.getrandbits(256);b=rng.getrandbits(256);p=self.native_point(rng.randrange(1,N));z=rng.randrange(1,base.P);wanted=self.native_result(a,p,b)
    if i<32:self.assertEqual(wanted,point_add(point_mul(a%N),point_mul(b%N,p)));independent+=1
    stream.write(a.to_bytes(32,'big')+b.to_bytes(32,'big')+(p[0]*z*z%base.P).to_bytes(32,'big')+(p[1]*z*z*z%base.P).to_bytes(32,'big')+z.to_bytes(32,'big'))
    oracle.write(b'\0'+bytes(64) if wanted is None else b'\1'+wanted[0].to_bytes(32,'big')+wanted[1].to_bytes(32,'big'))
  self.assertEqual(inputs.stat().st_size,160*COUNT);self.assertEqual(expected.stat().st_size,65*COUNT)
  self.report.update({'stream_sha256':file_sha(inputs),'expected_sha256':file_sha(expected),'independent_affine_cross_checks':independent,'oracle_generation_seconds':time.monotonic()-start})
  def run_profile(pair):
   index,(label,lib)=pair
   outputs=self.dir/(label+'_results.bin');start=time.monotonic()
   with inputs.open('rb') as stream,outputs.open('wb') as target:r=subprocess.run([str(self.dir/(label+'_stream'))],stdin=stream,stdout=target,stderr=subprocess.PIPE,timeout=1200,check=False)
   self.assertEqual(r.returncode,0,r.stderr.decode());self.assertEqual(r.stderr.decode(),f'{COUNT} public dual multiplications\n');self.assertEqual(outputs.stat().st_size,97*COUNT)
   with expected.open('rb') as oracle,outputs.open('rb') as target:
    for i in range(COUNT):
     want=oracle.read(65);got=target.read(97);self.assertEqual(got[0],1,(label,i));x,y,z=[int.from_bytes(got[j:j+32],'big') for j in [1,33,65]]
     self.assertLess(x,base.P);self.assertLess(y,base.P);self.assertLess(z,base.P)
     if want[0]==0:self.assertEqual((x,y,z),(1,1,0),(label,i))
     else:
      ex=int.from_bytes(want[1:33],'big');ey=int.from_bytes(want[33:],'big');self.assertNotEqual(z,0,(label,i));self.assertEqual(x,ex*z*z%base.P,(label,i));self.assertEqual(y,ey*z*z*z%base.P,(label,i))
    self.assertEqual(target.read(1),b'')
   self.report['modes'][index].update({'triples':COUNT,'output_sha256':file_sha(outputs),'elapsed_seconds':time.monotonic()-start,'failures':0})
  with ThreadPoolExecutor(max_workers=2) as pool:list(pool.map(run_profile,enumerate(self.libs)))
 def test_public_edges_infinity_opposites_aliases_and_invalid_points(self):
  cases=[];values=[0,1,2,3,15,16,17,31,32,255,2**255,N-1,N,2**256-1]
  for a in values:
   for b in [0,1,17,N-1,N]:
    for p in [G,None]:cases.append((a,p,b))
  cases.extend([(1,G,N-1),(N-1,G,1),(3,(G[0],base.P-G[1]),3)])
  for label,lib in self.libs:
   for a,p,b in cases:
    result=lib.ecmult_vartime(scalar(a),group.point(p,7),scalar(b));self.assertTrue(result.ok);self.assert_point(result.value,self.native_result(a,p,b))
   alias=group.point(G)
   for f in [alias.x,alias.y,alias.z]:
    for i,n in enumerate(base.digits(base.P).limb):f.limb[i]+=n
   result=lib.ecmult_vartime(scalar(3),alias,scalar(17));self.assertTrue(result.ok);self.assert_point(result.value,self.native_point(20))
   for p in [group.Point(),group.Point(base.digits(1),base.digits(0),base.digits(1)),group.Point(base.digits(0),base.digits(1),base.digits(0))]:
    for a,b in [(0,0),(1,1),(N-1,N-1)]:
     result=lib.ecmult_vartime(scalar(a),p,scalar(b));self.assertFalse(result.ok);self.assert_point(result.value,None)
  self.report['edge_triples_per_mode']=len(cases)+10
 def test_recoding_against_independent_bigint_for_all_bits_windows_and_carries(self):
  rng=random.Random(2575);values=[0,1,2,3,15,16,17,N-1,N,2**256-1]+[1<<i for i in range(256)]+[(1<<(word*32+3))-1 for word in range(1,8)]+[rng.getrandbits(256) for _ in range(1024)];comparisons=0
  for label,lib in self.libs:
   for width in range(2,6):
    for value in values:
     expected,length,ok=wnaf(value,width);actual=lib.sm_wnaf_vartime(scalar(value),width)
     self.assertEqual((actual.ok,actual.length),(ok,length));self.assertEqual(list(actual.digits),expected);comparisons+=1
   for width in [-2**63,-1,0,1,6,2**63-1]:
    actual=lib.sm_wnaf_vartime(scalar(1),width);self.assertFalse(actual.ok);self.assertEqual(actual.length,0);self.assertEqual(list(actual.digits),[0]*257)
  self.report['recoding_comparisons']=comparisons;self.report['recoding_refusals']=12
 def test_real_wasm_edge_random_multiply_and_recoding(self):
  rng=random.Random(5757);rows=[]
  for a,b,p in [(0,0,G),(1,N-1,G),(N-1,1,G),(3,17,None),(N,N,G),(2**256-1,2**256-1,G)]+[(rng.getrandbits(256),rng.getrandbits(256),self.native_point(rng.randrange(1,N))) for _ in range(64)]:
   wanted=self.native_result(a,p,b);raw=group.point(p,rng.randrange(1,base.P));rows.append({'a':f'{a:064x}','b':f'{b:064x}','point':[str(v) for f in [raw.x,raw.y,raw.z] for v in f.limb],'expected':None if wanted is None else [f'{v:064x}' for v in wanted],'ok':True})
  for raw in [group.Point(),group.Point(base.digits(1),base.digits(0),base.digits(1))]:rows.append({'a':f'{1:064x}','b':f'{1:064x}','point':[str(v) for f in [raw.x,raw.y,raw.z] for v in f.limb],'expected':None,'ok':False})
  recodings=[]
  for width in [1,2,3,4,5,6]:
   for value in [0,1,17,2**31,2**32,2**255,N-1,N,2**256-1]:
    digits,length,ok=wnaf(value,width);recodings.append({'scalar':f'{value:064x}','width':width,'length':length if ok else -1,'digits':digits})
  fixtures=self.dir/'wasm.json';fixtures.write_text(json.dumps({'operations':rows,'recodings':recodings}));reports=[]
  for label,lib in self.libs:
   wasm=self.dir/(label+'.wasm');base.run('/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-I'+str(self.dir),self.dir/(label+'.c'),ROOT/'coax/tests/nostr/secp_mult_wasm.c','-mexec-model=reactor','-Wl,--import-memory,--export-memory,--max-memory=67108864,--export=__heap_base','-lm','-o',wasm)
   reports.append({'mode':label,'artifact_sha256':file_sha(wasm),'wasm_compiler':base.run('/opt/homebrew/opt/llvm/bin/clang','--version').stdout.splitlines()[0],**json.loads(base.run('node',ROOT/'coax/tests/nostr/secp_mult_wasm.cjs',wasm,fixtures).stdout)})
  self.report['wasm']=reports
 def test_sanitizers_stream_refusal_and_actual_fault_detection(self):
  probe=self.dir/'sanitized.c';probe.write_text('''#include "checked.c"
#include <assert.h>
int main(void){coax_t_ScScalar a={0},b={0};coax_t_SgPoint g=sg_generator();
for(int i=0;i<24;i++){a.limbs.e[i%8]=UINT32_MAX;b.limbs.e[(i+3)%8]=1;coax_t_SgResult r=ecmult_vartime(a,g,b);assert(r.ok);assert(sg_valid_mask(r.value)==UINT64_MAX);}
coax_t_SgResult bad=ecmult_vartime(a,(coax_t_SgPoint){0},b);assert(!bad.ok);assert(sg_equal_mask(bad.value,sg_infinity())==UINT64_MAX);
assert(!sm_wnaf_vartime(a,INT64_MIN).ok);assert(!sm_wnaf_vartime(a,INT64_MAX).ok);return 0;}''')
  exe=self.dir/'sanitized';base.run('cc','-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer',probe,'-lm','-o',exe);base.run(exe)
  for label,lib in self.libs:
   result=subprocess.run([str(self.dir/(label+'_stream'))],input=bytes(159),capture_output=True);self.assertEqual(result.returncode,5);self.assertEqual(result.stdout,b'')
  source=(self.dir/'checked.c').read_text();start,end,body=function_body(source,'ecmult_vartime');self.assertIn('l_pd',body);self.assertIn('l_gd',body)
  faults={}
  faults['missing_generator_term']=source[:start]+body.replace('l_gd != INT64_C(0)','l_gd != l_gd',1)+source[end:]
  faults['missing_point_term']=source[:start]+body.replace('l_pd != INT64_C(0)','l_pd != l_pd',1)+source[end:]
  begin,finish,wbody=function_body(source,'sm_wnaf_vartime');self.assertIn('INT64_C(32)',wbody)
  faults['lost_word_shift_carry']=source[:begin]+wbody.replace('INT64_C(31)','INT64_C(30)',1)+source[finish:]
  stale,count=re.subn(r'l_carry = coax_shr_u64\(l_wide, [^;]+;', 'l_carry = l_carry;', wbody,count=1);self.assertEqual(count,1)
  faults['stale_propagated_carry']=source[:begin]+stale+source[finish:]
  detected=[]
  for name,changed in faults.items():
   self.assertNotEqual(changed,source,name);path=self.dir/(name+'.c');path.write_text(changed);artifact=self.dir/(name+'.dylib');base.run('cc','-std=c11','-O2','-shared','-fPIC',path,'-lm','-o',artifact);lib=C.CDLL(str(artifact));lib.ecmult_vartime.argtypes=[Scalar,group.Point,Scalar];lib.ecmult_vartime.restype=group.Result
   bad=False
   for a,b in [(1,1),(3,17),(2**64+1,2**32+17),(2**35-1,17)]:
    result=lib.ecmult_vartime(scalar(a),group.point(G),scalar(b));want=self.native_point((a+b)%N);x,y,z=[base.number(f) for f in [result.value.x,result.value.y,result.value.z]]
    if not result.ok or not z or x!=want[0]*z*z%base.P or y!=want[1]*z*z*z%base.P:bad=True;break
   self.assertTrue(bad,name);detected.append(name)
  self.report['actual_runtime_faults_detected']=detected;self.report['sanitizer_triples']=24;self.report['truncated_stream_refusals']=2
 def test_scope_naming_and_real_compile_time_generator_table(self):
  source=SOURCE.read_text();self.assertTrue(suffix_policy(source));self.assertFalse(suffix_policy(source.replace('sm_wnaf_vartime ::','sm_wnaf ::')))
  self.assertIn('SM_G_TABLE :: #run sm_generator_table_vartime();',source);self.assertIn('SM_G_WINDOW :: 2;',source);self.assertIn('SM_P_WINDOW :: 5;',source)
  # Both real compilation profiles above execute the unchanged aggregate100k
  # #run gate. No increased step limit, pre-generated math or fixture generator.
  self.report['suffix_policy']='all new procedures carry _vartime; names are documentation only, not general secret-taint enforcement';self.report['compile_time_generator_table']='two signed width2 entries generated by #run in both modes under default aggregate100k steps'

if __name__=='__main__':unittest.main()
