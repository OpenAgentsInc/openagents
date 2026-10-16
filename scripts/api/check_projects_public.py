#!/usr/bin/env python3
"""Check the deployed project reference and signed-out routes without creating data."""
import argparse
import json
from pathlib import Path
import urllib.error
import urllib.request

from check_keys_public import NoRedirect, check


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--api', default='https://api.openagents.com')
    parser.add_argument('--website', default='https://openagents.com')
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    result = check(args.api.rstrip('/'), args.website.rstrip('/'), 'projects', 15)
    opener = urllib.request.build_opener(NoRedirect())
    project = '/v1/projects/prj_0000000000000000'
    # Synthetic IDs, no bearer or cookies. These cannot act for an account.
    for method, path, body in [
        ('GET', project, None),
        ('PATCH', project, {'instructions': None}),
        ('POST', project + '/repositories', {'repository': 'example/repository'}),
        ('DELETE', project + '/repositories/example/repository', None),
        ('GET', project + '/members', None),
        ('PATCH', project + '/members/schema-probe', {'role': 'viewer'}),
        ('DELETE', project + '/members/schema-probe', None),
        ('POST', project + '/leave', {}),
        ('GET', project + '/access', None),
        ('POST', project + '/invitations', {'role': 'viewer'}),
        ('POST', '/v1/projects/invitations/preview', {'code': 'prjinv_schema-probe'}),
        ('POST', '/v1/projects/invitations/accept', {'code': 'prjinv_schema-probe'}),
    ]:
        url = args.api.rstrip('/') + path
        request = urllib.request.Request(url, method=method,
            data=json.dumps(body).encode() if body is not None else None,
            headers={'Content-Type': 'application/json'})
        try:
            opener.open(request, timeout=30)
        except urllib.error.HTTPError as response:
            assert response.code == 401, (method, path, response.code)
            answer = json.load(response)
            assert answer['error']['code'] == 'signed_out'
        else:
            raise AssertionError('Project operation unexpectedly accepted a signed-out request')
        result['routing_probes'].append({'method': method, 'url': url,
                                       'status': 401, 'code': 'signed_out'})
    if args.output:
        args.output.write_text(json.dumps(result, indent=2) + '\n')
    print(f"PASS project reference: {result['operations']} operations; "
          f"{len(result['routing_probes'])} routing/auth probes; no credentials or project changes")


if __name__ == '__main__':
    main()
