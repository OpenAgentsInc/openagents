#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: recomputes the examples' output with the Python transliteration of Core, outside Coax)
"""Checks the signature-hash examples of Bitcoin chapters 37 and 38 against
script_reference.py, the Python transliteration of Bitcoin Core's
SignatureHash, which shares no code with Coax.

  python3 coax/tests/bitcoin/sighash_examples.py legacy bitcoin/fixtures/legacy-sighash.out
  python3 coax/tests/bitcoin/sighash_examples.py v0 bitcoin/fixtures/segwit-sighash.out

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


def coax_constants(path):
    """NAME :: "hex"; and Example.{...} rows of a Coax example."""
    text = path.read_text()
    consts = dict(re.findall(r'^([A-Z_0-9]+) :: "([0-9a-f]+)";', text, re.M))
    rows = []
    for m in re.finditer(r'Example\.\{label="([^"]+)", tx=([A-Z_0-9]+), index=(\d+), amount=(\d+), hashtype=(0x[0-9a-f]+|\d+),\s*'
                         r'code="([0-9a-f]+)",\s*sighash="([0-9a-f]+)"\}', text):
        rows.append(dict(label=m.group(1), tx=consts[m.group(2)], index=int(m.group(3)), amount=int(m.group(4)),
                         hashtype=int(m.group(5), 0), code=m.group(6), sighash=m.group(7)))
    return consts, rows


def v0(out):
    """Chapter 38: every published BIP-143 hash and its three hashes, the
    amount, the signed spends and the byte counts."""
    agreed = 0
    example = HERE.parents[1] / '../bitcoin/examples/segwit-sighash.coax'
    consts, more = coax_constants(example.resolve())
    bip = (HERE.parent / 'vectors/upstream/bip143/bip-0143.mediawiki').read_text()
    vec = re.findall(r'BtVec143\.\{tx="([0-9a-f]+)",index=(\d+),code="([0-9a-f]+)",amount=(\d+),hashtype=(\d+),sighash="([0-9a-f]+)"',
                     (HERE / 'sighash_vectors.coax').read_text())
    cases = [dict(tx=t, index=int(i), code=c[2:], amount=int(a), hashtype=int(h), sighash=s) for t, i, c, a, h, s in vec]
    assert len(cases) == 8 and len(more) == 5
    for c in cases + more:
        tx = read(c['tx'])
        # The values copied into the example are BIP-143's own.
        assert c['sighash'] in bip and c['tx'] in bip and c['code'] in bip, c['sighash']
        seen = []
        real = T.sha256d
        def spy(b):
            seen.append(b)
            return real(b)
        T.sha256d = spy
        try:
            h = S.sighash_v0(bytes.fromhex(c['code']), tx, c['index'], c['hashtype'], c['amount'])
        finally:
            T.sha256d = real
        pre = seen[-1]
        assert h.hex() == c['sighash']
        block = ('    hashPrevouts %s\n    hashSequence %s\n    hashOutputs  %s\n    sigHash      %s\n'
                 % (pre[4:36].hex(), pre[36:68].hex(), pre[-40:-8].hex(), h.hex()))
        assert block in out, c['sighash']
        agreed += 4
    assert '13 of 13 match' in out
    # Every non-zero intermediate hash the example prints is one BIP-143 prints.
    for value in re.findall(r'(?:hashPrevouts|hashSequence|hashOutputs ) ([0-9a-f]{64})', out):
        assert value == '0' * 64 or value in bip, value
        agreed += 1
    tx = read(cases[0]['tx'])
    code = bytes.fromhex(cases[0]['code'])
    assert 'scriptCode   19' + code.hex() + '\n' in out
    for amount in (600000000, 600000001):
        assert 'BIP-143, %d sat: %s' % (amount, S.sighash_v0(code, tx, 1, 1, amount).hex()) in out
        assert 'legacy, %d sat: %s' % (amount, S.sighash_legacy(code, tx, 1, 1).hex()) in out
        agreed += 2
    flags = S.F['P2SH'] | S.F['WITNESS']
    swapped = read(consts['P2WSH_B_SWAPPED'])
    spks = re.search(r'P2WSH_B_SPK :: \[2\] string\.\["([0-9a-f]+)", "([0-9a-f]+)"\]', example.resolve().read_text()).groups()
    for k in range(2):
        i = swapped['ins'][k]
        error = S.verify_script(i['script'], bytes.fromhex(spks[1 - k]), i['witness'], flags, S.Checker(swapped, k, 16777215))
        assert '  pairs swapped, input %d: %s\n' % (k, S.ERRORS[error]) in out and error == 0
        agreed += 1
    signed = read(consts['NO_FAD_SIGNED'])
    i = signed['ins'][0]
    error = S.verify_script(i['script'], bytes.fromhex(consts['NO_FAD_SPK']), i['witness'], flags, S.Checker(signed, 0, 200000))
    assert '  without FindAndDelete: %s\n' % S.ERRORS[error] in out and error == 0
    assert S.verify_script(i['script'], bytes.fromhex(consts['NO_FAD_SPK']), i['witness'], flags, S.Checker(signed, 0, 200001)) == S.E['CHECKSIGVERIFY']
    agreed += 1
    for n in (1, 10, 100, 1000, 5000, 6756):
        t = p2pkh_tx(n)
        legacy_all = n * preimage_bytes(lambda: S.sighash_legacy(bytes(25), t, 0, 1))[0]
        for i in t['ins']:
            i['script'] = b''
        once = preimage_bytes(lambda: S.sighash_v0(bytes(25), t, 0, 1, 0))
        assert len(once) == 4 and once[3] == 182
        total = sum(once[:3]) + n * once[3]
        assert 'n %d: legacy %d, BIP-143 %d (%d once, 182 per input)' % (n, legacy_all, total, sum(once[:3])) in out, n
        agreed += 1
    return agreed


def main():
    mode, path = sys.argv[1], sys.argv[2]
    out = Path(path).read_text()
    agreed = {'legacy': legacy, 'v0': v0}[mode](out)
    print('%s: %d values agree with script_reference.py' % (path, agreed))


if __name__ == '__main__':
    main()
