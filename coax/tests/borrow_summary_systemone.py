#!/usr/bin/env python3
"""Offline native caller-buffer oracles for the unchanged System One adapter."""
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

ROOT=Path(__file__).resolve().parents[1]
FIXTURE=ROOT/'tests/borrow-summary-return/systemone.coax'
DRIVER=FIXTURE.with_name('systemone-native.c')
COMPILER=Path(os.environ.get('COAX_TEST_COMPILER',ROOT/'bin/coax')).resolve()
SOURCES=(FIXTURE,DRIVER,Path(__file__),ROOT/'lib/systemone/transport.coax',ROOT/'lib/nostr/json.coax',ROOT/'lib/agent/claude/transport.coax')
def digest(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()

class SystemOneReturnedBuffers(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        where=os.environ.get('COAX_BORROW_SYSTEMONE_RECORD_DIR')
        cls.directory=Path(where) if where else Path(tempfile.mkdtemp(prefix='coax-systemone-borrow-'))
        if where:cls.directory.mkdir(parents=True,exist_ok=False)
        cls.c=cls.directory/'systemone.c';cls.header=cls.directory/'systemone.h'
        cls.record={'schema':'coax.borrow-return.systemone.native.v1','scope':'Actual so_digits/so_stderr caller-owned return content, pointer/extent/guards and post-return use; no async/request retention or lifetime-safety proof. Digits edge vectors include INT64_MIN+1/MAX; no INT64_MIN conversion-support claim.','compiler':str(COMPILER),'compiler_sha256':digest(COMPILER),'source_before':{str(p):digest(p) for p in SOURCES},'profiles':{},'commands':[],'outcome':'not-checked'}
        try:
            raw=cls.execute('emit',[COMPILER,'c',FIXTURE,'--library','--arithmetic','checked','--header',cls.header,'--profile-report',cls.directory/'emission-profile.json'])
            cls.c.write_bytes(raw);cls.record.update(c_sha256=digest(cls.c),header_sha256=digest(cls.header))
            manifest=json.loads((cls.directory/'emission-profile.json').read_text())
            cls.record['input_manifest']=manifest
            cls.record['consumed_input_before']={}
            for category in ('sources','descriptors'):
                if type(manifest[category]) is not list:raise AssertionError('missing/malformed '+category+' manifest')
                for fact in manifest[category]:
                    path=fact.get('resolved_path') or fact['path'];observed=digest(path)
                    cls.record['consumed_input_before'][path]=observed
                    if observed!=fact['sha256']:raise AssertionError('consumed '+category+' changed after emission: '+path)
            cls.record['consumed_inputs_match_manifest']=True
            if not manifest['sources']:raise AssertionError('empty source manifest')
        finally:cls.save()
    @classmethod
    def save(cls):
        cls.record['source_after']={str(p):digest(p) for p in SOURCES}
        cls.record['compiler_sha256_after']=digest(COMPILER)
        cls.record['consumed_input_after']={p:digest(p) for p in cls.record.get('consumed_input_before',{})}
        same=cls.record['source_before']==cls.record['source_after'] and cls.record['compiler_sha256']==cls.record['compiler_sha256_after'] and cls.record.get('consumed_input_before',{})==cls.record['consumed_input_after']
        cls.record['inputs_unchanged']=same;artifacts=True
        for name,path in [('c',cls.c),('header',cls.header)]:
            if name+'_sha256' in cls.record:
                observed=digest(path) if path.is_file() else None;cls.record[name+'_sha256_after']=observed;artifacts=artifacts and observed==cls.record[name+'_sha256']
        cls.record['artifacts_unchanged']=artifacts
        if not same:cls.record['outcome']='source-changed'
        elif not artifacts:cls.record['outcome']='artifact-changed'
        elif set(cls.record['profiles'])=={'O0','O2','sanitized'}:cls.record['outcome']='passed'
        (cls.directory/'qualification.json').write_text(json.dumps(cls.record,indent=2)+'\n')
    @classmethod
    def execute(cls,label,argv,env=None):
        cls.save()
        if not cls.record['inputs_unchanged'] or not cls.record['artifacts_unchanged']:raise AssertionError('input/artifact changed before '+label)
        argv=list(map(os.fspath,argv));start=time.monotonic();r=subprocess.run(quiet_command(argv),capture_output=True,timeout=180,env=env)
        for stream,raw in [('stdout',r.stdout),('stderr',r.stderr)]:(cls.directory/(label+'.'+stream)).write_bytes(raw)
        program=shutil.which(argv[0]);cls.record['commands'].append({'label':label,'argv':argv,'resolved_program':program,'resolved_program_sha256':digest(program) if program and Path(program).is_file() else None,'returncode':r.returncode,'elapsed_s':time.monotonic()-start,'stdout_sha256':hashlib.sha256(r.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(r.stderr).hexdigest()});cls.save()
        if r.returncode:raise AssertionError(f'{label} returned {r.returncode}: '+r.stderr.decode(errors='replace'))
        return r.stdout
    def qualify(self,name,flags):
        cc=shlex.split(os.environ.get('CC','cc'));binary=self.directory/name
        self.execute(name+'-compile',[*cc,'-std=c11',*flags,'-Wall','-Wextra','-Werror','-Wno-unused-function','-Wno-unused-variable','-Wno-unused-parameter','-Wno-misleading-indentation','-ffp-contract=off','-fno-fast-math','-I',self.directory,self.c,DRIVER,'-lm','-o',binary])
        observed=json.loads(self.execute(name+'-run',[binary],env={**os.environ,'ASAN_OPTIONS':'detect_leaks=0','UBSAN_OPTIONS':'halt_on_error=1'}))
        self.assertEqual(observed['digits_cases'],129);self.assertEqual(observed['stderr_cases'],45)
        self.record['profiles'][name]={'outcome':'passed','binary':str(binary),'binary_sha256':digest(binary),'observed':observed};self.save();self.assertTrue(self.record['inputs_unchanged']);self.assertTrue(self.record['artifacts_unchanged'])
    def test_checked_O0(self):self.qualify('O0',['-O0'])
    def test_checked_O2(self):self.qualify('O2',['-O2'])
    def test_checked_sanitizers(self):self.qualify('sanitized',['-O1','-g','-fsanitize=address,undefined','-fno-omit-frame-pointer'])

if __name__=='__main__':unittest.main()
