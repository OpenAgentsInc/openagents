#!/usr/bin/env python3
"""Independent bigint checks for the actual Coax field representation.

Explicit test tooling only. No keys, group arithmetic, timing assurance or
production Python dependency. Raw digit cases independently enforce limb bounds.
"""
import ctypes as C
import hashlib
import json
import os
from pathlib import Path
import platform
import random
import resource
import subprocess
import sys
import tempfile
import unittest

ROOT=Path(__file__).resolve().parents[3]
COMPILER=Path(os.environ.get('COAX_TEST_COMPILER', ROOT/'coax/bin/coax'))
SOURCE=ROOT/'coax/lib/crypto/secp_field.coax'
sys.path.insert(0,str(ROOT/'tools/coax-nostr/fuzz'))
from oracle import P, field as reference_field

M26=2**26-1
M22=2**22-1
MAX64=2**64-1
class View(C.Structure):_fields_=[('data',C.POINTER(C.c_uint8)),('count',C.c_int64)]
class Field(C.Structure):_fields_=[('limb',C.c_uint64*10)]
class Result(C.Structure):_fields_=[('value',Field),('ok',C.c_bool)]
class Debug(C.Structure):_fields_=[('value',Field),('magnitude',C.c_int64),('normalized',C.c_bool)]
def run(*args,**kw):
 r=subprocess.run([str(a) for a in args],capture_output=True,text=True,**kw)
 if r.returncode:raise RuntimeError(f'{args[0]} failed ({r.returncode}): {r.stderr or r.stdout}')
 return r
def array(data):return (C.c_uint8*len(data))(*data)
def view(data,count=None):return View(C.cast(data,C.POINTER(C.c_uint8)),len(data) if count is None else count)
def number(value):return sum(int(n)<<(26*i) for i,n in enumerate(value.limb))
def digits(value):return Field((C.c_uint64*10)(*((value>>(26*i))&(M22 if i==9 else M26) for i in range(10))))
def raw_cases(seed=20261028,count=512):
 rng=random.Random(seed)
 for magnitude in range(1,9):
  yield magnitude,[(M22 if i==9 else M26)*magnitude for i in range(10)]
  for i in range(10):
   x=[0]*10;x[i]=(M22 if i==9 else M26)*magnitude;yield magnitude,x
 for _ in range(count):
  magnitude=rng.randrange(1,9)
  yield magnitude,[rng.randrange((M22 if i==9 else M26)*magnitude+1) for i in range(10)]
def input_cases():
 rng=random.Random(34026)
 return sorted(set([0,1,2,P-2,P-1,P,P+1,2**256-1]+[x for i in range(256) for x in [2**i-1,2**i,2**i+1] if x<2**256]))+[rng.getrandbits(256) for _ in range(1024)]

class SecpField(unittest.TestCase):
 @classmethod
 def setUpClass(cls):
  cls.temp=tempfile.TemporaryDirectory(prefix='coax-secp-field-');cls.dir=Path(cls.temp.name);cls.libs=[]
  cls.evidence={'schema':'coax.secp-field-repr.v1','seed':20261028,'scope':'normalize/load/store/strict/add/sub/negate/equality only; public deterministic fixtures; no field multiplication, constant-time, one-hour fuzz or security approval claim','compiler':run(COMPILER,'version').stdout.strip(),'compiler_sha256':hashlib.sha256(COMPILER.read_bytes()).hexdigest(),'platform':platform.platform(),'native_compiler':run('cc','--version').stdout.splitlines()[0],'sources':{str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in [SOURCE,ROOT/'coax/lib/crypto/bytes.coax',Path(__file__),ROOT/'coax/tests/nostr/secp_field_wasm.c',ROOT/'coax/tests/nostr/secp_field_wasm.cjs',ROOT/'tools/coax-nostr/fuzz/oracle.py']},'modes':[]}
  for label,flags in [('checked',[]),('release',['--release'])]:
   c=cls.dir/(label+'.c');header=cls.dir/'secp_field.h'
   c.write_text(run(COMPILER,'c',SOURCE,'--library','--header',header,*flags).stdout)
   run('cc','-std=c11','-O2','-shared','-fPIC',c,'-lm','-o',cls.dir/(label+'.dylib'))
   lib=C.CDLL(str(cls.dir/(label+'.dylib')))
   for name in ['sf_normalize','sf_negate']:getattr(lib,name).argtypes=[Field];getattr(lib,name).restype=Field
   for name in ['sf_add','sf_subtract']:getattr(lib,name).argtypes=[Field,Field];getattr(lib,name).restype=Field
   for name in ['sf_load','sf_load_strict']:getattr(lib,name).argtypes=[View];getattr(lib,name).restype=Result
   lib.sf_store.argtypes=[Field,View];lib.sf_store.restype=C.c_bool
   lib.sf_equal_mask.argtypes=[Field,Field];lib.sf_equal_mask.restype=C.c_uint64
   lib.sf_debug_valid.argtypes=[Debug];lib.sf_debug_valid.restype=C.c_bool
   for name in ['sf_debug_normalize','sf_debug_negate']:getattr(lib,name).argtypes=[Debug];getattr(lib,name).restype=Debug
   lib.sf_debug_add.argtypes=[Debug,Debug];lib.sf_debug_add.restype=Debug
   cls.libs.append((label,lib))
   cls.evidence['modes'].append({'mode':label,'target':'native-c11','flags':['-std=c11','-O2','-shared','-fPIC'],'generated_c_sha256':hashlib.sha256(c.read_bytes()).hexdigest(),'artifact_sha256':hashlib.sha256((cls.dir/(label+'.dylib')).read_bytes()).hexdigest()})
 @classmethod
 def tearDownClass(cls):
  if os.environ.get('COAX_FIELD_REPORT'):
   Path(os.environ['COAX_FIELD_REPORT']).write_text(json.dumps(cls.evidence,indent=2,sort_keys=True)+'\n')
  cls.temp.cleanup()
 def canonical(self,value,expected):
  self.assertEqual(number(value),expected)
  self.assertLess(number(value),P)
  for i,n in enumerate(value.limb):self.assertLessEqual(n,M22 if i==9 else M26)
 def store(self,lib,value):
  out=array([165]*40);v=View(C.cast(C.byref(out,4),C.POINTER(C.c_uint8)),32)
  self.assertTrue(lib.sf_store(value,v));self.assertEqual(bytes(out)[:4],b'\xa5'*4);self.assertEqual(bytes(out)[36:],b'\xa5'*4)
  return bytes(out)[4:36]
 def test_independent_native_integer_oracle_in_both_compiler_modes(self):
  cases=input_cases();comparisons=0
  for label,lib in self.libs:
   for i,a in enumerate(cases):
    b=cases[(i*17+9)%len(cases)];source=array(a.to_bytes(32,'big'));loaded=lib.sf_load(view(source));self.assertTrue(loaded.ok)
    self.canonical(loaded.value,reference_field('normalize',a));self.assertEqual(self.store(lib,loaded.value),(a%P).to_bytes(32,'big'))
    strict=lib.sf_load_strict(view(source));self.assertEqual(strict.ok,a<P);self.canonical(strict.value,a if a<P else 0)
    x,y=digits(a),digits(b)
    for op,name in [('add','sf_add'),('sub','sf_subtract')]:self.canonical(getattr(lib,name)(x,y),reference_field(op,a,b));comparisons+=1
    self.canonical(lib.sf_negate(x),reference_field('negate',a));comparisons+=1
    self.assertEqual(lib.sf_equal_mask(x,y),MAX64 if a%P==b%P else 0)
    self.assertEqual(lib.sf_equal_mask(x,loaded.value),MAX64);comparisons+=2
  self.evidence['native_unsigned256_cases_per_mode']=len(cases);self.evidence['native_oracle_comparisons']=comparisons
 def test_raw_magnitudes_and_debug_metadata_against_bigint(self):
  rows=list(raw_cases());comparisons=0
  for label,lib in self.libs:
   for magnitude,limbs in rows:
    x=Field((C.c_uint64*10)(*limbs));expected=number(x)%P;d=Debug(x,magnitude,False)
    self.assertTrue(lib.sf_debug_valid(d));self.canonical(lib.sf_normalize(x),expected)
    checked=lib.sf_debug_normalize(d);self.assertEqual((checked.magnitude,checked.normalized),(1,True));self.assertTrue(lib.sf_debug_valid(checked));self.canonical(checked.value,expected)
    self.canonical(lib.sf_negate(x),-expected%P);self.canonical(lib.sf_add(x,x),2*expected%P)
    self.assertEqual(self.store(lib,x),expected.to_bytes(32,'big'));comparisons+=5
   for magnitude in range(1,9):
    for i in range(10):
     x=Field();x.limb[i]=magnitude*(M22 if i==9 else M26)+1
     self.assertFalse(lib.sf_debug_valid(Debug(x,magnitude,False)))
  self.evidence['raw_limb_cases_per_mode']=len(rows);self.evidence['raw_oracle_comparisons']=comparisons
 def test_wire_refusal_preserves_output_and_zeroes_rejected_results(self):
  for label,lib in self.libs:
   source=array([165]*40);zero=Field()
   for n in [-2**63,-1,0,1,31,33,40,2**63-1]:
    for name in ['sf_load','sf_load_strict']:
     result=getattr(lib,name)(view(source,n));self.assertFalse(result.ok);self.assertEqual(number(result.value),0)
    before=bytes(source);self.assertFalse(lib.sf_store(zero,view(source,n)));self.assertEqual(bytes(source),before)
 def test_sanitized_defined_arithmetic_and_output_guards(self):
  c=self.dir/'sanitized.c';exe=self.dir/'sanitized'
  c.write_text('''#include "checked.c"
#include <assert.h>
int main(void){
 uint64_t seed=20261028;uint8_t out[40];
 for(int test=0;test<4096;test++){
  coax_t_SecpFieldDebug d={0};d.magnitude=1+test%8;
  for(int i=0;i<10;i++){seed=seed*UINT64_C(6364136223846793005)+1;
   d.value.limb.e[i]=seed%((uint64_t)d.magnitude*(i==9?UINT64_C(4194303):UINT64_C(67108863))+1);}
  d=sf_debug_normalize(d);assert(sf_debug_valid(d));
  coax_t_SecpField sum=sf_add(d.value,sf_negate(d.value)),zero={0};
  assert(sf_equal_mask(sum,zero)==UINT64_MAX);
  for(int i=0;i<40;i++)out[i]=165;
  assert(sf_store(d.value,(coax_v_u8){out+4,32}));
  for(int i=0;i<4;i++){assert(out[i]==165);assert(out[i+36]==165);}
 }
 return 0;
}''')
  run('cc','-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer',c,'-lm','-o',exe)
  run(exe);self.evidence['sanitized_native_cases']=4096
 def test_debug_operations_trap_actual_bound_violations(self):
  # These exceed the declared digit bound at magnitude8. They do not assume
  # magnitude9 overflows u64, and run in isolated processes with cores disabled.
  count=0
  for label,lib in self.libs:
   c=self.dir/(label+'_negative.c');exe=self.dir/(label+'_negative')
   c.write_text(f'''#include "{label}.c"
int main(int argc,char**argv){{coax_t_SecpFieldDebug d={{0}};d.magnitude=8;
 int i=atoi(argv[1]);d.value.limb.e[i]=8*(i==9?UINT64_C(4194303):UINT64_C(67108863))+1;
 d=sf_debug_normalize(d);return d.normalized?0:2;}}
''')
   run('cc','-std=c11','-O2',c,'-lm','-o',exe)
   for i in range(10):
    result=subprocess.run([str(exe),str(i)],capture_output=True,text=True,preexec_fn=lambda:resource.setrlimit(resource.RLIMIT_CORE,(0,0)))
    self.assertNotEqual(result.returncode,0);self.assertIn('assertion failed',result.stderr);count+=1
  self.evidence['isolated_bound_traps']=count
 def test_wasm_matches_independent_supplied_fixtures(self):
  cases=input_cases();operations=[]
  for i,a in enumerate(cases[::7]):
   b=cases[(i*31+2)%len(cases)]
   for op in ['normalize','add','sub','negate']:
    operations.append({'op':op,'a':f'{a:064x}','b':f'{b:064x}','expected':f'{reference_field(op,a,b):064x}'})
  raw=[{'magnitude':m,'limbs':[str(x) for x in d],'expected':f'{sum(x<<(26*i) for i,x in enumerate(d))%P:064x}'} for m,d in raw_cases(count=96)]
  fixtures=self.dir/'fixtures.json';fixtures.write_text(json.dumps({'operations':operations,'raw':raw}))
  reports=[]
  for label,lib in self.libs:
   wasm=self.dir/(label+'.wasm')
   run('/opt/homebrew/opt/llvm/bin/clang','--target=wasm32-wasip1','--sysroot=/opt/homebrew/opt/wasi-libc/share/wasi-sysroot','-resource-dir=/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes','-std=c11','-O2','-I'+str(self.dir),self.dir/(label+'.c'),ROOT/'coax/tests/nostr/secp_field_wasm.c','-mexec-model=reactor','-Wl,--import-memory,--export-memory,--max-memory=67108864,--export=__heap_base','-lm','-o',wasm)
   report=json.loads(run('node',ROOT/'coax/tests/nostr/secp_field_wasm.cjs',wasm,fixtures).stdout)
   reports.append({'mode':label,'optimization':'O2, no LTO','wasm_compiler':run('/opt/homebrew/opt/llvm/bin/clang','--version').stdout.splitlines()[0],'artifact_sha256':hashlib.sha256(wasm.read_bytes()).hexdigest(),**report})
  self.evidence['wasm']=reports
 def test_seeded_wrong_reduction_is_detected_by_independent_oracle(self):
  original=(self.dir/'checked.c').read_text()
  # Alter an actual emitted kernel, not its expected-result model. Every977
  # in this program belongs to field reduction; source metadata has no such literal.
  self.assertIn('UINT64_C(977)',original)
  changed=original.replace('UINT64_C(977)','UINT64_C(976)')
  c=self.dir/'seeded.c';c.write_text(changed);run('cc','-std=c11','-O2','-shared','-fPIC',c,'-lm','-o',self.dir/'seeded.dylib')
  lib=C.CDLL(str(self.dir/'seeded.dylib'));lib.sf_normalize.argtypes=[Field];lib.sf_normalize.restype=Field
  x=Field((C.c_uint64*10)(*([8*M26]*9+[8*M22])))
  self.assertNotEqual(number(lib.sf_normalize(x)),number(x)%P)
  self.evidence['seeded_fault']='emitted reduction977→976 detected at admitted magnitude8'

if __name__=='__main__':unittest.main()
