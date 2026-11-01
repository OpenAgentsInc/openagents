#!/usr/bin/env python3
"""Run changed Aethyr examples and retain precisely matching prior evidence."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import resource
import signal
import subprocess
import time

ROOT = Path(__file__).resolve().parents[2]
MANIFEST = 'aethyr/book/examples.json'
DEADLINE = None


def sha(data):
    return hashlib.sha256(data).hexdigest()


def file_sha(path):
    return sha(Path(path).read_bytes())


def execute(command, timeout, preexec_fn=None):
    """Timeouts and interruptions reap the compiler and its child toolchain."""
    if DEADLINE is not None:
        timeout = min(timeout, max(0.001, DEADLINE - time.monotonic()))
    process = subprocess.Popen(command, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                               start_new_session=True, preexec_fn=preexec_fn)
    try:
        stdout, stderr = process.communicate(timeout=timeout)
        return subprocess.CompletedProcess(command, process.returncode, stdout, stderr)
    except BaseException:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        process.communicate(timeout=5)
        raise


def reusable(old, inputs, item, compiler_sha):
    return (old is not None and old.get('status') == 'passed' and
            old.get('inputs') == inputs and old.get('item') == item and
            old.get('compiler_sha256') == compiler_sha and
            {r['mode'] for r in old.get('runs', []) if r.get('status') == 'passed'} == set(item['modes']))


def previous_rows(path, commit):
    if path is None:
        return {}, None
    data = path.read_bytes()
    proof = json.loads(data)
    if proof['schema'] == 2:
        return {r['source']: r for r in proof['examples']}, sha(data)
    # The original chapter-8 proof records both modes together. Bind that
    # proof AND the old run options to their published commit before reuse.
    if proof['schema'] != 1 or not commit:
        raise ValueError('legacy evidence requires its published --previous-commit')
    relative = str(path.resolve().relative_to(ROOT))
    result = execute(['git', 'show', f'{commit}:{relative}'], 30)
    result.check_returncode()
    published = result.stdout
    if published != data:
        raise ValueError('legacy evidence differs from its published commit')
    result = execute(['git', 'show', f'{commit}:{MANIFEST}'], 30)
    result.check_returncode()
    manifest = json.loads(result.stdout)
    items = {item['source']: item for item in manifest['examples']}
    rows = {}
    for row in proof['freshly_executed'] + proof['reused']:
        item = items[row['source']]
        rows[row['source']] = dict(row, status='passed', item=item,
                                   compiler_sha256=proof['compiler_sha256'],
                                   runs=[{'mode': mode, 'status': 'passed', 'published_commit': commit}
                                         for mode in item['modes']])
    return rows, sha(data)


def inputs_for(compiler, item):
    result = execute([str(compiler), 'deps', item['source']], 60)
    result.check_returncode()
    graph = json.loads(result.stdout)
    paths = [Path(p).resolve() for key in ('sources', 'descriptors') for p in graph[key]]
    paths.append(ROOT / item['stdout'])
    return {str(p.relative_to(ROOT)): file_sha(p) for p in sorted(set(paths))}


def run_example(compiler, item, mode):
    command = [str(compiler), 'run', item['source']]
    if mode == 'release':
        command.append('--release')
    if 'run_steps' in item:
        command += ['--run-steps', str(item['run_steps'])]
    stack = item.get('stack_kib')
    def limits():
        resource.setrlimit(resource.RLIMIT_STACK, (stack * 1024, resource.getrlimit(resource.RLIMIT_STACK)[1]))
    start = time.monotonic()
    try:
        result = execute(command, item.get('timeout', 60), limits if stack else None)
        expected = (ROOT / item['stdout']).read_bytes()
        passed = (result.returncode == item['exit_status'] and not result.stderr and result.stdout == expected)
        return {'mode': mode, 'status': 'passed' if passed else 'failed',
                'command': command, 'seconds': round(time.monotonic() - start, 3),
                'exit_status': result.returncode, 'stdout_sha256': sha(result.stdout),
                'stderr': result.stderr.decode(errors='replace'), 'stdout_matches': result.stdout == expected}
    except subprocess.TimeoutExpired:
        return {'mode': mode, 'status': 'failed', 'command': command, 'timeout': item.get('timeout', 60)}


def main():
    global DEADLINE
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--coax', type=Path, default=ROOT / 'coax/bin/coax')
    parser.add_argument('--previous', type=Path)
    parser.add_argument('--previous-commit')
    parser.add_argument('--receipt', type=Path, required=True)
    parser.add_argument('--sources', nargs='*', help='qualify just these inventory entries, for bounded slices')
    parser.add_argument('--max-seconds', type=int, default=810, help='whole command limit, 1 to 840 seconds')
    args = parser.parse_args()
    if not 1 <= args.max_seconds <= 840:
        parser.error('--max-seconds must be 1 to 840')
    DEADLINE = time.monotonic() + args.max_seconds
    def terminate(number, frame):
        raise SystemExit(128 + number)
    signal.signal(signal.SIGTERM, terminate)
    compiler = args.coax.resolve()
    compiler_sha = file_sha(compiler)
    manifest_bytes = (ROOT / MANIFEST).read_bytes()
    manifest = json.loads(manifest_bytes)
    selected = set(args.sources) if args.sources else {x['source'] for x in manifest['examples']}
    if selected - {x['source'] for x in manifest['examples']}:
        parser.error('a selected source is outside the closed example inventory')
    old_rows, previous_sha = previous_rows(args.previous, args.previous_commit)
    proof = {'schema': 2, 'status': 'running', 'compiler_sha256': compiler_sha,
             'runner_sha256': file_sha(__file__), 'manifest_sha256': sha(manifest_bytes),
             'previous': str(args.previous) if args.previous else None, 'previous_sha256': previous_sha,
             'previous_commit': args.previous_commit, 'examples': []}
    args.receipt.parent.mkdir(parents=True, exist_ok=True)
    def publish():
        args.receipt.write_text(json.dumps(proof, indent=2) + '\n')
    publish()
    try:
        for item in manifest['examples']:
            if item['source'] not in selected:
                continue
            inputs = inputs_for(compiler, item)
            old = old_rows.get(item['source'])
            row = {'source': item['source'], 'inputs': inputs, 'item': item,
                   'compiler_sha256': compiler_sha, 'status': 'running'}
            proof['examples'].append(row)
            if reusable(old, inputs, item, compiler_sha):
                row.update(status='passed', retained=True, runs=old['runs'])
            else:
                row['retained'] = False
                row['runs'] = []
                for mode in item['modes']:
                    row['runs'].append(run_example(compiler, item, mode))
                    publish()
                    if row['runs'][-1]['status'] != 'passed':
                        raise RuntimeError(f"{item['source']} {mode} failed; see {args.receipt}")
                row['status'] = 'passed'
            # An edit during a compile/run must not acquire a passing receipt.
            if inputs_for(compiler, item) != inputs or file_sha(compiler) != compiler_sha:
                row['status'] = 'failed'
                raise RuntimeError(f"qualification inputs changed: {item['source']}")
            publish()
            print(f"{'retained' if row['retained'] else 'executed'}: {item['source']}", flush=True)
        if (ROOT / MANIFEST).read_bytes() != manifest_bytes or file_sha(__file__) != proof['runner_sha256']:
            raise RuntimeError('qualification manifest or runner changed')
        if file_sha(compiler) != compiler_sha or any(file_sha(ROOT / p) != value
                for row in proof['examples'] for p, value in row['inputs'].items()):
            raise RuntimeError('an earlier qualification input changed before final publication')
        proof['status'] = 'passed'
        proof['executed_runs'] = sum(len(r['runs']) for r in proof['examples'] if not r['retained'])
        proof['retained_runs'] = sum(len(r['runs']) for r in proof['examples'] if r['retained'])
    except BaseException:
        proof['status'] = 'failed'
        raise
    finally:
        publish()


if __name__ == '__main__':
    main()
