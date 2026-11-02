#!/usr/bin/env python3
"""Tooling-only B2 metadata/process cleanup checks; never invokes Coax or C."""
import argparse
import contextlib
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import shutil
import sys
import tempfile
import time
from unittest import mock
from b2_support import ROOT,H,bounded_run,file_sha,sha,strip_regions
from b2_helper_composition import validate_checker_reuse
import b2_helper_composition as suite

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--out',required=True,type=Path);a=p.parse_args();out=a.out.resolve();out.mkdir(parents=True,exist_ok=False);checks=[]
 raw='// é before the check\nkeep :: 1;\n#test "utf8 é" { #assert true; }\ntail :: 2;\n'.encode();projected,regions=strip_regions(raw);encoded=projected.encode();assert len(encoded)==len(raw)
 for f in regions:
  assert sha(raw[f['start']:f['end']])==f['sha256']
  assert all(c in (10,32) for c in encoded[f['start']:f['end']])
 assert encoded[:regions[0]['start']]==raw[:regions[0]['start']] and encoded[regions[0]['end']:]==raw[regions[0]['end']:];checks.append('UTF8 byte ranges preserve all other bytes and total extent')
 assert H==ROOT/'coax/bin';checks.append('default native artifact directory is installed checkout, not private temporary evidence')
 with tempfile.TemporaryDirectory(prefix='b2-reuse-metadata-') as temporary:
  base=Path(temporary);generated=base/'generated';generated.mkdir();files={ROOT/'coax/tests/b2_support.py':(ROOT/'coax/tests/b2_support.py').read_bytes()}
  for name in ['checker.c','checker.h']:(generated/name).write_text('retained '+name+'\n')
  record=dict(status='passed',inputs_unchanged=True,tools_unchanged=True,artifacts_unchanged=True,tools_before=dict(compiler_sha256=file_sha(sys.executable)),runtime_projection=[{'scope':'literal metadata unit fixture'}],source_before={str(k.relative_to(ROOT)):dict(bytes=len(v),sha256=sha(v)) for k,v in files.items()},artifacts={'generated/'+n:file_sha(generated/n) for n in ['checker.c','checker.h']})
  (base/'qualification.json').write_text(json.dumps(record));validate_checker_reuse(generated,files,Path(sys.executable))
  changed={k:v+b'changed' for k,v in files.items()}
  try:validate_checker_reuse(generated,changed,Path(sys.executable))
  except ValueError:pass
  else:raise AssertionError('changed checker closure accepted')
  (generated/'checker.h').write_text('damaged header')
  try:validate_checker_reuse(generated,files,Path(sys.executable))
  except ValueError:pass
  else:raise AssertionError('damaged generated header accepted')
  checks.append('retained checker reuse rejects source and generated header drift')
 native=out/'literal-native-input';native.mkdir()
 for name in ['lexer.c','lexer.h','parser.c','parser.h']:shutil.copyfile(ROOT/'coax/bootstrap'/name,native/name)
 for name in ['checker.c','checker.h']:(native/name).write_text('literal metadata-only '+name+'\n')
 observed=out/'native-drift-driver';files={ROOT/'coax/tests/b2_support.py':(ROOT/'coax/tests/b2_support.py').read_bytes()}
 def drift(*args):
  (native/'lexer.c').write_text('changed after admission')
  return {'scope':'literal unit fixture, not a real checker emission'}
 argv=['b2-helper-tooling','--compiler',sys.executable,'--out',str(observed),'--native-dir',str(native),'--native-source-root',str(ROOT),'--checker-generated-dir',str(native),'--emit-only']
 with mock.patch('sys.argv',argv),mock.patch.object(suite,'closure',return_value=files),mock.patch.object(suite,'validate_checker_reuse',side_effect=drift),contextlib.redirect_stdout(open(os.devnull,'w')):
  try:suite.main()
  except RuntimeError as exc:assert 'native lexer/parser artifacts changed' in str(exc)
  else:raise AssertionError('final native input drift returned success')
 failed=json.loads((observed/'qualification.json').read_text());assert failed['status']=='failed' and not failed['commands'];checks.append('final native C/header drift fails driver and retained record without executing Coax/C')
 child_source="import os,subprocess,sys,time; from pathlib import Path; c=subprocess.Popen([sys.executable,'-c','import time;time.sleep(60)']); Path(sys.argv[1]).write_text(str(os.getpid())+' '+str(c.pid)); print('owned-group-prefix',flush=True);time.sleep(60)"
 def absent(pid):
  until=time.monotonic()+2
  while time.monotonic()<until:
   try:os.kill(pid,0)
   except ProcessLookupError:return True
   time.sleep(.01)
  return False
 for mode in ['timeout','interrupt']:
  pidfile=out/(mode+'.pids');old=None
  if mode=='interrupt':
   def interrupt(signum,frame):raise KeyboardInterrupt('owned tooling interruption')
   old=signal.signal(signal.SIGALRM,interrupt);signal.setitimer(signal.ITIMER_REAL,.3)
  try:
   bounded_run([sys.executable,'-c',child_source,str(pidfile)],cwd=ROOT,timeout=.3 if mode=='timeout' else 5)
  except (subprocess.TimeoutExpired,KeyboardInterrupt) as exc:
   assert type(exc) is (subprocess.TimeoutExpired if mode=='timeout' else KeyboardInterrupt)
   (out/(mode+'.stdout')).write_bytes(exc.output);(out/(mode+'.stderr')).write_bytes(exc.stderr);assert b'owned-group-prefix' in exc.output
  else:raise AssertionError('bounded runner unexpectedly succeeded')
  finally:
   if old is not None:signal.setitimer(signal.ITIMER_REAL,0);signal.signal(signal.SIGALRM,old)
  parent,child=map(int,pidfile.read_text().split());assert absent(parent) and absent(child)
  try:os.waitpid(parent,os.WNOHANG)
  except ChildProcessError:pass
  else:raise AssertionError('group leader was not reaped')
  checks.append(mode+' kills owned parent/descendant group, reaps leader and retains partial logs')
 receipt=dict(schema=1,status='passed',scope=__doc__,checks=checks,tooling={str(x.relative_to(ROOT)):file_sha(x) for x in [Path(__file__),ROOT/'coax/tests/b2_support.py',ROOT/'coax/tests/b2_helper_composition.py']},artifacts={str(x.relative_to(out)):file_sha(x) for x in out.rglob('*') if x.is_file()});(out/'qualification.json').write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps(dict(status='passed',checks=len(checks),receipt=str(out/'qualification.json'))))
if __name__=='__main__':main()
