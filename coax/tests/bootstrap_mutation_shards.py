#!/usr/bin/env python3
"""The bootstrap replays real complete shard records; observations never waive it."""
import hashlib
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT=Path(__file__).resolve().parents[1]
COMPILER=Path(os.environ.get('COAX_TEST_COMPILER',ROOT/'bin/coax')).resolve()
HOOK=ROOT/'bootstrap-mutation-shards.sh'

def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()

class BootstrapMutationShards(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp=tempfile.TemporaryDirectory(prefix='coax-real-shard-replay-');cls.work=Path(cls.temp.name)
        cls.entry=cls.work/'sample.coax';cls.entry.write_text('f::(x:int)->int{return x+1;}\n#test "values"{#assert f(1)==2 && f(2)==3;}\nmain::(){}\n')
        cls.closure=sha(cls.entry);cls.records=[]
        for part in (1,2):
            r=subprocess.run([COMPILER,'mutate',cls.entry,'--shard',f'{part}/2'],capture_output=True,timeout=30)
            if r.returncode:raise AssertionError(r.stderr.decode())
            record=cls.work/f'{part}.rec';record.write_bytes(r.stdout);cls.records.append(record)

    @classmethod
    def tearDownClass(cls):cls.temp.cleanup()

    def manifest(self,name='proof',*,compiler_hash=None,closure=None,records=None):
        records=self.records if records is None else records
        path=self.work/(name+'.manifest')
        rows=['coax-bootstrap-shards-v1','compiler\t'+(compiler_hash or sha(COMPILER)),'closure\t'+(closure or self.closure),'entry\t'+str(self.entry)]
        rows += ['record\t'+sha(record)+'\t'+str(record)for record in records]
        path.write_text('\n'.join(rows)+'\n');return path

    def invoke(self,path):return subprocess.run(['bash',HOOK,path,COMPILER,self.closure,self.entry],capture_output=True,timeout=30)

    def test_complete_real_shards_replay_the_actual_merge(self):
        r=self.invoke(self.manifest());self.assertEqual(r.returncode,0,r.stderr.decode());self.assertIn(b'0 survived',r.stdout)

    def test_bad_binary_binding_is_refused(self):
        r=self.invoke(self.manifest('bad-binary',compiler_hash='0'*64));self.assertNotEqual(r.returncode,0);self.assertIn(b'compiler',r.stderr)

    def test_bad_closure_binding_is_refused(self):
        r=self.invoke(self.manifest('bad-closure',closure='0'*64));self.assertNotEqual(r.returncode,0);self.assertIn(b'closure',r.stderr)

    def test_missing_record_file_is_refused(self):
        path=self.manifest('missing');path.write_text(path.read_text().replace(str(self.records[0]),str(self.work/'absent.rec')))
        r=self.invoke(path);self.assertNotEqual(r.returncode,0);self.assertIn(b'record',r.stderr)

    def test_missing_shard_coverage_is_refused_by_real_merge(self):
        r=self.invoke(self.manifest('incomplete',records=self.records[:1]));self.assertNotEqual(r.returncode,0);self.assertIn(b'is missing',r.stderr)

    def test_record_mutation_is_refused_before_merge(self):
        path=self.manifest('changed');bad=self.work/'changed.rec';bad.write_text(self.records[0].read_text()+'not-a-result\n');path.write_text(path.read_text().replace(str(self.records[0]),str(bad)))
        r=self.invoke(path);self.assertNotEqual(r.returncode,0);self.assertIn(b'record hash',r.stderr)

    def test_empty_manifest_never_falls_back_to_full_mutation(self):
        r=self.invoke(self.manifest('empty',records=[]));self.assertNotEqual(r.returncode,0);self.assertIn(b'no shard',r.stderr)

class PrivateReplayConstruction(unittest.TestCase):
    def test_private_replay_requires_its_real_construction_record(self):
        with tempfile.TemporaryDirectory(prefix='coax-missing-stage0-record-')as directory:
            work=Path(directory)
            (work/'stage0').write_bytes(COMPILER.read_bytes());(work/'stage0').chmod(0o755)
            (work/'proto_runtime.inc').write_text('not a construction receipt\n')
            r=subprocess.run(['bash',ROOT/'bootstrap-build.sh','--private-replay-checker-cache',work,work/'absent.manifest'],cwd=ROOT,capture_output=True,timeout=60)
            self.assertNotEqual(r.returncode,0);self.assertIn(b'missing retained stage0.record',r.stderr)

if __name__=='__main__':unittest.main()
