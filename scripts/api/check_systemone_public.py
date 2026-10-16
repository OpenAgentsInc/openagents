#!/usr/bin/env python3
"""Qualify public System One schema and discovery without making an inference call."""
import argparse
import datetime
import hashlib
import json
from pathlib import Path
import urllib.error
import urllib.request
import public_reference


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *_args, **_kwargs):
        return None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--api', default='https://api.openagents.com', help='Public API origin, without /v1')
    parser.add_argument('--website', default='https://openagents.com')
    parser.add_argument('--output', type=Path)
    parser.add_argument('--browser-baseline', type=Path, help='A previously saved public vault/release.json')
    parser.add_argument('--malformed-call', action='store_true', help='Send only invalid JSON to verify routing/refusals; never send a state, questions, or credentials')
    args = parser.parse_args()
    api, site = args.api.rstrip('/'), args.website.rstrip('/')
    assert api.startswith('https://') and site.startswith('https://')
    opener = urllib.request.build_opener(NoRedirect())

    def call(url, method='GET', body=None):
        headers = {'Accept': 'application/json, text/event-stream'}
        if body is not None:
            headers['Content-Type'] = 'application/json'
        request = urllib.request.Request(url, data=body, method=method, headers=headers)
        try:
            result = opener.open(request, timeout=30)
        except urllib.error.HTTPError as error:
            result = error
        with result:
            raw = result.read(8 * 1024 * 1024 + 1)
            assert len(raw) <= 8 * 1024 * 1024
            return result.status, result.headers, raw

    def read_json(url):
        status, _, raw = call(url)
        assert status == 200, 'Public read failed: ' + url
        return json.loads(raw), hashlib.sha256(raw).hexdigest()

    document, digest = read_json(api + '/v1/openapi.json')
    operations = public_reference.check(document, api)
    assert '/v1/classify' not in document['paths'] and not any(p.startswith('/v1/jobs') for p in document['paths'])
    reference = api + '/v1/openapi.json'
    card, _ = read_json(site + '/.well-known/agent-card.json')
    assert card['x-openagents-api-reference']['url'] == reference
    interfaces = [i['url'] for i in card['supportedInterfaces']]
    assert api + '/v1/systemone' in interfaces
    assert not any(i.startswith(api + '/v1/classify') or i.startswith(api + '/v1/jobs') for i in interfaces)
    assert not any(s['id'] in ('batch-classification', 'durable-jobs') for s in card['skills'])
    catalog, _ = read_json(site + '/.well-known/api-catalog')
    entry = next(e for e in catalog['linkset'] if e['anchor'] == api + '/v1')
    assert entry['service-desc'][0]['href'] == reference
    mcp, _ = read_json(site + '/.well-known/mcp/server-card.json')
    assert mcp['x-openagents-api-reference'] == reference
    status, _, guide = call(site + '/docs/api/system-one.md')
    assert status == 200 and b'X-Workspace-Id' in guide
    status, _, llms = call(site + '/llms.txt')
    assert status == 200 and b'https://api.openagents.com/v1/openapi.json' in llms
    rpc = json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': 'tools/call',
                      'params': {'name': 'read_doc', 'arguments': {'name': 'api/system-one'}}}).encode()
    status, _, raw = call(site + '/mcp/docs', 'POST', rpc)
    reply = json.loads(raw)
    assert status == 200 and 'error' not in reply
    assert reply['result']['content'][0]['text'].startswith('# Typed decisions with System One')
    report = {'at_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
              'api': api, 'website': site, 'schema_sha256': digest, 'operations': operations,
              'systemone_slice': 'matches owning generated operations and components',
              'discovery': 'agent card, API catalog, MCP card, llms.txt and guide passed',
              'unpublished_operations': 'classification and jobs absent', 'malformed_calls': []}
    if args.browser_baseline:
        release, _ = read_json(site + '/vault/release.json')
        assert release['files'] == json.loads(args.browser_baseline.read_text())['files']
        report['browser_assets'] = 'unchanged'
    if args.malformed_call:
        urls = [api + '/v1/systemone']
        if api != site:
            urls += [site + '/v1/systemone', site + '/api/v1/systemone']
        for url in urls:
            # This is deliberately not JSON and cannot name a model, state or question.
            status, headers, raw = call(url, 'POST', b'{')
            error = json.loads(raw)['error']
            assert status == 400 and error['code'] in ('malformed', 'invalid_request')
            if api != site and url.startswith(site + '/'):
                assert headers.get('deprecation') == 'true'
            report['malformed_calls'].append({'url': url, 'status': status, 'code': error['code']})
    if args.output:
        args.output.write_text(json.dumps(report, indent=2) + '\n')
    print('PASS public System One:', operations, 'operations; schemas, discovery and scope verified')
    if args.malformed_call:
        print('PASS invalid-JSON refusals; no credential, state, questions, or inference sent')


if __name__ == '__main__':
    main()
