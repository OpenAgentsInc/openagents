#!/usr/bin/env python3
"""Inspect the published disk and clean boot files on the isolated host."""

import hashlib
import json
from pathlib import Path
import re
import socket
import subprocess
import sys

HOME = Path.home()
assert HOME == Path('/var/lib/space-spike/home')
assert socket.gethostname().split('.')[0] == 'oa-space-spike-11811'
state = Path(sys.argv[1]).resolve()
assert state.parent == HOME / 'evidence' and '11811' in state.name
work = json.loads((state / 'publication-work.json').read_text())
template = work['accepted']['template_name']
rootfs = work['rootfs']
assert re.fullmatch(r'oaenv-build-\d+-[a-f0-9]{24}-space', template)
assert re.fullmatch(r'sha256:[a-f0-9]{64}', rootfs)
published = json.loads((state / 'published-template.json').read_text())
assert published['name'] == template and published['content_hash'] == rootfs
base = next(row for row in published['snapshots']
            if row['vcpu'] == 2 and row['memory_mib'] == 4096)
assert re.fullmatch(r'(sha256:)?[a-f0-9]{64}', base['snapshot_hash'])
report_name = sys.argv[2] if len(sys.argv) > 2 else 'raw-privacy.json'
assert re.fullmatch(r'raw-privacy(?:-[a-z0-9-]+)?\.json', report_name)
report_path = state / report_name
assert not report_path.exists(), 'Keep the previous privacy evidence'
nodes = ['space-local-worker', 'space-local-worker4']
canary = 'oa-qualification-private-canary-11811'
report = {'schema': 'openagents.space.environment_privacy_check.v1',
          'job': work['job'], 'inputs_digest': work['inputs_digest'],
          'rootfs_digest': rootfs.removeprefix('sha256:'),
          'snapshot_hash': base['snapshot_hash'], 'files': [],
          'canary_matches': 0, 'base_snapshots': [], 'disk_only_publication': True,
          'base_created_from_published_disk': True}

# Admission public keys belong to the pinned provider seed. Customer SSH
# directories are removed; only these exact public keys may be restored.
seed = HOME / '.space-devbox/kind/seed/build'
seed_manifest = json.loads((seed / 'manifest.json').read_text())
assert seed_manifest['content_hash'] == work['base']['provider_identity']
before = subprocess.check_output(['sha256sum', str(seed / 'rootfs.ext4')], text=True).split()[0]
assert 'sha256:' + before == seed_manifest['content_hash']
mount = state / 'seed-admission-mount'
mount.mkdir(mode=0o700)
subprocess.run(['sudo', 'mount', '-o', 'loop,ro,noload',
                str(seed / 'rootfs.ext4'), str(mount)], check=True)
try:
    admission = {'base_rootfs_digest': seed_manifest['content_hash'].removeprefix('sha256:')}
    for name, path in [('root', 'root/.ssh/authorized_keys'),
                       ('user', 'home/user/.ssh/authorized_keys')]:
        output = subprocess.check_output(['sudo', 'sha256sum', str(mount / path)], text=True)
        admission[name + '_authorized_keys_sha256'] = output.split()[0]
    report['admission'] = admission
finally:
    subprocess.run(['sudo', 'umount', str(mount)], check=True)
    mount.rmdir()
after = subprocess.check_output(['sha256sum', str(seed / 'rootfs.ext4')], text=True).split()[0]
assert before == after


def run(*args):
    return subprocess.check_output(list(args), text=True, timeout=180)


disk_seen = False
memory_seen = False
for node in nodes:
    directory = '/space/cache/firecracker/templates/' + template
    exists = subprocess.run(['docker', 'exec', node, 'test', '-d', directory],
                            capture_output=True).returncode
    if exists:
        continue
    files = run('docker', 'exec', node, 'find', directory, '-maxdepth', '6',
                '-type', 'f').splitlines()
    for path in sorted(files):
        assert path.startswith(directory + '/') and '\n' not in path
        relative = path[len(directory) + 1:]
        # These are publication-owned files. Sandbox directories are excluded.
        assert '..' not in relative.split('/')
        scanned = subprocess.run(['docker', 'exec', node, 'grep', '-a', '-F',
                                  '-q', '--', canary, path], capture_output=True,
                                 timeout=180)
        assert scanned.returncode in (0, 1), 'The artifact could not be read'
        assert scanned.returncode == 1, 'The synthetic credential reached an artifact'
        digest = run('docker', 'exec', node, 'sha256sum', path).split()[0]
        assert re.fullmatch(r'[a-f0-9]{64}', digest)
        if relative == rootfs + '/rootfs.ext4':
            assert 'sha256:' + digest == rootfs
            disk_seen = True
        if relative.startswith('snapshots/' + rootfs + '/2vcpu-4096mib/'):
            memory_seen |= Path(path).name == 'snapshot.base'
        report['files'].append({'worker': node, 'path': relative,
                                'sha256': digest, 'canary_matches': 0})
    snapshot_dir = directory + '/snapshots/' + rootfs + '/2vcpu-4096mib'
    if subprocess.run(['docker', 'exec', node, 'test', '-f',
                       snapshot_dir + '/snapshot.base'], capture_output=True).returncode == 0:
        observed = run('docker', 'exec', node, '/space/bin/space-tmplmgr', 'get',
                       '--template', template, '--content-hash', rootfs)
        local_base = re.search(r'^snapshot:\s+([a-f0-9]{64}) \(vcpu=2, mem=4096MiB\)',
                               observed, re.MULTILINE)
        assert local_base, 'The local clean base identity is missing'
        combined = hashlib.sha256()
        for name in ('snapshot.base', 'vmstate.base'):
            path = snapshot_dir + '/' + name
            size = int(run('docker', 'exec', node, 'stat', '-c', '%s', path).strip())
            combined.update(name.encode())
            combined.update(size.to_bytes(8, 'little'))
            process = subprocess.Popen(['docker', 'exec', node, 'cat', path], stdout=subprocess.PIPE)
            try:
                for chunk in iter(lambda: process.stdout.read(1024 * 1024), b''):
                    combined.update(chunk)
                assert process.wait(timeout=180) == 0
            finally:
                process.stdout.close()
                if process.poll() is None:
                    process.kill()
                    process.wait()
        assert combined.hexdigest() == local_base.group(1)
        report['base_snapshots'].append({'worker': node, 'snapshot_hash': combined.hexdigest(),
                                         'vcpu': 2, 'memory_mib': 4096})
assert any(row['snapshot_hash'] == base['snapshot_hash'].removeprefix('sha256:')
           for row in report['base_snapshots']), 'The published base was not inspected'
assert disk_seen, 'The published disk was not inspected'
assert memory_seen, 'The clean base memory was not inspected'
report['passed'] = True
report_path.write_text(json.dumps(report, indent=2) + '\n')
report_path.chmod(0o600)
print(json.dumps({'passed': True, 'files_checked': len(report['files']),
                  'rootfs_digest': report['rootfs_digest'],
                  'snapshot_hash': report['snapshot_hash'], 'canary_matches': 0}))
