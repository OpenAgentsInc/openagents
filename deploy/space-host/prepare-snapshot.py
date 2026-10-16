#!/usr/bin/env python3
"""Set a clean seed base-snapshot policy on the disposable Direct worker."""

import argparse
import json
from pathlib import Path
import socket
import subprocess
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('policy', choices=['on', 'off'])
parser.add_argument('evidence', type=Path)
args = parser.parse_args()
home = Path.home()
assert socket.gethostname().split('.')[0] == 'oa-space-spike-11810'
assert home == Path('/var/lib/space-spike/home')
assert args.evidence.parent.resolve() == home / 'evidence'
assert not args.evidence.exists(), 'Keep previous evidence; choose a new path'
profile = json.loads((home / 'evidence/direct-profile-11810.json').read_text())
assert profile['prepared']
node = profile['worker']
manifest = json.loads((home / '.space-devbox/kind/seed/build/manifest.json').read_text())
assert manifest['name'] == 'seed'
assert manifest['content_hash'] == 'sha256:2728d439a511dc9a8755ef229f718a20d12a8cad83fe0d198e3e541f0afcc0a5'
report = {'schema': 'openagents.space.clean_base_preparation.v1', 'policy': args.policy,
          'worker': node, 'template': 'seed@' + manifest['content_hash'],
          'execution': manifest['execution'], 'vcpu': 2, 'memory_mib': 1024,
          'source': 'Clean seed built from pinned public upstream; no customer checkpoint'}


def save():
    args.evidence.write_text(json.dumps(report, indent=2) + '\n')


def rpc(method, body, timeout=30):
    return json.loads(subprocess.check_output([
        'docker', 'exec', node, '/space/bin/grpcurl', '-plaintext',
        '-import-path', '/space/spike-proto', '-proto', 'template/manager/v1/manager.proto',
        '-d', json.dumps(body), 'unix:///space/run/template-manager/template-manager.sock',
        'space.template.manager.v1.TemplateManager/' + method], text=True, timeout=timeout))


query = {'template': {'name': 'seed', 'content_hash': manifest['content_hash']},
         'vcpu': 2, 'memory_mib': 1024}
report['before'] = rpc('GetTemplate', query)
report['pools'] = rpc('PoolStatus', {'template': 'seed'})
assert not report['pools'].get('pools'), 'This test must not use a warm pool'
report['node_execution_inventory'] = json.loads(subprocess.check_output([
    'docker', 'exec', node, '/space/bin/grpcurl', '-plaintext',
    '-import-path', '/space/spike-proto', '-proto', 'node/v1/node.proto', '-d', '{}',
    'unix:///space/run/node-manager/node-manager.sock',
    'space.node.v1.NodeManager/GetExecutionInventory'], text=True, timeout=30))
execution = report['execution']
inventory = report['node_execution_inventory']
assert execution['runtime']['kind'] == 'firecracker'
assert execution['guest'] == {'os': 'linux', 'architecture': 'x86_64'}
runtime = next(item for item in inventory['runtimes']
               if item['runtime'] == {'kind': 'RUNTIME_KIND_FIRECRACKER',
                                      'version': execution['runtime']['version']})
assert {'os': 'GUEST_OPERATING_SYSTEM_LINUX', 'architecture': 'CPU_ARCHITECTURE_X86_64'} in runtime['supportedGuests']
assert any(item['identity'] == execution['state_format'] and item.get('canCreate')
           and item.get('canLoad') for item in runtime['stateFormats'])
kernels = {item['component']['digest'] for item in inventory['components']
           if item['component']['kind'] == 'ARTIFACT_COMPONENT_KIND_KERNEL'}
requested_kernels = {item['digest'] for item in execution['components'] if item['kind'] == 'kernel'}
assert requested_kernels and requested_kernels <= kernels
save()
started = time.monotonic()
report['update'] = rpc('UpdateTemplateSnapshotPolicy', {
    'template': 'seed', 'ensure_base_snapshot': args.policy == 'on',
    'snapshot_vcpu': 2, 'snapshot_memory_mib': 1024}, timeout=300)
report['preparation_ms'] = (time.monotonic() - started) * 1000
save()
assert not report['update'].get('error'), 'Snapshot policy preparation failed'
report['after'] = rpc('GetTemplate', query)
if args.policy == 'on':
    assert report['after'].get('snapshotHash'), 'The clean base snapshot is missing'
    assert report['after']['snapshotVcpu'] == 2 and report['after']['snapshotMemoryMib'] == 1024
else:
    assert not report['after'].get('snapshotHash'), 'The base snapshot still exists'
report['prepared'] = True
save()
print(json.dumps({'prepared': True, 'policy': args.policy, 'preparation_ms': report['preparation_ms']}))
