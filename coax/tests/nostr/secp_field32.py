#!/usr/bin/env python3
"""Independent bigint checks for the experimental canonical 8x32 field."""
import ctypes as C
import json
import os
from pathlib import Path
import random
import tempfile
import unittest
import secp_field as base
from secp_field_mul import file_sha
ROOT=base.ROOT
SOURCE=ROOT/'coax/tests/nostr/secp_field32_adapter.coax'
COUNT=int(os.environ.get('COAX_FIELD32_COUNT','100000'))
if not 1<=COUNT<=100000:raise ValueError('random-pair budget is 1..100000')
class Field32(C.Structure):_fields_=[('limb',C.c_uint32*8)]
def pack(value):return Field32((C.c_uint32*8)(*[(value>>(32*i))&0xffffffff for i in range(8)]))
def number(value):return sum(int(x)<<(32*i) for i,x in enumerate(value.limb))
class Arithmetic(unittest.TestCase):
 @classmethod
 def setUpClass(cls):
  cls.temp=tempfile.TemporaryDirectory(prefix='coax-secp-field32-');directory=Path(cls.temp.name);cls.libs=[]
  cls.report={'schema':'coax.secp-field32.v1','scope':'canonical field arithmetic only; no secret-use or target constant-time approval','compiler':base.run(base.COMPILER,'version').stdout.strip(),'compiler_sha256':file_sha(base.COMPILER),'seed':20261029,'random_pairs':COUNT,'sources':{str(p.relative_to(ROOT)):file_sha(p) for p in [SOURCE,Path(__file__),ROOT/'coax/lib/crypto/secp_field32.coax',ROOT/'coax/lib/crypto/bytes.coax']},'modes':[]}
  for label,flags in [('checked',[]),('release',['--release'])]:
   c=directory/(label+'.c');c.write_text(base.run(base.COMPILER,'c',SOURCE,'--library',*flags).stdout)
   libpath=directory/(label+'.dylib');base.run('cc','-std=c11','-O2','-shared','-fPIC',c,'-lm','-o',libpath);lib=C.CDLL(str(libpath))
   for op in ['add','sub','mul']:
    fn=getattr(lib,'sf32_test_'+op);fn.argtypes=[Field32,Field32];fn.restype=Field32
   cls.libs.append((label,lib));cls.report['modes'].append({'mode':label,'generated_c_sha256':file_sha(c),'artifact_sha256':file_sha(libpath)})
 @classmethod
 def tearDownClass(cls):
  if os.environ.get('COAX_FIELD32_REPORT'):Path(os.environ['COAX_FIELD32_REPORT']).write_text(json.dumps(cls.report,indent=2,sort_keys=True)+'\n')
  cls.temp.cleanup()
 def test_canonical_random_and_boundaries(self):
  edges=sorted(set([0,1,2,base.P-1,base.P-2,base.P-2**128]+[1<<i for i in range(256)]))
  rng=random.Random(20261029);cases=[(a,b) for a in edges for b in [0,1,2,base.P-1,base.P-2,base.P-2**128]]+[(rng.randrange(base.P),rng.randrange(base.P)) for _ in range(COUNT)]
  for label,lib in self.libs:
   for index,(a,b) in enumerate(cases):
    for op,want in [('add',(a+b)%base.P),('sub',(a-b)%base.P),('mul',(a*b)%base.P)]:
     got=number(getattr(lib,'sf32_test_'+op)(pack(a),pack(b)))
     self.assertLess(got,base.P,(label,index,op));self.assertEqual(got,want,(label,index,op))
  self.report['pairs_per_mode']=len(cases);self.report['operations_per_pair']=3
if __name__=='__main__':unittest.main()
