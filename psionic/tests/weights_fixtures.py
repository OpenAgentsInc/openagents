#!/usr/bin/env python3
"""Owned deterministic Safetensors fixtures; --write creates, default verifies.

Uses only JSON/struct/hashlib. Never imports the reader or downloads weights.
--header emits native test inputs and --coax-header emits small native-free
example/check constants from the exact same committed original byte artifacts.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct

ROOT = Path(__file__).resolve().parents[2]
DEST = ROOT / 'psionic/fixtures/models/ch04'
CONFIG = b'{"model_type":"psionic-owned-linear","rows":2,"width":3}\n'
TOKENIZER = b'{"version":"owned-byte-identity-v1","tokens":["x"]}\n'


def digest(data):
    return hashlib.sha256(data).hexdigest()


def blob(entries):
    header = {'__metadata__': {'format': 'pt'}}
    payload = bytearray()
    for name, dtype, shape, values in entries:
        start = len(payload)
        for value in values:
            packed = struct.pack('<f', value)
            payload.extend(packed[2:] if dtype == 'BF16' else packed)
        header[name] = dict(dtype=dtype, shape=shape, data_offsets=[start, len(payload)])
    raw = json.dumps(header, separators=(',', ':'), sort_keys=True).encode()
    raw += b' ' * (-len(raw) % 8)
    return struct.pack('<Q', len(raw)) + raw + payload


def artifacts():
    weight = ('linear.weight', 'BF16', [2, 3], [1, 2, 3, 4, 5, 6])
    scale = ('scale', 'BF16', [], [2])
    bias = ('linear.bias', 'F32', [2], [0.5, -0.5])
    single = blob([weight, scale, bias])  # F32 bias starts at unaligned offset 14.
    first, second = blob([weight]), blob([bias, scale])
    index = {'metadata': {'total_size': 22}, 'weight_map': {
        'scale': 'model-00002-of-00002.safetensors',
        'linear.weight': 'model-00001-of-00002.safetensors',
        'linear.bias': 'model-00002-of-00002.safetensors'}}
    data = {'config.json': CONFIG, 'tokenizer.json': TOKENIZER,
            'model.safetensors': single, 'model-00001-of-00002.safetensors': first,
            'model-00002-of-00002.safetensors': second,
            'model.safetensors.index.json': (json.dumps(index, indent=2) + '\n').encode()}
    manifest = {'schema': 1, 'profile': 'original-weights-v1',
                'origin': 'openagents-owned-ch04-fixture', 'revision': 'owned-v1',
                'provenance': 'Deterministic numerical fixtures authored in this repository; no external weights.',
                'config_sha256': digest(CONFIG), 'tokenizer_sha256': digest(TOKENIZER),
                'artifacts': {k: {'bytes': len(v), 'sha256': digest(v)} for k, v in data.items()},
                'tensors': [{'name': n, 'dtype': d, 'shape': s, 'values': v} for n, d, s, v in [weight, bias, scale]]}
    data['manifest.json'] = (json.dumps(manifest, indent=2) + '\n').encode()
    return data


def symbol(name):
    return 'CH04_' + ''.join(c.upper() if c.isalnum() else '_' for c in name)


def coax_header(expected):
    lines = ['// Owned fixture bytes, verified against the committed artifacts. Tooling only.']
    for name, data in expected.items():
        if name == 'manifest.json':
            continue
        lines.append(symbol(name) + ' :: [' + str(len(data)) + '] u8.[' + ','.join(map(str, data)) + '];')
        lines.append(symbol(name) + '_SHA :: ' + json.dumps(digest(data)) + ';')
    return '\n'.join(lines) + '\n'


def main():
    p = argparse.ArgumentParser(); p.add_argument('--write', action='store_true'); p.add_argument('--header', action='store_true'); p.add_argument('--coax-header', action='store_true')
    args = p.parse_args(); expected = artifacts()
    if args.write:
        DEST.mkdir(parents=True, exist_ok=True)
        for name, data in expected.items():
            (DEST / name).write_bytes(data)
        (DEST / 'fixtures.coax').write_text(coax_header(expected))
    for name, data in expected.items():
        assert (DEST / name).read_bytes() == data, 'fixture drift: ' + name
    assert (DEST / 'fixtures.coax').read_text() == coax_header(expected), 'embedded fixture drift'
    if args.header:
        print('/* Owned original byte fixtures; verified by weights_fixtures.py. */')
        for name, data in expected.items():
            print('static const unsigned char ' + symbol(name) + '[] = {' + ','.join(map(str, data)) + '};')
        m = json.loads(expected['manifest.json'])
        for name, entry in m['artifacts'].items():
            print('static const char ' + symbol(name) + '_SHA[] = ' + json.dumps(entry['sha256']) + ';')
        print('#define CH04_CHUNK_SHA ' + json.dumps(digest(bytes(i % 251 for i in range(1048576 + 17)))))
        print('#define CH04_CONFIG_SHA CH04_CONFIG_JSON_SHA')
        print('#define CH04_TOKENIZER_SHA CH04_TOKENIZER_JSON_SHA')
    elif args.coax_header:
        print(coax_header(expected), end='')
    else:
        print('owned Safetensors single/sharded fixtures and manifest match their independent generator')


if __name__ == '__main__':
    main()
