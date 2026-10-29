#!/usr/bin/env python3
"""Group correctness against independent affine math and pinned public libsecp.

Test-only native/Wasm adapters. No production dependency or private identity.
"""
import ctypes as C
import hashlib
import json
import os
from pathlib import Path
import random
import re
import subprocess
import tempfile
import unittest
import secp_field as base
from secp_field_mul import file_sha
from oracle import G,N,point_add,point_mul,lift_x,NativeOracle,DEFAULT_LIBRARY

ROOT=base.ROOT
SOURCE=ROOT/'coax/lib/crypto/secp_group.coax'
COUNT=int(os.environ.get('COAX_GROUP_COUNT','1024'))
if not 1<=COUNT<=1024:raise ValueError('random pair budget is1..1024')
class Point(C.Structure):_fields_=[('x',base.Field),('y',base.Field),('z',base.Field)]
class Affine(C.Structure):_fields_=[('x',base.Field),('y',base.Field),('infinity',C.c_uint64)]
class Result(C.Structure):_fields_=[('value',Point),('ok',C.c_bool)]
class AffineResult(C.Structure):_fields_=[('value',Affine),('ok',C.c_bool)]
class Encoded(C.Structure):_fields_=[('bytes',C.c_uint8*33),('length',C.c_int64),('ok',C.c_bool)]
class PointView(C.Structure):_fields_=[('data',C.POINTER(Point)),('count',C.c_int64)]
def point(value,z=1):
 if value is None:return Point(base.digits(1),base.digits(1),base.digits(0))
 x,y=value;return Point(base.digits(x*z*z%base.P),base.digits(y*z*z*z%base.P),base.digits(z))
def affine(value):
 if value is None:return Affine(base.Field(),base.Field(),base.MAX64)
 return Affine(base.digits(value[0]),base.digits(value[1]),0)
def compressed(value):return b'\0' if value is None else bytes([2+(value[1]&1)])+value[0].to_bytes(32,'big')
def body_of(source,name):
 match=re.search(r'(?:coax_t_SgResult|coax_t_SgPoint|uint64_t) (?:coax_p_)?'+name+r'\([^;{}]*\)\s*\{',source)
 if not match:raise ValueError('missing emitted function '+name)
 start=match.end();end=source.index('\n}',start);return start,end,source[start:end]
def unconditional_body(source,name):
 _,_,body=body_of(source,name)
 return re.search(r'\b(if|switch|while)\s*\(',body) is None and '?' not in body

class Group(unittest.TestCase):
 @classmethod
 def setUpClass(cls):
  cls.temp=tempfile.TemporaryDirectory(prefix='coax-secp-group-');cls.dir=Path(cls.temp.name);cls.libs=[]
  cls.native=NativeOracle(DEFAULT_LIBRARY)
  cls.parse=cls.native.lib.secp256k1_ec_pubkey_parse;cls.parse.argtypes=[C.c_void_p,C.c_void_p,C.c_void_p,C.c_size_t];cls.parse.restype=C.c_int
  cls.serialize=cls.native.lib.secp256k1_ec_pubkey_serialize;cls.serialize.argtypes=[C.c_void_p,C.c_void_p,C.POINTER(C.c_size_t),C.c_void_p,C.c_uint];cls.serialize.restype=C.c_int
  cls.combine=cls.native.lib.secp256k1_ec_pubkey_combine;cls.combine.argtypes=[C.c_void_p,C.c_void_p,C.POINTER(C.c_void_p),C.c_size_t];cls.combine.restype=C.c_int
  cls.report={'schema':'coax.secp-group.v1','scope':'bounded group correctness and structural source/codegen checks; no scalar-multiply, target constant-time, secret-erasure, device or independent security approval claim','seed':20261028,'compiler':base.run(base.COMPILER,'version').stdout.strip(),'compiler_sha256':file_sha(base.COMPILER),'native_compiler':base.run('cc','--version').stdout.splitlines()[0],'budgets':{'random_pairs':COUNT,'maximum_random_pairs':1024,'max_table':256,'source_steps_per_check':20000000,'compile_time_aggregate_steps':100000,'max_fixture_bytes':8388608},'sources':{str(p.relative_to(ROOT)):file_sha(p) for p in [SOURCE,ROOT/'coax/lib/crypto/secp_field.coax',ROOT/'coax/lib/crypto/bytes.coax',Path(__file__),ROOT/'coax/tests/nostr/secp_group_wasm.c',ROOT/'coax/tests/nostr/secp_group_wasm.cjs',ROOT/'tools/coax-nostr/fuzz/oracle.py',ROOT/'tools/coax-nostr/fuzz/UPSTREAM.json']},'libsecp_artifact_sha256':file_sha(DEFAULT_LIBRARY),'libsecp_source_commit':json.loads((ROOT/'tools/coax-nostr/fuzz/UPSTREAM.json').read_text())['commit'],'modes':[]}
  for label,mode in [('checked',[]),('release',['--release'])]:
   c=cls.dir/(label+'.c');c.write_text(base.run(base.COMPILER,'c',SOURCE,'--library','--header',cls.dir/'secp_group.h',*mode).stdout)
   libpath=cls.dir/(label+'.dylib');base.run('cc','-std=c11','-O2','-shared','-fPIC',c,'-lm','-o',libpath);lib=C.CDLL(str(libpath))
   for n in ['sg_infinity','sg_generator']:getattr(lib,n).argtypes=[];getattr(lib,n).restype=Point
   lib.sg_valid_mask.argtypes=[Point];lib.sg_valid_mask.restype=C.c_uint64
   lib.sg_equal_mask.argtypes=[Point,Point];lib.sg_equal_mask.restype=C.c_uint64
   lib.sg_add.argtypes=[Point,Point];lib.sg_add.restype=Result
   for n in ['sg_double','sg_negate']:getattr(lib,n).argtypes=[Point];getattr(lib,n).restype=Result
   lib.sg_from_affine.argtypes=[Affine];lib.sg_from_affine.restype=Result
   lib.sg_to_affine.argtypes=[Point];lib.sg_to_affine.restype=AffineResult
   lib.sg_mixed_add.argtypes=[Point,Affine];lib.sg_mixed_add.restype=Result
   lib.sg_pick.argtypes=[PointView,C.c_uint64];lib.sg_pick.restype=Result
   for n in ['sg_decode_public','sg_lift_x_public']:getattr(lib,n).argtypes=[base.View];getattr(lib,n).restype=Result
   lib.sg_encode_public.argtypes=[Point];lib.sg_encode_public.restype=Encoded
   cls.libs.append((label,lib));cls.report['modes'].append({'mode':label,'optimization':'O2','generated_c_sha256':file_sha(c),'artifact_sha256':file_sha(libpath)})
 @classmethod
 def tearDownClass(cls):
  cls.native.close();cls.report['fixture_bytes']=sum(p.stat().st_size for p in cls.dir.rglob('*') if p.is_file())
  if cls.report['fixture_bytes']>cls.report['budgets']['max_fixture_bytes']:raise AssertionError('fixture budget exceeded')
  if os.environ.get('COAX_GROUP_REPORT'):Path(os.environ['COAX_GROUP_REPORT']).write_text(json.dumps(cls.report,indent=2,sort_keys=True)+'\n')
  cls.temp.cleanup()
 @classmethod
 def native_parse(cls,value):
  encoded=compressed(value);out=(C.c_uint8*64)()
  if cls.parse(cls.native.context,out,C.c_char_p(encoded),len(encoded))!=1:raise ValueError('public point parse failed')
  return out
 @classmethod
 def native_affine(cls,pub):
  out=(C.c_uint8*65)();n=C.c_size_t(65)
  if cls.serialize(cls.native.context,out,C.byref(n),pub,2)!=1 or n.value!=65:raise ValueError('public point serialization failed')
  return int.from_bytes(bytes(out)[1:33],'big'),int.from_bytes(bytes(out)[33:],'big')
 @classmethod
 def native_point(cls,k):
  encoded=cls.native.point(k.to_bytes(32,'big'));pub=(C.c_uint8*64)()
  if encoded is None or cls.parse(cls.native.context,pub,C.c_char_p(encoded),len(encoded))!=1:raise ValueError('public point creation failed')
  return cls.native_affine(pub)
 @classmethod
 def native_sum(cls,a,b):
  if a is None:return b
  if b is None:return a
  pubs=[cls.native_parse(a),cls.native_parse(b)];pointers=(C.c_void_p*2)(*(C.cast(p,C.c_void_p).value for p in pubs));out=(C.c_uint8*64)()
  return cls.native_affine(out) if cls.combine(cls.native.context,out,pointers,2) else None
 def assert_point(self,value,wanted):
  coords=[]
  for field in [value.x,value.y,value.z]:
   n=base.number(field);self.assertLess(n,base.P)
   for i,digit in enumerate(field.limb):self.assertLessEqual(digit,base.M22 if i==9 else base.M26)
   coords.append(n)
  x,y,z=coords
  if wanted is None:self.assertEqual(coords,[1,1,0]);return
  self.assertNotEqual(z,0);inverse=pow(z,-1,base.P)
  self.assertEqual((x*inverse**2%base.P,y*inverse**3%base.P),wanted)
 def assert_result(self,result,wanted,ok=True):
  self.assertEqual(result.ok,ok);self.assert_point(result.value,wanted if ok else None)
 def test_random_scaled_add_double_mixed_and_independent_public_oracles(self):
  rng=random.Random(20261028);cases=[(None,None),(None,G),(G,None),(G,G),(G,(G[0],base.P-G[1]))]
  beta=pow(2,(base.P-1)//3,base.P);self.assertNotEqual(beta,1);self.assertEqual(pow(beta,3,base.P),1)
  cases.append((G,(G[0]*beta%base.P,G[1]))) # H!=0 with equal affine y: generic r=0
  for i in range(COUNT):
   k=1+rng.randrange(N-1);a=self.native_point(k)
   b=a if i%8==0 else (a[0],base.P-a[1]) if i%8==1 else self.native_point(1+rng.randrange(N-1))
   cases.append((a,b))
  count=0
  for a,b in cases:
   expected=point_add(a,b);self.assertEqual(self.native_sum(a,b),expected)
   za=1+rng.randrange(base.P-1);zb=1+rng.randrange(base.P-1)
   for label,lib in self.libs:
    pa,pb=point(a,za),point(b,zb)
    self.assert_result(lib.sg_add(pa,pb),expected);self.assert_result(lib.sg_mixed_add(pa,affine(b)),expected)
    self.assert_result(lib.sg_double(pa),point_add(a,a));self.assert_result(lib.sg_negate(pa),None if a is None else (a[0],-a[1]%base.P))
    self.assertEqual(lib.sg_equal_mask(pa,point(a)),base.MAX64)
    self.assertEqual(lib.sg_equal_mask(pa,pb),base.MAX64 if a==b else 0);count+=6
  self.report['scaled_oracle_comparisons']=count;self.report['random_pairs_per_mode']=COUNT;self.report['libsecp_pair_comparisons']=len(cases);self.report['same_y_generic_case']=True
 def test_public_encoding_lifts_and_affine_conversion(self):
  rng=random.Random(334);points=[None,G,point_mul(2),point_mul(3)]+[self.native_point(1+rng.randrange(N-1)) for _ in range(64)]
  count=0
  for wanted in points:
   raw=compressed(wanted);data=base.array(raw)
   for label,lib in self.libs:
    p=point(wanted,1+rng.randrange(base.P-1));result=lib.sg_to_affine(p);self.assertTrue(result.ok)
    self.assertEqual((base.number(result.value.x),base.number(result.value.y),result.value.infinity),(0,0,base.MAX64) if wanted is None else (*wanted,0))
    self.assert_result(lib.sg_from_affine(result.value),wanted)
    encoded=lib.sg_encode_public(p);self.assertTrue(encoded.ok);self.assertEqual(encoded.length,len(raw));self.assertEqual(bytes(encoded.bytes)[:encoded.length],raw)
    self.assertEqual(bytes(encoded.bytes)[encoded.length:],b'\0'*(33-encoded.length))
    self.assert_result(lib.sg_decode_public(base.view(data)),wanted)
    if wanted is not None:
     xdata=base.array(wanted[0].to_bytes(32,'big'));self.assert_result(lib.sg_lift_x_public(base.view(xdata)),lift_x(wanted[0]))
    count+=4+(wanted is not None)
  self.report['public_roundtrip_comparisons']=count
 def test_invalid_formats_points_flags_and_table_boundaries_refuse_canonically(self):
  for label,lib in self.libs:
   bad=Point();self.assert_result(lib.sg_add(bad,point(G)),None,False);self.assert_result(lib.sg_double(bad),None,False)
   self.assertEqual(lib.sg_equal_mask(bad,point(None)),0);self.assertEqual(lib.sg_equal_mask(point(None),bad),0)
   for x in range(3):
    for y in range(3):self.assertEqual(lib.sg_valid_mask(Point(base.digits(x),base.digits(y),base.Field())),base.MAX64 if (x,y)==(1,1) else 0)
   for flag in [1,2,2**32,2**63,base.MAX64-1]:self.assert_result(lib.sg_from_affine(Affine(base.digits(G[0]),base.digits(G[1]),flag)),None,False)
   invalid=Point(base.digits(2**255+1),base.Field(),base.digits(1));encoded=lib.sg_encode_public(invalid)
   self.assertFalse(encoded.ok);self.assertEqual(encoded.length,0);self.assertEqual(bytes(encoded.bytes),b'\0'*33)
   for prefix in range(256):
    data=base.array(bytes([prefix])+G[0].to_bytes(32,'big'));wanted=(G[0],G[1] if prefix==2 else base.P-G[1])
    self.assert_result(lib.sg_decode_public(base.view(data)),wanted,prefix in (2,3))
   for raw in [b'',b'\1',b'\0'*32,b'\2'+b'\0'*32,b'\3'+b'\0'*32,b'\2'*34,b'\4'*65]:
    data=base.array(raw);self.assert_result(lib.sg_decode_public(base.view(data)),None,False)
   data=base.array(compressed(G))
   for n in [-2**63,-1,0,32,34,2**63-1]:self.assert_result(lib.sg_decode_public(base.view(data,n)),None,False)
   for x in [0,base.P,base.P+1,2**256-1]:
    data=base.array(x.to_bytes(32,'big'));self.assert_result(lib.sg_lift_x_public(base.view(data)),None,False)
   table=(Point*4)(point(G),point((G[0],base.P-G[1])),point(None),Point())
   for i in range(6):self.assert_result(lib.sg_pick(PointView(table,4),i),[G,(G[0],base.P-G[1]),None,None,None,None][i],i<3)
   self.assert_result(lib.sg_pick(PointView(table,4),2**63),None,False)
   for n in [-2**63,-1,0,257,2**63-1]:self.assert_result(lib.sg_pick(PointView(table,n),0),None,False)
   large=(Point*257)();large[255]=point(G)
   self.assert_result(lib.sg_pick(PointView(large,256),255),G);self.assert_result(lib.sg_pick(PointView(large,257),255),None,False)
  self.report['public_prefix_cases_per_mode']=256
 def test_scoped_codegen_has_no_conditionals_in_exception_kernels_or_secret_index_load(self):
  for label,lib in self.libs:
   source=(self.dir/(label+'.c')).read_text()
   for name in ['sg_add_valid','sg_double_valid','sg_same_valid','sg_valid_mask']:
    self.assertTrue(unconditional_body(source,name),name)
   _,_,pick=body_of(source,'sg_pick')
   accesses=[s for s in pick.splitlines() if 'coax_elem_v_t_SgPoint' in s]
   self.assertEqual(len(accesses),1);self.assertIn('l_i,',accesses[0]);self.assertNotIn('l_which,',accesses[0].split('coax_elem_v_t_SgPoint')[1])
   # The structural guard catches an actual inserted conditional; it does not
   # claim complete taint analysis or prove target machine/JIT behavior.
   begin,_,_=body_of(source,'sg_add_valid');seeded=source[:begin]+'\n  if (1) {}\n'+source[begin:]
   self.assertFalse(unconditional_body(seeded,'sg_add_valid'))
  self.report['structural_codegen_scope']='add/double/equality/curve kernels no explicit C conditionals; table element indexed by public loop; whole-program/target CT unclaimed'
 def test_actual_wasm_general_scaled_exception_decode_and_table_cases(self):
  def words(value):return [str(n) for f in [value.x,value.y,value.z] for n in f.limb]
  def awords(value):return [str(n) for f in [value.x,value.y] for n in f.limb]+[str(value.infinity)]
  def expected(value):return None if value is None else [f'{n:064x}' for n in value]
  rng=random.Random(8810);pairs=[(None,None),(None,G),(G,None),(G,G),(G,(G[0],base.P-G[1]))]+[(self.native_point(1+rng.randrange(N-1)),self.native_point(1+rng.randrange(N-1))) for _ in range(32)]
  beta=pow(2,(base.P-1)//3,base.P);pairs.append((G,(G[0]*beta%base.P,G[1])))
  operations=[]
  for a,b in pairs:
   pa=words(point(a,1+rng.randrange(base.P-1)));pb=words(point(b,1+rng.randrange(base.P-1)))
   for op,want,q in [(0,point_add(a,b),pb),(1,point_add(a,a),pb),(2,None if a is None else (a[0],-a[1]%base.P),pb),(3,point_add(a,b),awords(affine(b)))]:operations.append({'op':op,'a':pa,'b':q,'expected':expected(want),'ok':True})
  operations.append({'op':0,'a':words(Point()),'b':words(point(G)),'expected':None,'ok':False})
  decodes=[]
  for raw,want,ok,lift in [(compressed(G),G,True,False),(b'\0',None,True,False),(b'\3'+b'\0'*32,None,False,False),(b'\2'+base.P.to_bytes(32,'big'),None,False,False),(G[0].to_bytes(32,'big'),G,True,True),(b'\0'*32,None,False,True),(b'',None,False,False)]:decodes.append({'bytes':raw.hex(),'expected':expected(want),'ok':ok,'lift':lift})
  table=[words(point(G)),words(point(None)),words(Point())];picks=[]
  for index in [0,1,2,3,2**63,base.MAX64]:picks.append({'table':table,'which':str(index),'expected':expected(G if index==0 else None),'ok':index<2})
  encodes=[{'point':words(point(v)),'bytes':compressed(v).hex(),'length':len(compressed(v))} for v in [None,G,point_add(G,G)]]
  encodes.append({'point':words(Point(base.digits(2**255+1),base.Field(),base.digits(1))),'bytes':'00'*33,'length':-1})
  fixture=self.dir/'group.json';fixture.write_text(json.dumps({'operations':operations,'decodes':decodes,'picks':picks,'encodes':encodes}));reports=[]
  for label,lib in self.libs:
   wasm=self.dir/(label+'.wasm')
   base.run('/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-I'+str(self.dir),self.dir/(label+'.c'),ROOT/'coax/tests/nostr/secp_group_wasm.c','-mexec-model=reactor','-Wl,--import-memory,--export-memory,--max-memory=67108864,--export=__heap_base','-lm','-o',wasm)
   result=json.loads(base.run('node',ROOT/'coax/tests/nostr/secp_group_wasm.cjs',wasm,fixture).stdout)
   reports.append({'mode':label,'artifact_sha256':file_sha(wasm),'wasm_compiler':base.run('/opt/homebrew/opt/llvm/bin/clang','--version').stdout.splitlines()[0],**result})
  self.report['wasm']=reports
 def test_sanitized_exceptions_and_actual_emitted_faults(self):
  c=self.dir/'sanitized.c';exe=self.dir/'sanitized'
  c.write_text('''#include "checked.c"
#include <assert.h>
int main(void){coax_t_SgPoint a=sg_generator(),inf=sg_infinity();
 for(int i=0;i<128;i++){
  coax_t_SgResult doubled=sg_double(a),negative=sg_negate(a),sum=sg_add(a,negative.value);
  assert(doubled.ok&&negative.ok&&sum.ok);assert(sg_equal_mask(sum.value,inf)==UINT64_MAX);
  assert(sg_valid_mask(doubled.value)==UINT64_MAX);
  coax_t_SgPoint table[3]={a,inf,{0}};
  assert(!sg_pick((coax_v_t_SgPoint){table,3},UINT64_C(9223372036854775808)).ok);
  assert(!sg_pick((coax_v_t_SgPoint){table,257},0).ok);
  assert(!sg_pick((coax_v_t_SgPoint){table,-1},0).ok);
  a=sg_add(doubled.value,sg_generator()).value;
 }return 0;}''')
  base.run('cc','-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer',c,'-lm','-o',exe);base.run(exe)
  original=(self.dir/'checked.c').read_text();begin,end,body=body_of(original,'sg_add_valid');lines=body.splitlines()
  equal=[i for i,s in enumerate(lines) if 'coax_p_sg_choose(l_same,' in s];self.assertEqual(len(equal),1)
  remove_equal=lines.copy();remove_equal[equal[0]]='  l_r = l_r;'
  infinity=[i for i,s in enumerate(lines) if 'coax_p_sg_choose(sf_equal_mask((l_a).z' in s];self.assertEqual(len(infinity),1)
  remove_inf=lines.copy();remove_inf[infinity[0]]='  l_r = l_r;'
  same=[i for i,s in enumerate(lines) if 'uint64_t l_same =' in s];self.assertEqual(len(same),1)
  remove_opposite=lines.copy();remove_opposite[same[0]]='  uint64_t l_same = sf_equal_mask(l_h,l_zero);'
  variants={'missing_equal_case':('\n'.join(remove_equal),point(G),point(G),point_add(G,G)),
   'missing_infinity_case':('\n'.join(remove_inf),point(None),point(G),G),
   'missing_opposite_case':('\n'.join(remove_opposite),point(G),point((G[0],base.P-G[1])),None)}
  detected=[]
  for name,(changed,a,b,want) in variants.items():
   path=self.dir/(name+'.c');path.write_text(original[:begin]+changed+original[end:]);library=self.dir/(name+'.dylib');base.run('cc','-std=c11','-O2','-shared','-fPIC',path,'-lm','-o',library)
   lib=C.CDLL(str(library));lib.sg_add.argtypes=[Point,Point];lib.sg_add.restype=Result;lib.sg_encode_public.argtypes=[Point];lib.sg_encode_public.restype=Encoded
   result=lib.sg_add(a,b);encoded=lib.sg_encode_public(result.value)
   self.assertNotEqual((result.ok,encoded.ok,bytes(encoded.bytes)[:encoded.length]),(True,True,compressed(want)));detected.append(name)
  self.report['sanitized_exception_cases']=128;self.report['detected_emitted_faults']=detected

if __name__=='__main__':unittest.main()
