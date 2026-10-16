#!/usr/bin/env python3
"""Collect only the public workload response from a research VM's logs."""
import json
import os
from pathlib import Path
import subprocess
import sys

vm, destination = sys.argv[1:]
project = os.environ.get('LAYERTREE_PROJECT', 'openagentsgemini')
zone = os.environ.get('LAYERTREE_ZONE', 'us-central1-a')
instance = json.loads(subprocess.check_output([
    'gcloud', 'compute', 'instances', 'describe', vm,
    '--project', project, '--zone', zone, '--format=json',
]))
if instance.get('labels', {}).get('purpose') != 'attestation-spike':
    raise RuntimeError('Use a research attestation probe.')
logs = json.loads(subprocess.check_output([
    'gcloud', 'logging', 'read',
    f'resource.type="gce_instance" AND resource.labels.instance_id="{instance["id"]}"',
    '--project', project, '--freshness=4h', '--limit=1000', '--format=json',
]))
found = {}
for entry in logs:
    payload = entry.get('jsonPayload', {})
    for message in (payload.get('MESSAGE'), payload.get('message'), entry.get('textPayload')):
        if not isinstance(message, str):
            continue
        try:
            value = json.loads(message)
        except ValueError:
            continue
        if isinstance(value, dict) and value.get('schema') in ('openagents.layertree.attestation-probe.v1', 'openagents.layertree.key-lifecycle.v1'):
            if not isinstance(value.get('token'), str):
                raise RuntimeError('The workload response has no token.')
            found[value['token']] = value
if len(found) != 1:
    raise RuntimeError(f'Expected one workload response; found {len(found)}. Check the launcher logs.')
out = Path(destination)
out.mkdir(parents=True, exist_ok=True)
(out / 'probe.json').write_text(json.dumps(next(iter(found.values())), indent=2) + '\n')
public = {k: instance[k] for k in (
    'name', 'id', 'creationTimestamp', 'status', 'machineType', 'zone',
    'confidentialInstanceConfig', 'labels',
) if k in instance}
(out / 'instance.json').write_text(json.dumps(public, indent=2) + '\n')
print('Public probe response saved; verify its token before accepting its claims.')
