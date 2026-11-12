#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: libsodium checks what ed25519.coax decides)
"""Differential test of coax/lib/crypto/ed25519.coax against libsodium's
crypto_sign_verify_detached.

libsodium 1.0.20 (the release tarball, pinned by its SHA-256) is downloaded
and built into coax/tests/auth/.cache/ (gitignored, never shipped or linked
into anything else) and called through ctypes. It runs configure and make, so
run this on a build host, not the Mac:

  coax/remote/remote.sh run -- python3 -I coax/tests/auth/ed25519_libsodium.py
  coax/remote/remote.sh run -- python3 -I coax/tests/auth/ed25519_libsodium.py --cases 20000 --seed 7

It then:
1. asks libsodium about every published case ed25519_vectors_test.coax runs
   (RFC 8032 7.1, all 210 Wycheproof cases with make_ed25519_vectors.py's
   ACCEPTABLE table, the twelve "Taming the many EdDSAs" cases) and requires
   the same verdict that test expects;
2. makes a seeded corpus of signatures (libsodium's own, and adversarial ones:
   flipped bits, S + L and larger S, keys and R with torsion parts, small-order
   keys and R, R = O with S = k a, non-canonical encodings of A and R, random
   keys), records libsodium's verdict on each, and writes it as Coax;
3. runs ed25519_libsodium_test.coax, which requires ed25519_verify to agree
   on every case.

With no --cases, step 2 makes the committed corpus
(ed25519_libsodium_vectors.coax, 600 cases, seed 13221) and requires the
committed file to be byte for byte the same (--write rewrites it). With
--cases N it writes N cases and a copy of the test under .cache/run/ and runs
that instead. The Python point arithmetic below only builds inputs; every
verdict is libsodium's."""
if not __debug__: raise SystemExit('tests require Python assertions')
import argparse, ctypes as C, hashlib, os, platform, random, subprocess, sys, tarfile, urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
CACHE = HERE / '.cache'
VERSION = 'libsodium-1.0.20'
URL = 'https://download.libsodium.org/libsodium/releases/%s.tar.gz' % VERSION
SHA256 = 'ebb65ef6ca439333c2bb41a0c1990587288da07f6c7fd07cb3a18cc18d30ce19'
COMMITTED = HERE / 'ed25519_libsodium_vectors.coax'
TEST = HERE / 'ed25519_libsodium_test.coax'
DEFAULT_CASES, DEFAULT_SEED = 600, 13221

# ---- libsodium
def build():
    lib = CACHE / VERSION / 'install/lib' / ('libsodium.dylib' if platform.system() == 'Darwin' else 'libsodium.so')
    if lib.exists(): return lib
    CACHE.mkdir(exist_ok=True)
    tarball = CACHE / (VERSION + '.tar.gz')
    if not tarball.exists() or hashlib.sha256(tarball.read_bytes()).hexdigest() != SHA256:
        with urllib.request.urlopen(URL, timeout=120) as r: data = r.read()
        assert hashlib.sha256(data).hexdigest() == SHA256, 'libsodium tarball digest differs from the pin'
        tarball.write_bytes(data)
    src = CACHE / VERSION / 'src'
    src.mkdir(parents=True, exist_ok=True)
    with tarfile.open(tarball) as t:
        # Regular files and directories under the tree only (no links, no
        # absolute or parent paths), on Pythons without extraction filters.
        for m in t.getmembers():
            assert (m.isfile() or m.isdir()) and not m.name.startswith('/') and '..' not in Path(m.name).parts, m.name
        t.extractall(src)
    tree = src / VERSION
    prefix = CACHE / VERSION / 'install'
    jobs = str(min(8, os.cpu_count() or 2))
    subprocess.run(['./configure', '--prefix=' + str(prefix), '--disable-dependency-tracking', '--enable-shared', '--disable-static'],
                   cwd=tree, check=True, stdout=subprocess.DEVNULL)
    subprocess.run(['make', '-j', jobs, 'install'], cwd=tree, check=True, stdout=subprocess.DEVNULL)
    assert lib.exists(), lib
    return lib

class Sodium:
    def __init__(self, path):
        self.lib = C.CDLL(str(path))
        assert self.lib.sodium_init() >= 0
        self.lib.crypto_sign_verify_detached.argtypes = [C.c_char_p, C.c_char_p, C.c_ulonglong, C.c_char_p]
        self.lib.crypto_sign_seed_keypair.argtypes = [C.c_char_p, C.c_char_p, C.c_char_p]
        self.lib.crypto_sign_detached.argtypes = [C.c_char_p, C.c_void_p, C.c_char_p, C.c_ulonglong, C.c_char_p]
    def verify(self, sig, msg, key):
        # The C call reads exactly 64 and 32 bytes: other lengths refuse here,
        # as crypto_sign_open's callers do.
        if len(sig) != 64 or len(key) != 32: return False
        return self.lib.crypto_sign_verify_detached(sig, msg, len(msg), key) == 0
    def keypair(self, seed):
        pk, sk = C.create_string_buffer(32), C.create_string_buffer(64)
        assert self.lib.crypto_sign_seed_keypair(pk, sk, seed) == 0
        return pk.raw, sk.raw
    def sign(self, msg, sk):
        sig = C.create_string_buffer(64)
        assert self.lib.crypto_sign_detached(sig, None, msg, len(msg), sk) == 0
        return sig.raw

# ---- Edwards arithmetic, only to build adversarial inputs
P = 2**255 - 19
L = 2**252 + 27742317777372353535851937790883648493
D = (-121665 * pow(121666, -1, P)) % P
SQRTM1 = pow(2, (P - 1) // 4, P)
O = (0, 1, 1, 0)
def add(p, q):
    a = (p[1] - p[0]) * (q[1] - q[0]) % P; b = (p[1] + p[0]) * (q[1] + q[0]) % P
    c = p[3] * 2 * D * q[3] % P; d = p[2] * 2 * q[2] % P
    e, f, g, h = b - a, d - c, d + c, b + a
    return (e * f % P, g * h % P, f * g % P, e * h % P)
def mul(k, p):
    r = O
    while k:
        if k & 1: r = add(r, p)
        p = add(p, p); k >>= 1
    return r
def neg(p): return ((-p[0]) % P, p[1], p[2], (-p[3]) % P)
def enc(p):
    zi = pow(p[2], -1, P); x = p[0] * zi % P; y = p[1] * zi % P
    return (y | ((x & 1) << 255)).to_bytes(32, 'little')
def dec(b):
    """Lenient decoding (y reduced mod p): the point, or None."""
    v = int.from_bytes(b, 'little'); sign = v >> 255; y = (v & ((1 << 255) - 1)) % P
    u = (y * y - 1) % P; w = (D * y * y + 1) % P
    x = pow(u * pow(w, -1, P), (P + 3) // 8, P)
    if (w * x * x - u) % P: x = x * SQRTM1 % P
    if (w * x * x - u) % P: return None
    if x & 1 != sign: x = (-x) % P
    return (x, y, 1, x * y % P)
B = dec(bytes.fromhex('58' + '66' * 31))
TORSION = [dec(bytes.fromhex(h)) for h in (
    '0100000000000000000000000000000000000000000000000000000000000000',
    'ecffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f',
    '0000000000000000000000000000000000000000000000000000000000000000',
    '0000000000000000000000000000000000000000000000000000000000000080',
    '26e8958fc2b227b045c3f489f2ef98f0d5dfac05d3c63339b13802886d53fc05',
    '26e8958fc2b227b045c3f489f2ef98f0d5dfac05d3c63339b13802886d53fc85',
    'c7176a703d4dd84fba3c0b760d10670f2a2053fa2c39ccc64ec7fd7792ac037a',
    'c7176a703d4dd84fba3c0b760d10670f2a2053fa2c39ccc64ec7fd7792ac03fa')]
assert all(t is not None and mul(8, t)[0] == 0 for t in TORSION)
def h(*parts): return int.from_bytes(hashlib.sha512(b''.join(parts)).digest(), 'little') % L
def secret_scalar(seed):
    a = bytearray(hashlib.sha512(seed).digest()[:32]); a[0] &= 248; a[31] &= 127; a[31] |= 64
    return int.from_bytes(a, 'little')
def le(n): return (n % 2**256).to_bytes(32, 'little')
def flip(b, bit): b = bytearray(b); b[bit // 8] ^= 1 << (bit % 8); return bytes(b)
def noncanonical(rng):
    """y + p for a y below 19 (fits in 255 bits), either sign: some are points."""
    y = rng.randrange(0, 19)
    return ((y + P) | (rng.randrange(2) << 255)).to_bytes(32, 'little')

def sign_with(a, key, msg, r, rpoint):
    """S for R = rpoint (whose discrete log part is r) under scalar a and key bytes."""
    rb = enc(rpoint)
    return rb + le((r + h(rb, key, msg) * a) % L)

def corpus(sodium, count, seed):
    rng = random.Random(seed)
    kinds = ['valid', 'message', 'r-bit', 's-bit', 'key-bit', 's-plus-l', 's-high', 'mixed-key', 'mixed-r', 'mixed-both',
             'small-key', 'small-r', 'r-identity', 'noncanonical-r', 'noncanonical-key', 'random-key', 'x0-sign', 's-zero']
    out = []
    for i in range(count):
        kind = kinds[i % len(kinds)]
        seed_bytes = rng.randbytes(32)
        pk, sk = sodium.keypair(seed_bytes)
        a = secret_scalar(seed_bytes)
        msg = rng.randbytes(rng.choice([0, 1, 2, 31, 32, 33, 64, rng.randrange(0, 200)]))
        sig = sodium.sign(msg, sk)
        key = pk
        r = rng.randrange(1, L)
        if kind == 'message':
            msg = flip(msg, rng.randrange(8 * len(msg))) if msg else b'\0'
        elif kind == 'r-bit': sig = flip(sig, rng.randrange(256))
        elif kind == 's-bit': sig = flip(sig, 256 + rng.randrange(256))
        elif kind == 'key-bit': key = flip(key, rng.randrange(256))
        elif kind == 's-plus-l':
            s = int.from_bytes(sig[32:], 'little') + L * rng.randrange(1, 16)
            sig = sig[:32] + le(s)
        elif kind == 's-high':
            s = int.from_bytes(sig[32:], 'little') | (rng.randrange(1, 8) << 253)
            sig = sig[:32] + le(s)
        elif kind in ('mixed-key', 'mixed-r', 'mixed-both'):
            t_key = TORSION[rng.randrange(1, 8)] if kind != 'mixed-r' else O
            t_r = TORSION[rng.randrange(1, 8)] if kind != 'mixed-key' else O
            key = enc(add(mul(a, B), t_key))
            sig = sign_with(a, key, msg, r, add(mul(r, B), t_r))
        elif kind == 'small-key':
            key = enc(TORSION[rng.randrange(8)])
            sig = sign_with(0, key, msg, 0, TORSION[rng.randrange(8)]) if rng.randrange(2) else enc(TORSION[rng.randrange(8)]) + le(rng.randrange(L))
        elif kind == 'small-r':
            # R a torsion point and S = k a: [S]B - [k]A = O, which equals R only for R = O.
            rb = enc(TORSION[rng.randrange(8)])
            sig = rb + le(h(rb, key, msg) * a % L)
        elif kind == 'r-identity':
            rb = enc(O)
            sig = rb + le(h(rb, key, msg) * a % L)
        elif kind == 'noncanonical-r':
            rb = noncanonical(rng)
            sig = rb + (le(h(rb, key, msg) * a % L) if rng.randrange(2) else sig[32:])
        elif kind == 'noncanonical-key':
            key = noncanonical(rng)
        elif kind == 'random-key': key = rng.randbytes(32)
        elif kind == 'x0-sign':
            key = enc(TORSION[rng.choice([0, 1])])
            key = key[:31] + bytes([key[31] | 0x80])
        elif kind == 's-zero':
            sig = sig[:32] + bytes(32)
        out.append((kind, key, msg, sig, sodium.verify(sig, msg, key)))
    return out

def render(cases, seed):
    lines = ['// Generated by ed25519_libsodium.py (seed %d): %d cases, each with the verdict' % (seed, len(cases)),
             "// of libsodium 1.0.20's crypto_sign_verify_detached. Test data only.",
             'EdDiff :: struct {kind:string;key:string;msg:string;sig:string;valid:bool;}',
             'ED_DIFF :: [%d] EdDiff.[' % len(cases)]
    lines.append(',\n'.join('  EdDiff.{kind="%s",key="%s",msg="%s",sig="%s",valid=%s}' % (
        k, key.hex(), msg.hex(), sig.hex(), 'true' if v else 'false') for k, key, msg, sig, v in cases) + '];')
    return '\n'.join(lines) + '\n'

# ---- the published vectors
def published(sodium):
    sys.path.insert(0, str(HERE))
    import make_ed25519_vectors as mv
    n = 0
    for t in mv.rfc():
        assert sodium.verify(bytes.fromhex(t['signature']), bytes.fromhex(t['message']), bytes.fromhex(t['public'] if 'public' in t else t['key'])), t['name']
        n += 1
    keys, cases = mv.wycheproof()
    for c in cases:
        got = sodium.verify(bytes.fromhex(c['sig']), bytes.fromhex(c['msg']), bytes.fromhex(keys[c['key']]))
        assert got == c['valid'], ('wycheproof', c['id'], c['result'], got)
        n += 1
    for i, c in enumerate(mv.speccheck()):
        got = sodium.verify(bytes.fromhex(c['sig']), bytes.fromhex(c['msg']), bytes.fromhex(c['key']))
        assert got == c['valid'], ('taming', i, got)
        n += 1
    return n

def coax_test(path):
    coax = ROOT / 'coax/bin/coax'
    if not coax.exists():
        print('coax/bin/coax is missing: run the Coax test on a build host'); return 1
    return subprocess.run([str(coax), 'test', str(path)], cwd=ROOT).returncode

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--cases', type=int); ap.add_argument('--seed', type=int, default=DEFAULT_SEED)
    ap.add_argument('--write', action='store_true', help='rewrite the committed corpus')
    a = ap.parse_args()
    sodium = Sodium(build())
    print('published vectors: %d cases, libsodium agrees with every expectation' % published(sodium))
    if a.cases is None:
        cases = corpus(sodium, DEFAULT_CASES, DEFAULT_SEED)
        text = render(cases, DEFAULT_SEED)
        if a.write: COMMITTED.write_text(text)
        assert COMMITTED.read_text() == text, 'ed25519_libsodium_vectors.coax differs from what libsodium gives (--write)'
        target = TEST
    else:
        cases = corpus(sodium, a.cases, a.seed)
        run = CACHE / 'run'; run.mkdir(parents=True, exist_ok=True)
        (run / 'ed25519_libsodium_vectors.coax').write_text(render(cases, a.seed))
        target = run / 'ed25519_libsodium_test.coax'
        target.write_text(TEST.read_text().replace('#load "../../lib/crypto/ed25519.coax";',
                                                   '#load "%s";' % (ROOT / 'coax/lib/crypto/ed25519.coax')))
    valid = sum(c[4] for c in cases)
    kinds = {}
    for c in cases: kinds.setdefault(c[0], [0, 0])[c[4]] += 1
    print('corpus: %d cases, %d verify; by kind (refused, verified): %s' % (len(cases), valid, kinds))
    return coax_test(target)

if __name__ == '__main__':
    sys.exit(main())
