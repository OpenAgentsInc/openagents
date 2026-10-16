#!/usr/bin/env python3
"""Read the exact seed and host compatibility of the isolated environment test."""

import hashlib
import json
from pathlib import Path
import socket
import subprocess

HOME = Path.home()
assert socket.gethostname().split('.')[0] == 'oa-space-spike-11811'
assert HOME == Path('/var/lib/space-spike/home')
assert subprocess.check_output(['git', '-C', str(HOME / 'SPACE'), 'rev-parse', 'HEAD'], text=True).strip() == 'de87b436c35dac056564bdd5b2a113c9046e189b'
seed = HOME / '.space-devbox/kind/seed/build'
manifest = json.loads((seed / 'manifest.json').read_text())
prepared = json.loads((HOME / 'evidence/direct-profile-11811.json').read_text())
assert prepared['prepared']
assert manifest['name'] == 'seed'


def sha(path):
    digest = hashlib.sha256()
    with path.open('rb') as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b''):
            digest.update(chunk)
    return digest.hexdigest()


assert 'sha256:' + sha(seed / 'rootfs.ext4') == manifest['content_hash']
mount = HOME / 'evidence/seed-profile-mount-11811'
mount.mkdir(mode=0o700)
assert subprocess.run(['mountpoint', '-q', str(mount)]).returncode != 0
subprocess.run(['sudo', 'mount', '-o', 'loop,ro,noload', str(seed / 'rootfs.ext4'), str(mount)], check=True)
try:
    daemon = sha(mount / 'usr/local/bin/spaced-svc')
    version = (mount / 'etc/spaced/version').read_text().strip()
    family = (mount / 'etc/spaced/compatibility-family').read_text().strip()
finally:
    subprocess.run(['sudo', 'umount', str(mount)], check=True)
    mount.rmdir()
assert 'sha256:' + sha(seed / 'rootfs.ext4') == manifest['content_hash']
execution = manifest['execution']
assert execution['runtime'] == {'kind': 'firecracker', 'version': '1.15.1'}
assert execution['guest'] == {'os': 'linux', 'architecture': 'x86_64'}
kernel = next(item for item in execution['components'] if item['kind'] == 'kernel')
runtime = HOME / 'vendor/firecracker/v1.15.1/bin/firecracker'
fields = {}
for line in Path('/proc/cpuinfo').read_text().split('\n\n')[0].splitlines():
    if ':' in line:
        key, value = line.split(':', 1)
        if key.strip() in ('model name', 'flags'):
            fields[key.strip()] = value.strip()
inputs = {'execution': execution, 'spaced_compatibility_family': family,
          'host_cpu': fields, 'shape': {'vcpus': 2, 'memory_mib': 1024}}
encoded = json.dumps(inputs, sort_keys=True, separators=(',', ':')).encode()
profile = {'platform': {'os': 'linux', 'architecture': 'x86_64'},
           'guest_daemon': {'revision': version, 'digest': daemon},
           'kernel': {'revision': kernel['version'], 'digest': kernel['digest'].removeprefix('sha256:')},
           'host_runtime': {'revision': '1.15.1', 'digest': sha(runtime)},
           'snapshot_format': execution['state_format']['format'] + '-' + execution['state_format']['version'],
           'boot_profile_digest': hashlib.sha256(encoded).hexdigest()}
base = {'kind': 'space-template', 'reference': 'seed@' + manifest['content_hash'],
        'provider_identity': manifest['content_hash']}
for name, value in [('compatibility-11811.json', profile), ('compatibility-inputs-11811.json', inputs),
                    ('base-artifact-11811.json', base)]:
    path = HOME / 'evidence' / name
    assert not path.exists(), 'Keep the previous profile evidence'
    path.write_text(json.dumps(value, indent=2) + '\n')
    path.chmod(0o600)
print(json.dumps({'prepared': True, 'template': base['reference'], 'daemon_sha256': daemon,
                  'runtime_sha256': profile['host_runtime']['digest'],
                  'boot_profile_sha256': profile['boot_profile_digest']}))
