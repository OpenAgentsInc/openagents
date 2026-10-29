#!/usr/bin/env python3
"""macOS-only lock-inheritance counterexamples; not a service or provider."""
import os,subprocess,tempfile,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3];src=ROOT/'coax/tests/nostr/fi_flock_restart_probe.c'
with tempfile.TemporaryDirectory(prefix='fi-flock-restart-') as td:
 d=Path(td);exe=d/'probe';subprocess.run(['cc','-std=c11','-O2','-Wall','-Wextra','-Werror',str(src),'-o',str(exe)],check=True)
 def run(mode,lock,marker):return subprocess.run([str(exe),mode,str(lock),str(marker)],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=5).returncode
 def wait_marker(marker):
  end=time.monotonic()+2
  while time.monotonic()<end:
   if marker.exists():
    value=marker.read_text()
    if value.endswith('\n') and value[:-1].isdigit():return int(value[:-1])
   time.sleep(.002)
  raise AssertionError('resolver marker absent')
 def wait_gone(pid):
  end=time.monotonic()+5
  while time.monotonic()<end:
   try:os.kill(pid,0)
   except ProcessLookupError:return
   time.sleep(.005)
  raise AssertionError('test resolver did not exit naturally')
 for mode in ('before','early','after'):
  lock=d/(mode+'.lock');marker=d/(mode+'.pid');assert run(mode,lock,marker)==0
  if mode=='before':assert run('probe',lock,marker)==0;continue
  assert run('probe',lock,marker)==23;pid=wait_marker(marker);assert run('probe',lock,marker)==23
  wait_gone(pid);assert run('probe',lock,marker)==0
 # Negative controls: a mutable pathname or explicit child unlock defeats a
 # lock-based restart boundary. Never use those conditions as authority.
 for mode in ('replace','unlock'):
  lock=d/(mode+'.lock');marker=d/(mode+'.pid');assert run('unlock' if mode=='unlock' else 'after',lock,marker)==0
  pid=wait_marker(marker)
  if mode=='replace':assert run('probe',lock,marker)==23;lock.unlink()
  assert run('probe',lock,marker)==0;os.kill(pid,0);wait_gone(pid)
 print('macOS inherited lock survived service/supervisor exit; pathname replacement and explicit unlock defeated it as expected')
