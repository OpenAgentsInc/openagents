#!/usr/bin/env python3
"""Pack retained experiment files reproducibly, or verify an extracted bundle."""
import argparse
import fnmatch
import gzip
import hashlib
import json
from pathlib import Path
import tarfile

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('mode', choices=['pack','verify'])
p.add_argument('--root', type=Path, required=True)
p.add_argument('--manifest', type=Path, required=True)
p.add_argument('--archive', type=Path)
p.add_argument('--exclude', action='append', default=[])
p.add_argument('--scan-credentials', action='store_true')
a = p.parse_args()

secrets = set()
if a.scan_credentials:
    def gather(value):
        if isinstance(value, str) and len(value) > 24:
            secrets.add(value.encode())
        elif isinstance(value, dict):
            for child in value.values():
                gather(child)
        elif isinstance(value, list):
            for child in value:
                gather(child)
    for name in ['.codex/auth.json', '.openagents/jev.json']:
        path = Path.home() / name
        if path.exists():
            gather(json.loads(path.read_text()))
    bearer = Path.home() / '.openagents/bearer'
    if bearer.exists():
        secrets.add(bearer.read_bytes().strip())
    if not secrets:
        raise ValueError('No credential values available for the requested scan')


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


if a.mode == 'pack':
    if not a.archive:
        raise ValueError('Packing needs --archive')
    entries = []
    for path in sorted(a.root.rglob('*')):
        if any(fnmatch.fnmatch(str(path.relative_to(a.root)), pattern) for pattern in a.exclude):
            continue
        if path.is_symlink():
            raise ValueError('Archive refuses symlinks: '+str(path))
        if not path.is_file():
            continue
        if any(s and s in path.read_bytes() for s in secrets):
            raise ValueError('Credential content found; refusing publication')
        entries.append(dict(path=str(path.relative_to(a.root)),bytes=path.stat().st_size,sha256=digest(path)))
    with a.archive.open('wb') as destination:
        with gzip.GzipFile(filename='',mode='wb',fileobj=destination,mtime=0) as compressed:
            with tarfile.open(fileobj=compressed,mode='w|') as archive:
                for entry in entries:
                    path = a.root / entry['path']
                    if digest(path) != entry['sha256']:
                        raise ValueError('File changed while archiving')
                    info = tarfile.TarInfo(entry['path'])
                    info.size = entry['bytes']
                    info.mode = 0o644
                    with path.open('rb') as data:
                        archive.addfile(info,data)
    a.manifest.write_text(json.dumps(dict(schema='openagents.retained-files.v1',archive=a.archive.name,
        archive_sha256=digest(a.archive), exclusions=a.exclude,
        credential_scan='exact local values, zero matches' if a.scan_credentials else 'not requested',
        files=entries),indent=2)+'\n')
    print(len(entries),'files; archive SHA-256',digest(a.archive))
else:
    manifest = json.loads(a.manifest.read_text())
    if a.archive and digest(a.archive) != manifest['archive_sha256']:
        raise ValueError('Archive digest mismatch')
    for entry in manifest['files']:
        path = a.root / entry['path']
        if not path.resolve().is_relative_to(a.root.resolve()) or path.is_symlink():
            raise ValueError('Invalid retained path')
        if path.stat().st_size != entry['bytes'] or digest(path) != entry['sha256']:
            raise ValueError('Retained file mismatch: '+entry['path'])
    print('Verified',len(manifest['files']),'retained files')
