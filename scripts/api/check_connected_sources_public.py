#!/usr/bin/env python3
"""Check the deployed source schema and signed-out routes without credentials."""
import argparse
import json
from pathlib import Path
import urllib.error
import urllib.request

from check_keys_public import NoRedirect
from public_reference import check


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--api', default='https://api.openagents.com')
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    origin = args.api.rstrip('/')
    assert origin.startswith('https://'), 'Use an HTTPS deployment origin'
    opener = urllib.request.build_opener(NoRedirect())
    with opener.open(origin + '/v1/openapi.json', timeout=30) as response:
        document = json.load(response)
    operations = check(document, origin)
    selected = [(path, method) for path, item in document['paths'].items()
                for method, operation in item.items()
                if isinstance(operation, dict)
                and operation.get('tags') == ['Connected project sources']]
    assert len(selected) == 8, 'Expected eight qualified source operations'
    probes = []
    for path, method in selected:
        path = path.replace('{id}', 'prj_0000000000000000').replace('{source}', 'source-schema-0001').replace('{tool}', 'drive_search')
        request = urllib.request.Request(origin + path, method=method.upper(),
            data=b'{}' if method == 'post' else None,
            headers={'Content-Type': 'application/json'})
        try:
            opener.open(request, timeout=30)
        except urllib.error.HTTPError as response:
            assert response.code in (401, 404), (method, path, response.code)
            answer = json.load(response)
            assert answer['error']['code'] == ('signed_out' if response.code == 401 else 'not_found')
            probes.append({'method': method.upper(), 'path': path,
                           'status': response.code, 'code': answer['error']['code']})
        else:
            raise AssertionError('Source operation accepted a signed-out request')
    result = {'api': origin, 'operations': operations, 'source_operations': len(selected),
              'routing_probes': probes, 'credentials_used': False, 'objects_created': 0}
    if args.output:
        args.output.write_text(json.dumps(result, indent=2) + '\n')
    print(f'PASS connected sources: {len(selected)} operations and signed-out probes; no credentials or resource changes')


if __name__ == '__main__':
    main()
