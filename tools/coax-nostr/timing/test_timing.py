#!/usr/bin/env python3
"""Independent raw-file/statistics and floating-point refusal checks."""
import ctypes as C
import json
import math
from pathlib import Path
import random
import struct
import tempfile
import unittest
import run as harness

class Moment(C.Structure):_fields_=[('n',C.c_int64),('mean',C.c_double),('m2',C.c_double)]
class Stats(C.Structure):_fields_=[('a',Moment),('b',Moment),('error',C.c_bool)]
class Result(C.Structure):_fields_=[('status',C.c_int64),('n0',C.c_int64),('n1',C.c_int64),('mean0',C.c_double),('mean1',C.c_double),('variance0',C.c_double),('variance1',C.c_double),('t',C.c_double),('degenerate',C.c_bool)]
class View(C.Structure):_fields_=[('data',C.POINTER(Stats)),('count',C.c_int64)]

class Timing(unittest.TestCase):
 @classmethod
 def setUpClass(cls):
  cls.temp=tempfile.TemporaryDirectory(prefix='coax-timing-check-');cls.directory=Path(cls.temp.name);cls.libs=[]
  for name,mode in [('checked',[]),('release',['--release'])]:
   source=cls.directory/(name+'.c');source.write_text(harness.run(harness.ROOT/'coax/bin/coax','c',harness.HERE/'model.coax','--library',*mode))
   target=cls.directory/(name+'.dylib');harness.run('cc','-std=c11','-O2','-shared','-fPIC',source,'-lm','-o',target)
   lib=C.CDLL(str(target));lib.ct_result.argtypes=[Stats];lib.ct_result.restype=Result
   lib.ct_observe.argtypes=[View,C.c_int64,C.c_double];lib.ct_observe.restype=C.c_bool;cls.libs.append(lib)
 @classmethod
 def tearDownClass(cls):cls.temp.cleanup()
 def test_nan_infinity_and_bad_classes_never_return_favorable_results(self):
  for lib in self.libs:
   for value in [math.nan,math.inf,-math.inf,-1,1000000001]:
    stats=Stats();self.assertFalse(lib.ct_observe(View(C.pointer(stats),1),0,value));self.assertTrue(stats.error);self.assertEqual(stats.a.n,0);self.assertEqual(lib.ct_result(stats).status,4)
   for field,values in [('mean',[math.nan,math.inf,-math.inf,-1,1000000001]),('m2',[math.nan,math.inf,-math.inf,-1,math.nextafter(500000.0*1e18,math.inf)])]:
    for value in values:
     stats=Stats(Moment(500000,1,1),Moment(500000,1,1),False);setattr(stats.a,field,value)
     self.assertEqual(lib.ct_result(stats).status,4)
   stats=Stats();self.assertFalse(lib.ct_observe(View(C.pointer(stats),1),2,1));self.assertEqual(lib.ct_result(stats).status,4)
 def test_emitted_online_moments_and_welch_match_independent_two_pass_oracle(self):
  rng=random.Random(20261028);groups=[[rng.randrange(100,200) for _ in range(2048)],[rng.randrange(110,210) for _ in range(2048)]]
  for lib in self.libs:
   stats=Stats()
   for group,values in enumerate(groups):
    for value in values:self.assertTrue(lib.ct_observe(View(C.pointer(stats),1),group,value))
   for got,values in zip([stats.a,stats.b],groups):
    mean=math.fsum(values)/len(values);m2=math.fsum((v-mean)**2 for v in values)
    self.assertAlmostEqual(got.mean,mean,places=9);self.assertAlmostEqual(got.m2,m2,places=6)
   # Populate an independently computed million-sample summary, not a fake
   # observation claim; this specifically checks emitted Welch arithmetic.
   a=Moment(500000,151,499999*867.0);b=Moment(500000,159,499999*901.0)
   result=lib.ct_result(Stats(a,b,False));expected=(151-159)/math.sqrt(867/500000+901/500000)
   self.assertTrue(math.isclose(result.t,expected,rel_tol=1e-13));self.assertEqual(result.status,1)
 def test_raw_version_size_classes_ticks_and_counts_are_bounded(self):
  path=self.directory/'raw.bin'
  valid=b'CT01'+struct.pack('<Q',4)+b''.join(struct.pack('<BQ',i%2,100+i)for i in range(4))
  path.write_bytes(valid);self.assertEqual(harness.decode(path)['status'],3)
  cases=[b'',b'BAD!'+valid[4:],valid[:-1],valid+b'X',b'CT01'+struct.pack('<Q',2000001),valid[:12]+struct.pack('<BQ',2,1)+valid[21:],valid[:12]+struct.pack('<BQ',0,1000000001)+valid[21:],valid[:12]+b''.join(struct.pack('<BQ',0,1)for _ in range(4))]
  for data in cases:
   path.write_bytes(data)
   with self.assertRaises(ValueError):harness.decode(path)
 def test_planted_synthetic_leak_is_detected_and_classifier_fault_is_caught(self):
  groups=[[100,101]*250000,[110,111]*250000];expected=harness.summarize(groups)
  self.assertEqual(expected['status'],1);self.assertGreater(abs(expected['t']),10)
  path=self.directory/'synthetic.bin'
  with path.open('wb')as out:
   out.write(b'CT01'+struct.pack('<Q',1000000))
   for group,base in [(0,100),(1,110)]:out.write((struct.pack('<BQ',group,base)+struct.pack('<BQ',group,base+1))*250000)
  faulty={**expected,'status':2}
  with self.assertRaisesRegex(AssertionError,'status disagreement'):harness.verify(path,faulty)
 def test_retained_actual_browser_raw_rows_match_coax_statistics(self):
  directory=harness.ROOT/'coax/bin/nostr-timing-2026-10-28';path=directory/'browser-report.json'
  self.assertTrue(path.exists(),'Run the explicit headless browser fixture first; missing evidence is a failure')
  report=json.loads(path.read_text());self.assertTrue(report['crossOriginIsolated']);self.assertEqual(report['samples'],1000000)
  for row in report['runs']:
   independent=harness.verify(directory/('browser-'+row['fixture']+'.bin'),row['coax']);self.assertEqual(independent['n0'],500000);self.assertEqual(independent['n1'],500000)
   row['independent']=independent;row['raw_sha256']=harness.sha(directory/('browser-'+row['fixture']+'.bin'))
  path.write_text(json.dumps(report,indent=2,sort_keys=True)+'\n')
if __name__=='__main__':unittest.main()
