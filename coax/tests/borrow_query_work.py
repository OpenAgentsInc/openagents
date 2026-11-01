#!/usr/bin/env python3
"""Whole-file bounded memo-work regression; maintenance tooling only."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[2]
SOURCE = Path(__file__).with_name('borrow-query-work') / 'many-independent-returns.coax'
sys.path.insert(0, str(ROOT / 'coax/tools'))
from bounded_process import run


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--compiler', default=os.environ.get('COAX_TEST_COMPILER', str(ROOT / 'coax/bin/coax')))
    parser.add_argument('--record-dir')
    args = parser.parse_args()
    compiler = Path(args.compiler).resolve()
    directory = Path(args.record_dir).resolve() if args.record_dir else Path(tempfile.mkdtemp(prefix='coax-borrow-query-work-'))
    assert not directory.is_relative_to(ROOT), 'keep observations outside source inputs'
    if args.record_dir:
        directory.mkdir(parents=True, exist_ok=False)
    inputs = [SOURCE, Path(__file__).resolve(), ROOT / 'coax/tools/bounded_process.py', ROOT / 'coax/tests/quiet_crash.py', compiler]
    before = {str(path): sha(path) for path in inputs}
    argv = [str(compiler), 'check', '--json', str(SOURCE)]
    observation = run(argv, cwd=str(ROOT), env=os.environ.copy(), timeout_s=60, max_output_bytes=1048576, deadline=time.monotonic() + 60, quiet_faults=True)
    for stream in ['stdout', 'stderr']:
        raw = observation.pop(stream)
        path = directory / (stream + '.log')
        path.write_bytes(raw)
        observation[stream] = {'path': str(path), 'bytes': len(raw), 'sha256': sha(path)}
    after = {str(path): sha(path) for path in inputs}
    flags = ['timeout', 'deadline_exhausted', 'stdout_limit', 'stderr_limit', 'output_limit', 'group_killed', 'kill_errno', 'launch_errno', 'launch_error', 'read_errno']
    passed = observation['started'] and observation['returncode'] == 0 and observation['stdout_eof'] and observation['stderr_eof'] and not any(observation.get(key) for key in flags) and observation['stdout']['bytes'] == 0 and observation['stderr']['bytes'] == 0 and before == after
    record = {'schema': 'coax.borrow-query-work.source.v1', 'scope': 'One original whole-file acceptance regression with1000 independent caller-string-return procedures; no budget flags or source rewriting, no native execution or broad ownership claim.', 'outcome': 'passed' if passed else 'failed', 'expected': 'accepted with no diagnostics under normal limits', 'compiler': str(compiler), 'compiler_sha256': before[str(compiler)], 'source_before': before, 'source_after': after, 'inputs_unchanged': before == after, 'observation': observation}
    path = directory / 'qualification.json'
    path.write_text(json.dumps(record, indent=2) + '\n')
    print(json.dumps({'outcome': record['outcome'], 'elapsed_s': observation['elapsed_s'], 'record': str(path)}))
    return 0 if passed else 1


if __name__ == '__main__':
    sys.exit(main())
