"""Single-trial integration with injected phases; no live preflight or model call."""
import copy
import fcntl
import json
import os
from pathlib import Path
import tarfile
import tempfile
import sys
import unittest
from unittest.mock import patch

import candidate
import report
import trial
from test_report import Fixture, PRICES
import test_schedule


class TrialTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.fixture = Fixture(self.root)
        self.registration = test_schedule.ScheduleTests.registration(self)
        self.runtime = {'schema':'openagents.delegation.trial-config.v1',
                        'preparation_timeout_s':60,'native_timeout_s':1500,'acceptance_timeout_s':260,
                        'native_common':{'cli_budget_usd':2,'timeout_s':600,'toolchain':{'environment':{'CARGO_PROFILE_DEV_DEBUG':'0','CARGO_PROFILE_TEST_DEBUG':'0'}},
                                         'provider_meter':{'admission_target_usd':8,'models':{
                                             model:{'max_requests':100,'usd_per_million':rates} for model,rates in PRICES.items()}}},
                        'tasks':{},'arms':{}}
        for task in self.fixture.manifest['tasks']:
            template={'allowed_paths':['src'],'packages':['fixture'],'total_timeout_s':240,
                      'toolchain':{'environment':{'CARGO_PROFILE_DEV_DEBUG':'0','CARGO_PROFILE_TEST_DEBUG':'0'}},'checker':{'path':str(self.root/'checker.rs'),
                        'sha256':self.registration['task_artifacts'][task['task_id']]['checker']['sha256']}}
            self.runtime['tasks'][task['task_id']]={'source_repo':str(self.root/'source-repo'),
                'target_seed':str(self.root/'seed'), 'acceptance_template':self.fixture.write('runtime/'+task['task_id']+'.json',template)}
        for arm, config in self.fixture.manifest['arms'].items():
            runtime={'tools':None} if not config['preparation'] else {'tools':'Bash,Read,Edit,Write,Glob,Grep',
                         'system_prompt':self.fixture.write('runtime/system.txt',b'lean system',raw=True)}
            self.runtime['arms'][arm]=runtime
            config['argv_tail']=trial.native_arguments({'model':config['primary_model'],'effort':'medium',
                                 'cli_budget_usd':2,'tools':runtime['tools'],
                                 **({'system_file':'some path'} if config['preparation'] else {})})
        self.registration['artifacts']['trial_config']=self.fixture.write('runtime/config.json',self.runtime)
        self.protocol=self.fixture.protocol
        self.protocol['registration']['report_bindings']['arms']=copy.deepcopy(self.fixture.manifest['arms'])
        self.registration['protocol']=self.fixture.write('protocol.json',self.protocol)
        self.path=self.root/'registration.json'
        self.path.write_text(json.dumps(self.registration))
        self.credential=self.root/'credential.private'
        self.credential.write_text('synthetic fixture only')
        self.credential.chmod(0o600)
        self.calls=[]
        self.fail_phase=None
        self.quality=True
        self.budget_exit=False
        self.native_charge=None
        self.jev_fallback=False

    def choose_first(self, arm):
        # Fixture schedule order is synthetic and is never represented as a live seal.
        cells=self.registration['schedule']
        first=next(i for i,c in enumerate(cells[:6]) if c['arm']==arm)
        cells[0],cells[first]=cells[first],cells[0]
        self.protocol['registration']['report_bindings']['schedule']=copy.deepcopy(cells)
        self.registration['protocol']=self.fixture.write('protocol.json',self.protocol)
        self.path.write_text(json.dumps(self.registration))
        return cells[0]['run_id']

    def fake_phase(self, name, argv, output, timeout):
        records=[json.loads(line) for line in (output/'launches.jsonl').read_text().splitlines()]
        self.assertEqual(records[-1]['event'],'launch_intent')
        self.assertEqual(records[-1]['phase'],name)
        self.calls.append(name)
        if self.fail_phase==name:
            raise RuntimeError('injected interrupted phase')
        if name=='preparation':
            destination=Path(argv[argv.index('--output')+1]);destination.mkdir()
            run_id=records[-1]['run_id']
            cell=next(c for c in self.registration['schedule'] if c['run_id']==run_id)
            task=next(t for t in self.protocol['registration']['report_bindings']['tasks'] if t['task_id']==cell['task_id'])
            pack=b'prepared exact bytes\r\n'
            (destination/'briefing.md').write_bytes(pack)
            (destination/'candidates.json').write_bytes((self.root/(task['task_id']+'/pool.json')).read_bytes())
            mode=argv[argv.index('--mode')+1]
            call=None
            if mode=='jev':
                call={'attempts':1,'requested_model':'jev-1.13.0','served_model':'jev-1.13.0',
                      'usage':{'input_tokens':100},'cost_usd':.0000042,'outcome':'answered','selection':'system_one'}
                if self.jev_fallback:
                    call={'attempts':1,'requested_model':'jev-1.13.0','selection':'deterministic_fallback',
                          'outcome':'failed','cost_usd':None,'http_status':402}
            prep={**task['preparation'],'mode':mode,'coverage':{'candidate_units':1},'system_one':call,
                  'briefing_sha256':report.sha(pack),'briefing_bytes':len(pack)}
            (destination/'preparation.json').write_text(json.dumps(prep))
        elif name=='native':
            config=json.loads(Path(argv[-2]).read_text());destination=Path(argv[-1]);destination.mkdir()
            prompt=Path(config['prompt_file']).read_bytes()
            (destination/'changes.json').write_text('{}')
            with tarfile.open(destination/'candidate.tar.gz','w:gz'):
                pass
            identity,payload=candidate.write_manifest(destination,config['source_commit'],config['source_archive_sha256'],{})
            usage={'input_tokens':100,'output_tokens':10}
            amount=report.price_bounds(usage,PRICES[config['model']])[0]
            row={'schema':'openagents.delegation.native-attempt.v1','run_id':config['run_id'],
                 'status':'complete','execution_closed':True,'model_completed':not self.budget_exit,
                 'exit_code':1 if self.budget_exit else 0,'cost_usd':self.native_charge or amount,
                 'model':config['model'],'effort':config['effort'],'source_commit':config['source_commit'],
                 'source_archive_sha256':config['source_archive_sha256'],'cli_sha256':config['binary_sha256'],
                 'cli_hash_after':config['binary_sha256'],'cli_version':config['binary_version'],
                 'argv':[config['binary']]+trial.native_arguments(config),'prompt_sha256':report.sha(prompt),
                 'delivered_prompt_sha256':report.sha(('Benchmark run ID: '+config['run_id']+'\n\n').encode()+prompt),
                 'candidate_manifest_sha256':identity,'candidate_payload_sha256':payload,'snapshot_commit':'1'*40}
            if config.get('cargo_features'):row['cargo_features']=config['cargo_features']
            (destination/'result.json').write_text(json.dumps(row))
            provider=[{'run_id':config['run_id'],'phase':'admitted','call_id':config['run_id']+'-call',
                       'model':config['model'],'path':'/v1/messages'},
                      {'run_id':config['run_id'],'phase':'finished','call_id':config['run_id']+'-call',
                       'status':'complete','http_status':200,'served_model':config['model'],'usage_status':'reported',
                       'usage':usage,'cost_usd':amount}]
            (destination/'provider-calls.jsonl').write_text(''.join(json.dumps(r)+'\n' for r in provider))
        elif name=='acceptance':
            config=json.loads(Path(argv[-2]).read_text());destination=Path(argv[-1]);destination.mkdir()
            self.assertEqual(config['expected_snapshot_commit'],'1'*40)
            checks={'schema':'openagents.delegation.final-checks.v1','run_id':config['run_id'],
                    'candidate_manifest_sha256':config['candidate_manifest_sha256'],'completed':True,'execution_closed':True,
                    **{k:{'passed':True} for k in ('scope','format','ordinary','independent')}}
            if config.get('cargo_features'):checks['cargo_features']=config['cargo_features']
            checks['independent']['passed']=self.quality
            (destination/'checks.json').write_text(json.dumps(checks))
        return {'exit_code':0,'timed_out':False}

    def run_trial(self, run_id, validator=None):
        # Test-only injection skips real registration; no fake live call is performed.
        return trial.run(self.path,run_id,self.root/'runs'/run_id,self.credential,
                         phase_runner=self.fake_phase,validator=validator or (lambda path:{'ready_for_external_dispatch':True}))

    def test_control_has_exact_base_prompt_and_all_bound_artifacts(self):
        run_id=self.choose_first('A');result=self.run_trial(run_id)
        self.assertEqual(result['status'],'complete',result)
        self.assertTrue(result['accepted']);self.assertTrue(result['accounting_complete'])
        self.assertEqual(self.calls,['native','acceptance'])
        task=next(t for t in self.protocol['registration']['report_bindings']['tasks'] if t['task_id']==result['cell']['task_id'])
        self.assertEqual((self.root/result['artifacts']['prompt']['path']).read_bytes(),(self.root/task['base_prompt']['path']).read_bytes())
        endpoint=report.artifact(self.root,result['artifacts']['endpoint'])
        self.assertTrue(endpoint['execution_closed'])
        self.assertLess(endpoint['start_monotonic_ns'],result['phases'][0]['start_monotonic_ns'])
        self.assertGreaterEqual(endpoint['end_monotonic_ns'],result['phases'][-1]['end_monotonic_ns'])
        for ref in result['artifacts'].values():
            self.assertTrue((self.root/ref['path']).is_file())
        public=json.dumps(result)+json.dumps(endpoint)+(self.root/'runs'/run_id/'launches.jsonl').read_text()
        self.assertNotIn(str(self.credential),public)
        self.assertNotIn('synthetic fixture only',public)

    def feature_task(self, prompt=True, acceptance=True):
        run_id=self.choose_first('A')
        cell=next(c for c in self.registration['schedule'] if c['run_id']==run_id)
        task=next(t for t in self.protocol['registration']['report_bindings']['tasks'] if t['task_id']==cell['task_id'])
        task['cargo_features']=['fixture/blocking']
        if prompt:
            task['base_prompt']=self.fixture.write('runtime/feature-prompt.txt',
                b'Common check: cargo test --locked --offline -p fixture --features fixture/blocking',raw=True)
        if acceptance:
            settings=self.runtime['tasks'][task['task_id']]
            template=report.artifact(self.root,settings['acceptance_template'])
            template['cargo_features']=['fixture/blocking']
            settings['acceptance_template']=self.fixture.write('runtime/feature-template.json',template)
        self.registration['artifacts']['trial_config']=self.fixture.write('runtime/config.json',self.runtime)
        self.registration['protocol']=self.fixture.write('protocol.json',self.protocol)
        self.path.write_text(json.dumps(self.registration))
        return run_id

    def test_feature_policy_reaches_native_and_acceptance_with_common_prompt(self):
        run_id=self.feature_task();result=self.run_trial(run_id)
        self.assertTrue(result['accepted'],result)
        for name in ('native','acceptance'):
            config=json.loads((self.root/'runs'/run_id/(name+'-config.private.json')).read_text())
            self.assertEqual(config['cargo_features'],['fixture/blocking'])
        prompt=report.artifact(self.root,result['artifacts']['prompt'],'bytes')
        self.assertIn(b'cargo test --locked --offline -p fixture --features fixture/blocking',prompt)

    def test_missing_common_feature_instruction_blocks_before_paid_phase(self):
        result=self.run_trial(self.feature_task(prompt=False))
        self.assertEqual(result['status'],'failed')
        self.assertEqual(self.calls,[])
        self.assertEqual(result['cost_upper_usd'],0)

    def test_acceptance_feature_mismatch_blocks_before_paid_phase(self):
        result=self.run_trial(self.feature_task(acceptance=False))
        self.assertEqual(result['status'],'failed')
        self.assertEqual(self.calls,[])

    def test_acceptance_receipt_cannot_drop_required_features(self):
        run_id=self.feature_task()
        def dropped(name,argv,output,timeout):
            result=self.fake_phase(name,argv,output,timeout)
            if name=='acceptance':
                path=Path(argv[-1])/'checks.json';value=json.loads(path.read_text())
                value.pop('cargo_features');path.write_text(json.dumps(value))
            return result
        result=trial.run(self.path,run_id,self.root/'runs'/run_id,self.credential,phase_runner=dropped,
                         validator=lambda path:{'ready_for_external_dispatch':True})
        self.assertEqual(result['status'],'failed')
        self.assertIsNone(result['accepted'])

    def test_native_feature_receipt_must_match_registered_task(self):
        run_id=self.feature_task()
        def mismatched(name,argv,output,timeout):
            result=self.fake_phase(name,argv,output,timeout)
            if name=='native':
                path=Path(argv[-1])/'result.json';value=json.loads(path.read_text())
                value['cargo_features']=['fixture/other'];path.write_text(json.dumps(value))
            return result
        result=trial.run(self.path,run_id,self.root/'runs'/run_id,self.credential,phase_runner=mismatched,
                         validator=lambda path:{'ready_for_external_dispatch':True})
        self.assertEqual(self.calls,['native'])
        self.assertEqual(result['status'],'failed')
        self.assertIsNone(result['accepted'])

    def test_prepared_prompt_preserves_exact_pack_and_jev_cost(self):
        run_id=self.choose_first('F');result=self.run_trial(run_id)
        self.assertEqual(result['status'],'complete',result)
        self.assertEqual(self.calls,['preparation','native','acceptance'])
        prompt=(self.root/result['artifacts']['prompt']['path']).read_bytes()
        self.assertEqual(prompt,b'public task and common instructions\n\nprepared exact bytes\r\n')
        self.assertAlmostEqual(result['cost_upper_usd'],.0003+.0000042)

    def test_failed_quality_and_budget_exit_are_retained_without_external_repair(self):
        self.quality=False;self.budget_exit=True
        run_id=self.choose_first('D');result=self.run_trial(run_id)
        self.assertEqual(result['status'],'complete',result)
        self.assertFalse(result['accepted']);self.assertEqual(self.calls,['native','acceptance'])
        self.assertGreater(result['cost_lower_usd'],0)

    def test_existing_attempt_is_inspect_only_even_after_paid_launch_failure(self):
        self.fail_phase='native';run_id=self.choose_first('A');first=self.run_trial(run_id)
        self.assertEqual(first['status'],'incomplete')
        self.assertFalse(first['execution_closed']);self.assertIsNone(first['cost_upper_usd'])
        calls=list(self.calls);second=self.run_trial(run_id)
        self.assertTrue(second['inspection_only']);self.assertFalse(second['replayed'])
        self.assertEqual(self.calls,calls)
        self.assertIn('endpoint',first['artifacts'])

    def test_unknown_jev_fallback_stays_assigned_and_blocks_further_admission(self):
        self.jev_fallback=True;run_id=self.choose_first('F');result=self.run_trial(run_id)
        self.assertTrue(result['accepted']);self.assertFalse(result['accounting_complete'])
        second_id=self.registration['schedule'][1]['run_id'];calls=list(self.calls)
        second=self.run_trial(second_id)
        self.assertEqual(second['status'],'failed');self.assertEqual(self.calls,calls)

    def test_unsealed_registration_never_launches_a_phase(self):
        run_id=self.choose_first('A')
        self.protocol['registration']['sealed']=False
        self.registration['protocol']=self.fixture.write('protocol.json',self.protocol)
        self.path.write_text(json.dumps(self.registration))
        result=self.run_trial(run_id,trial.schedule.validate)
        self.assertEqual(self.calls,[]);self.assertEqual(result['status'],'failed')
        self.assertEqual(result['cost_upper_usd'],0)
        self.assertIn('endpoint',result['artifacts'])

    def test_native_charge_above_broker_keeps_accounting_unknown(self):
        self.native_charge=10
        result=self.run_trial(self.choose_first('A'))
        self.assertTrue(result['accepted']);self.assertFalse(result['accounting_complete'])
        self.assertIn('native_cost_exceeds_provider_upper',result['errors'])

    def test_reordered_or_alternate_directory_attempt_cannot_launch(self):
        run_id=self.choose_first('A')
        later=self.registration['schedule'][1]['run_id']
        result=self.run_trial(later)
        self.assertEqual(self.calls,[]);self.assertEqual(result['status'],'failed')
        with self.assertRaises(ValueError):
            trial.run(self.path,run_id,self.root/'alternate',self.credential,phase_runner=self.fake_phase)

    def test_second_arm_uses_original_block_admission_and_counts_prior_cost(self):
        run_id=self.choose_first('A');first=self.run_trial(run_id)
        second_id=self.registration['schedule'][1]['run_id'];second=self.run_trial(second_id)
        self.assertEqual(second['status'],'complete',second)
        self.assertEqual(first['block_admission'],second['block_admission'])

    def test_full_block_budget_refusal_keeps_zero_cost_failure_receipt(self):
        self.registration['accounting']=[{'run_id':'prior-work','cost_upper_usd':100}]
        run_id=self.choose_first('A');result=self.run_trial(run_id)
        self.assertEqual(self.calls,[]);self.assertEqual(result['status'],'failed')
        self.assertEqual(result['cost_upper_usd'],0)

    def test_runtime_config_tampering_blocks_all_phases(self):
        run_id=self.choose_first('A')
        (self.root/self.registration['artifacts']['trial_config']['path']).write_text('{}')
        result=self.run_trial(run_id)
        self.assertEqual(self.calls,[]);self.assertEqual(result['status'],'failed')
        self.assertTrue(result['execution_closed'])

    def test_active_coordinator_lock_prevents_another_launch(self):
        run_id=self.choose_first('A')
        with (self.root/'trial-coordinator.lock').open('a') as lock:
            fcntl.flock(lock.fileno(),fcntl.LOCK_EX|fcntl.LOCK_NB)
            with self.assertRaisesRegex(ValueError,'execution slot'):
                self.run_trial(run_id)
        self.assertEqual(self.calls,[])

    def test_real_fixture_process_and_timeout_are_not_retried(self):
        output=self.root/'process-fixture';output.mkdir()
        result=trial.process_phase('fixture',[sys.executable,'-c','print("local fixture")'],output,5)
        self.assertEqual(result,{'exit_code':0,'timed_out':False})
        self.assertEqual((output/'fixture.stdout.log').read_text(),'local fixture\n')
        timeout=trial.process_phase('timeout',[sys.executable,'-c','import time;time.sleep(5)'],output,.01)
        self.assertTrue(timeout['timed_out'])

    def test_report_entry_is_bound_but_does_not_invent_a_blinded_review(self):
        result=self.run_trial(self.choose_first('A'))
        entry=report.artifact(self.root,result['report_entry'])
        self.assertEqual(entry['run_id'],result['run_id'])
        self.assertEqual(entry['artifacts']['endpoint'],result['artifacts']['endpoint'])
        self.assertNotIn('review',entry['artifacts'])

    def test_malformed_native_result_still_retains_unknown_endpoint(self):
        run_id=self.choose_first('A')
        def malformed(name,argv,output,timeout):
            result=self.fake_phase(name,argv,output,timeout)
            if name=='native':
                (Path(argv[-1])/'result.json').write_text('[1]')
            return result
        result=trial.run(self.path,run_id,self.root/'runs'/run_id,self.credential,phase_runner=malformed,
                         validator=lambda path:{'ready_for_external_dispatch':True})
        self.assertFalse(result['execution_closed']);self.assertIsNone(result['cost_upper_usd'])
        self.assertIn('endpoint',result['artifacts'])

    def test_native_scratch_is_durably_retained_then_removed_before_acceptance(self):
        run_id=self.choose_first('A')
        shared=self.root/'shared-target';shared.mkdir();(shared/'keep').write_text('shared')
        def with_targets(name,argv,output,timeout):
            if name == 'acceptance':
                self.assertFalse((output/'native'/'workspace').exists())
                self.assertFalse((output/'native'/'home'/'target').exists())
                retained=json.loads((output/'native-retention.json').read_text())
                for ref in retained['artifacts'].values():
                    report.artifact(self.root,ref,'binary')
                self.assertTrue(retained['private_logs'])
                released=json.loads((output/'native-scratch-release.json').read_text())
                self.assertEqual(released['status'],'complete')
            result=self.fake_phase(name,argv,output,timeout)
            if name in ('native','acceptance'):
                target=Path(argv[-1])/'home'/'target';target.mkdir(parents=True)
                (target/'build-artifact').write_text('owned')
                workspace=Path(argv[-1])/'workspace';workspace.mkdir()
                (workspace/'reconstructible.rs').write_text('owned source')
                (Path(argv[-1])/'retained.stdout').write_text('private log')
            return result
        result=trial.run(self.path,run_id,self.root/'runs'/run_id,self.credential,phase_runner=with_targets,
                         validator=lambda path:{'ready_for_external_dispatch':True})
        self.assertTrue(result['cleanup']['outside_primary_endpoint'])
        self.assertEqual(len(result['cleanup']['targets']),2)
        self.assertTrue(all(row['removed'] for row in result['cleanup']['targets']))
        released=result['native_scratch_release']
        self.assertEqual(len(released['targets']),2)
        self.assertTrue(all(row['removed'] for row in released['targets']))
        self.assertTrue(released['inside_primary_endpoint'])
        self.assertGreaterEqual(released['free_bytes_before'],0)
        self.assertGreaterEqual(released['free_bytes_after'],0)
        endpoint=report.artifact(self.root,result['artifacts']['endpoint'])
        self.assertLess(endpoint['start_monotonic_ns'],released['start_monotonic_ns'])
        self.assertLessEqual(released['end_monotonic_ns'],result['phases'][-1]['start_monotonic_ns'])
        self.assertLessEqual(released['end_monotonic_ns'],endpoint['end_monotonic_ns'])
        self.assertEqual((shared/'keep').read_text(),'shared')
        self.assertTrue((self.root/'runs'/run_id/'native'/'candidate-manifest.json').is_file())
        self.assertTrue((self.root/'runs'/run_id/'acceptance'/'checks.json').is_file())
        self.assertEqual((self.root/'runs'/run_id/'native'/'retained.stdout').read_text(),'private log')
        self.assertTrue(result['private_logs'])
        for refs in self.registration['task_artifacts'].values():
            self.assertTrue((self.root/refs['source_archive']['path']).is_file())

    def test_failed_native_scratch_release_blocks_acceptance_without_cleanup_retry(self):
        run_id=self.choose_first('A')
        def with_scratch(name,argv,output,timeout):
            result=self.fake_phase(name,argv,output,timeout)
            if name=='native':
                (output/'native'/'workspace').mkdir()
                (output/'native'/'workspace'/'keep').write_text('retained')
            return result
        with patch('trial.shutil.rmtree',side_effect=PermissionError('retained failure')) as removal:
            removal.avoids_symlink_attacks=True
            result=trial.run(self.path,run_id,self.root/'runs'/run_id,self.credential,
                             phase_runner=with_scratch,validator=lambda path:{'ready_for_external_dispatch':True})
            self.assertEqual(removal.call_count,1)
        self.assertEqual(self.calls,['native'])
        self.assertEqual(result['status'],'failed')
        self.assertEqual(result['failed_stage'],'native_scratch_release')
        self.assertTrue(result['execution_closed']);self.assertTrue(result['accounting_complete'])
        self.assertIsNone(result['accepted'])
        self.assertEqual(result['native_scratch_release']['targets'],
                         [{'path':'native/workspace','removed':False,'error_type':'PermissionError'}])
        self.assertTrue((self.root/'runs'/run_id/'native'/'workspace'/'keep').exists())
        self.assertEqual(result['cleanup']['targets'],[])
        self.assertIn('native_retention',result['artifacts'])
        self.assertIn('native_scratch_release',result['artifacts'])
        next_id=self.registration['schedule'][1]['run_id']
        later=self.run_trial(next_id)
        self.assertEqual(later['status'],'failed')
        self.assertEqual(self.calls,['native'])
        self.assertEqual(later['cost_upper_usd'],0)

    def test_unknown_native_closure_preserves_scratch_and_does_not_launch_acceptance(self):
        run_id=self.choose_first('A')
        def uncertain(name,argv,output,timeout):
            result=self.fake_phase(name,argv,output,timeout)
            if name=='native':
                destination=Path(argv[-1])
                row=json.loads((destination/'result.json').read_text())
                row['execution_closed']=False
                (destination/'result.json').write_text(json.dumps(row))
                (destination/'workspace').mkdir()
                (destination/'home'/'target').mkdir(parents=True)
            return result
        result=trial.run(self.path,run_id,self.root/'runs'/run_id,self.credential,
                         phase_runner=uncertain,validator=lambda path:{'ready_for_external_dispatch':True})
        self.assertEqual(self.calls,['native'])
        self.assertFalse(result['execution_closed']);self.assertIsNone(result['cost_upper_usd'])
        self.assertTrue((self.root/'runs'/run_id/'native'/'workspace').exists())
        self.assertTrue((self.root/'runs'/run_id/'native'/'home'/'target').exists())
        self.assertNotIn('native_scratch_release',result)

    def test_missing_provider_evidence_preserves_native_scratch(self):
        run_id=self.choose_first('A')
        def missing(name,argv,output,timeout):
            result=self.fake_phase(name,argv,output,timeout)
            if name=='native':
                destination=Path(argv[-1])
                (destination/'provider-calls.jsonl').unlink()
                (destination/'workspace').mkdir()
                (destination/'home'/'target').mkdir(parents=True)
            return result
        result=trial.run(self.path,run_id,self.root/'runs'/run_id,self.credential,
                         phase_runner=missing,validator=lambda path:{'ready_for_external_dispatch':True})
        self.assertEqual(self.calls,['native'])
        self.assertEqual(result['status'],'failed')
        self.assertIsNone(result['cost_upper_usd'])
        self.assertTrue((self.root/'runs'/run_id/'native'/'workspace').exists())
        self.assertTrue((self.root/'runs'/run_id/'native'/'home'/'target').exists())

    def test_cleanup_refuses_a_symlink_to_a_shared_target(self):
        output=self.root/'owned';(output/'native'/'home').mkdir(parents=True)
        shared=self.root/'shared';shared.mkdir();(shared/'keep').write_text('keep')
        (output/'native'/'home'/'target').symlink_to(shared,target_is_directory=True)
        result=trial.cleanup_targets(output,True,False)
        self.assertFalse(result[0]['removed'])
        self.assertEqual((shared/'keep').read_text(),'keep')

    def test_cleanup_tolerates_only_disappearing_files_during_removal(self):
        unlink = os.unlink
        for error_type in (FileNotFoundError, PermissionError):
            with self.subTest(error=error_type.__name__):
                output=self.root/error_type.__name__
                workspace=output/'acceptance'/'workspace'
                workspace.mkdir(parents=True)
                (workspace/'gc.pid').write_text('transient')
                def concurrent_unlink(path,*args,**kwargs):
                    if Path(path).name=='gc.pid':
                        if error_type is FileNotFoundError:
                            unlink(path,*args,**kwargs)
                        else:
                            raise PermissionError('Retained removal failure')
                    return unlink(path,*args,**kwargs)
                with patch('os.unlink',side_effect=concurrent_unlink):
                    result=trial.cleanup_targets(output,False,True,True)
                if error_type is FileNotFoundError:
                    self.assertEqual(result,[{'path':'acceptance/workspace','removed':True}])
                    self.assertFalse(workspace.exists())
                else:
                    self.assertEqual(result,[{'path':'acceptance/workspace','removed':False,
                                              'error_type':'PermissionError'}])
                    self.assertTrue((workspace/'gc.pid').is_file())

    def test_cleanup_retains_a_missing_removal_root_error(self):
        output=self.root/'owned'
        workspace=output/'native'/'workspace'
        workspace.mkdir(parents=True)
        remove=trial.shutil.rmtree
        def disappearing_root(path,**kwargs):
            remove(path)
            remove(path,**kwargs)
        with patch('trial.shutil.rmtree',side_effect=disappearing_root) as mocked:
            mocked.avoids_symlink_attacks=True
            result=trial.cleanup_targets(output,True,False,True)
        self.assertEqual(result,[{'path':'native/workspace','removed':False,
                                  'error_type':'FileNotFoundError'}])

    def test_changed_imported_helper_stops_next_phase(self):
        run_id=self.choose_first('B')
        def mutate_helper(name,argv,output,timeout):
            result=self.fake_phase(name,argv,output,timeout)
            if name=='preparation':
                (self.root/'harness'/'seed_manifest.py').write_text('changed')
            return result
        result=trial.run(self.path,run_id,self.root/'runs'/run_id,self.credential,phase_runner=mutate_helper,
                         validator=lambda path:{'ready_for_external_dispatch':True})
        self.assertEqual(self.calls,['preparation'])
        self.assertEqual(result['status'],'incomplete')

    def test_shared_build_profile_is_required_before_any_phase(self):
        run_id=self.choose_first('A')
        self.runtime['native_common']['toolchain']['environment'].pop('CARGO_PROFILE_TEST_DEBUG')
        self.registration['artifacts']['trial_config']=self.fixture.write('runtime/config.json',self.runtime)
        self.path.write_text(json.dumps(self.registration))
        result=self.run_trial(run_id)
        self.assertEqual(self.calls,[]);self.assertEqual(result['status'],'failed')

    def test_workspace_cleanup_requires_reconstruction_and_corresponding_closure(self):
        output=self.root/'owned'
        for name in ('native','acceptance'):
            (output/name/'workspace').mkdir(parents=True)
            (output/name/'workspace'/'file').write_text('source')
        self.assertEqual(trial.cleanup_targets(output,True,True,False),[])
        result=trial.cleanup_targets(output,True,False,True)
        self.assertEqual(result,[{'path':'native/workspace','removed':True}])
        self.assertTrue((output/'acceptance'/'workspace'/'file').exists())

    def test_workspace_and_parent_symlinks_cannot_remove_shared_data(self):
        shared=self.root/'shared';(shared/'workspace').mkdir(parents=True)
        (shared/'workspace'/'keep').write_text('keep')
        output=self.root/'owned';(output/'native').mkdir(parents=True)
        (output/'native'/'workspace').symlink_to(shared/'workspace',target_is_directory=True)
        (output/'acceptance').symlink_to(shared,target_is_directory=True)
        result=trial.cleanup_targets(output,True,True,True)
        self.assertEqual(len(result),2)
        self.assertTrue(all(not row['removed'] for row in result))
        self.assertEqual((shared/'workspace'/'keep').read_text(),'keep')

    def test_premature_acceptance_receipt_cannot_pass_or_trigger_workspace_cleanup(self):
        run_id=self.choose_first('A')
        def premature(name,argv,output,timeout):
            result=self.fake_phase(name,argv,output,timeout)
            if name=='acceptance':
                destination=Path(argv[-1]);checks=json.loads((destination/'checks.json').read_text())
                checks.pop('execution_closed');(destination/'checks.json').write_text(json.dumps(checks))
                (destination/'workspace').mkdir();(destination/'workspace'/'keep').write_text('pending')
            return result
        result=trial.run(self.path,run_id,self.root/'runs'/run_id,self.credential,phase_runner=premature,
                         validator=lambda path:{'ready_for_external_dispatch':True})
        self.assertIsNone(result['accepted']);self.assertFalse(result['execution_closed'])
        self.assertEqual(result['status'],'incomplete')
        self.assertTrue((self.root/'runs'/run_id/'acceptance'/'workspace'/'keep').exists())


if __name__=='__main__':unittest.main()
