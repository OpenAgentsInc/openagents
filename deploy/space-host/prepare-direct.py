#!/usr/bin/env python3
"""Prepare one verified Direct worker on a disposable SPACE host.

Kind reports worker names; Direct requires UUIDs. Keep the worker's existing
UUID, limit its API/controller DaemonSets to that node, and register it through
the authenticated Direct API. This changes no provider source or credentials.
"""

import json
import os
from pathlib import Path
import socket
import subprocess
import urllib.error
import urllib.request
import uuid

HOME = Path.home()
ISSUE = os.environ.get('SPACE_SPIKE_ISSUE', '11810')
assert ISSUE in ('11810', '11811', '11812')
assert socket.gethostname().split('.')[0] == 'oa-space-spike-' + ISSUE
assert HOME == Path('/var/lib/space-spike/home')
assert (HOME / 'install.log').read_text().rstrip().endswith('SPACE installation and first sandbox passed.')
PIN = 'de87b436c35dac056564bdd5b2a113c9046e189b'
assert subprocess.check_output(['git', '-C', str(HOME / 'SPACE'), 'rev-parse', 'HEAD'], text=True).strip() == PIN
BASE = os.environ['SPACE_API_URL']
assert BASE == 'http://127.0.0.1:8080'
TOKEN = os.environ['SPACE_API_TOKEN']
KUBE = ['kubectl', '--context', 'kind-space-local', '-n', 'space']


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        return None


HTTP = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())


def call(path, method='GET', body=None):
    raw = None if body is None else json.dumps(body).encode()
    req = urllib.request.Request(BASE + path, method=method, data=raw,
        headers={'Authorization': 'Bearer ' + TOKEN, 'Content-Type': 'application/json'})
    try:
        response = HTTP.open(req, timeout=60)
    except urllib.error.HTTPError as error:
        response = error
    with response:
        raw = response.read(4 * 1024 * 1024 + 1)
        assert len(raw) <= 4 * 1024 * 1024
        return response.status, json.loads(raw) if raw else {}


def kube(*args, data=None):
    return subprocess.check_output([*KUBE, *args], input=data, text=True)


def save(path, value):
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, 'w') as out:
        json.dump(value, out, indent=2)
        out.write('\n')


report_path = HOME / f'evidence/direct-profile-{ISSUE}.json'
assert not report_path.exists(), 'Inspect the existing Direct preparation before repeating it'
status, listing = call('/v1/sandboxes')
assert status == 200
items = listing if isinstance(listing, list) else next(
    (listing[key] for key in ('items', 'sandboxes', 'data') if isinstance(listing.get(key), list)), None)
assert items == [], 'Delete the isolated installer sandbox before changing worker configuration'
status, nodes = call('/admin/api/v1/nodes?include_occupancy=false')
assert status == 200 and isinstance(nodes, list)
pods = json.loads(kube('get', 'pods', '-l',
    'app.kubernetes.io/component=node-api,app.kubernetes.io/name=space-worker', '-o', 'json'))['items']
pods = sorted((pod for pod in pods if pod['status'].get('phase') == 'Running'), key=lambda p: p['spec']['nodeName'])
assert pods
pod = pods[0]
name, address = pod['spec']['nodeName'], pod['status']['hostIP']
node = next(row for row in nodes if row.get('k8s_node_name') == name or row.get('name') == name)
node_id = str(uuid.UUID(node['id']))
assert node['managed_by'] == 'k8s' and node['role'] == 'worker'
objects = {}
for resource in ('deployment/space-api', 'deployment/space-cluster-controller',
                 'daemonset/space-node-api', 'daemonset/space-node-ctl'):
    objects[resource] = json.loads(kube('get', resource, '-o', 'json'))
save(HOME / 'evidence/direct-original-kubernetes-private.json', objects)
controller_replicas = objects['deployment/space-cluster-controller']['spec']['replicas']
report = {'schema': 'openagents.space.direct_profile.v1', 'upstream_commit': PIN,
          'worker': name, 'worker_uuid': node_id, 'endpoint': address + ':9800',
          'original_sandbox_count': 0, 'controller_replicas': controller_replicas,
          'credentials_changed': False, 'source_patched': False, 'steps': []}
save(report_path, report)


def step(name):
    report['steps'].append(name)
    save(report_path, report)


kube('scale', 'deployment/space-cluster-controller', '--replicas=0')
kube('set', 'env', 'deployment/space-cluster-controller', 'SPACE_K8S_CONTROL_PLANE=false')
step('paused Kubernetes controller')
for resource, container in [('daemonset/space-node-api', 'node-api'), ('daemonset/space-node-ctl', 'node-ctl')]:
    patch = {'spec': {'template': {'spec': {
        'nodeSelector': {'kubernetes.io/hostname': name},
        'containers': [{'name': container, 'env': [
            {'name': 'SPACE_NODE_ID', 'value': node_id, 'valueFrom': None}]}]}}}}
    if container == 'node-ctl':
        patch['spec']['template']['spec']['containers'][0]['args'] = ['--poll-interval-secs=5']
    kube('patch', resource, '--type=strategic', '-p', json.dumps(patch))
    kube('rollout', 'status', resource, '--timeout=300s')
    step(resource + ' reports existing UUID on one worker')
# The stopped Kubernetes controller cannot recreate this unused registration.
# Only the worker selected above can be removed; active or target placements
# prevent the deletion. The Direct API then verifies the same UUID and role.
pg = json.loads(kube('get', 'pods', '-l', 'app.kubernetes.io/name=space-postgres', '-o', 'json'))['items']
assert len(pg) == 1
sql = f'''DELETE FROM nodes n WHERE n.id='{node_id}' AND n.managed_by='k8s' AND n.role='worker'
AND NOT EXISTS (SELECT 1 FROM sandboxes s WHERE s.active_node_id=n.id OR s.target_node_id=n.id)
RETURNING n.id;'''
removed = kube('exec', '-i', pg[0]['metadata']['name'], '--', 'sh', '-c',
    'exec psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" -v ON_ERROR_STOP=1 -At', data=sql)
assert node_id in removed, 'An active placement prevented replacement of the test registration'
step('removed only the unused Kubernetes registration')
kube('set', 'env', 'deployment/space-api', 'SPACE_K8S_CONTROL_PLANE=false')
kube('rollout', 'status', 'deployment/space-api', '--timeout=300s')
status, admitted = call('/admin/api/v1/nodes', 'POST', {
    'id': node_id, 'name': 'openagents-direct-worker-' + ISSUE, 'address': address,
    'node_api_endpoint': address + ':9800', 'role': 'worker', 'verify': True})
assert status in (200, 201) and admitted['id'] == node_id, 'Direct worker verification failed'
report['admission_status'] = status
step('Direct API verified the original worker UUID and role')
kube('scale', 'deployment/space-cluster-controller', '--replicas=' + str(controller_replicas))
kube('rollout', 'status', 'deployment/space-cluster-controller', '--timeout=300s')
step('Direct operation dispatcher started')
status, _ = call('/admin/api/v1/nodes:refresh-status', 'POST', {'role': 'worker', 'force': True})
assert status in (200, 202)
status, nodes = call('/admin/api/v1/nodes?include_occupancy=false')
assert status == 200
selected = next(row for row in nodes if row['id'] == node_id)
report['node'] = {key: selected.get(key) for key in ('id', 'role', 'managed_by', 'state', 'scheduling_status', 'address', 'node_api_endpoint')}
report['prepared'] = True
save(report_path, report)
print(json.dumps({'prepared': True, 'worker': name, 'worker_uuid': node_id, 'admission_status': report['admission_status']}))
