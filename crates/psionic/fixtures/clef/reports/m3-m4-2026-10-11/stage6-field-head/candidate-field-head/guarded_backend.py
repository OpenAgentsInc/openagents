import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time

REPO = Path('workspace/openagents')
ROOT = Path(__file__).parent
sys.path.insert(0, str(REPO / 'crates/psionic/fixtures/clef/tools'))
from mac_quiet_bench import OwnedProcessGuard, available_memory, file_sha

FILES = [
    'clef_prefill.rs', 'clef_prefill/head_blocks.rs', 'clef_prefill/head_blocks/tests.rs',
    'kernels/clef_prefill.metal', 'kernels/clef_dequant_q6_k.metal', 'kernels/clef_head_blocks.metal',
]
SOURCE = REPO / 'crates/psionic/crates/psionic-backend-metal/src'
manifest = {str((SOURCE / name).relative_to(REPO)): file_sha(SOURCE / name) for name in FILES}
base = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=REPO, text=True).strip()
patch = subprocess.check_output(['git', 'diff', '--', *manifest], cwd=REPO)
(ROOT / 'candidate.patch').write_bytes(patch)
for name in FILES:
    destination = ROOT / 'sources' / name
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(SOURCE / name, destination)
(ROOT / 'source.json').write_text(json.dumps({
    'base_commit': base, 'sources': manifest, 'candidate_patch_sha256': hashlib.sha256(patch).hexdigest(),
    'static_check': {'command': ['xcrun', '-sdk', 'macosx', 'metal', '-std=metal4.0',
                                '-c', 'combined-head.metal'], 'exit': 0,
                     'air_sha256': file_sha(ROOT / 'combined-head.air')},
}, indent=2) + '\n')
before = available_memory()
record = {'status': 'preflight', 'preflight': before, 'cpu_only': True,
          'max_rss_bytes': 4 * 1024**3, 'minimum_available_memory_bytes': 12 * 1024**3,
          'startup_available_memory_bytes': 16 * 1024**3, 'deadline_seconds': 1800}
if before['available_memory_bytes'] < 16 * 1024**3:
    record.update(status='refused', reason='less than 16 GiB available')
    (ROOT / 'guard.json').write_text(json.dumps(record, indent=2) + '\n')
    raise SystemExit('build preflight refused')
env = {key: value for key, value in os.environ.items() if not key.startswith('PSIONIC_CLEF_')}
env.update(CARGO_TARGET_DIR='user-home/work/openagents-target-agent1', CARGO_BUILD_JOBS='2')
command = ['cargo', 'test', '--release', '--manifest-path', 'crates/psionic/Cargo.toml',
           '-p', 'psionic-backend-metal', '--lib', 'clef', '--message-format=json',
           '--', '--test-threads=1']
record['command'] = command
record['environment'] = {key: env[key] for key in ['CARGO_TARGET_DIR', 'CARGO_BUILD_JOBS']}
start = time.monotonic()
with (ROOT / 'backend.jsonl').open('w') as stdout, (ROOT / 'backend.stderr.log').open('w') as stderr:
    process = subprocess.Popen(command, cwd=REPO, env=env, stdout=stdout, stderr=stderr, start_new_session=True)
    guard = OwnedProcessGuard(process, 4 * 1024**3, ROOT / 'memory.jsonl',
                              minimum_available_memory_bytes=12 * 1024**3)
    try:
        while process.poll() is None:
            guard.guard()
            if time.monotonic() - start > 1800:
                raise TimeoutError('backend check exceeded 1800 seconds')
            time.sleep(0.5)
        guard.guard()
        record.update(status='complete' if process.returncode == 0 else 'failed', exit=process.returncode)
    except BaseException as error:
        record.update(status='failed', error=str(error))
    finally:
        try:
            guard.close()
        except BaseException as error:
            record.update(status='failed', cleanup_error=str(error))
        record['memory'] = guard.receipt()
        record['seconds'] = time.monotonic() - start
after = {str((SOURCE / name).relative_to(REPO)): file_sha(SOURCE / name) for name in FILES}
record['source_unchanged'] = manifest == after
if not record['source_unchanged']:
    record.update(status='failed', reason='source changed during compilation')
for line in (ROOT / 'backend.jsonl').read_text().splitlines():
    if not line.startswith('{'):
        continue
    row = json.loads(line)
    if row.get('reason') == 'compiler-artifact' and row.get('target', {}).get('name') == 'psionic_backend_metal' and row.get('executable'):
        executable = Path(row['executable'])
        saved = ROOT / 'psionic-backend-metal-tests'
        shutil.copy2(executable, saved)
        record['test_binary'] = {'path': str(saved), 'sha256': file_sha(saved), 'bytes': saved.stat().st_size,
                                 'source_executable': str(executable)}
(ROOT / 'guard.json').write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps(record), flush=True)
raise SystemExit(0 if record['status'] == 'complete' else 1)
