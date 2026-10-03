#!/usr/bin/env python3
"""Summarize registered native pilot attempts without executing any trial."""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import math
from pathlib import Path
import statistics
import sys
import tarfile
import uuid

LEGACY = Path(__file__).resolve().parent.parent / 'delegation-study'
sys.path.insert(0, str(LEGACY))
try:
    import candidate
    import trial
    spec = importlib.util.spec_from_file_location('_native_pilot_meter', LEGACY / 'report.py')
    meter = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(meter)
finally:
    sys.path.remove(str(LEGACY))

ARMS = ('bare', 'deterministic', 'jev')
TASKS = ('alternative-beta', 'alternative-gamma')
ORDER = [(TASKS[0], 1, ('bare', 'deterministic', 'jev')),
         (TASKS[1], 1, ('jev', 'bare', 'deterministic')),
         (TASKS[0], 2, ('deterministic', 'jev', 'bare')),
         (TASKS[1], 2, ('bare', 'deterministic', 'jev'))]
EXPECTED = [(task, rep, arm) for task, rep, arms in ORDER for arm in arms]


def number(value):
    return type(value) in (int, float) and math.isfinite(value) and value >= 0


def numeric(value):
    return value if number(value) else None


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def read(path):
    path = Path(path)
    if path.is_symlink() or not path.is_file() or path.stat().st_size > 32 * 1024 * 1024:
        raise ValueError('Invalid bounded JSON artifact')
    value = json.loads(path.read_bytes())
    if not isinstance(value, dict):
        raise ValueError('Artifact is not an object')
    return value


def object_value(value):
    return value if isinstance(value, dict) else {}


def total(values):
    return math.fsum(values) if values and all(number(v) for v in values) else None


def average(values):
    return statistics.mean(values) if values and all(number(v) for v in values) else None


def ratio(first, second):
    return first / second if number(first) and number(second) and second > 0 else None


def optional(path, errors, label):
    if not path.exists():
        return {}
    try:
        return read(path)
    except (OSError, ValueError, TypeError):
        errors.append(label + '_unreadable')
        return {}


def check_plan(plan):
    if plan.get('schema') != 'openagents.jev-lifecycle.native-pilot-plan.v1' or plan.get('status') != 'frozen_before_execution':
        raise ValueError('Expected a frozen native pilot plan')
    schedule = plan.get('schedule', [])
    if (not isinstance(schedule, list) or len(schedule) != 12 or not all(isinstance(x, dict) for x in schedule)
            or [(x.get('task_id'), x.get('repetition'), x.get('arm')) for x in schedule] != EXPECTED):
        raise ValueError('The plan differs from the registered twelve-attempt order')
    ids = [r.get('run_id') for r in schedule]
    if not all(isinstance(v, str) for v in ids) or len(set(ids)) != 12:
        raise ValueError('The plan has duplicate or invalid run identities')
    try:
        if any(str(uuid.UUID(v)) != v for v in ids): raise ValueError('Invalid UUID')
    except (ValueError, AttributeError):
        raise ValueError('The plan has duplicate or invalid run identities') from None
    if [r.get('position') for r in schedule] != list(range(1, 13)):
        raise ValueError('The plan has invalid positions')
    if not isinstance(plan.get('prices'), dict) or not plan['prices']:
        raise ValueError('The plan lacks frozen prices')


def bound_result(path, row, label, errors):
    value = optional(path, errors, label)
    if value:
        ref = object_value(object_value(row.get('artifacts')).get(label))
        if ref.get('sha256') != sha(path):
            errors.append(label + '_artifact_unbound')
    return value


def provider_cost(path, run_id, prices, errors):
    """Keep other valid calls if an admitted row has a malformed path field."""
    try:
        if path.is_symlink() or path.stat().st_size > 32 * 1024 * 1024: raise ValueError('ledger bound')
        raw = path.read_text()
        lines = []
        for line in raw.splitlines(keepends=True):
            try:
                item = json.loads(line)
                if isinstance(item, dict) and item.get('phase') == 'admitted' and not isinstance(item.get('path', ''), str):
                    item['path'] = ''  # The meter rejects this call and retains unrelated known costs.
                    line = json.dumps(item) + ('\n' if line.endswith('\n') else '')
                    errors.append('provider_path_invalid')
            except (ValueError, TypeError):
                pass  # The meter retains malformed-row errors itself.
            lines.append(line)
        return meter.provider_cost(''.join(lines), run_id, list(prices), prices)
    except (OSError, ValueError, TypeError, KeyError, AttributeError):
        errors.append('provider_ledger_unreadable')
        return None


def read_attempt(entry, plan, runs, configs):
    identity = {k: entry[k] for k in ('position', 'run_id', 'task_id', 'repetition', 'arm')}
    directory = runs / entry['run_id']
    errors = []
    config_path = configs / (entry['run_id'] + '.json') if configs else Path(entry['config']['path'])
    config = optional(config_path, errors, 'config')
    config_valid = bool(config) and sha(config_path) == entry['config'].get('sha256')
    if (not config_valid or any(config.get(k) != entry[k] for k in ('run_id', 'task_id', 'arm'))
            or config.get('module_hashes') != plan.get('module_hashes')):
        errors.append('config_binding_invalid'); config_valid = False
    intent = optional(directory / 'launch-intent.json', errors, 'launch_intent')
    final_exists = (directory / 'pilot.json').exists()
    receipt = optional(directory / 'pilot.json', errors, 'pilot')
    through = optional(directory / 'through-checks.json', errors, 'through_checks')
    row = receipt or through or intent
    if receipt and receipt.get('schema') != 'openagents.jev-lifecycle.native-pilot.v1':
        errors.append('pilot_schema_invalid')
    launched = bool(row) or directory.exists()
    if row and (any(row.get(k) != entry[k] for k in ('run_id', 'task_id', 'arm'))
                or row.get('config_sha256') != entry['config'].get('sha256')):
        errors.append('run_binding_invalid')
    native = bound_result(directory / 'native/result.json', row, 'native', errors)
    checks = bound_result(directory / 'acceptance/checks.json', row, 'checks', errors)
    nt = object_value(config.get('native_template')); ct = object_value(config.get('acceptance_template'))
    if nt.get('model') != 'claude-sonnet-5-5' or nt.get('effort') != 'medium':
        errors.append('registered_native_model_or_effort_invalid'); config_valid = False
    candidate_id = row.get('candidate_manifest_sha256')
    source = row.get('source_commit') or native.get('source_commit')
    native_valid = False
    if native:
        bindings = {'run_id': entry['run_id'], 'source_commit': source,
                    'source_archive_sha256': nt.get('source_archive_sha256'),
                    'cli_sha256': nt.get('binary_sha256'), 'cli_hash_after': nt.get('binary_sha256'),
                    'cli_version': nt.get('binary_version'), 'model': nt.get('model'), 'effort': nt.get('effort'),
                    'target_seed_manifest_sha256': nt.get('target_seed_manifest_sha256'),
                    'candidate_manifest_sha256': candidate_id}
        served = native.get('served_models')
        native_valid = (native.get('schema') == 'openagents.delegation.native-attempt.v1'
                        and all(v is not None and native.get(k) == v for k, v in bindings.items()))
        native_valid &= isinstance(served, list) and all(isinstance(m, str) and m in plan['prices'] for m in served)
        if native.get('model_completed') is True and (not isinstance(served, list) or nt.get('model') not in served):
            native_valid = False
        if native.get('cargo_features', []) != nt.get('cargo_features', []): native_valid = False
        argv_config = dict(nt, tools=None)
        if entry['arm'] != 'bare':
            argv_config.update(system_file='bound-lean-system', tools='Bash,Read,Edit,Write,Glob,Grep')
        try:
            argv = native.get('argv')
            if not isinstance(argv, list) or argv[1:] != trial.native_arguments(argv_config): native_valid = False
        except (ValueError, KeyError, TypeError):
            native_valid = False
        prompt_path = directory / 'preparation/prompt.txt'
        try:
            if prompt_path.is_symlink() or prompt_path.stat().st_size > 2 * 1024 * 1024: raise ValueError('prompt bound')
            prompt = prompt_path.read_bytes()
            delivered = ('Benchmark run ID: ' + entry['run_id'] + '\n\n').encode() + prompt
            if (native.get('prompt_sha256') != hashlib.sha256(prompt).hexdigest()
                    or native.get('delivered_prompt_sha256') != hashlib.sha256(delivered).hexdigest()):
                native_valid = False
        except (OSError, ValueError):
            native_valid = False
        if not native_valid: errors.append('native_identity_invalid')
    checks_valid = False
    if checks:
        bindings = {'run_id': entry['run_id'], 'source_commit': source,
                    'source_archive_sha256': nt.get('source_archive_sha256'),
                    'candidate_manifest_sha256': candidate_id,
                    'checker_sha256': object_value(ct.get('checker')).get('sha256')}
        checks_valid = (checks.get('schema') == 'openagents.delegation.final-checks.v1'
                        and all(v is not None and checks.get(k) == v for k, v in bindings.items())
                        and checks.get('cargo_features', []) == ct.get('cargo_features', []))
        if not checks_valid: errors.append('checks_identity_invalid')
    candidate_valid = False
    if candidate_id is not None:
        try:
            candidate.validate(directory / 'native', candidate_id, source, nt.get('source_archive_sha256'))
            candidate_valid = True
        except (OSError, ValueError, KeyError, TypeError, AttributeError, EOFError, tarfile.TarError):
            errors.append('candidate_invalid')
    # Recompute provider requests once. CLI totals are reconciliation evidence,
    # never an additional charge. Keep valid request costs when other rows fail.
    ledger = directory / 'native/provider-calls.jsonl'
    provider = None
    if ledger.exists():
        provider = provider_cost(ledger, entry['run_id'], plan['prices'], errors)
    native_started = bool(native) or ledger.exists() or (directory / 'native-config.private.json').exists() or 'native' in object_value(row.get('phases'))
    native_low = provider['lower_usd'] if provider else 0.0
    native_high = provider['upper_usd'] if provider and native.get('execution_closed') is True else 0.0 if launched and not native_started and row.get('execution_closed') is True else None
    cli_cost = numeric(native.get('cost_usd'))
    if cli_cost is not None and native_high is not None and cli_cost > native_high + 1e-9:
        errors.append('cli_exceeds_provider_bound'); native_high = None
    gateway_path = directory / 'preparation/gateway-call/receipt.json'
    gateway = optional(gateway_path, errors, 'gateway')
    jev_low = 0.0
    jev_high = 0.0 if entry['arm'] != 'jev' else None
    if gateway:
        if number(gateway.get('cost_usd')):
            jev_low = gateway['cost_usd']
            no_call = gateway.get('cost_status') == 'no_call' and gateway.get('attempts') == 0 and jev_low == 0
            jev_high = jev_low if gateway.get('cost_status') == 'gateway_reported' or no_call else None
        if entry['arm'] != 'jev': errors.append('unexpected_gateway_charge')
    elif entry['arm'] == 'jev' and launched and row.get('failed_stage') == 'validation' and row.get('execution_closed') is True:
        jev_high = 0.0
    lower = native_low + jev_low
    reported_low = numeric(row.get('cost_lower_usd'))
    if reported_low is not None and reported_low > lower + 1e-9:
        # A missing sidecar must not erase an already retained known charge.
        lower = reported_low; errors.append('known_cost_retained_without_ledger_confirmation')
    upper = native_high + jev_high if native_high is not None and jev_high is not None else None
    if upper is not None and upper < lower - 1e-9:
        upper = None; errors.append('cost_bounds_disagree')
    complete_accounting = upper is not None and row.get('accounting_complete') is True
    point = lower if complete_accounting and abs(upper - lower) < 1e-9 else None
    if provider and (provider['errors'] or provider['unknown_calls']): errors.append('provider_accounting_incomplete')
    cleanup = row.get('cleanup')
    cleanup_ok = isinstance(cleanup, list) and all(isinstance(r, dict) and r.get('removed') is True for r in cleanup)
    released = object_value(row.get('native_scratch_release'))
    if released and released.get('status') != 'complete': cleanup_ok = False
    closed = row.get('execution_closed') is True
    observed_wall = numeric(row.get('endpoint_wall_s'))
    checks_wall = numeric(row.get('checks_endpoint_wall_s'))
    final_wall = observed_wall if receipt and closed and cleanup_ok and observed_wall is not None and (checks_wall is None or observed_wall >= checks_wall) else None
    outcomes = {}
    for name in ('scope', 'format', 'ordinary', 'independent'):
        value = object_value(checks.get(name)).get('passed')
        outcomes[name] = value if type(value) is bool else None
        if checks and type(value) is not bool: errors.append('check_outcome_invalid')
    if checks and released.get('status') != 'complete':
        cleanup_ok = False; final_wall = None; errors.append('native_scratch_release_unconfirmed')
    quality_valid = config_valid and native_valid and checks_valid and candidate_valid and not errors
    accepted = bool(quality_valid and receipt and row.get('status') == 'complete' and closed
                    and native.get('execution_closed') is True and checks.get('execution_closed') is True
                    and checks.get('completed') is True and all(v is True for v in outcomes.values())
                    and complete_accounting)
    if row.get('accepted') is True and not accepted: errors.append('reported_acceptance_not_verified')
    stages = {}
    for name, phase in object_value(row.get('phases')).items():
        if name in ('preparation', 'native', 'acceptance', 'final_cleanup'):
            stages[name] = numeric(object_value(phase).get('wall_s'))
    stages['native_scratch_release'] = numeric(released.get('wall_s'))
    stages['native_retained'] = numeric(native.get('total_retained_wall_s'))
    prep = object_value(row.get('preparation'))
    stages['preparation_reported'] = numeric(prep.get('wall_s'))
    stages['gateway_call'] = numeric(gateway.get('wall_s'))
    telemetry_errors = []
    for name, filename in [('catalog', 'catalog.json'), ('materialization', 'pack.json')]:
        value = optional(directory / 'preparation' / filename, telemetry_errors, name)
        stages[name] = numeric(value.get('wall_s'))
    check_phases = {name: {k: numeric(v.get(k)) for k in ('wall_s', 'elapsed_s') if k in v}
                    for name in ('scope', 'format', 'ordinary', 'independent') if isinstance((v := checks.get(name)), dict)}
    for name, phase in check_phases.items():
        for part in ('compile', 'test'):
            value = object_value(object_value(checks.get(name)).get(part))
            if value: phase[part + '_wall_s'] = numeric(value.get('wall_s'))
    setup_times = {k: numeric(v) for k, v in object_value(checks.get('phases')).items()
                   if k in ('seed_validation_s', 'export_s', 'git_snapshot_s', 'apply_candidate_s', 'seed_copy_s', 'setup_s')}
    native_phases = {k: numeric(v) for k, v in object_value(native.get('phases')).items()
                     if k in ('export_s', 'git_snapshot_s', 'target_seed_copy_s', 'prepare_s', 'executor_s', 'capture_s', 'provider_drain_s')}
    return {**identity, 'launched': launched, 'final_receipt_present': bool(receipt), 'final_receipt_file_present': final_exists,
            'status': row.get('status') if row.get('status') in ('complete', 'failed', 'incomplete') else 'unstarted' if not launched else 'unknown',
            'reported_accepted': row.get('accepted') is True, 'accepted': accepted,
            'model_completed': native.get('model_completed') is True and native_valid and config_valid and 'native_artifact_unbound' not in errors and 'run_binding_invalid' not in errors,
            'execution_closed': closed, 'cleanup_confirmed': cleanup_ok, 'safe_to_continue': row.get('safe_to_continue') is True,
            'accounting_complete': complete_accounting, 'cost_lower_usd': lower, 'cost_upper_usd': upper if complete_accounting else None,
            'cost_usd': point, 'native_cost_lower_usd': native_low, 'native_cost_upper_usd': native_high,
            'jev_cost_lower_usd': jev_low, 'jev_cost_upper_usd': jev_high, 'cli_reported_cost_usd': cli_cost,
            'provider_calls': provider['admitted_calls'] if provider else None,
            'unknown_provider_calls': provider['unknown_calls'] if provider else None,
            'served_models': provider['served_models'] if provider else [],
            'provider_usage': provider['usage'] if provider else {},
            'checks_endpoint_wall_s': checks_wall, 'observed_endpoint_wall_s': observed_wall, 'endpoint_wall_s': final_wall,
            'phase_wall_s': stages, 'native_phases_s': native_phases,
            'check_phases': check_phases, 'check_setup_s': setup_times, 'check_outcomes': outcomes,
            'failed_stage': row.get('failed_stage') if row.get('failed_stage') in ('validation', 'preparation', 'native', 'native_scratch_release', 'acceptance') else None,
            'artifact_errors': sorted(set(errors)), 'telemetry_errors': sorted(set(telemetry_errors)),
            'pilot_sha256': sha(directory / 'pilot.json') if receipt else None}


def summarize(rows):
    accepted = sum(r['accepted'] for r in rows)
    low = math.fsum(r['cost_lower_usd'] for r in rows)
    point = total([r['cost_usd'] for r in rows])
    upper = total([r['cost_upper_usd'] for r in rows])
    return {'assigned': len(rows), 'launched': sum(r['launched'] for r in rows),
            'final_receipts': sum(r['final_receipt_present'] for r in rows), 'accepted': accepted,
            'model_completed': sum(r['model_completed'] for r in rows),
            'unstarted': sum(not r['launched'] for r in rows),
            'artifact_error_attempts': sum(bool(r['artifact_errors']) for r in rows),
            'failed_or_incomplete_receipts': sum(r['status'] in ('failed', 'incomplete', 'unknown') for r in rows),
            'known_cost_attempts': sum(r['cost_usd'] is not None for r in rows),
            'primary_time_attempts': sum(r['endpoint_wall_s'] is not None for r in rows),
            'cost_lower_usd': low, 'cost_upper_usd': upper, 'cost_usd': point,
            'cost_per_accepted_usd': point / accepted if point is not None and accepted else None,
            'cost_per_accepted_lower_usd': low / accepted if accepted else None,
            'cost_per_accepted_upper_usd': upper / accepted if upper is not None and accepted else None,
            'mean_cost_usd': average([r['cost_usd'] for r in rows]),
            'mean_endpoint_wall_s': average([r['endpoint_wall_s'] for r in rows]),
            'observed_mean_cost_usd': average([r['cost_usd'] for r in rows if r['cost_usd'] is not None]),
            'observed_mean_endpoint_wall_s': average([r['endpoint_wall_s'] for r in rows if r['endpoint_wall_s'] is not None]),
            'phase_totals_s': {name: total([r['phase_wall_s'].get(name) for r in rows]) for name in ('preparation', 'native', 'acceptance', 'native_scratch_release', 'final_cleanup')}}


def comparison(first, second, rows):
    cells = {(r['task_id'], r['repetition'], r['arm']): r for r in rows}
    pairs, tasks = [], []
    for task in TASKS:
        arm_rows = {arm: [cells[(task, rep, arm)] for rep in (1, 2)] for arm in (first, second)}
        tasks.append({'task_id': task, 'means': {arm: {'cost_usd': average([r['cost_usd'] for r in rs]), 'endpoint_wall_s': average([r['endpoint_wall_s'] for r in rs])} for arm, rs in arm_rows.items()}})
        for rep in (1, 2):
            a, b = cells[(task, rep, first)], cells[(task, rep, second)]
            pair = {'task_id': task, 'repetition': rep}
            for label, key in (('cost', 'cost_usd'), ('time', 'endpoint_wall_s')):
                left, right = a[key], b[key]
                pair[label + '_ratio'] = ratio(left, right)
                pair[label + '_difference'] = left - right if number(left) and number(right) else None
            pair['lower_cost_and_time'] = (pair['cost_difference'] < 0 and pair['time_difference'] < 0) if pair['cost_difference'] is not None and pair['time_difference'] is not None else None
            pairs.append(pair)
    ratios = {label: ratio(total([t['means'][first][key] for t in tasks]), total([t['means'][second][key] for t in tasks])) for label, key in (('cost', 'cost_usd'), ('time', 'endpoint_wall_s'))}
    return {'treatment': first, 'comparator': second, 'per_task': tasks, 'matched_pairs': pairs,
            'aggregate_ratio_of_summed_task_means': ratios,
            'evaluable_pairs': sum(p['lower_cost_and_time'] is not None for p in pairs),
            'lower_cost_and_time_pairs': sum(p['lower_cost_and_time'] is True for p in pairs)}


def unregistered_attempt(directory, prices):
    """Retain discovered charges without treating extra attempts as replacements."""
    errors = []
    row = optional(directory / 'pilot.json', errors, 'unregistered_pilot')
    through = optional(directory / 'through-checks.json', errors, 'unregistered_checks')
    intent = optional(directory / 'launch-intent.json', errors, 'unregistered_intent')
    observed = row or through or intent
    run_id = observed.get('run_id', directory.name)
    ledger = directory / 'native/provider-calls.jsonl'
    provider = provider_cost(ledger, run_id, prices, errors) if ledger.exists() else None
    gateway = optional(directory / 'preparation/gateway-call/receipt.json', errors, 'unregistered_gateway')
    recovered = (provider['lower_usd'] if provider else 0.0) + (numeric(gateway.get('cost_usd')) or 0.0)
    reported = numeric(observed.get('cost_lower_usd'))
    return {'directory_name_sha256': hashlib.sha256(directory.name.encode()).hexdigest(),
            'cost_lower_usd': max(recovered, reported or 0.0), 'cost_upper_usd': None,
            'reported_cost_lower_usd': reported, 'reported_cost_upper_usd': numeric(observed.get('cost_upper_usd')),
            'artifact_errors': sorted(set(errors + ['unregistered_attempt']))}


def panel_receipt(path, plan, plan_digest, rows):
    errors = []
    value = optional(Path(path), errors, 'panel') if path else {}
    attempts = value.get('attempts', [])
    if not value:
        errors.append('panel_receipt_missing')
    elif (value.get('schema') != 'openagents.jev-lifecycle.native-panel.v1'
            or value.get('plan_sha256') != plan_digest
            or value.get('driver_sha256') != object_value(plan.get('module_hashes')).get('jev-lifecycle/run_native_panel.py')
            or value.get('probe_sha256') != object_value(plan.get('capability_probe')).get('sha256')):
        errors.append('panel_binding_invalid')
    if not isinstance(attempts, list) or len(attempts) > 12 or not all(isinstance(a, dict) for a in attempts):
        errors.append('panel_attempts_invalid'); attempts = []
    for actual, expected, row in zip(attempts, plan['schedule'], rows):
        if (any(actual.get(k) != expected.get(k) for k in ('run_id', 'position', 'task_id', 'arm', 'repetition'))
                or object_value(actual.get('config')).get('sha256') != expected['config']['sha256']):
            errors.append('panel_attempt_order_invalid')
        observed_digest = actual.get('receipt_sha256', actual.get('observed_receipt_sha256'))
        if observed_digest is not None and observed_digest != row['pilot_sha256']:
            errors.append('panel_pilot_digest_mismatch')
        if value.get('status') == 'complete' and observed_digest is None:
            errors.append('panel_pilot_digest_missing')
    if any(r['launched'] for r in rows[len(attempts):]):
        errors.append('attempt_outside_panel_sequence')
    status = value.get('status')
    statuses = ('running', 'complete', 'stopped_cost_admission', 'stopped_unknown_cost', 'stopped_incomplete_attempt', 'stopped_infrastructure')
    return {'sha256': sha(path) if value else None, 'status': status if status in statuses else None,
            'recorded_attempts': len(attempts), 'complete_valid': not errors and status == 'complete' and len(attempts) == 12,
            'known_prior_cost_usd': numeric(value.get('known_prior_cost_usd')), 'artifact_errors': sorted(set(errors))}


def build(plan_path, run_root, config_root=None, panel_path=None):
    plan_path, runs = Path(plan_path), Path(run_root)
    plan = read(plan_path); check_plan(plan)
    configs = Path(config_root) if config_root else None
    rows = [read_attempt(e, plan, runs, configs) for e in plan['schedule']]
    known_ids = {r['run_id'] for r in rows}
    extra = []
    for path in sorted(runs.iterdir()) if runs.exists() else []:
        artifacts = ('pilot.json', 'through-checks.json', 'launch-intent.json', 'native/provider-calls.jsonl', 'preparation/gateway-call/receipt.json')
        if path.is_dir() and path.name not in known_ids and any((path / name).exists() for name in artifacts):
            extra.append(unregistered_attempt(path, plan['prices']))
    arms = {arm: summarize([r for r in rows if r['arm'] == arm]) for arm in ARMS}
    panel = panel_receipt(panel_path, plan, sha(plan_path), rows)
    comparisons = {a + '/' + b: comparison(a, b, rows) for a, b in [('jev', 'bare'), ('jev', 'deterministic'), ('deterministic', 'bare')]}
    complete = all(r['final_receipt_present'] and r['accounting_complete'] and r['cost_usd'] is not None and r['endpoint_wall_s'] is not None and not r['artifact_errors'] for r in rows) and not extra and panel['complete_valid']
    gates = {'complete_valid_panel': complete, 'jev_accepts_all_four': arms['jev']['accepted'] == 4,
             'jev_accepts_no_fewer': all(arms['jev']['accepted'] >= arms[a]['accepted'] for a in ('bare', 'deterministic'))}
    for comparator in ('bare', 'deterministic'):
        value = comparisons['jev/' + comparator]
        for metric, r in value['aggregate_ratio_of_summed_task_means'].items():
            gates[f'{metric}_reduction_10pct_vs_{comparator}'] = r <= .9 if r is not None else None
        gates['three_of_four_joint_pairs_vs_' + comparator] = value['lower_cost_and_time_pairs'] >= 3 if value['evaluable_pairs'] == 4 else None
    return {'schema': 'openagents.jev-lifecycle.native-pilot-report.v1', 'plan_sha256': sha(plan_path),
            'status': 'complete' if complete else 'partial' if any(not r['final_receipt_present'] for r in rows) else 'incomplete_or_invalid', 'scheduled_attempts': 12,
            'rows': rows, 'arms': arms, 'per_task': {task: {arm: summarize([r for r in rows if r['task_id'] == task and r['arm'] == arm]) for arm in ARMS} for task in TASKS},
            'comparisons': comparisons, 'directional_win_gates': gates,
            'directional_pilot_win': all(v is True for v in gates.values()) if complete else None,
            'panel_receipt': panel, 'unregistered_attempts': extra,
            'panel_known_cost_lower_usd': max(math.fsum(r['cost_lower_usd'] for r in rows), panel['known_prior_cost_usd'] or 0.0) + math.fsum(x['cost_lower_usd'] for x in extra),
            'setup_and_capability_costs_included': False,
            'limits': ['Exposed development tasks; no general superiority claim.', 'Provider costs are priced usage estimates plus gateway-reported Jev cost, not subscription invoices.', 'CLI totals are retained for reconciliation and never added as a second charge.', 'Primary means require every registered task repetition; observed partial means are labeled separately.', 'Checker behavior is not rerun; retained candidate identities and completed check receipts are validated. Source export and public task identity rely on the frozen runner receipts bound to the registered archive.', 'A complete valid panel requires the serial driver receipt and its ordered attempt digests.', 'Initial setup, capability probing, machine and engineering costs are separate.']}


def markdown(report):
    def show(value): return 'unknown' if value is None else f'{value:.6f}'
    lines = ['# Native pilot results', '', f"Panel status: **{report['status']}**. Twelve attempts are assigned; unrun or unreadable attempts remain in the denominator.", '', '| Arm | Accepted / assigned | Launched | Native completed | Known cost attempts | Total cost USD | Mean primary seconds |', '| --- | ---: | ---: | ---: | ---: | ---: | ---: |']
    for arm, value in report['arms'].items():
        lines.append(f"| {arm} | {value['accepted']}/{value['assigned']} | {value['launched']} | {value['model_completed']} | {value['known_cost_attempts']} | {show(value['cost_usd'])} | {show(value['mean_endpoint_wall_s'])} |")
    lines += ['', 'Primary time includes confirmed scratch cleanup. Failed-attempt costs remain included. Detailed task means, four matched pairs, stage timings, accounting bounds, artifact errors, and every win gate are in the JSON report.', '', '## Directional win gates', '']
    lines += [f"- `{name}`: {'unknown' if value is None else str(value).lower()}" for name, value in report['directional_win_gates'].items()]
    lines += ['', 'This exposed-task pilot does not establish general coding reliability. Setup and capability costs are separate.']
    return '\n'.join(lines) + '\n'


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--plan', type=Path, required=True)
    parser.add_argument('--runs', type=Path, required=True)
    parser.add_argument('--configs', type=Path)
    parser.add_argument('--panel', type=Path, help='Serial driver panel.json; required for a complete valid panel')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    result = build(args.plan, args.runs, args.configs, args.panel)
    args.output.mkdir(parents=True, exist_ok=False)
    (args.output / 'report.json').write_text(json.dumps(result, indent=2) + '\n')
    (args.output / 'report.md').write_text(markdown(result))
    print(json.dumps({'status': result['status'], 'launched': sum(v['launched'] for v in result['arms'].values()), 'directional_pilot_win': result['directional_pilot_win']}))
