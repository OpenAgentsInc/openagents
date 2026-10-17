#!/usr/bin/env python3
"""Qualify CAD versions, face pulls, and STEP exports with dedicated accounts."""
import argparse
import datetime
import hashlib
import http.cookiejar
import json
import os
from pathlib import Path
import subprocess
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--api', required=True)
    parser.add_argument('--web', required=True)
    parser.add_argument('--commit', required=True)
    parser.add_argument('--evidence', type=Path, required=True)
    parser.add_argument('--browser-capture', type=Path)
    args = parser.parse_args()
    primary = os.environ.get('OPENAGENTS_TOKEN', '')
    other = os.environ.get('OPENAGENTS_OTHER_TOKEN', '')
    assert primary and other and primary != other, 'Two dedicated test accounts are required'
    assert args.api.startswith('https://') and args.web.startswith('https://'), 'Use HTTPS deployments'
    class NoRedirect(urllib.request.HTTPRedirectHandler):
        def redirect_request(self, *_args, **_kwargs):
            return None
    opener = urllib.request.build_opener(NoRedirect())
    class WebsiteRedirect(urllib.request.HTTPRedirectHandler):
        def redirect_request(self, request, response, code, message, headers, url):
            target = urllib.parse.urlsplit(url)
            expected = urllib.parse.urlsplit(args.web)
            if target.scheme != 'https' or target.netloc != expected.netloc:
                return None
            return super().redirect_request(request, response, code, message, headers, url)
    browser_cookies = http.cookiejar.CookieJar()
    website = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(browser_cookies), WebsiteRedirect())
    browser_session = None
    issued_browser_session = False
    def call(method, path, data=None, token=primary, extra=None, web=False):
        headers = {'Accept': 'application/json', 'X-OpenAgents-Test': '1'}
        if token and (not web or not browser_session):
            credential = browser_session if token == primary and browser_session else token
            headers['Authorization'] = 'Bearer ' + credential
        if data is not None:
            headers['Content-Type'] = 'application/json'
        headers.update(extra or {})
        request = urllib.request.Request((args.web if web else args.api).rstrip('/') + path,
            data=json.dumps(data).encode() if data is not None else None, headers=headers, method=method)
        try:
            response = (website if web else opener).open(request, timeout=100)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            raw = response.read(16 * 1024 * 1024 + 1)
            assert len(raw) <= 16 * 1024 * 1024, 'Response exceeds the test limit'
            return response.status, json.loads(raw) if raw and 'application/json' in response.headers.get('Content-Type', '') else raw
    checks = []
    def check(name, condition):
        assert condition, name
        checks.append(name)
        print('PASS', name, flush=True)
    accounts = []
    for credential in (primary, other):
        status, body = call('GET', '/v1/account', token=credential)
        assert status == 200 and 'chris@openagents.com' not in json.dumps(body).lower(), 'Use dedicated test accounts'
        accounts.append(body['account']['id'])
    assert accounts[0] != accounts[1]
    identifier = 'fan-' + uuid.uuid4().hex[:16]
    project_name = 'CAD Labs qualification ' + uuid.uuid4().hex[:16]
    project = job = artifact = None
    hashes = []
    try:
        if primary.startswith(('sess_', 'oa_sk_')):
            browser_session = primary
        else:
            status, signed = call('POST', '/api/v1/sessions', token=primary, web=True)
            browser_session = signed.get('token') if status in (200, 201) else None
            issued_browser_session = bool(browser_session)
        assert browser_session and browser_session.startswith(('sess_', 'oa_sk_')), 'A dedicated browser credential is required'
        browser_cookies.set_cookie(http.cookiejar.Cookie(0, 'oa_cloud_session', browser_session,
            None, False, urllib.parse.urlsplit(args.web).hostname, False, False,
            '/', True, True, None, True, None, None, {'HttpOnly': None}, False))
        status, signed = call('GET', '/v1/account', token=browser_session)
        check('dedicated browser session', status == 200 and signed['account']['id'] == accounts[0])
        status, body = call('POST', '/v1/projects', {'name': project_name})
        project = body.get('id') if status == 201 else None
        check('dedicated project', project is not None)
        space = 'project:' + project
        query = '?' + urllib.parse.urlencode({'space': space})
        prefix = '/v1/labs/documents/' + identifier
        features = [{'id': 'body', 'op': 'solid.box', 'params': {'length': 50, 'width': 50, 'height': 3, 'center': [0, 0, 1.5]}},
                    {'id': 'airflow', 'op': 'solid.cylinder', 'params': {'diameter': 36, 'height': 20, 'base': [0, 0, -5], 'operation': 'cut'}}]
        for number, (x, y) in enumerate([(-16, -16), (16, -16), (16, 16), (-16, 16)]):
            features.append({'id': 'mount-' + str(number), 'op': 'solid.cylinder',
                             'params': {'diameter': 3.4, 'height': 20, 'base': [x, y, -5], 'operation': 'cut'}})
        content = {'kind': 'part.v1', 'id': identifier, 'title': '40 mm fan mounting bracket',
                   'material': 'Aluminum', 'parameters': [], 'features': features}
        request = {'space': space, 'arguments': {'doc': identifier, 'document': content, 'summary': 'Designed a bracket for a 40 mm fan'}}
        status, body = call('POST', '/v1/labs/kinds/cad/tools/cad_from_json', request)
        check('checked fan bracket', status == 200 and body['result']['version']['number'] == 1)
        hashes.append(body['result']['version']['content_sha256'])
        if args.browser_capture:
            page = args.web.rstrip('/') + '/cad/' + identifier + '?' + urllib.parse.urlencode({'v': 1, 'project': project})
            subprocess.run(['openagents', 'browser', 'run', '--', 'python3',
                str(Path(__file__).with_name('browser-labs-cad.py')), '--url', page,
                '--capture', str(args.browser_capture)], check=True, timeout=120,
                env={**os.environ, 'OPENAGENTS_BROWSER_TOKEN': browser_session})
            check('isolated browser picked the top face', True)
        # A named box face is the same selection the 3D viewer sends.
        edit = {'space': space, 'base': 1, 'arguments': {'doc': identifier, 'part': identifier, 'face': 'body:+z', 'delta_mm': 5}}
        status, body = call('POST', '/v1/labs/kinds/cad/tools/cad_drag_face', edit)
        check('picked face five millimeters thicker', status == 200 and body['result']['version']['number'] == 2
              and body['result']['version']['content']['features'][0]['params']['height'] == 8)
        hashes.append(body['result']['version']['content_sha256'])
        check('stale face pull refused', call('POST', '/v1/labs/kinds/cad/tools/cad_drag_face', edit)[0] == 409)
        edit['base'] = 2
        check('current Pro plan checked', call('POST', '/v1/labs/kinds/cad/tools/cad_drag_face', edit,
              extra={'X-OpenAgents-Test-Plan': 'free'})[0] == 402)
        status, body = call('GET', prefix + '/versions' + query)
        check('both versions in the project', status == 200 and [v['number'] for v in body['data']] == [1, 2])
        status, card = call('GET', prefix + '/versions/2/card' + query)
        check('3D card with poster and project link', status == 200 and card['view'] == 'model3d' and card['primary'] and card['poster'] and 'project=' in card['page'])
        page = '/cad/' + identifier + '?' + urllib.parse.urlencode({'v': 2, 'project': project})
        status, body = call('GET', page, web=True)
        check('existing CAD page and versions', status == 200 and b'40 mm fan mounting bracket' in body and b'v1' in body and b'v2' in body)
        work = {'space': space, 'version': {'document': identifier, 'version': 2},
                'want': {'want': 'job', 'type': 'export', 'params': {'formats': ['step']}}}
        retry = {'Idempotency-Key': identifier + '-step'}
        status, body = call('POST', '/v1/labs/jobs', work, extra=retry)
        job = body.get('id') if status == 201 else None
        check('STEP job admitted', job is not None)
        check('STEP retry shares its job', call('POST', '/v1/labs/jobs', work, extra=retry)[1]['id'] == job)
        deadline = time.monotonic() + 120
        while time.monotonic() < deadline:
            status, body = call('GET', '/v1/labs/jobs/' + job + query)
            assert status == 200, 'Job read failed'
            if body['status'] not in ('queued', 'running'):
                break
            time.sleep(2)
        check('STEP job completed', body['status'] == 'done')
        for candidate in body['artifacts']:
            status, link = call('GET', '/v1/labs/artifacts/' + candidate + query)
            if status == 200 and link['artifact']['media_type'] == 'model/step':
                artifact = candidate
                break
        check('STEP artifact recorded', artifact is not None)
        status, data = call('GET', '/v1/labs/artifacts/' + artifact + '/content' + query, extra={'Range': 'bytes=0-12'})
        check('STEP range download', status == 206 and data == b'ISO-10303-21;')
        check('second account cannot read the project design', call('GET', prefix + query, token=other)[0] == 404)
        check('second account cannot change the project design', call('POST', '/v1/labs/kinds/cad/tools/cad_drag_face', edit, token=other)[0] == 404)
        call('GET', prefix + '/versions/2/card' + query)
        check('reading a card starts no work', len(call('GET', '/v1/labs/jobs' + query + '&document=' + identifier)[1]['data']) == 1)
    finally:
        failures = []
        def cleanup(method, path, expected, label):
            try:
                if call(method, path)[0] not in expected:
                    failures.append(label)
            except Exception:
                failures.append(label)
        if project:
            query = '?' + urllib.parse.urlencode({'space': 'project:' + project})
            if job:
                cleanup('DELETE', '/v1/labs/jobs/' + job + query, (200, 404), 'Job cancellation')
            cleanup('DELETE', '/v1/labs/documents/' + identifier + query, (200, 404), 'Design deletion')
            cleanup('GET', '/v1/labs/documents/' + identifier + query, (404,), 'Design still readable')
            if artifact:
                cleanup('GET', '/v1/labs/artifacts/' + artifact + '/content' + query, (404,), 'STEP still readable')
            cleanup('DELETE', '/v1/projects/' + project, (200, 204, 404), 'Project deletion')
            cleanup('GET', '/v1/projects/' + project, (404,), 'Project still readable')
        else:
            # Recover a project even if its create response was lost.
            cursor = None
            seen = set()
            while True:
                params = {'limit': 200}
                if cursor:
                    params['after'] = cursor
                status, listing = call('GET', '/v1/projects?' + urllib.parse.urlencode(params))
                if status != 200:
                    failures.append('Project cleanup lookup')
                    break
                for item in listing['data']:
                    if item['name'] == project_name:
                        cleanup('DELETE', '/v1/projects/' + item['id'], (200, 204, 404), 'Recovered project deletion')
                cursor = listing.get('next')
                if not cursor:
                    break
                if cursor in seen:
                    failures.append('Project cleanup cursor did not advance')
                    break
                seen.add(cursor)
        if issued_browser_session:
            try:
                if call('POST', '/v1/device/sign-out', {}, token=browser_session)[0] not in (200, 204):
                    failures.append('Browser session sign-out')
            except Exception:
                failures.append('Browser session sign-out')
        assert not failures, 'Cleanup failed: ' + ', '.join(failures)
        print('PASS cleanup: design, STEP access, and dedicated project removed', flush=True)
    evidence = {'schema': 'openagents.labs-cad-evidence.v1', 'checked_at': datetime.datetime.now(datetime.timezone.utc).isoformat(),
                'source_commit': args.commit, 'api': args.api, 'test_accounts': accounts,
                'checks': checks, 'version_sha256': hashes, 'cleanup_verified': True}
    if args.browser_capture:
        evidence['browser_capture_sha256'] = hashlib.sha256(args.browser_capture.read_bytes()).hexdigest()
    args.evidence.parent.mkdir(parents=True, exist_ok=True)
    args.evidence.write_text(json.dumps(evidence, indent=2) + '\n')


if __name__ == '__main__':
    main()
