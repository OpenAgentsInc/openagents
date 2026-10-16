#!/usr/bin/env python3
"""Check the combined public schema against front-door rules and qualified slices."""
import argparse
import json
from pathlib import Path
import re
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
METHODS = {'get', 'post', 'put', 'patch', 'delete', 'head', 'options', 'trace'}
INVENTORY = ROOT / 'docs/sdk/route-inventory.json'


def rules():
    source = (ROOT / 'crates/openagents-web/src/api_alias.rs').read_text()
    table = source.split('const PUBLIC:', 1)[1].split('\n];', 1)[0]
    pattern = r'Rule\(\s*(GET|POST|Methods::Any|Methods::Only\(&\[[^\]]*\]\)),\s*"([^"]+)"\s*,?\s*\)'
    result = []
    for method, path in re.findall(pattern, table):
        allowed = METHODS if method == 'Methods::Any' else {method.lower()} if method in ('GET', 'POST') else {s.lower() for s in re.findall(r'"([A-Z]+)"', method)}
        result.append((path, allowed))
    assert len(result) == table.count('Rule('), 'Unread forwarding rule; update the checker'
    return result


def forwarded(path, method):
    for pattern, methods in rules():
        parts = pattern.split('/')
        expression = '/'.join('.*' if p == '**' else '[^/]+' if p == '*' else re.escape(p) for p in parts)
        if pattern.endswith('/**'):
            expression = expression[:-len('/.*')] + '(?:/.*)?'
        if re.fullmatch(expression, path) and ('get' if method == 'head' else method) in methods:
            return True
    return False


def source_inventory():
    # This is a source navigation index, not a generated client contract.
    # Keep dynamic mounts in the owning Rust route tests rather than guessing them.
    found = []
    constant_mounts = []
    for file in sorted((ROOT / 'crates/openagents-web/src').rglob('*.rs')):
        if 'tests' in file.parts or 'test' in file.stem:
            continue
        source = file.read_text().split('#[cfg(test)]', 1)[0]
        constants = dict(re.findall(r'const\s+(\w+):\s*&str\s*=\s*"([^"]+)"', source))
        for match in re.finditer(r'\.route\(\s*([A-Z][A-Z_0-9]*)\s*,', source):
            path = constants.get(match[1], '')
            if path.startswith('/v1/'):
                constant_mounts.append({'path': path, 'source': str(file.relative_to(ROOT)), 'constant': match[1]})
        for match in re.finditer(r'\.route\(\s*"(/v1/[^"\s]+)"', source):
            found.append({'path': match[1], 'source': str(file.relative_to(ROOT)), 'line': source[:match.start()].count('\n') + 1})
    source = (ROOT / 'crates/gateway/src/audience.rs').read_text()
    gateway = []
    for path, audience, tag in re.findall(r'route\("(/v1/[^"]+)", (Public|FirstParty|Internal), "([^"]*)"', source):
        gateway.append({'path': path, 'audience': audience, 'domain': tag,
                        'public_front_pattern': any(forwarded(path, method) for method in METHODS)})
    return {'format': 'openagents.route-inventory.v1',
            'scope': 'Literal web mounts and gateway audience declarations; not a public schema or permission to expose a route. Constants/dynamic mounts remain in owning route tests.',
            'web_literal_mounts': found, 'web_constant_mounts': constant_mounts, 'gateway_audience': gateway,
            'forwarding_rules': [{'pattern': p, 'methods': sorted(m)} for p, m in rules()]}


def check(document, expected_origin=None, complete=True):
    assert document.get('openapi') == '3.1.0', 'Expected OpenAPI 3.1'
    servers = document.get('servers', [])
    assert len(servers) == 1, 'Expected one canonical server'
    origin = servers[0]['url']
    assert origin.startswith('https://') or origin.startswith(('http://127.0.0.1:', 'http://localhost:')), 'Unsafe API server'
    assert not origin.endswith('/v1') and not origin.endswith('/'), 'Paths already contain /v1; server must be an origin'
    if expected_origin:
        assert origin == expected_origin, 'Wrong deployment origin'
    slices = [json.loads((ROOT / name).read_text()) for name in ('docs/sdk/openapi.app.json', 'docs/sdk/openapi.systemone.json')]
    app = {'paths': {path: item for slice in slices for path, item in slice['paths'].items()}}
    # Thread execution remains distinct from read-only saved history. Keep
    # both list envelopes and destructive sync delivery visible to clients.
    for slice in slices:
        thread_paths = slice['paths']
        if '/v1/threads' not in thread_paths:
            continue
        listing = thread_paths['/v1/threads']['get']
        assert len(listing['responses']['200']['content']['application/json']['schema']['oneOf']) == 2
        take = thread_paths['/v1/threads/synced/{session}/replies']['post']
        assert 'at most once' in take['description']
        assert 'revision-pinned' in thread_paths['/v1/threads/{id}/messages']['get']['description']
        for path, item in thread_paths.items():
            if path.startswith('/v1/history'):
                assert set(item) == {'get'}, 'Saved history must remain read-only'
    ids = set()
    operations = 0
    paths = document['paths']
    for path, item in paths.items():
        assert path.startswith('/v1/'), 'Mixed path bases: ' + path
        for method, operation in item.items():
            if method not in METHODS:
                continue
            assert method in app['paths'].get(path, {}) or forwarded(path, method), 'Operation is not served by this public front: ' + method + ' ' + path
            ident = operation.get('operationId')
            assert ident and ident not in ids, 'Missing or duplicate operation ID: ' + str(ident)
            ids.add(ident)
            operations += 1
    for path, item in app['paths'].items():
        if not complete and path not in paths:
            continue
        for method, operation in item.items():
            if method in METHODS:
                assert paths.get(path, {}).get(method) == operation, 'Qualified app operation drift: ' + method + ' ' + path
    for slice in slices:
        if not complete and not any(path in paths for path in slice['paths']):
            continue
        for section, entries in slice['components'].items():
            for name, value in entries.items():
                assert document.get('components', {}).get(section, {}).get(name) == value, 'Qualified component drift: ' + section + '/' + name
    def refs(value):
        if isinstance(value, dict):
            for key, child in value.items():
                if key == '$ref':
                    assert child.startswith('#/'), 'Unexpected external schema reference'
                    target = document
                    for segment in child[2:].split('/'):
                        target = target[segment.replace('~1', '/').replace('~0', '~')]
                else:
                    refs(child)
        elif isinstance(value, list):
            for child in value:
                refs(child)
    refs(document)
    return operations


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--schema', type=Path, help='Validate a saved public document')
    parser.add_argument('--slice', action='store_true', help='For --schema only: validate a standalone qualified slice without requiring other domains')
    parser.add_argument('--api', help='Read only: fetch ORIGIN/v1/openapi.json without credentials')
    parser.add_argument('--write-inventory', action='store_true')
    args = parser.parse_args()
    if args.write_inventory:
        INVENTORY.write_text(json.dumps(source_inventory(), indent=2) + '\n')
    assert json.loads(INVENTORY.read_text()) == source_inventory(), 'Route inventory drift; run --write-inventory and review the change'
    if args.schema:
        print('PASS public reference:', check(json.loads(args.schema.read_text()), complete=not args.slice), 'operations')
    if args.api:
        origin = args.api.rstrip('/')
        assert origin.startswith('https://'), 'Use an HTTPS public origin'
        request = urllib.request.Request(origin + '/v1/openapi.json', headers={'Accept': 'application/json'})
        with urllib.request.urlopen(request, timeout=30) as response:
            raw = response.read(8 * 1024 * 1024 + 1)
            assert len(raw) <= 8 * 1024 * 1024
            document = json.loads(raw)
        print('PASS deployed public reference:', check(document, origin), 'operations')
    print('PASS source inventory and forwarding rules')


if __name__ == '__main__':
    main()
