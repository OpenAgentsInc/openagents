#!/usr/bin/env python3
"""Replay literal artifacts on opened development candidates, without inference."""
import argparse
import concurrent.futures
import json
from pathlib import Path
import subprocess
import tempfile
import time

from archive_checks import cli, container, copy_file, stop
from reproduce import sha, snapshot, tree, write
from seal_archive import population


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ('cohort', 'jobs', 'predictions', 'preflight', 'out', 'binary', 'runtime'):
        p.add_argument('--' + name, type=Path, required=True)
    a = p.parse_args()
    if a.out.exists():
        raise ValueError('Use a new development record directory')
    rows = json.loads(a.predictions.read_text())['predictions']
    population(rows)
    libraries = json.loads((a.runtime / 'manifest.json').read_text())
    if any(sha(a.runtime / item['name']) != item['sha256'] for item in libraries):
        raise ValueError('Checker runtime differs from its retained manifest')
    images = {v['task']: v['image'] for v in json.loads((a.preflight / 'preflight.json').read_text())}
    a.out.mkdir(parents=True)
    write(a.out / 'provenance.json', {
        'schema': 'openagents.literal-artifact-development.v1', 'development_only': True,
        'prediction_sha256': sha(a.predictions), 'binary_sha256': sha(a.binary),
        'runtime': libraries, 'model_calls': 0,
        'source': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
        'warning': 'The 72 official outcomes were already opened; this is not held-out confirmation.',
    })
    for task in sorted(images):
        out = a.out / 'plans' / task
        instruction = a.cohort / 'tasks/archive' / task / 'instruction.md'
        command = [str(a.binary), 'checks', 'contract', 'literal-plan', '--instruction', str(instruction),
                   '--task', task, '--workdir', '/app']
        started = time.monotonic()
        process = subprocess.run(command, capture_output=True, text=True, timeout=30)
        write(out / 'process.json', {'command': command, 'exit': process.returncode,
                                    'stdout': process.stdout, 'stderr': process.stderr,
                                    'seconds': time.monotonic() - started,
                                    'instruction_sha256': sha(instruction)})
        if process.returncode:
            raise ValueError('Literal plan failed; keep the original record')
        write(out / 'plan.json', json.loads(process.stdout))

    def check(row):
        dest = a.out / row['trial']
        record = {k: row[k] for k in ('job', 'trial', 'task', 'executor', 'candidate_identity')}
        record['call'] = 'unknown'
        started = time.monotonic()
        cid = None
        try:
            with tempfile.TemporaryDirectory(prefix='truth9646-') as temp:
                root = Path(temp)
                trial = a.jobs / row['job'] / row['trial']
                identity = snapshot(trial, root, allow_public_files=True)
                if identity['snapshot']['archive']['sha256'] != row['candidate_identity']:
                    raise ValueError('Candidate differs from the original seal')
                write(dest / 'snapshot.json', identity)
                cid = container(images[row['task']], a.binary, a.runtime, root / 'app')
                plan = a.out / 'plans' / row['task'] / 'plan.json'
                copy_file(plan, cid, '/tmp/plan.json')
                result = cli(cid, ['literal-run', '--plan', '/tmp/plan.json'], dest / 'run-process.json')
                write(dest / 'report.json', result)
                if tree(root) != identity['files']:
                    raise ValueError('Candidate changed during read-only replay')
                if result['call'] not in (None, 'fail'):
                    raise ValueError('Literal artifacts must never certify a pass')
                record.update(call=result['call'] or 'unknown', report_sha256=sha(dest / 'report.json'),
                              plan_sha256=sha(plan), candidate_unchanged=True)
        except (ValueError, OSError, subprocess.SubprocessError) as error:
            record['error'] = str(error)
        finally:
            stop(cid)
        record['seconds'] = time.monotonic() - started
        write(dest / 'process.json', record)
        return record

    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
        records = list(pool.map(check, rows))
    write(a.out / 'records.json', records)
    print(json.dumps({'trials': len(records), 'failure_calls': sum(r['call'] == 'fail' for r in records),
                      'unavailable': sum('error' in r for r in records), 'model_calls': 0,
                      'development_only': True}))


if __name__ == '__main__':
    main()
