#!/usr/bin/env python3
"""Admit one verified Direct builder on the isolated environment host.

The Kubernetes controller is paused while its unused registration is replaced.
Existing build jobs and sandbox placements prevent that replacement.
"""

import os, json, subprocess, socket, uuid, urllib.request, urllib.error
from pathlib import Path
h = Path.home()
assert h == Path('/var/lib/space-spike/home')
assert socket.gethostname().startswith('oa-space-spike-11811')
assert subprocess.check_output(['git', '-C', str(h / 'SPACE'), 'rev-parse', 'HEAD'], text=True).strip() == 'de87b436c35dac056564bdd5b2a113c9046e189b'
k = ['kubectl', '--context', 'kind-space-local', '-n', 'space']

def kube(*args, data=None):
    return subprocess.check_output(k + list(args), input=data, text=True)
http = urllib.request.build_opener(urllib.request.ProxyHandler({}))

def call(path, method='GET', body=None):
    req = urllib.request.Request('http://127.0.0.1:8080' + path, method=method, data=None if body is None else json.dumps(body).encode(), headers={'Authorization': 'Bearer ' + os.environ['SPACE_API_TOKEN'], 'Content-Type': 'application/json'})
    try:
        r = http.open(req, timeout=60)
    except urllib.error.HTTPError as e:
        r = e
    with r:
        return (r.status, json.load(r))
report = h / 'evidence/direct-builder-preparation-11811.json'
assert not report.exists()
status, nodes = call('/admin/api/v1/nodes?include_occupancy=false')
assert status == 200
node = sorted([n for n in nodes if n['role'] == 'builder' and n['managed_by'] == 'k8s'], key=lambda n: n['name'])[0]
nodeid = str(uuid.UUID(node['id']))
name = node['name']
address = node['address']
assert name == 'space-local-worker4'
pg = json.loads(kube('get', 'pods', '-l', 'app.kubernetes.io/name=space-postgres', '-o', 'json'))['items']
assert len(pg) == 1
sql = f"SELECT COUNT(*) FROM sandboxes WHERE active_node_id='{nodeid}' OR target_node_id='{nodeid}'; SELECT COUNT(*) FROM template_build_jobs WHERE builder_node_id='{nodeid}';"
r = kube('exec', '-i', pg[0]['metadata']['name'], '--', 'sh', '-c', 'exec psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" -At -v ON_ERROR_STOP=1', data=sql)
assert r.strip().splitlines() == ['0', '0'], r
controller = json.loads(kube('get', 'deployment/space-cluster-controller', '-o', 'json'))['spec']['replicas']
kube('scale', 'deployment/space-cluster-controller', '--replicas=0')
for resource, container in [('daemonset/space-builder-node-api', 'node-api'), ('daemonset/space-builder-node-ctl', 'node-ctl')]:
    patch = {'spec': {'template': {'spec': {'nodeSelector': {'kubernetes.io/hostname': name}, 'containers': [{'name': container, 'env': [{'name': 'SPACE_NODE_ID', 'value': nodeid, 'valueFrom': None}]}]}}}}
    if container == 'node-ctl':
        patch['spec']['template']['spec']['containers'][0]['args'] = ['--poll-interval-secs=5']
    kube('patch', resource, '--type=strategic', '-p', json.dumps(patch))
    kube('rollout', 'status', resource, '--timeout=300s')
sql = f"DELETE FROM nodes n WHERE id='{nodeid}' AND managed_by='k8s' AND role='builder' AND NOT EXISTS (SELECT 1 FROM sandboxes s WHERE s.active_node_id=n.id OR s.target_node_id=n.id) AND NOT EXISTS (SELECT 1 FROM template_build_jobs b WHERE b.builder_node_id=n.id) RETURNING id;"
r = kube('exec', '-i', pg[0]['metadata']['name'], '--', 'sh', '-c', 'exec psql -U "$POSTGRES_USER" -d "$POSTGRES_DB" -At -v ON_ERROR_STOP=1', data=sql)
assert nodeid in r
status, admitted = call('/admin/api/v1/nodes', 'POST', {'id': nodeid, 'name': 'openagents-direct-builder-11811', 'address': address, 'node_api_endpoint': address + ':9800', 'role': 'builder', 'verify': True})
assert status in (200, 201) and admitted['id'] == nodeid, (status, admitted)
admission_status = status
kube('set', 'env', 'deployment/space-admin-api', 'SPACE_K8S_CONTROL_PLANE=false')
kube('rollout', 'status', 'deployment/space-admin-api', '--timeout=300s')
kube('scale', 'deployment/space-cluster-controller', '--replicas=' + str(controller))
kube('rollout', 'status', 'deployment/space-cluster-controller', '--timeout=300s')
status, _ = call('/admin/api/v1/nodes:refresh-status', 'POST', {'role': 'builder', 'force': True})
assert status in (200, 202)
report.write_text(json.dumps({'prepared': True, 'worker': name, 'worker_uuid': nodeid, 'endpoint': address + ':9800', 'admission_status': admission_status, 'original_active_placements': 0, 'original_build_jobs': 0, 'source_patched': False}, indent=2) + '\n')
print(report.read_text())
