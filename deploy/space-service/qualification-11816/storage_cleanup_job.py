#!/usr/bin/env python3
"""#11816: render a one-task Cloud Run job that applies saved storage-cleanup proofs for one synthetic owner.

Reuses recovery_job.py's checks of the private Computers service, identity, network,
proxy pin, and numeric database secret. Deploys nothing; lane 0 reviews and runs it.
"""
import argparse, copy, json, re, shlex, sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import recovery_job

def render(service, digest, owner, proofs):
    if not re.fullmatch(r'account:[0-9a-f]{64}', owner) or not proofs or len(proofs) > 64:
        raise ValueError('Name one synthetic owner and 1 to 64 proofs.')
    for p in proofs:
        if p.get('schema') != 'openagents.computers.storage-cleanup.v1' or p.get('owner') != owner or p.get('retained_objects') or p.get('live_resources'):
            raise ValueError('Every proof must be a complete inventory for this owner.')
    job = recovery_job.render(service, digest)
    lines = ['set -eu', 'umask 077']
    script = job['spec']['template']['spec']['template']['spec']['containers'][0]['args'][1]
    head = script.split('umask 077', 1)[0]
    body = []
    for i, p in enumerate(proofs):
        path = f'/home/oa-computers/proof-{i}.json'
        body.append(f"printf '%s' {shlex.quote(json.dumps(p, separators=(',', ':')))} > {path}")
        body.append(f"/usr/local/bin/oa-computers-usage storage-cleanup {owner} {path}")
    body.append(f"/usr/local/bin/oa-computers-usage account-cleanup-status {owner}")
    job['spec']['template']['spec']['template']['spec']['containers'][0]['args'][1] = head + 'umask 077\n' + '\n'.join(body) + '\n'
    job['metadata']['name'] = 'oa-space-cleanup-11816'
    job['metadata']['labels'] = {'openagents-issue': '11816', 'openagents-service': 'space'}
    return job

if __name__ == '__main__':
    a = argparse.ArgumentParser(description=__doc__)
    a.add_argument('--source-service', type=Path, required=True); a.add_argument('--image-digest', required=True)
    a.add_argument('--owner', required=True); a.add_argument('--proofs', type=Path, required=True); a.add_argument('--output', type=Path, required=True)
    x = a.parse_args()
    job = render(json.loads(x.source_service.read_text()), x.image_digest, x.owner, json.loads(x.proofs.read_text()))
    x.output.write_text(json.dumps(job, indent=2) + '\n'); x.output.chmod(0o600)
    print(json.dumps({'job': job['metadata']['name'], 'rendered': True, 'deployed': False, 'proofs': len(json.loads(x.proofs.read_text()))}))
