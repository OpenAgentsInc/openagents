#!/usr/bin/env python3
# Firm reason: 2 (independent process/I/O oracle for the native ABI, no model inference).
from pathlib import Path
import array, datetime, fcntl, hashlib, json, os, select, signal
import subprocess, sys, termios, time

binary, wav, root, dest = map(Path, sys.argv[1:5])
dest.mkdir(parents=True, exist_ok=True)
def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()
inputs = [binary, wav, root/'psionic/platform/parakeet-file.c',
          root/'psionic/tests/parakeet-file-effects-fixture.c',
          root/'psionic/build/parakeet/parakeet-file.c', Path(__file__)]
before = {str(p): sha(p) for p in inputs}
env = os.environ.copy()
env['PKFILE_FIXTURE_LARGE'] = '1'
for key in ['PKFILE_FIXTURE_SILENT', 'PKFILE_FIXTURE_FAIL_AT', 'PKFILE_FIXTURE_CANCEL_AT']:
    env.pop(key, None)
argv = [str(binary), 'transcribe', 'fixture-model', str(wav), '--json']
receipt = {'schema': 'psionic.parakeet.file-slow-pipe-cancel.v1',
           'scope': 'Actual production file-main ABI linked to injected model effects; no model loaded or inference performed.',
           'argv': argv, 'environment_overrides': {'PKFILE_FIXTURE_LARGE': '1'},
           'input_sha256': before, 'started_utc': datetime.datetime.now(datetime.timezone.utc).isoformat()}
process = None
passed = False
try:
    with (dest/'stderr.txt').open('wb') as errors:
        start = time.monotonic()
        process = subprocess.Popen(argv, stdout=subprocess.PIPE, stderr=errors,
                                   stdin=subprocess.DEVNULL, env=env, cwd=root)
        ready = select.select([process.stdout], [], [], 20)[0]
        if not ready:
            raise AssertionError('No publication began within 20 seconds')
        first = os.read(process.stdout.fileno(), 1)
        receipt['first_stdout_byte_hex'] = first.hex()
        receipt['publication_started_ms'] = (time.monotonic()-start)*1000
        if first != b'{':
            raise AssertionError('Expected the start of the spooled JSON publication')
        # Stop reading. Nine large injected transcripts exceed the pipe buffer.
        time.sleep(0.3)
        pending = array.array('i', [0])
        fcntl.ioctl(process.stdout.fileno(), termios.FIONREAD, pending, True)
        receipt['queued_stdout_bytes_before_signal'] = pending[0]
        receipt['alive_before_signal'] = process.poll() is None
        if not receipt['alive_before_signal'] or pending[0] < 4096:
            raise AssertionError('The process was not blocked with queued publication bytes')
        signalled = time.monotonic()
        process.send_signal(signal.SIGINT)
        receipt['signal'] = 'SIGINT'
        try:
            code = process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            receipt['whole_process_timeout'] = True
            raise AssertionError('The whole process failed to exit within five seconds')
        receipt['signal_to_exit_ms'] = (time.monotonic()-signalled)*1000
        receipt['exit_status'] = code
        receipt['whole_process_timeout'] = False
        output = first + process.stdout.read()
    (dest/'stdout.txt').write_bytes(output)
    errors = (dest/'stderr.txt').read_text()
    receipt['stdout_bytes'] = len(output)
    receipt['stdout_sha256'] = hashlib.sha256(output).hexdigest()
    receipt['partial_output_diagnostic'] = 'transcription cancelled during output; output may be partial' in errors
    try:
        json.loads(output)
        receipt['complete_json_published'] = True
    except (ValueError, UnicodeDecodeError):
        receipt['complete_json_published'] = False
    receipt['identities_unchanged'] = before == {str(p):sha(p) for p in inputs}
    passed = (code == 130 and receipt['partial_output_diagnostic'] and
              not receipt['complete_json_published'] and receipt['identities_unchanged'])
    if not passed:
        raise AssertionError('Cancellation status, partial-output policy or source identity check failed')
except Exception as error:
    receipt['error'] = str(error)
finally:
    if process is not None and process.poll() is None:
        process.kill()
        process.wait(timeout=3)
        receipt['terminated_owned_child'] = True
    receipt['passed'] = passed
    (dest/'receipt.json').write_text(json.dumps(receipt, indent=2)+'\n')
    print(json.dumps({k:receipt.get(k) for k in ['passed','exit_status','signal_to_exit_ms','queued_stdout_bytes_before_signal','stdout_bytes','error']}))
sys.exit(0 if passed else 1)
