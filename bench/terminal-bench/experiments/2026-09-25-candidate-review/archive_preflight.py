#!/usr/bin/env python3
"""Pin public archive-task environments without running agents or graders."""
import argparse
import concurrent.futures
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import time
import tomllib

from reproduce import tree, write

TASKS = '''circuit-fibsqrt constraints-scheduling distribution-search
financial-document-processor multi-source-data-merger openssl-selfsigned-cert
polyglot-c-py regex-log sparql-university write-compressor bn-fit-modify
model-extraction-relu-logits'''.split()
PIN = '3b5caaa4863d64dda7f0957bf4fc2d4f019202d4'


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--upstream', type=Path, required=True)
    p.add_argument('--out', type=Path, required=True)
    a = p.parse_args()
    head = subprocess.check_output(['git', '-C', str(a.upstream), 'rev-parse', 'HEAD'], text=True).strip()
    if head != PIN:
        raise ValueError('Upstream checkout does not match the declared pin')
    if subprocess.check_output(['git', '-C', str(a.upstream), 'status', '--porcelain'], text=True).strip():
        raise ValueError('Upstream checkout is not clean')
    a.out.mkdir(parents=True, exist_ok=True)

    def run(task):
        out = a.out / task
        if (out / 'result.json').exists():
            return json.loads((out / 'result.json').read_text())
        out.mkdir(exist_ok=True)
        public = a.upstream / 'archive' / task
        config = tomllib.loads((public / 'task.toml').read_text())
        record = {'task': task, 'upstream': PIN, 'environment': config['environment'],
                  'public_files': tree(public / 'environment'),
                  'instruction_sha256': hashlib.sha256((public / 'instruction.md').read_bytes()).hexdigest(),
                  'agent_ran': False, 'grader_ran': False}
        started = time.monotonic()
        try:
            if shutil.disk_usage(a.out).free < 15 * 1024**3:
                raise ValueError('Preflight requires 15 GiB free')
            tag = 'truth9584-archive/' + task + ':public'
            with (out / 'build.log').open('w') as log:
                subprocess.run(['docker', 'build', '-t', tag, str(public / 'environment')],
                               stdout=log, stderr=subprocess.STDOUT, timeout=900, check=True)
            image = subprocess.check_output(['docker', 'image', 'inspect', '--format', '{{.Id}}', tag], text=True).strip()
            # Inventory declared public inputs and available tools only. No tests,
            # candidate code, model calls, or package installation runs here.
            probe = subprocess.run([
                'docker', 'run', '--rm', '--network', 'none', '--read-only', '--cap-drop', 'ALL',
                '--security-opt', 'no-new-privileges', '--pids-limit', '64', '--memory', '512m',
                '--tmpfs', '/tmp:rw,size=32m', '--entrypoint', 'sh', image, '-c',
                'for tool in bash python3 gcc openssl Rscript uv; do command -v "$tool" || true; done; '
                'du -sk /app; find /app -type f -exec sha256sum {} +'],
                capture_output=True, text=True, timeout=60)
            write(out / 'probe.json', {'exit': probe.returncode, 'stdout': probe.stdout, 'stderr': probe.stderr})
            if probe.returncode:
                raise ValueError('Public environment inventory failed')
            record.update(image=image, tag=tag, status='available')
        except (ValueError, OSError, subprocess.SubprocessError) as error:
            record.update(status='unavailable', error=str(error))
        record['seconds'] = time.monotonic() - started
        write(out / 'result.json', record)
        return record

    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
        records = []
        for record in pool.map(run, TASKS):
            records.append(record)
            print(json.dumps({k: record.get(k) for k in ['task', 'status', 'image', 'error', 'seconds']}), flush=True)
    write(a.out / 'preflight.json', records)


if __name__ == '__main__':
    main()
