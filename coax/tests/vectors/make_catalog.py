#!/usr/bin/env python3
"""Mechanical source index. It computes no cryptographic expected outputs."""
import argparse
import csv
import hashlib
import io
import json
from pathlib import Path
import re

HERE = Path(__file__).resolve().parent
INPUTS = [
    'upstream/bip340/test-vectors.csv',
    'upstream/nip44/nip44.vectors.json',
    'upstream/wycheproof/ecdh_secp256k1_test.json',
    'upstream/nist/SHA256ShortMsg.rsp',
    'upstream/nist/SHA256LongMsg.rsp',
    'upstream/nips/44.md',
]

def catalog(root):
    inputs = [{'path': p, 'sha256': hashlib.sha256((root/p).read_bytes()).hexdigest()} for p in INPUTS]
    rows = list(csv.DictReader(io.StringIO((root/INPUTS[0]).read_text())))
    nip = json.loads((root/INPUTS[1]).read_bytes())['v2']
    wyche = json.loads((root/INPUTS[2]).read_bytes())
    sha = []
    for path in INPUTS[3:5]:
        text = (root/path).read_text()
        lengths = [int(n) for n in re.findall(r'^Len = (\d+)$', text, re.M)]
        sha.append({'path': path, 'cases': len(lengths), 'message_lengths_bits': lengths})
    # Transcribe published table fields mechanically, without deriving expected
    # ciphertext/digests through an implementation under test.
    extended = []
    text = (root/INPUTS[5]).read_text()
    pattern = r'^\| (65535|65536|65537) \| (.*?) \| (\d+) \| `([a-f0-9]{64})` \| `([a-f0-9]{64})` \|$'
    for row in re.finditer(pattern, text, re.M):
        extended.append({'plaintext_len': int(row[1]), 'prefix': row[2], 'padded_len': int(row[3]), 'plaintext_sha256': row[4], 'payload_sha256': row[5]})
    if len(extended) != 3:
        raise ValueError('the pinned NIP-44 extended-prefix table must contain three rows')
    return {
        'schema': 'coax.crypto-vector-catalog.v1',
        'scope': 'Source indexing only; not a Coax algorithm result or an adapted ECDH oracle',
        'inputs': inputs,
        'bip340': [{'id': int(r['index']), 'message_bytes': len(bytes.fromhex(r['message'])), 'verification': r['verification result'] == 'TRUE', 'has_signing_input': bool(r['secret key'])} for r in rows],
        'nip44': {category: {name: len(value) for name, value in groups.items()} for category, groups in nip.items()},
        'nip44_extended': extended,
        'wycheproof': {
            'algorithm': wyche['algorithm'], 'source_cases': wyche['numberOfTests'],
            'groups': len(wyche['testGroups']),
            'adaptation': 'None. Source public keys are encoded; a reviewed raw-x/x-only adapter is required before NIP-44 use.',
        },
        'nist_sha256': sha,
    }

def encoded(root):
    return (json.dumps(catalog(root), indent=2) + '\n').encode()

def refresh(root):
    value = json.loads((root/'manifest.json').read_text())
    path = 'derived/catalog.json'; data = encoded(root)
    (root/path).parent.mkdir(parents=True, exist_ok=True)
    (root/path).write_bytes(data)
    entry = {
        'path': path, 'kind': 'derived', 'url': 'local:make_catalog.py',
        'revision': 'adapter:source-index-v1', 'retrieved': value['retrieved'],
        'license': 'Apache-2.0 (OpenAgents source index; upstream input licenses unchanged)',
        'sha256': hashlib.sha256(data).hexdigest(), 'bytes': len(data),
        'covers': ['source-index'], 'generator': 'make_catalog.py',
        'generator_sha256': hashlib.sha256((root/'make_catalog.py').read_bytes()).hexdigest(),
        'inputs': catalog(root)['inputs'],
    }
    value['files'] = [x for x in value['files'] if x['path'] != path] + [entry]
    (root/'manifest.json').write_text(json.dumps(value, indent=2) + '\n')

if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--write', action='store_true', help='explicitly regenerate catalog and its manifest entry')
    args = p.parse_args()
    if args.write:
        refresh(HERE)
    else:
        print(encoded(HERE).decode(), end='')
