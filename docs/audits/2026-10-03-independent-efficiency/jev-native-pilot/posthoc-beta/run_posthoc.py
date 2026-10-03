#!/usr/bin/env python3
"""Run the separate beta diagnostic only after the twelve-attempt panel closes."""
import argparse
import copy
from datetime import datetime, timezone
import fcntl
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import sys
import tarfile
import time
import uuid

SOURCE = '7ec5f6e83d018ab2f25d7a61ff695de81f2f55f6'
DIAGNOSTIC_SHA = 'd91083ae434b72265e688b0a2061d626efb6f701337503fa85e79b294e83f062'
REFERENCE = {'candidate.tar.gz': '55e41de3062f116604ff6dcd54850622af2ac4e7aefefc28ebee1b8fbfc7d7c6',
             'changes.json': '47b77f3430428dad7cb8f06681525e48bec1def069ae1d468adf6b7c9d96b919'}


def sha(path):
    h = hashlib.sha256()
    with Path(path).open('rb') as f:
        while block := f.read(1024 * 1024):
            h.update(block)
    return h.hexdigest()


def read(path):
    path = Path(path)
    if path.is_symlink() or not path.is_file() or path.stat().st_size > 32 * 1024 * 1024:
        raise ValueError('Invalid retained JSON file')
    value = json.loads(path.read_bytes())
    if not isinstance(value, dict):
        raise ValueError('Expected a JSON object')
    return value


def bound(ref):
    path = Path(ref['path'])
    if sha(path) != ref['sha256']:
        raise ValueError('A frozen artifact changed')
    return read(path)


def utc():
    return datetime.now(timezone.utc).isoformat()


def barrier(plan_path, panel_path):
    """Read completion first; no candidate is executed under an active panel."""
    plan, panel = read(plan_path), read(panel_path)
    if (panel.get('status') != 'complete' or panel.get('plan_sha256') != sha(plan_path)
            or len(panel.get('attempts', [])) != 12 or len(plan.get('schedule', [])) != 12):
        raise ValueError('The complete native panel is required before diagnostics')
    expected = {r['run_id']: r for r in plan['schedule']}
    if len(expected) != 12 or {r['run_id'] for r in panel['attempts']} != set(expected):
        raise ValueError('The completed panel does not match its plan')
    for entry in panel['attempts']:
        scheduled = expected[entry['run_id']]
        receipt_path = Path(scheduled['output']) / 'pilot.json'
        if sha(receipt_path) != entry['receipt_sha256']:
            raise ValueError('A primary receipt changed')
        receipt = read(receipt_path)
        if (receipt.get('run_id') != entry['run_id'] or receipt.get('config_sha256') != scheduled['config']['sha256']
                or receipt.get('execution_closed') is not True
                or receipt.get('accounting_complete') is not True or receipt.get('safe_to_continue') is not True):
            raise ValueError('An original attempt has unconfirmed effects')
    return plan


def run(args):
    os.umask(0o077)
    plan_path, panel_path = args.plan.resolve(), args.panel_receipt.resolve()
    plan = barrier(plan_path, panel_path)
    harness = args.harness_dir.resolve()
    # The old harness and source-selector modules are exactly the panel's files.
    for relative, expected in plan['module_hashes'].items():
        if sha(harness.parent / relative) != expected:
            raise ValueError('A frozen harness module changed')
    if sha(args.diagnostic) != DIAGNOSTIC_SHA:
        raise ValueError('The retrospective diagnostic draft changed')
    for name, expected in REFERENCE.items():
        if sha(args.reference_candidate / name) != expected:
            raise ValueError('The qualified historical reference payload changed')
    sys.path.insert(0, str(harness))
    import candidate
    import check_candidate
    import trial

    selected = [r for r in plan['schedule'] if r['task_id'] == 'alternative-beta']
    if len(selected) != 6:
        raise ValueError('Expected all six beta attempts')
    inputs = []
    common = None
    for scheduled in selected:
        config = bound(scheduled['config'])
        task_manifest = bound(config['task_manifest'])
        task = next(t for t in task_manifest['tasks'] if t['id'] == 'alternative-beta')
        if task['source_commit'] != SOURCE:
            raise ValueError('The beta source changed')
        native = config['native_template']
        template = config['acceptance_template']
        fixed = {'native': {k: native[k] for k in ('source_archive', 'source_archive_sha256', 'target_seed', 'target_seed_manifest_sha256', 'toolchain')},
                 'acceptance': template, 'slot_lock': config['slot_lock']}
        if common is None:
            common = fixed
        elif common != fixed:
            raise ValueError('The beta attempts have different source, seed or check policies')
        primary = read(Path(scheduled['output']) / 'pilot.json')
        native_dir = Path(scheduled['output']) / 'native'
        identity = primary['candidate_manifest_sha256']
        candidate.validate(native_dir, identity, SOURCE, native['source_archive_sha256'])
        native_receipt = read(native_dir / 'result.json')
        if native_receipt.get('candidate_manifest_sha256') != identity:
            raise ValueError('The primary native capture differs')
        inputs.append({'role': 'native_candidate', 'blind_id': str(uuid.uuid4()),
                       'source_run_id': scheduled['run_id'], 'candidate_dir': str(native_dir),
                       'candidate_manifest_sha256': identity, 'expected_snapshot_commit': native_receipt['snapshot_commit'],
                       'primary_receipt_sha256': sha(Path(scheduled['output']) / 'pilot.json')})
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False, mode=0o700)
    panel_lock = (panel_path.parent.parent / '.native-panel.lock').open('a')
    slot_lock = Path(common['slot_lock']).open('a')
    fcntl.flock(panel_lock.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
    fcntl.flock(slot_lock.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
    row = {'schema': 'openagents.jev-lifecycle.beta-posthoc.v1', 'status': 'preparing',
           'primary_scores_unchanged': True, 'post_hoc': True, 'model_calls': 0,
           'started_at': utc(), 'plan_sha256': sha(plan_path), 'panel_receipt_sha256': sha(panel_path),
           'runner_sha256': sha(__file__), 'diagnostic_sha256': DIAGNOSTIC_SHA,
           'historical_reference_payload': REFERENCE, 'common_bindings': common, 'attempts': []}
    started = time.monotonic()
    trial.durable(output / 'diagnostic.json', row)
    try:
        # Controls use private copies; all original native artifacts stay immutable.
        controls = []
        for role in ('historical_reference', 'unmodified_base'):
            directory = output / 'inputs' / role
            directory.mkdir(parents=True)
            if role == 'historical_reference':
                for name in REFERENCE:
                    shutil.copyfile(args.reference_candidate / name, directory / name)
            else:
                with tarfile.open(directory / 'candidate.tar.gz', 'w:gz'):
                    pass
                (directory / 'changes.json').write_text('{}\n')
            identity, _ = candidate.write_manifest(directory, SOURCE, common['native']['source_archive_sha256'], read(directory / 'changes.json'))
            controls.append({'role': role, 'blind_id': str(uuid.uuid4()), 'candidate_dir': str(directory),
                             'candidate_manifest_sha256': identity})
        row['inputs'] = controls + inputs
        row['status'] = 'running'
        trial.durable(output / 'diagnostic.json', row)
        for item in row['inputs']:
            attempt_dir = output / item['blind_id']
            attempt_dir.mkdir()
            config = copy.deepcopy(common['acceptance'])
            # Replace only the independent test target. Ordinary checks, scope,
            # profiles, feature policy and 240-second deadline remain unchanged.
            config.update(run_id=str(uuid.uuid4()), source_commit=SOURCE,
                          candidate_dir=item['candidate_dir'], candidate_manifest_sha256=item['candidate_manifest_sha256'],
                          source_archive=common['native']['source_archive'], source_archive_sha256=common['native']['source_archive_sha256'],
                          target_seed=common['native']['target_seed'], target_seed_manifest_sha256=common['native']['target_seed_manifest_sha256'])
            config['checker'] = {'path': str(args.diagnostic.resolve()), 'sha256': DIAGNOSTIC_SHA,
                                 'package': 'coderbench', 'target': 'posthoc_trace_integrity',
                                 'injection': 'crates/coderbench/tests/posthoc_trace_integrity.rs'}
            if 'expected_snapshot_commit' in item:
                config['expected_snapshot_commit'] = item['expected_snapshot_commit']
            trial.durable(attempt_dir / 'config.json', config)
            record = dict(item, status='launch_intent', started_at=utc(), config_sha256=sha(attempt_dir / 'config.json'))
            row['attempts'].append(record)
            trial.durable(output / 'diagnostic.json', row)
            result = check_candidate.run(config, attempt_dir / 'acceptance')
            record.update(status=result['status'], completed=result['completed'], execution_closed=result['execution_closed'],
                          diagnostic_checks_passed=result['accepted'], total_wall_s=result['total_wall_s'],
                          checks_sha256=sha(attempt_dir / 'acceptance/checks.json'),
                          outcomes={p: result[p] for p in ('scope', 'format', 'ordinary', 'independent')})
            record['retained'] = {p.name: {'sha256': sha(p), 'bytes': p.stat().st_size}
                                  for p in (attempt_dir / 'acceptance').iterdir() if p.is_file()}
            trial.durable(attempt_dir / 'retained.json', record)
            trial.durable(output / 'diagnostic.json', row)
            cleanup_started = time.monotonic()
            record['cleanup'] = trial.cleanup_targets(attempt_dir, False, result['execution_closed'] is True, True)
            record['cleanup_wall_s'] = time.monotonic() - cleanup_started
            trial.durable(attempt_dir / 'retained.json', record)
            trial.durable(output / 'diagnostic.json', row)
            if result['execution_closed'] is not True or not all(r['removed'] for r in record['cleanup']):
                raise ValueError('Diagnostic execution closure or scratch cleanup failed')
            if item['role'] == 'historical_reference' and (result['completed'] is not True or result['accepted'] is not True):
                row['status'] = 'stopped_reference_diagnostic_failure'
                break
            if item['role'] == 'unmodified_base' and (result['completed'] is not True or result['ordinary']['passed'] is not True or result['independent']['passed'] is not False):
                row['status'] = 'stopped_base_control_unexpected'
                break
        else:
            row['status'] = 'complete'
    except BaseException as error:
        row.update(status='stopped_infrastructure', error_type=type(error).__name__)
    finally:
        row['finished_at'] = utc()
        row['total_wall_s'] = time.monotonic() - started
        trial.durable(output / 'diagnostic.json', row)
        slot_lock.close(); panel_lock.close()
    return row


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('plan', 'panel-receipt', 'harness-dir', 'reference-candidate', 'diagnostic', 'output'):
        parser.add_argument('--' + name, type=Path, required=True)
    args = parser.parse_args()
    def interrupted(number, frame):
        raise InterruptedError('posthoc_interrupted')
    for number in (signal.SIGTERM, signal.SIGINT):
        signal.signal(number, interrupted)
    result = run(args)
    print(json.dumps({k: result.get(k) for k in ('status', 'total_wall_s', 'model_calls')}))
    raise SystemExit(0 if result['status'] == 'complete' else 1)


if __name__ == '__main__':
    main()
