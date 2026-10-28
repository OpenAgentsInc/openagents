#!/usr/bin/env python3
"""Independent scalar arithmetic, actual C ABI/Wasm and seeded carry checks."""
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
import time
import unittest
from scalar_candidate import Scalar,Inverse,scalar,integer,library

ROOT=Path(__file__).resolve().parents[3]
COMPILER=Path(os.environ.get('COAX_TEST_COMPILER',ROOT/'coax/bin/coax'))
SOURCE=ROOT/'coax/lib/crypto/secp_scalar.coax'
N=0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141
class View(C.Structure):_fields_=[('data',C.POINTER(C.c_uint8)),('count',C.c_int64)]
class Load(C.Structure):_fields_=[('value',Scalar),('overflow',C.c_bool),('ok',C.c_bool)]
class Window(C.Structure):_fields_=[('value',C.c_uint32),('ok',C.c_bool)]
def run(*args,**kw):
 result=subprocess.run([str(x) for x in args],capture_output=True,text=True,**kw)
 if result.returncode:raise RuntimeError(f'{args[0]} failed ({result.returncode}): {result.stderr or result.stdout}')
 return result
def array(data):return(C.c_uint8*len(data))(*data)
def view(data,at=0,count=None):return View(C.cast(C.byref(data,at),C.POINTER(C.c_uint8)),len(data)-at if count is None else count)
def expected(op,a,b):
 if op==0:return a%N
 if op==1:return(a+b)%N
 if op==2:return(a-b)%N
 if op==3:return a*b%N
 if op==4:return-a%N
 return None if a%N==0 else pow(a,-1,N)
def actual(lib,op,a,b):
 if op==0:return integer(lib.sc_normalize(a))
 if op==4:return integer(lib.sc_negate(a))
 if op==5:
  inv=lib.sc_inverse(a);return integer(inv.value) if inv.ok else None
 return integer(getattr(lib,['','','sc_sub','sc_mul'][op] if op>1 else 'sc_add')(a,b))

class Scalars(unittest.TestCase):
 @classmethod
 def setUpClass(cls):
  cls.temp=tempfile.TemporaryDirectory(prefix='coax-scalar-');cls.dir=Path(cls.temp.name);cls.libs=[]
  for label,mode in [('checked',[]),('release',['--release'])]:
   source=cls.dir/(label+'.c');source.write_text(run(COMPILER,'c',SOURCE,'--library','--header',cls.dir/'scalar.h',*mode).stdout)
   artifact=cls.dir/(label+'.dylib');run('cc','-std=c11','-O2','-shared','-fPIC',source,'-lm','-o',artifact);lib=library(artifact)
   lib.sc_load.argtypes=[View];lib.sc_load.restype=Load;lib.sc_reduce_bytes.argtypes=[View];lib.sc_reduce_bytes.restype=Load
   lib.sc_store.argtypes=[Scalar,View];lib.sc_store.restype=C.c_bool
   for name in ['sc_is_zero','sc_overflow','sc_secret_valid']:
    getattr(lib,name).argtypes=[Scalar];getattr(lib,name).restype=C.c_bool
   lib.sc_window.argtypes=[Scalar,C.c_int64,C.c_int64];lib.sc_window.restype=Window
   lib.sc_evaluate.argtypes=[C.c_int64,View,View,View];lib.sc_evaluate.restype=C.c_bool
   cls.libs.append(lib)
 @classmethod
 def tearDownClass(cls):cls.temp.cleanup()
 def test_edges_carries_normalization_and_inverse(self):
  edges=[0,1,(1<<32)-1,1<<32,(1<<64)-1,1<<255,N-1,N,(1<<256)-1]
  for a in edges:
   for b in edges:
    for op in range(6):
     want=expected(op,a,b)
     for lib in self.libs:self.assertEqual(actual(lib,op,scalar(a),scalar(b)),want,(op,hex(a),hex(b)))
 def test_million_independent_random_operations_checked_and_optimized(self):
  count=int(os.environ.get('COAX_SCALAR_CASES','1000000'));self.assertGreaterEqual(count,1000000)
  rng=random.Random(20261028);corpus=hashlib.sha256();counts=[0]*6;start=time.monotonic()
  for index in range(count):
   a=rng.getrandbits(256);b=rng.getrandbits(256);op=5 if index%1000==0 else index%5;counts[op]+=1
   corpus.update(bytes([op])+a.to_bytes(32,'big')+b.to_bytes(32,'big'));want=expected(op,a,b);x=scalar(a);y=scalar(b)
   for lib in self.libs:self.assertEqual(actual(lib,op,x,y),want,(index,op,hex(a),hex(b)))
  inputs=[SOURCE,ROOT/'coax/lib/crypto/bytes.coax',ROOT/'coax/tests/nostr/scalar_fixtures.coax',Path(__file__),ROOT/'coax/tests/nostr/scalar_candidate.py',ROOT/'coax/tests/nostr/scalar_wasm.c',ROOT/'coax/tests/nostr/scalar_wasm.cjs']
  report={'schema':'coax.scalar-differential.v1','seed':20261028,'requests':count,'native_comparisons':2*count,'operations':dict(zip(['normalize','add','sub','mul','negate','inverse'],counts)),'oracle':'independent Python unbounded integers and pow(a,-1,n)','elapsed_seconds':time.monotonic()-start,'corpus_sha256':corpus.hexdigest(),'compiler_sha256':hashlib.sha256(COMPILER.read_bytes()).hexdigest(),'compiler_version':run(COMPILER,'--version').stdout.strip(),'host':platform.platform(),'native_compiler':run('cc','--version').stdout.splitlines()[0],'flags':['-std=c11','-O2','checked and --release'],'source_sha256':{str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in inputs},'artifacts':{label:hashlib.sha256((self.dir/(label+'.dylib')).read_bytes()).hexdigest() for label in ['checked','release']}}
  path=Path(os.environ.get('COAX_SCALAR_REPORT','/tmp/coax-scalar-differential.json'));path.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({'requests':count,'operations':counts,'elapsed_seconds':report['elapsed_seconds']}))
 def test_canonical_bytes_windows_and_transactional_aliases(self):
  for lib in self.libs:
   for a in [0,1,N-1,N,(1<<256)-1]:
    raw=array(a.to_bytes(32,'big'));loaded=lib.sc_load(view(raw));reduced=lib.sc_reduce_bytes(view(raw))
    self.assertEqual((loaded.ok,loaded.overflow,integer(loaded.value)),(a<N,a>=N,a if a<N else 0))
    self.assertEqual((reduced.ok,reduced.overflow,integer(reduced.value)),(True,a>=N,a%N))
    self.assertEqual(lib.sc_secret_valid(scalar(a)),0<a<N);self.assertEqual(lib.sc_overflow(scalar(a)),a>=N)
    encoded=array([165]*40);self.assertEqual(lib.sc_store(scalar(a),view(encoded)),a<N)
    self.assertEqual(bytes(encoded),a.to_bytes(32,'big')+b'\xa5'*8 if a<N else b'\xa5'*40)
    for offset in range(256):
     for width in [1,min(8,256-offset)]:
      result=lib.sc_window(scalar(a),offset,width);self.assertTrue(result.ok);self.assertEqual(result.value,(a>>offset)&((1<<width)-1))
   a=array((N-1).to_bytes(32,'big'));b=array((2).to_bytes(32,'big'));out=array([165]*40)
   for op in range(6):
    self.assertTrue(lib.sc_evaluate(op,view(a),view(b),view(out)));self.assertEqual(bytes(out)[:32],expected(op,N-1,2).to_bytes(32,'big'));self.assertEqual(bytes(out)[32:],b'\xa5'*8)
   before=bytes(a);self.assertFalse(lib.sc_evaluate(0,view(a,count=-1),view(b),view(out)));self.assertEqual(bytes(a),before)
   self.assertTrue(lib.sc_evaluate(1,view(a),view(b),view(a)));self.assertEqual(bytes(a),(1).to_bytes(32,'big'))
   zero=array(bytes(32));saved=bytes(out);self.assertFalse(lib.sc_evaluate(5,view(zero),view(b),view(out)));self.assertEqual(bytes(out),saved)
 def test_seeded_lost_high_product_carry_is_detected(self):
  text=(self.dir/'checked.c').read_text();start=text.index('static coax_t_ScScalar coax_p_sc_montgomery(',text.index('static coax_t_ScScalar coax_p_sc_montgomery(')+1)
  before=text[:start];body=text[start:];body,changed=re.subn(r'l_carry = coax_shr_u64\(l_wide,[^;]+;','l_carry = 0;',body,count=1);self.assertEqual(changed,1)
  fault=self.dir/'carry-fault.c';fault.write_text(before+body);artifact=self.dir/'carry-fault.dylib'
  run('cc','-std=c11','-O2','-shared','-fPIC',fault,'-lm','-o',artifact);bad=library(artifact)
  self.assertNotEqual(actual(bad,3,scalar(N-1),scalar(N-1)),1)
 def test_existing_differential_reference_and_json_contract(self):
  sys.path.insert(0,str(ROOT/'tools/coax-nostr/fuzz'));from differential import compare
  rng=random.Random(2265);requests=[]
  for value in [0,1,N-1,N,2**256-1]+[rng.getrandbits(256) for _ in range(3)]:
   for op in ['normalize','add','sub','mul','negate','inverse']:requests.append({'module':'scalar','op':op,'a':format(value,'064x'),'b':format(N-1,'064x')})
  for request in requests:
   response=json.loads(run(sys.executable,ROOT/'coax/tests/nostr/scalar_candidate.py','--library',self.dir/'checked.dylib',input=json.dumps(request)).stdout)
   self.assertTrue(compare(request,response,self.dir/'regressions.jsonl'))
 def test_native_sanitizers_and_metadata_refusals(self):
  host=self.dir/'sanitized.c';host.write_text('''#include "scalar.h"
#include <assert.h>
int main(void){uint8_t a[33]={0},b[33]={0},out[40];for(int i=0;i<40;i++)out[i]=165;a[31]=1;b[31]=2;
coax_v_u8 x={a,32},y={b,32},o={out,40};
for(int op=0;op<6;op++)assert(sc_evaluate(op,x,y,o));
for(int i=32;i<40;i++)assert(out[i]==165);
assert(!sc_evaluate(0,(coax_v_u8){a,-1},y,o));assert(!sc_evaluate(0,x,(coax_v_u8){b,33},o));
assert(!sc_evaluate(0,x,y,(coax_v_u8){out,31}));assert(!sc_evaluate(6,x,y,o));
assert(!sc_window(sc_load(x).value,-9223372036854775807LL-1,1).ok);
assert(!sc_window(sc_load(x).value,9223372036854775807LL,1).ok);
assert(sc_evaluate(1,x,y,(coax_v_u8){a+1,32}));assert(a[32]==3);
coax_t_ScScalar s={{.e={0xffffffff,0xffffffff,0xffffffff,0xffffffff,0xffffffff,0xffffffff,0xffffffff,0xffffffff}}};
for(int i=0;i<10000;i++){s=sc_mul(s,s);s=sc_add(s,sc_negate(s));s.limbs.e[i%8]=0xffffffff;}
return 0;}''')
  run('cc','-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer',self.dir/'checked.c',host,'-lm','-o',self.dir/'sanitized');run(self.dir/'sanitized')
 def test_actual_wasm_edge_and_random_integer_oracle(self):
  clang=Path(os.environ.get('COAXIAL_CLANG','/opt/homebrew/opt/llvm/bin/clang'));sysroot=Path(os.environ.get('COAXIAL_SYSROOT','/opt/homebrew/opt/wasi-libc/share/wasi-sysroot'));resources=Path(os.environ.get('COAXIAL_RESOURCES','/opt/homebrew/opt/wasi-runtimes/share/wasi-runtimes'))
  self.assertTrue(clang.exists() and sysroot.exists() and resources.exists(),'explicit actual Wasm toolchain required')
  rng=random.Random(20261028);rows=[]
  for index in range(2000):
   a=rng.getrandbits(256);b=rng.getrandbits(256);op=5 if index%100==0 else index%5
   want=expected(op,a,b);rows.append({'op':op,'a':format(a,'064x'),'b':format(b,'064x'),'value':None if want is None else format(want,'064x')})
  for a in [0,1,N-1,N,2**256-1]:
   for op in range(6):
    want=expected(op,a,N-1);rows.append({'op':op,'a':format(a,'064x'),'b':format(N-1,'064x'),'value':None if want is None else format(want,'064x')})
  fixture=self.dir/'wasm.json';fixture.write_text(json.dumps(rows));env=os.environ.copy();env['PATH']='/opt/homebrew/opt/lld/bin:'+env.get('PATH','')
  for label in ['checked','release']:
   module=self.dir/(label+'.wasm');run(clang,'--target=wasm32-wasip1','--sysroot='+str(sysroot),'-resource-dir='+str(resources),'-std=c11','-O2','-ffp-contract=off','-fno-fast-math','-I'+str(self.dir),'-mexec-model=reactor',self.dir/(label+'.c'),ROOT/'coax/tests/nostr/scalar_wasm.c','-Wl,--import-memory,--export-memory,--export=__heap_base,--max-memory=67108864,-z,stack-size=1048576,--gc-sections','-o',module,env=env)
   report=json.loads(run('node',ROOT/'coax/tests/nostr/scalar_wasm.cjs',module,fixture).stdout);self.assertEqual(report['comparisons'],len(rows))

if __name__=='__main__':unittest.main()
