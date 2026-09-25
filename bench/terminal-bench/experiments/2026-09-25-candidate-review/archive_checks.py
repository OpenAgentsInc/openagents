#!/usr/bin/env python3
"""Run frozen public-file checks, then reproduced review, without reading grades."""
import argparse
import concurrent.futures
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import subprocess
import tarfile
import tempfile
from types import SimpleNamespace

from archive_preflight import TASKS
from prepare import prepare
from reproduce import run as reproduce, sha, snapshot, tree, write


def plan_digest(plan):
    value = dict(plan, digest='')
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':'),
                                    ensure_ascii=False).encode()).hexdigest()


def narrow(plan):
    if plan_digest(plan) != plan['digest']:
        raise ValueError('Cannot reproduce the Rust plan digest')
    result = dict(plan)
    result['items'] = [i for i in plan['items']
                       if i['kind'] in ('path', 'format') and i['source'] == 'instruction'
                       and not i.get('command') and not i.get('not_executable')
                       and i.get('path', '').startswith('/app/')
                       and '..' not in PurePosixPath(i['path']).parts]
    result['digest'] = plan_digest(result)
    return result


def container(image, binary, runtime, app=None):
    command = ['docker', 'run', '-d', '--rm', '--network', 'none', '--read-only',
               '--cap-drop', 'ALL', '--security-opt', 'no-new-privileges', '--pids-limit', '128',
               '--memory', '2g', '--memory-swap', '2g', '--cpus', '2',
               '--tmpfs', '/tmp:rw,size=256m', '--workdir', '/app', '--env', 'HOME=/tmp',
               '--env', 'PYTHONDONTWRITEBYTECODE=1',
               '--mount', f'type=bind,src={binary},dst=/opt/contract-check,readonly',
               '--mount', f'type=bind,src={runtime},dst=/opt/contract-runtime,readonly']
    if app:
        command += ['--mount', f'type=bind,src={app},dst=/app,readonly']
    return subprocess.check_output(command + ['--entrypoint', 'sleep', image, '180'],
                                   text=True, timeout=30).strip()


def cli(container_id, args, dest):
    done = subprocess.run(['docker', 'exec', container_id, '/opt/contract-runtime/ld-linux-x86-64.so.2',
                           '--library-path', '/opt/contract-runtime', '/opt/contract-check', 'checks', 'contract', *args],
                          capture_output=True, text=True, timeout=90)
    write(dest, {'exit': done.returncode, 'stdout': done.stdout, 'stderr': done.stderr})
    if done.returncode:
        raise ValueError('Contract command failed; see retained process record')
    return json.loads(done.stdout)


def copy_file(path, container_id, dest):
    # Docker's archive-copy endpoint refuses this read-only root even when the
    # destination is a writable tmpfs. Stream bytes through the container instead.
    done = subprocess.run(['docker', 'exec', '-i', container_id, 'sh', '-c',
                           'cat > "$1"', 'sh', dest], input=path.read_bytes(),
                          capture_output=True, timeout=30)
    if done.returncode:
        raise ValueError('Cannot stage a public check input: ' + done.stderr.decode(errors='replace'))


def stop(container_id):
    if container_id:
        subprocess.run(['docker', 'rm', '-f', container_id], capture_output=True, timeout=30)


def make_plan(task, image, a):
    out = a.out / 'plans' / task
    if (out / 'process.json').exists():
        return
    out.mkdir(parents=True, exist_ok=True)
    cid = None
    try:
        cid = container(image, a.binary, a.runtime)
        instruction = a.cohort / 'tasks/archive' / task / 'instruction.md'
        copy_file(instruction, cid, '/tmp/instruction.md')
        plan = cli(cid, ['plan', '--instruction', '/tmp/instruction.md', '--workdir', '/app',
                         '--task', task, '--jev', 'off'], out / 'extract-process.json')
        write(out / 'original-plan.json', plan)
        selected = narrow(plan)
        write(out / 'plan.json', selected)
        copy_file(out / 'plan.json', cid, '/tmp/plan.json')
        baseline = cli(cid, ['run', '--plan', '/tmp/plan.json'], out / 'baseline-process.json')
        write(out / 'baseline.json', baseline)
        write(out / 'process.json', {'image': image, 'call': 'plan', 'items': len(selected['items']),
                                     'binary_sha256': sha(a.binary), 'plan_sha256': sha(out / 'plan.json')})
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        write(out / 'process.json', {'image': image, 'call': 'unknown', 'error': str(error)})
    finally:
        stop(cid)


def check(row, image, a, env):
    out = a.out / row['trial']
    if (out / 'combined.json').exists():
        return
    out.mkdir(parents=True, exist_ok=True)
    record = {'call': 'unknown', 'contract_call': 'unknown', 'reproduced_call': 'unknown',
              'task': row['task'], 'trial': row['trial'], 'job': row['job']}
    cid = None
    with tempfile.TemporaryDirectory(prefix='truth9584-archive-') as temp:
        root = Path(temp)
        try:
            trial = a.jobs / row['job'] / row['trial']
            identity = snapshot(trial, root, allow_public_files=True)
            write(out / 'snapshot.json', identity)
            record['candidate_identity'] = identity['snapshot']['archive']['sha256']
            # Public-file identity checks are also performed by reproduced.py.
            # The cheap arm needs only the complete retained /app view.
            plan_path = a.out / 'plans' / row['task'] / 'plan.json'
            if plan_path.exists():
                cid = container(image, a.binary, a.runtime, root / 'app')
                copy_file(plan_path, cid, '/tmp/plan.json')
                report = cli(cid, ['run', '--plan', '/tmp/plan.json'], out / 'contract-process.json')
                write(out / 'contract.json', report)
                if any(i['outcome'] == 'differed' for i in report['items']):
                    record['contract_call'] = 'fail'
                if tree(root) != identity['files']:
                    raise ValueError('Candidate changed during the contract check')
        except (ValueError, OSError, subprocess.SubprocessError, tarfile.TarError) as error:
            record['contract_call'] = 'unknown'
            record['contract_error'] = str(error)
        finally:
            stop(cid)
    if record['contract_call'] == 'fail':
        record.update(call='fail', reproduced_call='not_requested',
                      review_skip='The frozen OR rule is already decided by an executed file check.')
    elif row.get('input'):
        args = SimpleNamespace(out=a.out, jobs=a.jobs, binary=a.binary, record_name='reproduced',
                               allow_public_files=True, prompt='literal-v2')
        reproduce(row, args, {row['task']: image}, env)
        review = json.loads((out / 'reproduced/process.json').read_text())
        record['reproduced_call'] = review['call']
        record['call'] = 'fail' if review['call'] == 'fail' and not review.get('error') else 'unknown'
    write(out / 'combined.json', record)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ['cohort', 'preflight', 'jobs', 'out', 'binary', 'runtime']:
        p.add_argument('--' + name, type=Path, required=True)
    p.add_argument('--plans-only', action='store_true')
    a = p.parse_args()
    runtime = json.loads((a.runtime / 'manifest.json').read_text())
    if any(sha(a.runtime / f['name']) != f['sha256'] for f in runtime):
        raise ValueError('Contract runtime differs from its manifest')
    a.out.mkdir(parents=True, exist_ok=True)
    images = {r['task']: r['image'] for r in json.loads((a.preflight / 'preflight.json').read_text())}
    for task in TASKS:
        make_plan(task, images[task], a)
    if a.plans_only:
        print('Public plans and untouched baselines retained. No candidates or grades read.')
        return
    rows = []
    for arm in ['luna', 'astra']:
        config = json.loads((a.cohort / (arm + '-job.json')).read_text())
        job = a.jobs / config['job_name']
        for trial_config in sorted(job.glob('*/config.json')):
            task = Path(json.loads(trial_config.read_text())['task']['path']).name
            if task not in TASKS:
                raise ValueError('Unexpected task in the frozen job')
            row = {'job': job.name, 'trial': trial_config.parent.name, 'task': task, 'executor': arm}
            row.update(prepare(row, a.jobs, a.out))
            rows.append(row)
    write(a.out / 'manifest.json', rows)
    env = dict(os.environ)
    env['TYPESAFE_API_KEY'] = json.loads((Path.home() / '.openagents/jev.json').read_text())['api_key']
    env['CODEX_AUTH_JSON_PATH'] = str(Path.home() / '.codex/auth.json')
    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
        for _ in pool.map(lambda row: check(row, images[row['task']], a, env), rows):
            pass
    print('Retained', len(rows), 'candidate check records; no official outcomes opened.')


if __name__ == '__main__':
    main()
