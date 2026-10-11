#!/usr/bin/env python3
"""Bound one task-owned synthetic child. Never controls unrelated processes."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

root = Path(__file__).resolve().parent
repo = Path(os.environ['CLEF_REPO'])
sys.path.insert(0, str(repo / 'crates/psionic/fixtures/clef/tools'))
from mac_quiet_bench import OwnedProcessGuard, available_memory

parser = argparse.ArgumentParser()
parser.add_argument('mode', choices=['build', 'check', 'bench'])
parser.add_argument('--output', required=True, type=Path)
args = parser.parse_args()
args.output = (root / args.output).resolve()
if root not in args.output.parents:
    raise SystemExit('output must remain in this task scratch directory')
leases = set(os.environ.get('OPENAGENTS_LEASES', '').split(','))
required = {'build', 'memory'} if args.mode == 'build' else {'gpu', 'memory'}
if not required.issubset(leases):
    raise SystemExit(f'requires {sorted(required)} leases')
if 'quiet' in leases:
    raise SystemExit('this experiment must not use a quiet lease')
args.output.mkdir(parents=True, exist_ok=False)
cap = (4 if args.mode == 'build' else 2) * 1024**3
floor = 12 * 1024**3
preflight = available_memory()
record = {'mode': args.mode, 'status': 'preflight', 'preflight': preflight,
          'max_owned_group_bytes': cap, 'minimum_available_bytes': floor,
          'minimum_start_bytes': cap + floor, 'deadline_seconds': (180 if args.mode == 'build' else 60),
          'conditions': 'shared machine; fixed two-SIMD-group full-K Flash projections; no model execution',
          'source_sha256': {name: hashlib.sha256((root / name).read_bytes()).hexdigest()
                            for name in ['baseline.metal', 'candidate.metal', 'production-shapes.metal', 'production-fixed-extract.metal', 'production-silu-extract.metal', 'production-planner.rs', 'probe.swift', 'weights.json', 'build.py', 'guard.py', 'manifest.json', 'summarize.py']},
          'guard_dependency_sha256': {name: hashlib.sha256((repo / 'crates/psionic/fixtures/clef/tools' / name).read_bytes()).hexdigest()
                                      for name in ['mac_quiet_bench.py', 'mac_memory.py']}}
def save():
    (args.output / 'receipt.json').write_text(json.dumps(record, indent=2) + '\n')
save()
if preflight['available_memory_bytes'] < cap + floor:
    record.update(status='refused', reason='insufficient available memory before launch')
    save()
    raise SystemExit(1)
command = ([sys.executable, str(root / 'build.py')] if args.mode == 'build' else
           [str(root / 'probe'), str(root), args.mode, str(root.parent / 'gemm-tile-probe')])
record['command'] = command
if args.mode != 'build':
    record['build_artifacts'] = json.loads((root / 'build-artifacts.json').read_text())
    expected = record['build_artifacts']
    if expected['source_sha256'] != record['source_sha256']:
        record.update(status='refused', reason='source hashes changed since build')
        save()
        raise SystemExit(1)
    for name, sha in expected['artifacts'].items():
        if hashlib.sha256((root / name).read_bytes()).hexdigest() != sha:
            record.update(status='refused', reason='built artifact hash changed')
            save()
            raise SystemExit(1)
environment = dict(os.environ, RAYON_NUM_THREADS='2', OMP_NUM_THREADS='2',
                   OPENBLAS_NUM_THREADS='2', VECLIB_MAXIMUM_THREADS='2')
environment = {key: value for key, value in environment.items()
               if not key.startswith('DYLD_') and key not in {'LD_PRELOAD', 'LD_LIBRARY_PATH'}}
process = guard = None
began = time.monotonic()
def interrupt(signum, _frame):
    raise KeyboardInterrupt(f'signal {signum}')
signal.signal(signal.SIGTERM, interrupt)
try:
    with (args.output / 'stdout.jsonl').open('wb') as out, (args.output / 'stderr.log').open('wb') as err:
        process = subprocess.Popen(command, env=environment, stdin=subprocess.DEVNULL,
                                   stdout=out, stderr=err, start_new_session=True)
        guard = OwnedProcessGuard(process, cap, args.output / 'memory.jsonl',
                                  minimum_available_memory_bytes=floor)
        while process.poll() is None:
            guard.guard()
            if time.monotonic() - began > record['deadline_seconds']:
                raise TimeoutError('synthetic probe exceeded its bounded deadline')
            time.sleep(.2)
        guard.guard()
        record.update(status='complete' if process.returncode == 0 else 'failed', exit=process.returncode)
except BaseException as error:
    record.update(status='failed', error_type=type(error).__name__, error=str(error))
finally:
    if guard is not None:
        try:
            guard.close()
        except BaseException as error:
            record.update(status='failed', cleanup_error=str(error))
        record['memory'] = guard.receipt()
    elif process is not None:
        # Construction failure must still clean up only this new child group.
        from mac_quiet_bench import stop_owned_group
        try:
            stop_owned_group(process)
        except BaseException as error:
            record.update(status='failed', cleanup_error=str(error))
    record['seconds'] = time.monotonic() - began
    save()
print(json.dumps(record), flush=True)
raise SystemExit(0 if record['status'] == 'complete' else 1)
