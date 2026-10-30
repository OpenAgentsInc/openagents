#!/usr/bin/env python3
"""Read-only deployed NIP route/Markdown/immutable-asset checks.

python3 coax/web/tests/nip_pages_http.py https://openagents.com
The normal release walk separately checks actual WebGPU engine startup.
"""
from concurrent.futures import ThreadPoolExecutor
import hashlib
from html.parser import HTMLParser
import json
from pathlib import Path
import re
import sys
from urllib.parse import urljoin, urlparse
from urllib.request import urlopen

ROOT = Path(__file__).resolve().parents[3]
APP = ROOT / 'apps/openagents-web'


class Host(HTMLParser):
    def __init__(self):
        super().__init__()
        self.attributes = None

    def handle_starttag(self, tag, attributes):
        attrs = dict(attributes)
        if tag == 'script' and 'data-app' in attrs:
            assert self.attributes is None, 'multiple Coaxial hosts'
            self.attributes = attrs


def main():
    base = sys.argv[1].rstrip('/')
    parsed = urlparse(base)
    assert parsed.scheme in ('http', 'https') and parsed.netloc
    rows = json.loads((APP / 'nip/catalog.json').read_text())['pages']
    routes = [('/nip', 'nip-guide', 0)] + [
        (f"/nip/{r['number']:02}", 'nip01' if r['number'] == 1 else 'nip-guide', i + 1)
        for i, r in enumerate(rows)
    ]

    def get(path):
        url = urljoin(base + '/', path)
        assert urlparse(url).netloc == parsed.netloc
        with urlopen(url, timeout=20) as response:
            assert response.status == 200, path
            return response.read(), response.headers.get_content_type()

    def route_check(item):
        path, module, page = item
        engines = []
        for suffix in ('', '/index.html'):
            data, kind = get(path + suffix)
            assert kind == 'text/html', (path, kind)
            host = Host()
            host.feed(data.decode())
            attrs = host.attributes
            assert attrs and attrs['data-app'] == module, (path, attrs)
            assert int(attrs['data-page']) == page, (path, attrs)
            engines.append(tuple(attrs[name] for name in ('src', 'data-wasm', 'data-wgsl')))
        assert engines[0] == engines[1], path
        markdown, kind = get(path + '/index.md')
        assert markdown == (APP / (path.lstrip('/') + '/index.md')).read_bytes(), path
        assert kind in ('text/markdown', 'text/plain'), (path, kind)
        return path, engines[0]

    with ThreadPoolExecutor(max_workers=4) as pool:
        checked = list(pool.map(route_check, routes))
    assets = sorted({asset for _, files in checked for asset in files})
    identities = {}
    for path in assets:
        data, kind = get(path)
        match = re.search(r'\.([0-9a-f]{12})\.(wasm|wgsl|js)$', path)
        assert match, ('asset is not content-named', path)
        digest = hashlib.sha256(data).hexdigest()
        if match[2] == 'wasm':
            assert digest.startswith(match[1]), ('content identity', path)
            assert data[:8] == b'\0asm\x01\0\0\0' and kind == 'application/wasm', path
        # deploy.sh names text assets before strip.ts removes comments. The
        # URL fingerprints their pre-strip source; record the actual wire
        # bytes independently. Wasm is not stripped, so its hash must match.
        identities[path] = {'bytes': len(data), 'sha256': digest,
                            'url_source_fingerprint': match[1]}
    print(json.dumps({'base': base, 'routes': [p for p, _ in checked],
                      'html_checks': len(routes) * 2, 'markdown_checks': len(routes),
                      'assets': identities}, indent=2))


if __name__ == '__main__':
    main()
