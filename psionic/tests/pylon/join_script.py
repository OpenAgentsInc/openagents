#!/usr/bin/env python3
"""Offline lifecycle test of the actual join script; no relay or secrets."""
import os,pathlib,shutil,signal,subprocess,tempfile,time,unittest
ROOT=pathlib.Path(__file__).resolve().parents[3]
ENGINE='''#!/usr/bin/env python3
import socket,sys
p=int(sys.argv[sys.argv.index('--port')+1]);s=socket.socket();s.bind(('127.0.0.1',p));s.listen()
while True:
 c,_=s.accept();c.close()
'''
PYLON='''#!/usr/bin/env python3
import os,pathlib,re,sys,time
args=sys.argv[1:];path=pathlib.Path(args[args.index('--test-key-label-file')+1]);label=path.read_text()
assert re.fullmatch('[0-9a-f]{64}',label) and (path.stat().st_mode&0o777)==0o600
assert label not in args and '--test-key-label' not in args
print('pylon: TEST-ONLY key, signed in Coax; pubkey '+'1'*64,flush=True)
print('pylon: npub npub1fixture',flush=True)
print('pylon: online, waiting for jobs',flush=True)
stop=pathlib.Path(args[args.index('--stop-file')+1])
while not stop.exists():time.sleep(.05)
'''
class Join(unittest.TestCase):
 def test_stop_during_build_or_warmup_never_starts_a_pylon(self):
  for phase in ('build','warmup'):
   with self.subTest(phase=phase),tempfile.TemporaryDirectory(prefix='join-stop-stage-') as t:
    d=pathlib.Path(t);r=d/'source';state=d/'install';marker=d/'waiting';launched=d/'pylon-started'
    for folder in ['coax/bin','coax/platform','psionic/platform','psionic/fixtures/pylon','psionic/fixtures/models/mammoth','psionic/tests']:(r/folder).mkdir(parents=True,exist_ok=True)
    for file in ['psionic/fixtures/pylon/psionic-toy.selfcheck','psionic/fixtures/models/mammoth/inventory.h','psionic/tests/elephant_manifest.py','psionic/tests/serve_build_id.py']:(r/file).write_text('')
    (r/'coax/provenance.sh').write_text('#!/bin/sh\nexit 0\n');(r/'coax/provenance.sh').chmod(0o755)
    (r/'coax/bin/coax').write_text('#!/usr/bin/env python3\nprint(\'{"sources":[]}\')\n');(r/'coax/bin/coax').chmod(0o755)
    wait='import os,pathlib,time\npathlib.Path('+repr(str(marker))+').write_text(str(os.getpid()))\ntime.sleep(60)\n'
    pylon='#!/usr/bin/env python3\nimport pathlib\npathlib.Path('+repr(str(launched))+').touch()\n'
    for script,name,text in [('pylon-build.sh','pylon',pylon),('serve-build.sh','psionic-serve','#!/usr/bin/env python3\n'+wait)]:
     code='#!/usr/bin/env python3\nimport pathlib,sys\n'
     if phase=='build' and name=='pylon':code+=wait
     code+='p=pathlib.Path(sys.argv[1])/'+repr(name)+'\np.parent.mkdir(parents=True,exist_ok=True)\np.write_text('+repr(text)+')\np.chmod(0o755)\n'
     path=r/'psionic/platform'/script;path.write_text(code);path.chmod(0o755)
    env={**os.environ,'OPENAGENTS_PYLON_HOME':str(state),'COAX_CC':shutil.which('gcc') or shutil.which('cc'),'CXX':shutil.which('g++') or shutil.which('c++')}
    with (d/'out').open('w') as out,(d/'err').open('w') as err:p=subprocess.Popen(['bash',str(ROOT/'scripts/join-pylon.sh'),'--source',str(r)],env=env,stdout=out,stderr=err)
    try:
     deadline=time.monotonic()+10
     while not marker.exists() and time.monotonic()<deadline:
      if p.poll() is not None:self.fail((d/'out').read_text()+(d/'err').read_text())
      time.sleep(.05)
     self.assertTrue(marker.exists())
     stop=subprocess.run(['bash',str(ROOT/'scripts/join-pylon.sh'),'--stop'],env=env,capture_output=True,text=True,timeout=5);self.assertEqual(stop.returncode,0,stop.stderr)
     self.assertEqual(p.wait(timeout=5),0,(d/'err').read_text());self.assertFalse(launched.exists());self.assertFalse((state/'state/join.lock').exists())
     with self.assertRaises(ProcessLookupError):os.kill(int(marker.read_text()),0)
    finally:
     if p.poll() is None:p.send_signal(signal.SIGTERM);p.wait(timeout=10)
 def test_private_label_foreground_start_stop_and_no_secret_argv(self):
  with tempfile.TemporaryDirectory(prefix='join-script-') as t:
   d=pathlib.Path(t);r=d/'source with spaces';home=d/'home';home.mkdir();state=d/'install with spaces'
   for p in ['coax/bin','psionic/platform','psionic/fixtures/pylon','psionic/fixtures/models/mammoth','psionic/tests','coax/platform']:(r/p).mkdir(parents=True,exist_ok=True)
   (r/'psionic/fixtures/models/mammoth/inventory.h').write_text('')
   (r/'psionic/fixtures/pylon/psionic-toy.selfcheck').write_text('{}')
   (r/'psionic/tests/elephant_manifest.py').write_text('');(r/'psionic/tests/serve_build_id.py').write_text('')
   (r/'coax/provenance.sh').write_text('#!/bin/sh\nexit 0\n');(r/'coax/provenance.sh').chmod(0o755)
   (r/'coax/bin/coax').write_text('#!/usr/bin/env python3\nimport json\nprint(json.dumps({"sources":[]}))\n');(r/'coax/bin/coax').chmod(0o755)
   for script,name,text in [('pylon-build.sh','pylon',PYLON),('serve-build.sh','psionic-serve',ENGINE)]:
    p=r/'psionic/platform'/script;p.write_text('#!/usr/bin/env python3\nimport pathlib,sys\np=pathlib.Path(sys.argv[1])/'+repr(name)+'\np.parent.mkdir(parents=True,exist_ok=True)\np.write_text('+repr(text)+')\np.chmod(0o755)\n');p.chmod(0o755)
   env={**os.environ,'HOME':str(home),'OPENAGENTS_PYLON_HOME':str(state),'COAX_CC':shutil.which('gcc') or shutil.which('cc'),'CXX':shutil.which('g++') or shutil.which('c++')}
   output=d/'stdout';err=d/'stderr'
   with output.open('w') as o,err.open('w') as e:p=subprocess.Popen(['bash',str(ROOT/'scripts/join-pylon.sh'),'--source',str(r),'--slug','fixture'],env=env,stdout=o,stderr=e)
   try:
    deadline=time.time()+20
    while time.time()<deadline and 'Pylon online:' not in output.read_text():
     if p.poll() is not None:self.fail(output.read_text()+err.read_text())
     time.sleep(.1)
    self.assertIn('Pylon online: fixture',output.read_text());self.assertIn('npub1fixture',output.read_text())
    label=(state/'state/demo-label').read_text();self.assertEqual(len(label),64);self.assertNotIn(label,output.read_text()+err.read_text());self.assertEqual((state/'state/demo-label').stat().st_mode&0o777,0o600)
    stop=subprocess.run(['bash',str(ROOT/'scripts/join-pylon.sh'),'--stop'],env=env,capture_output=True,text=True);self.assertEqual(stop.returncode,0,stop.stderr)
    self.assertEqual(p.wait(timeout=10),0);self.assertFalse((state/'state/join.lock').exists())
    with output.open('w') as o,err.open('w') as e:p=subprocess.Popen(['bash',str(ROOT/'scripts/join-pylon.sh'),'--source',str(r)],env=env,stdout=o,stderr=e)
    deadline=time.time()+20
    while time.time()<deadline and 'Pylon online:' not in output.read_text():
     if p.poll() is not None:self.fail(output.read_text()+err.read_text())
     time.sleep(.1)
    self.assertIn('Pylon online: fixture',output.read_text());self.assertEqual((state/'state/demo-label').read_text(),label)
    stop=subprocess.run(['bash',str(ROOT/'scripts/join-pylon.sh'),'--model','psionic-toy','--stop'],env=env,capture_output=True,text=True);self.assertEqual(stop.returncode,0,stop.stderr)
    self.assertEqual(p.wait(timeout=10),0);self.assertFalse((state/'state/join.lock').exists())
   finally:
    if p.poll() is None:p.send_signal(signal.SIGTERM);p.wait(timeout=10)
 def test_failed_start_and_stop_never_follow_a_stop_symlink(self):
  with tempfile.TemporaryDirectory(prefix='join-link-') as t:
   d=pathlib.Path(t);r=d/'source';state=d/'install/state';state.mkdir(parents=True)
   for folder in ['coax','psionic/platform','psionic/fixtures/pylon']:(r/folder).mkdir(parents=True,exist_ok=True)
   (r/'psionic/platform/pylon-build.sh').write_text('')
   (r/'psionic/fixtures/pylon/psionic-toy.selfcheck').write_text('{}')
   (r/'coax/provenance.sh').write_text('#!/bin/sh\nexit 1\n');(r/'coax/provenance.sh').chmod(0o755)
   target=d/'keep';target.write_text('preserve this');(state/'stop').symlink_to(target)
   env={**os.environ,'OPENAGENTS_PYLON_HOME':str(state.parent)}
   for options in [['--source',str(r)],['--stop']]:
    p=subprocess.run(['bash',str(ROOT/'scripts/join-pylon.sh'),*options],env=env,capture_output=True,text=True)
    self.assertNotEqual(p.returncode,0);self.assertEqual(target.read_text(),'preserve this')
 def test_help(self):
  r=subprocess.run(['bash',str(ROOT/'scripts/join-pylon.sh'),'--help'],capture_output=True,text=True);self.assertEqual(r.returncode,0);self.assertIn('--source',r.stdout)
if __name__=='__main__':unittest.main()
