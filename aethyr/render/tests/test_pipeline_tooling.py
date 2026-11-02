#!/usr/bin/env python3
"""Failure controls for every transported word and the complete ABI inventory."""
import copy
import importlib.util
from pathlib import Path
import unittest
p=Path(__file__).with_name('check_pipeline.py');s=importlib.util.spec_from_file_location('pipeline_qualification',p)
m=importlib.util.module_from_spec(s);s.loader.exec_module(m)
class PipelineTooling(unittest.TestCase):
 def test_every_pixel_word_and_tail_is_observed(self):
  r,pixels,_=m.oracle(0);data={'result':[r[0],100,*r[1:]],'pixels':[[m.bits(x) for x in pixel] for pixel in pixels]}
  m.verify(data,0)
  for pixel in range(16):
   for word in range(9):
    broken=copy.deepcopy(data);broken['pixels'][pixel][word]^=1
    with self.assertRaises(AssertionError):m.verify(broken,0)
  for field in [0,2,3,4,5]:
   broken=copy.deepcopy(data);broken['result'][field]+=1
   with self.assertRaises(AssertionError):m.verify(broken,0)
 def test_header_refuses_missing_added_and_reordered_fields(self):
  header='\n'.join('struct coax_t_'+name+' { '+' '.join('uint32_t '+x+';' for x in fields)+' };' for name,fields in m.FIELDS.items())
  m.header_inventory(header)
  for name,fields in m.FIELDS.items():
   for field in fields:
    with self.assertRaises(AssertionError):m.header_inventory(header.replace('uint32_t '+field+';','',1))
  with self.assertRaises(AssertionError):m.header_inventory(header.replace('uint32_t r;','uint32_t extra; uint32_t r;',1))
  with self.assertRaises(AssertionError):m.header_inventory(header.replace('uint32_t r; uint32_t g;','uint32_t g; uint32_t r;',1))
 def test_work_boundary_and_short_view_are_exact(self):
  for scenario,short in [(6,False),(0,True)]:
   r,pixels,steps=m.oracle(scenario,short);data={'result':[r[0],steps,*r[1:]],'pixels':[[m.bits(x) for x in pixel] for pixel in pixels]}
   m.verify(data,scenario,short)
   data['result'][1]+=1
   with self.assertRaises(AssertionError):m.verify(data,scenario,short)
if __name__=='__main__':unittest.main()
