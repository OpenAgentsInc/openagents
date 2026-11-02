#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: published keys and a pure-Python SEC 1 / BIP-340 codec check the Coax codec)
"""Point encodings against published keys and a Python reference (Bitcoin
chapter 17).

Reads public keys from files pinned in coax/tests/vectors/upstream/, each
checked against its SHA-256 first:

- Bitcoin Core's script_tests.json: G and a second key in both forms, and
  the hybrid and wrong-length keys that its STRICTENC cases refuse;
- the SEC 1 points inside Wycheproof's ecdh_secp256k1_test.json public keys
  that carry the named-curve secp256k1 header: 455 distinct valid uncompressed points,
  one compressed point, and the off-curve, invalid-compressed, twist and
  empty points;
- every extended public key of BIP-32 vectors 1-4, and vector 5's three keys
  with an invalid point;
- BIP-340's test-vectors.csv public keys (rows 5 and 14 are invalid);
- BIP-341's wallet-test-vectors.json internal and output keys.

With --write it writes the subset that coax/tests/bitcoin/point_vectors.coax
holds for bitcoin/examples/checks/point-encodings.coax; without it, it checks
that file is current. Then it builds coax/lib/crypto/secp_encode.coax as a C
library in checked and release modes and checks, in both:

- every published key decodes or refuses as stated, and each decoded key
  re-encodes in all three forms exactly as the Python reference does;
- 300 random points round-trip between all three forms;
- 3,000 random and corrupted encodings (random x, flipped y, every prefix
  byte, x or y at least p, wrong lengths) decode or refuse exactly as the
  Python reference says.

The reference below is test code only. Run from the repository root:

  python3 coax/tests/bitcoin/points.py
  python3 coax/tests/bitcoin/points.py --write
"""
if not __debug__: raise SystemExit('tests require Python assertions')
import csv
import ctypes as C
import hashlib
import json
import os
from pathlib import Path
import random
import re
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[3]
COMPILER = Path(os.environ.get('COAX_TEST_COMPILER', ROOT/'coax/bin/coax'))
SOURCE = ROOT/'coax/lib/crypto/secp_encode.coax'
TABLE = ROOT/'coax/tests/bitcoin/point_vectors.coax'
VECTORS = ROOT/'coax/tests/vectors/upstream'
PINS = {
    'bitcoin-core/script_tests.json': 'ccfadc13f9b06888faf57a4a2f86dd8feff35c80aa710cbf3eb421f326b8d8b7',
    'wycheproof/ecdh_secp256k1_test.json': '049fa2be70e9db836a043aba6132b53cd97de4c0de5277bca6d65a2770075cae',
    'bip32/bip-0032.mediawiki': 'e5e00a8289db2f681052cf24a745320afc225e66b25d1e489a7c884d2fc7f11f',
    'bip340/test-vectors.csv': '34c9d1d9c3a88d524bc80778540dc43f8306ec249a7485293063c376db851c2d',
    'bip341/wallet-test-vectors.json': '403e19fb81dd1f31e745699216308f61fb403774b2aafa87b631b8f7c042d37f',
}

# ---- the Python reference: affine arithmetic, SEC 1 and BIP-340 x-only.
P = 2**256 - 2**32 - 977
N = 0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141
G = (0x79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798,
     0x483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8)


def add(a, b):
    if a is None: return b
    if b is None: return a
    if a[0] == b[0] and (a[1] + b[1]) % P == 0: return None
    if a == b: m = 3 * a[0] * a[0] * pow(2 * a[1], -1, P)
    else: m = (b[1] - a[1]) * pow(b[0] - a[0], -1, P)
    x = (m * m - a[0] - b[0]) % P
    return (x, (m * (a[0] - x) - a[1]) % P)


def mul(k, a=G):
    r = None
    while k:
        if k & 1: r = add(r, a)
        a = add(a, a); k >>= 1
    return r


def lift(x):
    if x >= P: return None
    y = pow((x ** 3 + 7) % P, (P + 1) // 4, P)
    return (x, y if y % 2 == 0 else P - y) if y * y % P == (x ** 3 + 7) % P else None


def decode(b):
    """SEC 1 compressed or uncompressed; None for everything else."""
    if len(b) == 33 and b[0] in (2, 3):
        pt = lift(int.from_bytes(b[1:], 'big'))
        return pt and (pt[0], pt[1] if b[0] == 2 else P - pt[1])
    if len(b) == 65 and b[0] == 4:
        x, y = int.from_bytes(b[1:33], 'big'), int.from_bytes(b[33:], 'big')
        return (x, y) if x < P and y < P and (y * y - x ** 3 - 7) % P == 0 else None
    return None


def decode_any(b):
    return lift(int.from_bytes(b, 'big')) if len(b) == 32 else decode(b)


def encode(pt, form):
    x = pt[0].to_bytes(32, 'big')
    if form == 32: return x
    if form == 33: return bytes([2 + pt[1] % 2]) + x
    return b'\x04' + x + pt[1].to_bytes(32, 'big')


# ---- published keys
def source(name):
    data = (VECTORS/name).read_bytes()
    assert hashlib.sha256(data).hexdigest() == PINS[name], name
    return data.decode()


B58 = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'


def base58check(text):
    n = 0
    for ch in text: n = n * 58 + B58.index(ch)
    raw = n.to_bytes(82, 'big')
    assert hashlib.sha256(hashlib.sha256(raw[:-4]).digest()).digest()[:4] == raw[-4:], text
    return raw[:-4]


def published():
    """(source, label, hex, expected) rows, expected True to decode, and
    (compressed, uncompressed) pairs of one key."""
    rows, pairs = [], []
    # Bitcoin Core: the pushed keys, and the comments that say which refuse.
    tests = json.loads(source('bitcoin-core/script_tests.json'))
    flat = [r for r in tests if len(r) >= 5 and all(isinstance(x, str) for x in r[:5])]
    def pushed(comment, size):
        row = next(r for r in flat if r[4] == comment)
        assert 'STRICTENC' in row[2] and row[3] == 'PUBKEYTYPE', row
        return re.search(rf'0x{size:02x} 0x([0-9a-f]+)', row[1].lower()).group(1)
    for comment, size in [('P2PK with hybrid pubkey', 0x41),
                          ('P2PK with invalid length for uncompressed key', 0x21),
                          ('P2PK with invalid length for compressed key', 0x41),
                          ('33-byte pubkey with a prefix other than 0x02/0x03 is incorrectly encoded for STRICTENC', 0x21)]:
        rows.append(('script_tests', comment, pushed(comment, size), False))
    # Keys a signature verifies against in an OK case, and G's two forms.
    text = json.dumps(tests).lower()
    for key, label in [('0479be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8', 'G, uncompressed (P2PK)'),
                       ('0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798', 'G, compressed'),
                       ('048282263212c609d9ea2a6e3e172de238d8c39cabd5ac1ca10646e23fd5f5150811f8a8098557dfe45e8256e830b60ace62d613ac2f7b17bed31b6eaff6e26caf', 'P2PK anyonecanpay key, uncompressed'),
                       ('038282263212c609d9ea2a6e3e172de238d8c39cabd5ac1ca10646e23fd5f51508', 'P2PKH key, compressed'),
                       ('03363d90d447b00c9c99ceac05b6262ee053441c7e55552ffe526bad8f83ff4640', 'P2PK with multi-byte hashtype key')]:
        assert ('0x%02x 0x%s' % (len(key) // 2, key)) in text, label
        rows.append(('script_tests', label, key, True))
    pairs += [(rows[5][2], rows[4][2]), (rows[7][2], rows[6][2])]
    # Wycheproof: the SEC 1 point after the named-curve secp256k1 header.
    wy = json.loads(source('wycheproof/ecdh_secp256k1_test.json'))
    heads = {'3056301006072a8648ce3d020106052b8104000a034200': 65, '3036301006072a8648ce3d020106052b8104000a032200': 33,
             '3015301006072a8648ce3d020106052b8104000a030100': 0}
    seen = set()
    for t in wy['testGroups'][0]['tests']:
        head = next((h for h in heads if t['public'].startswith(h) and len(t['public']) == len(h) + 2 * heads[h]), None)
        if head is None or 'InvalidAsn' in t['flags'] or 'WrongCurve' in t['flags'] and heads[head] == 65:
            continue  # the ASN.1 or the curve parameters are what is wrong, not the point
        point = t['public'][len(head):]
        if point in seen: continue
        seen.add(point)
        ok = t['result'] != 'invalid'
        if not ok:
            assert set(t['flags']) & {'InvalidCurveAttack', 'InvalidPublic', 'InvalidCompressedPublic', 'WrongCurve', 'InvalidEncoding'}, t
        rows.append(('wycheproof', f"tcId {t['tcId']}: {t['comment'] or ', '.join(t['flags'])}", point, ok))
    # BIP-32: every extended public key of vectors 1-4, and vector 5's
    # invalid points.
    text = source('bip32/bip-0032.mediawiki')
    vectors = re.split(r'===Test vector \d+===', text)[1:]
    assert len(vectors) == 5
    for i, vector in enumerate(vectors[:4]):
        for path, xpub in re.findall(r'\* Chain (\S+)\n\*\* ext pub: (\w+)', vector):
            rows.append(('bip32', f'vector {i + 1} {path}', base58check(xpub)[45:78].hex(), True))
    for xpub, reason in re.findall(r'\* (xpub\w+) \((invalid pubkey[^)]*)\)', vectors[4]):
        rows.append(('bip32', f'vector 5 ({reason})', base58check(xpub)[45:78].hex(), False))
    assert sum(r[0] == 'bip32' and not r[3] for r in rows) == 3
    # BIP-340: x-only keys; rows 5 and 14 are invalid keys.
    for row in csv.DictReader(source('bip340/test-vectors.csv').splitlines()):
        bad = row['index'] in ('5', '14')
        assert bad == ('public key' in row['comment'] and ('not on the curve' in row['comment'] or 'not a valid X' in row['comment'])), row
        rows.append(('bip340', f"row {row['index']}", row['public key'].lower(), not bad))
    # BIP-341: internal keys and tweaked output keys.
    wallet = json.loads(source('bip341/wallet-test-vectors.json'))
    for i, item in enumerate(wallet['scriptPubKey']):
        rows.append(('bip341', f'scriptPubKey {i} internal key', item['given']['internalPubkey'], True))
        rows.append(('bip341', f'scriptPubKey {i} output key', item['intermediary']['tweakedPubkey'], True))
    for row in rows:
        assert (decode_any(bytes.fromhex(row[2])) is not None) == row[3], row
    return rows, pairs


def table(rows, pairs):
    """The Coax table: all but Wycheproof's valid points, which run here."""
    keep = [r for r in rows if r[0] != 'wycheproof' or not r[3] or 'normal case' in r[1] or 'compressed public key' in r[1]]
    out = ['// Generated by coax/tests/bitcoin/points.py --write from the files pinned in',
           '// coax/tests/vectors/upstream/ (see SOURCES.md there). Published keys,',
           '// copied, and whether each one decodes. Test data only.',
           'PvKey :: struct {source:string;label:string;hex:string;ok:bool;}',
           f'PV_KEYS :: [{len(keep)}] PvKey.[']
    out += [f'  PvKey.{{source="{s}",label="{l}",hex="{h}",ok={str(ok).lower()}}},' for s, l, h, ok in keep]
    out[-1] = out[-1].rstrip(',')
    out += ['];', '// One key in compressed and uncompressed form.',
            'PvPair :: struct {compressed:string;uncompressed:string;}', f'PV_PAIRS :: [{len(pairs)}] PvPair.[']
    out += [f'  PvPair.{{compressed="{c}",uncompressed="{u}"}},' for c, u in pairs]
    out[-1] = out[-1].rstrip(',')
    out.append('];')
    return '\n'.join(out) + '\n'


# ---- the Coax codec as a C library
class View(C.Structure):
    _fields_ = [('data', C.POINTER(C.c_uint8)), ('count', C.c_int64)]


def run(*args):
    r = subprocess.run([str(a) for a in args], capture_output=True, text=True)
    assert r.returncode == 0, f'{args[0]} failed: {r.stderr or r.stdout}'
    return r.stdout


def library(directory, mode):
    c = directory/f'{mode}.c'
    c.write_text(run(COMPILER, 'c', SOURCE, '--library', *(['--release'] if mode == 'release' else [])))
    shared = directory/f'{mode}.so'
    run('cc', '-std=c11', '-O2', '-shared', '-fPIC', c, '-lm', '-o', shared)
    lib = C.CDLL(str(shared))
    lib.se_recode.argtypes = [View, C.c_int64, View]; lib.se_recode.restype = C.c_int64
    def recode(data, form):
        src = (C.c_uint8 * max(len(data), 1)).from_buffer_copy(data or b'\0')
        out = (C.c_uint8 * 65)()
        n = lib.se_recode(View(C.cast(src, C.POINTER(C.c_uint8)), len(data)), form, View(C.cast(out, C.POINTER(C.c_uint8)), 65))
        assert n in (0, form), n
        return bytes(out[:n]) if n else None
    return recode


def agree(recode, data, label):
    """The codec decodes data exactly when the reference does, to the same point."""
    pt = decode_any(data)
    for form in (32, 33, 65):
        assert recode(data, form) == (encode(pt, form) if pt else None), (label, data.hex(), form)
    return pt is not None


def corrupted(rng):
    """Encodings near the valid ones, and random ones."""
    pt = mul(rng.randrange(1, N)); u = encode(pt, 65); c = encode(pt, 33)
    pbytes = P.to_bytes(32, 'big')
    yield u; yield c; yield c[1:]
    yield bytes([rng.randrange(256)]) + u[1:]                  # every prefix byte
    yield bytes([rng.randrange(256)]) + c[1:]
    yield bytes([6 + pt[1] % 2]) + u[1:]                       # hybrid
    yield u[:64] + bytes([u[64] ^ 1])                          # y off by one bit
    yield u[:33] + (P - pt[1]).to_bytes(32, 'big')             # the other y
    yield b'\x02' + rng.randbytes(32); yield rng.randbytes(32)  # random x: about half lift
    yield b'\x04' + pbytes + u[33:]; yield u[:33] + pbytes     # x = p, y = p
    yield b'\x02' + (P + rng.randrange(2**256 - P)).to_bytes(32, 'big')
    yield u[:rng.randrange(65)]; yield c + b'\x00'


def main():
    rows, pairs = published()
    text = table(rows, pairs)
    if '--write' in sys.argv[1:]:
        TABLE.write_text(text); print(f'wrote {TABLE.relative_to(ROOT)}')
    assert TABLE.read_text() == text, f'{TABLE.relative_to(ROOT)} is stale: run with --write'
    rng = random.Random(17)
    with tempfile.TemporaryDirectory(prefix='coax-points-') as temp:
        for mode in ('checked', 'release'):
            recode = library(Path(temp), mode)
            for s, label, h, ok in rows:
                assert agree(recode, bytes.fromhex(h), f'{s} {label}') == ok, (s, label)
            for c, u in pairs:
                assert recode(bytes.fromhex(c), 65).hex() == u and recode(bytes.fromhex(u), 33).hex() == c
            for _ in range(300):
                pt = mul(rng.randrange(1, N))
                for a in (32, 33, 65):
                    for b in (32, 33, 65):
                        want = encode(pt if a != 32 else lift(pt[0]), b)
                        assert recode(encode(pt, a), b) == want, (mode, pt, a, b)
            cases = accepted = 0
            for _ in range(200):
                for data in corrupted(rng):
                    accepted += agree(recode, data, mode); cases += 1
            counts = {s: sum(r[0] == s for r in rows) for s in ('script_tests', 'wycheproof', 'bip32', 'bip340', 'bip341')}
            print(f'{mode}: {len(rows)} published keys {counts} '
                  f'({sum(not r[3] for r in rows)} refuse), {len(pairs)} pairs, 300 random points x 9 form pairs, '
                  f'{cases} corrupted encodings ({accepted} decode)')


if __name__ == '__main__':
    main()
