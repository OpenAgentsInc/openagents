#!/usr/bin/env python3
"""Run the reserved literal-artifact confirmation with a published protocol."""
import argparse
import concurrent.futures
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile
from types import SimpleNamespace

HERE = Path(__file__).resolve().parent
PREVIOUS = HERE.parent / '2026-09-25-candidate-review'
sys.path.insert(0, str(PREVIOUS))

import archive_run
from archive_checks import cli, container, copy_file, make_plan, stop
from join_archive import labels, published_seal
from measure_archive import paired_rows, separation
from measure_prospective import measure
from prepare import prepare as candidate_packet
from reproduce import run as reproduce, sha, snapshot, tree, write
from seal_archive import baseline, population
from tbench import jobconfig, runner


def credentials():
    env = dict(os.environ)
    env['OPENAGENTS_API_KEY'] = (Path.home() / '.openagents/bearer').read_text().strip()
    env['TYPESAFE_API_KEY'] = json.loads((Path.home() / '.openagents/jev.json').read_text())['api_key']
    env['CODEX_AUTH_JSON_PATH'] = str(Path.home() / '.codex/auth.json')
    return env


def validate(a):
    rule = json.loads(a.protocol.read_text())
    if rule['schema'] != 'openagents.literal-confirmation-protocol.v1':
        raise ValueError('Unsupported protocol')
    for name, key in [('binary', 'executor_sha256'), ('check_binary', 'check_sha256'),
                      ('review_binary', 'review_sha256')]:
        if sha(getattr(a, name)) != rule[key]:
            raise ValueError('Artifact differs: ' + name)
    libraries = json.loads((a.runtime / 'manifest.json').read_text())
    if libraries != rule['runtime'] or any(sha(a.runtime / r['name']) != r['sha256'] for r in libraries):
        raise ValueError('Checker runtime differs')
    if sha(a.preflight / 'preflight.json') != rule['preflight_sha256']:
        raise ValueError('Preflight differs')
    for name, digest in rule['source_sha256'].items():
        if sha(HERE.parent / name) != digest:
            raise ValueError('Frozen orchestration differs: ' + name)
    tasks = rule['tasks']
    if not tasks or len(tasks) != len(set(tasks)) or any('/' in t or t in ('.', '..') for t in tasks):
        raise ValueError('Invalid reserved tasks')
    if a.action not in ('prepare', 'measure', 'join'):
        root = Path(subprocess.check_output(['git', 'rev-parse', '--show-toplevel'], text=True).strip())
        name = str(a.protocol.resolve().relative_to(root))
        if subprocess.check_output(['git', 'show', 'origin/main:' + name]) != a.protocol.read_bytes():
            raise ValueError('Protocol must be published before candidate generation or checking')
    a.run_id = rule['run_id']
    return rule


def request(a, rule, arm):
    return archive_run.request(a, arm, rule['tasks'])


def prepare(a, rule):
    archive_run.prepare(a, rule['tasks'])
    write(a.out / 'protocol-copy.json', rule)
    for arm in ('luna', 'astra'):
        req = request(a, rule, arm)
        config = jobconfig.build_job_config(req.panel, req.profile, req.tasks, req.agent,
                                            auth_mode=req.auth_mode, agent_kwargs=req.agent_kwargs,
                                            checkout=req.checkout, jobs_dir=req.jobs_dir, job_name=req.job_name)
        write(a.out / (arm + '-job.json'), config)


def run(a, rule):
    if sha(a.out / 'protocol-copy.json') != sha(a.protocol):
        raise ValueError('Prepared protocol differs')
    if shutil.disk_usage(a.out).free < 20 * 1024**3:
        raise ValueError('At least 20 GiB free is required before launch')
    if a.arm:
        req = request(a, rule, a.arm)
        if (a.jobs / req.job_name).exists():
            raise ValueError('Job already exists; retain it instead of replacing attempts')
        os.environ.update(credentials())
        runner.run(req, harbor_argv0=str(Path(sys.executable).parent / 'harbor'))
        return

    def arm(name):
        command = [sys.executable, str(Path(__file__).resolve()), *sys.argv[1:], '--arm', name]
        with (a.out / (name + '-launcher.log')).open('w') as log:
            result = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT)
        write(a.out / (name + '-launcher.json'), {'exit': result.returncode, 'arm': name})
        return {'arm': name, 'exit': result.returncode, 'outcomes_displayed': False}

    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
        for result in pool.map(arm, ('luna', 'astra')):
            print(json.dumps(result), flush=True)


def plans(a, rule, images):
    checks = a.out / 'checks'
    old = SimpleNamespace(out=checks, cohort=a.out, binary=a.check_binary, runtime=a.runtime)
    for task in rule['tasks']:
        make_plan(task, images[task], old)
        dest = checks / 'plans' / task / 'literal-plan.json'
        if dest.exists():
            continue
        instruction = a.out / 'tasks/archive' / task / 'instruction.md'
        proc = subprocess.run([str(a.check_binary), 'checks', 'contract', 'literal-plan',
                               '--instruction', str(instruction), '--task', task, '--workdir', '/app'],
                              capture_output=True, text=True, timeout=30)
        write(dest.parent / 'literal-extract-process.json',
              {'exit': proc.returncode, 'stdout': proc.stdout, 'stderr': proc.stderr})
        if proc.returncode:
            raise ValueError('Literal plan failed before checking candidates')
        write(dest, json.loads(proc.stdout))


def check_one(a, rule, images, row, env):
    checks = a.out / 'checks'
    dest = checks / row['trial']
    if (dest / 'combined.json').exists():
        return
    record = {k: row[k] for k in ('job', 'trial', 'task')}
    record.update(call='unknown', contract_call='unknown', literal_call='unknown', reproduced_call='unknown')
    cid = None
    with tempfile.TemporaryDirectory(prefix='truth9584-confirmation-') as name:
        root = Path(name)
        try:
            identity = snapshot(a.jobs / row['job'] / row['trial'], root, allow_public_files=True)
            write(dest / 'snapshot.json', identity)
            record['candidate_identity'] = identity['snapshot']['archive']['sha256']
            cid = container(images[row['task']], a.check_binary, a.runtime, root / 'app')
            for kind, verb, filename in [('contract', 'run', 'plan.json'),
                                          ('literal', 'literal-run', 'literal-plan.json')]:
                plan = checks / 'plans' / row['task'] / filename
                if not plan.exists():
                    record[kind + '_error'] = 'Public plan unavailable'
                    continue
                copy_file(plan, cid, '/tmp/' + filename)
                result = cli(cid, [verb, '--plan', '/tmp/' + filename], dest / (kind + '-process.json'))
                write(dest / (kind + '.json'), result)
                failed = (any(i['outcome'] == 'differed' for i in result['items'])
                          if kind == 'contract' else result['call'] == 'fail')
                record[kind + '_call'] = 'fail' if failed else 'unknown'
            if tree(root) != identity['files']:
                raise ValueError('Read-only candidate changed')
            record['candidate_unchanged'] = True
        except (ValueError, OSError, subprocess.SubprocessError, tarfile.TarError) as error:
            record.update(contract_call='unknown', literal_call='unknown', contract_error=str(error))
        finally:
            stop(cid)
    if 'fail' in (record['contract_call'], record['literal_call']):
        record.update(call='fail', reproduced_call='not_requested', review_skip='A literal or original file check decides the OR.')
    elif row.get('input'):
        args = SimpleNamespace(out=checks, jobs=a.jobs, binary=a.review_binary, record_name='reproduced',
                               allow_public_files=True, prompt='literal-v2', execution_profile='owner-exec')
        reproduce(row, args, images, env)
        review = json.loads((dest / 'reproduced/process.json').read_text())
        record['reproduced_call'] = review['call'] if not review.get('error') else 'unknown'
        record['call'] = 'fail' if record['reproduced_call'] == 'fail' else 'unknown'
    write(dest / 'combined.json', record)


def check(a, rule):
    images = {r['task']: r['image'] for r in json.loads((a.preflight / 'preflight.json').read_text())}
    plans(a, rule, images)
    if a.plans_only:
        print('Public plans retained; no candidate or outcome read.')
        return
    rows = []
    for arm in ('luna', 'astra'):
        job = a.jobs / request(a, rule, arm).job_name
        for config in sorted(job.glob('*/config.json')):
            if not (config.parent / 'result.json').exists():
                continue
            task = Path(json.loads(config.read_text())['task']['path']).name
            if task not in rule['tasks']:
                raise ValueError('Unexpected task')
            row = {'job': job.name, 'trial': config.parent.name, 'task': task, 'executor': arm}
            row.update(candidate_packet(row, a.jobs, a.out / 'checks'))
            rows.append(row)
    write(a.out / 'checks/manifest.json', rows)
    env = credentials()
    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
        list(pool.map(lambda row: check_one(a, rule, images, row, env), rows))
    print('Retained checks for', len(rows), 'completed attempts; outcomes unopened.')


def prediction(row, checks, jobs):
    dest = checks / row['trial']
    combined = json.loads((dest / 'combined.json').read_text())
    if any(combined[k] != row[k] for k in ('job', 'trial', 'task')):
        raise ValueError('Check attribution differs')
    failed = {k: combined[k + '_call'] == 'fail' for k in ('contract', 'literal', 'reproduced')}
    if (combined['call'] == 'fail') != any(failed.values()):
        raise ValueError('Combined call violates the frozen OR')
    for kind in ('contract', 'literal'):
        if not failed[kind]:
            continue
        if combined.get('candidate_unchanged') is not True or combined.get('contract_error'):
            raise ValueError('Unavailable or changed candidate cannot establish a file failure')
        report = json.loads((dest / (kind + '.json')).read_text())
        supported = (any(i['outcome'] == 'differed' for i in report['items']) if kind == 'contract'
                     else report['call'] == 'fail' and any(i['outcome']['outcome'] == 'differed' for i in report['items']))
        if not supported or combined['reproduced_call'] != 'not_requested':
            raise ValueError('Cheap failure lacks a difference or failed to skip review')
    if failed['reproduced']:
        process = json.loads((dest / 'reproduced/process.json').read_text())
        report = dest / 'reproduced/review.json'
        if (process.get('error') or process['call'] != 'fail' or process['execution_profile'] != 'owner-exec'
                or sha(report) != process['review_sha256'] or json.loads(report.read_text())['call'] != 'fail'):
            raise ValueError('Reproduced failure lacks a valid original report')
    trial = jobs / row['job'] / row['trial']
    config = trial / 'config.json'
    if Path(json.loads(config.read_text())['task']['path']).name != row['task'] or not (trial / 'result.json').exists():
        raise ValueError('Missing completed trial or wrong task')
    comp = trial / 'agent/episode/artifacts/composition.json'
    calls = baseline(json.loads(comp.read_text()) if comp.exists() else {})
    calls.update({'checks.public-files': 'fail' if failed['contract'] else None,
                  'checks.literal-artifacts': 'fail' if failed['literal'] else None,
                  'verdict.literal-executed': 'fail' if any(failed.values()) else None})
    records = {'config.json': sha(config)}
    if comp.exists():
        records[str(comp.relative_to(trial))] = sha(comp)
    candidate = combined.get('candidate_identity')
    if any(failed.values()) and not candidate:
        raise ValueError('Failure lacks an attributable candidate identity')
    if candidate and sha(trial / 'agent/episode/snapshot/workspace.tar.gz') != candidate:
        raise ValueError('Candidate identity differs')
    return {k: row[k] for k in ('job', 'trial', 'task', 'executor')} | {
        'calls': calls, 'candidate_identity': candidate, 'reproduced_call': combined['reproduced_call'],
        'unavailable': row.get('unavailable'), 'contract_error': combined.get('contract_error'),
        'trial_records_sha256': records, 'check_records_sha256': tree(dest)}


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('action', choices=['prepare', 'run', 'check', 'seal', 'join', 'measure'])
    for name in ('protocol', 'out', 'jobs', 'binary', 'check-binary', 'review-binary', 'runtime', 'preflight', 'upstream'):
        p.add_argument('--' + name, type=Path, required=True)
    p.add_argument('--arm', choices=['luna', 'astra'])
    p.add_argument('--plans-only', action='store_true')
    p.add_argument('--seal-commit')
    p.add_argument('--seal-path')
    a = p.parse_args()
    rule = validate(a)
    if a.action in ('prepare', 'run', 'check'):
        {'prepare': prepare, 'run': run, 'check': check}[a.action](a, rule)
        return
    checks = a.out / 'checks'
    sealed = a.out / 'sealed.json'
    if a.action == 'seal':
        if sealed.exists():
            raise ValueError('Refusing to replace sealed predictions')
        rows = json.loads((checks / 'manifest.json').read_text())
        population(rows, rule['tasks'])
        write(sealed, {'schema': 'openagents.literal-confirmation-predictions.v1', 'contains_grades': False,
                       'protocol_sha256': sha(a.protocol), 'plans_sha256': tree(checks / 'plans'),
                       'predictions': [prediction(r, checks, a.jobs) for r in rows]})
        print('Sealed', len(rows), 'predictions:', sha(sealed))
    elif a.action == 'join':
        if (a.out / 'labels.json').exists():
            raise ValueError('Refusing to replace joined outcomes')
        published_seal(sealed, a.seal_commit, a.seal_path)
        predictions = json.loads(sealed.read_text())
        if predictions['protocol_sha256'] != sha(a.protocol):
            raise ValueError('Sealed protocol differs')
        write(a.out / 'labels.json', {'prediction_sha256': sha(sealed), 'seal_commit': a.seal_commit,
                                     'labels': labels(predictions, a.jobs, checks, rule['tasks'])})
        print('Joined outcomes after verifying the published seal.')
    else:
        official = json.loads((a.out / 'labels.json').read_text())
        if official['prediction_sha256'] != sha(sealed):
            raise ValueError('Labels identify different predictions')
        rows = paired_rows(json.loads(sealed.read_text())['predictions'], official['labels'], rule['tasks'])
        signals = list(rows[0]['calls'])
        graded = [r for r in rows if r['reward'] in (0, 1)]
        result = {'schema': 'openagents.literal-confirmation-measurement.v1',
                  'protocol_sha256': sha(a.protocol), 'prediction_sha256': sha(sealed),
                  'labels_sha256': sha(a.out / 'labels.json'), 'all': measure(rows, signals),
                  'by_executor': {e: measure([r for r in rows if r['executor'] == e], signals) for e in ('luna', 'astra')},
                  'within_task': {s: separation(graded, s) for s in signals},
                  'within_task_by_executor': {e: {s: separation([r for r in graded if r['executor'] == e], s)
                                                  for s in signals} for e in ('luna', 'astra')},
                  'bootstrap_seed': 9584, 'bootstrap_resamples': 10000, 'predictions': rows}
        write(a.out / 'measurement.json', result)
        print(json.dumps(result['all'], indent=2))


if __name__ == '__main__':
    main()
