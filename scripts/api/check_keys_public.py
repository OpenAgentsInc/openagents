#!/usr/bin/env python3
"""Check the deployed key reference and signed-out behavior without credentials."""
import argparse
import json
from pathlib import Path
import urllib.error
import urllib.request

import public_reference


def check(api, website):
    with urllib.request.urlopen(api + '/v1/openapi.json', timeout=30) as response:
        document = json.load(response)
    operations = public_reference.check(document, expected_origin=api)
    probes = []
    bases = [api + '/v1']
    if website != api:
        bases += [website + '/v1', website + '/api/v1']
    for base in bases:
        for method, path in [('GET', '/keys'), ('POST', '/keys'), ('DELETE', '/keys/schema-probe')]:
            # No bearer or cookie, so these requests cannot create or revoke a key.
            data = b'{}' if method == 'POST' else None
            request = urllib.request.Request(base + path, data=data, method=method,
                                             headers={'Content-Type': 'application/json'})
            try:
                urllib.request.urlopen(request, timeout=30)
            except urllib.error.HTTPError as response:
                status, headers, body = response.code, response.headers, json.load(response)
            else:
                raise AssertionError('Signed-out key operation unexpectedly succeeded')
            assert status == 401, (method, base + path, status)
            assert body['error']['code'] == 'signed_out'
            assert headers['Cache-Control'] == 'no-store, private'
            if base != api + '/v1':
                assert headers['Deprecation'] == 'true'
            probes.append({'method': method, 'url': base + path, 'status': status,
                           'code': body['error']['code']})
    return {'operations': operations, 'key_operations': 3, 'signed_out_probes': probes,
            'credentials_used': False, 'keys_created': 0, 'keys_revoked': 0}


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
          f"{len(result['signed_out_probes'])} signed-out probes; no credentials or key changes")


if __name__ == '__main__':
    main()
