#!/usr/bin/env python3
"""Seal the complete archive cohort without opening official outcomes."""
import argparse
from collections import Counter
import json
from pathlib import Path

from archive_preflight import TASKS
from prepare import sha
from reproduce import tree, write


def population(rows, tasks=TASKS):
    keys = [(r['job'], r['trial']) for r in rows]
    if len(set(keys)) != len(keys):
        raise ValueError('Duplicate candidate identity')
    expected = Counter({(task, arm): 3 for task in tasks for arm in ('luna', 'astra')})
    if Counter((r['task'], r['executor']) for r in rows) != expected:
        raise ValueError('The seal requires every declared attempt, including unavailable candidates')


def baseline(composition):
    checks = (composition.get('final_checks') or {}).get('verdicts', {})
    final = ('fail' if checks.get('failed', 0) else
             'pass' if checks.get('passed', 0) and not checks.get('inconclusive', 0) else None)
    combined = ((composition.get('verdict') or {}).get('final') or {}).get('call')
    return {'checks.final': final,
            'verdict.combined': combined if combined in ('pass', 'fail') else None}


def prediction(row, checks, jobs):
    record = checks / row['trial']
    combined = json.loads((record / 'combined.json').read_text())
    if any(combined[k] != row[k] for k in ('job', 'trial', 'task')):
        raise ValueError('Check attribution differs from the declared candidate')
    contract_fail = combined['contract_call'] == 'fail'
    review_fail = combined['reproduced_call'] == 'fail'
    if (combined['call'] == 'fail') != (contract_fail or review_fail):
        raise ValueError('Combined call does not follow the frozen OR rule')
    if contract_fail:
        contract = json.loads((record / 'contract.json').read_text())
        if not any(i['outcome'] == 'differed' for i in contract['items']):
            raise ValueError('File failure has no executed difference')
        if combined['reproduced_call'] != 'not_requested':
            raise ValueError('The cheap arm must skip an unnecessary review')
    elif (record / 'reproduced/process.json').exists():
        process = json.loads((record / 'reproduced/process.json').read_text())
        review = record / 'reproduced/review.json'
        if review.exists() and sha(review) != process.get('review_sha256'):
            raise ValueError('Reproduced review identity changed')
        if review_fail and (process.get('error') or process.get('call') != 'fail'):
            raise ValueError('Failed or unavailable review cannot establish a defect')
    elif review_fail:
        raise ValueError('Reproduced failure has no process record')
    trial = jobs / row['job'] / row['trial']
    config = json.loads((trial / 'config.json').read_text())
    if Path(config['task']['path']).name != row['task']:
        raise ValueError('Trial configuration identifies a different task')
    if not (trial / 'result.json').exists():
        raise ValueError('Trial is still running; outcome contents remain unopened')
    composition = trial / 'agent/episode/artifacts/composition.json'
    calls = baseline(json.loads(composition.read_text()) if composition.exists() else {})
    calls.update({'checks.public-files': 'fail' if contract_fail else None,
                  'verdict.executed': 'fail' if combined['call'] == 'fail' else None})
    identities = {'config.json': sha(trial / 'config.json')}
    if composition.exists():
        identities[str(composition.relative_to(trial))] = sha(composition)
    candidate = combined.get('candidate_identity')
    if candidate and sha(trial / 'agent/episode/snapshot/workspace.tar.gz') != candidate:
        raise ValueError('Reviewed candidate archive changed')
    return {k: row[k] for k in ('job', 'trial', 'task', 'executor')} | {
        'calls': calls, 'candidate_identity': candidate,
        'reproduced_call': combined['reproduced_call'],
        'unavailable': row.get('unavailable'),
        'contract_error': combined.get('contract_error'),
        'trial_records_sha256': identities, 'check_records_sha256': tree(record)}


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ('checks', 'jobs', 'out'):
        p.add_argument('--' + name, type=Path, required=True)
    a = p.parse_args()
    if a.out.exists():
        raise ValueError('Refusing to replace sealed predictions')
    manifest = a.checks / 'manifest.json'
    rows = json.loads(manifest.read_text())
    population(rows)
    result = {'schema': 'openagents.archive-predictions.v1', 'contains_grades': False,
              'manifest_sha256': sha(manifest), 'plans_sha256': tree(a.checks / 'plans'),
              'predictions': [prediction(r, a.checks, a.jobs) for r in rows]}
    write(a.out, result)
    print(json.dumps({'trials': len(rows), 'sha256': sha(a.out), 'grades_opened': False}))


if __name__ == '__main__':
    main()
