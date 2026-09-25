#!/usr/bin/env python3
"""Review the existing mini-tasks' known candidates as development controls."""
import argparse
import concurrent.futures
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import time

from reproduce import tree, write

TASKS = ['log-severity', 'interactive-terminal', 'cancel-cleanup', 'git-recovery']
DOCKERFILE = '''FROM python:3.11-slim-bookworm
RUN apt-get update && apt-get install -y --no-install-recommends bash git coreutils && rm -rf /var/lib/apt/lists/*
WORKDIR /app
'''


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--out', type=Path, required=True)
    p.add_argument('--binary', type=Path, required=True)
    p.add_argument('--prompt', choices=['v1', 'literal-v2'], default='v1')
    p.add_argument('--tasks', nargs='+', choices=TASKS, default=TASKS)
    a = p.parse_args()
    if shutil.disk_usage(a.out.parent).free < 10 * 1024**3:
        raise ValueError('Mini-task preflight needs 10 GiB free before building')
    a.out.mkdir(parents=True, exist_ok=True)
    context = a.out / 'environment'
    context.mkdir(exist_ok=True)
    (context / 'Dockerfile').write_text(DOCKERFILE)
    with (context / 'build.log').open('w') as log:
        subprocess.run(['docker', 'build', '-t', 'truth9584-review/mini:development', str(context)],
                       stdout=log, stderr=subprocess.STDOUT, check=True, timeout=600)
    image = subprocess.check_output(['docker', 'image', 'inspect', '--format', '{{.Id}}',
                                     'truth9584-review/mini:development'], text=True).strip()
    write(context / 'identity.json', {'image': image, 'files': tree(context),
                                     'binary_sha256': hashlib.sha256(a.binary.read_bytes()).hexdigest()})
    env = dict(os.environ)
    env['TYPESAFE_API_KEY'] = json.loads((Path.home() / '.openagents/jev.json').read_text())['api_key']
    env['CODEX_AUTH_JSON_PATH'] = str(Path.home() / '.codex/auth.json')
    instructions = {}
    for task in a.tasks:
        dest = a.out / 'public' / task
        if not dest.exists():
            subprocess.run([str(a.binary), 'minitask', 'setup', task, str(dest)], check=True, capture_output=True)
        instructions[task] = dest.with_suffix('.instruction.md').read_text()

    def run(pair):
        task, variant = pair
        out = a.out / (task + '-' + variant)
        if (out / 'result.json').exists():
            return json.loads((out / 'result.json').read_text())
        out.mkdir(exist_ok=True)
        with (out / 'candidate.log').open('w') as log:
            ran = subprocess.run([str(a.binary), 'minitask', 'run', task, '--executor', 'scripted',
                                  '--script', variant, '--jev', 'off', '--deadline', '120',
                                  '--out', str(out / 'episodes'), '--json'], stdout=log,
                                 stderr=subprocess.STDOUT, timeout=180)
        episodes = list((out / 'episodes').glob('*/manifest.json'))
        if len(episodes) != 1 or ran.returncode not in (0, 1):
            raise ValueError('Mini-task candidate did not finish normally')
        episode = episodes[0].parent
        work = episode / 'work'
        original = tree(work)
        identity = hashlib.sha256(json.dumps(original, sort_keys=True).encode()).hexdigest()
        # A bounded source map complements the full read-only workspace.
        files = {}
        for path in sorted(work.glob('*')):
            if path.is_file() and not path.is_symlink() and path.stat().st_size < 16000:
                try:
                    files['/app/' + path.name] = path.read_text()
                except UnicodeError:
                    pass
        packet = {'candidate': {'task': instructions[task], 'files': files,
                               'coverage': 'The complete unchanged candidate is mounted read-only at /app. Only /tmp is writable. The public environment has Python 3.11, bash, and git. There is no network or grader; a setup limitation is not a candidate failure.'},
                  'candidate_identity': identity}
        write(out / 'input.json', packet)
        write(out / 'candidate-files.json', original)
        container = None
        started = time.monotonic()
        try:
            container = subprocess.check_output([
                'docker', 'run', '-d', '--rm', '--network', 'none', '--read-only', '--cap-drop', 'ALL',
                '--security-opt', 'no-new-privileges', '--pids-limit', '128', '--memory', '2g', '--memory-swap', '2g',
                '--cpus', '2', '--tmpfs', '/tmp:rw,size=128m', '--mount', f'type=bind,src={work},dst=/app,readonly',
                '--workdir', '/app', '--env', 'HOME=/tmp', '--env', 'PYTHONDONTWRITEBYTECODE=1',
                '--env', 'GIT_CONFIG_COUNT=1', '--env', 'GIT_CONFIG_KEY_0=safe.directory', '--env', 'GIT_CONFIG_VALUE_0=/app',
                '--label', 'openagents.candidate-review=1', '--label', 'openagents.candidate-identity=' + identity,
                '--entrypoint', 'sleep', image, '400'], text=True, timeout=30).strip()
            with (out / 'review.log').open('w') as log:
                result = subprocess.run([str(a.binary), 'checks', 'reproduced-review', '--input', str(out / 'input.json'),
                                         '--container', container, '--out', str(out / 'reproduced'),
                                         '--prompt', a.prompt],
                                        env=env, stdout=log, stderr=subprocess.STDOUT, timeout=340)
            if tree(work) != original:
                raise ValueError('Review changed a candidate')
            review = json.loads((out / 'reproduced/review.json').read_text())
            grade = json.loads((episode / 'verification/grade.json').read_text())
            record = {'task': task, 'variant': variant, 'grade': grade, 'call': review['call'],
                      'review_exit': result.returncode, 'score': review['score'], 'error': review['error'],
                      'known_native_cost_usd': review['known_native_cost_usd'],
                      'jev_input_tokens': sum(f.get('input_tokens') or 0 for f in review['findings']),
                      'seconds': time.monotonic() - started, 'candidate_identity': identity}
            write(out / 'result.json', record)
            return record
        finally:
            if container:
                subprocess.run(['docker', 'rm', '-f', container], capture_output=True, timeout=30)

    pairs = [(t, v) for t in a.tasks for v in ['good', 'bad']]
    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
        for result in pool.map(run, pairs):
            print(json.dumps(result), flush=True)


if __name__ == '__main__':
    main()
