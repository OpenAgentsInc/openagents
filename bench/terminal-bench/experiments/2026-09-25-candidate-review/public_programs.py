#!/usr/bin/env python3
"""Generate public checks or execute them in bounded, networkless containers."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time
import uuid

ENTRY = '''import json, runpy
try:
    data=json.load(open('/work/input.json'))
    program=runpy.run_path('/work/check.py')
    result=program['check'](data['candidate'],data['provided'])
    print(json.dumps({'observations':result}))
except BaseException as e:
    print(json.dumps({'error':type(e).__name__+': '+str(e)[:1000]}))
'''

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('mode', choices=['generate', 'run'])
p.add_argument('--manifest', type=Path, required=True)
p.add_argument('--partition', choices=['calibration', 'held-out', 'prospective'], required=True)
p.add_argument('--binary', type=Path)
p.add_argument('--programs', type=Path)
p.add_argument('--image', help='Pinned local Docker image ID, required for execution')
a = p.parse_args()
rows = [r for r in json.loads(a.manifest.read_text()) if r['split'] == a.partition]


def generate(row):
    source = Path(row['input'])
    out = source.parent / 'generated'
    if (out / 'program.json').exists():
        return row['task'], 'retained'
    out.mkdir(exist_ok=True)
    with (out / 'process.log').open('w') as log:
        try:
            result = subprocess.run([str(a.binary), 'checks', 'public-program', '--input', str(source), '--out', str(out)],
                                    stdout=log, stderr=subprocess.STDOUT, timeout=260)
            status = 'exit ' + str(result.returncode)
        except subprocess.TimeoutExpired:
            status = 'generation deadline; unavailable usage remains unknown'
    return row['task'], status


def run(row):
    if not row.get('input'):
        return row['trial'], 'no retained final packet'
    source = Path(row['input'])
    dest = source.parent / 'public-program'
    output = dest / 'observation.json'
    if output.exists():
        return row['trial'], 'retained'
    dest.mkdir(exist_ok=True)
    task = a.programs / row['task'].split('/')[-1]
    program_path = task / 'generated/program.json'
    if not program_path.exists():
        return row['trial'], 'no completed generator'
    program = json.loads(program_path.read_text())
    if program['program'] is None:
        output.write_text(json.dumps({'error': 'No valid program arrived', 'score': None})+'\n')
        return row['trial'], 'generator unknown'
    candidate = json.loads(source.read_text())
    spec = json.loads((task / 'spec.json').read_text())
    data = {'candidate': candidate['files'], 'provided': spec['provided']}
    payload = json.dumps(data).encode()
    name = 'truth9584-' + uuid.uuid4().hex[:20]
    began = time.monotonic()
    with tempfile.TemporaryDirectory(prefix='truth-public-') as scratch:
        work = Path(scratch)
        work.chmod(0o755)
        (work / 'input.json').write_bytes(payload)
        (work / 'entry.py').write_text(ENTRY)
        (work / 'check.py').write_text(program['program']['python'])
        command = ['docker', 'run', '--rm', '--name', name, '--network', 'none', '--read-only',
                   '--cap-drop', 'ALL', '--security-opt', 'no-new-privileges', '--pids-limit', '64',
                   '--memory', '512m', '--memory-swap', '512m', '--cpus', '1', '--user', '1000:1000',
                   '--tmpfs', '/tmp:rw,nosuid,size=64m', '--mount', 'type=bind,source='+scratch+',target=/work,readonly',
                   '--workdir', '/tmp', '--env', 'PYTHONDONTWRITEBYTECODE=1', a.image,
                   'sh', '-c', 'timeout -s KILL 20s python3 -I /work/entry.py 2>&1 | head -c 65536']
        process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
        error = None
        try:
            stdout, stderr = process.communicate(timeout=30)
        except subprocess.TimeoutExpired:
            error = 'Host execution deadline'
            os.killpg(process.pid, signal.SIGKILL)
            stdout, stderr = process.communicate()
        finally:
            # Killing the client alone does not terminate Docker's container.
            subprocess.run(['docker', 'rm', '-f', name], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=10)
    parsed = None
    if error is None and process.returncode == 0:
        try:
            parsed = json.loads(stdout)
        except (ValueError, UnicodeError):
            error = 'Program did not return one JSON document'
    admissions = {r['id']:r for r in program['admissions']}
    observations = parsed.get('observations') if isinstance(parsed, dict) else None
    if not isinstance(observations, list) or len(observations) > 6:
        error = error or (parsed.get('error') if isinstance(parsed, dict) else None) or 'No valid observations'
        observations = []
    seen = set()
    for observation in observations:
        if (not isinstance(observation, dict) or observation.get('id') not in admissions
            or observation['id'] in seen or observation.get('verdict') not in ['passed', 'failed', 'unknown']
            or any(not isinstance(observation.get(k), str) for k in ['expected', 'actual', 'reason'])):
            error = 'Invalid or repeated check observation'
            break
        seen.add(observation['id'])
    scores = []
    if error is None:
        for observation in observations:
            admission = admissions[observation['id']]
            score = admission.get('score')
            if observation['verdict'] == 'failed' and admission['grounded'] and isinstance(score, (int,float)) and 0 <= score <= 1:
                scores.append(score)
    record = {'schema':'openagents.public-program-observation.v1', 'job':row['job'], 'trial':row['trial'],
              'input_sha256':hashlib.sha256(payload).hexdigest(), 'program_sha256':hashlib.sha256(program_path.read_bytes()).hexdigest(),
              'image':a.image, 'seconds':time.monotonic()-began, 'exit_code':process.returncode,
              'stdout':stdout.decode(errors='replace'), 'stderr':stderr[:65536].decode(errors='replace'),
              'error':error, 'observations':observations, 'score':max(scores) if scores else None}
    output.write_text(json.dumps(record, indent=2)+'\n')
    return row['trial'], error or 'recorded'


if a.mode == 'generate' and not a.binary:
    p.error('generate requires --binary')
if a.mode == 'run' and (not a.programs or not a.image or not a.image.startswith('sha256:')):
    p.error('run requires --programs and a pinned --image sha256:...')
with ThreadPoolExecutor(max_workers=4 if a.mode == 'generate' else 2) as pool:
    for identity, status in pool.map(generate if a.mode == 'generate' else run, rows):
        print(identity, status, flush=True)
