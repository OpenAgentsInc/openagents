#!/usr/bin/env python3
"""Render the synthetic lifecycle-recovery job for the release lane to review."""
import argparse
import copy
import json
from pathlib import Path
import re
import shlex

PROJECT = 'openagentsgemini'
PREFIX = 'oa-space-service'
PROXY = 'gcr.io/cloud-sql-connectors/cloud-sql-proxy@sha256:86e4f3cc266020e7ee07740df78d7511e2c340c274b56f84d80b7251913af9ad'


def render(service, digest):
    if not re.fullmatch(r'sha256:[0-9a-f]{64}', digest):
        raise ValueError('Use a checked image digest.')
    metadata = service['metadata']
    if metadata['name'] != PREFIX + '-computers' or metadata['labels'].get('openagents-issue') != '11814':
        raise ValueError('Use the maintained private Computers service.')
    template = service['spec']['template']
    spec = template['spec']
    identity = PREFIX + '-computers@' + PROJECT + '.iam.gserviceaccount.com'
    if spec['serviceAccountName'] != identity:
        raise ValueError('The private service identity differs.')
    annotations = template['metadata']['annotations']
    interfaces = json.loads(annotations['run.googleapis.com/network-interfaces'])
    expected_network = [{'network': PREFIX, 'subnetwork': PREFIX + '-run'}]
    if interfaces != expected_network or annotations['run.googleapis.com/vpc-access-egress'] != 'all-traffic':
        raise ValueError('The private service network differs.')
    containers = {c['name']: c for c in spec['containers']}
    env = next(e for e in containers['computers']['env'] if e['name'] == 'OA_COMPUTERS_DATABASE_URL')
    expected_secret = {'name': PREFIX + '-computers-proxy-dsn', 'key': '1'}
    if env.get('valueFrom', {}).get('secretKeyRef') != expected_secret or 'value' in env:
        raise ValueError('Use the existing numeric database secret reference.')
    proxy = copy.deepcopy(containers['db-proxy'])
    if proxy['image'] != PROXY or '--private-ip' not in proxy['args'] or proxy['args'][-1] != PROJECT + ':us-central1:' + PREFIX:
        raise ValueError('The checked private database proxy differs.')
    if set(proxy) - {'name', 'image', 'args', 'resources', 'startupProbe'}:
        raise ValueError('The database proxy has unsupported settings.')
    proxy['args'] += ['--quitquitquit', '--exit-zero-on-sigterm']
    evidence = Path(__file__).resolve().parents[2] / 'docs/sdk/computers-usage-evidence.json'
    recovery = json.loads(evidence.read_text())['lifecycle_retry_increment']['after_close']
    if recovery['account'] != 'space-qualification-11814' or recovery['operator_request']['computer'] != 'cmp_ef8caf1babae3b48ed87':
        raise ValueError('The saved synthetic recovery request differs.')
    request = json.dumps(recovery['operator_request'], separators=(',', ':'))
    # The proxy must exit on success and failure so the job has a finite life.
    script = '''set -eu
shutdown_proxy() {
    result=$?
    trap - EXIT
    if exec 3<>/dev/tcp/127.0.0.1/9091; then
        printf 'GET /quitquitquit HTTP/1.1\\r\\nHost: localhost\\r\\nConnection: close\\r\\n\\r\\n' >&3
        head -c 4096 <&3 >/dev/null || true
        exec 3>&-
    else
        result=1
    fi
    exit "$result"
}
trap shutdown_proxy EXIT
umask 077
printf '%s' REQUEST > /home/oa-computers/lifecycle-retry.json
/usr/local/bin/oa-computers-usage lifecycle-retry space-qualification-11814 /home/oa-computers/lifecycle-retry.json
'''.replace('REQUEST', shlex.quote(request))
    job_annotations = {
        'run.googleapis.com/vpc-access-egress': 'all-traffic',
        'run.googleapis.com/network-interfaces': json.dumps(expected_network),
        'run.googleapis.com/container-dependencies': json.dumps({'operator': ['db-proxy']}),
    }
    return {'apiVersion': 'run.googleapis.com/v1', 'kind': 'Job', 'metadata': {
        'name': 'oa-space-retry-11939', 'labels': {'openagents-issue': '11939', 'openagents-service': 'space'},
        'annotations': {'run.googleapis.com/launch-stage': 'BETA'}}, 'spec': {'template': {
            'metadata': {'annotations': job_annotations}, 'spec': {'parallelism': 1, 'taskCount': 1,
                'template': {'spec': {'serviceAccountName': identity, 'maxRetries': 0, 'timeoutSeconds': 120,
                    'containers': [{'name': 'operator', 'image': 'us-central1-docker.pkg.dev/' + PROJECT + '/' + PREFIX + '/computers@' + digest,
                        'command': ['/bin/bash'], 'args': ['-c', script],
                        'resources': {'limits': {'cpu': '1', 'memory': '512Mi'}}, 'env': [env]}, proxy]}}}}}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source-service', type=Path, required=True)
    parser.add_argument('--image-digest', required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    try:
        if args.source_service.stat().st_size > 1024 * 1024:
            raise ValueError('The service export exceeds 1 MiB.')
        job = render(json.loads(args.source_service.read_text()), args.image_digest)
    except (ValueError, KeyError, StopIteration, TypeError):
        raise SystemExit('The private recovery configuration could not be verified.') from None
    args.output.write_text(json.dumps(job, indent=2) + '\n')
    args.output.chmod(0o600)
    print(json.dumps({'job': job['metadata']['name'], 'rendered': True, 'deployed': False,
        'account': 'space-qualification-11814', 'tasks': 1, 'automatic_retries': 0, 'timeout_seconds': 120}))


if __name__ == '__main__':
    main()
