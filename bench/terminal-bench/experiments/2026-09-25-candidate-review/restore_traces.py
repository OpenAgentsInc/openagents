#!/usr/bin/env python3
"""Verify and restore the content-addressed trace bundle into a new directory."""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import tarfile

p = argparse.ArgumentParser(description=__doc__)
p.add_argument('--manifest', type=Path, required=True)
p.add_argument('--archive', type=Path, required=True)
p.add_argument('--out', type=Path, required=True)
a = p.parse_args()
manifest = json.loads(a.manifest.read_text())
if a.out.exists():
    raise ValueError('Use a new output directory; existing jobs are never overwritten')
expected = {r['sha256']:r['bytes'] for r in manifest['files']}
for row in manifest['files']:
    name = PurePosixPath(row['path'])
    if name.is_absolute() or '..' in name.parts or str(name) != row['path']:
        raise ValueError('Invalid retained trace path')
blobs = {}
with tarfile.open(a.archive,'r:gz') as archive:
    for member in archive:
        name = PurePosixPath(member.name)
        if not member.isfile() or len(name.parts)!=2 or name.parts[0]!='blobs':
            raise ValueError('Invalid blob archive member')
        sha = name.parts[1]
        if sha in blobs or sha not in expected or member.size != expected[sha]:
            raise ValueError('Unexpected or conflicting blob')
        data = archive.extractfile(member).read()
        if hashlib.sha256(data).hexdigest() != sha:
            raise ValueError('Blob digest mismatch')
        blobs[sha] = data
if set(blobs) != set(expected):
    raise ValueError('Missing retained blobs')
a.out.mkdir(parents=True)
for row in manifest['files']:
    path = a.out / row['path']
    path.parent.mkdir(parents=True,exist_ok=True)
    path.write_bytes(blobs[row['sha256']])
print('Verified and restored',len(manifest['files']),'files from',len(blobs),'blobs')
