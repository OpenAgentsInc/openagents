#!/usr/bin/env python3
"""Check the deployed file reference and signed-out routes without creating data."""
import argparse
import hashlib
import json
from pathlib import Path
import urllib.error
import urllib.request

from check_keys_public import NoRedirect
import public_reference


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--api', default='https://api.openagents.com')
    parser.add_argument('--website', default='https://openagents.com')
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    api, website = args.api.rstrip('/'), args.website.rstrip('/')
    opener = urllib.request.build_opener(NoRedirect())
    with opener.open(api + '/v1/openapi.json', timeout=30) as response:
        operations = public_reference.check(json.load(response), expected_origin=api)
    project = 'prj_0000000000000000'
    file = '/v1/files/file_000000000000000000000000'
    attachments = '/v1/projects/' + project + '/attachments'
    upload = '/v1/files/uploads/upl_000000000000000000000000'
    query = '?project=' + project
    cases = [
        ('POST', '/v1/files?filename=probe.txt', b'x'),
        ('GET', file, None), ('DELETE', file, None),
        ('GET', file + '/content', None),
        ('GET', attachments, None),
        ('POST', attachments, {'document': {'title': 'Probe', 'text': 'Probe'}}),
        ('DELETE', attachments + '/file_000000000000000000000000', None),
        ('GET', '/v1/projects/' + project + '/documents/pdoc_000000000000000000000000', None),
        ('POST', '/v1/files/uploads', {'request_id': '0' * 64, 'project': project,
             'filename': 'probe.txt', 'size_bytes': 1, 'sha256': hashlib.sha256(b'x').hexdigest()}),
        ('GET', upload + query, None), ('DELETE', upload + query, None),
        ('PUT', upload + '/parts/0' + query, b'x'),
        ('POST', upload + '/complete' + query, None),
    ]
    probes = []
    requests = [(api + path, method, body, 401) for method, path, body in cases]
    if website != api:
        requests += [(website + file, 'GET', None, 401),
                     (website + '/api' + file, 'GET', None, 404)]
    for url, method, body, expected in requests:
        data = json.dumps(body).encode() if isinstance(body, dict) else body
        headers = {'Content-Type': 'application/json' if isinstance(body, dict)
                   else 'application/octet-stream', 'X-Part-SHA256': hashlib.sha256(b'x').hexdigest()}
        try:
            opener.open(urllib.request.Request(url, method=method, data=data, headers=headers), timeout=30)
        except urllib.error.HTTPError as response:
            assert response.code == expected, (method, url, response.code)
            if expected == 401:
                assert json.load(response)['error']['code'] == 'signed_out'
        else:
            raise AssertionError('File operation unexpectedly accepted a signed-out request')
        probes.append({'method': method, 'url': url, 'status': expected})
    result = {'operations': operations, 'file_operations': 13, 'routing_probes': probes,
              'credentials_used': False, 'objects_created': 0}
    if args.output:
        args.output.write_text(json.dumps(result, indent=2) + '\n')
    print(f'PASS file reference: {operations} operations; {len(probes)} signed-out routing probes')


if __name__ == '__main__':
    main()
