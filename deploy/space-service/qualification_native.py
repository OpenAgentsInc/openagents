#!/usr/bin/env python3
"""Run bounded native qualification requests without exposing credentials."""
import argparse
import http.client
import json
import os
from pathlib import Path
import subprocess
from cluster_secrets import identity, secret

ROOT = Path('/space/qualification-11814')


def request(admin, method, path, data=None):
    fixture = secret('qualification', identity())
    if admin:
        result = subprocess.run(['/usr/local/bin/k3s','kubectl','-n','space','get','service','space-admin-api','-o','jsonpath={.spec.clusterIP}'], capture_output=True, check=True, timeout=10)
        connection = http.client.HTTPConnection(result.stdout.decode(), 8080, timeout=30)
        headers = {'Authorization': 'Bearer ' + fixture['admin_session']}
    else:
        connection = http.client.HTTPSConnection('oa-space-service-api-ezxz4mgdsq-uc.a.run.app', timeout=30)
        headers = {'X-API-Key': fixture['api_key'], 'Authorization': 'Bearer ' + fixture['api_key']}
    headers['Content-Type'] = 'application/json'
    try:
        connection.request(method, path, body=json.dumps(data).encode() if data is not None else None, headers=headers)
        response = connection.getresponse()
        raw = response.read(1048577)
        if len(raw)>1048576: raise RuntimeError('The qualification response exceeded its size limit.')
        value = json.loads(raw) if raw else None
        if not 200 <= response.status < 300:
            # Native API errors contain no caller headers or raw credentials.
            raise RuntimeError(f'Native request failed with HTTP {response.status}: {json.dumps(value)[:1000]}')
        return value
    finally:
        connection.close()


def main(action):
    if os.getuid()!=0: raise RuntimeError('Run qualification through the owned control identity.')
    ROOT.mkdir(mode=0o700, exist_ok=True)
    if action=='probe':
        nodes=request(True,'GET','/admin/api/v1/nodes')
        (ROOT/'nodes.json').write_text(json.dumps(nodes,indent=2)+'\n')
        sandboxes=request(False,'GET','/api/v1/sandboxes')
        print(json.dumps({'admin_nodes': nodes, 'native_sandboxes': sandboxes}))
    elif action=='build':
        state=ROOT/'seed-build.json'
        if state.exists(): raise RuntimeError('Inspect the saved build before submitting another.')
        # Create the marker first: a lost response must not cause a duplicate build.
        state.write_text(json.dumps({'state':'submission_pending','template':'seed'})+'\n')
        value=request(True,'POST','/admin/api/v1/system-templates/seed/builds',{'publish_tags':['latest']})
        state.write_text(json.dumps(value,indent=2)+'\n')
        print(json.dumps(value))
    else:
        value=json.loads((ROOT/'seed-build.json').read_text())
        job=value.get('job_id')
        if not job: raise RuntimeError('No build receipt is saved. Inspect the operation before retrying submission.')
        import uuid
        job=str(uuid.UUID(job))
        value=request(True,'GET','/admin/api/v1/templates/seed/builds/'+job)
        (ROOT/'seed-build-status.json').write_text(json.dumps(value,indent=2)+'\n')
        print(json.dumps(value))


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('action',choices=['probe','build','status'])
    main(parser.parse_args().action)
