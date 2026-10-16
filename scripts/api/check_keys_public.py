#!/usr/bin/env python3
"""Check the deployed key reference and signed-out behavior without credentials."""
import argparse
import json
from pathlib import Path
import urllib.error
import urllib.request

import public_reference


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *_args, **_kwargs):
        return None


def check(api, website, resource='keys', qualified_operations=3):
    opener = urllib.request.build_opener(NoRedirect())
    with opener.open(api + '/v1/openapi.json', timeout=30) as response:
        document = json.load(response)
    operations = public_reference.check(document, expected_origin=api)
    probes = []
    bases = [api + '/v1']
    if website != api:
        bases += [website + '/v1', website + '/api/v1']
    for base in bases:
        for method, path in [('GET', '/' + resource), ('POST', '/' + resource),
                             ('DELETE', '/' + resource + '/schema-probe')]:
            # No bearer or cookie. Invalid JSON also prevents a valid creation.
            data = b'{' if method == 'POST' else None
            request = urllib.request.Request(base + path, data=data, method=method,
                                             headers={'Content-Type': 'application/json'})
            try:
                opener.open(request, timeout=30)
            except urllib.error.HTTPError as response:
                status, headers, raw = response.code, response.headers, response.read(65536)
            else:
                raise AssertionError('Signed-out operation unexpectedly succeeded')
            # This older prefix never mounted these web-owned handlers.
            # Preserve its 404 rather than claiming all aliases serve every API.
            if base == website + '/api/v1':
                assert status == 404, (method, base + path, status)
                probes.append({'method': method, 'url': base + path, 'status': status})
                continue
            body = json.loads(raw)
            assert status == 401, (method, base + path, status)
            assert body['error']['code'] == 'signed_out'
            assert headers['Cache-Control'] == 'no-store, private'
            if base != api + '/v1':
                assert headers['Deprecation'] == 'true'
            probes.append({'method': method, 'url': base + path, 'status': status,
                           'code': body['error']['code']})
    scope = 'key_operations' if resource == 'keys' else 'project_operations'
    return {'operations': operations, scope: qualified_operations, 'routing_probes': probes,
            'credentials_used': False, 'keys_created': 0, 'keys_revoked': 0,
            'projects_created': 0, 'projects_deleted': 0}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--api', default='https://api.openagents.com')
    parser.add_argument('--website', default='https://openagents.com')
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    result = check(args.api.rstrip('/'), args.website.rstrip('/'))
    if args.output:
        args.output.write_text(json.dumps(result, indent=2) + '\n')
    print(f"PASS key reference: {result['operations']} operations; "
          f"{len(result['routing_probes'])} routing/auth probes; no credentials or key changes")


if __name__ == '__main__':
    main()
