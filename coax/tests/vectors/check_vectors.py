#!/usr/bin/env python3
"""Verify committed crypto source bytes/provenance, offline unless --fetch is explicit."""
import argparse
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import re
import sys
import urllib.request
import zipfile

HERE = Path(__file__).resolve().parent

class IntegrityError(ValueError):
    pass

def require(condition, message):
    if not condition:
        raise IntegrityError(message)

def local(root, path):
    require(isinstance(path, str) and bool(path), 'invalid path')
    parts = PurePosixPath(path)
    require(not parts.is_absolute() and '..' not in parts.parts and str(parts) == path, 'invalid path: '+path)
    current = root
    for part in parts.parts:
        current = current/part
        require(not current.is_symlink(), 'symlink: '+path)
    require(current.is_file(), 'missing file: '+path)
    return current

def digest(data):
    return hashlib.sha256(data).hexdigest()

def audit(root=HERE, fetch=False):
    root = Path(root)
    try:
        manifest = json.loads((root/'manifest.json').read_text())
    except (OSError, ValueError) as e:
        raise IntegrityError('manifest: '+str(e)) from e
    require(manifest.get('schema') == 'coax.crypto-vectors.v1', 'manifest schema')
    files = manifest.get('files')
    require(isinstance(files, list) and files, 'manifest files')
    indexed = {}
    for entry in files:
        require(isinstance(entry, dict), 'invalid entry')
        for field in ('path', 'kind', 'url', 'revision', 'retrieved', 'license', 'sha256'):
            require(isinstance(entry.get(field), str) and bool(entry[field].strip()), 'missing '+field)
        path = entry['path']
        require(path not in indexed, 'duplicate path: '+path)
        require(path.startswith(('upstream/', 'derived/')), 'invalid corpus path: '+path)
        require(entry['kind'] in ('upstream', 'derived'), 'invalid kind: '+path)
        require(bool(re.fullmatch(r'\d{4}-\d{2}-\d{2}', entry['retrieved'])), 'invalid retrieved: '+path)
        require(bool(re.fullmatch(r'[a-f0-9]{64}', entry['sha256'])), 'invalid digest: '+path)
        data = local(root, path).read_bytes()
        require(digest(data) == entry['sha256'], 'digest mismatch: '+path)
        require(type(entry.get('bytes')) is int and len(data) == entry['bytes'], 'byte count mismatch: '+path)
        require(isinstance(entry.get('covers'), list) and bool(entry['covers']) and all(isinstance(x, str) and x for x in entry['covers']), 'covers: '+path)
        if entry['kind'] == 'upstream':
            require(entry['url'].startswith('https://'), 'source URL: '+path)
            if entry['revision'].startswith('git:'):
                sha = entry['revision'][4:]
                require(bool(re.fullmatch(r'[a-f0-9]{40}', sha)) and '/'+sha+'/' in entry['url'], 'unpinned revision: '+path)
            elif entry['revision'].startswith('RFC '):
                n = entry['revision'][4:]
                require(n.isdigit() and entry['url'] == f'https://www.rfc-editor.org/rfc/rfc{n}.txt', 'RFC revision: '+path)
            else:
                require('archive_member' in entry and 'archive_sha256' in entry, 'unpinned revision: '+path)
                require(bool(re.fullmatch(r'[a-f0-9]{64}', entry['archive_sha256'])), 'archive digest: '+path)
        indexed[path] = entry
    for parent in ('upstream', 'derived'):
        for path in (root/parent).rglob('*'):
            if path.is_file() or path.is_symlink():
                require(path.relative_to(root).as_posix() in indexed, 'unlisted corpus file: '+str(path.relative_to(root)))
    for path, entry in indexed.items():
        if entry['kind'] != 'derived':
            continue
        inputs = entry.get('inputs')
        require(path == 'derived/catalog.json' and entry.get('generator') == 'make_catalog.py', 'unsupported derived adapter: '+path)
        require(isinstance(inputs, list) and bool(inputs), 'derived input: '+path)
        for item in inputs:
            require(isinstance(item, dict) and item.get('path') in indexed, 'derived input path: '+path)
            source = indexed[item['path']]
            require(source['kind'] == 'upstream' and item.get('sha256') == source['sha256'], 'derived input digest: '+path)
        generator = local(root, entry.get('generator'))
        require(digest(generator.read_bytes()) == entry.get('generator_sha256'), 'generator digest: '+path)
    # The sole adapter indexes source rows; it never runs untrusted source code.
    from make_catalog import encoded
    require((root/'derived/catalog.json').read_bytes() == encoded(root), 'derived catalog differs from recorded source inputs')
    if fetch:
        cache = {}
        for path, entry in indexed.items():
            if entry['kind'] != 'upstream':
                continue
            url = entry['url']
            if url not in cache:
                request = urllib.request.Request(url, headers={'User-Agent': 'Mozilla/5.0 OpenAgents-vector-audit'})
                with urllib.request.urlopen(request, timeout=45) as response:
                    cache[url] = response.read(16 * 1024 * 1024 + 1)
                require(len(cache[url]) <= 16 * 1024 * 1024, 'source download budget: '+path)
            data = cache[url]
            if 'archive_member' in entry:
                require(digest(data) == entry['archive_sha256'], 'upstream archive digest: '+path)
                with zipfile.ZipFile(io.BytesIO(data)) as archive:
                    data = archive.read(entry['archive_member'])
            require(digest(data) == entry['sha256'], 'upstream digest: '+path)
    return {'upstream_files': sum(x['kind'] == 'upstream' for x in files), 'derived_files': sum(x['kind'] == 'derived' for x in files), 'bytes': sum(x['bytes'] for x in files), 'network': fetch}

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--fetch', action='store_true', help='also re-read pinned HTTPS sources; never rewrites corpus')
    args = parser.parse_args()
    try:
        print(json.dumps(audit(fetch=args.fetch), sort_keys=True))
    except (IntegrityError, OSError, ValueError) as e:
        print('vector integrity: '+str(e), file=sys.stderr)
        raise SystemExit(1)
