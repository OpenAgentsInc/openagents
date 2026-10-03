"""No-network fixtures cover registration, admission, and interrupted accounting."""
import hashlib
import json
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest.mock import patch
import uuid

import run_coverage_review as runner

HARNESS = Path.cwd()


def save(path,value):
    path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(runner.encoded(value)+b'\n')


class Fixture:
    def __init__(self,root):
        self.root=Path(root);self.evidence=self.root/'evidence';self.output=self.root/'prepared-output'
        self.bundle=self.root/'bundle';self.bundle.mkdir();self.harness=self.root/'harness'
        original=runner.read(runner.HERE/'freeze.json')
        for name in original['prototype']:shutil.copyfile(runner.HERE/name,self.bundle/name)
        for name in original['imported_modules']:
            target=self.harness/name;target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(HARNESS/name,target)
        self.schedule=[]
        module_hashes={'jev-lifecycle/run_native_panel.py':'d'*64}
        task={'id':'alternative-beta','title':'Synthetic task','prompt':'Preserve values. Read the contract.',
              'source_commit':'a'*40,'packages':['toy'],'allowed_paths':['crates/toy/']}
        save(self.evidence/'inputs/alternative-beta/task.json',{'tasks':[task]})
        save(self.evidence/'inputs/alternative-beta/index.json',{'commit':'a'*40})
        for position in range(1,13):
            run_id=str(uuid.uuid4());arm=('bare','deterministic','jev')[(position-1)%3]
            config={'run_id':run_id,'arm':arm,'task_id':task['id'],'module_hashes':module_hashes,
                    'task_manifest':{'path':'/PRIVATE_REMOTE/task.json','sha256':runner.file_ref(self.evidence/'inputs/alternative-beta/task.json')['sha256']},
                    'index':{'path':'/PRIVATE_REMOTE/index.json','sha256':runner.file_ref(self.evidence/'inputs/alternative-beta/index.json')['sha256']},
                    'native_template':{'source_archive_sha256':'b'*64},'acceptance_template':{'checker':'PRIVATE_ORACLE'}}
            config_path=self.evidence/'plan'/f'{run_id}.json';save(config_path,config)
            entry={'position':position,'run_id':run_id,'task_id':task['id'],'arm':arm,'repetition':1+(position>6),
                   'config':{'path':'/REMOTE/'+run_id+'.json','sha256':runner.file_ref(config_path)['sha256']}}
            self.schedule.append(entry)
            native=self.evidence/'runs'/run_id/'native';native.mkdir(parents=True)
            save(native/'candidate-manifest.json',{'synthetic':position});candidate_hash=runner.file_ref(native/'candidate-manifest.json')['sha256']
            (native/'candidate.tar.gz').write_bytes(b'synthetic payload');save(native/'changes.json',{})
            save(native/'result.json',{'run_id':run_id,'source_commit':'a'*40,'source_archive_sha256':'b'*64,
                                      'candidate_manifest_sha256':candidate_hash,'accepted':False,'model':'PRIVATE_NATIVE_MODEL'})
            save(native.parent/'pilot.json',{'run_id':run_id,'candidate_manifest_sha256':candidate_hash,
                'accepted':True,'execution_closed':True,'finished_at':'synthetic','artifacts':{'native':{'sha256':runner.file_ref(native/'result.json')['sha256']}}})
        plan={'schema':'openagents.jev-lifecycle.native-pilot-plan.v1','schedule':self.schedule,'module_hashes':module_hashes,
              'capability_probe':{'sha256':'e'*64}}
        save(self.evidence/'plan/plan.json',plan)
        attempts=[dict(row,receipt_sha256=runner.file_ref(self.evidence/'runs'/row['run_id']/'pilot.json')['sha256']) for row in self.schedule]
        save(self.evidence/'panel/panel.json',{'schema':'openagents.jev-lifecycle.native-panel.v1','status':'complete','finished_at':'synthetic',
            'plan_sha256':runner.file_ref(self.evidence/'plan/plan.json')['sha256'],'driver_sha256':'d'*64,'probe_sha256':'e'*64,'attempts':attempts})
        original['original_native_plan']=runner.file_ref(self.evidence/'plan/plan.json')
        save(self.bundle/'freeze.json',original);self.freeze=runner.file_ref(self.bundle/'freeze.json')['sha256']
        self.preparations=0;self.calls=0;self.skip=set();self.costs=[];self.raise_after_receipt=None

    def prepare_case(self,harness,repo,task,index,native,identity,archive):
        self.preparations+=1
        if self.preparations in self.skip:
            return {'ready':False,'status':'skipped_mandatory_request_bound','errors':['mandatory_source_and_contracts_exceed_request_bound'],'model_calls':0,'request':None}
        request={'model':'typesafe-ai/jev','state':{'task':task['prompt'],'source_commit':task['source_commit'],'source':'synthetic candidate'},
                 'questions':{'r01':{'type':'choice','instructions':'Does the visible source implement the exact clause?',
                                    'criteria':{'demonstrated':'Visible handling','insufficient_evidence':'Missing evidence'}}}}
        raw=runner.encoded(request)
        return {'ready':True,'status':'ready','errors':[],'model_calls':0,'request':request,
                'request_bytes':len(raw),'request_sha256':hashlib.sha256(raw).hexdigest()}

    def prepare(self):
        return runner.prepare_all(self.harness,self.root/'unused-repo',self.evidence,self.output,
            bundle=self.bundle,expected_freeze=self.freeze,prepare_fn=self.prepare_case)

    def call(self,state,questions,out,timeout):
        self.calls+=1;self.last_timeout=timeout
        out.mkdir()
        raw=runner.encoded({'model':'typesafe-ai/jev','state':state,'questions':questions})
        (out/'request.json').write_bytes(raw)
        cost=self.costs[self.calls-1] if self.costs else .001
        receipt={'request_sha256':hashlib.sha256(raw).hexdigest(),'cost_usd':cost,'attempts':1,
                 'cost_status':'gateway_reported' if cost is not None else 'unknown','answers_valid':cost is not None}
        save(out/'receipt.json',receipt)
        if self.raise_after_receipt:raise self.raise_after_receipt()
        return receipt,{'answers':{}} if cost is not None else None

    def execute(self,**extra):
        return runner.execute(self.harness,self.evidence,self.output,runner.file_ref(self.output/'registration.json')['sha256'],
            bundle=self.bundle,expected_freeze=self.freeze,call_fn=self.call,**extra)


class RunnerTests(unittest.TestCase):
    def test_prepare_binds_all_twelve_and_oversize_skip_without_any_call(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture=Fixture(directory);fixture.skip={2,6}
            prepared=fixture.prepare()
            self.assertEqual(fixture.preparations,12);self.assertEqual(fixture.calls,0)
            self.assertEqual(prepared['model_calls'],0)
            self.assertEqual(len(prepared['cases']),12)
            self.assertEqual(sum(c['ready'] for c in prepared['cases']),10)
            self.assertEqual(prepared['cases'][1]['preparation_status'],'skipped_mandatory_request_bound')
            self.assertFalse((fixture.output/'prepared/02/request.json').exists())
            for path in (fixture.output/'prepared').glob('*/request.json'):
                raw=path.read_text()
                self.assertNotIn('PRIVATE_ORACLE',raw);self.assertNotIn('PRIVATE_NATIVE_MODEL',raw);self.assertNotIn('accepted',raw)
            self.assertFalse((fixture.output/'live.claim').exists())

    def test_cost_stop_and_skips_preserve_all_slots_and_no_retries(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture=Fixture(directory);fixture.skip={2};fixture.costs=[.03,.021]
            fixture.prepare();result=fixture.execute()
            self.assertEqual(fixture.calls,2);self.assertEqual(result['application_calls'],2)
            self.assertAlmostEqual(result['known_cost_usd'],.051)
            self.assertEqual(result['status'],'stopped_cost_admission')
            self.assertEqual(len(result['slots']),12)
            self.assertEqual(result['slots'][1]['status'],'skipped_preparation')
            self.assertEqual(result['slots'][3]['status'],'not_admitted_budget')
            self.assertEqual(fixture.last_timeout,30)
            with self.assertRaises(FileExistsError):fixture.execute()

    def test_unknown_cost_stops_and_keeps_prior_known_cost(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture=Fixture(directory);fixture.costs=[.01,None]
            fixture.prepare();result=fixture.execute()
            self.assertEqual(fixture.calls,2)
            self.assertFalse(result['accounting_complete']);self.assertIsNone(result['cost_usd'])
            self.assertEqual(result['known_cost_usd'],.01)
            self.assertEqual(result['status'],'stopped_unknown_cost')
            self.assertEqual(result['slots'][2]['status'],'not_called_after_stop')

    def test_outer_interruption_retains_gateway_checkpoint_and_never_retries(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture=Fixture(directory);fixture.costs=[None];fixture.raise_after_receipt=runner.CallDeadline
            fixture.prepare();result=fixture.execute()
            self.assertEqual(fixture.calls,1);self.assertEqual(result['slots'][0]['error_type'],'CallDeadline')
            self.assertTrue((fixture.output/'calls/01/gateway/receipt.json').is_file())
            self.assertFalse(result['accounting_complete'])
            with self.assertRaises(FileExistsError):fixture.execute()

    def test_missing_receipt_or_malformed_return_never_infers_zero_cost(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture=Fixture(directory);fixture.prepare()
            def failed(*args,**kwargs):raise RuntimeError('PRIVATE_AUTH_SENTINEL')
            fixture.call=failed;result=fixture.execute()
            self.assertEqual(result['status'],'stopped_unknown_cost');self.assertFalse(result['accounting_complete'])
            self.assertNotIn('PRIVATE_AUTH_SENTINEL',(fixture.output/'execution.json').read_text())
        with tempfile.TemporaryDirectory() as directory:
            fixture=Fixture(directory);fixture.prepare();fixture.call=lambda *a,**k:(None,None)
            result=fixture.execute()
            self.assertEqual(result['status'],'stopped_unknown_cost')
            self.assertFalse(result['accounting_complete']);self.assertIsNone(result['cost_usd'])

    def test_interrupted_known_charge_is_retained_even_without_a_valid_response(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture=Fixture(directory);fixture.costs=[.012];fixture.raise_after_receipt=runner.StopRequested
            fixture.prepare();result=fixture.execute()
            self.assertEqual(result['status'],'stopped_interrupted')
            self.assertEqual(result['known_cost_usd'],.012)
            self.assertTrue(result['accounting_complete'])
            self.assertEqual(fixture.calls,1)

    def test_tampered_request_registration_and_imported_module_refuse_before_calls(self):
        for which in ('request','registration','imported'):
            with self.subTest(which=which),tempfile.TemporaryDirectory() as directory:
                fixture=Fixture(directory);fixture.prepare()
                registered=runner.file_ref(fixture.output/'registration.json')['sha256']
                target={'request':fixture.output/'prepared/01/request.json','registration':fixture.output/'registration.json',
                        'imported':fixture.harness/'bench/jev-lifecycle/context.py'}[which]
                target.write_bytes(target.read_bytes()+b' ')
                with self.assertRaises(ValueError):
                    runner.execute(fixture.harness,fixture.evidence,fixture.output,registered,
                        bundle=fixture.bundle,expected_freeze=fixture.freeze,call_fn=fixture.call)
                self.assertEqual(fixture.calls,0)
                self.assertFalse((fixture.output/'live.claim').exists())

    def test_complete_batch_makes_at_most_twelve_calls_and_cannot_reprepare(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture=Fixture(directory);fixture.prepare();result=fixture.execute()
            self.assertEqual(result['status'],'complete');self.assertEqual(fixture.calls,12)
            self.assertEqual(result['gateway_attempts'],12)
            with self.assertRaises(FileExistsError):fixture.prepare()

    def test_gateway_setup_failure_retains_zero_call_receipt(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture=Fixture(directory);fixture.prepare()
            with patch.object(runner,'load',side_effect=ImportError('PRIVATE_AUTH_SENTINEL')):
                result=fixture.execute()
            self.assertEqual(result['status'],'stopped_infrastructure')
            self.assertEqual(result['application_calls'],0)
            self.assertEqual(result['cost_usd'],0)
            self.assertTrue(result['gateway_attempts_complete'])
            self.assertEqual(len(result['slots']),12)
            self.assertTrue((fixture.output/'live.claim').is_file())
            self.assertNotIn('PRIVATE_AUTH_SENTINEL',(fixture.output/'execution.json').read_text())
            with self.assertRaises(FileExistsError):fixture.execute()

    def test_signal_between_gateway_return_and_accounting_recovers_attempt_and_cost(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture=Fixture(directory);fixture.prepare();fixture.costs=[.009]
            original=runner.known_cost;observations=0
            def interrupted_once(receipt):
                nonlocal observations
                observations+=1
                if observations==1:raise runner.StopRequested()
                return original(receipt)
            with patch.object(runner,'known_cost',side_effect=interrupted_once):
                result=fixture.execute()
            self.assertEqual(result['application_calls'],1)
            self.assertEqual(result['gateway_attempts'],1)
            self.assertTrue(result['gateway_attempts_complete'])
            self.assertEqual(result['known_cost_usd'],.009)
            self.assertTrue(result['accounting_complete'])

    def test_ended_panel_without_execution_closure_refuses_all_preparation(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture=Fixture(directory);run_id=fixture.schedule[0]['run_id']
            pilot_path=fixture.evidence/'runs'/run_id/'pilot.json'
            pilot=runner.read(pilot_path);pilot['execution_closed']=False;save(pilot_path,pilot)
            panel_path=fixture.evidence/'panel/panel.json';panel=runner.read(panel_path)
            panel['status']='stopped_incomplete_attempt'
            panel['attempts'][0]['receipt_sha256']=runner.file_ref(pilot_path)['sha256'];save(panel_path,panel)
            with self.assertRaises(ValueError):fixture.prepare()
            self.assertEqual(fixture.preparations,0);self.assertEqual(fixture.calls,0)
            self.assertFalse(fixture.output.exists())

    def test_running_panel_cannot_prepare_and_confirmed_no_call_is_zero(self):
        with tempfile.TemporaryDirectory() as directory:
            fixture=Fixture(directory);path=fixture.evidence/'panel/panel.json';value=runner.read(path);value['status']='running';save(path,value)
            with self.assertRaises(ValueError):fixture.prepare()
            self.assertEqual(fixture.calls,0);self.assertFalse(fixture.output.exists())
        self.assertEqual(runner.known_cost({'cost_status':'no_call','cost_usd':0,'attempts':0}),0)
        self.assertIsNone(runner.known_cost({'cost_status':'no_call','cost_usd':0,'attempts':1}))


if __name__=='__main__':unittest.main()
