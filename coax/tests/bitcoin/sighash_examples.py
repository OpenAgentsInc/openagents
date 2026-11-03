#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: recomputes the examples' output with the Python transliteration of Core, outside Coax)
"""Checks the signature-hash examples of Bitcoin chapters 37 and 38 against
script_reference.py, the Python transliteration of Bitcoin Core's
SignatureHash, which shares no code with Coax.

  python3 coax/tests/bitcoin/sighash_examples.py legacy bitcoin/fixtures/legacy-sighash.out

Every hash the example prints must be one the reference computes for the
same transaction, input, scriptCode and hash type, and every byte count
must be the length of the preimage the reference hashes."""
import json
from pathlib import Path
import re
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import script_reference as S  # noqa: E402
import tx_reference as T  # noqa: E402

UPSTREAM = HERE.parent / 'vectors/upstream/bitcoin-core'
BIP143_TX = re.search(r'BtVec143\.\{tx="([0-9a-f]+)"', (HERE / 'sighash_vectors.coax').read_text()).group(1)


def read(hex_tx):
    return T.unserialize(bytes.fromhex(hex_tx), True)


def shown(h):
    """A hash as the examples print it: Core's uint256 order, byte 31 first."""
    return h[::-1].hex()


def preimage_bytes(compute):
    """The length of what the first SHA-256 of a sighash reads."""
    seen = []
    real = T.sha256d
    def spy(b):
        seen.append(len(b))
        return real(b)
    T.sha256d = spy
    try:
        compute()
    finally:
        T.sha256d = real
    return seen


def p2pkh_tx(n):
    """n P2PKH inputs (a 107-byte scriptSig each) and two P2PKH outputs."""
    return {'version': b'\1\0\0\0',
            'ins': [{'prevout': bytes(36), 'script': bytes(107), 'sequence': b'\xff' * 4, 'witness': []} for _ in range(n)],
            'outs': [{'value': bytes(8), 'script': bytes(25)}] * 2, 'locktime': bytes(4)}


def legacy(out):
    agreed = 0
    tx = read(BIP143_TX)
    code = bytes.fromhex('76a9141d0f172a0ecb48aee1be1f2687d2963ae33f71a188ac')
    for t in (0x01, 0x02, 0x03, 0x81, 0x82, 0x83, 0x00, 0x04, 0x21, 0x84, 0xc3, 0xff):
        assert '  0x%02x ' % t in out, hex(t)
        assert shown(S.sighash_legacy(code, tx, 1, t)) in out, hex(t)
        agreed += 1
    for c in ('ac', 'abac', 'ababac', 'acabac'):
        assert shown(S.sighash_legacy(bytes.fromhex(c), tx, 1, 1)) in out, c
        agreed += 1
    rows = json.loads((UPSTREAM / 'sighash.json').read_text())[1:]
    single = [read(r[0]) for r in rows]
    single = [t for t in single if len(t['ins']) > len(t['outs'])][:2]
    for t in single:
        m = len(t['outs'])
        assert shown(S.sighash_legacy(code, t, m, 3)) == '00' * 31 + '01' and shown(S.sighash_legacy(code, t, m, 0x83)) == '00' * 31 + '01'
    assert shown(S.sighash_legacy(code, single[0], len(single[0]['outs']), 1)) in out
    assert '0x03: ' + '00' * 31 + '01' in out
    agreed += 2
    for n in (1, 10, 100, 1000, 5000, 6756):
        t = p2pkh_tx(n)
        size = len(T.serialize(t, False))
        per = preimage_bytes(lambda: S.sighash_legacy(bytes(25), t, 0, 1))[0]
        assert 'n %d: %d bytes, %d hashed per input, %d in all' % (n, size, per, n * per) in out, n
        agreed += 1
    assert 'defined hash types: 12 of 12 match' in out and 'undefined hash types: 488 of 488 match' in out
    return agreed


def main():
    mode, path = sys.argv[1], sys.argv[2]
    out = Path(path).read_text()
    agreed = {'legacy': legacy}[mode](out)
    print('%s: %d values agree with script_reference.py' % (path, agreed))


if __name__ == '__main__':
    main()
