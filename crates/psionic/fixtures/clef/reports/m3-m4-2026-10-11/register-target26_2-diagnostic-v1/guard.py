#!/usr/bin/env python3
"""Run one bounded diagnostic child and stop only its owned process group."""
import argparse
import json
import os
import signal
import subprocess
import sys
import time
from pathlib import Path
from common import ROOT, REPO, sha, verify

parser = argparse.ArgumentParser()
parser.add_argument('mode', choices=['build', 'export'])
parser.add_argument('--output', required=True, type=Path)
args = parser.parse_args()
output = (ROOT / args.output).resolve()
if ROOT not in output.parents:
    raise SystemExit('output must remain in this diagnostic directory')
leases = set(os.environ.get('OPENAGENTS_LEASES', '').split(','))
required = {'build', 'memory'} if args.mode == 'build' else {'gpu', 'memory'}
if not required.issubset(leases) or 'quiet' in leases:
    raise SystemExit('required build/GPU and memory leases are missing, or quiet lease is present')
frozen, manifest = verify()
sys.path.insert(0, str(REPO / 'crates/psionic/fixtures/clef/tools'))
from mac_quiet_bench import OwnedProcessGuard, available_memory, stop_owned_group

output.mkdir(parents=True, exist_ok=False)
cap = (4 if args.mode == 'build' else 2) * 1024**3
floor = 12 * 1024**3
deadline = 180 if args.mode == 'build' else 30
record = {'identity': manifest['identity'], 'mode': args.mode, 'status': 'preflight',
          'max_owned_group_bytes': cap, 'minimum_available_bytes': floor,
          'minimum_start_bytes': cap + floor, 'deadline_seconds': deadline,
          'frozen_sources_sha256': sha(ROOT / 'frozen-sources.json'), **frozen,
          'library_sha256': None, 'sdk_header_sha256': manifest['sdk_header_sha256'],
          'no_quiet_lease': True, 'gemm_or_model_execution': False}
def save():
    (output / 'receipt.json').write_text(json.dumps(record, indent=2) + '\n')
save()
try:
    preflight = available_memory()
except BaseException as error:
    record.update(status='refused', error_type=type(error).__name__, error=str(error)); save(); raise
record['preflight'] = preflight
if preflight['available_memory_bytes'] < cap + floor:
    record.update(status='refused', reason='insufficient available memory before launch'); save(); raise SystemExit(1)
command = [sys.executable, str(ROOT / 'build.py')]
if args.mode == 'export':
    built = json.loads((ROOT / 'build-artifacts.json').read_text())
    if (built['source_sha256'] != frozen['source_sha256'] or
        built['frozen_sources_sha256'] != sha(ROOT / 'frozen-sources.json') or
        built['candidate_source_unchanged'] is not True or
        any(sha(ROOT / name) != digest for name, digest in built['artifacts'].items()) or
        built['library_sha256'] != sha(ROOT / 'target26_2.metallib')):
        record.update(status='refused', reason='host build identity differs'); save(); raise SystemExit(1)
    record['build_artifacts'] = built
    record['library_sha256'] = built['library_sha256']
    command = [str(ROOT / 'exporter-target26_2'), str(ROOT / 'target26_2.metallib'), str(output)]
record['command'] = command
save()
environment = dict(os.environ, RAYON_NUM_THREADS='2', OMP_NUM_THREADS='2',
                   OPENBLAS_NUM_THREADS='2', VECLIB_MAXIMUM_THREADS='2', SWIFTPM_MAXIMUM_CONCURRENT_JOBS='2')
environment = {key: value for key, value in environment.items()
               if not key.startswith('DYLD_') and key not in {'LD_PRELOAD', 'LD_LIBRARY_PATH'}}
process = guard = None
started = time.monotonic()
def interrupt(signum, _frame):
    raise KeyboardInterrupt(f'signal {signum}')
signal.signal(signal.SIGTERM, interrupt)
try:
    with (output / 'stdout.jsonl').open('xb') as out, (output / 'stderr.log').open('xb') as err:
        process = subprocess.Popen(command, env=environment, stdin=subprocess.DEVNULL,
                                   stdout=out, stderr=err, start_new_session=True)
        guard = OwnedProcessGuard(process, cap, output / 'memory.jsonl', minimum_available_memory_bytes=floor)
        while process.poll() is None:
            guard.guard()
            if time.monotonic() - started > deadline:
                raise TimeoutError('diagnostic exceeded its fixed deadline')
            time.sleep(.1)
        guard.guard()
        record.update(status='complete' if process.returncode == 0 else 'failed', exit=process.returncode)
        if verify() != (frozen, manifest):
            raise ValueError('diagnostic inputs changed during run')
except BaseException as error:
    record.update(status='failed', error_type=type(error).__name__, error=str(error))
finally:
    if guard is not None:
        try:
            guard.close()
        except BaseException as error:
            record.update(status='failed', cleanup_error=str(error))
        record['memory'] = guard.receipt()
        if guard.failure:
            record.update(status='failed', monitor_failure=guard.failure)
    elif process is not None:
        try:
            stop_owned_group(process)
        except BaseException as error:
            record.update(status='failed', cleanup_error=str(error))
    record['seconds'] = time.monotonic() - started
    save()
print(json.dumps(record), flush=True)
raise SystemExit(0 if record['status'] == 'complete' else 1)
