#!/usr/bin/env python3
"""Bound one owned build or probe group; leave unrelated processes alone."""
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
parser.add_argument('mode', choices=['build', 'bench'])
parser.add_argument('--output', required=True, type=Path)
args = parser.parse_args()
output = (ROOT / args.output).resolve()
if ROOT not in output.parents:
    raise SystemExit('output must remain under this probe directory')
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
deadline = 180 if args.mode == 'build' else 60
record = {'identity': manifest['identity'], 'mode': args.mode, 'status': 'preflight',
          'max_owned_group_bytes': cap, 'minimum_available_bytes': floor,
          'minimum_start_bytes': cap + floor, 'deadline_seconds': deadline,
          'conditions': 'shared machine; resident Flash FFN scheduling probe; no model execution',
          'frozen_sources_sha256': sha(ROOT / 'frozen-sources.json'), **frozen,
          'no_quiet_lease': True, 'model_execution': False, 'load_before': os.getloadavg()}

def save():
    (output / 'receipt.json').write_text(json.dumps(record, indent=2) + '\n')
save()
process = monitor = None
began = time.monotonic()
def interrupt(signum, _frame):
    raise KeyboardInterrupt(f'signal {signum}')
signal.signal(signal.SIGTERM, interrupt)
try:
    record['preflight'] = available_memory()
    if record['preflight']['available_memory_bytes'] < cap + floor:
        raise MemoryError('insufficient available memory before launch')
    command = [sys.executable, str(ROOT / 'build.py')]
    if args.mode == 'bench':
        built = json.loads((ROOT / 'build-artifacts.json').read_text())
        assert built['source_sha256'] == frozen['source_sha256'], 'build source identity differs'
        assert built['guard_dependency_sha256'] == frozen['guard_dependency_sha256'], 'build guard differs'
        assert built['frozen_sources_sha256'] == sha(ROOT / 'frozen-sources.json'), 'build freeze differs'
        assert set(built['artifacts']) == {'shapes.air', 'pointwise.air', 'kernels.metallib', 'probe'}, 'build artifact set differs'
        assert all(sha(ROOT / name) == digest for name, digest in built['artifacts'].items()), 'build artifact changed'
        record['build_artifacts'] = built
        command = [str(ROOT / 'probe'), str(ROOT), str(ROOT.parent / 'gemm-tile-probe')]
    record['command'] = command; save()
    environment = dict(os.environ, RAYON_NUM_THREADS='2', OMP_NUM_THREADS='2',
                       OPENBLAS_NUM_THREADS='2', VECLIB_MAXIMUM_THREADS='2', SWIFTPM_MAXIMUM_CONCURRENT_JOBS='2')
    environment = {k: v for k, v in environment.items()
                   if not k.startswith('DYLD_') and k not in {'LD_PRELOAD', 'LD_LIBRARY_PATH', 'PYTHONOPTIMIZE'}}
    with (output / 'stdout.jsonl').open('xb') as stdout, (output / 'stderr.log').open('xb') as stderr:
        process = subprocess.Popen(command, env=environment, stdin=subprocess.DEVNULL,
                                   stdout=stdout, stderr=stderr, start_new_session=True)
        monitor = OwnedProcessGuard(process, cap, output / 'memory.jsonl', minimum_available_memory_bytes=floor)
        while process.poll() is None:
            monitor.guard()
            if time.monotonic() - began > deadline:
                raise TimeoutError('probe exceeded its fixed deadline')
            time.sleep(.1)
        monitor.guard()
        record.update(status='complete' if process.returncode == 0 else 'failed', exit=process.returncode)
        assert verify() == (frozen, manifest), 'source changed during run'
except BaseException as error:
    record.update(status='failed', error_type=type(error).__name__, error=str(error))
finally:
    if monitor is not None:
        try:
            monitor.close()
        except BaseException as error:
            record.update(status='failed', cleanup_error=str(error))
        record['memory'] = monitor.receipt()
        if monitor.failure:
            record.update(status='failed', monitor_failure=monitor.failure)
    elif process is not None:
        try:
            stop_owned_group(process)
        except BaseException as error:
            record.update(status='failed', cleanup_error=str(error))
    record['seconds'] = time.monotonic() - began
    record['load_after'] = os.getloadavg()
    save()
print(json.dumps(record), flush=True)
raise SystemExit(0 if record['status'] == 'complete' else 1)
