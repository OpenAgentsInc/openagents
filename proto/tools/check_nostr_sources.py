#!/usr/bin/env python3
"""Fail a Nostr mirror build if its schema/source inventory or pins drift."""
import hashlib
import json
from pathlib import Path
import sys


def check(root):
    proto=root/'proto';manifest=json.loads((proto/'nostr/MANIFEST.json').read_text())
    upstream=json.loads((root/'nips/manifest.json').read_text())
    commits={row['name']:row['commit'] for row in upstream['sources']}
    schemas={p.relative_to(proto).as_posix() for p in (proto/'nostr').rglob('*.proto')}
    packages=manifest['packages']
    if manifest['schema']!=1 or {p['proto'] for p in packages}!=schemas or len(packages)!=len(schemas):
        raise ValueError('Nostr mirror inventory drift')
    names=set()
    for row in packages:
        expected='.'.join(Path(row['proto']).parts[:-1])
        if row['package']!=expected or expected in names or row['status']!='alpha' or not row['sources']:
            raise ValueError('Invalid mirror registration: '+expected)
        names.add(expected)
        for source in row['sources']:
            path=Path(source['path'])
            if len(path.parts)<3 or path.parts[0]!='nips' or path.parts[1] not in commits or '..' in path.parts:
                raise ValueError('Invalid NIP source path')
            if source['commit']!=commits[path.parts[1]] or hashlib.sha256((root/path).read_bytes()).hexdigest()!=source['sha256']:
                raise ValueError('Stale Nostr mirror: '+expected+'; review source '+str(path))
    print(f'Nostr source pins/hashes: {len(packages)} alpha packages checked')


if __name__=='__main__':
    try:check(Path(__file__).resolve().parents[2])
    except (ValueError,OSError,KeyError) as error:sys.exit(str(error))
