"""Check replacement provenance and execution with synthetic studies only."""
import contextlib
import io
import json
import os
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest import mock

import run_replacement_block as replacement

coordinator = replacement.coordinator


class ReplacementTests(unittest.TestCase):
    @contextlib.contextmanager
    def fixture(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            prior = root / 'prior'
            for panel in ['development', 'heldout']:
                for ordinal, condition in enumerate(coordinator.PRIOR_ORDER, 1):
                    coordinator.save(prior / panel / (str(ordinal) + '-' + condition) / 'result.json',
                                     {'condition': condition, 'exit_code': 0, 'cost_complete': True})
            config, inputs = {}, []
            def bound(path):
                return {'path': str(path), 'sha256': coordinator.sha(path.read_bytes())}
            for key in ['instructions_file', 'instruction_manifest', 'instruction_block', 'task_file', 'brief_file']:
                path = root / key
                path.write_text('Synthetic input.\n')
                config[key] = str(path)
                inputs.append(bound(path))
            config_path = root / 'config.json'
            coordinator.save(config_path, config)
            for name in ['runner', 'guard']:
                (root / name).write_text('Synthetic harness; never imported.\n')
            plan = {'schema': coordinator.SCHEMA, 'models': coordinator.MODELS,
                    'claude_cli_version': coordinator.CLAUDE_CLI_VERSION, 'effort': 'medium',
                    'order': coordinator.ORDER, 'runner': bound(root / 'runner'),
                    'instruction_guard': bound(root / 'guard'),
                    'coordinator_sha256': coordinator.sha(coordinator.read(coordinator.__file__)),
                    'task_config': bound(config_path), 'input_files': inputs, 'prior_round_runs': str(prior)}
            class FakeRunner:
                MODEL = coordinator.MODELS['opus']
                def run(self, config, condition, run_dir):
                    run_dir.mkdir(parents=True, exist_ok=False)
                    result = {'model': self.MODEL, 'effort': 'medium',
                              'init': {'model': self.MODEL, 'claude_code_version': coordinator.CLAUDE_CLI_VERSION},
                              'accepted': True, 'completed': True, 'cost_complete': True,
                              'cost_usd_list_estimate': 0.1,
                              'attempts': [{'result': {'modelUsage': {self.MODEL: {}}}}]}
                    coordinator.save(run_dir / 'result.json', result)
                    (run_dir / 'events.jsonl').write_text('Synthetic private event.\n')
                    return result
            fake = FakeRunner()
            study = root / 'original'
            with mock.patch.object(coordinator, 'load_runner', return_value=fake), contextlib.redirect_stdout(io.StringIO()):
                coordinator.execute(plan, study)
            (study / 'harness').mkdir()
            shutil.copyfile(root / 'runner', study / 'harness/replay.py')
            shutil.copyfile(root / 'guard', study / 'harness/instruction_guard.py')
            for family in coordinator.MODELS:
                coordinator.save(study / 'warmups' / family / 'result.json', {'warmup': True})
                (study / 'warmups' / family / 'events.jsonl').write_text('Synthetic warmup.\n')
            executable = root / 'pinned/claude'
            executable.parent.mkdir()
            executable.write_text('#!/bin/sh\nprintf "2.1.287 (Claude Code)\\n"\n')
            executable.chmod(0o500)
            extra = root / 'additional-evidence.json'
            extra.write_text('{"synthetic_amendment": true}\n')
            record = replacement.registration(study / 'plan.json', study, root / 'replacement', executable, [extra])
            yield root, record, fake

    def run_fake(self, record, fake):
        with mock.patch.object(coordinator, 'load_runner', return_value=fake), contextlib.redirect_stdout(io.StringIO()):
            return replacement.execute(record, replacement.canonical(record))

    def test_complete_block_preserves_original_and_pins_and_restores_environment(self):
        with self.fixture() as (root, record, fake):
            original = {path.relative_to(root / 'original'): path.read_bytes()
                        for path in (root / 'original').rglob('*') if path.is_file()}
            observed = []
            original_run = fake.run
            def run(config, condition, run_dir):
                observed.append((run_dir.name, shutil.which('claude'), os.environ.get('DISABLE_AUTOUPDATER')))
                return original_run(config, condition, run_dir)
            fake.run = run
            with mock.patch.dict(os.environ, {'PATH': '/usr/bin:/bin', 'DISABLE_AUTOUPDATER': 'old-value'}):
                receipt = self.run_fake(record, fake)
                self.assertEqual(os.environ['PATH'], '/usr/bin:/bin')
                self.assertEqual(os.environ['DISABLE_AUTOUPDATER'], 'old-value')
            self.assertEqual(receipt['completed_ordinals'], [13, 14, 15, 16])
            self.assertEqual([name for name, _, _ in observed], [row['label'] for row in coordinator.schedule()[12:]])
            self.assertTrue(all(path == record['executable']['path'] and disabled == '1' for _, path, disabled in observed))
            output = root / 'replacement'
            for path, data in original.items():
                self.assertEqual((root / 'original' / path).read_bytes(), data)
            self.assertEqual(len(list((output / 'runs').iterdir())), 16)
            self.assertEqual((output / 'plan.json').read_bytes(), (root / 'original/plan.json').read_bytes())
            for item in record['copied_files']:
                self.assertEqual((output / item['path']).read_bytes(), (root / 'original' / item['path']).read_bytes())
            for row in coordinator.schedule():
                arm = json.loads((output / 'runs' / row['label'] / 'arm-result.json').read_bytes())
                self.assertEqual(arm['plan_sha256'], record['original_plan']['canonical_sha256'])
            self.assertEqual((output / 'replacement-registration.json').read_bytes(), replacement.canonical(record))
            self.assertEqual(len((output / 'replacement-events.jsonl').read_text().splitlines()), 9)
            self.assertGreater(receipt['initial_setup_elapsed_s'], 0)
            self.assertGreater(receipt['wrapper_overhead_elapsed_s'], 0)
            self.assertGreater(receipt['final_validation_elapsed_s'], 0)
            self.assertEqual([item['ordinal'] for item in receipt['arm_timings']], [13, 14, 15, 16])
            for timing in receipt['arm_timings']:
                self.assertGreater(timing['before_binary_verification_elapsed_s'], 0)
                self.assertGreater(timing['after_binary_verification_elapsed_s'], 0)
                self.assertGreaterEqual(timing['preflight_elapsed_s'], timing['before_binary_verification_elapsed_s'])
            self.assertAlmostEqual(receipt['elapsed_s'], receipt['wrapper_overhead_elapsed_s'] +
                                   sum(item['coordinator_elapsed_s'] for item in receipt['arm_timings']))
            self.assertEqual(json.loads((output / 'replacement-complete.json').read_bytes()), receipt)

    def test_original_inputs_manifest_plan_and_harness_tampering_are_refused(self):
        for relative in ['additional-evidence.json', 'task_file', 'original/warmups/opus/events.jsonl',
                         'original/runs/1-A-opus-control/events.jsonl',
                         'original/runs/1-A-opus-control/result.json',
                         'original/harness/replay.py', 'original/plan.json']:
            with self.subTest(relative=relative), self.fixture() as (root, record, fake):
                path = root / relative
                path.write_bytes(path.read_bytes() + b' changed')
                with self.assertRaises((ValueError, json.JSONDecodeError)):
                    self.run_fake(record, fake)
                self.assertFalse((root / 'replacement').exists())

    def test_registration_tampering_wrong_order_and_stale_output_are_refused(self):
        changes = [('arms', ['A', 'D', 'C', 'B']), ('ordinals', [13, 14, 16, 15]),
                   ('block', 3), ('helper_sha256', '0' * 64), ('environment', {}), ('copied_files', [])]
        for key, value in changes:
            with self.subTest(key=key), self.fixture() as (root, record, fake):
                record[key] = value
                with self.assertRaises(ValueError):
                    self.run_fake(record, fake)
                self.assertFalse((root / 'replacement').exists())
        with self.fixture() as (root, record, fake):
            (root / 'replacement').mkdir()
            (root / 'replacement/retained').write_bytes(b'preserve')
            with self.assertRaisesRegex(ValueError, 'already exist'):
                self.run_fake(record, fake)
            self.assertEqual((root / 'replacement/retained').read_bytes(), b'preserve')

    def test_tampered_wrong_version_writable_linked_and_symlinked_binary_refuse(self):
        for mutation in ['bytes', 'version', 'writable', 'hardlink', 'symlink']:
            with self.subTest(mutation=mutation), self.fixture() as (root, record, fake):
                executable = Path(record['executable']['path'])
                if mutation in ['bytes', 'version']:
                    executable.chmod(0o700)
                    executable.write_text('#!/bin/sh\nprintf "2.1.288 (Claude Code)\\n"\n')
                    executable.chmod(0o500)
                    if mutation == 'version':
                        record['executable'] = replacement.executable_record(executable)
                elif mutation == 'writable':
                    executable.chmod(0o700)
                elif mutation == 'hardlink':
                    os.link(executable, root / 'linked')
                else:
                    executable.rename(root / 'moved')
                    executable.symlink_to(root / 'moved')
                with self.assertRaises(ValueError):
                    self.run_fake(record, fake)
                self.assertFalse((root / 'replacement/replacement-complete.json').exists())
                self.assertFalse((root / 'replacement/runs/13-D-sonnet-treatment').exists())

    def test_binary_changed_during_arm_is_retained_and_cannot_complete(self):
        with self.fixture() as (root, record, fake):
            original_run = fake.run
            def run(config, condition, run_dir):
                result = original_run(config, condition, run_dir)
                executable = Path(record['executable']['path'])
                executable.chmod(0o700)
                executable.write_bytes(executable.read_bytes() + b'# changed\n')
                executable.chmod(0o500)
                return result
            fake.run = run
            with mock.patch.dict(os.environ, {}, clear=True):
                with self.assertRaisesRegex(ValueError, 'executable differs'):
                    self.run_fake(record, fake)
                self.assertNotIn('PATH', os.environ)
                self.assertNotIn('DISABLE_AUTOUPDATER', os.environ)
            output = root / 'replacement'
            self.assertTrue((output / 'runs/13-D-sonnet-treatment/arm-result.json').exists())
            self.assertFalse((output / 'runs/14-A-opus-control').exists())
            self.assertFalse((output / 'replacement-complete.json').exists())
            events = [json.loads(line) for line in (output / 'replacement-events.jsonl').read_text().splitlines()]
            self.assertFalse(events[-1]['binary_binding_ok'])

    def test_copy_tampering_and_unexpected_existing_run_stop_before_next_arm(self):
        for mutation in ['copy', 'run']:
            with self.subTest(mutation=mutation), self.fixture() as (root, record, fake):
                original_run = fake.run
                def run(config, condition, run_dir):
                    result = original_run(config, condition, run_dir)
                    if mutation == 'copy':
                        (root / 'replacement/warmups/opus/events.jsonl').write_bytes(b'changed')
                    else:
                        (root / 'replacement/runs/14-A-opus-control').mkdir()
                    return result
                fake.run = run
                with self.assertRaises(ValueError):
                    self.run_fake(record, fake)
                self.assertFalse((root / 'replacement/replacement-complete.json').exists())
                self.assertFalse((root / 'replacement/runs/14-A-opus-control/arm-result.json').exists())

    def test_additional_input_changes_between_arms_stop_before_next_run(self):
        with self.fixture() as (root, record, fake):
            original_run = fake.run
            def run(config, condition, run_dir):
                result = original_run(config, condition, run_dir)
                (root / 'additional-evidence.json').write_text('Changed after the first arm.\n')
                return result
            fake.run = run
            with self.assertRaisesRegex(ValueError, 'additional registered input'):
                self.run_fake(record, fake)
            self.assertTrue((root / 'replacement/runs/13-D-sonnet-treatment/arm-result.json').exists())
            self.assertFalse((root / 'replacement/runs/14-A-opus-control').exists())
            self.assertFalse((root / 'replacement/replacement-complete.json').exists())

    def test_registration_does_not_execute_the_binary_and_wrong_schedule_is_refused(self):
        with self.fixture() as (root, record, fake):
            with mock.patch.object(replacement.subprocess, 'run') as launched:
                replacement.validate(record)
            launched.assert_not_called()
            schedule = root / 'original/schedule.json'
            data = json.loads(schedule.read_bytes())
            data['runs'][12], data['runs'][13] = data['runs'][13], data['runs'][12]
            schedule.write_bytes(replacement.canonical(data))
            with self.assertRaisesRegex(ValueError, 'schedule'):
                self.run_fake(record, fake)
            self.assertFalse((root / 'replacement').exists())

    def test_coordinator_failure_still_checks_binary_and_restores_environment(self):
        with self.fixture() as (root, record, fake):
            def run(*args):
                raise RuntimeError('Synthetic coordinator failure')
            fake.run = run
            with mock.patch.dict(os.environ, {'PATH': '/usr/bin:/bin', 'DISABLE_AUTOUPDATER': 'original'}):
                with self.assertRaisesRegex(RuntimeError, 'Synthetic coordinator'):
                    self.run_fake(record, fake)
                self.assertEqual(os.environ['PATH'], '/usr/bin:/bin')
                self.assertEqual(os.environ['DISABLE_AUTOUPDATER'], 'original')
            events = [json.loads(line) for line in (root / 'replacement/replacement-events.jsonl').read_text().splitlines()]
            self.assertEqual([event['phase'] for event in events], ['setup', 'before', 'after'])
            self.assertTrue(events[-1]['binary_binding_ok'])
            self.assertFalse((root / 'replacement/replacement-complete.json').exists())


if __name__ == '__main__':
    unittest.main()
