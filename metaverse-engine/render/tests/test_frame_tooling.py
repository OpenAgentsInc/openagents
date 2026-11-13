#!/usr/bin/env python3
# Firm reason: 2 (hand-pinned unbounded-integer endpoint cases for the independent upload oracle; ABI/protocol orchestration is Coax)
"""Independent mathematical endpoint cases; no product or compiler execution."""
import importlib.util
from pathlib import Path
import unittest
p=Path(__file__).with_name('check_frame.py')
s=importlib.util.spec_from_file_location('frame_qualification',p)
m=importlib.util.module_from_spec(s);s.loader.exec_module(m)
class EndpointOracle(unittest.TestCase):
 def test_mathematical_endpoint_does_not_wrap(self):
  self.assertFalse(m.upload_ok([0,2**64-8,16,4,0,4,4]))
  self.assertFalse(m.upload_ok([2**64-4,2**64-8,7,8,0,8,4]))
  self.assertTrue(m.upload_ok([2**64-4,2**64-4,3,0,4,4,4]))
if __name__=='__main__':unittest.main()
