#!/usr/bin/env python3
"""Pack retained experiment files reproducibly, or verify an extracted bundle."""
import argparse
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
a = p.parse_args()


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


if a.mode == 'pack':
    if not a.archive:
        raise ValueError('Packing needs --archive')
    entries = []
    for path in sorted(a.root.rglob('*')):
        if path.is_symlink():
            raise ValueError('Archive refuses symlinks: '+str(path))
        if not path.is_file():
            continue
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
        archive_sha256=digest(a.archive),files=entries),indent=2)+'\n')
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
