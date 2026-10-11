"""Bound one explicitly handed-off phase; never start a model or server."""
import argparse
import json
import os
from pathlib import Path
import signal
import shutil
import subprocess
import sys
import time
from common import ROOT, WORK, REPO, freeze_check, sha, successful_phase

parser = argparse.ArgumentParser()
parser.add_argument('phase', choices=['stage', 'build', 'test'])
args = parser.parse_args()
leases = set(os.environ.get('OPENAGENTS_LEASES', '').split(','))
required = {'memory', 'disk'} | ({'build'} if args.phase == 'build' else set())
assert required <= leases and not {'quiet', 'gpu'} & leases, 'required leases absent or forbidden quiet/GPU lease present'
frozen = freeze_check()
output = ROOT / (args.phase + '-1')
output.mkdir(exist_ok=False)
cap = (2 if args.phase == 'test' else 4) * 1024**3
floor = 12 * 1024**3
disk_cap = 4 * 1024**3
disk_floor = 25 * 1024**3
timeout = {'stage': 300, 'build': 900, 'test': 60}[args.phase]
sys.path.insert(0, str(REPO / 'crates/psionic/fixtures/clef/tools'))
from mac_quiet_bench import OwnedProcessGuard, available_memory, stop_owned_group

record = {'phase': args.phase, 'status': 'preflight', 'cap_bytes': cap,
          'minimum_start_bytes': cap + floor, 'floor_bytes': floor,
          'disk_cap_bytes': disk_cap, 'free_disk_floor_bytes': disk_floor, 'timeout_seconds': timeout,
          'network_enabled': args.phase == 'stage', 'model_or_server_started': False,
          'no_quiet_lease': True, 'source_freeze_sha256': sha(ROOT / 'frozen-sources.json'),
          'source_identity': frozen, 'disk_sampling_seconds': 1,
          'disk_scope': 'work plus workflow receipts; symlink targets excluded',
          'disk_bound_kind': 'sampled logical bytes; overshoot fails and is retained'}


def save():
    (output / 'receipt.json').write_text(json.dumps(record, indent=2, allow_nan=False) + '\n')


def used_bytes():
    total = 0
    for base, dirs, files in os.walk(ROOT, followlinks=False):
        dirs[:] = [d for d in dirs if not (Path(base) / d).is_symlink()]
        for name in files:
            path = Path(base) / name
            try:
                info = path.lstat()
            except FileNotFoundError:
                continue  # Go may remove a temporary file during this sample.
            total += info.st_size
    return total


def free_disk():
    free = shutil.disk_usage(ROOT).free
    record['minimum_free_disk_bytes'] = min(record.get('minimum_free_disk_bytes', free), free)
    if free < disk_floor:
        raise OSError('free disk space crossed the fixed 25 GiB floor')
    return free


def interrupted(signum, _frame):
    raise KeyboardInterrupt(f'signal {signum}')


signal.signal(signal.SIGTERM, interrupted)
save()
process = monitor = None
began = time.monotonic()
last_disk = 0.0
try:
    previous = {'stage': [], 'build': ['stage'], 'test': ['stage', 'build']}[args.phase]
    record['prerequisites'] = {phase: successful_phase(phase) for phase in previous}
    record['free_disk_before_bytes'] = free_disk()
    record['memory_before'] = available_memory()
    if record['memory_before']['available_memory_bytes'] < cap + floor:
        raise MemoryError('insufficient memory before this phase')
    record['disk_before_bytes'] = used_bytes()
    if record['disk_before_bytes'] >= disk_cap:
        raise OSError('scratch storage cap already reached')
    script = {'stage': 'stage.py', 'build': 'build.py', 'test': 'run_tests.py'}[args.phase]
    command = [sys.executable, str(ROOT / script)]
    env = dict(os.environ, PYTHONDONTWRITEBYTECODE='1')
    env = {k: v for k, v in env.items() if not k.startswith('DYLD_') and k not in {'LD_PRELOAD', 'LD_LIBRARY_PATH', 'PYTHONOPTIMIZE'}}
    record['command'] = command
    save()
    with (output / 'stdout.log').open('xb') as stdout, (output / 'stderr.log').open('xb') as stderr:
        process = subprocess.Popen(command, env=env, stdin=subprocess.DEVNULL,
                                   stdout=stdout, stderr=stderr, start_new_session=True)
        monitor = OwnedProcessGuard(process, cap, output / 'memory.jsonl', minimum_available_memory_bytes=floor)
        with (output / 'disk.jsonl').open('x') as disk:
            while process.poll() is None:
                monitor.guard()
                elapsed = time.monotonic() - began
                if elapsed > timeout:
                    raise TimeoutError('fixed phase deadline exceeded')
                if elapsed - last_disk >= 1:
                    used = used_bytes()
                    free = free_disk()
                    disk.write(json.dumps({'elapsed_seconds': elapsed, 'logical_bytes': used, 'free_disk_bytes': free}) + '\n')
                    disk.flush()
                    if used > disk_cap:
                        raise OSError('scratch storage exceeded its fixed cap')
                    last_disk = elapsed
                time.sleep(.1)
        monitor.guard()
        record.update(status='complete' if process.returncode == 0 else 'failed', exit=process.returncode)
        assert freeze_check() == frozen, 'source changed during phase'
        assert record['prerequisites'] == {phase: successful_phase(phase) for phase in previous}, 'prerequisite changed during phase'
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
    try:
        record['disk_after_bytes'] = used_bytes()
        record['free_disk_after_bytes'] = free_disk()
        if record['disk_after_bytes'] > disk_cap:
            record.update(status='failed', disk_error='scratch storage cap exceeded')
    except BaseException as error:
        record.update(status='failed', disk_sampling_error=str(error))
    if record['status'] == 'complete':
        try:
            name = {'stage': 'stage.json', 'build': 'build.json', 'test': 'tests.json'}[args.phase]
            artifact = json.loads((WORK / name).read_text())
            assert artifact['frozen_sources_sha256'] == record['source_freeze_sha256']
            record['phase_artifact'] = {'path': 'work/' + name, 'sha256': sha(WORK / name)}
        except BaseException as error:
            record.update(status='failed', artifact_error=str(error))
    record['seconds'] = time.monotonic() - began
    save()
print(json.dumps(record), flush=True)
raise SystemExit(0 if record['status'] == 'complete' else 1)
