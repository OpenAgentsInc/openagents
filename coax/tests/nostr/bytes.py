#!/usr/bin/env python3
"""Scoped byte-helper native oracle, alias/boundary and codegen checks."""
import ctypes as C
from pathlib import Path
import os
import platform
import random
import re
import shutil
import subprocess
import tempfile
import unittest

ROOT=Path(__file__).resolve().parents[3]
COMPILER=Path(os.environ.get('COAX_TEST_COMPILER',ROOT/'coax/bin/coax'))
PURE=ROOT/'coax/lib/crypto/bytes.coax'
BRIDGE=ROOT/'coax/lib/crypto/bytes-memory.coax'
MAX64=2**64-1
MAX32=2**32-1

def run(*args,**kw):
 r=subprocess.run([str(a) for a in args],capture_output=True,text=True,**kw)
 if r.returncode: raise RuntimeError(f'{args[0]} failed ({r.returncode}): {r.stderr or r.stdout}')
 return r

class View(C.Structure): _fields_=[('data',C.POINTER(C.c_uint8)),('count',C.c_int64)]
class U32(C.Structure): _fields_=[('value',C.c_uint32),('ok',C.c_bool)]
class U64(C.Structure): _fields_=[('value',C.c_uint64),('ok',C.c_bool)]
def view(data,at=0,count=None):
 if count is None:count=len(data)-at
 return View(C.cast(C.byref(data,at),C.POINTER(C.c_uint8)),count)
def array(data):return (C.c_uint8*len(data))(*data)

class Bytes(unittest.TestCase):
 @classmethod
 def setUpClass(cls):
  cls.temp=tempfile.TemporaryDirectory(prefix='coax-crypto-bytes-');cls.dir=Path(cls.temp.name)
  cls.libs=[]
  for mode in ['', '--release']:
   label='checked' if not mode else 'release';c=cls.dir/(label+'.c');header=cls.dir/'bytes.h'
   c.write_text(run(COMPILER,'c',BRIDGE,'--library','--header',header,*([mode] if mode else [])).stdout)
   run('cc','-std=c11','-O2','-fPIC','-shared',c,'-lm','-o',cls.dir/(label+'.dylib'))
   lib=C.CDLL(str(cls.dir/(label+'.dylib')))
   lib.cb_mask.argtypes=[C.c_uint64];lib.cb_mask.restype=C.c_uint64
   lib.cb_span.argtypes=[C.c_int64]*3;lib.cb_span.restype=C.c_bool
   lib.cb_ct_equal.argtypes=[View,View];lib.cb_ct_equal.restype=C.c_uint64
   for bits,typ,res in [(32,C.c_uint32,U32),(64,C.c_uint64,U64)]:
    f=getattr(lib,'cb_ct_select'+str(bits));f.argtypes=[typ]*3;f.restype=typ
    for endian in ['be','le']:
     f=getattr(lib,f'cb_load_{endian}{bits}');f.argtypes=[View,C.c_int64];f.restype=res
     f=getattr(lib,f'cb_store_{endian}{bits}');f.argtypes=[View,C.c_int64,typ];f.restype=C.c_bool
   lib.cb_copy.argtypes=[View,View];lib.cb_copy.restype=C.c_bool
   lib.cb_ct_select_bytes.argtypes=[C.c_uint64,View,View,View];lib.cb_ct_select_bytes.restype=C.c_bool
   cls.libs.append(lib)
 @classmethod
 def tearDownClass(cls):cls.temp.cleanup()

 def test_endian_random_and_edge_vectors_against_python(self):
  randomizer=random.Random(0xB17E5)
  for bits in [32,64]:
   count=bits//8
   values=[0,1,2**bits-1,2**(bits-1),0x01234567]+[randomizer.getrandbits(bits) for _ in range(300)]
   for endian in ['be','le']:
    order='big' if endian=='be' else 'little'
    for value in values:
     expected=b'\xa5\xa5'+value.to_bytes(count,order)+b'\xa5\xa5'
     for lib in self.libs:
      data=array([0xa5]*(count+4))
      self.assertTrue(getattr(lib,f'cb_store_{endian}{bits}')(view(data),2,value))
      self.assertEqual(bytes(data),expected)
      result=getattr(lib,f'cb_load_{endian}{bits}')(view(data),2)
      self.assertTrue(result.ok);self.assertEqual(result.value,value)

 def test_bounds_refuse_without_writes_or_reads(self):
  for lib in self.libs:
   for total,at,count in [(0,0,0),(1,0,1),(1048576,1048576,0),(-1,0,0),(0,-1,0),(0,0,-1),(1048577,0,0),(2**63-1,0,0),(16,2**63-1,1),(16,0,2**63-1)]:
    expected=0<=total<=1048576 and 0<=at<=total and 0<=count<=total-at
    self.assertEqual(lib.cb_span(total,at,count),expected)
   for bits in [32,64]:
    for endian in ['be','le']:
     for capacity in [-1,0,1,3,7,16]:
      for at in [-2**63,-1,0,1,15,16,2**63-1]:
       data=array(range(16));before=bytes(data);v=view(data,count=capacity)
       valid=0<=capacity<=1048576 and 0<=at<=capacity and bits//8<=capacity-at
       loaded=getattr(lib,f'cb_load_{endian}{bits}')(v,at)
       self.assertEqual(loaded.ok,valid)
       self.assertEqual(getattr(lib,f'cb_store_{endian}{bits}')(v,at,0),valid)
       if not valid:self.assertEqual(bytes(data),before);self.assertEqual(loaded.value,0)

 def test_masks_and_selection_against_independent_bit_oracle(self):
  rng=random.Random(340)
  for lib in self.libs:
   for bit in [0,1,2,3,2**63,MAX64]:self.assertEqual(lib.cb_mask(bit),MAX64 if bit&1 else 0)
   for bits in [32,64]:
    maskmax=2**bits-1
    for _ in range(400):
     mask,a,b=(rng.getrandbits(bits) for _ in range(3))
     self.assertEqual(getattr(lib,'cb_ct_select'+str(bits))(mask,a,b),(a&mask)|(b&(maskmax^mask)))
   for n in [0,1,2,31,32,65]:
    a=array(rng.randbytes(n));b=array(bytes(a));out=array([0xa5]*(n+2))
    self.assertEqual(lib.cb_ct_equal(view(a),view(b)),MAX64)
    for i in range(n):
     b[i]^=1;self.assertEqual(lib.cb_ct_equal(view(a),view(b)),0);b[i]^=1
    b=array(rng.randbytes(n))
    for mask in [0,MAX64,0x5555555555555555]:
     self.assertTrue(lib.cb_ct_select_bytes(mask,view(a),view(b),view(out)))
     expected=bytes((x&(mask&255))|(y&(~mask&255)) for x,y in zip(a,b))
     self.assertEqual(bytes(out)[:n],expected);self.assertEqual(bytes(out)[n:],b'\xa5\xa5')

 def test_alias_metadata_bridge_refuses_before_mutation(self):
  for lib in self.libs:
   for source_at,dest_at,count in [(0,0,4),(0,1,4),(1,0,4),(4,4,4)]:
    data=array(range(16));before=bytes(data)
    self.assertFalse(lib.cb_copy(view(data,source_at,count),view(data,dest_at,count)))
    self.assertEqual(bytes(data),before)
    other=array([10]*count)
    self.assertFalse(lib.cb_ct_select_bytes(MAX64,view(data,source_at,count),view(other),view(data,dest_at,count)))
    self.assertEqual(bytes(data),before)
    self.assertFalse(lib.cb_ct_select_bytes(MAX64,view(other),view(data,source_at,count),view(data,dest_at,count)))
    self.assertEqual(bytes(data),before)
   data=array(range(16));self.assertTrue(lib.cb_copy(view(data,0,4),view(data,8,4)))
   self.assertEqual(bytes(data)[8:12],bytes(range(4)))
   empty=array([]);self.assertTrue(lib.cb_copy(view(empty),view(empty)))
   a=array([1,2]);b=array([3]);out=array([5,6]);before=bytes(out)
   self.assertFalse(lib.cb_ct_select_bytes(MAX64,view(a),view(b),view(out)));self.assertEqual(bytes(out),before)
   self.assertEqual(lib.cb_ct_equal(view(a),view(b)),0)

 def test_sanitized_boundary_and_alias_harness(self):
  host=self.dir/'sanitized.c'
  host.write_text('''#include "checked.c"
#include <assert.h>
int main(void) {
 uint8_t x[32],y[16],out[16];
 for(int i=0;i<32;i++)x[i]=(uint8_t)i;
 for(int i=0;i<16;i++){y[i]=(uint8_t)(255-i);out[i]=165;}
 coax_v_u8 a={x,16},b={y,16},o={out,16};
 assert(cb_ct_equal(a,a)==UINT64_MAX);
 assert(cb_ct_select_bytes(UINT64_MAX,a,b,o));
 for(int i=0;i<16;i++)assert(out[i]==x[i]);
 assert(!cb_copy((coax_v_u8){x,8},(coax_v_u8){x+1,8}));
 assert(!cb_ct_select_bytes(0,b,a,(coax_v_u8){x+1,16}));
 assert(cb_copy((coax_v_u8){x,8},(coax_v_u8){x+16,8}));
 assert(!cb_copy((coax_v_u8){x,-1},o));
 assert(!cb_ct_select_bytes(0,(coax_v_u8){x,-1},b,o));
 assert(cb_ct_equal((coax_v_u8){x,-1},(coax_v_u8){x,-1})==0);
 assert(cb_store_be32(o,0,UINT32_MAX));
 assert(cb_load_be32(o,0).value==UINT32_MAX);
 assert(!cb_load_be64(a,INT64_MAX).ok);
 assert(!cb_store_be64(o,INT64_MAX,1));
 assert(!cb_copy((coax_v_u8){x,0},(coax_v_u8){out,-1}));
 return 0;
}''')
  run('cc','-std=c11','-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer',host,'-lm','-o',self.dir/'sanitized')
  run(self.dir/'sanitized')

 def test_word_mask_and_select_codegen_has_no_conditional_branch(self):
  tool=shutil.which('llvm-objdump') or shutil.which('objdump')
  self.assertIsNotNone(tool,'objdump is required for the native codegen gate')
  branch=re.compile(r'\t(b\.[a-z]{2}|cbn?z|tbn?z|j(?!mp)[a-z]{1,3})\s')
  for mode in ['checked','release']:
   obj=self.dir/(mode+'.o')
   run('cc','-std=c11','-O2','-c',self.dir/(mode+'.c'),'-o',obj)
   for name in ['cb_mask','cb_ct_select64','cb_ct_select32']:
    symbol=('_' if platform.system()=='Darwin' else '')+name
    assembly=run(tool,'-d','--disassemble-symbols='+symbol,obj).stdout
    found=re.search(r'<'+symbol+r'>:\n(.*?)(?:\n\n|\Z)',assembly,re.S)
    self.assertIsNotNone(found,f'{symbol} is absent')
    self.assertIsNone(branch.search(found[1]),f'{mode} {name} branches:\n{found[1]}')

 def test_seeded_wrong_selection_is_detected(self):
  # A real independently altered emitted program, not only a model assertion.
  original=(self.dir/'checked.c').read_text()
  definition=re.search(r'uint64_t cb_ct_select64\([^;{}]*\)\s*\{',original)
  self.assertIsNotNone(definition)
  body=definition.end()-1;end=original.index('\n}',body)
  altered=original[:body+1]+'\nreturn l_b;'+original[end:]
  c=self.dir/'seeded.c';c.write_text(altered)
  run('cc','-std=c11','-O2','-fPIC','-shared',c,'-lm','-o',self.dir/'seeded.dylib')
  lib=C.CDLL(str(self.dir/'seeded.dylib'));lib.cb_ct_select64.argtypes=[C.c_uint64]*3;lib.cb_ct_select64.restype=C.c_uint64
  self.assertNotEqual(lib.cb_ct_select64(MAX64,7,9),7)

if __name__=='__main__':unittest.main()
