#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: copies published headers and checks each one's hash and link outside Coax)
"""Writes coax/tests/bitcoin/blockindex_vectors.coax for Bitcoin chapter 59
from mainnet/headers-0-2016.txt, pinned in coax/tests/vectors/upstream/:

  python3 -I coax/tests/bitcoin/make_blockindex_vectors.py

The file holds mainnet's first 2,017 headers, genesis to block 2,016, one
per line as height, block hash and the 80 bytes in hex, from mempool.space.
Each header hashes to its block hash and names the line before it as its
previous block.
"""
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent / 'vectors' / 'upstream'
NAME = 'mainnet/headers-0-2016.txt'
OUT = HERE / 'blockindex_vectors.coax'
NOW = 1793664000   # 2026-11-03, the node's clock in the example


def sha256d(b):
    return hashlib.sha256(hashlib.sha256(b).digest()).digest()


def headers():
    manifest = json.loads((HERE.parent / 'vectors' / 'manifest.json').read_text())
    pin = {e['path']: e['sha256'] for e in manifest['files']}['upstream/' + NAME]
    data = (ROOT / NAME).read_bytes()
    assert hashlib.sha256(data).hexdigest() == pin, NAME
    out = []
    prev = bytes(32)
    for k, line in enumerate(data.decode().splitlines()):
        height, block_hash, hexed = line.split(' ')
        raw = bytes.fromhex(hexed)
        assert int(height) == k and len(raw) == 80 and sha256d(raw)[::-1].hex() == block_hash and raw[4:36] == prev, k
        prev = sha256d(raw)
        out.append(raw)
    assert len(out) == 2017
    return out


def main():
    hs = headers()
    text = ('// Mainnet\'s first 2,017 headers, genesis to block 2,016, for Bitcoin\n'
            '// chapter 59, as mempool.space serves them\n'
            '// (coax/tests/vectors/upstream/mainnet/headers-0-2016.txt, listed in\n'
            '// SOURCES.md). Written by make_blockindex_vectors.py.\n'
            f'BI_MAINNET_COUNT :: {len(hs)};\n'
            f'BI_NOW :: {NOW};   // 2026-11-03\n'
            f'BI_MAINNET_HEADERS :: "{"".join(h.hex() for h in hs)}";\n')
    OUT.write_text(text)
    print(f'wrote {OUT.name}: {len(hs)} headers')


if __name__ == '__main__':
    main()
