"""Run the frozen native or browser benchmark under outer quiet/GPU leases."""
from functools import partial
import hashlib
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import subprocess
import signal
import sys
import threading
import time

BASE = Path(__file__).resolve().parent
ROOT = BASE / 'water-w11'
SOURCE = '499b25d0839a01dbbbc524e1374b1ee5c3e52644'
BROWSER_SOURCE = '499b25d0839a01dbbbc524e1374b1ee5c3e52644'
WORKTREE_SOURCE = '499b25d0839a01dbbbc524e1374b1ee5c3e52644'
assert subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip() == WORKTREE_SOURCE
changed = subprocess.check_output(['git', 'diff', '--name-only', SOURCE, 'HEAD'], cwd=ROOT, text=True).splitlines()
assert changed == [], changed
assert not subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT, text=True).strip()
space = os.statvfs(BASE)
assert space.f_bavail * space.f_frsize > 25 * 10**9
assert 'quiet' in os.environ.get('OPENAGENTS_LEASES', ''), 'Take the quiet lease first'
assert 'gpu' in os.environ.get('OPENAGENTS_LEASES', ''), 'Take the GPU lease first'
mode = sys.argv[1]
case = os.environ.get('WATER_W11_CASE')
suffix = mode + ('-' + case.replace('/', '-') if case else '')
output = BASE / ('w11-metrics-' + SOURCE) / suffix
output.mkdir(parents=True, exist_ok=True)
started = time.time()


def run(command, **kwargs):
    process = subprocess.Popen(command, start_new_session=True, **kwargs)
    try:
        process.wait(timeout=900)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGTERM)
        try:
            process.wait(timeout=15)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.wait()
    return process


if mode == 'native':
    record = json.loads((BASE / ('w11-native-' + SOURCE) / 'artifact.json').read_text())
    executable = Path(record['original_path'])
    assert record['source_sha'] == SOURCE and record['compiler_exit'] == 0
    assert executable.stat().st_size == record['bytes']
    assert hashlib.sha256(executable.read_bytes()).hexdigest() == record['sha256']
    (output / 'native-inputs.json').write_text(json.dumps(record, indent=2) + '\n')
    environment = os.environ.copy()
    environment['WATER_W11_OUTPUT'] = str(output)
    command = [str(executable), 'w11::water_w11_fixed_views', '--ignored', '--exact',
               '--test-threads=1', '--nocapture']
    with (output / 'run.stdout').open('w') as stdout, (output / 'run.stderr').open('w') as stderr:
        result = run(command, env=environment, stdout=stdout, stderr=stderr)
    assert hashlib.sha256(executable.read_bytes()).hexdigest() == record['sha256']
elif mode == 'browser':
    identity = json.loads((BASE / 'w11-product-byte-identity-499b25d083.json').read_text())
    assert identity['browser_compile_source'] == BROWSER_SOURCE
    assert identity['native_compile_source'] == SOURCE and identity['source_equivalence_verified']
    site = BASE / ('w11-wasm-' + BROWSER_SOURCE) / 'site'

    class Handler(SimpleHTTPRequestHandler):
        def log_message(self, *_):
            pass

    server = ThreadingHTTPServer(('127.0.0.1', 0), partial(Handler, directory=str(site)))
    server.daemon_threads = True
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    origin = 'http://127.0.0.1:' + str(server.server_port)
    command = ['openagents', 'browser', 'run', '--json', '--', 'python3',
               str(ROOT / 'bench/verse/2026-10-08/water-w11/browser.py'), origin, str(output)]
    try:
        with (output / 'run.stdout').open('w') as stdout, (output / 'run.stderr').open('w') as stderr:
            result = run(command, cwd=ROOT, stdout=stdout, stderr=stderr)
    finally:
        server.shutdown()
        server.server_close()
        thread.join()
else:
    raise ValueError(mode)
receipt = {'source_sha':SOURCE, 'worktree_sha':WORKTREE_SOURCE, 'worktree_documentation_only_delta':changed, 'command':command, 'exit':result.returncode,
           'selected_case':case, 'selected_browser_pairs':os.environ.get('WATER_W11_BROWSER_CASES'),
           'started_at_unix_ms':round(started * 1000), 'ended_at_unix_ms':round(time.time() * 1000),
           'wrapper_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
(output / 'run.json').write_text(json.dumps(receipt, indent=2) + '\n')
print(json.dumps(receipt), flush=True)
print((output / 'run.stdout').read_text()[-16000:], flush=True)
print((output / 'run.stderr').read_text()[-8000:], flush=True)
sys.exit(result.returncode)
