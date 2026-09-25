#!/usr/bin/env python3
"""Open archive outcomes only after verifying a published prediction seal."""
import argparse
from datetime import datetime
import json
from pathlib import Path
import subprocess

from prepare import sha
from reproduce import tree, write
from seal_archive import population


def elapsed(value):
    if not value or not value.get('started_at') or not value.get('finished_at'):
        return None
    seconds = (datetime.fromisoformat(value['finished_at'].replace('Z', '+00:00'))
               - datetime.fromisoformat(value['started_at'].replace('Z', '+00:00'))).total_seconds()
    if seconds < 0:
        raise ValueError('Official timing ends before it begins')
    return seconds


def published_seal(path, commit, git_path):
    subprocess.run(['git', 'merge-base', '--is-ancestor', commit, 'origin/main'], check=True)
    committed = subprocess.check_output(['git', 'show', commit + ':' + git_path])
    if committed != path.read_bytes():
        raise ValueError('Predictions differ from the published seal')


def labels(predictions, jobs, checks, tasks=None):
    rows = predictions['predictions']
    population(rows) if tasks is None else population(rows, tasks)
    if tree(checks / 'plans') != predictions['plans_sha256']:
        raise ValueError('Public check plans changed after sealing')
    # Validate all evidence before opening even the first outcome.
    for row in rows:
        trial = jobs / row['job'] / row['trial']
        if tree(checks / row['trial']) != row['check_records_sha256']:
            raise ValueError('Check records changed after sealing')
        for name, digest in row['trial_records_sha256'].items():
            if sha(trial / name) != digest:
                raise ValueError('Trial record changed after sealing')
        if row['candidate_identity'] and sha(trial / 'agent/episode/snapshot/workspace.tar.gz') != row['candidate_identity']:
            raise ValueError('Candidate changed after sealing')
    result = []
    for row in rows:
        path = jobs / row['job'] / row['trial'] / 'result.json'
        value = json.loads(path.read_text())
        if Path(value['config']['task']['path']).name != row['task']:
            raise ValueError('Official result task differs from sealed identity')
        if value['trial_name'] != row['trial']:
            raise ValueError('Official result trial differs from sealed identity')
        reward = ((value.get('verifier_result') or {}).get('rewards') or {}).get('reward')
        if reward not in (None, 0, 1):
            raise ValueError('Unexpected official reward')
        result.append({k: row[k] for k in ('job', 'trial', 'task', 'executor')} | {
            'reward': reward, 'exception_type': (value.get('exception_info') or {}).get('exception_type'),
            'result_sha256': sha(path),
            'seconds': {'trial': elapsed(value),
                        **{stage: elapsed(value.get(stage)) for stage in
                           ('environment_setup', 'agent_setup', 'agent_execution', 'verifier')}}})
    return result


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ('predictions', 'jobs', 'checks', 'out'):
        p.add_argument('--' + name, type=Path, required=True)
    p.add_argument('--seal-commit', required=True)
    p.add_argument('--seal-path', required=True)
    a = p.parse_args()
    if a.out.exists():
        raise ValueError('Refusing to replace joined outcomes')
    published_seal(a.predictions, a.seal_commit, a.seal_path)
    result = {'prediction_sha256': sha(a.predictions), 'seal_commit': a.seal_commit,
              'labels': labels(json.loads(a.predictions.read_text()), a.jobs, a.checks)}
    write(a.out, result)
    print('Joined', len(result['labels']), 'official outcomes to the published seal.')


if __name__ == '__main__':
    main()
