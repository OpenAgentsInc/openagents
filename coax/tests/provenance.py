#!/usr/bin/env python3
"""Build-record consistency and target-tree reuse, without network or a rebuild."""
from pathlib import Path
import os
import shutil
import subprocess
import signal
import time
import tempfile
import unittest

ROOT=Path(__file__).resolve().parents[1]
SCRIPT=ROOT/'provenance.sh'
class ProvenanceTests(unittest.TestCase):
 @classmethod
 def setUpClass(cls):
  cls.tmp=tempfile.TemporaryDirectory(prefix='coax-provenance-');cls.dir=Path(cls.tmp.name)
  cls.addClassCleanup(cls.tmp.cleanup)
  cls.helper=cls.dir/'check'; c=cls.dir/'provenance.c'; h=cls.dir/'provenance.h'
  with c.open('w') as out:
   subprocess.run([str(ROOT/'bin/coax'),'c',str(ROOT/'lib/compiler/provenance.coax'),
                   '--library','--header',str(h)],stdout=out,check=True,timeout=120)
  subprocess.run(['cc','-std=c11','-O1','-I',str(cls.dir),str(c),str(ROOT/'provenance-bridge.c'),
                  '-o',str(cls.helper)],check=True,timeout=120)
 def setUp(self):
  self.tmp=tempfile.TemporaryDirectory(prefix='coax-provenance-tree-');self.addCleanup(self.tmp.cleanup)
  self.tree=Path(self.tmp.name)/'repo';(self.tree/'coax').mkdir(parents=True);(self.tree/'scripts').mkdir()
  for name in ('src','bootstrap','lib/compiler','lib/proto'):
   shutil.copytree(ROOT/name,self.tree/'coax'/name)
  for name in ('build.sh','bootstrap-build.sh','provenance.sh','provenance-bridge.c','release.coax','release-model.coax'):
   shutil.copy2(ROOT/name,self.tree/'coax'/name)
  shutil.copy2(ROOT.parent/'scripts/coax-release.sh',self.tree/'scripts/coax-release.sh')
  self.binary=Path(self.tmp.name)/'coax';self.binary.write_bytes(b'tested compiler fixture');self.binary.chmod(0o755)
  self.helper_copy=Path(self.tmp.name)/'helper';shutil.copy2(self.helper,self.helper_copy)
  self.record=Path(self.tmp.name)/'record'
  self.identity=self.run_script('fingerprint',self.tree,success=True).stdout.strip()
  self.run_script('write',self.tree,self.binary,self.helper_copy,self.record,self.identity,success=True)
 def run_script(self,*args,success):
  result=subprocess.run([str(SCRIPT),*[str(x) for x in args]],text=True,capture_output=True,timeout=120)
  self.assertEqual(result.returncode==0,success,result.stdout+result.stderr)
  return result
 def verify(self,success=True): self.run_script('verify',self.tree,self.binary,self.helper_copy,self.record,success=success)
 def test_identical_inputs_and_unrelated_changes_reuse_without_rebuild(self):
  self.verify();(self.tree/'unrelated.txt').write_text('new remote work');self.verify()
  duplicate=Path(self.tmp.name)/'other';shutil.copytree(self.tree,duplicate)
  self.run_script('verify',duplicate,self.binary,self.helper_copy,self.record,success=True)
 def test_source_seed_and_added_input_are_rejected(self):
  for relative in ('coax/src/eval.cpp','coax/bootstrap/budget.c','coax/lib/compiler/budget.coax','coax/build.sh'):
   path=self.tree/relative;old=path.read_bytes();path.write_bytes(old+b'\n');self.verify(False);path.write_bytes(old)
  extra=self.tree/'coax/src/new.cpp';extra.write_text('// new compiler source\n');self.verify(False);extra.unlink();self.verify()
 def test_binary_helper_record_substitution_and_malformed_records_refuse(self):
  for path in (self.binary,self.helper_copy):
   old=path.read_bytes();path.write_bytes(old+b'x');self.verify(False);path.write_bytes(old)
  old=self.record.read_text()
  for record in ('',old+'extra\n',old.replace('coax-provenance-v1','coax-provenance-v2'),old.replace('a','A',1)):
   self.record.write_text(record);self.verify(False)
  self.record.write_text(old);self.record.unlink();self.verify(False)
 def test_install_snapshots_and_verifies_before_publication(self):
  source=Path(self.tmp.name)/'built';source.mkdir()
  shutil.copy2(self.binary,source/'coax');shutil.copy2(self.helper_copy,source/'provenance-check');shutil.copy2(self.record,source/'provenance.record')
  self.run_script('install',self.tree,source,success=True)
  self.assertEqual((self.tree/'coax/bin/coax').read_bytes(),self.binary.read_bytes())
  (self.tree/'coax/bin/coax').unlink()
  (source/'coax').write_bytes(b'substituted compiler')
  self.run_script('install',self.tree,source,success=False)
  self.assertFalse((self.tree/'coax/bin/coax').exists())
 def test_build_configuration_is_part_of_identity(self):
  result=subprocess.run([str(SCRIPT),'verify',str(self.tree),str(self.binary),str(self.helper_copy),str(self.record)],
                        text=True,capture_output=True,timeout=120,env={**os.environ,'COAX_HOST_TARGET':'different-build-target'})
  self.assertNotEqual(result.returncode,0)
 def test_release_refuses_stale_compiler_before_cloud_commands(self):
  caller=Path(self.tmp.name)/'caller';(caller/'coax/web').mkdir(parents=True);(caller/'coax/bin').mkdir()
  shutil.copy2(ROOT/'web/release.sh',caller/'coax/web/release.sh')
  shutil.copy2(self.binary,caller/'coax/bin/coax');shutil.copy2(self.helper_copy,caller/'coax/bin/provenance-check');shutil.copy2(self.record,caller/'coax/bin/provenance.record')
  (self.tree/'coax/src/eval.cpp').write_text('changed target source')
  fake=Path(self.tmp.name)/'fake';fake.mkdir()
  git=fake/'git';git.write_text("""#!/usr/bin/env python3
import os,shutil,sys
a=sys.argv[1:]
if 'rev-parse' in a: print('abcdef12')
elif 'log' in a: print('fixture')
elif 'worktree' in a and 'add' in a: shutil.copytree(os.environ['TARGET_TREE'],a[-2])
""");git.chmod(0o755)
  cloud=fake/'gcloud';cloud.write_text("#!/bin/sh\ntouch \"$CLOUD_MARKER\"\nexit 99\n");cloud.chmod(0o755)
  marker=Path(self.tmp.name)/'cloud-used';tmp=Path(self.tmp.name)/'release-temp';tmp.mkdir()
  env={**os.environ,'PATH':str(fake)+os.pathsep+os.environ['PATH'], 'TARGET_TREE':str(self.tree),
       'CLOUD_MARKER':str(marker),'TMPDIR':str(tmp)}
  result=subprocess.run([str(caller/'coax/web/release.sh')],capture_output=True,text=True,timeout=120,env=env)
  self.assertNotEqual(result.returncode,0,result.stdout+result.stderr)
  self.assertIn('compiler inputs or binary differ',result.stderr)
  self.assertFalse(marker.exists())
 def test_write_refuses_a_source_change_after_the_original_fingerprint(self):
  old=self.run_script('fingerprint',self.tree,success=True).stdout.strip()
  candidate=Path(self.tmp.name)/'late-record'
  self.run_script('write',self.tree,self.binary,self.helper_copy,candidate,old,success=True)
  candidate.unlink()
  (self.tree/'coax/src/eval.cpp').write_text('changed after build began')
  self.run_script('write',self.tree,self.binary,self.helper_copy,candidate,old,success=False)
  self.assertFalse(candidate.exists())
 def test_bootstrap_consumes_private_snapshot_during_transient_edit(self):
  bootstrap=self.tree/'coax/bootstrap-build.sh'
  bootstrap.write_text("""#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
mkdir -p bin
touch "$TEST_READY"
while [[ ! -e "$TEST_START" ]]; do sleep 0.05; done
printf 'compiler fixture\\n' > bin/coax
cat src/eval.cpp >> bin/coax
chmod +x bin/coax
cp "$TEST_HELPER" bin/provenance-check
for name in lexer.c lexer.h parser.c parser.h checker.c checker.h sources.c sources.h proto_runtime.inc proto_importer.c proto_importer.h; do : > "bin/$name"; done
input="$(./provenance.sh fingerprint ..)"
./provenance.sh write .. bin/coax bin/provenance-check bin/provenance.record "$input"
touch "$TEST_READ"
while [[ ! -e "$TEST_FINISH" ]]; do sleep 0.05; done
""");bootstrap.chmod(0o755)
  ready=Path(self.tmp.name)/'ready';start=Path(self.tmp.name)/'start';read=Path(self.tmp.name)/'read';finish=Path(self.tmp.name)/'finish'
  source=self.tree/'coax/src/eval.cpp';original=source.read_bytes()
  env={**os.environ,'TEST_READY':str(ready),'TEST_START':str(start),'TEST_READ':str(read),
       'TEST_FINISH':str(finish),'TEST_HELPER':str(self.helper_copy)}
  proc=subprocess.Popen([str(self.tree/'coax/build.sh')],stdout=subprocess.PIPE,stderr=subprocess.PIPE,
                        text=True,env=env,start_new_session=True)
  try:
   deadline=time.monotonic()+10
   while not ready.exists() and proc.poll() is None and time.monotonic()<deadline: time.sleep(.05)
   self.assertTrue(ready.exists(),'private bootstrap did not start')
   source.write_text('transient incompatible source');start.touch()
   deadline=time.monotonic()+10
   while not read.exists() and proc.poll() is None and time.monotonic()<deadline: time.sleep(.05)
   self.assertTrue(read.exists(),'private bootstrap did not read its snapshot')
   source.write_bytes(original);finish.touch()
   stdout,stderr=proc.communicate(timeout=20)
   self.assertEqual(proc.returncode,0,stdout+stderr)
   self.assertTrue((self.tree/'coax/bin/coax').read_bytes().endswith(original))
  finally:
   source.write_bytes(original)
   if proc.poll() is None:
    os.killpg(proc.pid,signal.SIGTERM);proc.communicate(timeout=10)
 def test_internal_bootstrap_refuses_direct_invocation(self):
  result=subprocess.run([str(ROOT/'bootstrap-build.sh')],capture_output=True,text=True,timeout=5)
  self.assertNotEqual(result.returncode,0)
  self.assertIn('internal private-snapshot bootstrap',result.stderr)
 def test_input_symlinks_refuse(self):
  path=self.tree/'coax/src/substitution.cpp';path.symlink_to(self.tree/'unrelated.txt')
  self.verify(False)

if __name__=='__main__': unittest.main()
