#!/usr/bin/env python3
"""Run one frozen twelve-attempt pilot serially, stopping on unknown effects."""
from __future__ import annotations
import argparse
import fcntl
import json
import os
from pathlib import Path
import signal
import sys

import native_pilot as pilot

EXPECTED = [(task, rep, arm) for task, rep, arms in [
    ('alternative-beta', 1, ('bare', 'deterministic', 'jev')),
    ('alternative-gamma', 1, ('jev', 'bare', 'deterministic')),
    ('alternative-beta', 2, ('deterministic', 'jev', 'bare')),
    ('alternative-gamma', 2, ('bare', 'deterministic', 'jev')),
] for arm in arms]


def validate_plan(plan):
    schedule = plan['schedule']
    if (plan['status'] != 'frozen_before_execution' or
        [(r['task_id'], r['repetition'], r['arm']) for r in schedule] != EXPECTED or
        [r['position'] for r in schedule] != list(range(1, 13)) or
        len({r['run_id'] for r in schedule}) != 12 or
        len({r['output'] for r in schedule}) != 12):
        raise ValueError('The frozen schedule is invalid')
    if plan['policy']['panel_observed_cost_admission_stop_usd'] != 24:
        raise ValueError('The panel admission stop changed')
    for row in schedule:
        config = json.loads(pilot.bound_file(row['config']))
        pilot.validate(config)
        if any(config[k] != row[k] for k in ('run_id', 'task_id', 'arm')):
            raise ValueError('The scheduled configuration changed identity')
        if Path(row['output']).exists():
            raise ValueError('An attempt already exists; this driver never resumes or retries it')
    return schedule


def run(plan_path, output, master, probe_path):
    os.umask(0o077)
    plan_path, output = Path(plan_path).resolve(), Path(output).resolve()
    plan = pilot.read_json(plan_path)
    schedule = validate_plan(plan)
    if plan.get('capability_probe') != {'path': str(Path(probe_path).resolve()), 'sha256': pilot.sha(Path(probe_path).read_bytes())}:
        raise ValueError('The capability probe differs from the frozen plan')
    probe = pilot.read_json(probe_path)
    if not (probe.get('model_completed') is True and probe.get('execution_closed') is True
            and probe.get('provider_accounting_complete') is True
            and probe.get('model') == 'claude-sonnet-5-5'
            and 'claude-sonnet-5-5' in probe.get('served_models', [])):
        raise ValueError('The required Sonnet capability probe is incomplete')
    output.mkdir(mode=0o700, exist_ok=False)
    lock = (output.parent / '.native-panel.lock').open('a')
    fcntl.flock(lock.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
    row = {'schema': 'openagents.jev-lifecycle.native-panel.v1', 'status': 'running',
           'started_at': pilot.utc(), 'plan_sha256': pilot.sha(plan_path.read_bytes()),
           'driver_sha256': pilot.sha(Path(__file__).read_bytes()),
           'probe_sha256': pilot.sha(Path(probe_path).read_bytes()),
           'cost_upper_usd': 0.0, 'known_prior_cost_usd': 0.0, 'attempts': [], 'scheduled_attempts': 12}
    pilot.trial.durable(output / 'panel.json', row)
    try:
        for scheduled in schedule:
            if row['cost_upper_usd'] >= 24:
                row['status'] = 'stopped_cost_admission'; break
            config = json.loads(pilot.bound_file(scheduled['config']))
            pilot.verify_modules(config)
            credential = Path(config['credential_file'])
            if credential.exists():
                raise ValueError('An unconsumed credential remains from an earlier launch')
            # The real bearer stays outside every executor mount. The broker consumes this copy.
            key = pilot.read_key(master)
            with open(credential, 'x', opener=lambda path, flags: os.open(path, flags, 0o600)) as handle:
                handle.write(key)
            key = None
            intent = dict(scheduled, launched_at=pilot.utc())
            row['attempts'].append(intent)
            pilot.trial.durable(output / 'panel.json', row)
            result = pilot.trial.process_phase('attempt-' + str(scheduled['position']),
                [sys.executable, str(pilot.HERE / 'native_pilot.py'),
                 '--config', scheduled['config']['path'], '--output', scheduled['output']], output, 1700)
            receipt_path = Path(scheduled['output']) / 'pilot.json'
            receipt = pilot.read_json(receipt_path)
            intent.update(observed_receipt_sha256=pilot.sha(receipt_path.read_bytes()),
                          exit_code=result['exit_code'], timed_out=result['timed_out'], accepted=receipt.get('accepted'),
                          safe_to_continue=receipt.get('safe_to_continue'),
                          cost_upper_usd=receipt.get('cost_upper_usd'))
            if receipt.get('run_id') != scheduled['run_id']:
                raise ValueError('The attempt receipt names another run')
            if receipt.get('accounting_complete') is not True or not pilot.finite(receipt.get('cost_upper_usd')):
                row['status'] = 'stopped_unknown_cost'; row['cost_upper_usd'] = None; break
            intent['receipt_sha256'] = pilot.sha(receipt_path.read_bytes())
            row['cost_upper_usd'] += receipt['cost_upper_usd']
            row['known_prior_cost_usd'] = row['cost_upper_usd']
            if result['timed_out'] or result['exit_code'] != 0 or receipt.get('safe_to_continue') is not True:
                row['status'] = 'stopped_incomplete_attempt'; break
            pilot.trial.durable(output / 'panel.json', row)
        else:
            row['status'] = 'complete'
    except BaseException as error:
        row.update(status='stopped_infrastructure', error_type=type(error).__name__)
        # An interrupted launch can have unobserved charges; never infer zero.
        if row['attempts'] and 'receipt_sha256' not in row['attempts'][-1]:
            row['cost_upper_usd'] = None
    finally:
        row['finished_at'] = pilot.utc()
        pilot.trial.durable(output / 'panel.json', row)
        lock.close()
    return row


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('plan', 'output', 'credential-master', 'probe-result'):
        parser.add_argument('--' + name, type=Path, required=True)
    args = parser.parse_args()
    def interrupted(number, frame):
        raise InterruptedError('panel_interrupted')
    for number in (signal.SIGTERM, signal.SIGINT):
        signal.signal(number, interrupted)
    result = run(args.plan, args.output, args.credential_master, args.probe_result)
    print(json.dumps(result))
    raise SystemExit(0 if result['status'] == 'complete' else 1)


if __name__ == '__main__':
    main()
