#!/usr/bin/env python3
"""Run one exposed-task pilot attempt; retained attempts are never relaunched."""
from __future__ import annotations

import argparse
import fcntl
from datetime import datetime, timezone
from contextlib import contextmanager
import hashlib
import json
import math
import os
from pathlib import Path
import signal
import stat
import sys
import time
import uuid

HERE = Path(__file__).resolve().parent
LEGACY = HERE.parent / 'delegation-study'
sys.path.insert(0, str(LEGACY))
import candidate
import report as legacy_report
import trial
from seed_manifest import cargo_features, feature_check_command
import context
import gateway
import spans

SCHEMA = 'openagents.jev-lifecycle.native-pilot.v1'
TOOLS = 'Bash,Read,Edit,Write,Glob,Grep'
OLD_MODULES = ('run_remote.py', 'check_candidate.py', 'broker.py', 'bridge.py',
               'run_inner.py', 'candidate.py', 'capture_limits.py', 'seed_manifest.py',
               'trial.py', 'report.py', 'schedule.py')
NEW_MODULES = ('native_pilot.py', 'make_native_plan.py', 'run_native_panel.py', 'gateway.py', 'spans.py', 'context.py')
NATIVE_KEYS = {'source_archive', 'source_archive_sha256', 'target_seed',
               'target_seed_manifest_sha256', 'binary', 'binary_sha256', 'binary_version',
               'model', 'effort', 'provider_meter', 'toolchain', 'cargo_features',
               'timeout_s', 'cli_budget_usd', 'initialize_git', 'capture_limits'}
CHECK_KEYS = {'packages', 'allowed_paths', 'checker', 'toolchain', 'cargo_features', 'total_timeout_s'}


def utc():
    return datetime.now(timezone.utc).isoformat()


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def read_json(path, limit=16 * 1024 * 1024):
    path = Path(path)
    if path.is_symlink() or not path.is_file() or path.stat().st_size > limit:
        raise ValueError('Invalid or oversized retained JSON file')
    value = json.loads(path.read_bytes())
    if not isinstance(value, dict):
        raise ValueError('A retained JSON record must be an object')
    return value


def bound_file(ref, limit=16 * 1024 * 1024):
    path = Path(ref['path'])
    if path.is_symlink() or not path.is_file() or path.stat().st_size > limit:
        raise ValueError('Invalid or oversized input artifact')
    raw = path.read_bytes()
    if sha(raw) != ref['sha256']:
        raise ValueError('An input artifact changed')
    return raw


def module_hashes():
    return {str(p.relative_to(HERE.parent)): sha(p.read_bytes())
            for p in [*(LEGACY / n for n in OLD_MODULES), *(HERE / n for n in NEW_MODULES)]}


def verify_modules(config):
    if config['module_hashes'] != module_hashes():
        raise ValueError('The pilot module closure changed')


def finite(value):
    return type(value) in (int, float) and math.isfinite(value) and value >= 0


def read_key(path):
    path = Path(path)
    info = path.lstat()
    if not stat.S_ISREG(info.st_mode) or stat.S_IMODE(info.st_mode) != 0o600 or info.st_size > 16384:
        raise ValueError('The gateway key must be a bounded private regular file with mode0600')
    key = path.read_text().strip()
    if not key or '\n' in key or '\r' in key:
        raise ValueError('The gateway key is invalid')
    return key


def gateway_call(config, catalog, output):
    """Keep gateway authentication out of every native subprocess environment."""
    if 'AI_GATEWAY_API_KEY' in os.environ:
        raise ValueError('Pass a private key file instead of an inherited gateway key')
    key = read_key(config['gateway_key_file'])
    try:
        os.environ['AI_GATEWAY_API_KEY'] = key
        return gateway.call(spans.state(catalog), spans.questions(catalog), output, timeout=30)
    finally:
        os.environ.pop('AI_GATEWAY_API_KEY', None)
        key = None


def prepare(config, task, output):
    """Prepare from the same pinned source catalog; Jev selects source only."""
    output.mkdir(mode=0o700)
    started = time.monotonic()
    repo = Path(config['source_repo'])
    bindings = context.tree(repo, task['source_commit'])
    blocks = []
    readings = task['applicable_instruction_paths'] + task.get('required_public_readings', [])
    seen = set()
    for ref in readings:
        if ref['path'] in seen:
            continue
        seen.add(ref['path'])
        raw = context.read_blob(repo, ref['path'], bindings)
        if sha(raw) != ref['sha256']:
            raise ValueError('An applicable instruction changed')
        blocks.append(f"## Source instructions: {ref['path']}\n\n".encode() + raw)
    features = cargo_features(task.get('cargo_features', []), task['packages'])
    package_args = ' '.join('-p ' + p for p in task['packages'])
    ordinary = 'cargo test --locked --offline ' + package_args
    if features:
        ordinary += ' --features ' + ','.join(features)
    envelope = ('This is an isolated historical coding exercise. Produce the candidate in the working tree; '
                'the harness checks the final state after the session ends. Follow the applicable source instructions below.\n\n'
                'The exercise does not authorize claiming or updating live issues, committing, pushing, deploying, or contacting other systems. '
                'Direct external networking is unavailable. Keep the configured external CARGO_TARGET_DIR and generated build outputs outside the source tree.\n\n'
                'Allowed edit roots: ' + ', '.join(task['allowed_paths']) + '\n'
                'Named checks: cargo fmt ' + package_args + ' -- --check; ' + ordinary + '\n\n'
                'Run the named checks and requested focused regressions. Report changes, checks, and unresolved failures.\n\n# Task\n\n'
                + task.get('title', '') + '\n\n' + task['prompt'])
    common = envelope.encode() + b'\n\n' + b'\n\n'.join(blocks)
    if features:
        common += b'\n\nRequired ordinary check: `' + feature_check_command(features).encode() + b'`.\n'
    (output / 'common-prompt.txt').write_bytes(common)
    index = json.loads(bound_file(config['index'], 128 * 1024 * 1024))
    if index.get('commit') != task['source_commit']:
        raise ValueError('The source index and task differ')
    row = {'schema': 'openagents.jev-lifecycle.native-preparation.v1', 'arm': config['arm'],
           'source_commit': task['source_commit'], 'common_prompt_sha256': sha(common),
           'index_sha256': config['index']['sha256'], 'cost_usd': 0.0,
           'accounting_complete': True, 'status': 'complete'}
    pack = b''
    if config['arm'] != 'bare':
        catalog = spans.catalog(repo, task['source_commit'], index, task)
        trial.durable(output / 'catalog.json', catalog)
        choices = None
        if config['arm'] == 'jev':
            receipt, response = gateway_call(config, catalog, output / 'gateway-call')
            trial.durable(output / 'gateway-receipt.json', receipt)
            row['gateway'] = receipt
            if (receipt.get('outcome') != 'answered' or receipt.get('answers_valid') is not True
                    or not isinstance(response, dict) or receipt.get('cost_status') != 'gateway_reported'
                    or not finite(receipt.get('cost_usd'))):
                raise ValueError('Jev preparation did not return a charged valid answer')
            row.update(cost_usd=receipt['cost_usd'], accounting_complete=True)
            choices = {identity: response['answers'][identity]['choice'] for identity in catalog['clauses']}
        rendered = spans.pack(repo, catalog, choices=choices)
        pack = rendered['text'].encode()
        if len(pack) > 16384 or rendered['sha256'] != sha(pack):
            raise ValueError('The source pack violates its byte or identity bound')
        trial.durable(output / 'pack.json', rendered)
        (output / 'briefing.md').write_bytes(pack)
        row.update(catalog_sha256=catalog['catalog_sha256'], pack_sha256=sha(pack), pack_bytes=len(pack))
    prompt = common + (b'\n\n' + pack if pack else b'')
    (output / 'prompt.txt').write_bytes(prompt)
    row.update(prompt_sha256=sha(prompt), wall_s=time.monotonic() - started)
    trial.durable(output / 'preparation.json', row)
    return row


@contextmanager
def preparation_deadline(seconds=90):
    def expired(number, frame):
        raise TimeoutError('Preparation wall deadline exceeded')
    previous = signal.signal(signal.SIGALRM, expired)
    signal.setitimer(signal.ITIMER_REAL, seconds)
    try:
        yield
    finally:
        signal.setitimer(signal.ITIMER_REAL, 0)
        signal.signal(signal.SIGALRM, previous)


def validate(config):
    if config.get('schema') != SCHEMA or str(uuid.UUID(config['run_id'])) != config['run_id']:
        raise ValueError('Invalid pilot configuration identity')
    if config['arm'] not in ('bare', 'deterministic', 'jev'):
        raise ValueError('Unknown pilot arm')
    verify_modules(config)
    bound_file(config['protocol'])
    manifest = json.loads(bound_file(config['task_manifest']))
    tasks = [t for t in manifest['tasks'] if t['id'] == config['task_id']]
    if len(tasks) != 1:
        raise ValueError('Expected one public task')
    task = tasks[0]
    native, checks = config['native_template'], config['acceptance_template']
    if set(native) - NATIVE_KEYS or set(checks) - CHECK_KEYS:
        raise ValueError('Templates contain derived or unknown fields')
    if (native.get('model') != 'claude-sonnet-5-5' or native.get('effort') != 'medium'
            or native.get('timeout_s') != 600 or native.get('cli_budget_usd') != 2
            or native.get('initialize_git') is not True or checks.get('total_timeout_s') != 240):
        raise ValueError('The common model, effort, budget or deadlines changed')
    features = cargo_features(task.get('cargo_features', []), task['packages'])
    if (cargo_features(native.get('cargo_features', [])) != features
            or cargo_features(checks.get('cargo_features', [])) != features
            or checks['packages'] != task['packages'] or checks['allowed_paths'] != task['allowed_paths']
            or native['toolchain'] != checks['toolchain']):
        raise ValueError('Task, native and final checks must share scope, features and toolchain')
    if native['provider_meter']['admission_target_usd'] != 8:
        raise ValueError('The broker admission target changed')
    bound_file({'path': checks['checker']['path'], 'sha256': checks['checker']['sha256']})
    if config['arm'] != 'bare':
        bound_file(config['lean_system'])
    if 'AI_GATEWAY_API_KEY' in os.environ:
        raise ValueError('Gateway authentication must not be inherited')
    return task


def native_binding(native, config):
    expected = {'run_id': config['run_id'], 'source_commit': config['source_commit'],
                'source_archive_sha256': config['source_archive_sha256'], 'prompt_sha256': config['prompt_sha256'],
                'cli_sha256': config['binary_sha256'], 'cli_hash_after': config['binary_sha256'],
                'cli_version': config['binary_version'], 'model': config['model'], 'effort': config['effort'],
                'target_seed_manifest_sha256': config['target_seed_manifest_sha256']}
    if any(native.get(k) != v for k, v in expected.items()):
        raise ValueError('Native result identity differs from its input')
    allowed = set(config['provider_meter']['models'])
    served = native.get('served_models')
    if (not isinstance(served, list) or any(m not in allowed for m in served)
            or (native.get('model_completed') is True and config['model'] not in served)):
        raise ValueError('Native served models differ from the admitted models')
    if native.get('argv', [])[1:] != trial.native_arguments(config):
        raise ValueError('Native prompt/tool arguments changed')
    if cargo_features(native.get('cargo_features', [])) != cargo_features(config.get('cargo_features', [])):
        raise ValueError('Native features changed')
    prompt = Path(config['prompt_file']).read_bytes()
    delivered = ('Benchmark run ID: ' + config['run_id'] + '\n\n').encode() + prompt
    if native.get('delivered_prompt_sha256') != sha(delivered):
        raise ValueError('Delivered native prompt changed')


def accounting(output, config, native_started, native_closed, prep_started):
    native_low = 0.0; native_high = 0.0 if not native_started else None
    provider = None
    if native_started and (output / 'native/provider-calls.jsonl').is_file():
        meter = config['native_template']['provider_meter']
        prices = {k: v['usd_per_million'] for k, v in meter['models'].items()}
        provider = legacy_report.provider_cost((output / 'native/provider-calls.jsonl').read_text(),
                                               config['run_id'], list(prices), prices)
        native_low = provider['lower_usd']
        native_high = provider['upper_usd'] if native_closed else None
        result_path = output / 'native/result.json'
        if result_path.exists():
            native = read_json(result_path)
            if finite(native.get('cost_usd')) and native_high is not None and native['cost_usd'] > native_high + 1e-9:
                native_high = None
    jev_cost = 0.0
    if prep_started and config['arm'] == 'jev':
        receipt = output / 'preparation/gateway-call/receipt.json'
        if not receipt.exists():
            jev_cost = None
        else:
            value = read_json(receipt)
            jev_cost = value.get('cost_usd') if finite(value.get('cost_usd')) else None
    known = native_high is not None and jev_cost is not None
    return {'provider': provider, 'native_cost_lower_usd': native_low, 'native_cost_upper_usd': native_high,
            'jev_cost_usd': jev_cost, 'cost_lower_usd': native_low + (jev_cost or 0.0),
            'cost_upper_usd': native_high + jev_cost if known else None, 'accounting_complete': known}


def run(config_path, output, *, prepare_fn=prepare, phase_fn=trial.process_phase):
    started = time.monotonic_ns()
    config_path, output = Path(config_path).resolve(), Path(output).resolve()
    os.umask(0o077)
    output.mkdir(parents=True, exist_ok=False, mode=0o700)
    config = read_json(config_path)
    row = {'schema': SCHEMA, 'run_id': config.get('run_id'), 'arm': config.get('arm'),
           'task_id': config.get('task_id'), 'config_sha256': sha(config_path.read_bytes()),
           'status': 'incomplete', 'accepted': False, 'started_at': utc(), 'phases': {}, 'errors': []}
    trial.durable(output / 'launch-intent.json', row)
    trial.durable(output / 'config.private.json', config)
    native_started = native_closed = check_started = check_closed = prep_started = released_started = False
    native = {}; checks = {}; identity = None; candidate_valid = False
    stage = 'validation'
    lock = None

    def phase(name, argv, timeout):
        verify_modules(config)
        trial.event(output, {'phase': name, 'event': 'launch_intent', 'at': utc(),
                             'monotonic_ns': time.monotonic_ns(), 'timeout_s': timeout})
        began = time.monotonic()
        result = phase_fn(name, argv, output, timeout)
        row['phases'][name] = dict(result, wall_s=time.monotonic() - began)
        trial.event(output, {'phase': name, 'event': 'returned', 'at': utc(), **result})
        return result

    try:
        task = validate(config)
        lock = Path(config['slot_lock']).open('a')
        fcntl.flock(lock.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
        row['source_commit'] = task['source_commit']
        stage = 'preparation'; prep_started = True
        trial.event(output, {'phase': stage, 'event': 'launch_intent', 'at': utc()})
        before = time.monotonic()
        with preparation_deadline():
            prepared = prepare_fn(config, task, output / 'preparation')
        row['phases'][stage] = {'wall_s': time.monotonic() - before}
        row['preparation'] = prepared
        if not prepared.get('accounting_complete') or prepared.get('status') != 'complete':
            raise ValueError('Preparation is incomplete')
        if 'AI_GATEWAY_API_KEY' in os.environ:
            raise ValueError('Gateway authentication leaked into the native environment')
        nconfig = dict(config['native_template'], run_id=config['run_id'], source_commit=task['source_commit'],
                       credential_file=config['credential_file'], prompt_file=str(output / 'preparation/prompt.txt'),
                       prompt_sha256=prepared['prompt_sha256'], tools=None)
        if config['arm'] != 'bare':
            nconfig.update(system_file=config['lean_system']['path'], system_sha256=config['lean_system']['sha256'], tools=TOOLS)
        trial.durable(output / 'native-config.private.json', nconfig)
        stage = 'native'; native_started = True
        process = phase(stage, [sys.executable, str(LEGACY / 'run_remote.py'),
                                str(output / 'native-config.private.json'), str(output / 'native')], 1200)
        native = read_json(output / 'native/result.json')
        native_closed = not process['timed_out'] and native.get('execution_closed') is True
        if not native_closed:
            raise ValueError('Native execution closure is unconfirmed')
        native_binding(native, nconfig)
        identity = native['candidate_manifest_sha256']
        candidate.validate(output / 'native', identity, task['source_commit'], nconfig['source_archive_sha256'])
        candidate_valid = True
        if not isinstance(native.get('snapshot_commit'), str) or len(native['snapshot_commit']) != 40:
            raise ValueError('Missing native snapshot identity')
        row.update(accounting(output, config, native_started, native_closed, prep_started))
        if not row['accounting_complete']:
            raise ValueError('Native or preparation accounting is incomplete')
        stage = 'native_scratch_release'; released_started = True
        release = trial.release_native_scratch(output, output, native, identity)
        row['native_scratch_release'] = release
        if release['status'] != 'complete':
            raise ValueError('Native scratch release failed; final checks were not launched')
        aconfig = dict(config['acceptance_template'], run_id=config['run_id'], source_commit=task['source_commit'],
                       source_archive=nconfig['source_archive'], source_archive_sha256=nconfig['source_archive_sha256'],
                       candidate_dir=str(output / 'native'), candidate_manifest_sha256=identity,
                       expected_snapshot_commit=native['snapshot_commit'], target_seed=nconfig['target_seed'],
                       target_seed_manifest_sha256=nconfig['target_seed_manifest_sha256'])
        trial.durable(output / 'acceptance-config.private.json', aconfig)
        stage = 'acceptance'; check_started = True
        process = phase(stage, [sys.executable, str(LEGACY / 'check_candidate.py'),
                                str(output / 'acceptance-config.private.json'), str(output / 'acceptance')], 260)
        checks = read_json(output / 'acceptance/checks.json')
        check_closed = not process['timed_out'] and checks.get('execution_closed') is True
        expected = {k: aconfig[k] for k in ('run_id', 'source_commit', 'source_archive_sha256', 'candidate_manifest_sha256')}
        expected['checker_sha256'] = aconfig['checker']['sha256']
        if (not check_closed or checks.get('completed') is not True
                or checks.get('schema') != 'openagents.delegation.final-checks.v1'
                or any(checks.get(k) != v for k, v in expected.items())
                or cargo_features(checks.get('cargo_features', [])) != cargo_features(task.get('cargo_features', []))):
            raise ValueError('Final checks are incomplete or name another input')
        outcomes = [checks.get(k, {}).get('passed') for k in ('scope', 'format', 'ordinary', 'independent')]
        if any(type(v) is not bool for v in outcomes):
            raise ValueError('Final check outcomes are missing')
        row['candidate_checks_passed'] = all(outcomes)
        row['model_completed'] = native.get('model_completed') is True
        row['accepted'] = row['candidate_checks_passed']
        row['status'] = 'complete'
    except BaseException as error:
        row.update(status='failed', failed_stage=stage)
        row['errors'].append(type(error).__name__)
    finally:
        row['execution_closed'] = (not native_started or native_closed) and (not check_started or check_closed)
        try:
            row.update(accounting(output, config, native_started, native_closed, prep_started))
        except (OSError, ValueError, KeyError, TypeError, AttributeError):
            row.update(accounting_complete=False, cost_upper_usd=None)
            row['errors'].append('accounting_unavailable')
        row['candidate_manifest_sha256'] = identity
        try:
            row['private_logs'] = trial.retain_logs(output, output)
            row['artifacts'] = {name: trial.reference(output, output / relative)
                                for name, relative in [('native', 'native/result.json'), ('checks', 'acceptance/checks.json'),
                                                       ('candidate', 'native/candidate-manifest.json'), ('payload', 'native/candidate.tar.gz')]
                                if (output / relative).is_file()}
        except (OSError, ValueError):
            row['errors'].append('retention_failed'); row['status'] = 'incomplete'
        row['checks_endpoint_wall_s'] = (time.monotonic_ns() - started) / 1e9
        trial.durable(output / 'through-checks.json', row)
        # Both scratch releases are measured. A failed native release is never retried.
        before = time.monotonic()
        row['cleanup'] = trial.cleanup_targets(output, native_closed and not released_started,
                                                check_closed, candidate_valid)
        row['phases']['final_cleanup'] = {'wall_s': time.monotonic() - before}
        cleanup_ok = all(r['removed'] for r in row['cleanup']) and (not released_started or row.get('native_scratch_release', {}).get('status') == 'complete')
        row['safe_to_continue'] = (row['execution_closed'] and row.get('accounting_complete') is True
                                   and cleanup_ok and row['status'] == 'complete')
        if not row['execution_closed']:
            row['status'] = 'incomplete'
        row['accepted'] = (row.get('candidate_checks_passed') is True and row['execution_closed']
                           and row.get('accounting_complete') is True and row['status'] == 'complete')
        row['finished_at'] = utc()
        row['endpoint_wall_s'] = (time.monotonic_ns() - started) / 1e9
        row['endpoint_scope'] = 'configuration and source preparation through retained checks and confirmed scratch cleanup; final receipt serialization excluded'
        trial.durable(output / 'pilot.json', row)
        if lock is not None:
            lock.close()
    return row


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--config', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    def interrupted(number, frame):
        raise InterruptedError('pilot_interrupted')
    for number in (signal.SIGTERM, signal.SIGINT):
        signal.signal(number, interrupted)
    result = run(args.config, args.output)
    print(json.dumps({k: result.get(k) for k in ('status', 'accepted', 'safe_to_continue', 'accounting_complete', 'cost_upper_usd', 'endpoint_wall_s')}))
    raise SystemExit(0 if result['safe_to_continue'] else 1)


if __name__ == '__main__':
    main()
