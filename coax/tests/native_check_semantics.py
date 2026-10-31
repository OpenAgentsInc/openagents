#!/usr/bin/env python3
"""Independent C3 values, writes, cleanup, typed-trap and seeded-artifact controls."""
import hashlib
import base64
import copy
import importlib.util
import json
import os
from pathlib import Path
import shlex
import signal
import sys
import tempfile
import time
import unittest
from unittest import mock

COAX=Path(__file__).resolve().parents[1]
REPO=COAX.parent
FIXTURES=COAX/'tests/native-check-semantics'
COMPILER=Path(os.environ.get('COAX_SEMANTICS_TEST_COMPILER',
                            os.environ.get('COAX_TEST_COMPILER',COAX/'bin/coax'))).resolve()

def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def load(name,path):
    if name in sys.modules:return sys.modules[name]
    if str(path.parent) not in sys.path:sys.path.insert(0,str(path.parent))
    spec=importlib.util.spec_from_file_location(name,path)
    module=importlib.util.module_from_spec(spec);sys.modules[name]=module
    try:spec.loader.exec_module(module)
    except Exception:
        sys.modules.pop(name,None)
        raise
    return module
def encoded(value):
    if isinstance(value,bytes):return {'hex':value.hex()}
    if isinstance(value,Path):return str(value)
    raise TypeError(type(value).__name__)

def raw_bytes(value):
    if isinstance(value,bytes):return value
    raw=base64.b64decode(value['base64'],validate=True)
    if len(raw)!=value['bytes'] or hashlib.sha256(raw).hexdigest()!=value['sha256']:
        raise AssertionError('retained byte evidence is not intact')
    return raw

QUALIFIED_MULTIPLY=None
def qualified_multiply(test):
    global QUALIFIED_MULTIPLY
    tool=test.tool()
    if QUALIFIED_MULTIPLY is None:
        retained=os.environ.get('COAX_SEMANTICS_MULTIPLY_RECORD')
        if retained:
            QUALIFIED_MULTIPLY=json.loads(Path(retained).read_text())
        else:
            QUALIFIED_MULTIPLY=tool.run([FIXTURES/'multiply.coax'],keep=test.dir/'qualified-original',compiler=COMPILER)
            (test.records/'qualified-multiply.json').write_text(json.dumps(QUALIFIED_MULTIPLY,indent=2)+'\n')
    test.assertEqual(QUALIFIED_MULTIPLY['compiler_sha256'],sha(COMPILER))
    test.assertEqual(QUALIFIED_MULTIPLY['outcome'],'passed')
    test.assertTrue(QUALIFIED_MULTIPLY['baseline_qualified'])
    test.observations.append({'qualified_original':QUALIFIED_MULTIPLY})
    return QUALIFIED_MULTIPLY

class RecordedTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.records=Path(os.environ.get('COAX_SEMANTICS_RECORD_DIR') or tempfile.mkdtemp(prefix='coax-native-semantics-'))
        cls.records.mkdir(parents=True,exist_ok=True)
        cls.process=load('semantics_bounded_process',COAX/'tools/bounded_process.py')
    def setUp(self):
        self.dir=Path(tempfile.mkdtemp(prefix=self._testMethodName+'-',dir=self.records))
        self.started=time.monotonic();self.observations=[]
        self.before={str(p):sha(p) for p in FIXTURES.iterdir() if p.is_file()}
        self.compiler_sha=sha(COMPILER)
    def tearDown(self):
        after={str(p):sha(p) for p in FIXTURES.iterdir() if p.is_file()}
        record={'method':self._testMethodName,'seconds':time.monotonic()-self.started,
                'compiler':str(COMPILER),'compiler_sha256':self.compiler_sha,'compiler_sha256_after':sha(COMPILER),
                'harness_sha256':sha(__file__),'fixtures_before':self.before,'fixtures_after':after,
                'observations':self.observations}
        policy=getattr(self,'policy',None)
        if policy is not None:record['policy_identity']=policy.identity
        (self.dir/'receipt.json').write_text(json.dumps(record,indent=2,default=encoded)+'\n')
        self.assertEqual(self.before,after,'original fixture bytes changed')
        self.assertEqual(self.compiler_sha,sha(COMPILER),'compiler changed during focused test')
    def command(self,argv):
        result=self.process.run(list(map(str,argv)),cwd=str(REPO),env=None,timeout_s=10,
                                max_output_bytes=1048576,deadline=time.monotonic()+30,quiet_faults=True)
        self.observations.append(result);return result
    def compiler(self,*args):return self.command([COMPILER,*args])
    def tool(self):return load('coax_native_semantics_execution',COAX/'tools/native_semantics_execution.py')

class CheckFirstTests(RecordedTests):
    def test_verification_only_trap_setter_links_against_original_checks(self):
        generated=self.dir/'checks.c'
        emitted=self.compiler('checks-c',FIXTURES/'hook-control.coax','-o',generated)
        self.assertEqual(emitted['returncode'],0,emitted['stderr'].decode(errors='replace'))
        binary=self.dir/'hook-host'
        linked=self.command([*shlex.split(os.environ.get('CC','cc')),'-std=c11','-O0',generated,
                             FIXTURES/'hook-driver.c','-lm','-o',binary])
        self.assertEqual(linked['returncode'],0,linked['stderr'].decode(errors='replace'))

    def test_strict_semantics_api_qualifies_original_positive_baseline(self):
        result=self.tool().run([FIXTURES/'hook-control.coax'],keep=self.dir/'artifacts',compiler=COMPILER)
        self.observations.append(result)
        self.assertEqual(result['schema'],'coax.native-checks.semantics.v1')
        self.assertEqual(result['outcome'],'passed')
        self.assertTrue(result['baseline_qualified'])
        self.assertFalse(result['artifact_only'])
        self.assertEqual(result['source_minimization'],'not-supported')

    def test_normal_product_c_bytes_match_the_pre_hook_control(self):
        control=json.loads((FIXTURES/'manifest.json').read_text())['normal_product_c']
        self.assertEqual(sha(FIXTURES/'hook-control.coax'),control['source_sha256'])
        emitted=self.compiler(*control['argv'])
        self.assertEqual(emitted['returncode'],0,emitted['stderr'].decode(errors='replace'))
        self.assertEqual(hashlib.sha256(emitted['stdout']).hexdigest(),control['c_sha256'])
        self.assertNotIn(b'coax_verification_set_trap_v1',emitted['stdout'])


class HostCompatibilityTests(RecordedTests):
    def test_default_abi2_public_entry_preserves_its_original_profile_bindings(self):
        quantified=load('native_forall_execution',COAX/'tools/native_forall_execution.py')
        sentinel=object();files=[FIXTURES/'hook-control.coax']
        with mock.patch.object(quantified,'_run_profile',return_value=sentinel) as delegate:
            actual=quantified.run(files)
        self.assertIs(actual,sentinel)
        delegate.assert_called_once()
        args,options=delegate.call_args
        self.assertEqual(args,(files,))
        self.assertEqual(options['schema'],'coax.native-checks.execution.v2')
        self.assertEqual(options['observer'],COAX/'tools/native-forall-observer.c')
        self.assertIs(options['policy_factory'],quantified.get_policy)
        self.assertIs(options['observe'],quantified._observe)
        self.assertEqual({key:options[key] for key in ('keep','steps','only','timeout_s','max_output_bytes','cc','compiler','abi')},
                         {'keep':None,'steps':None,'only':None,'timeout_s':10.0,'max_output_bytes':1048576,'cc':None,'compiler':None,'abi':2})
        self.assertEqual(options.get('extra_transport',()),())
        self.observations.append({'scope':'Host delegation only; no compiler/native effect',
                                  'schema':options['schema'],'observer':str(options['observer']),'abi':options['abi']})


class OriginalEvaluatorTests(RecordedTests):
    def positive(self,name,counts):
        evidence=self.dir/(name+'.json')
        result=self.compiler('test',FIXTURES/(name+'.coax'),'--library','--evidence',evidence)
        self.assertEqual(result['returncode'],0,result['stderr'].decode(errors='replace'))
        record=json.loads(evidence.read_text());self.observations.append({'original_evidence':record})
        self.assertEqual(record['verdict'],'passed')
        self.assertEqual([claim['cases'] for claim in record['claims']],counts)
        self.assertTrue(all(claim['outcome']=='passed' for claim in record['claims']))

    def test_ordered_value_and_write_guards_pass_original_evaluator(self):self.positive('values-writes',[1])
    def test_nested_return_break_continue_cleanup_passes_original_evaluator(self):self.positive('cleanup',[1,1,1])
    def test_division_remainder_and_cast_controls_pass_original_evaluator(self):self.positive('arithmetic',[6,1])
    def test_frozen49_product_table_passes_original_evaluator(self):self.positive('multiply',[49])

    def test_dynamic_traps_retain_exact_original_json_diagnostic_and_failed_baseline(self):
        manifest=json.loads((FIXTURES/'manifest.json').read_text())
        for expected in manifest['traps']:
            with self.subTest(source=expected['fixture']):
                evidence=self.dir/(expected['fixture']+'.json')
                result=self.compiler('test',FIXTURES/(expected['fixture']+'.coax'),'--library',
                                     '--json','--evidence',evidence)
                self.assertNotEqual(result['returncode'],0)
                record=json.loads(evidence.read_text());self.observations.append({'original_failed_evidence':record})
                self.assertEqual(record['verdict'],'counterexample')
                self.assertEqual(record['claims'][0]['outcome'],'counterexample')
                diagnostics=[json.loads(line) for line in result['stderr'].splitlines()]
                hits=[entry for entry in diagnostics if entry['code']==expected['code']]
                self.assertEqual(len(hits),1,diagnostics)
                diagnostic=hits[0]
                self.assertEqual(Path(diagnostic['file']).resolve(),(FIXTURES/expected['source']).resolve())
                self.assertEqual(diagnostic['line'],expected['line'])
                self.assertIsInstance(diagnostic['col'],int)
                self.assertGreater(diagnostic['col'],0)
                self.assertIsInstance(diagnostic['message'],str)
                self.assertTrue(diagnostic['message'])
                projection={key:diagnostic[key] for key in ('code','line','col','message')}
                projection['file']=Path(diagnostic['file']).name
                self.assertEqual(projection,expected['diagnostic'])


class PositiveSemanticsTests(RecordedTests):
    def positive(self,name):
        result=self.tool().run([FIXTURES/(name+'.coax')],keep=self.dir/'artifacts',compiler=COMPILER)
        self.observations.append(result)
        self.assertEqual(result['schema'],'coax.native-checks.semantics.v1')
        self.assertEqual(result['outcome'],'passed')
        self.assertTrue(result['baseline_qualified'])
        self.assertFalse(result['artifact_only'])
        self.assertEqual(result['source_minimization'],'not-supported')
        self.assertEqual(set(result['files'][0]['profiles']),{'-O0','-O2'})
        return result
    def test_ordered_values_writes_calls_and_guards(self):self.positive('values-writes')
    def test_nested_return_break_continue_cleanup_and_guards(self):self.positive('cleanup')
    def test_truncating_division_remainder_and_cast_boundaries(self):self.positive('arithmetic')


class TrapParityTests(RecordedTests):
    def oracle_controls(self,result,facts):
        tool=self.tool();policy=tool.get_policy(COMPILER);raw=tool._raw(result)
        authored=raw['native_expected']
        self.assertEqual(authored[0]['identity']['source'],str(Path(facts['declaration']['source']).resolve()))
        self.assertEqual(authored[0]['identity']['line'],facts['declaration']['line'])
        self.assertEqual(authored[0]['identity']['column'],facts['declaration']['column'])
        for profile in raw['profiles'].values():
            metadata=profile['metadata']
            self.assertEqual(policy.metadata(authored,metadata['stdout'],metadata),0)
            for field,value in (('line',facts['declaration']['line']+1),('column',2),
                                ('source',str(self.dir/'wrong-original-source.coax'))):
                changed=copy.deepcopy(authored);changed[0]['identity'][field]=value
                self.assertNotEqual(policy.metadata(changed,metadata['stdout'],metadata),0)
            self.assertNotEqual(policy.metadata([],metadata['stdout'],metadata),0)
        class RetainedOriginalSession:
            remaining_bytes=67108864
            environment=None
            def run(inner,argv,**options):
                if len(argv)>1 and argv[1]=='version':return raw['compiler_version_execution']
                if len(argv)>1 and argv[1]=='test':return raw['baseline_execution']
                raise AssertionError('missing declaration oracle reached a new compiler/native effect')
        missing=dict(facts);missing.pop('declaration')
        with mock.patch.object(tool.common,'Session',return_value=RetainedOriginalSession()):
            refused=tool.run_trap(FIXTURES/Path(raw['file']).name,missing,
                                  keep=self.dir/'missing-oracle',compiler=COMPILER)
        self.observations.append({'scope':'Retained original version/baseline replay and metadata-only oracle controls; no new process',
                                  'missing_oracle':refused})
        self.assertNotEqual(refused['outcome'],'passed')
        self.assertFalse(refused['baseline_qualified'])
        self.assertFalse(refused['profiles'])

    def trap(self,name):
        expected=next(value for value in json.loads((FIXTURES/'manifest.json').read_text())['traps']
                      if value['fixture']==name)
        facts={'kind':expected['kind'],'source':FIXTURES/expected['source'],'line':expected['line'],
               'evaluator_code':expected['code'].encode(),
               'declaration':{'source':str((FIXTURES/expected['declaration']['source']).resolve()),
                              'line':expected['declaration']['line'],
                              'column':expected['declaration']['column']}}
        result=self.tool().run_trap(FIXTURES/(name+'.coax'),facts,keep=self.dir/'artifacts',compiler=COMPILER)
        self.observations.append(result)
        self.assertEqual(result['schema'],'coax.native-checks.semantics.v1')
        self.assertEqual(result['outcome'],'passed','Matched parity is an experiment result, not authored-check qualification')
        self.assertTrue(result['artifact_only'])
        self.assertFalse(result['baseline_qualified'])
        self.assertEqual(result['source_minimization'],'not-supported')
        self.assertEqual(result['baseline']['verdict'],'counterexample')
        self.assertEqual(result['baseline_validation']['outcome'],'counterexample')
        self.assertEqual(Path(result['baseline']['claims'][0]['at']['file']).resolve(),
                         (FIXTURES/expected['source']).resolve())
        self.assertEqual(result['baseline']['claims'][0]['at']['line'],expected['line'])
        diagnostic=next(d for d in result['evaluator_diagnostics'] if d['code']==expected['code'])
        projection={key:diagnostic[key] for key in ('code','line','col','message')}
        projection['file']=Path(diagnostic['file']).name
        self.assertEqual(projection,expected['diagnostic'])
        self.assertEqual(set(result['profiles']),{'-O0','-O2'})
        for profile in result['profiles'].values():
            self.assertEqual(profile['outcome'],'passed')
            self.assertEqual(len(profile['checks']),1)
            check=profile['checks'][0]
            self.assertEqual(check['outcome'],'passed')
            self.assertEqual(check['native_outcome'],'counterexample')
            self.assertTrue(check['recognized_trap'])
            self.assertTrue(check['trap']['ok'])
            self.assertEqual(check['trap']['kind'],expected['kind'])
            self.assertEqual(check['trap']['line'],expected['line'])
            self.assertEqual(raw_bytes(check['trap']['source']),str((FIXTURES/expected['source']).resolve()).encode())
            self.assertEqual(check['process']['returncode'],-signal.SIGABRT)
        self.oracle_controls(result,facts)
    def test_dynamic_assertion(self):self.trap('trap-assert')
    def test_dynamic_add_overflow(self):self.trap('trap-overflow')
    def test_dynamic_division_by_zero(self):self.trap('trap-divzero')
    def test_dynamic_index_bounds(self):self.trap('trap-bounds')
    def test_dynamic_slice_bounds(self):self.trap('trap-slice')
    def test_dynamic_checked_cast(self):self.trap('trap-cast')
    def test_dynamic_shift_bounds(self):self.trap('trap-shift')
    def test_loaded_physical_source(self):self.trap('trap-loaded')


class SeedReductionTests(RecordedTests):
    def test_seeded_multiply_artifact_is_detected_and_reduced_without_rewriting_source(self):
        tool=self.tool();source=FIXTURES/'multiply.coax';before=source.read_bytes()
        qualified=qualified_multiply(self)
        self.assertEqual(qualified['outcome'],'passed')
        self.assertTrue(qualified['baseline_qualified'])
        expected=json.loads((FIXTURES/'manifest.json').read_text())['mul_add_seed']
        result=tool.seed_and_reduce(qualified,expected={'kind':expected['kind'],
            'source':str(source.resolve()),'line':expected['line']},keep=self.dir/'seeded',optimization='2',seed_id='mul-add')
        self.observations.append(result)
        self.assertEqual(result['schema'],'coax.native-checks.semantics.v1')
        self.assertEqual(result['outcome'],'passed')
        self.assertTrue(result['artifact_only'])
        self.assertFalse(result['baseline_qualified'])
        self.assertEqual(result['source_minimization'],'not-supported')
        self.assertEqual(result['original_baseline']['verdict'],'passed')
        self.assertEqual(result['seeded_native_outcome'],'counterexample')
        self.assertEqual(result['initial_values'],[18446744073709551613,18446744073709551613])
        self.assertEqual(result['reduced_values'],[0,18446744073709551615])
        self.assertLessEqual(len(result['replays']),32)
        signed=lambda values:[value-(1<<64) if value>=(1<<63) else value for value in values]
        self.assertLess(sum(map(abs,signed(result['reduced_values']))),sum(map(abs,signed(result['initial_values']))))
        self.assertEqual(set(result['profiles']),{'-O0','-O2'})
        for profile in result['profiles'].values():
            self.assertEqual(profile['checks'][0]['native_outcome'],'counterexample')
            self.assertTrue(profile['checks'][0]['recognized_trap'])
            self.assertEqual(profile['checks'][0]['trap']['line'],expected['line'])
        original=Path(result['original_c']).read_bytes();seeded=Path(result['seeded_c']).read_bytes()
        self.assertEqual(original,Path(qualified['files'][0]['artifact']['c']).read_bytes())
        # The copied verification artifact's checked-integer helper family is
        # seeded; original source/call sites and literal expected products stay intact.
        anchor=b'if (__builtin_mul_overflow(a, b, &r))'
        replacement=b'if (__builtin_add_overflow(a, b, &r))'
        self.assertEqual(original.count(anchor),1)
        self.assertEqual(seeded,original.replace(anchor,replacement,1))
        self.assertEqual(source.read_bytes(),before)
        self.assertTrue(any(replay['accepted'] for replay in result['replays']))
        self.assertEqual(result['signature']['optimization'],2)
        self.assertEqual(result['signature']['seed_id'],'mul-add')
        self.assertEqual(result['signature']['kind'],1)
        self.assertEqual(result['signature']['line'],expected['line'])
        for replay in result['replays']:
            self.assertIn(replay['observation']['native_outcome'],('passed','counterexample'))
            self.assertFalse(replay['observation'].get('baseline_qualified',False))
            self.assertLessEqual(replay['observation']['completed_cases'],1,
                                 'single-tuple replay must not fabricate earlier traversal')
            if replay['accepted']:
                self.assertEqual(replay['observation']['trap']['kind'],1)
                self.assertEqual(replay['observation']['trap']['line'],expected['line'])
                self.assertEqual(raw_bytes(replay['observation']['trap']['source']),str(source.resolve()).encode())


class ProtocolPolicyTests(RecordedTests):
    @classmethod
    def setUpClass(cls):
        super().setUpClass();cls.execution=load('coax_native_semantics_execution',COAX/'tools/native_semantics_execution.py')
        cls.policy=cls.execution.get_policy(COMPILER,cls.records/'protocol-policy')
    def test_literal_structured_trap_vector_rejects_incomplete_or_changed_shapes(self):
        vectors=json.loads((FIXTURES/'protocol-vectors.json').read_text())
        wire=bytes.fromhex(vectors['trap_assert']);actual=self.policy.decode_trap(wire)
        self.observations.append({'scope':'Declared ST1 byte vector only; no native process', 'wire':wire,'result':actual})
        self.assertEqual(actual,{'ok':True,'kind':1,'ordinal':2,'declaration_index':7,'line':11,'source':b'abc'})
        for bad in (wire[:8],wire[:-1],wire+b'\0',wire[:8]+b'\x22\0\0\0'+wire[12:],
                    wire[:12]+b'\xff\xff\xff\xff'+wire[16:]):
            self.assertFalse(self.policy.decode_trap(bad)['ok'])

    def test_literal_single_case_progress_proves_only_the_requested_tuple(self):
        vectors=json.loads((FIXTURES/'protocol-vectors.json').read_text())
        begin=bytes.fromhex(vectors['single_begin']);end=bytes.fromhex(vectors['single_end'])
        check={'ordinal':2,'declaration_index':7,'name':b'declared replay','source':b'abc','line':1,'column':1,
            'kind':2,'cases':6,'case_count':6,'bindings':[
                {'name':b'enabled','type_name':b'bool','members':[],
                 'domain':{'kind':1,'bits':8,'is_signed':False,'first':0,'count':2,'values':[]}},
                {'name':b'offset','type_name':b'int','members':[],
                 'domain':{'kind':5,'bits':64,'is_signed':True,'first':-2,'count':3,'values':[]}}]}
        expected={'kind':1,'source':b'abc','line':11}
        process={'started':True,'returncode':-signal.SIGABRT,'timeout':False,'deadline_exhausted':False,
            'output_limit':False,'stdout_limit':False,'stderr_limit':False,'read_errno':None,
            'stdout_eof':True,'stderr_eof':True,'stdout':bytes.fromhex(vectors['trap_assert']),'stderr':b''}
        region=bytearray(32769);region[0]=0;region[1:1+len(begin)]=begin
        failed=self.policy.classify(check,expected,bytes(region),process,replay_index=5)
        self.observations.append({'scope':'Declared SP1/process facts only; no native observation', 'result':failed})
        self.assertTrue(failed['recognized_trap'])
        self.assertEqual(failed['native_outcome'],'counterexample')
        self.assertEqual(failed['values'],[1,0])
        self.assertFalse(failed['baseline_qualified'])
        wrong=self.policy.classify(check,expected,bytes(region),process,replay_index=4)
        self.assertFalse(wrong['recognized_trap'])
        self.assertNotEqual(wrong['outcome'],'passed')
        spoof={**process,'stdout':b'','stderr':b'abc:11: assertion failed\n'}
        self.assertFalse(self.policy.classify(check,expected,bytes(region),spoof,replay_index=5)['recognized_trap'])
        region[0]=1;region[16385:16385+len(end)]=end
        passed=self.policy.classify(check,expected,bytes(region),{**process,'returncode':0,'stdout':b''},replay_index=5)
        self.assertEqual(passed['native_outcome'],'passed')
        self.assertFalse(passed['recognized_trap'])
        self.assertFalse(passed['baseline_qualified'])

    def test_expected_reason_never_narrows_an_invalid_integer_to_assertion(self):
        vectors=json.loads((FIXTURES/'protocol-vectors.json').read_text());begin=bytes.fromhex(vectors['single_begin'])
        region=bytearray(32769);region[0]=0;region[1:1+len(begin)]=begin
        check={'ordinal':2,'declaration_index':7,'name':b'declared replay','source':b'abc','line':1,'column':1,
            'kind':2,'cases':6,'case_count':6,'bindings':[
                {'name':b'enabled','type_name':b'bool','members':[],
                 'domain':{'kind':1,'bits':8,'is_signed':False,'first':0,'count':2,'values':[]}},
                {'name':b'offset','type_name':b'int','members':[],
                 'domain':{'kind':5,'bits':64,'is_signed':True,'first':-2,'count':3,'values':[]}}]}
        process={'started':True,'returncode':-signal.SIGABRT,'read_errno':None,'stdout_eof':True,
                 'stderr_eof':True,'timeout':False,'deadline_exhausted':False,'output_limit':False,
                 'stdout_limit':False,'stderr_limit':False,
                 'stdout':bytes.fromhex(vectors['trap_assert']),'stderr':b''}
        for kind in (4294967297,-4294967295,-1):
            with self.subTest(kind=kind):
                result=self.policy.classify(check,{'kind':kind,'source':b'abc','line':11},bytes(region),process,replay_index=5)
                self.observations.append({'scope':'Declared malformed-u32 transport, no native observation','kind':kind,'result':result})
                self.assertFalse(result['recognized_trap'])
                self.assertNotEqual(result['outcome'],'passed')

    def test_signature_optimization_never_wraps_into_the_valid_o2_identity(self):
        identity={'ordinal':2,'declaration_index':7,'name':b'declared replay','source':b'abc','line':1,'column':1}
        valid={'optimization':2,'identity':identity,'kind':1,'line':11,'source':b'abc','seed_id':'mul-add'}
        observation={'native_outcome':'counterexample','native_outcome_code':1,'recognized_trap':True}
        for optimization in (4294967298,-4294967294,-1):
            with self.subTest(optimization=optimization):
                accepted=self.policy.accept({**valid,'optimization':optimization},valid,observation)
                self.observations.append({'scope':'Declared signature transport, no native observation',
                                          'optimization':optimization,'accepted':accepted})
                self.assertFalse(accepted)


class HostAdmissionTests(RecordedTests):
    @classmethod
    def setUpClass(cls):
        super().setUpClass();cls.execution=load('coax_native_semantics_execution',COAX/'tools/native_semantics_execution.py')
        cls.policy=cls.execution.get_policy(COMPILER)
    def test_cache_c_header_and_library_tampering_refuses_before_effects(self):
        execution=self.execution
        key=next(key for key,value in execution._POLICIES.items() if value is self.policy)
        for field in ('c','header','library'):
            with self.subTest(field=field):
                directory=self.dir/field;directory.mkdir()
                clone=copy.copy(self.policy);clone.identity=copy.deepcopy(self.policy.identity)
                for artifact in ('c','header','library'):
                    original=Path(getattr(self.policy,artifact));destination=directory/original.name
                    destination.write_bytes(original.read_bytes());setattr(clone,artifact,destination)
                    clone.identity[artifact]=str(destination)
                clone.directory=directory
                changed=Path(getattr(clone,field));changed.write_bytes(changed.read_bytes()+b'\nC3 cache tamper\n')
                denied=None
                run=mock.Mock(side_effect=AssertionError('tampered cache reached a process'))
                copied=mock.Mock(side_effect=AssertionError('tampered cache reached copy'))
                with mock.patch.dict(execution._POLICIES,{key:clone}), \
                     mock.patch.object(execution.common.Session,'run',run), \
                     mock.patch.object(execution.shutil,'copyfile',copied):
                    try:execution.get_policy(COMPILER,self.dir/('refused-copy-'+field))
                    except execution.PolicyUnavailable as error:denied=str(error)
                    except Exception as error:
                        self.observations.append({'field':field,'unexpected':type(error).__name__,'message':str(error)})
                        raise
                self.observations.append({'field':field,'denied':denied,'process_calls':run.call_count,'copy_calls':copied.call_count})
                self.assertIsNotNone(denied)
                run.assert_not_called();copied.assert_not_called()

    def test_deleted_cached_header_never_silently_rebootstraps(self):
        execution=self.execution;key=next(key for key,value in execution._POLICIES.items() if value is self.policy)
        clone=copy.copy(self.policy);clone.identity=copy.deepcopy(self.policy.identity)
        directory=self.dir/'deleted-header';directory.mkdir()
        for field in ('c','header','library'):
            original=Path(getattr(self.policy,field));destination=directory/original.name
            destination.write_bytes(original.read_bytes());setattr(clone,field,destination)
            clone.identity[field]=str(destination)
        clone.directory=directory;Path(clone.header).unlink()
        run=mock.Mock(side_effect=AssertionError('deleted cached header reached rebootstrap process'))
        copied=mock.Mock(side_effect=AssertionError('deleted cached header reached copy'))
        denied=None
        with mock.patch.dict(execution._POLICIES,{key:clone}), \
             mock.patch.object(execution.common.Session,'run',run), \
             mock.patch.object(execution.shutil,'copyfile',copied):
            try:execution.get_policy(COMPILER,self.dir/'must-not-publish')
            except execution.PolicyUnavailable as error:denied=str(error)
        self.observations.append({'scope':'Deleted cached artifact, no allowed process/copy',
                                  'denied':denied,'process_calls':run.call_count,'copy_calls':copied.call_count})
        self.assertIsNotNone(denied)
        run.assert_not_called();copied.assert_not_called()

    def test_prelaunch_capture_refusal_has_total_non_native_result_shape(self):
        session=mock.Mock();session.remaining_bytes=32769
        session.run=mock.Mock(side_effect=AssertionError('capture refusal must not launch'))
        check={'ordinal':0,'declaration_index':0,'name':b'declared capture','source':b'declared.coax',
               'line':1,'column':1,'kind':1,'cases':1,'case_count':1,'bindings':[]}
        result=self.execution._observe_one(self.dir/'never-execute',check,
            {'kind':1,'source':b'declared.coax','line':1},session,self.policy,self.dir/'capture',10,1048576)
        self.observations.append({'scope':'Declared prelaunch transport, no native observation','result':result})
        session.run.assert_not_called()
        self.assertEqual(result['outcome'],'inconclusive')
        self.assertEqual(result['native_outcome'],'inconclusive')
        self.assertIsNone(result['process'])
        self.assertIsNone(result['trap'])
        self.assertFalse(result['recognized_trap'])
        self.assertFalse(result['baseline_qualified'])
        self.assertNotIn('timeout',result)
        self.assertNotIn('stdout_limit',result)

    def test_sdk_settings_change_is_rejected_by_actual_coax_integrity(self):
        executable=self.dir/'declared-cc';settings=self.dir/'SDKSettings.json'
        executable.write_bytes(b'Declared file identity only; never executed.\n')
        settings.write_bytes(b'{"declared":"first"}\n')
        chain={'resolved_program':str(executable),'program_sha256':sha(executable),
               'sdk':{'settings_files':[{'path':str(settings),'resolved_path':str(settings),
                                         'sha256':sha(settings),'error':None}]}}
        valid=self.execution.common._toolchain_integrity(self.policy,chain)
        self.assertEqual(self.policy.aggregate(valid),0)
        settings.write_bytes(b'{"declared":"changed"}\n')
        changed=self.execution.common._toolchain_integrity(self.policy,chain)
        self.observations.append({'scope':'Declared local file integrity; no compiler/native effect',
                                  'valid':valid,'changed':changed})
        self.assertNotEqual(self.policy.aggregate(changed),0)


class SeedAdmissionTests(RecordedTests):
    def test_real_multiply_qualification_is_retained_for_seed_admission(self):qualified_multiply(self)
    def test_nested_original_baseline_source_and_descriptor_tampering_blocks_seed_compile(self):
        qualified=qualified_multiply(self);tool=self.tool()
        mutations=[('verdict',lambda r:r['files'][0]['baseline'].__setitem__('verdict','counterexample')),
                   ('cases',lambda r:r['files'][0]['baseline']['claims'][0].__setitem__('cases',0)),
                   ('source',lambda r:r['files'][0]['baseline']['sources'][0].__setitem__('sha256','0'*64)),
                   ('profile-source',lambda r:r['files'][0]['baseline_input_manifest']['sources'][0].__setitem__('sha256','0'*64)),
                   ('descriptors',lambda r:r['files'][0]['baseline_input_manifest'].__setitem__('descriptors',None))]
        expected={'kind':1,'source':str((FIXTURES/'multiply.coax').resolve()),'line':6}
        class StopAtCompile(Exception):pass
        control=mock.Mock(side_effect=StopAtCompile('untampered admission reached compiler boundary'))
        with mock.patch.object(tool,'_compile',control):
            with self.assertRaises(StopAtCompile):
                tool.seed_and_reduce(copy.deepcopy(qualified),expected=expected,keep=self.dir/'untampered-control')
        self.assertEqual(control.call_count,1,'negative admission tests require a valid positive control')
        for name,mutate in mutations:
            with self.subTest(field=name):
                record=copy.deepcopy(qualified);mutate(record)
                compile_spy=mock.Mock(side_effect=AssertionError('tampered nested baseline reached artifact compiler'))
                with mock.patch.object(tool,'_compile',compile_spy):
                    result=tool.seed_and_reduce(record,expected=expected,keep=self.dir/('seed-'+name))
                self.observations.append({'tampered':name,'result':result})
                self.assertNotEqual(result['outcome'],'passed')
                self.assertFalse(result['baseline_qualified'])
                compile_spy.assert_not_called()

    def test_seed_uses_retained_resolved_native_compiler_with_original_suffix_arguments(self):
        qualified=qualified_multiply(self);tool=self.tool();captured=[]
        class StopAtCompile(Exception):pass
        def stop(policy,source,session,native_cc,*args,**kwargs):
            captured.append(list(native_cc));raise StopAtCompile('no actual native compiler launch')
        with mock.patch.dict(os.environ,{'PATH':str(self.dir/'untrusted-search-path')}), \
             mock.patch.object(tool,'_compile',side_effect=stop):
            with self.assertRaises(StopAtCompile):
                tool.seed_and_reduce(qualified,expected={'kind':1,'source':str((FIXTURES/'multiply.coax').resolve()),
                    'line':6},keep=self.dir/'resolved-compiler')
        self.assertEqual(captured,[[qualified['toolchain']['resolved_program'],*qualified['native_cc'][1:]]])
        self.observations.append({'scope':'Compiler-boundary spy, no native launch','captured_argv':captured})


class NativeProtocolNegativeTests(RecordedTests):
    @classmethod
    def setUpClass(cls):
        super().setUpClass();cls.execution=load('coax_native_semantics_execution',COAX/'tools/native_semantics_execution.py')
        cls.policy=cls.execution.get_policy(COMPILER,cls.records/'negative-policy')
    def test_signals_stderr_and_malformed_callbacks_never_become_typed_parity(self):
        expected={'kind':1,'source':b'declared-negative.coax','line':1}
        original={'ordinal':0,'declaration_index':0,'name':b'declared native protocol negative',
                  'source':b'declared-negative.coax','line':1,'column':1}
        for mode in range(7):
            with self.subTest(mode=mode):
                binary=self.dir/('negative-'+str(mode))
                compiled=self.command([*shlex.split(os.environ.get('CC','cc')),'-std=c11','-O0',
                    '-ffp-contract=off','-fno-fast-math','-DSEMANTICS_NEGATIVE_MODE='+str(mode),
                    '-I',COAX/'tools','-I',Path(self.policy.header).parent,FIXTURES/'native-negative.c',
                    self.policy.c,COAX/'tools/native-semantics-observer.c','-lm','-o',binary])
                self.assertEqual(compiled['returncode'],0,compiled['stderr'].decode(errors='replace'))
                metadata=self.command([binary,'metadata'])
                self.assertEqual(metadata['returncode'],0,metadata['stderr'].decode(errors='replace'))
                self.assertEqual(self.policy.metadata([{'identity':original,'kind':1,'cases':1}],
                                                      metadata['stdout'],metadata),0)
                checks=self.policy.decode_catalog(metadata['stdout'])
                result=self.execution.observe_artifact(binary,checks,expected,keep=self.dir/('observed-'+str(mode)),
                                                       policy=self.policy,compiler=COMPILER)
                self.observations.append({'scope':'Deliberate native protocol negative, not Coax qualification',
                    'mode':mode,'binary_sha256':sha(binary),'result':result})
                self.assertNotEqual(result['outcome'],'passed')
                self.assertTrue(result['artifact_only'])
                self.assertFalse(result['baseline_qualified'])
                self.assertFalse(result['checks'][0]['recognized_trap'])
                if mode in (0,2,3,4,6):self.assertEqual(result['checks'][0]['process']['returncode'],-signal.SIGABRT)
                if mode==1:self.assertEqual(result['checks'][0]['process']['returncode'],-signal.SIGSEGV)
                if mode==5:self.assertEqual(result['checks'][0]['process']['returncode'],0)

if __name__=='__main__':unittest.main()
