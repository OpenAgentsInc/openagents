#!/usr/bin/env python3
"""Corrupt and restore only the clean base VM state on the disposable host."""

import hashlib
import json
from pathlib import Path
import socket
import subprocess
import sys

assert sys.argv[1:] in (['corrupt'], ['restore'])
home = Path.home()
assert socket.gethostname().split('.')[0] == 'oa-space-spike-11810'
assert home == Path('/var/lib/space-spike/home')
prepared = json.loads((home / 'evidence/direct-base-on-11810.json').read_text())
assert prepared['prepared'] and prepared['policy'] == 'on'
assert prepared['template'] == 'seed@sha256:2728d439a511dc9a8755ef229f718a20d12a8cad83fe0d198e3e541f0afcc0a5'
node = prepared['worker']
directory = prepared['after']['snapshotDir']
assert directory == '/space/cache/firecracker/templates/seed/snapshots/sha256:2728d439a511dc9a8755ef229f718a20d12a8cad83fe0d198e3e541f0afcc0a5/2vcpu-1024mib'
assert '..' not in directory.split('/')
source = directory + '/vmstate.base'
backup = source + '.11810-backup'
path = home / 'evidence/direct-corruption-11810.json'
marker = 'CORRUPT-BASE-SNAPSHOT-11810'
corrupt_digest = hashlib.sha256(marker.encode()).hexdigest()


def run(*args):
    return subprocess.check_output(['docker', 'exec', node, *args], text=True, timeout=30)


def digest(filename):
    return run('sha256sum', filename).split()[0]


def save(report):
    path.write_text(json.dumps(report, indent=2) + '\n')


if sys.argv[1] == 'corrupt':
    assert not path.exists(), 'Inspect the previous fault injection before repeating it'
    measured = json.loads((home / 'evidence/direct-snapshot20-11810.json').read_text())
    assert measured['passed'] and measured['count'] == 20
    assert subprocess.run(['docker', 'exec', node, 'test', '-e', backup]).returncode != 0
    report = {'schema': 'openagents.space.corrupt_base_test.v1', 'worker': node,
              'source': source, 'backup': backup, 'original_sha256': digest(source),
              'method': 'Truncate only the clean base VM-state file to a fixed test marker',
              'expected_corrupt_sha256': corrupt_digest}
    run('cp', '--reflink=always', '--preserve=all', source, backup)
    assert digest(backup) == report['original_sha256']
    report['backup_verified'] = True
    save(report)
    run('sh', '-c', 'printf "%s" "$2" > "$1"', 'sh', source, marker)
    report['corrupt_sha256'] = digest(source)
    assert report['corrupt_sha256'] == corrupt_digest != report['original_sha256']
    save(report)
else:
    report = json.loads(path.read_text())
    assert report['source'] == source and report['backup_verified']
    if report.get('restored'):
        assert digest(source) == report['original_sha256']
    else:
        assert digest(backup) == report['original_sha256']
        assert digest(source) in (report['original_sha256'], corrupt_digest)
        run('mv', backup, source)
        assert digest(source) == report['original_sha256']
        report['restored'] = True
        save(report)
print(json.dumps({'action': sys.argv[1], 'record': str(path), 'restored': report.get('restored', False)}))
