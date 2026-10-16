#!/usr/bin/env python3
"""Check the published Computers contract; --live runs a dedicated-account flow.

Live checks require the approved test account and delete their computer. Tokens,
account details, command output, and file contents are never written to evidence.
"""
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import secrets
import shlex
import time
import urllib.error
import urllib.parse
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
DEPLOYMENTS = {
    'production': ('https://api.openagents.com', 'acct_83d16a35955ef908'),
    'staging': ('https://staging.openagents.com', 'acct_dc7a799879686fc5'),
}


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        return None


def compare_contract(published, expected, origin):
    assert published['openapi'] == '3.1.0'
    assert published['servers'] == [{'url': origin}]
    operations = {}
    for path, item in expected['paths'].items():
        for method, operation in item.items():
            assert published['paths'].get(path, {}).get(method) == operation, f'Operation drift: {method} {path}'
            ident = operation['operationId']
            assert ident not in operations, 'Duplicate operation ID'
            operations[ident] = (method.upper(), path, operation)
    for section, entries in expected['components'].items():
        for name, value in entries.items():
            assert published['components'].get(section, {}).get(name) == value, f'Component drift: {section}/{name}'
    return operations


def credential(path):
    token = os.environ.get('OPENAGENTS_TEST_TOKEN')
    if token:
        return token
    assert path.stat().st_mode & 0o077 == 0, 'Test credential file must be private'
    for line in path.read_text().splitlines():
        if line.strip().startswith('#') or '=' not in line:
            continue
        key, value = line.removeprefix('export ').split('=', 1)
        if key.strip() == 'OPENAGENTS_TOKEN':
            return shlex.split(value)[0]
    raise RuntimeError('Dedicated test credential unavailable')


class Check:
    def __init__(self, origin, operations, token):
        self.origin, self.operations, self.token = origin, operations, token
        self.http = urllib.request.build_opener(NoRedirect())
        self.records = []
        self.started = time.monotonic()

    def call(self, ident, parameters=None, query=None, body=None, binary=None, headers=None):
        method, path, _ = self.operations[ident]
        for key, value in (parameters or {}).items():
            path = path.replace('{' + key + '}', urllib.parse.quote(value, safe=''))
        assert '{' not in path and path.startswith('/v1/')
        if query:
            path += '?' + urllib.parse.urlencode(query)
        sent = {'Authorization': 'Bearer ' + self.token, 'X-OpenAgents-Test': '1'}
        sent.update(headers or {})
        data = binary
        if body is not None:
            data = json.dumps(body).encode()
            sent['Content-Type'] = 'application/json'
        elif binary is not None:
            sent.update({'Content-Type': 'application/octet-stream', 'X-Content-SHA256': hashlib.sha256(binary).hexdigest()})
        if data is not None or method in ('POST', 'PUT'):
            sent['Content-Length'] = str(len(data or b''))
        request = urllib.request.Request(self.origin + path, data=data, headers=sent, method=method)
        try:
            response = self.http.open(request, timeout=75)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            raw = response.read(4194304 + 1)
            assert len(raw) <= 4194304, 'Response exceeds the check bound'
            status, checksum = response.status, response.headers.get('X-Content-SHA256')
        try:
            value = json.loads(raw)
        except ValueError:
            value = {}
        self.records.append({'operation': ident, 'status': status, 'state': value.get('state'),
                             'elapsed_seconds': round(time.monotonic() - self.started, 3)})
        print(json.dumps(self.records[-1]), flush=True)
        return status, value, raw, checksum

    def observe(self, ident, parameters, predicate, seconds=360):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            status, value, _, _ = self.call(ident, parameters)
            if status in (429, 502, 503, 504):
                time.sleep(2)
                continue
            assert status == 200, 'Resource observation failed'
            if predicate(value):
                return value
            time.sleep(2)
        raise RuntimeError('Resource observation timed out')


def live(origin, expected_account, operations, token, output, schema_hash):
    check = Check(origin, operations, token)
    computer = None
    planned = False
    verified = False
    passed = False
    cleanup_errors = []
    key = 'sdk-contract-' + secrets.token_hex(12)
    create = {'body': {'size': 'small', 'idempotency_key': key}, 'headers': {'Idempotency-Key': key}}
    data = b'\x00\xffSDK contract\n'
    digest = hashlib.sha256(data).hexdigest()
    command = None
    try:
        status, account, _, _ = check.call('getAccount')
        assert status == 200 and account.get('account', {}).get('id') == expected_account, 'Use the dedicated test account'
        verified = True
        planned = True
        status, value, _, _ = check.call('createComputer', **create)
        assert status in (200, 202) and value.get('id', '').startswith('cmp_'), 'Create failed'
        computer = value['id']
        params = {'id': computer}
        check.observe('getComputer', params, lambda v: v.get('state') == 'awake')
        status, value, _, _ = check.call('writeComputerFile', params, {'path': 'input.bin'}, binary=data)
        assert status == 200 and value == {'size': len(data), 'sha256': digest}, 'Upload failed'
        status, value, _, _ = check.call('createComputerCommand', params,
            body={'command': 'sha256sum input.bin', 'timeout_seconds': 30, 'idempotency_key': key + '-command'},
            headers={'Idempotency-Key': key + '-command'})
        assert status == 202 and value.get('id', '').startswith('trn_'), 'Command creation failed'
        command = value['id']
        command_params = {**params, 'command': command}
        result = check.observe('getComputerCommand', command_params,
            lambda v: v.get('state') in ('succeeded', 'failed', 'cancelled', 'timed_out', 'unknown'))
        assert result['state'] == 'succeeded' and result['exit_code'] == 0, 'Hash command failed'
        status, value, _, _ = check.call('attachComputerCommand', command_params, {'max_bytes': 65536})
        assert status == 200 and value['encoding'] == 'base64' and value['terminal'], 'Output unavailable'
        assert base64.b64decode(value['stdout'], validate=True).startswith(digest.encode() + b' '), 'Guest hash differs'
        assert check.call('stopComputer', params)[0] == 202
        check.observe('getComputer', params, lambda v: v.get('state') == 'stopped')
        assert check.call('readComputerFile', params, {'path': 'input.bin'})[0] == 409, 'Read implicitly woke the computer'
        assert check.call('wakeComputer', params)[0] == 202
        check.observe('getComputer', params, lambda v: v.get('state') == 'awake')
        status, _, actual, checksum = check.call('readComputerFile', params, {'path': 'input.bin'})
        assert status == 200 and actual == data and checksum == digest, 'Readback differs'
        passed = True
    finally:
        if planned and computer is None:
            for _ in range(6):
                try:
                    status, value, _, _ = check.call('createComputer', **create)
                    if status in (200, 202) and value.get('id', '').startswith('cmp_'):
                        computer = value['id']
                        break
                except Exception:
                    pass
                time.sleep(2)
            if computer is None:
                cleanup_errors.append('Could not resolve the original create request')
        if computer:
            try:
                for _ in range(30):
                    if check.call('deleteComputer', {'id': computer}, {'kind': 'cloud'})[0] == 202:
                        break
                    time.sleep(2)
                else:
                    raise RuntimeError('Delete was not accepted')
                check.observe('getComputer', {'id': computer}, lambda v: v.get('state') == 'deleted', 480)
                cursor = None
                for _ in range(20):
                    query = {'kind': 'cloud', 'limit': 200}
                    if cursor:
                        query['after'] = cursor
                    status, page, _, _ = check.call('listComputers', query=query)
                    assert status == 200 and all(v.get('id') != computer for v in page['computers']), 'Deleted computer is listed'
                    cursor = page['next_cursor']
                    if not cursor:
                        break
                else:
                    raise RuntimeError('Cleanup list exceeded the check bound')
            except Exception as error:
                cleanup_errors.append(type(error).__name__)
        evidence = {'schema': 'openagents.computers.openapi.evidence.v1', 'origin': origin,
            'account': expected_account if verified else None, 'account_verified': verified, 'computer': computer, 'command': command,
            'passed': passed, 'cleaned': bool(computer) and not cleanup_errors,
            'cleanup_errors': cleanup_errors, 'request_key': key, 'contract_sha256': schema_hash,
            'file_bytes': len(data), 'file_sha256': digest, 'observations': check.records,
            'elapsed_seconds': round(time.monotonic() - check.started, 3)}
        output.write_text(json.dumps(evidence, indent=2) + '\n')
        assert not cleanup_errors, 'Check cleanup failed; inspect retained evidence'
    assert passed


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--mode', choices=DEPLOYMENTS, default='production')
    parser.add_argument('--live', action='store_true')
    parser.add_argument('--evidence', type=Path)
    parser.add_argument('--token-env-file', type=Path, default=Path.home() / 'work/.secrets/openagents-test-account.env')
    args = parser.parse_args()
    origin, account = DEPLOYMENTS[args.mode]
    expected_raw = (ROOT / 'docs/sdk/openapi.computers.json').read_bytes()
    request = urllib.request.Request(origin + '/v1/openapi.json', headers={'Accept': 'application/json'})
    with urllib.request.build_opener(NoRedirect()).open(request, timeout=30) as response:
        raw = response.read(8 * 1024 * 1024 + 1)
        assert len(raw) <= 8 * 1024 * 1024
    operations = compare_contract(json.loads(raw), json.loads(expected_raw), origin)
    print('PASS published Computers contract:', len(operations), 'operations', flush=True)
    if args.live:
        assert args.evidence, '--live requires --evidence for cleanup records'
        live(origin, account, operations, credential(args.token_env_file), args.evidence, hashlib.sha256(expected_raw).hexdigest())
        print('PASS independent public workflow and cleanup')


if __name__ == '__main__':
    main()
