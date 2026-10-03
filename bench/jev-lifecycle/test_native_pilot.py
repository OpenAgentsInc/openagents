import gzip
import json
import os
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import uuid

sys.path.insert(0, str(Path(__file__).resolve().parent))
import native_pilot as pilot


class Pilot(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name).resolve()
        self.task = {'id': 'alternative-beta', 'source_commit': 'a' * 40,
                     'prompt': 'Implement the public requirement.', 'packages': ['atif'],
                     'allowed_paths': ['crates/atif/'], 'applicable_instruction_paths': []}
        self.protocol = self.root / 'protocol.md'; self.protocol.write_text('Exploratory common protocol.\n')
        self.manifest = self.write('task.json', {'tasks': [self.task]})
        self.index = self.write('index.json', {'commit': 'a' * 40})
        self.checker = self.root / 'checker.rs'; self.checker.write_text('// private checker\n')
        self.system = self.root / 'lean.txt'; self.system.write_text('Lean system.\n')
        self.key = self.root / 'gateway-key'; self.key.write_text('synthetic-key'); self.key.chmod(0o600)
        self.config = {'schema': pilot.SCHEMA, 'protocol': self.ref(self.protocol), 'run_id': str(uuid.uuid4()), 'arm': 'bare',
                       'task_manifest': self.ref(self.manifest), 'task_id': self.task['id'],
                       'index': self.ref(self.index), 'source_repo': str(self.root),
                       'lean_system': self.ref(self.system), 'credential_file': str(self.root / 'native-token'),
                       'gateway_key_file': str(self.key), 'module_hashes': pilot.module_hashes(),
                       'slot_lock': str(self.root / 'slot.lock'),
                       'native_template': {'source_archive': str(self.root / 'source.tar'),
                           'source_archive_sha256': 'b' * 64, 'target_seed': str(self.root / 'seed'),
                           'target_seed_manifest_sha256': 'c' * 64, 'binary': '/opt/claude',
                           'binary_sha256': 'd' * 64, 'binary_version': '2.1.288 (Claude Code)',
                           'model': 'claude-sonnet-5-5', 'effort': 'medium', 'timeout_s': 600,
                           'cli_budget_usd': 2, 'initialize_git': True, 'toolchain': {},
                           'provider_meter': {'admission_target_usd': 8, 'models': {'claude-sonnet-5-5':
                               {'usd_per_million': {'input': 2, 'output': 10, 'cache_write_5m': 2.5,
                                                    'cache_write_1h': 4, 'cache_read': .2}}}}},
                       'acceptance_template': {'packages': ['atif'], 'allowed_paths': ['crates/atif/'],
                           'toolchain': {}, 'total_timeout_s': 240,
                           'checker': {'path': str(self.checker), 'sha256': pilot.sha(self.checker.read_bytes()),
                                       'package': 'atif', 'target': 'private_test', 'injection': 'crates/atif/tests/private_test.rs'}}}
        self.phases = []
        self.native_closed = True
        self.model_completed = True
        self.native_result_malformed = False
        self.checks_pass = True

    def tearDown(self):
        self.tmp.cleanup()

    def write(self, name, data):
        p = self.root / name; p.write_text(json.dumps(data)); return p

    def ref(self, path):
        return {'path': str(path), 'sha256': pilot.sha(path.read_bytes())}

    def prep(self, config, task, output):
        output.mkdir()
        prompt = b'Same public task and full source instructions.'
        if config['arm'] != 'bare':
            prompt += b'\n\nPinned source pack.'
        (output / 'prompt.txt').write_bytes(prompt)
        if config['arm'] == 'jev':
            call = output / 'gateway-call'; call.mkdir()
            (call / 'receipt.json').write_text(json.dumps({'cost_usd': .003, 'status': 'answered'}))
        return {'accounting_complete': True, 'status': 'complete', 'prompt_sha256': pilot.sha(prompt)}

    def phase(self, name, argv, output, timeout):
        self.phases.append(name)
        self.assertNotIn('AI_GATEWAY_API_KEY', os.environ)
        config = json.loads(Path(argv[-2]).read_text()); dest = Path(argv[-1]); dest.mkdir()
        (dest / 'workspace').mkdir(); (dest / 'home/target').mkdir(parents=True)
        if name == 'native':
            self.assertEqual(timeout, 1200)
            with tarfile.open(dest / 'candidate.tar.gz', 'w:gz'):
                pass
            (dest / 'changes.json').write_text('{}')
            identity, payload = pilot.candidate.write_manifest(dest, config['source_commit'], config['source_archive_sha256'], {})
            prompt = Path(config['prompt_file']).read_bytes()
            result = {k: config[k] for k in ('run_id', 'source_commit', 'source_archive_sha256', 'prompt_sha256',
                                            'model', 'effort', 'target_seed_manifest_sha256')}
            result.update(status='complete', execution_closed=self.native_closed, model_completed=self.model_completed,
                          cli_sha256=config['binary_sha256'], cli_hash_after=config['binary_sha256'],
                          cli_version=config['binary_version'], argv=[config['binary']] + pilot.trial.native_arguments(config),
                          delivered_prompt_sha256=pilot.sha(('Benchmark run ID: ' + config['run_id'] + '\n\n').encode() + prompt),
                          snapshot_commit='e' * 40, candidate_manifest_sha256=identity,
                          served_models=[config['model']], cost_usd=.012)
            (dest / 'result.json').write_text('null' if self.native_result_malformed else json.dumps(result))
            lines = [{'run_id': config['run_id'], 'call_id': 'one', 'phase': 'admitted', 'model': config['model'], 'path': '/v1/messages'},
                     {'run_id': config['run_id'], 'call_id': 'one', 'phase': 'finished', 'status': 'complete',
                      'http_status': 200, 'usage_status': 'reported', 'served_model': config['model'],
                      'usage': {'input_tokens': 1000, 'output_tokens': 1000}, 'cost_usd': .012}]
            (dest / 'provider-calls.jsonl').write_text(''.join(json.dumps(v) + '\n' for v in lines))
            (dest / 'events.jsonl').write_text('{"type":"result"}\n')
        else:
            self.assertEqual(timeout, 260)
            self.assertFalse((output / 'native/workspace').exists())
            result = {k: config[k] for k in ('run_id', 'source_commit', 'source_archive_sha256', 'candidate_manifest_sha256')}
            result.update(schema='openagents.delegation.final-checks.v1', checker_sha256=config['checker']['sha256'],
                          execution_closed=True, completed=True)
            for key in ('scope', 'format', 'ordinary', 'independent'):
                result[key] = {'passed': self.checks_pass}
            (dest / 'checks.json').write_text(json.dumps(result))
        return {'exit_code': 0, 'timed_out': False}

    def run_pilot(self):
        config = self.write('config.json', self.config)
        return pilot.run(config, self.root / 'out', prepare_fn=self.prep, phase_fn=self.phase)

    def test_full_flow_cost_once_and_cleanup(self):
        self.config['arm'] = 'jev'
        result = self.run_pilot()
        self.assertTrue(result['accepted']); self.assertTrue(result['safe_to_continue'])
        self.assertAlmostEqual(result['cost_upper_usd'], .015)
        self.assertEqual(self.phases, ['native', 'acceptance'])
        self.assertFalse((self.root / 'out/native/workspace').exists())
        self.assertFalse((self.root / 'out/acceptance/workspace').exists())
        self.assertTrue((self.root / 'out/native/candidate-manifest.json').exists())
        with self.assertRaises(FileExistsError):
            pilot.run(self.root / 'config.json', self.root / 'out', prepare_fn=self.prep, phase_fn=self.phase)
        self.assertEqual(len(self.phases), 2)

    def test_budget_ended_model_and_final_patch_acceptance_are_separate(self):
        self.model_completed = False
        result = self.run_pilot()
        self.assertTrue(result['candidate_checks_passed']); self.assertTrue(result['accepted'])
        self.assertFalse(result['model_completed'])
        self.assertTrue(result['safe_to_continue'])

    def test_unconfirmed_closure_preserves_scratch_and_stops(self):
        self.native_closed = False
        result = self.run_pilot()
        self.assertFalse(result['safe_to_continue']); self.assertFalse(result['accounting_complete'])
        self.assertTrue((self.root / 'out/native/workspace').exists())
        self.assertEqual(self.phases, ['native'])

    def test_cleanup_failure_never_launches_checks_or_retries(self):
        with patch.object(pilot.trial, 'release_native_scratch', return_value={'status': 'failed'}) as release:
            result = self.run_pilot()
        release.assert_called_once()
        self.assertFalse(result['safe_to_continue']); self.assertEqual(self.phases, ['native'])
        self.assertTrue((self.root / 'out/native/workspace').exists())

    def test_malformed_native_result_retains_unknown_accounting(self):
        self.native_result_malformed = True
        result = self.run_pilot()
        self.assertFalse(result['accounting_complete']); self.assertFalse(result['safe_to_continue'])
        self.assertTrue((self.root / 'out/pilot.json').exists())
        self.assertEqual(self.phases, ['native'])

    def test_feature_or_module_mismatch_prevents_paid_launch(self):
        self.config['native_template']['cargo_features'] = ['atif/extra']
        result = self.run_pilot()
        self.assertEqual(self.phases, []); self.assertFalse(result['safe_to_continue'])
        self.assertEqual(result['cost_upper_usd'], 0)
        self.config['native_template'].pop('cargo_features')
        self.config['module_hashes']['jev-lifecycle/context.py'] = '0' * 64
        with self.assertRaises(ValueError):
            pilot.validate(self.config)

    def test_unknown_preparation_cost_stops_before_native(self):
        self.config['arm'] = 'jev'
        def failed(config, task, output):
            output.mkdir(); call = output / 'gateway-call'; call.mkdir()
            (call / 'receipt.json').write_text(json.dumps({'status': 'failed', 'cost_usd': None}))
            raise ValueError('unknown charge')
        result = pilot.run(self.write('config.json', self.config), self.root / 'out', prepare_fn=failed, phase_fn=self.phase)
        self.assertFalse(result['accounting_complete']); self.assertEqual(self.phases, [])

    def test_gateway_key_removed_after_exception(self):
        def failure(*args, **kwargs):
            self.assertEqual(os.environ.get('AI_GATEWAY_API_KEY'), 'synthetic-key')
            raise ValueError('synthetic transport error')
        with patch.object(pilot.spans, 'state', return_value={}), patch.object(pilot.spans, 'questions', return_value={}), patch.object(pilot.gateway, 'call', side_effect=failure):
            with self.assertRaises(ValueError):
                pilot.gateway_call(self.config, {}, self.root / 'call')
        self.assertNotIn('AI_GATEWAY_API_KEY', os.environ)
        self.key.chmod(0o644)
        with self.assertRaises(ValueError):
            pilot.read_key(self.key)

    def test_common_instructions_identical_across_arms(self):
        text = b'Full instructions, with exact source bytes.\n'
        self.task['applicable_instruction_paths'] = [{'path': 'AGENTS.md', 'sha256': pilot.sha(text)}]
        self.task['required_public_readings'] = [{'path': 'contract.md', 'sha256': pilot.sha(text)}]
        catalog = {'clauses': {'r01': 'Do it'}, 'catalog_sha256': 'c' * 64}
        rendered = {'text': 'Source evidence.', 'sha256': pilot.sha(b'Source evidence.')}
        with patch.object(pilot.context, 'tree', return_value={}), patch.object(pilot.context, 'read_blob', return_value=text), patch.object(pilot.spans, 'catalog', return_value=catalog), patch.object(pilot.spans, 'pack', return_value=rendered):
            for arm in ('bare', 'deterministic'):
                self.config['arm'] = arm
                pilot.prepare(self.config, self.task, self.root / arm)
        self.assertEqual((self.root / 'bare/common-prompt.txt').read_bytes(), (self.root / 'deterministic/common-prompt.txt').read_bytes())
        self.assertIn(text, (self.root / 'bare/prompt.txt').read_bytes())
        self.assertIn(b'contract.md', (self.root / 'bare/prompt.txt').read_bytes())
        self.assertIn(b'Allowed edit roots: crates/atif/', (self.root / 'bare/prompt.txt').read_bytes())
        self.assertEqual((self.root / 'deterministic/prompt.txt').read_bytes(), (self.root / 'bare/prompt.txt').read_bytes() + b'\n\nSource evidence.')


    def test_real_gateway_receipt_shape_prepares_jev_pack(self):
        self.config['arm'] = 'jev'
        cat = {'clauses': {'r01': 'Requirement'}, 'catalog_sha256': 'c' * 64}
        rendered = {'text': 'Exact source.', 'sha256': pilot.sha(b'Exact source.')}
        receipt = {'outcome': 'answered', 'answers_valid': True, 'cost_status': 'gateway_reported', 'cost_usd': .003}
        response = {'answers': {'r01': {'choice': 'p001'}}}
        with patch.object(pilot.context, 'tree', return_value={}), patch.object(pilot.spans, 'catalog', return_value=cat), patch.object(pilot.spans, 'pack', return_value=rendered) as packed, patch.object(pilot, 'gateway_call', return_value=(receipt, response)):
            result = pilot.prepare(self.config, self.task, self.root / 'prepared')
        self.assertEqual(result['cost_usd'], .003)
        self.assertEqual(packed.call_args.kwargs['choices'], {'r01': 'p001'})

    def test_late_accounting_error_clears_final_acceptance(self):
        original = pilot.accounting
        calls = 0
        def accounting(*args):
            nonlocal calls
            calls += 1
            if calls > 1:
                raise ValueError('synthetic damaged ledger')
            return original(*args)
        with patch.object(pilot, 'accounting', side_effect=accounting):
            result = self.run_pilot()
        self.assertTrue(result['candidate_checks_passed'])
        self.assertFalse(result['accepted']); self.assertFalse(result['safe_to_continue'])

    def test_preparation_wall_deadline_is_independent_of_socket_timeout(self):
        import time
        with self.assertRaises(TimeoutError):
            with pilot.preparation_deadline(.01):
                time.sleep(.05)



    def test_plan_freezes_twelve_unique_configs_with_common_task_bindings(self):
        import make_native_plan as planner
        root = self.root / 'remote'; (root / 'inputs').mkdir(parents=True)
        (root / 'inputs/protocol.md').write_text('Frozen protocol.')
        (root / 'inputs/lean-system.txt').write_text('Lean system.')
        (root / 'probe').mkdir()
        (root / 'probe/result.json').write_text(json.dumps({'model_completed': True, 'execution_closed': True,
            'provider_accounting_complete': True, 'provider_cost_usd': .1, 'cli_sha256': planner.BINARY_HASH,
            'cli_hash_after': planner.BINARY_HASH, 'cli_version': planner.BINARY_VERSION,
            'model': 'claude-sonnet-5-5', 'served_models': ['claude-sonnet-5-5']}))
        task_ids = {}
        for alias in ('alternative-beta', 'alternative-gamma'):
            inputs = root / 'inputs' / alias; inputs.mkdir()
            task = dict(self.task, id=alias)
            (inputs / 'task.json').write_text(json.dumps({'tasks': [task]}))
            (inputs / 'index.json').write_text(json.dumps({'commit': task['source_commit']}))
            (inputs / 'seed-config.json').write_text(json.dumps({'packages': task['packages'], 'toolchain': {'environment': {}}}))
            checker = inputs / 'checker.rs'; checker.write_text('private checker')
            task_ids[alias] = (task['source_commit'], pilot.sha(checker.read_bytes()))
            seed_root = root / 'seeds' / alias; (seed_root / 'seed').mkdir(parents=True)
            (seed_root / 'source.tar').write_bytes(b'synthetic archive')
            (seed_root / 'seed/seed-manifest.json').write_text(json.dumps({'packages': task['packages']}))
        original_digest = planner.digest
        def digest(path):
            return planner.BINARY_HASH if str(path) == planner.BINARY else original_digest(path)
        with patch.object(planner, 'TASKS', task_ids), patch.object(planner, 'digest', side_effect=digest), patch.object(planner, 'validate_seed'), patch.object(planner.subprocess, 'check_output', return_value=planner.BINARY_VERSION):
            plan = planner.make(root, self.root / 'plan')
        rows = plan['schedule']
        self.assertEqual(len({r['run_id'] for r in rows}), 12)
        self.assertEqual([r['arm'] for r in rows], ['bare', 'deterministic', 'jev', 'jev', 'bare', 'deterministic', 'deterministic', 'jev', 'bare', 'bare', 'deterministic', 'jev'])
        configs = [json.loads(Path(r['config']['path']).read_text()) for r in rows]
        for alias in task_ids:
            same = [c for c in configs if c['task_id'] == alias]
            self.assertEqual(len({json.dumps(c['native_template'], sort_keys=True) for c in same}), 1)
            self.assertEqual(len({json.dumps(c['acceptance_template'], sort_keys=True) for c in same}), 1)
        self.assertIn('jev-lifecycle/make_native_plan.py', plan['module_hashes'])
        self.assertEqual(plan['protocol']['sha256'], pilot.sha(b'Frozen protocol.'))
        self.assertEqual(plan['capability_probe']['sha256'], pilot.sha((root / 'probe/result.json').read_bytes()))


if __name__ == '__main__':
    unittest.main()
