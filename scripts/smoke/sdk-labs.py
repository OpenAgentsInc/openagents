#!/usr/bin/env python3
"""Qualify the gated Labs API with two explicit test accounts and remove fixtures."""
import argparse
import base64
import datetime
import json
import os
from pathlib import Path
import urllib.error
import urllib.request
import uuid


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *_args, **_kwargs):
        return None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--api', required=True)
    parser.add_argument('--commit', required=True)
    parser.add_argument('--evidence', type=Path, required=True)
    args = parser.parse_args()
    primary = os.environ.get('OPENAGENTS_TOKEN', '')
    other = os.environ.get('OPENAGENTS_OTHER_TOKEN', '')
    assert primary and other and primary != other, 'Two explicit test-account credentials are required'
    opener = urllib.request.build_opener(NoRedirect())
    origin = args.api.rstrip('/')
    assert origin.startswith('https://'), 'Use a public HTTPS deployment'
    checks = []

    def call(method, path, data=None, token=primary, extra=None):
        headers = {'Accept': 'application/json', 'X-OpenAgents-Test': '1'}
        if token:
            headers['Authorization'] = 'Bearer ' + token
        if data is not None:
            headers['Content-Type'] = 'application/json'
        headers.update(extra or {})
        req = urllib.request.Request(origin + path,
            data=json.dumps(data).encode() if data is not None else None,
            headers=headers, method=method)
        try:
            response = opener.open(req, timeout=30)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            raw = response.read(4 * 1024 * 1024 + 1)
            assert len(raw) <= 4 * 1024 * 1024, 'Response exceeds the test limit'
            kind = response.headers.get('Content-Type', '')
            return response.status, json.loads(raw) if 'application/json' in kind and raw else raw, response.headers

    def check(name, condition):
        assert condition, name
        checks.append(name)
        print('PASS', name, flush=True)

    accounts = []
    for credential in (primary, other):
        status, body, _ = call('GET', '/v1/account', token=credential)
        assert status == 200 and body.get('account', {}).get('id'), 'Test account did not authenticate'
        account = body['account']
        assert 'chris@openagents.com' not in json.dumps(account).lower(), 'Owner accounts are not test fixtures'
        accounts.append(account['id'])
    assert accounts[0] != accounts[1], 'Isolation needs separate accounts'
    status, hidden, _ = call('GET', '/v1/labs/kinds', token=other)
    check('second-account kind gate', status == 200 and not any(k['kind'] == 'echo' for k in hidden['data'])
          and call('GET', '/v1/labs/kinds/echo/tools', token=other)[0] == 403)
    check('signed-out refusal', call('GET', '/v1/labs/kinds', token=None)[0] == 401)
    status, kinds, _ = call('GET', '/v1/labs/kinds')
    check('gated Echo kind', status == 200 and any(k['kind'] == 'echo' for k in kinds['data']))
    identifier = 'labs-api-' + uuid.uuid4().hex
    print('Fixture:', identifier, flush=True)
    job_id = artifact_id = None
    created = False
    cleaned = False
    prefix = '/v1/labs/documents/' + identifier
    try:
        request = {'id': identifier, 'kind': 'echo', 'schema': 'echo.v1', 'title': 'Labs API fixture',
                   'content': {'text': 'Synthetic first version'}, 'summary': 'Created for qualification'}
        status, version, _ = call('POST', '/v1/labs/documents', request)
        created = status == 201
        check('create document', created and version['number'] == 1)
        edit = {'base': 1, 'change': {'type': 'intent', 'intent': {'intent': 'set', 'text': 'Synthetic second version'}}, 'summary': 'Updated for qualification'}
        check('checked edit', call('POST', prefix + '/versions', edit)[0] == 201)
        check('stale base refusal', call('POST', prefix + '/versions', edit)[0] == 409)
        edit['base'] = 2
        edit['change']['intent']['text'] = ''
        check('kind refusal', call('POST', prefix + '/versions', edit)[0] == 422)
        check('version and card reads', call('GET', prefix + '/versions/2')[0] == 200 and call('GET', prefix + '/versions/2/card')[0] == 200)
        work = {'version': {'document': identifier, 'version': 2}, 'want': {'want': 'card'}}
        retry = {'Idempotency-Key': identifier}
        status, job, _ = call('POST', '/v1/labs/jobs', work, extra=retry)
        job_id = job.get('id') if status == 201 else None
        check('job admission', status == 201 and job['status'] == 'queued')
        repeated = call('POST', '/v1/labs/jobs', work, extra=retry)
        check('job retry identity', repeated[0] == 201 and repeated[1]['id'] == job_id)
        payload = b'Synthetic Labs file\n'
        upload = {'version': {'document': identifier, 'version': 2}, 'media_type': 'text/plain',
                  'class': 'no_training', 'content_base64': base64.b64encode(payload).decode()}
        status, artifact, _ = call('POST', '/v1/labs/uploads', upload)
        artifact_id = artifact.get('id') if status == 201 else None
        check('bounded source upload', status == 201 and artifact['no_training'])
        artifact_path = '/v1/labs/artifacts/' + artifact_id
        status, link, _ = call('GET', artifact_path)
        check('short signed file link', status == 200 and link['expires_seconds'] == 60 and link['url'].startswith('https://storage.googleapis.com/'))
        req = urllib.request.Request(link['url'], headers={'Range': 'bytes=1-4'})
        try:
            with opener.open(req, timeout=30) as response:
                check('signed file range', response.status == 206 and response.read(16) == payload[1:5])
        except urllib.error.URLError:
            raise AssertionError('Signed download failed') from None
        status, part, headers = call('GET', artifact_path + '/content', extra={'Range': 'bytes=1-4'})
        check('authenticated file range', status == 206 and part == payload[1:5] and headers.get('Content-Range') == f'bytes 1-4/{len(payload)}')
        check('add note', call('POST', prefix + '/notes', {'version': 2, 'locator': {'at': 'none'}, 'text': 'Synthetic review note'})[0] == 201)
        check('read notes', call('GET', prefix + '/notes')[0] == 200)
        status, events, _ = call('GET', prefix + '/events?since=0')
        cursors = [event['cursor'] for event in events.get('data', [])]
        check('ordered event polling', status == 200 and len(cursors) >= 5 and cursors == sorted(set(cursors)))
        req = urllib.request.Request(origin + prefix + '/events', headers={'Authorization': 'Bearer ' + primary, 'Accept': 'text/event-stream', 'X-OpenAgents-Test': '1'})
        with opener.open(req, timeout=15) as response:
            lines = []
            for _ in range(8):
                line = response.readline(1024 * 1024)
                lines.append(line)
                if line in (b'\n', b'\r\n'):
                    break
            check('event stream', response.status == 200 and b'event: changes' in b''.join(lines))
        reads = [prefix, prefix + '/versions', prefix + '/versions/2', prefix + '/versions/2/card',
                 prefix + '/notes', prefix + '/events', '/v1/labs/jobs/' + job_id,
                 artifact_path, artifact_path + '/content']
        check('second-account reads refused', all(call('GET', path, token=other)[0] == 404 for path in reads))
        check('second-account changes refused', call('POST', prefix + '/versions', edit, token=other)[0] == 404
              and call('DELETE', prefix, token=other)[0] == 404
              and call('POST', '/v1/labs/jobs', work, token=other, extra={'Idempotency-Key': identifier + '-other'})[0] == 404
              and call('POST', '/v1/labs/uploads', upload, token=other)[0] == 404
              and call('DELETE', '/v1/labs/jobs/' + job_id, token=other)[0] == 404
              and call('POST', prefix + '/notes', {'version': 2, 'locator': {'at': 'none'}, 'text': 'Denied'}, token=other)[0] == 404)
        check('read did not start work', call('GET', '/v1/labs/jobs/' + job_id)[1]['status'] == 'queued')
    finally:
        if job_id:
            status = call('DELETE', '/v1/labs/jobs/' + job_id)[0]
            assert status in (200, 404), 'Job cancellation failed'
        # Clean the chosen ID even if the create response was lost.
        assert call('DELETE', prefix)[0] in (200, 404), 'Document cleanup failed'
        assert call('GET', prefix)[0] == 404, 'Document remains readable'
        if artifact_id:
            assert call('GET', '/v1/labs/artifacts/' + artifact_id + '/content')[0] == 404, 'Artifact remains readable'
        cleaned = True
        print('Cleanup: fixture document and job removed from the API; artifact reads refused', flush=True)
    evidence = {'schema': 'openagents.labs-api-evidence.v1', 'checked_at': datetime.datetime.now(datetime.timezone.utc).isoformat(),
                'api': origin, 'source_commit': args.commit, 'test_accounts': accounts, 'checks': checks,
                'cleanup': {'verified': cleaned, 'document_read': 404, 'artifact_read': 404},
                'scope': 'Gated Echo fixture API; CAD and other kind integrations have separate issues.'}
    args.evidence.parent.mkdir(parents=True, exist_ok=True)
    args.evidence.write_text(json.dumps(evidence, indent=2) + '\n')
    print('PASS Labs API qualification', flush=True)


if __name__ == '__main__':
    main()
