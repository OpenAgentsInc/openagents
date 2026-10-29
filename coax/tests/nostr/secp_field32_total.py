#!/usr/bin/env python3
"""Independent full-width integer oracle for total reductions."""
import ctypes as C
import hashlib
import json
import os
from pathlib import Path
import random
import tempfile
import unittest
import secp_field as base
from secp_field32 import Field32,pack,number
from secp_field_mul import file_sha
ROOT=base.ROOT
SOURCE=ROOT/'coax/tests/nostr/secp_field32_total_adapter.coax'
COUNT=100000
class TotalField(unittest.TestCase):
 @classmethod
 def setUpClass(cls):
  cls.temp=tempfile.TemporaryDirectory(prefix='coax-total-field-');directory=Path(cls.temp.name);cls.libs=[]
  paths=[SOURCE,Path(__file__),ROOT/'coax/lib/crypto/secp_field32_total.coax',ROOT/'coax/lib/crypto/secp_field32.coax',ROOT/'coax/lib/crypto/bytes.coax',ROOT/'coax/tests/nostr/secp_field32.py',ROOT/'coax/tests/nostr/secp_field.py',ROOT/'coax/tests/nostr/secp_field_mul.py',ROOT/'coax/tests/quiet_crash.py',ROOT/'tools/coax-nostr/fuzz/oracle.py']
  cls.report={'schema':'coax.total-field-oracle.v1','scope':'public full-width integer fixtures; no target or production approval','compiler_sha256':file_sha(base.COMPILER),'sources':{str(p.relative_to(ROOT)):file_sha(p) for p in paths},'seed':20261029,'random_pairs':COUNT,'modes':[]}
  for name,flags in [('checked',[]),('release',['--release'])]:
   c=directory/(name+'.c');c.write_text(base.run(base.COMPILER,'c',SOURCE,'--library',*flags).stdout)
   dylib=directory/(name+'.dylib');base.run('cc','-std=c11','-O2','-shared','-fPIC',c,'-lm','-o',dylib);lib=C.CDLL(str(dylib))
   for op in ('add','mul'):
    fn=getattr(lib,'sf32_total_test_'+op);fn.argtypes=[Field32,Field32];fn.restype=Field32
   cls.libs.append((name,lib));cls.report['modes'].append({'mode':name,'c_sha256':file_sha(c),'library_sha256':file_sha(dylib)})
 @classmethod
 def tearDownClass(cls):
  if os.environ.get('COAX_TOTAL_FIELD_REPORT'):Path(os.environ['COAX_TOTAL_FIELD_REPORT']).write_text(json.dumps(cls.report,indent=2)+'\n')
  cls.temp.cleanup()
 def test_random_and_full_width_boundaries(self):
  n=1<<256;edges=sorted(set([0,1,2,base.P-1,base.P,base.P+1,n-2,n-1]+[1<<i for i in range(256)]))
  rng=random.Random(20261029);cases=[(a,b) for a in edges for b in (0,1,base.P-1,base.P,n-1)]+[(rng.getrandbits(256),rng.getrandbits(256)) for _ in range(COUNT)]
  for label,lib in self.libs:
   for i,(a,b) in enumerate(cases):
    for op,want in [('add',(a+b)%base.P),('mul',(a*b)%base.P)]:
     got=number(getattr(lib,'sf32_total_test_'+op)(pack(a),pack(b)))
     self.assertLess(got,base.P,(label,i,op));self.assertEqual(got,want,(label,i,op))
  self.report['pairs_per_mode']=len(cases);self.report['operations_per_pair']=2
if __name__=='__main__':unittest.main()
