#!/usr/bin/env python3
"""Prepare the clean seed cache and native network slots for the Direct test.

These are operator preparation calls. Customer creates still go through the
Computers service and Rust HTTP driver.
"""

import hashlib
import json
from pathlib import Path
import socket
import subprocess
import time

HOME = Path.home()
assert socket.gethostname().split('.')[0] == 'oa-space-spike-11810'
assert HOME == Path('/var/lib/space-spike/home')
profile = json.loads((HOME / 'evidence/direct-profile-11810.json').read_text())
assert profile['prepared']
node = profile['worker']
assert node.startswith('space-local-worker')
seed = HOME / '.space-devbox/kind/seed/build'
manifest = json.loads((seed / 'manifest.json').read_text())
assert manifest['name'] == 'seed'
assert manifest['content_hash'] == 'sha256:2728d439a511dc9a8755ef229f718a20d12a8cad83fe0d198e3e541f0afcc0a5'
report = {'schema': 'openagents.space.direct_worker_preparation.v1', 'worker': node,
          'template': 'seed@' + manifest['content_hash'], 'execution': manifest['execution']}
path = HOME / 'evidence/direct-worker-preparation-11810.json'
assert not path.exists(), 'Inspect the existing preparation before repeating it'


def save():
    path.write_text(json.dumps(report, indent=2) + '\n')


def run(*args, timeout=120):
    return subprocess.check_output(list(args), text=True, timeout=timeout)


binary = HOME / '.space-devbox/kind/rust-artifacts/artifacts/bin/space-tmplmgr'
assert binary.is_file()
report['template_cli_sha256'] = hashlib.sha256(binary.read_bytes()).hexdigest()
run('docker', 'cp', str(binary), node + ':/space/bin/space-tmplmgr')
# Keep template settings local while the test uses Direct on the Kind worker.
# Inventory publication remains active. The pauses expire with the test host.
control = binary.with_name('space-nodectl')
assert control.is_file()
run('docker', 'cp', str(control), node + ':/space/bin/space-nodectl')
report['paused_inbound_facets'] = ['template-base-snapshot-policy', 'template-desired-state']
for facet in report['paused_inbound_facets']:
    run('docker', 'exec', node, '/space/bin/space-nodectl', 'disable', facet,
        'inbound', '--ttl', '6h', '--reason', 'isolated Direct API qualification 11810')
save()
probe = subprocess.run(['docker', 'exec', node, '/space/bin/space-tmplmgr', 'get',
    '--template', 'seed', '--content-hash', manifest['content_hash']], capture_output=True, text=True, timeout=30)
report['already_cached'] = probe.returncode == 0
save()
if not report['already_cached']:
    digest = hashlib.sha256()
    with (seed / 'rootfs.ext4').open('rb') as source:
        for block in iter(lambda: source.read(1024 * 1024), b''):
            digest.update(block)
    assert 'sha256:' + digest.hexdigest() == manifest['content_hash']
    target = '/space/cache/firecracker/templates/seed'
    for filename in ['rootfs.ext4', manifest['content_hash'] + '/rootfs.ext4']:
        assert subprocess.run(['docker', 'exec', node, 'test', '-e', target + '/' + filename]).returncode != 0
    run('docker', 'exec', node, 'mkdir', '-p', target)
    for filename in ['rootfs.ext4', 'metadata.json']:
        run('docker', 'cp', str(seed / filename), node + ':' + target + '/' + filename, timeout=180)
    registered = run('docker', 'exec', node, '/space/bin/space-tmplmgr', 'register', '--template', 'seed')
    assert manifest['content_hash'] in registered, 'Registered seed identity differs'
    report['registered_source_sha256'] = digest.hexdigest()
    save()
run('docker', 'exec', node, '/space/bin/space-tmplmgr', 'get', '--template', 'seed',
    '--content-hash', manifest['content_hash'])
helper = None
try:
    helper = run('docker', 'create', '--label', 'openagents-issue=11810', 'space-seed-publish:local').strip()
    grpcurl = HOME / 'grpcurl-11810'
    run('docker', 'cp', helper + ':/usr/local/bin/grpcurl', str(grpcurl))
    report['grpcurl_sha256'] = hashlib.sha256(grpcurl.read_bytes()).hexdigest()
    run('docker', 'cp', str(grpcurl), node + ':/space/bin/grpcurl')
finally:
    if helper:
        run('docker', 'rm', helper)
run('docker', 'cp', str(HOME / 'SPACE/proto'), node + ':/space/spike-proto')


def rpc(method, value):
    return json.loads(run('docker', 'exec', node, '/space/bin/grpcurl', '-plaintext',
        '-import-path', '/space/spike-proto', '-proto', 'node/v1/node.proto',
        '-d', json.dumps(value), 'unix:///space/run/node-manager/node-manager.sock',
        'space.node.v1.NodeManager/' + method, timeout=30))


report['network_before'] = rpc('GetNetworkPoolStatus', {})
report['network_configured'] = rpc('SetNetworkPoolConfig', {
    'target_ready_slots': 2, 'retained_ready_slots': 2, 'min_ready_slots': 1})
save()
deadline = time.monotonic() + 120
while time.monotonic() < deadline:
    status = rpc('GetNetworkPoolStatus', {})
    if status.get('networkSlotsReady', 0) >= 2:
        report['network_ready'] = status
        report['prepared'] = True
        save()
        print(json.dumps({'prepared': True, 'template': report['template'], 'worker': node}))
        break
    time.sleep(1)
else:
    raise RuntimeError('Native network slots did not become ready')
