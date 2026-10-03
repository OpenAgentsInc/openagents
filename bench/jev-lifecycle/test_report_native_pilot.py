"""Synthetic complete and partial panels exercise the registered analysis gates."""
import json
from pathlib import Path
import tarfile
import tempfile
import unittest
import uuid

import report_native_pilot as report

MODEL = 'claude-sonnet-5-5'
RATES = {'input': 2, 'output': 10, 'cache_write_5m': 2.5, 'cache_write_1h': 4, 'cache_read': .2}
MODULES = {'frozen.py':'1'*64,'jev-lifecycle/run_native_panel.py':'6'*64}


def save(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value) + '\n')


class Panel:
    def __init__(self, root):
        self.root = Path(root); self.configs = self.root/'configs'; self.runs = self.root/'runs'
        self.schedule = []
        for position, (task, repetition, arm) in enumerate(report.EXPECTED, 1):
            run_id = str(uuid.uuid4())
            config = {'run_id': run_id, 'task_id': task, 'arm': arm, 'module_hashes': MODULES,
                      'native_template': {'source_archive_sha256':'2'*64, 'binary_sha256':'3'*64,
                                          'binary_version':'2.1.288', 'model':MODEL, 'effort':'medium',
                                          'target_seed_manifest_sha256':'4'*64, 'cargo_features':[]},
                      'acceptance_template': {'checker':{'sha256':'5'*64}, 'cargo_features':[]}}
            path = self.configs/(run_id+'.json');save(path,config)
            self.schedule.append({'position':position,'run_id':run_id,'task_id':task,'repetition':repetition,'arm':arm,
                                  'config':{'path':str(path),'sha256':report.sha(path)},'output':'/PRIVATE/unused'})
        self.plan={'schema':'openagents.jev-lifecycle.native-pilot-plan.v1','status':'frozen_before_execution',
                   'schedule':self.schedule,'module_hashes':MODULES,'prices':{MODEL:RATES},'capability_probe':{'sha256':'7'*64}}
        self.path=self.root/'plan.json';save(self.path,self.plan)

    def run(self, position, *, native_cost=1, wall=100, accepted=True, completed=True):
        entry=self.schedule[position-1];run_id=entry['run_id'];d=self.runs/run_id
        native_dir=d/'native';native_dir.mkdir(parents=True)
        with tarfile.open(native_dir/'candidate.tar.gz','w:gz'):pass
        save(native_dir/'changes.json',{})
        candidate_id,_=report.candidate.write_manifest(native_dir,'a'*40,'2'*64,{})
        native={'schema':'openagents.delegation.native-attempt.v1','run_id':run_id,'source_commit':'a'*40,'source_archive_sha256':'2'*64,'cli_sha256':'3'*64,
                'cli_hash_after':'3'*64,'cli_version':'2.1.288','model':MODEL,'effort':'medium',
                'target_seed_manifest_sha256':'4'*64,'candidate_manifest_sha256':candidate_id,
                'served_models':[MODEL],'execution_closed':True,'model_completed':completed,
                'cost_usd':native_cost,'total_retained_wall_s':wall*.7}
        prompt=b'Synthetic public task and full common instructions.'
        (d/'preparation').mkdir()
        (d/'preparation/prompt.txt').write_bytes(prompt)
        native['prompt_sha256']=report.hashlib.sha256(prompt).hexdigest()
        native['delivered_prompt_sha256']=report.hashlib.sha256(('Benchmark run ID: '+run_id+'\n\n').encode()+prompt).hexdigest()
        args={'model':MODEL,'effort':'medium','tools':None}
        if entry['arm']!='bare':args.update(system_file='bound-lean-system',tools='Bash,Read,Edit,Write,Glob,Grep')
        native['argv']=['/synthetic/claude']+report.trial.native_arguments(args)
        save(native_dir/'result.json',native)
        usage={'input_tokens':int(native_cost*500000),'output_tokens':0}
        events=[{'run_id':run_id,'call_id':'one','phase':'admitted','path':'/v1/messages','model':MODEL},
                {'run_id':run_id,'call_id':'one','phase':'finished','status':'complete','http_status':200,
                 'usage_status':'reported','served_model':MODEL,'usage':usage,'cost_usd':native_cost}]
        (native_dir/'provider-calls.jsonl').write_text(''.join(json.dumps(e)+'\n' for e in events))
        checks={'schema':'openagents.delegation.final-checks.v1','run_id':run_id,'source_commit':'a'*40,
                'source_archive_sha256':'2'*64,'candidate_manifest_sha256':candidate_id,'checker_sha256':'5'*64,
                'completed':True,'execution_closed':True,
                **{name:{'passed':accepted if name=='independent' else True,'wall_s':1} for name in ('scope','format','ordinary','independent')}}
        save(d/'acceptance/checks.json',checks)
        jev=.01 if entry['arm']=='jev' else 0
        if jev:save(d/'preparation/gateway-call/receipt.json',{'cost_usd':jev,'cost_status':'gateway_reported','wall_s':.2})
        row={'schema':'openagents.jev-lifecycle.native-pilot.v1',**{k:entry[k] for k in ('run_id','task_id','arm')},
             'config_sha256':entry['config']['sha256'],'source_commit':'a'*40,'status':'complete','accepted':accepted,
             'execution_closed':True,'accounting_complete':True,'model_completed':completed,
             'candidate_manifest_sha256':candidate_id,'cost_lower_usd':native_cost+jev,'cost_upper_usd':native_cost+jev,
             'checks_endpoint_wall_s':wall-2,'endpoint_wall_s':wall,'cleanup':[{'removed':True}],
             'native_scratch_release':{'status':'complete','wall_s':1},'safe_to_continue':True,
             'phases':{'preparation':{'wall_s':1},'native':{'wall_s':wall*.7},'acceptance':{'wall_s':10},'final_cleanup':{'wall_s':2}},
             'artifacts':{name:{'sha256':report.sha(d/path)} for name,path in [('native','native/result.json'),('checks','acceptance/checks.json')]}}
        save(d/'pilot.json',row);return d,row

    def panel_receipt(self):
        attempts=[]
        for entry in self.schedule:
            path=self.runs/entry['run_id']/'pilot.json'
            if not path.exists():break
            attempts.append(dict(entry,receipt_sha256=report.sha(path)))
        path=self.root/'panel.json'
        save(path,{'schema':'openagents.jev-lifecycle.native-panel.v1','status':'complete' if len(attempts)==12 else 'running',
                   'attempts':attempts,'plan_sha256':report.sha(self.path),'driver_sha256':MODULES['jev-lifecycle/run_native_panel.py'],'probe_sha256':'7'*64})
        return path

    def build(self):return report.build(self.path,self.runs,self.configs,self.panel_receipt())


class NativeReportTests(unittest.TestCase):
    def test_complete_panel_win_includes_failed_comparator_cost_and_budget_ended_acceptance(self):
        with tempfile.TemporaryDirectory() as directory:
            panel=Panel(directory)
            for entry in panel.schedule:
                arm=entry['arm'];cost={'bare':2,'deterministic':1.5,'jev':1}[arm]
                panel.run(entry['position'],native_cost=cost,wall={'bare':200,'deterministic':150,'jev':100}[arm],
                          accepted=not(entry['position']==1),completed=not(entry['position']==3))
            result=panel.build()
            self.assertEqual(result['status'],'complete')
            self.assertTrue(result['directional_pilot_win'])
            self.assertEqual(result['arms']['bare']['accepted'],3)
            self.assertEqual(result['arms']['bare']['cost_per_accepted_usd'],8/3)
            self.assertEqual(result['arms']['jev']['accepted'],4)
            self.assertEqual(result['arms']['jev']['model_completed'],3)
            self.assertAlmostEqual(result['arms']['jev']['cost_usd'],4.04)
            self.assertEqual(result['comparisons']['jev/bare']['lower_cost_and_time_pairs'],4)
            self.assertNotIn('/PRIVATE',json.dumps(result))
            self.assertNotIn(str(Path(directory)),json.dumps(result))

    def test_partial_panel_keeps_four_assigned_and_does_not_complete_missing_pairs(self):
        with tempfile.TemporaryDirectory() as directory:
            panel=Panel(directory);panel.run(1)
            result=panel.build()
            self.assertIsNone(result['directional_pilot_win'])
            self.assertEqual(result['arms']['bare']['assigned'],4)
            self.assertEqual(result['arms']['bare']['launched'],1)
            self.assertIsNone(result['arms']['bare']['mean_cost_usd'])
            self.assertEqual(result['arms']['bare']['observed_mean_cost_usd'],1)
            self.assertEqual(len(result['comparisons']['jev/bare']['matched_pairs']),4)
            self.assertEqual(result['comparisons']['jev/bare']['evaluable_pairs'],0)
            self.assertIn('1/4',report.markdown(result))

    def test_malformed_pilot_retains_valid_provider_and_gateway_charges(self):
        with tempfile.TemporaryDirectory() as directory:
            panel=Panel(directory);d,row=panel.run(3,native_cost=1.25)
            (d/'pilot.json').write_text('{invalid')
            result=panel.build();observed=result['rows'][2]
            self.assertFalse(observed['accepted'])
            self.assertAlmostEqual(observed['cost_lower_usd'],1.26)
            self.assertIsNone(observed['cost_usd'])
            self.assertIn('pilot_unreadable',observed['artifact_errors'])

    def test_unknown_provider_call_preserves_prior_cost_and_never_becomes_zero(self):
        with tempfile.TemporaryDirectory() as directory:
            panel=Panel(directory);d,row=panel.run(1,native_cost=1.25)
            ledger=d/'native/provider-calls.jsonl'
            with ledger.open('a') as target:target.write(json.dumps({'run_id':row['run_id'],'call_id':'pending','phase':'admitted','path':'/v1/messages','model':MODEL})+'\n')
            observed=panel.build()['rows'][0]
            self.assertEqual(observed['native_cost_lower_usd'],1.25)
            self.assertIsNone(observed['cost_upper_usd'])
            self.assertEqual(observed['unknown_provider_calls'],1)
            self.assertFalse(observed['accepted'])

    def test_ratio_is_sum_of_task_means_not_average_of_ratios(self):
        with tempfile.TemporaryDirectory() as directory:
            panel=Panel(directory)
            for entry in panel.schedule:
                base=1 if entry['task_id']=='alternative-beta' else 9
                cost=1 if entry['arm']=='jev' else base
                panel.run(entry['position'],native_cost=cost,wall=10 if entry['arm']=='jev' else base*10)
            comp=panel.build()['comparisons']['jev/bare']
            self.assertAlmostEqual(comp['aggregate_ratio_of_summed_task_means']['cost'],2.02/10)
            self.assertAlmostEqual(comp['aggregate_ratio_of_summed_task_means']['time'],.2)
            self.assertEqual(comp['evaluable_pairs'],4)

    def test_changed_candidate_or_missing_cleanup_cannot_pass(self):
        with tempfile.TemporaryDirectory() as directory:
            panel=Panel(directory);d,row=panel.run(1)
            (d/'native/changes.json').write_text('{"different":{"before":null,"after":null}}')
            observed=panel.build()['rows'][0]
            self.assertFalse(observed['accepted']);self.assertIn('candidate_invalid',observed['artifact_errors'])
            d2,row2=panel.run(2);row2['cleanup']=[{'removed':False}];save(d2/'pilot.json',row2)
            observed=panel.build()['rows'][1]
            self.assertTrue(observed['accepted']) # Patch quality is distinct from scratch cleanup.
            self.assertIsNone(observed['endpoint_wall_s'])
            self.assertFalse(observed['cleanup_confirmed'])

    def test_changed_prompt_or_tool_mode_invalidates_acceptance_without_losing_cost(self):
        with tempfile.TemporaryDirectory() as directory:
            panel=Panel(directory);d,row=panel.run(2)
            (d/'preparation/prompt.txt').write_text('Different prompt')
            first=panel.build()['rows'][1]
            self.assertFalse(first['accepted']);self.assertEqual(first['cost_usd'],1)
            d,row=panel.run(3);native=report.read(d/'native/result.json')
            native['argv']=native['argv'][:-2]
            save(d/'native/result.json',native);row['artifacts']['native']['sha256']=report.sha(d/'native/result.json');save(d/'pilot.json',row)
            second=panel.build()['rows'][2]
            self.assertFalse(second['accepted']);self.assertIn('native_identity_invalid',second['artifact_errors'])

    def test_wrong_order_and_duplicate_identity_are_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            panel=Panel(directory);bad=json.loads(json.dumps(panel.plan))
            bad['schedule'][0],bad['schedule'][1]=bad['schedule'][1],bad['schedule'][0]
            with self.assertRaisesRegex(ValueError,'order'):report.check_plan(bad)
            bad=json.loads(json.dumps(panel.plan));bad['schedule'][1]['run_id']=bad['schedule'][0]['run_id']
            with self.assertRaisesRegex(ValueError,'duplicate'):report.check_plan(bad)

    def test_cli_cost_is_not_added_twice_and_reported_known_cost_survives_missing_sidecar(self):
        with tempfile.TemporaryDirectory() as directory:
            panel=Panel(directory);d,row=panel.run(1,native_cost=2)
            self.assertEqual(panel.build()['rows'][0]['cost_usd'],2)
            (d/'native/provider-calls.jsonl').unlink()
            observed=panel.build()['rows'][0]
            self.assertEqual(observed['cost_lower_usd'],2)
            self.assertIsNone(observed['cost_usd'])

    def test_malformed_native_fields_and_extra_bad_provider_path_keep_known_cost(self):
        with tempfile.TemporaryDirectory() as directory:
            panel=Panel(directory);d,row=panel.run(1,native_cost=1.25)
            native=report.read(d/'native/result.json');native.update(argv=None,served_models=123)
            save(d/'native/result.json',native);row['artifacts']['native']['sha256']=report.sha(d/'native/result.json');save(d/'pilot.json',row)
            ledger=d/'native/provider-calls.jsonl'
            with ledger.open('a') as target:
                target.write(json.dumps({'run_id':row['run_id'],'call_id':'bad','phase':'admitted','path':None,'model':MODEL})+'\n')
                target.write(json.dumps({'run_id':row['run_id'],'call_id':'bad','phase':'finished','status':'complete','http_status':200,'usage_status':'reported','served_model':MODEL,'usage':{},'cost_usd':0})+'\n')
            observed=panel.build()['rows'][0]
            self.assertFalse(observed['accepted'])
            self.assertEqual(observed['cost_lower_usd'],1.25)
            self.assertIsNone(observed['cost_upper_usd'])
            self.assertIn('provider_path_invalid',observed['artifact_errors'])
            self.assertIn('native_identity_invalid',observed['artifact_errors'])

    def test_interrupted_unregistered_attempt_keeps_charges_without_replacing_cells(self):
        with tempfile.TemporaryDirectory() as directory:
            panel=Panel(directory);d,row=panel.run(3,native_cost=1.25)
            (d/'pilot.json').unlink()
            original_id=panel.schedule[2]['run_id'];replacement_id=str(uuid.uuid4())
            old=panel.configs/(original_id+'.json');new=panel.configs/(replacement_id+'.json')
            config=report.read(old);config['run_id']=replacement_id;save(new,config)
            panel.plan['schedule'][2].update(run_id=replacement_id,config={'path':str(new),'sha256':report.sha(new)})
            save(panel.path,panel.plan)
            result=panel.build()
            self.assertEqual(len(result['unregistered_attempts']),1)
            self.assertEqual(result['arms']['jev']['launched'],0)
            self.assertAlmostEqual(result['panel_known_cost_lower_usd'],1.26)
            self.assertIsNone(result['directional_pilot_win'])

    def test_optional_timing_damage_does_not_change_verified_patch_quality(self):
        with tempfile.TemporaryDirectory() as directory:
            panel=Panel(directory);d,row=panel.run(2)
            (d/'preparation/catalog.json').write_text('null')
            observed=panel.build()['rows'][1]
            self.assertTrue(observed['accepted'])
            self.assertEqual(observed['telemetry_errors'],['catalog_unreadable'])
            self.assertIsNone(observed['phase_wall_s']['catalog'])

    def test_driver_order_and_pilot_digests_are_required_for_a_complete_panel(self):
        with tempfile.TemporaryDirectory() as directory:
            panel=Panel(directory)
            for entry in panel.schedule:panel.run(entry['position'])
            absent=report.build(panel.path,panel.runs,panel.configs)
            self.assertIsNone(absent['directional_pilot_win'])
            self.assertEqual(absent['arms']['bare']['accepted'],4)
            path=panel.panel_receipt();value=report.read(path)
            value['attempts'][1],value['attempts'][0]=value['attempts'][0],value['attempts'][1]
            save(path,value)
            broken=report.build(panel.path,panel.runs,panel.configs,path)
            self.assertIn('panel_attempt_order_invalid',broken['panel_receipt']['artifact_errors'])
            self.assertIn('panel_pilot_digest_mismatch',broken['panel_receipt']['artifact_errors'])
            self.assertIsNone(broken['directional_pilot_win'])

    def test_confirmed_no_gateway_call_is_zero_but_missing_receipt_is_unknown(self):
        with tempfile.TemporaryDirectory() as directory:
            panel=Panel(directory);entry=panel.schedule[2];d=panel.runs/entry['run_id']
            row={'schema':'openagents.jev-lifecycle.native-pilot.v1',**{k:entry[k] for k in ('run_id','task_id','arm')},
                 'config_sha256':entry['config']['sha256'],'status':'failed','failed_stage':'preparation',
                 'execution_closed':True,'accounting_complete':True,'cost_lower_usd':0,'cost_upper_usd':0,
                 'cleanup':[],'checks_endpoint_wall_s':1,'endpoint_wall_s':1.1}
            save(d/'pilot.json',row)
            save(d/'preparation/gateway-call/receipt.json',{'attempts':0,'cost_status':'no_call','cost_usd':0})
            observed=panel.build()['rows'][2]
            self.assertEqual(observed['cost_usd'],0)
            self.assertFalse(observed['accepted'])
            (d/'preparation/gateway-call/receipt.json').unlink()
            self.assertIsNone(panel.build()['rows'][2]['cost_usd'])


if __name__=='__main__':unittest.main()
