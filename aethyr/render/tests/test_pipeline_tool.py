#!/usr/bin/env python3
"""Bounded tooling fixtures: authored depth receipts and real child cleanup."""
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
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

 def test_nonzero_child_cannot_be_reported_as_success(self):
  with self.assertRaisesRegex(RuntimeError,'command failed 7'):
   tool.run_process([sys.executable,'-c','import sys;sys.exit(7)'],timeout=2)

 def live(self,pid):
  result=subprocess.run(['ps','-o','stat=','-p',str(pid)],capture_output=True,text=True,timeout=2)
  return result.returncode==0 and bool(result.stdout.strip()) and not result.stdout.strip().startswith('Z')

 def child_code(self,path):
  return "import os,time,json;from pathlib import Path;child=os.fork();"+f"Path({str(path)!r}).write_text(json.dumps([os.getpid(),child])) if child else None;"+"time.sleep(60)"

 def wait_pids(self,path):
  deadline=time.monotonic()+5
  while not path.exists() and time.monotonic()<deadline:time.sleep(.01)
  self.assertTrue(path.exists(),'fixture did not start its child')
  return json.loads(path.read_text())

 def assert_dead(self,pids):
  deadline=time.monotonic()+2
  while any(self.live(pid) for pid in pids) and time.monotonic()<deadline:time.sleep(.01)
  self.assertFalse(any(self.live(pid) for pid in pids),f'owned processes still live: {pids}')

 def test_timeout_removes_process_group(self):
  with tempfile.TemporaryDirectory() as directory:
   path=Path(directory)/'pids.json'
   with self.assertRaises(subprocess.TimeoutExpired):
    tool.run_process([sys.executable,'-c',self.child_code(path)],timeout=.2)
   self.assert_dead(self.wait_pids(path))

 def test_interrupts_remove_process_group_before_success(self):
  for signum in [signal.SIGINT,signal.SIGTERM]:
   with self.subTest(signum=signum),tempfile.TemporaryDirectory() as directory:
    path=Path(directory)/'pids.json';passed=Path(directory)/'passed'
    code='\n'.join(['import importlib.util,signal,sys',f'spec=importlib.util.spec_from_file_location("tool",{str(SOURCE)!r})','m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)','signal.signal(signal.SIGTERM,m.interrupted)',f'm.run_process([sys.executable,"-c",{self.child_code(path)!r}],timeout=10)',f'open({str(passed)!r},"w").write("passed")'])
    wrapper=subprocess.Popen([sys.executable,'-c',code],stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True)
    pids=[]
    try:
     pids=self.wait_pids(path);wrapper.send_signal(signum);wrapper.communicate(timeout=5)
     self.assertNotEqual(wrapper.returncode,0);self.assertFalse(passed.exists());self.assert_dead(pids)
    finally:
     if wrapper.poll() is None:os.killpg(wrapper.pid,signal.SIGKILL);wrapper.communicate(timeout=2)
     for pid in pids:
      if self.live(pid):os.kill(pid,signal.SIGKILL)

if __name__=='__main__':unittest.main()
