#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Python integers and libsecp256k1 decide each value outside Coax)
"""The cross-check of Bitcoin chapter 77, the GLV endomorphism.

It computes every line the example prints with Python integers that share
no code with Coax (the split transliterates libsecp256k1's
secp256k1_scalar_split_lambda, the wNAF counts follow chapter 16's
recoding), and compares the transcript with the fixture:

  python3 -I coax/tests/bitcoin/glv.py                          # compare with the fixture
  python3 -I coax/tests/bitcoin/glv.py --coax coax/bin/coax     # also run the example
  python3 -I coax/tests/bitcoin/glv.py --native                 # differential and timing

--native compiles coax/tests/bitcoin/glv_bench.coax to C (checked and
release), compares 2,000 seeded splits and 300 seeded multiplications
a·G + b·P on both paths with Python, then times chapter 19's and 23's
verifiers on chapter 16's path, on the endomorphism path and in
libsecp256k1 v0.8.0 (tools/coax-nostr/fuzz/build_oracle.py --fetch
--reviewed builds it).
"""
import ctypes as C
import hashlib
import os
import random
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent.parent
FIXTURE = REPO / 'bitcoin' / 'fixtures' / 'glv.out'
EXAMPLE = REPO / 'bitcoin' / 'examples' / 'glv.coax'
BENCH = HERE / 'glv_bench.coax'

P = 2**256 - 2**32 - 977
N = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141
G = (0x79BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798,
     0x483ADA7726A3C4655DA4FBFC0E1108A8FD17B448A68554199C47D08FFB10D4B8)
# libsecp256k1 v0.8.0, src/field.h (beta) and src/scalar_impl.h (the rest).
BETA = 0x7AE96A2B657C07106E64479EAC3434E99CF0497512F58995C1396C28719501EE
LAMBDA = 0x5363AD4CC05C30E0A5261C028812645A122E22EA20816678DF02967C1B23BD72
MINUS_B1 = 0xE4437ED6010E88286F547FA90ABFE4C3
MINUS_B2 = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE8A280AC50774346DD765CDA83DB1562C
G1 = 0x3086D221A7D46BCDE86C90E49284EB153DAA8A1471E8CA7FE893209A45DBB031
G2 = 0xE4437ED6010E88286F547FA90ABFE4C4221208AC9DF506C61571B4AE8AC47F71


def add(a, b):
    if a is None: return b
    if b is None: return a
    if a[0] == b[0] and (a[1] + b[1]) % P == 0: return None
    if a == b: slope = 3 * a[0] * a[0] * pow(2 * a[1], -1, P) % P
    else: slope = (b[1] - a[1]) * pow(b[0] - a[0], -1, P) % P
    x = (slope * slope - a[0] - b[0]) % P
    return x, (slope * (a[0] - x) - a[1]) % P


def mul(k, point):
    result = None
    while k:
        if k & 1: result = add(result, point)
        point, k = add(point, point), k >> 1
    return result


def lift(x, odd):
    y = pow((x ** 3 + 7) % P, (P + 1) // 4, P)
    assert y * y % P == (x ** 3 + 7) % P
    return x, (P - y if (y & 1) != odd else y)


def compressed(point):
    return b'\x00' if point is None else bytes([2 | (point[1] & 1)]) + point[0].to_bytes(32, 'big')


# ---- the split, as libsecp256k1 computes it
def mul_shift(k, g):
    product = k * g
    return (product >> 384) + ((product >> 383) & 1)


def split(k):
    c1 = mul_shift(k, G1) * MINUS_B1 % N
    c2 = mul_shift(k, G2) * MINUS_B2 % N
    r2 = (c1 + c2) % N
    r1 = (k - r2 * LAMBDA) % N
    return r1, r2


def half(r):
    return (N - r, True) if 2 * r >= N else (r, False)


# ---- chapter 16's recoding and the work each path does
def wnaf(value, width):
    digits = []
    for _ in range(257):
        d = 0
        if value & 1:
            d = value & ((1 << width) - 1)
            if d >= 1 << (width - 1): d -= 1 << width
            value -= d
        digits.append(d)
        value >>= 1
    return digits


def length(*streams):
    return max([i + 1 for s in streams for i, d in enumerate(s) if d] or [0])


def strauss_count(a, b):
    streams = (wnaf(a, 2), wnaf(b, 5))
    return length(*streams), sum(1 for s in streams for d in s if d)


def glv_count(a, b):
    streams = [wnaf(half(r)[0], 5) for r in split(a) + split(b)]
    joint = length(*streams)
    return max(joint - 1, 0), sum(1 for s in streams for d in s if d)


def tagged(tag, data):
    t = hashlib.sha256(tag.encode()).digest()
    return hashlib.sha256(t + t + data).digest()


SIG_1 = bytes.fromhex('934b1ea10a4b3c1757e2b0c017d0b6143ce3c9a7e6a4a49860d7a6ab210ee3d8'
                      '2442ce9d2b916064108014783e923ec36b49743e2ffa1c4496f01a512aafd9e5')
HASH_1 = bytes.fromhex('a0dc65ffca799873cbea0ac274015b9526505daaaed385155425f7337704883e')
BIP340_KEY = bytes.fromhex('dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659')
BIP340_MSG = bytes.fromhex('243f6a8885a308d313198a2e03707344a4093822299f31d0082efa98ec4e6c89')
BIP340_SIG = bytes.fromhex('6896bd60eeae296db48a229ff71dfe071bde413e6d43f917dc8dcf8c78de3341'
                           '8906d11ac976abccb20b091292bff4ea897efcb639ea871cfa95f6de339e4b0a')


def transcript():
    out = []
    h = lambda v: '%064x' % v
    out.append('beta: ' + h(BETA))
    out.append('lambda: ' + h(LAMBDA))
    assert P % 3 == 1 and N % 3 == 1
    out.append('beta^3 = 1 mod p: %s' % str(pow(BETA, 3, P) == 1).lower())
    out.append('lambda^3 = 1 mod n: %s, lambda^2 + lambda + 1 = 0: %s' % (
        str(pow(LAMBDA, 3, N) == 1).lower(), str((LAMBDA * LAMBDA + LAMBDA + 1) % N == 0).lower()))
    slow = mul(LAMBDA, G)
    out.append('x(lambda*G) by 256 doublings: ' + h(slow[0]))
    out.append('beta * x(G)                 : ' + h(BETA * G[0] % P))
    out.append('same point: %s' % str(slow == (BETA * G[0] % P, G[1])).lower())

    z = int.from_bytes(HASH_1, 'big') % N
    r, s = int.from_bytes(SIG_1[:32], 'big'), int.from_bytes(SIG_1[32:], 'big')
    w = pow(s, -1, N)
    u1, u2 = z * w % N, r * w % N

    def show_split(label, k):
        out.append('%s: %s' % (label, h(k)))
        r1, r2 = split(k)
        for name, part in (('r1', r1), ('r2', r2)):
            value, negative = half(part)
            assert value < 2**128
            out.append('  %s: %s %032x  (bit length %d)' % (name, '-' if negative else '+', value, value.bit_length()))
        out.append('  r1 + lambda*r2 = k: %s' % str((r1 + LAMBDA * r2) % N == k).lower())
    show_split('u1', u1)
    show_split('u2', u2)
    show_split('n - 1', N - 1)
    show_split('lambda', LAMBDA)
    show_split('near the bound', 0x7fffffffffffffffffffffffffffffffd576e73557a4501ddfe92f46681b20a0)

    old, new = strauss_count(u1, u2), glv_count(u1, u2)
    out.append('chapter 16, wNAF widths 2 and 5:         %d doublings, %d additions' % old)
    out.append('chapter 77, four width-5 wNAFs on halves: %d doublings, %d additions' % new)
    big_r = add(mul(u1, G), mul(u2, G))   # the key is 1, so Q = G
    out.append('same R: true')
    out.append('x(R): ' + h(big_r[0]))
    out.append('r   : ' + h(r))
    out.append('ecdsa_verify_vartime: %s' % str(big_r[0] % N == r).lower())
    z2 = int.from_bytes(HASH_1[:31] + bytes([HASH_1[31] ^ 1]), 'big') % N
    bad = add(mul(z2 * w % N, G), mul(u2, G))
    out.append('hash bit changed: same R: true')
    out.append('x(R): ' + h(bad[0]))
    out.append('ecdsa_verify_vartime: %s' % str(bad[0] % N == r).lower())

    px = int.from_bytes(BIP340_KEY, 'big')
    rx, ss = int.from_bytes(BIP340_SIG[:32], 'big'), int.from_bytes(BIP340_SIG[32:], 'big')
    e = int.from_bytes(tagged('BIP0340/challenge', BIP340_SIG[:32] + BIP340_KEY + BIP340_MSG), 'big') % N
    old, new = strauss_count(ss, N - e), glv_count(ss, N - e)
    out.append('BIP-340 vector 1: chapter 16 %d doublings, %d additions; chapter 77 %d doublings, %d additions' % (old + new))
    out.append('same R: true')
    big_r = add(mul(ss, G), mul(N - e, lift(px, 0)))
    out.append('schnorr_verify_vartime: %s' % str(big_r is not None and big_r[1] % 2 == 0 and big_r[0] == rx).lower())
    return out


# ---- native: differential and timing
class View(C.Structure):
    _fields_ = [('data', C.POINTER(C.c_uint8)), ('count', C.c_int64)]


def view(data):
    buffer = (C.c_uint8 * max(1, len(data)))(*data)
    return View(C.cast(buffer, C.POINTER(C.c_uint8)), len(data)), buffer


def run(*args):
    r = subprocess.run([str(a) for a in args], capture_output=True, text=True)
    if r.returncode:
        sys.exit('%s failed: %s' % (args[0], r.stderr or r.stdout))
    return r.stdout


def build(coax, directory, mode):
    label = 'glv-' + (mode[0].strip('-') if mode else 'checked')
    c = directory / (label + '.c')
    c.write_text(run(coax, 'c', BENCH, '--library', '--header', directory / (label + '.h'), *mode))
    run('cc', '-std=c11', '-O2', '-shared', '-fPIC', c, '-lm', '-o', directory / (label + '.dylib'))
    lib = C.CDLL(str(directory / (label + '.dylib')))
    for name in ('glv_bench_split', 'glv_bench_ecdsa_strauss', 'glv_bench_schnorr_strauss',
                 'ecdsa_verify_vartime', 'schnorr_verify_vartime'):
        getattr(lib, name).restype = C.c_bool
    lib.glv_bench_ecmult.restype = C.c_int64
    return lib


def call(fn, *args):
    views = [view(a) if isinstance(a, bytes) else (a, None) for a in args]
    return fn(*[v for v, _ in views])


def ecdsa_sign(secret, digest, rng):
    while True:
        k = rng.randrange(1, N)
        r = mul(k, G)[0] % N
        s = pow(k, -1, N) * (int.from_bytes(digest, 'big') + r * secret) % N
        if r and s:
            return r.to_bytes(32, 'big') + min(s, N - s).to_bytes(32, 'big')


def schnorr_sign(secret, message, rng):
    point = mul(secret, G)
    d = secret if point[1] % 2 == 0 else N - secret
    k0 = rng.randrange(1, N)
    big_r = mul(k0, G)
    k = k0 if big_r[1] % 2 == 0 else N - k0
    rb, pb = big_r[0].to_bytes(32, 'big'), point[0].to_bytes(32, 'big')
    e = int.from_bytes(tagged('BIP0340/challenge', rb + pb + message), 'big') % N
    return pb, rb + ((k + e * d) % N).to_bytes(32, 'big')


class Libsecp:
    def __init__(self):
        sys.path.insert(0, str(REPO / 'tools/coax-nostr/fuzz'))
        from oracle import DEFAULT_LIBRARY
        self.lib = L = C.CDLL(str(DEFAULT_LIBRARY))
        L.secp256k1_context_create.restype = C.c_void_p
        L.secp256k1_context_create.argtypes = [C.c_uint]
        for name in ('secp256k1_ec_pubkey_parse', 'secp256k1_ecdsa_signature_parse_compact',
                     'secp256k1_ecdsa_verify', 'secp256k1_xonly_pubkey_parse', 'secp256k1_schnorrsig_verify'):
            getattr(L, name).argtypes = None
        self.ctx = C.c_void_p(L.secp256k1_context_create(1))

    def ecdsa(self, digest, key, sig):
        L, pub, s = self.lib, C.create_string_buffer(64), C.create_string_buffer(64)
        if not L.secp256k1_ec_pubkey_parse(self.ctx, pub, key, C.c_size_t(len(key))): return False
        if not L.secp256k1_ecdsa_signature_parse_compact(self.ctx, s, sig): return False
        return bool(L.secp256k1_ecdsa_verify(self.ctx, s, digest, pub))

    def schnorr(self, message, key, sig):
        L, pub = self.lib, C.create_string_buffer(64)
        if not L.secp256k1_xonly_pubkey_parse(self.ctx, pub, key): return False
        return bool(L.secp256k1_schnorrsig_verify(self.ctx, sig, message, C.c_size_t(len(message)), pub))


def native(coax):
    rng = random.Random(77)
    secp = Libsecp()
    with tempfile.TemporaryDirectory(prefix='coax-glv-') as tmp:
        libs = {mode: build(coax, Path(tmp), [mode] if mode else []) for mode in ('', '--release')}
        for mode, lib in libs.items():
            # Splits: seeded scalars, the bounds' neighbourhood and small values.
            scalars = [rng.randrange(N) for _ in range(1990)] + [0, 1, 2, N - 1, N - 2, LAMBDA, N - LAMBDA,
                                                                  (N - 1) // 2, (N + 1) // 2, 2**128]
            for k in scalars:
                buf = (C.c_uint8 * 64)()
                kv, _kb = view(k.to_bytes(32, 'big'))
                ov = View(C.cast(buf, C.POINTER(C.c_uint8)), 64)
                assert lib.glv_bench_split(kv, ov)
                out = bytes(buf)
                r1, r2 = split(k)
                assert out == r1.to_bytes(32, 'big') + r2.to_bytes(32, 'big'), hex(k)
                assert half(r1)[0] < 2**128 and half(r2)[0] < 2**128 and (r1 + LAMBDA * r2) % N == k
            # Multiplications on both paths against Python.
            cases = [(rng.randrange(N), rng.randrange(1, N), rng.randrange(N)) for _ in range(290)]
            cases += [(0, 1, 0), (0, 1, 1), (1, 1, N - 1), (N - 1, 7, 0), (LAMBDA, 1, 0), (0, 5, LAMBDA),
                      (2**128, 3, 2**128 - 1), (1, 2, (N - 1) // 2), (0, N - 1, 1), (N - 1, 1, N - 1)]
            for a, d, b in cases:
                point = mul(d, G)
                want = compressed(add(mul(a, G), mul(b, point)))
                for glv in (True, False):
                    buf = (C.c_uint8 * 33)()
                    av, _a = view(a.to_bytes(32, 'big')); pv, _p = view(compressed(point)); bv, _b = view(b.to_bytes(32, 'big'))
                    got = lib.glv_bench_ecmult(av, pv, bv, glv, View(C.cast(buf, C.POINTER(C.c_uint8)), 33))
                    assert got in (1, 33) and bytes(buf)[:got] == want, (a, d, b, glv)
            print('%s: %d splits and %d multiplications on both paths agree with Python'
                  % (mode or 'checked', len(scalars), len(cases)))
        # Signatures, valid and changed, on the three verifiers.
        lib = libs['--release']
        ecdsa, schnorr = [], []
        for i in range(200):
            secret = rng.randrange(1, N)
            digest = rng.randbytes(32)
            sig = ecdsa_sign(secret, digest, rng)
            key = compressed(mul(secret, G))
            if i % 2: sig = sig[:63] + bytes([sig[63] ^ 1])
            ecdsa.append((digest, key, sig, i % 2 == 0))
            message = rng.randbytes(32)
            pub, ssig = schnorr_sign(secret, message, rng)
            if i % 2: ssig = ssig[:63] + bytes([ssig[63] ^ 1])
            schnorr.append((message, pub, ssig, i % 2 == 0))
        paths = {
            'ECDSA, chapter 16 path': lambda c: call(lib.glv_bench_ecdsa_strauss, c[0], c[1], c[2]),
            'ECDSA, endomorphism path': lambda c: call(lib.ecdsa_verify_vartime, c[0], c[1], c[2], True),
            'ECDSA, libsecp256k1': lambda c: secp.ecdsa(c[0], c[1], c[2]),
            'BIP-340, chapter 16 path': lambda c: call(lib.glv_bench_schnorr_strauss, c[0], c[1], c[2]),
            'BIP-340, endomorphism path': lambda c: call(lib.schnorr_verify_vartime, c[0], c[1], c[2]),
            'BIP-340, libsecp256k1': lambda c: secp.schnorr(c[0], c[1], c[2]),
        }
        rates = {}
        for name, fn in paths.items():
            corpus = ecdsa if name.startswith('ECDSA') else schnorr
            results = [bool(fn(c)) for c in corpus]
            assert results == [c[3] for c in corpus], name
            best = None
            for _ in range(3):
                start = time.perf_counter()
                for c in corpus: fn(c)
                elapsed = (time.perf_counter() - start) / len(corpus)
                best = elapsed if best is None else min(best, elapsed)
            rates[name] = best
            print('%-28s %8.1f us per verification (%d signatures, half changed, all decided as expected)'
                  % (name, best * 1e6, len(corpus)))
        for kind in ('ECDSA', 'BIP-340'):
            old, new, ref = (rates['%s, %s' % (kind, p)] for p in ('chapter 16 path', 'endomorphism path', 'libsecp256k1'))
            print('%s: the endomorphism path is %.2fx faster than chapter 16\'s; libsecp256k1 is %.1fx faster still'
                  % (kind, old / new, new / ref))


def main():
    lines = transcript()
    want = FIXTURE.read_text().splitlines() if FIXTURE.exists() else []
    if '--write' in sys.argv:
        FIXTURE.write_text('\n'.join(lines) + '\n')
        want = lines
    if lines != want:
        for a, b in zip(lines, want):
            if a != b:
                print('python: ' + a + '\nfixture: ' + b)
                break
        sys.exit('glv.py: the transcript differs from ' + FIXTURE.name)
    coax = REPO / 'coax/bin/coax'
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            got = subprocess.run([coax, 'run', *mode, str(EXAMPLE)], capture_output=True, text=True, check=True).stdout
            if got.splitlines() != lines:
                sys.exit('glv.py: the example differs in ' + (mode[0] if mode else 'debug'))
    print('glv.py: %d lines agree' % len(lines))
    if '--native' in sys.argv:
        native(coax)


if __name__ == '__main__':
    main()
