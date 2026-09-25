#!/usr/bin/env python3
"""Run the frozen archive cohort, retaining outcomes without displaying them."""
import argparse
import concurrent.futures
from dataclasses import replace
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tomllib

from archive_preflight import PIN, TASKS
from reproduce import tree, write
from tbench import agents, jobconfig, panel, runner

ARTIFACT_SHA = '7df7cde47d0f16c8f9200cbd21899254588c0b587f3bbe6f2272fd2eedfb0f5d'


def request(a, arm):
    tasks = []
    for name in TASKS:
        source = a.out / 'tasks/archive' / name
        config = tomllib.loads((source / 'task.toml').read_text())
        e = config['environment']
        tasks.append(panel.Task(name, 'archive/' + name, 'archive', 'confirmation',
                                panel.TaskResources(e['cpus'], e['memory_mb'], e['storage_mb'], e['gpus']),
                                int(config['agent']['timeout_sec'])))
    population = panel.Panel('https://github.com/harbor-framework/terminal-bench', PIN, tuple(tasks))
    profile = replace(jobconfig.load_job_profile('smoke'), id='archive-truth-confirmation',
                      description='Twelve unused archive tasks; three repetitions per executor.',
                      task_ids=tuple(TASKS), n_attempts=3, n_concurrent_trials=2, controls=False,
                      environment={'type': 'docker', 'import_path': 'tbench.warm_docker:WarmDockerEnvironment',
                                   'delete': True, 'force_build': False,
                                   'cpu_enforcement_policy': 'auto', 'memory_enforcement_policy': 'auto'})
    agent = replace(agents.load_agents()['coder-one-microluna-v13'],
                    id='coder-one-archive-' + arm)
    name = 'archive--coder-one-' + ('truth-confirmation' if arm == 'luna' else 'truth-control') + '--9584-literal-r1'
    policy = 'prospective-policy.json' if arm == 'luna' else 'prospective-astra-policy.json'
    return runner.RunRequest(population, profile, agent, tasks, auth_mode='auth-json',
                             agent_kwargs={'policy': str(a.out / policy), 'artifact_path': str(a.binary),
                                           'artifact_sha256': ARTIFACT_SHA},
                             checkout=a.out / 'tasks', jobs_dir=a.jobs, job_name=name)


def prepare(a):
    head = subprocess.check_output(['git', '-C', str(a.upstream), 'rev-parse', 'HEAD'], text=True).strip()
    if head != PIN:
        raise ValueError('Upstream checkout changed after preflight')
    preflight = json.loads((a.preflight / 'preflight.json').read_text())
    if {r['task'] for r in preflight} != set(TASKS) or any(r['status'] != 'available' for r in preflight):
        raise ValueError('All declared public environments must be available')
    if hashlib.sha256(a.binary.read_bytes()).hexdigest() != ARTIFACT_SHA:
        raise ValueError('Executor binary does not match the frozen artifact')
    a.out.mkdir(parents=True, exist_ok=True)
    provenance = []
    for r in preflight:
        source = a.upstream / 'archive' / r['task']
        if tree(source / 'environment') != r['public_files'] or hashlib.sha256((source / 'instruction.md').read_bytes()).hexdigest() != r['instruction_sha256']:
            raise ValueError('Public task changed after preflight')
        dest = a.out / 'tasks/archive' / r['task']
        original = (source / 'task.toml').read_text()
        config = tomllib.loads(original)
        # Pin the locally built public image. Instructions, graders, solutions,
        # allowances, and inputs remain byte-identical to the upstream task.
        old = 'docker_image = ' + json.dumps(config['environment']['docker_image'])
        new = 'docker_image = ' + json.dumps(r['image'])
        if original.count(old) != 1:
            raise ValueError('Cannot pin the declared public image')
        if not dest.exists():
            shutil.copytree(source, dest)
            (dest / 'task.toml').write_text(original.replace(old, new))
        before, after = tree(source), tree(dest)
        if {k: v for k, v in before.items() if k != 'task.toml'} != {k: v for k, v in after.items() if k != 'task.toml'}:
            raise ValueError('Staged task changed beyond its image pin')
        if (dest / 'task.toml').read_text() != original.replace(old, new):
            raise ValueError('Staged task configuration changed')
        provenance.append({'task': r['task'], 'upstream': PIN, 'public_image': r['image'],
                           'original_files': before, 'staged_files': after})
    for name in ['prospective-policy.json', 'prospective-astra-policy.json']:
        source, dest = Path(__file__).parent / name, a.out / name
        if dest.exists() and dest.read_bytes() != source.read_bytes():
            raise ValueError('Frozen policy differs from the retained one')
        shutil.copyfile(source, dest)
    write(a.out / 'task-provenance.json', provenance)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ['upstream', 'preflight', 'out', 'jobs', 'binary']:
        p.add_argument('--' + name, type=Path, required=True)
    p.add_argument('--prepare-only', action='store_true')
    p.add_argument('--arm', choices=['luna', 'astra'])
    a = p.parse_args()
    if a.arm:
        os.environ['OPENAGENTS_API_KEY'] = (Path.home() / '.openagents/bearer').read_text().strip()
        os.environ['TYPESAFE_API_KEY'] = json.loads((Path.home() / '.openagents/jev.json').read_text())['api_key']
        os.environ['CODEX_AUTH_JSON_PATH'] = str(Path.home() / '.codex/auth.json')
        req = request(a, a.arm)
        job = a.jobs / req.job_name
        if job.exists():
            raise ValueError('This job already exists; preserve it and investigate instead of rerunning')
        runner.run(req, harbor_argv0=str(Path(sys.executable).parent / 'harbor'))
        return
    prepare(a)
    for arm in ['luna', 'astra']:
        req = request(a, arm)
        config = jobconfig.build_job_config(req.panel, req.profile, req.tasks, req.agent,
                                            auth_mode=req.auth_mode, agent_kwargs=req.agent_kwargs,
                                            checkout=req.checkout, jobs_dir=req.jobs_dir, job_name=req.job_name)
        write(a.out / (arm + '-job.json'), config)
    if a.prepare_only:
        print('Prepared 72 trials: 12 tasks, two executors, three repetitions. No agent or grader ran.')
        return
    if shutil.disk_usage(a.out).free < 20 * 1024**3:
        raise ValueError('Cohort requires at least 20 GiB free before launch')

    def run_arm(arm):
        command = [sys.executable, str(Path(__file__).resolve())]
        for name in ['upstream', 'preflight', 'out', 'jobs', 'binary']:
            command += ['--' + name, str(getattr(a, name))]
        command += ['--arm', arm]
        with (a.out / (arm + '-launcher.log')).open('w') as log:
            result = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT)
        write(a.out / (arm + '-launcher.json'), {'arm': arm, 'exit': result.returncode})
        return {'arm': arm, 'launcher_exit': result.returncode, 'outcomes_displayed': False}

    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
        for result in pool.map(run_arm, ['luna', 'astra']):
            print(json.dumps(result), flush=True)


if __name__ == '__main__':
    main()
