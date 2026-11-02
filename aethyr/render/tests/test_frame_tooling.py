#!/usr/bin/env python3
"""Failure controls for the range-oracle transport inventory, without a rerun."""
import importlib.util
from pathlib import Path
import unittest
p=Path(__file__).with_name('check_frame.py')
s=importlib.util.spec_from_file_location('frame_qualification',p)
m=importlib.util.module_from_spec(s);s.loader.exec_module(m)
class Inventory(unittest.TestCase):
 def header(self):
  return '\n'.join('struct coax_t_'+name+' { '+' '.join('uint64_t '+field+';' for field in fields)+' };' for name,fields in m.FIELDS.items())
 def test_every_field_is_inventoried(self):
  m.header_inventory(self.header())
  for name,fields in m.FIELDS.items():
   for field in fields:
    with self.assertRaises(AssertionError):m.header_inventory(self.header().replace('uint64_t '+field+';','',1))
  with self.assertRaises(AssertionError):m.header_inventory(self.header().replace('width;','width; uint64_t surprise;',1))
 def test_mathematical_endpoint_does_not_wrap(self):
  self.assertFalse(m.upload_ok([0,2**64-8,16,4,0,4,4]))
  self.assertFalse(m.upload_ok([2**64-4,2**64-8,7,8,0,8,4]))
  self.assertTrue(m.upload_ok([2**64-4,2**64-4,3,0,4,4,4]))
 def test_protocol_is_deterministic(self):
  a=m.cases();b=m.cases();self.assertEqual(m.encode(a),m.encode(b))
  self.assertEqual(len(a),1891)
if __name__=='__main__':unittest.main()
