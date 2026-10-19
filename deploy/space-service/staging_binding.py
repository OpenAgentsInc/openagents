#!/usr/bin/env python3
"""Bind one dedicated staging test account to the private SPACE deployment.

Owner-approved for #11816 on 2026-10-18. Staging Computers keeps Boat as the
default and its existing `oa-boat-primary` connection (same fingerprint inputs
as the single-driver service), adds `space-private-11814` for exactly one
account, and reaches the internal SPACE API through Direct VPC egress on the
`oa-space-service-staging-run` subnet, which has NAT for Boat and Cloud SQL.
Pricing stays off. `--disable-space` keeps the connection and stops new SPACE
placement; `--rollback REVISION` returns all traffic to an earlier revision.
"""
import argparse
import hashlib
import json
import re
import subprocess

PROJECT = 'openagentsgemini'
REGION = 'us-central1'
SERVICE = 'oa-computers-staging'
RUNTIME = 'oa-vertex-inference@openagentsgemini.iam.gserviceaccount.com'
BOAT = 'https://oa-boat-157437760789.us-central1.run.app/api/v1'
PREFIX = b'openagents.web.chat.account.v1\0'


def gcloud(*args, data=None):
    result = subprocess.run(['gcloud', *args, '--project', PROJECT], input=data, capture_output=True)
    if result.returncode:
        raise RuntimeError('gcloud ' + ' '.join(args[:3]) + ' failed: ' + result.stderr.decode()[-400:])
    return result.stdout.decode()


def owner(account):
    if not re.fullmatch(r'acct_[0-9a-f]{16}', account):
        raise RuntimeError('Name one dedicated test account as acct_ followed by 16 hex digits.')
    return 'account:' + hashlib.sha256(PREFIX + account.encode()).hexdigest()


def registry(account, admit_space):
    return {'default': 'oa-boat-primary', 'policy': 'registry-v1', 'deployments': [
        {'id': 'oa-boat-primary', 'kind': 'boat', 'api_url': BOAT, 'credential_env': 'BOAT_API_KEY', 'legacy': True},
        {'id': 'space-private-11814', 'kind': 'space', 'api_url': 'https://oa-space-service-api-ezxz4mgdsq-uc.a.run.app',
         'data_plane_url': 'https://oa-space-service-data-ezxz4mgdsq-uc.a.run.app', 'credential_env': 'SPACE_API_TOKEN',
         'template': 'code-runtime@sha256:e2db8e6f2e96c6c61bea066c03a1d675a2afa1915dd6f6a1e2bd298ab2344149',
         'scope': '00000000-0000-4000-8000-000000011814', 'capacity': 2, 'admit_new': admit_space,
         'rollout_accounts': [owner(account)], 'shapes': {'small': {'vcpus': 2, 'memory_mib': 1024, 'min_disk_gib': None}}}]}


def apply(account, admit_space):
    body = json.dumps(registry(account, admit_space), separators=(',', ':')).encode()
    name = 'oa-computers-staging-registry-' + hashlib.sha256(body).hexdigest()[:16]
    if subprocess.run(['gcloud', 'secrets', 'describe', name, '--project', PROJECT], capture_output=True).returncode:
        gcloud('secrets', 'create', name, '--replication-policy=automatic', '--data-file=-',
               '--labels=openagents-issue=11816,openagents-service=space', data=body)
    for secret in (name, 'oa-space-service-computers-space-token'):
        gcloud('secrets', 'add-iam-policy-binding', secret, '--member=serviceAccount:' + RUNTIME,
               '--role=roles/secretmanager.secretAccessor', '--format=none')
    gcloud('run', 'services', 'update', SERVICE, '--region', REGION, '--quiet',
           '--network=oa-space-service', '--subnet=oa-space-service-staging-run', '--vpc-egress=all-traffic',
           '--update-secrets', f'SPACE_API_TOKEN=oa-space-service-computers-space-token:1,/var/run/space-registry/registry.json={name}:1',
           '--update-env-vars', 'OA_COMPUTERS_REGISTRY_FILE=/var/run/space-registry/registry.json')
    revision = json.loads(gcloud('run', 'services', 'describe', SERVICE, '--region', REGION, '--format=json'))['status']['latestReadyRevisionName']
    print(json.dumps({'service': SERVICE, 'revision': revision, 'registry_secret': name, 'default': 'oa-boat-primary',
                      'space_admit_new': admit_space, 'space_rollout_accounts': 1, 'pricing': 'off'}))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--account', help='The dedicated staging test account (acct_...).')
    parser.add_argument('--disable-space', action='store_true')
    parser.add_argument('--rollback', help='Send all traffic to this earlier revision.')
    args = parser.parse_args()
    if args.rollback:
        gcloud('run', 'services', 'update-traffic', SERVICE, '--region', REGION, '--to-revisions', args.rollback + '=100', '--quiet')
        print(json.dumps({'service': SERVICE, 'serving': args.rollback}))
    else:
        apply(args.account, not args.disable_space)


if __name__ == '__main__':
    main()
