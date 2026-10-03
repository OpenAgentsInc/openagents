#!/usr/bin/env python3
"""Coordinate a registered model-by-brief panel without changing its base runner."""
import argparse
import contextlib
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

SCHEMA = 'openagents.briefing.factorial.v1'
MODELS = {'opus': 'claude-opus-5-5', 'sonnet': 'claude-sonnet-5-5'}
ARMS = {'A': ('opus', 'control'), 'B': ('opus', 'treatment'), 'C': ('sonnet', 'control'), 'D': ('sonnet', 'treatment')}
ORDER = ['ABDC', 'BCAD', 'CDBA', 'DACB']
PRIOR_ORDER = ['control', 'treatment', 'treatment', 'control', 'control', 'treatment', 'treatment', 'control']
MAX_FILE = 4 * 1024 * 1024


def sha(data):
    return hashlib.sha256(data).hexdigest()


def read(path):
    with Path(path).open('rb') as handle:
        data = handle.read(MAX_FILE + 1)
    if len(data) > MAX_FILE:
        raise ValueError('Input exceeds its byte bound')
    return data


def schedule():
    result = []
    for block, order in enumerate(ORDER, 1):
        for arm in order:
            family, condition = ARMS[arm]
            ordinal = len(result) + 1
            result.append({'ordinal': ordinal, 'block': block, 'arm': arm, 'model': MODELS[family], 'effort': 'medium', 'condition': condition, 'label': str(ordinal) + '-' + arm + '-' + family + '-' + condition})
    return result


def validate_plan(plan):
    required = {'schema', 'models', 'effort', 'order', 'runner', 'instruction_guard', 'coordinator_sha256', 'task_config', 'input_files', 'prior_round_runs'}
    if set(plan) != required or plan['schema'] != SCHEMA:
        raise ValueError('Unknown or missing factorial plan fields')
    if plan['models'] != MODELS or plan['effort'] != 'medium' or plan['order'] != ORDER:
        raise ValueError('The registered models, effort, and balanced order must match the prospective protocol')
    if plan['coordinator_sha256'] != sha(read(__file__)):
        raise ValueError('The coordinator differs from its registration')
    for item in [plan['runner'], plan['instruction_guard'], plan['task_config'], *plan['input_files']]:
        if set(item) != {'path', 'sha256'} or sha(read(item['path'])) != item['sha256']:
            raise ValueError('A registered input is missing or changed')
    config = json.loads(read(plan['task_config']['path']))
    bound = {str(Path(item['path']).resolve()): item['sha256'] for item in plan['input_files']}
    for key in ['instructions_file', 'instruction_manifest', 'instruction_block', 'task_file', 'brief_file']:
        path = Path(config[key]).resolve()
        if bound.get(str(path)) != sha(read(path)):
            raise ValueError('The task configuration references an unregistered input')
    return config


def require_prior_finished(root):
    missing = []
    for panel in ['development', 'heldout']:
        for ordinal, condition in enumerate(PRIOR_ORDER, 1):
            path = Path(root) / panel / (str(ordinal) + '-' + condition) / 'result.json'
            if not path.is_file():
                missing.append(panel + '/' + str(ordinal))
                continue
            record = json.loads(read(path))
            if record.get('condition') != condition or 'exit_code' not in record or 'cost_complete' not in record:
                raise ValueError('A prior-round final record is malformed')
    if missing:
        raise ValueError('Round 2 is unfinished; no scored factorial run may start')


def exclusive(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists():
        if path.is_symlink() or read(path) != data:
            raise ValueError('An existing study artifact differs from its registration')
        return
    with path.open('xb') as handle:
        handle.write(data)


def save(path, data):
    exclusive(path, (json.dumps(data, indent=2, allow_nan=False) + '\n').encode())


def load_runner(plan, output):
    harness = output / 'harness'
    for field, name in [('runner', 'replay.py'), ('instruction_guard', 'instruction_guard.py')]:
        exclusive(harness / name, read(plan[field]['path']))
    guard_spec = importlib.util.spec_from_file_location('instruction_guard', harness / 'instruction_guard.py')
    guard = importlib.util.module_from_spec(guard_spec)
    guard_spec.loader.exec_module(guard)
    sys.modules['instruction_guard'] = guard
    runner_spec = importlib.util.spec_from_file_location('factorial_frozen_replay', harness / 'replay.py')
    runner = importlib.util.module_from_spec(runner_spec)
    runner_spec.loader.exec_module(runner)
    if runner.MODEL != MODELS['opus']:
        raise ValueError('The base runner does not have the registered Opus default')
    return runner


def served_models(result):
    models = set()
    for attempt in result.get('attempts', []):
        models.update(attempt.get('result', {}).get('modelUsage', {}))
    return sorted(models)


def execute(plan, output, first=1, last=16):
    if not 1 <= first <= last <= 16:
        raise ValueError('Choose a contiguous interval within the 16 registered runs')
    config = validate_plan(plan)
    require_prior_finished(plan['prior_round_runs'])
    output.mkdir(parents=True, mode=0o700, exist_ok=True)
    frozen_plan = (json.dumps(plan, sort_keys=True, separators=(',', ':')) + '\n').encode()
    plan_digest = sha(frozen_plan)
    exclusive(output / 'plan.json', frozen_plan)
    rows = schedule()
    save(output / 'schedule.json', {'plan_sha256': plan_digest, 'runs': rows})
    for row in rows[:first - 1]:
        previous = output / 'runs' / row['label'] / 'arm-result.json'
        if not previous.exists() or json.loads(read(previous)).get('plan_sha256') != plan_digest:
            raise ValueError('Earlier registered runs must be retained before resuming')
    runner = load_runner(plan, output)
    for row in rows[first - 1:last]:
        config = validate_plan(plan)
        require_prior_finished(plan['prior_round_runs'])
        run_dir = output / 'runs' / row['label']
        if run_dir.exists():
            raise ValueError('A registered run already exists; it will not be repeated')
        print(json.dumps({'starting': row}), flush=True)
        # Each arm uses the same frozen function. Only its full model ID changes.
        runner.MODEL = row['model']
        result = runner.run(config, row['condition'], run_dir)
        models = served_models(result)
        binding_ok = result.get('model') == row['model'] and result.get('effort') == 'medium' and result.get('init', {}).get('model') == row['model'] and models == [row['model']]
        record = {'schema': 'openagents.briefing.factorial-arm.v1', 'plan_sha256': plan_digest, **row, 'served_models_from_cumulative_usage': models, 'model_binding_ok': binding_ok, 'accepted': bool(result.get('accepted') and binding_ok), 'completed': bool(result.get('completed')), 'cost_complete': bool(result.get('cost_complete')), 'cost_usd_list_estimate': result.get('cost_usd_list_estimate'), 'runner_result_sha256': sha(read(run_dir / 'result.json'))}
        save(run_dir / 'arm-result.json', record)
        print(json.dumps({'finished': record}), flush=True)
        if not result.get('completed') or not result.get('cost_complete') or not binding_ok:
            raise RuntimeError('The run is retained; inspect its error, accounting, or model binding before continuing')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--plan', type=Path)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--first', type=int, default=1)
    parser.add_argument('--last', type=int, default=16)
    parser.add_argument('--execute', action='store_true')
    parser.add_argument('--self-test', action='store_true')
    args = parser.parse_args()
    if args.self_test:
        result = unittest.TextTestRunner().run(unittest.defaultTestLoader.loadTestsFromTestCase(CoordinatorTests))
        raise SystemExit(0 if result.wasSuccessful() else 1)
    if not args.plan:
        parser.error('--plan is required')
    plan = json.loads(read(args.plan))
    validate_plan(plan)
    if not args.execute:
        print(json.dumps({'execution': False, 'runs': schedule()}, indent=2))
        return
    if not args.output:
        parser.error('--output is required with --execute')
    execute(plan, args.output, args.first, args.last)


class CoordinatorTests(unittest.TestCase):
    def test_order_balances_positions_and_predecessors(self):
        rows = schedule()
        self.assertEqual(len(rows), 16)
        self.assertEqual(len({row['label'] for row in rows}), 16)
        for position in range(4):
            self.assertEqual({order[position] for order in ORDER}, set('ABCD'))
        predecessors = [(order[i], order[i + 1]) for order in ORDER for i in range(3)]
        self.assertEqual(len(set(predecessors)), 12)
        self.assertTrue(all(a != b for a, b in predecessors))

    def test_unfinished_prior_panel_refuses_execution(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with self.assertRaisesRegex(ValueError, 'unfinished'):
                require_prior_finished(root)
            for panel in ['development', 'heldout']:
                for ordinal, condition in enumerate(PRIOR_ORDER, 1):
                    save(root / panel / (str(ordinal) + '-' + condition) / 'result.json', {'condition': condition, 'exit_code': 0, 'cost_complete': True})
            require_prior_finished(root)

    def test_existing_artifacts_are_never_overwritten(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'artifact'
            exclusive(path, b'original')
            exclusive(path, b'original')
            with self.assertRaises(ValueError):
                exclusive(path, b'changed')
            self.assertEqual(path.read_bytes(), b'original')

    def test_served_model_usage_detects_cross_model_execution(self):
        result = {'attempts': [{'result': {'modelUsage': {MODELS['sonnet']: {}}}}, {'result': {'modelUsage': {MODELS['sonnet']: {}, MODELS['opus']: {}}}}]}
        self.assertEqual(served_models(result), sorted(MODELS.values()))

    def test_registered_execution_and_resume_with_a_fake_runner(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            prior = root / 'prior'
            for panel in ['development', 'heldout']:
                for ordinal, condition in enumerate(PRIOR_ORDER, 1):
                    save(prior / panel / (str(ordinal) + '-' + condition) / 'result.json', {'condition': condition, 'exit_code': 0, 'cost_complete': True})
            inputs = []
            config = {}
            for key in ['instructions_file', 'instruction_manifest', 'instruction_block', 'task_file', 'brief_file']:
                path = root / key
                path.write_text('Synthetic input.\n')
                config[key] = str(path)
                inputs.append({'path': str(path), 'sha256': sha(read(path))})
            config_path = root / 'config.json'
            save(config_path, config)
            for name in ['runner', 'guard']:
                (root / name).write_text('Synthetic file; never executed.\n')
            plan = {'schema': SCHEMA, 'models': MODELS, 'effort': 'medium', 'order': ORDER, 'runner': {'path': str(root / 'runner'), 'sha256': sha(read(root / 'runner'))}, 'instruction_guard': {'path': str(root / 'guard'), 'sha256': sha(read(root / 'guard'))}, 'coordinator_sha256': sha(read(__file__)), 'task_config': {'path': str(config_path), 'sha256': sha(read(config_path))}, 'input_files': inputs, 'prior_round_runs': str(prior)}
            class FakeRunner:
                MODEL = MODELS['opus']

                def run(self, config, condition, run_dir):
                    run_dir.mkdir(parents=True, exist_ok=False)
                    result = {'model': self.MODEL, 'effort': 'medium', 'init': {'model': self.MODEL}, 'accepted': True, 'completed': True, 'cost_complete': True, 'cost_usd_list_estimate': 0.1, 'attempts': [{'result': {'modelUsage': {self.MODEL: {}}}}]}
                    save(run_dir / 'result.json', result)
                    return result
            fake = FakeRunner()
            with mock.patch(__name__ + '.load_runner', return_value=fake), contextlib.redirect_stdout(io.StringIO()):
                execute(plan, root / 'study', 1, 1)
                with self.assertRaisesRegex(ValueError, 'already exists'):
                    execute(plan, root / 'study', 1, 1)
                execute(plan, root / 'study', 2, 3)
            rows = schedule()
            result = json.loads(read(root / 'study/runs' / rows[2]['label'] / 'arm-result.json'))
            self.assertEqual(result['model'], MODELS['sonnet'])
            self.assertEqual(result['condition'], 'treatment')
            self.assertTrue(result['model_binding_ok'])
            self.assertTrue(result['accepted'])
            (root / 'brief_file').write_text('Changed input.\n')
            with self.assertRaisesRegex(ValueError, 'changed'):
                validate_plan(plan)


if __name__ == '__main__':
    main()
