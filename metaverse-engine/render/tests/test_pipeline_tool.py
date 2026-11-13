#!/usr/bin/env python3
# Firm reason: 2 (hand-pinned binary32 depth/target words and helper-lane masks for the independently authored pipeline equations; process orchestration is Coax).
"""Independent depth-only equation answers; no process or product driver."""
import importlib.util
import json
from pathlib import Path
import unittest

SOURCE=Path(__file__).with_name('check_pipeline.py')
spec=importlib.util.spec_from_file_location('pipeline_tool',SOURCE)
tool=importlib.util.module_from_spec(spec);spec.loader.exec_module(tool)

class PipelineToolTests(unittest.TestCase):
 def test_depth_only_oracle_observes_all_targets_and_helper_mask(self):
  clear=[1065353216,1040187392,1048576000,1056964608,1061158912,1048576000,1056964608,1061158912,1065353216]
  for scenario,mask,counters in [(15,{0,1,2,3},[0,2,2,4,4]),(16,{0,2},[0,2,2,2,2])]:
   expected,pixels,steps=tool.oracle(scenario)
   self.assertEqual(expected,counters)
   for k,pixel in enumerate(pixels):
    words=list(clear)
    if k in mask:words[0]=1056964610
    self.assertEqual([tool.bits(x) for x in pixel],words)
   data={'result':[0,100,*counters[1:]],'pixels':[[tool.bits(x) for x in p] for p in pixels]}
   tool.verify(data,scenario)
   for pixel,component in [(0,1),(0,0),(1,0)]:
    bad=json.loads(json.dumps(data));bad['pixels'][pixel][component]^=1
    with self.assertRaises(AssertionError):tool.verify(bad,scenario)


if __name__=='__main__':unittest.main()
