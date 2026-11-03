#!/usr/bin/env python3
"""Independent retained-store supplied-fact native oracles; tooling only."""
import hashlib
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import tempfile
import time
import unittest
from quiet_crash import command as quiet_command
from retain_policy_vectors import ORACLE, generate as generate_vectors

ROOT=Path(__file__).resolve().parents[1]
SOURCE=ROOT/'lib/compiler/retain-policy.coax'
DEPENDENCY=ROOT/'lib/compiler/borrow-summary.coax'
DRIVER=Path(__file__).with_name('retain-policy-native.c')
VECTOR_ADAPTER=Path(__file__).with_name('retain_policy_vectors.py')
COMPILER=Path(os.environ.get('COAX_TEST_COMPILER',ROOT/'bin/coax')).resolve()

def digest(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()

class RetainPolicyNative(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        record_dir=os.environ.get('COAX_RETAIN_POLICY_RECORD_DIR')
        cls.directory=Path(record_dir) if record_dir else Path(tempfile.mkdtemp(prefix='coax-retain-policy-native-'))
        if record_dir:cls.directory.mkdir(parents=True,exist_ok=False)
        cls.record={'schema':'coax.retain-policy.native.v1','scope':'Standalone frame-relative supplied-store facts, independent literal table and actual native LP64 ABI only. No source acceptance/checker integration, host reclamation, helper composition or request lifecycle qualification.','compiler':str(COMPILER),'compiler_sha256':digest(COMPILER),'source_before':{str(p):digest(p) for p in (SOURCE,DEPENDENCY,DRIVER,Path(__file__),ORACLE,VECTOR_ADAPTER)},'commands':[],'profiles':{},'outcome':'not-checked'}
        cls.c=cls.directory/'retain-policy.c';cls.header=cls.directory/'retain-policy.h'
        try:
            cls.vector_include=cls.directory/'retain-policy-vectors.inc'
            cls.record['vectors']=generate_vectors(cls.vector_include)
            cls.record['vectors_sha256']=digest(cls.vector_include)
            output=cls.execute('emit',[COMPILER,'c',SOURCE,'--library','--arithmetic','checked','--header',cls.header,'--profile-report',cls.directory/'emission-profile.json'])
            cls.c.write_bytes(output)
            cls.record.update(c_sha256=digest(cls.c),header_sha256=digest(cls.header))
        finally:cls.save()
    @classmethod
    def save(cls):
        cls.record['source_after']={str(p):digest(p) for p in (SOURCE,DEPENDENCY,DRIVER,Path(__file__),ORACLE,VECTOR_ADAPTER)}
        cls.record['compiler_sha256_after']=digest(COMPILER)
        cls.record['inputs_unchanged']=cls.record['source_before']==cls.record['source_after'] and cls.record['compiler_sha256']==cls.record['compiler_sha256_after']
        artifacts_unchanged=True
        for name,path in [('c',cls.c),('header',cls.header),('vectors',getattr(cls,'vector_include',cls.directory/'retain-policy-vectors.inc'))]:
            if name+'_sha256' in cls.record:
                observed=digest(path) if path.is_file() else None
                cls.record[name+'_sha256_after']=observed
                artifacts_unchanged=artifacts_unchanged and observed==cls.record[name+'_sha256']
        cls.record['artifacts_unchanged']=artifacts_unchanged
        if not cls.record['inputs_unchanged']:cls.record['outcome']='source-changed'
        elif not artifacts_unchanged:cls.record['outcome']='artifact-changed'
        elif set(cls.record['profiles'])=={'O0','O2','sanitized'}:cls.record['outcome']='passed'
        (cls.directory/'qualification.json').write_text(json.dumps(cls.record,indent=2)+'\n')
    @classmethod
    def execute(cls,label,argv,env=None):
        cls.save()
        if not cls.record['inputs_unchanged'] or not cls.record['artifacts_unchanged']:
            raise AssertionError('source/compiler/emission changed before '+label)
        argv=list(map(os.fspath,argv));started=time.monotonic()
        result=subprocess.run(quiet_command(argv),capture_output=True,timeout=180,env=env)
        for name,raw in [('stdout',result.stdout),('stderr',result.stderr)]:
            (cls.directory/(label+'.'+name)).write_bytes(raw)
        resolved=shutil.which(argv[0]);cls.record['commands'].append({'label':label,'argv':argv,'resolved_program':resolved,'resolved_program_sha256':digest(resolved) if resolved and Path(resolved).is_file() else None,'returncode':result.returncode,'elapsed_s':time.monotonic()-started,'stdout_sha256':hashlib.sha256(result.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(result.stderr).hexdigest()})
        cls.save()
        if result.returncode:raise AssertionError(f'{label} returned {result.returncode}: '+result.stderr.decode(errors='replace'))
        return result.stdout
    def qualify(self,profile,flags):
        binary=self.directory/profile;cc=shlex.split(os.environ.get('CC','cc'))
        self.execute(profile+'-compile',[*cc,'-std=c11',*flags,'-Wall','-Wextra','-Werror','-Wno-unused-function','-Wno-unused-variable','-Wno-unused-parameter','-Wno-misleading-indentation','-Wno-unknown-pragmas','-ffp-contract=off','-fno-fast-math','-I',self.directory,self.c,DRIVER,'-lm','-o',binary])
        data=self.execute(profile+'-run',[binary],env={**os.environ,'ASAN_OPTIONS':'detect_leaks=0','UBSAN_OPTIONS':'halt_on_error=1'})
        observed=json.loads(data);self.assertGreater(observed['cases'],0);self.assertEqual(observed['calls'],observed['cases'])
        self.assertGreaterEqual(observed['cases'],1100);self.assertGreater(observed['refusals'],0)
        self.assertEqual(observed['independent_check_cases'],112);self.assertEqual(observed['independent_bounds_cases'],16)
        self.assertEqual([observed[k]for k in ['term_size','address_size','obligation_size','result_size']],[432,440,872,32])
        self.record['profiles'][profile]={'outcome':'passed','binary':str(binary),'binary_sha256':digest(binary),'observed':observed}
        self.save();self.assertTrue(self.record['inputs_unchanged']);self.assertTrue(self.record['artifacts_unchanged'])
    def test_checked_O0(self):self.qualify('O0',['-O0'])
    def test_checked_O2(self):self.qualify('O2',['-O2'])
    def test_checked_sanitizers(self):self.qualify('sanitized',['-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer'])

if __name__=='__main__':unittest.main()
