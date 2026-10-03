#!/usr/bin/env python3
"""Prepare all registered reviews, then optionally execute one frozen batch."""
from __future__ import annotations

import argparse
from contextlib import contextmanager
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import re
import signal
import time
import uuid

HERE = Path(__file__).resolve().parent
EXPECTED_FREEZE = '80288ea9a61fdfb3a2f6bd8e75c5347bc761073fd3cc1d05ee6ec9e9f8869b5c'
MAX_CALLS = 12
COST_STOP = .05
SOCKET_TIMEOUT = 30
OUTER_TIMEOUT = 90
MAX_REQUEST = 128 * 1024


class CallDeadline(BaseException):
    pass


class StopRequested(BaseException):
    pass


def utc():
    return datetime.now(timezone.utc).isoformat()


def encoded(value):
    return json.dumps(value, ensure_ascii=False, allow_nan=False, separators=(',', ':')).encode()


def file_ref(path, limit=512 * 1024 * 1024):
    path = Path(path)
    if path.is_symlink() or not path.is_file() or path.stat().st_size > limit:
        raise ValueError('Invalid bounded input artifact')
    value = hashlib.sha256(); size = 0
    with path.open('rb') as handle:
        while block := handle.read(1024 * 1024):
            size += len(block)
            if size > limit: raise ValueError('Input artifact grew past its bound')
            value.update(block)
    return {'sha256': value.hexdigest(), 'bytes': size}


def read(path, limit=128 * 1024 * 1024):
    file_ref(path, limit)
    value = json.loads(Path(path).read_bytes())
    if not isinstance(value, dict): raise ValueError('Expected an object artifact')
    return value


def durable(path, value):
    path = Path(path)
    temporary = path.with_name('.' + path.name + '.tmp')
    with temporary.open('wb') as handle:
        handle.write(encoded(value) + b'\n'); handle.flush(); os.fsync(handle.fileno())
    os.replace(temporary, path)


def load(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
    return module


def relative(value):
    path = Path(value)
    if not isinstance(value, str) or path.is_absolute() or '..' in path.parts or str(path) != value:
        raise ValueError('Invalid relative artifact path')
    return path


def verify_bundle(harness, bundle=HERE, expected_freeze=EXPECTED_FREEZE):
    bundle, harness = Path(bundle), Path(harness)
    if file_ref(bundle / 'freeze.json')['sha256'] != expected_freeze:
        raise ValueError('The preparer freeze changed')
    freeze = read(bundle / 'freeze.json')
    for name, reference in freeze['prototype'].items():
        if file_ref(bundle / relative(name)) != reference: raise ValueError('Frozen prototype changed')
    for name, reference in freeze['imported_modules'].items():
        if file_ref(harness / relative(name)) != reference: raise ValueError('Frozen imported module changed')
    return freeze


def prepare_all(harness, repo, evidence, output, *, bundle=HERE, expected_freeze=EXPECTED_FREEZE, prepare_fn=None):
    """Write every preparation and a final registration; make zero network calls."""
    os.umask(0o077)
    began = time.monotonic()
    harness, repo, evidence, output, bundle = map(Path, (harness, repo, evidence, output, bundle))
    freeze = verify_bundle(harness, bundle, expected_freeze)
    plan_path = evidence / 'plan/plan.json'; plan = read(plan_path)
    if file_ref(plan_path) != freeze['original_native_plan']: raise ValueError('The original native plan changed')
    panel_path = evidence / 'panel/panel.json'; panel = read(panel_path)
    ended = ('complete', 'stopped_cost_admission', 'stopped_unknown_cost', 'stopped_incomplete_attempt', 'stopped_infrastructure')
    if (panel.get('schema') != 'openagents.jev-lifecycle.native-panel.v1' or panel.get('status') not in ended
            or panel.get('plan_sha256') != file_ref(plan_path)['sha256'] or not panel.get('finished_at')
            or panel.get('driver_sha256') != plan.get('module_hashes', {}).get('jev-lifecycle/run_native_panel.py')
            or panel.get('probe_sha256') != plan.get('capability_probe', {}).get('sha256')):
        raise ValueError('The original native panel has not ended with a bound receipt')
    schedule = plan['schedule']
    if len(schedule) != 12 or [r['position'] for r in schedule] != list(range(1, 13)):
        raise ValueError('The original twelve-slot schedule is invalid')
    ids = [r['run_id'] for r in schedule]
    if len(set(ids)) != 12 or any(str(uuid.UUID(v)) != v for v in ids): raise ValueError('Invalid scheduled run UUID')
    attempts = panel.get('attempts')
    if not isinstance(attempts, list) or len(attempts) > 12: raise ValueError('Invalid ended panel sequence')
    for actual, expected in zip(attempts, schedule):
        if any(actual.get(k) != expected[k] for k in ('run_id', 'position', 'task_id', 'arm', 'repetition')):
            raise ValueError('Ended panel differs from original order')
        receipt_path = evidence / 'runs' / actual['run_id'] / 'pilot.json'
        expected_receipt = actual.get('receipt_sha256', actual.get('observed_receipt_sha256'))
        if file_ref(receipt_path)['sha256'] != expected_receipt:
            raise ValueError('A launched native receipt is not bound')
        receipt = read(receipt_path)
        if receipt.get('execution_closed') is not True or not receipt.get('finished_at'):
            raise ValueError('A launched native execution has not confirmed closure')
    output.mkdir(mode=0o700, parents=True, exist_ok=False)
    (output / 'prepared').mkdir(mode=0o700)
    imported = load(bundle / 'coverage_review.py', '_frozen_clause_preparer')
    prepare_case = prepare_fn or imported.prepare
    bound_inputs = {}

    def bind(path):
        ref = file_ref(path)
        bound_inputs[str(Path(path).relative_to(evidence))] = ref
        return ref

    bind(plan_path); bind(panel_path)
    cases = []
    durable(output / 'preparing.json', {'status': 'preparing', 'started_at': utc(), 'expected_slots': 12})
    for entry in schedule:
        position, run_id, alias = entry['position'], entry['run_id'], entry['task_id']
        if not re.fullmatch(r'[a-z0-9-]{1,64}', alias): raise ValueError('Invalid public task alias')
        case = {k: entry[k] for k in ('position', 'run_id', 'task_id', 'arm', 'repetition')}
        case.update(ready=False, preparation_status='invalid_input', errors=[])
        destination = output / 'prepared' / f'{position:02d}'; destination.mkdir(mode=0o700)
        case_began = time.monotonic()
        try:
            config_path = evidence / 'plan' / (run_id + '.json')
            if bind(config_path)['sha256'] != entry['config']['sha256']: raise ValueError('config_identity_invalid')
            config = read(config_path)
            if any(config.get(k) != entry[k] for k in ('run_id', 'task_id', 'arm')): raise ValueError('config_slot_invalid')
            if config.get('module_hashes') != plan.get('module_hashes'): raise ValueError('config_modules_invalid')
            task_path = evidence / 'inputs' / alias / 'task.json'
            index_path = evidence / 'inputs' / alias / 'index.json'
            if bind(task_path)['sha256'] != config['task_manifest']['sha256']: raise ValueError('task_identity_invalid')
            if bind(index_path)['sha256'] != config['index']['sha256']: raise ValueError('index_identity_invalid')
            tasks = read(task_path)['tasks']
            if len(tasks) != 1 or tasks[0]['id'] != alias: raise ValueError('task_slot_invalid')
            task = tasks[0]
            if position > len(attempts): raise ValueError('native_slot_not_executed')
            pilot_path = evidence / 'runs' / run_id / 'pilot.json'; pilot_hash = bind(pilot_path)['sha256']
            actual = attempts[position - 1]
            expected_pilot = actual.get('receipt_sha256', actual.get('observed_receipt_sha256'))
            if expected_pilot != pilot_hash: raise ValueError('pilot_identity_invalid')
            # Outcomes are not consulted. Only final candidate/source bindings enter preparation.
            pilot = read(pilot_path)
            native_dir = evidence / 'runs' / run_id / 'native'
            native_hash = bind(native_dir / 'result.json')['sha256']
            if pilot['artifacts']['native']['sha256'] != native_hash: raise ValueError('native_identity_invalid')
            native = read(native_dir / 'result.json')
            archive = config['native_template']['source_archive_sha256']
            identity = native['candidate_manifest_sha256']
            if (native.get('run_id') != run_id or native.get('source_commit') != task['source_commit']
                    or native.get('source_archive_sha256') != archive or pilot.get('candidate_manifest_sha256') != identity):
                raise ValueError('candidate_source_identity_invalid')
            if bind(native_dir / 'candidate-manifest.json')['sha256'] != identity: raise ValueError('candidate_manifest_invalid')
            bind(native_dir / 'candidate.tar.gz'); bind(native_dir / 'changes.json')
            prepared = prepare_case(harness, repo, task, read(index_path), native_dir, identity, archive)
            if prepared.get('model_calls') != 0: raise ValueError('preparer_made_a_call')
            case.update(ready=prepared.get('ready') is True, preparation_status=prepared.get('status'),
                        errors=prepared.get('errors', []), candidate_manifest_sha256=identity)
            durable(destination / 'preparation.json', prepared)
            if case['ready']:
                request = prepared['request']; raw = encoded(request)
                if (len(raw) > MAX_REQUEST or len(raw) != prepared['request_bytes']
                        or hashlib.sha256(raw).hexdigest() != prepared['request_sha256']):
                    raise ValueError('prepared_request_identity_invalid')
                (destination / 'request.json').write_bytes(raw)
                case['request'] = {'path': str((destination / 'request.json').relative_to(output)), **file_ref(destination / 'request.json')}
        except (OSError, ValueError, KeyError, TypeError) as error:
            reason = str(error) if type(error) is ValueError and re.fullmatch(r'[a-z_]+', str(error)) else type(error).__name__
            case.update(ready=False, preparation_status='invalid_input', errors=[reason])
            durable(destination / 'preparation.json', {'ready': False, 'status': 'invalid_input', 'errors': [reason], 'model_calls': 0})
        case['preparation_wall_s'] = time.monotonic() - case_began
        case['preparation'] = {'path': str((destination / 'preparation.json').relative_to(output)), **file_ref(destination / 'preparation.json')}
        cases.append(case)
        durable(output / 'preparing.json', {'status': 'preparing', 'finished_slots': len(cases), 'expected_slots': 12})
    registration = {'schema': 'openagents.jev-lifecycle.clause-review-registration.v1', 'status': 'prepared_before_calls',
        'created_at': utc(), 'freeze_sha256': expected_freeze,
        'runner': file_ref(Path(__file__)), 'runner_tests': file_ref(Path(__file__).with_name('test_run_coverage_review.py')),
        'original_plan': file_ref(plan_path), 'ended_panel': file_ref(panel_path),
        'schedule': [{k: r[k] for k in ('position', 'run_id', 'task_id', 'arm', 'repetition')} for r in schedule],
        'bound_evidence_files': bound_inputs, 'cases': cases, 'preparation_wall_s': time.monotonic() - began,
        'policy': {'max_calls': MAX_CALLS, 'cost_admission_stop_usd': COST_STOP, 'socket_timeout_s': SOCKET_TIMEOUT,
                   'outer_timeout_s': OUTER_TIMEOUT, 'request_bytes': MAX_REQUEST, 'retries': 0, 'repair': False},
        'model_calls': 0}
    durable(output / 'registration.json', registration)
    durable(output / 'preparing.json', {'status': 'prepared', 'finished_slots': 12, 'model_calls': 0})
    return registration


@contextmanager
def outer_deadline(seconds):
    def expired(_number, _frame): raise CallDeadline()
    previous = signal.signal(signal.SIGALRM, expired)
    timer = signal.setitimer(signal.ITIMER_REAL, seconds)
    began = time.monotonic()
    try:
        yield
    finally:
        signal.setitimer(signal.ITIMER_REAL, 0)
        signal.signal(signal.SIGALRM, previous)
        if timer[0] > 0: signal.setitimer(signal.ITIMER_REAL, max(.001, timer[0] - (time.monotonic() - began)), timer[1])


def known_cost(receipt):
    cost = receipt.get('cost_usd')
    valid = type(cost) in (int, float) and math.isfinite(cost) and cost >= 0
    if valid and receipt.get('cost_status') == 'gateway_reported': return cost
    if valid and cost == 0 and receipt.get('cost_status') == 'no_call' and receipt.get('attempts') == 0: return 0.0
    return None


def execute(harness, evidence, output, registration_sha256, *, bundle=HERE, expected_freeze=EXPECTED_FREEZE, call_fn=None):
    """Execute an existing registration once; never prepare, resume, or retry."""
    os.umask(0o077)
    harness, evidence, output, bundle = map(Path, (harness, evidence, output, bundle))
    verify_bundle(harness, bundle, expected_freeze)
    registration_path = output / 'registration.json'; registration = read(registration_path)
    if file_ref(registration_path)['sha256'] != registration_sha256:
        raise ValueError('The reviewed registration changed')
    if (registration.get('schema') != 'openagents.jev-lifecycle.clause-review-registration.v1'
            or registration.get('status') != 'prepared_before_calls' or registration['freeze_sha256'] != expected_freeze
            or registration['runner'] != file_ref(Path(__file__))
            or registration['runner_tests'] != file_ref(Path(__file__).with_name('test_run_coverage_review.py'))):
        raise ValueError('The prepared registration or runner changed')
    for path, ref in registration['bound_evidence_files'].items():
        if file_ref(evidence / relative(path)) != ref: raise ValueError('Bound evidence changed')
    for case in registration['cases']:
        for field in ('preparation', 'request'):
            if field in case:
                ref = case[field]
                if file_ref(output / relative(ref['path'])) != {k: ref[k] for k in ('sha256', 'bytes')}:
                    raise ValueError('A prepared review changed')
    original = read(evidence / 'plan/plan.json')
    if (len(registration['cases']) != 12 or registration['schedule'] !=
            [{k: e[k] for k in ('position', 'run_id', 'task_id', 'arm', 'repetition')} for e in original['schedule']]):
        raise ValueError('The case denominator or original schedule changed')
    for case, expected in zip(registration['cases'], registration['schedule']):
        if any(case.get(k) != v for k, v in expected.items()): raise ValueError('The prepared case order changed')
    with (output / 'live.claim').open('x') as handle:
        handle.write(json.dumps({'registration_sha256': file_ref(registration_path)['sha256'], 'claimed_at': utc()}))
        handle.flush(); os.fsync(handle.fileno())
    result = {'schema': 'openagents.jev-lifecycle.clause-review-execution.v1', 'status': 'running', 'started_at': utc(),
              'registration_sha256': file_ref(registration_path)['sha256'], 'slots': [], 'application_calls': 0,
              'gateway_attempts': 0, 'known_cost_usd': 0.0, 'cost_usd': 0.0, 'accounting_complete': True}
    began = time.monotonic()
    try:
        durable(output / 'execution.json', result)
        gateway = load(harness / 'bench/jev-lifecycle/gateway.py', '_clause_runner_gateway')
        call = call_fn or gateway.call
        (output / 'calls').mkdir(mode=0o700)
        for case in registration['cases']:
            row = {'position': case['position'], 'run_id': case['run_id'], 'status': 'skipped_preparation', 'cost_usd': 0.0}
            result['slots'].append(row)
            if not case['ready']: continue
            if result['application_calls'] >= MAX_CALLS or result['known_cost_usd'] >= COST_STOP:
                row['status'] = 'not_admitted_budget'; result['status'] = 'stopped_cost_admission'; break
            verify_bundle(harness, bundle, expected_freeze)
            ref = case['request']; request_path = output / relative(ref['path'])
            if file_ref(request_path) != {k: ref[k] for k in ('sha256', 'bytes')}: raise ValueError('Prepared request changed')
            request = read(request_path, MAX_REQUEST)
            if request.get('model') != gateway.MODEL: raise ValueError('Gateway model changed')
            gateway.validate_questions(request['questions'])
            directory = output / 'calls' / f"{case['position']:02d}"; directory.mkdir(mode=0o700)
            row.update(status='launch_intent', cost_usd=None, request_sha256=ref['sha256'])
            result['application_calls'] += 1
            durable(directory / 'launch-intent.json', dict(row, launched_at=utc()))
            durable(output / 'execution.json', result)
            receipt = {}; response = None; interrupted = None; call_began = time.monotonic()
            try:
                with outer_deadline(OUTER_TIMEOUT):
                    receipt, response = call(request['state'], request['questions'], directory / 'gateway', timeout=SOCKET_TIMEOUT)
                if not isinstance(receipt, dict): raise ValueError('Gateway receipt is not an object')
            except (Exception, CallDeadline, StopRequested, KeyboardInterrupt) as error:
                interrupted = type(error).__name__
                try: receipt = read(directory / 'gateway/receipt.json', 2 * 1024 * 1024)
                except (OSError, ValueError): receipt = {}
            row.update(wall_s=time.monotonic() - call_began, error_type=interrupted,
                       status='answered' if receipt.get('answers_valid') is True and response is not None else 'failed')
            cost = known_cost(receipt)
            row['cost_usd'] = cost
            if type(receipt.get('attempts')) is int and 0 <= receipt['attempts'] <= 1:
                row['gateway_attempts'] = receipt['attempts']
            else: row['gateway_attempts_unknown'] = True
            if (directory / 'gateway/receipt.json').is_file(): row['receipt'] = file_ref(directory / 'gateway/receipt.json')
            identity_valid = receipt.get('request_sha256') == ref['sha256'] and not row.get('gateway_attempts_unknown')
            if not identity_valid: row['request_identity_invalid'] = True
            if cost is None:
                result.update(accounting_complete=False, cost_usd=None, status='stopped_unknown_cost')
            else:
                result['known_cost_usd'] += cost; result['cost_usd'] = result['known_cost_usd']
            durable(directory / 'outcome.json', row)
            durable(output / 'execution.json', result)
            if cost is None: break
            if interrupted or not identity_valid:
                result['status'] = 'stopped_interrupted' if interrupted else 'stopped_identity'; break
        else:
            result['status'] = 'complete'
    except (Exception, CallDeadline, StopRequested, KeyboardInterrupt) as error:
        result.update(status='stopped_infrastructure', error_type=type(error).__name__)
    finally:
        # A signal can arrive after the gateway checkpoint but before accounting.
        # Recover that known charge without making another call or hiding uncertainty.
        for row in result['slots']:
            if 'request_sha256' in row and (row['cost_usd'] is None or 'gateway_attempts' not in row):
                try:
                    path = output / 'calls' / f"{row['position']:02d}" / 'gateway/receipt.json'
                    receipt = read(path, 2 * 1024 * 1024)
                    row.update(cost_usd=known_cost(receipt), receipt=file_ref(path))
                    if result['status'] == 'stopped_infrastructure': row['status'] = 'interrupted_retained'
                    if type(receipt.get('attempts')) is int and 0 <= receipt['attempts'] <= 1:
                        row['gateway_attempts'] = receipt['attempts']
                    else:
                        row['gateway_attempts_unknown'] = True
                    if receipt.get('request_sha256') != row.get('request_sha256'):
                        row['request_identity_invalid'] = True
                except (OSError, ValueError):
                    pass
        result['gateway_attempts'] = sum(row.get('gateway_attempts', 0) for row in result['slots'])
        result['gateway_attempts_complete'] = all(
            'gateway_attempts' in row for row in result['slots'] if 'request_sha256' in row)
        result['known_cost_usd'] = math.fsum(row['cost_usd'] for row in result['slots'] if row['cost_usd'] is not None)
        result['accounting_complete'] = all(row['cost_usd'] is not None for row in result['slots'])
        result['cost_usd'] = result['known_cost_usd'] if result['accounting_complete'] else None
        completed = {row['position'] for row in result['slots']}
        result['slots'].extend({'position': case['position'], 'run_id': case['run_id'],
            'status': 'not_called_after_stop' if case['ready'] else 'skipped_preparation', 'cost_usd': 0.0}
            for case in registration['cases'] if case['position'] not in completed)
        result.update(finished_at=utc(), wall_s=time.monotonic() - began)
        durable(output / 'execution.json', result)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('harness', 'repo', 'evidence', 'output'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--live', action='store_true', help='Execute an already prepared output exactly once')
    parser.add_argument('--registration-sha256', help='Required with --live; bind the reviewed prepared registration')
    args = parser.parse_args()
    if args.live and not args.registration_sha256: parser.error('--live requires --registration-sha256')
    def stop(_number, _frame): raise StopRequested()
    for number in (signal.SIGINT, signal.SIGTERM): signal.signal(number, stop)
    try:
        result = execute(args.harness, args.evidence, args.output, args.registration_sha256) if args.live else prepare_all(args.harness, args.repo, args.evidence, args.output)
        print(json.dumps({key: result.get(key) for key in ('status', 'model_calls', 'application_calls', 'accounting_complete', 'known_cost_usd', 'cost_usd')}))
        return 0 if result.get('status') in ('prepared_before_calls', 'complete') else 1
    except (Exception, CallDeadline, StopRequested, KeyboardInterrupt) as error:
        print(json.dumps({'status': 'refused', 'error_type': type(error).__name__}))
        return 1


if __name__ == '__main__': raise SystemExit(main())
