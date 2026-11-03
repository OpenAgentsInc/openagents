#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Python integers decide each value outside Coax)
"""The cross-check of Bitcoin chapter 25, BIP-340 batch verification.

It computes every line the example prints with Python integers that share
no code with Coax: BIP-340's Verify and BatchVerify as the pinned
bip-0340.mediawiki states them, ChaCha20 from RFC 8439 for the
randomizers, and the point-operation counts from chapter 77's split and
wNAF recoding (glv.py). Then it compares the transcript with the fixture:

  python3 -I coax/tests/bitcoin/batch.py                        # compare with the fixture
  python3 -I coax/tests/bitcoin/batch.py --coax coax/bin/coax   # also run the example
  python3 -I coax/tests/bitcoin/batch.py --coax coax/bin/coax --native

--native compiles coax/tests/bitcoin/batch_bench.coax to C (checked and
release), compares 300 seeded batches, a third of them with one changed
signature, with Python's single verification, then times batch and
single verification in the release build for batches of 1 to 64.
"""
import ctypes as C
import hashlib
import random
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import glv  # noqa: E402  chapter 77's oracle: curve arithmetic, split, wNAF

REPO = HERE.parent.parent.parent
FIXTURE = REPO / 'bitcoin' / 'fixtures' / 'schnorr-batch.out'
EXAMPLE = REPO / 'bitcoin' / 'examples' / 'schnorr-batch.coax'
BENCH = HERE / 'batch_bench.coax'
CSV = REPO / 'coax' / 'tests' / 'vectors' / 'upstream' / 'bip340' / 'test-vectors.csv'
BIP = REPO / 'coax' / 'tests' / 'vectors' / 'upstream' / 'bip340' / 'bip-0340.mediawiki'
P, N, G = glv.P, glv.N, glv.G


# ---- ChaCha20, RFC 8439 section 2.3
def chacha20_block(key, counter, nonce=bytes(12)):
    def rotl(v, c): return ((v << c) & 0xffffffff) | (v >> (32 - c))
    def quarter(s, a, b, c, d):
        s[a] = (s[a] + s[b]) & 0xffffffff; s[d] = rotl(s[d] ^ s[a], 16)
        s[c] = (s[c] + s[d]) & 0xffffffff; s[b] = rotl(s[b] ^ s[c], 12)
        s[a] = (s[a] + s[b]) & 0xffffffff; s[d] = rotl(s[d] ^ s[a], 8)
        s[c] = (s[c] + s[d]) & 0xffffffff; s[b] = rotl(s[b] ^ s[c], 7)
    words = lambda b: [int.from_bytes(b[i:i + 4], 'little') for i in range(0, len(b), 4)]
    state = [0x61707865, 0x3320646e, 0x79622d32, 0x6b206574] + words(key) + [counter] + words(nonce)
    s = list(state)
    for _ in range(10):
        quarter(s, 0, 4, 8, 12); quarter(s, 1, 5, 9, 13); quarter(s, 2, 6, 10, 14); quarter(s, 3, 7, 11, 15)
        quarter(s, 0, 5, 10, 15); quarter(s, 1, 6, 11, 12); quarter(s, 2, 7, 8, 13); quarter(s, 3, 4, 9, 14)
    return b''.join(((a + b) & 0xffffffff).to_bytes(4, 'little') for a, b in zip(s, state))


# RFC 8439 section 2.3.2's example block.
assert chacha20_block(bytes(range(32)), 1, bytes.fromhex('000000090000004a00000000'))[:16].hex() == \
    '10f1e7e4d13b5915500fdd1fa32071c4'


# ---- BIP-340
def lift_x(x):
    if x >= P: return None
    c = (x ** 3 + 7) % P
    y = pow(c, (P + 1) // 4, P)
    if y * y % P != c: return None
    return x, y if y % 2 == 0 else P - y


def challenge(r, key, message):
    return int.from_bytes(glv.tagged('BIP0340/challenge', r + key + message), 'big') % N


def verify(key, message, sig):
    point = lift_x(int.from_bytes(key, 'big'))
    r, s = int.from_bytes(sig[:32], 'big'), int.from_bytes(sig[32:], 'big')
    if point is None or r >= P or s >= N: return False
    e = challenge(sig[:32], key, message)
    big_r = glv.add(glv.mul(s, G), glv.mul(N - e, point))
    return big_r is not None and big_r[1] % 2 == 0 and big_r[0] == r


def randomizers(seed, u):
    a, counter, drawn = [1] if u else [], 0, 0
    while len(a) < u:
        block = chacha20_block(seed, counter); counter += 1
        for half in (block[:32], block[32:]):
            if len(a) < u:
                v = int.from_bytes(half, 'big'); drawn += 1
                if 1 <= v < N: a.append(v)
    return a, drawn


def seed_of(batch):
    return hashlib.sha256(b''.join(k for k, _, _ in batch) + b''.join(m for _, m, _ in batch)
                          + b''.join(s for _, _, s in batch)).digest()


ERRORS = ['valid', "the inputs don't describe a batch", 'public key is not an x coordinate on the curve',
          'r is not an x coordinate on the curve', 's is not below n', 'the combined equation does not hold']


def batch_verify(batch, a=None):
    """BIP-340's BatchVerify: (ok, error, index, doublings, additions, roots)."""
    if a is None: a = randomizers(seed_of(batch), len(batch))[0]
    g, terms, roots = 0, [], 0
    for i, (key, message, sig) in enumerate(batch):
        point = lift_x(int.from_bytes(key, 'big')); roots += 1
        if point is None: return False, 2, i, 0, 0, roots
        r, s = int.from_bytes(sig[:32], 'big'), int.from_bytes(sig[32:], 'big')
        if r >= P: return False, 3, i, 0, 0, roots
        if s >= N: return False, 4, i, 0, 0, roots
        e = challenge(sig[:32], key, message)
        big_r = lift_x(r); roots += 1
        if big_r is None: return False, 3, i, 0, 0, roots
        g = (g + a[i] * s) % N
        terms += [(big_r, (N - a[i]) % N), (point, (N - a[i] * e % N) % N)]
    total = glv.mul(g, G)
    for point, k in terms: total = glv.add(total, glv.mul(k, point))
    # The work of one Strauss chain over every half, plus a table of odd
    # multiples (one doubling, seven additions) for each point but G.
    streams = [glv.wnaf(glv.half(r)[0], 5) for k in [g] + [k for _, k in terms] for r in glv.split(k)]
    doubles = max(glv.length(*streams) - 1, 0) + len(terms)
    additions = sum(1 for s in streams for d in s if d) + 7 * len(terms)
    ok = total is None
    return ok, 0 if ok else 5, -1, doubles, additions, roots


def single_cost(batch):
    d = a = 0
    for key, message, sig in batch:
        e = challenge(sig[:32], key, message)
        dd, aa = glv.glv_count(int.from_bytes(sig[32:], 'big'), N - e)
        d += dd + 1; a += aa + 7
    return d, a, len(batch), len(batch)


def vectors():
    import csv
    rows = list(csv.DictReader(CSV.open()))
    return [(bytes.fromhex(r['public key']), bytes.fromhex(r['message']), bytes.fromhex(r['signature']),
             r['verification result'] == 'TRUE') for r in rows]


def report(out, label, result):
    ok, error, index = result[:3]
    if ok: out.append('%s: valid' % label)
    elif index >= 0: out.append('%s: refused at signature %d: %s' % (label, index + 1, ERRORS[error]))
    else: out.append('%s: refused: %s' % (label, ERRORS[error]))


def transcript():
    vec = vectors()
    for key, message, sig, valid in vec: assert verify(key, message, sig) == valid
    v = lambda i: vec[i][:3]
    valid = [0, 1, 2, 3, 4, 15, 16, 17, 18]
    out = ["BIP-340's valid vectors as one batch"]
    batch = [v(i) for i in valid]
    seed = seed_of(batch)
    out.append('  seed: ' + seed.hex())
    a, drawn = randomizers(seed, 9)
    for name, value in (('a1', a[0]), ('a2', a[1]), ('a9', a[8])): out.append('  %s: %064x' % (name, value))
    out.append('  ChaCha20 draws: %d, none skipped: %s' % (drawn, str(drawn == 8).lower()))
    r = batch_verify(batch)
    report(out, '  vectors 0-4 and 15-18', r)
    out.append('  one multiplication: %d doublings, %d additions, %d square roots, no inversion' % r[3:6])
    out.append('  nine verifications: %d doublings, %d additions, %d square roots, %d inversions' % single_cost(batch))
    out.append("BIP-340's invalid vectors, each between vectors 1 and 15")
    for i in range(5, 15):
        r = batch_verify([v(1), v(i), v(15)])
        assert not r[0] and not verify(*v(i))
        report(out, '  vector %d' % i, r)

    out.append('One bad signature among five')
    batch = [v(i) for i in range(5)]
    report(out, '  as signed', batch_verify(batch))
    sig = bytearray(batch[3][2]); sig[40] ^= 4
    batch[3] = (batch[3][0], batch[3][1], bytes(sig))
    report(out, "  a bit of signature 4's s changed", batch_verify(batch))
    out.append('  each alone: ' + ' '.join('ok' if verify(*b) else 'BAD' for b in batch))

    out.append('Two wrong signatures whose errors cancel')
    batch = [v(1), v(2)]
    for i, delta in ((0, 1), (1, -1)):
        key, message, sig = batch[i]
        s = (int.from_bytes(sig[32:], 'big') + delta) % N
        batch[i] = (key, message, sig[:32] + s.to_bytes(32, 'big'))
    out.append('  s1 + 1 and s2 - 1, each alone: %s, %s' % tuple(str(verify(*b)).lower() for b in batch))
    out.append('  summed with a1 = a2 = 1: %s' % ('valid' if batch_verify(batch, [1, 1])[0] else 'refused'))
    report(out, "  summed with BIP-340's randomizers", batch_verify(batch))

    out.append('Point operations per signature as the batch grows (the valid vectors, repeated)')
    out.append('      u | batch: doublings additions roots | alone: doublings additions roots inversions')
    for u in (1, 2, 4, 8, 16, 32, 64):
        batch = [v(valid[i % 9]) for i in range(u)]
        r = batch_verify(batch)
        assert r[0]
        s = single_cost(batch)
        out.append('%7d |%17d%10d%6d |%17d%10d%6d%11d' % (u, r[3] // u, r[4] // u, r[5] // u,
                                                           s[0] // u, s[1] // u, s[2] // u, s[3] // u))
    return out


def check_sources():
    text = BIP.read_text()
    for phrase in ('==== Batch Verification ====', "''a<sub>2...u</sub>''",
                   "seed = seed_hash(pk<sub>1</sub>..pk<sub>u</sub> || m<sub>1</sub>..m<sub>u</sub> || sig<sub>1</sub>..sig<sub>u</sub> )",
                   '[https://tools.ietf.org/html/rfc8439 ChaCha20] with key', "skipping integers not in the range ''1...n-1''"):
        assert phrase in text, phrase


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
    label = 'batch-' + (mode[0].strip('-') if mode else 'checked')
    c = directory / (label + '.c')
    c.write_text(run(coax, 'c', BENCH, '--library', '--header', directory / (label + '.h'), *mode))
    run('cc', '-std=c11', '-O2', '-shared', '-fPIC', c, '-lm', '-o', directory / (label + '.dylib'))
    lib = C.CDLL(str(directory / (label + '.dylib')))
    lib.batch_bench_verify.restype = C.c_bool
    lib.schnorr_verify_vartime.restype = C.c_bool
    return lib


def call(fn, *args):
    views = [view(a) for a in args]
    return fn(*[v for v, _ in views])


def native(coax):
    rng = random.Random(25)
    pool = []
    for _ in range(96):
        secret, message = rng.randrange(1, N), rng.randbytes(32)
        key, sig = glv.schnorr_sign(secret, message, rng)
        pool.append((key, message, sig))
    join = lambda batch, i: b''.join(b[i] for b in batch)
    with tempfile.TemporaryDirectory(prefix='coax-batch-') as tmp:
        libs = {mode: build(coax, Path(tmp), [mode] if mode else []) for mode in ('', '--release')}
        for mode, lib in libs.items():
            cases = changed = 0
            for n in range(300):
                batch = rng.sample(pool, rng.randrange(1, 65))
                if n % 3 == 0:
                    at, byte = rng.randrange(len(batch)), rng.randrange(64)
                    key, message, sig = batch[at]
                    batch[at] = (key, message, sig[:byte] + bytes([sig[byte] ^ (1 << rng.randrange(8))]) + sig[byte + 1:])
                    changed += 1
                want = all(verify(*b) for b in batch)
                got = call(lib.batch_bench_verify, join(batch, 0), join(batch, 1), join(batch, 2))
                assert got == want, (mode, n)
                cases += 1
            print('%s: %d batches (%d with one changed signature) agree with Python' % (mode or 'checked', cases, changed))
        lib = libs['--release']
        print('release build, microseconds per signature (best of 7):')
        for u in (1, 2, 4, 8, 16, 32, 64):
            batch = pool[:u]
            args = [view(join(batch, i)) for i in range(3)]
            singles = [[view(b[1]), view(b[0]), view(b[2])] for b in batch]
            rounds = max(2, 128 // u)
            best_b = best_s = None
            for _ in range(7):
                start = time.perf_counter()
                for _ in range(rounds): assert lib.batch_bench_verify(*[a for a, _ in args])
                tb = (time.perf_counter() - start) / (rounds * u)
                start = time.perf_counter()
                for _ in range(rounds):
                    for m, k, s in singles: assert lib.schnorr_verify_vartime(m[0], k[0], s[0])
                ts = (time.perf_counter() - start) / (rounds * u)
                best_b = tb if best_b is None else min(best_b, tb)
                best_s = ts if best_s is None else min(best_s, ts)
            print('  u = %2d: batch %7.1f, alone %7.1f, %.2fx' % (u, best_b * 1e6, best_s * 1e6, best_s / best_b))


def main():
    check_sources()
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
        sys.exit('batch.py: the transcript differs from ' + FIXTURE.name)
    print('schnorr-batch fixture: %d lines agree with Python' % len(lines))
    coax = REPO / 'coax/bin/coax'
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            got = subprocess.run([coax, 'run', *mode, str(EXAMPLE)], capture_output=True, text=True, check=True).stdout
            if got.splitlines() != lines:
                sys.exit('batch.py: the example differs in ' + (mode[0] if mode else 'debug'))
            print('native %s: agrees' % (mode[0].strip('-') if mode else 'debug'))
    if '--native' in sys.argv:
        native(coax)


if __name__ == '__main__':
    main()
