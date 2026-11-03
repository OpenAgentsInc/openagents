#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Python transliterations of Bitcoin Core and BOLT 11 decide each mutated input outside Coax)
"""The fuzzer of Bitcoin chapter 119.

It compiles coax/tests/bitcoin/fuzz_targets.coax to C twice, as a checked
build (every slice access and overflow is checked and traps) and a release
build, and sends each parser seeded mutations of the pinned vectors. For
every input it checks three things:

- the parser returns at all: a trap or a signal is a crash;
- an accepted input writes back to the same bytes, where the format has one
  encoding (fuzz_targets.coax returns 999 when it does not);
- the result equals the Python reference's: tx_reference.py (Core's
  transaction reader), p2p.py (Core's header and version reads),
  psbt_reference.py (Core's DecodeRawPSBT), bolt11.py (an independent BOLT 11
  reader) and a Base58Check reader written here from base58.cpp.

Inputs are a pure function of (target, seed, index), so any failure is
replayed from three numbers. Every failure found is kept in
fuzz_regressions.json, which is replayed first on every run.

  python3 -I coax/tests/bitcoin/fuzz.py                    # 2,000 inputs per target and build
  python3 -I coax/tests/bitcoin/fuzz.py --budget 50000     # a longer campaign
  python3 -I coax/tests/bitcoin/fuzz.py --target psbt --seed 7
  python3 -I coax/tests/bitcoin/fuzz.py --example --run      # the book example, in Python and Coax

There is no coverage feedback: the corpus is the pinned vectors, and each
input stacks one to eight mutations on one or two of them.
"""
import argparse
import ctypes as C
import hashlib
import json
import random
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent / 'lightning'))
import tx_reference as T  # noqa: E402
import psbt_reference as R  # noqa: E402
import psbt_cases as PC  # noqa: E402
import p2p as P  # noqa: E402
import core_messages  # noqa: E402
import bolt11 as B  # noqa: E402
import bolts  # noqa: E402

UP = REPO / 'coax/tests/vectors/upstream'
TARGETS_FILE = HERE / 'fuzz_targets.coax'
REGRESSIONS = HERE / 'fuzz_regressions.json'
ROUNDTRIP = 999
MAX_INPUT = 16384
MAINNET = bytes.fromhex('f9beb4d9')
CAPS = (64, 64, 256)
INV_NAMES = ['OK', 'BAD_BECH32', 'BAD_PREFIX', 'BAD_AMOUNT', 'TOO_SHORT', 'TRUNCATED', 'BAD_LENGTH', 'REQUIRED',
             'NOT_MINIMAL', 'BAD_UTF8', 'BAD_FEATURES', 'BAD_FALLBACK', 'BAD_ROUTE', 'BAD_SIGNATURE', 'CAPACITY']
B58 = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'
BECH32 = 'qpzry9x8gf2tvdw0s3jn54khce6mua7l'


def pinned_json(name):
    """A pinned JSON file, after its SHA-256 matches the manifest."""
    entry = next(e for e in json.loads((UP.parent / 'manifest.json').read_text())['files'] if e['path'] == 'upstream/' + name)
    data = (UP / name).read_bytes()
    assert hashlib.sha256(data).hexdigest() == entry['sha256'], name + ' differs from its pin'
    return json.loads(data)


# ---- the seed corpora, from the pinned vectors
def seeds_tx():
    out = []
    for name in ('bitcoin-core/tx_valid.json', 'bitcoin-core/tx_invalid.json'):
        out += [bytes.fromhex(row[1]) for row in pinned_json(name) if len(row) >= 3 and isinstance(row[1], str)]
    return sorted(set(out))


def seeds_p2p_version():
    M = core_messages.load()
    out = []
    for relay in (0, 1):
        for agent in (b'', b'/Satoshi:32.0.0/', b'x' * 256):
            v = M.msg_version()
            v.nVersion, v.nServices, v.nTime, v.nStartingHeight, v.relay = 70016, 1033, 1730000000, 840000, relay
            v.nNonce = 0x0102030405060708   # the framework draws a random nonce
            v.strSubVer = agent.decode()
            full = v.serialize()
            out += [full, full[:46], full[:80], full[:-1], full[:-5]]
    return sorted(set(out))


def seeds_p2p_frame():
    M = core_messages.load()
    payloads = [(b'version', s) for s in seeds_p2p_version()[:4]]
    payloads += [(b'verack', b''), (b'ping', M.msg_ping(0x0102030405060708).serialize()),
                 (b'sendcmpct', M.msg_sendcmpct(True, 2).serialize()), (b'feefilter', M.msg_feefilter(1000).serialize()),
                 (b'getaddr', b''), (b'inv', bytes([1]) + (1).to_bytes(4, 'little') + bytes(range(32)))]
    return [P.frame(MAINNET, c.decode(), p) for c, p in payloads]


def seeds_psbt():
    decoded = (R.unbase64(case[3]) for case in PC.all_cases())
    return sorted({b for b in decoded if b is not None})


def seeds_invoice():
    return [row['invoice'].encode() for row in bolts.bolt11_json()]


def seeds_base58check():
    out = [row[0].encode() for row in pinned_json('bitcoin-core/key_io_valid.json') if row[0][0] in '123mnc5KL9']
    out += [row[1].encode() for row in pinned_json('bitcoin-core/base58_encode_decode.json')]
    return sorted(set(out))


# ---- the references
def ref_tx(b):
    try:
        T.unserialize(b, True, CAPS)
        return 0
    except T.Refused as e:
        return e.code


def ref_tx_decode(b):
    tx, refused = T.decode(b, CAPS)
    if tx is None:
        return refused.code
    return 100 if tx['segwit'] else 0


FRAME = {'ok': 0, 'truncated': 1, 'bad magic': 2, 'oversize': 3, 'bad checksum': 4, 'bad command': 5}


def ref_p2p_frame(b):
    return FRAME[P.read_frame(b, MAINNET, 4000000)[0]]


def ref_p2p_version(b):
    result, parts = P.version_parts(b)
    return 100 + parts if result == 'ok' else 6


def ref_psbt(b):
    e = R.refusal(b)
    return 0 if e is None else e.code


def ref_invoice(b):
    try:
        B.decode(b.decode('latin-1'))
        return 0
    except B.Refused as e:
        return INV_NAMES.index(str(e))


def ref_base58check(b):
    """base58.cpp's DecodeBase58Check as base58.coax bounds it: the first
    bad character or the first byte past 1,024 stops the read."""
    text = b.decode('latin-1')
    ones = len(text) - len(text.lstrip('1'))
    value = 0
    for c in text[ones:]:
        if c not in B58:
            return 1
        value = value * 58 + B58.index(c)
        if (value.bit_length() + 7) // 8 > 1024:
            return 7
    data = bytes(ones) + (value.to_bytes((value.bit_length() + 7) // 8, 'big') if value else b'')
    if len(data) > 1024:
        return 7
    if len(data) < 4:
        return 3
    if hashlib.sha256(hashlib.sha256(data[:-4]).digest()).digest()[:4] != data[-4:]:
        return 2
    return 0


# ---- mutations
INTERESTING = [0x00, 0x01, 0x7f, 0x80, 0xfc, 0xfd, 0xfe, 0xff]


def mutate_bytes(rng, data, other):
    b = bytearray(data)
    for _ in range(rng.randint(1, 8)):
        op = rng.randrange(9)
        at = rng.randrange(len(b) + 1)
        if op == 0 and b:
            b[min(at, len(b) - 1)] ^= 1 << rng.randrange(8)
        elif op == 1 and b:
            b[min(at, len(b) - 1)] = rng.choice(INTERESTING)
        elif op == 2:
            b[at:at] = bytes(rng.randrange(256) for _ in range(rng.randint(1, 4)))
        elif op == 3 and b:
            del b[at:at + rng.randint(1, 16)]
        elif op == 4 and b:
            n = rng.randint(1, 32)
            b[at:at] = b[at:at + n]
        elif op == 5:
            del b[at:]
        elif op == 6 and other:
            s = rng.randrange(len(other) + 1)
            b[at:] = other[s:s + rng.randint(1, 64)] + b[at:]
        elif op == 7 and len(b) >= 4:
            # a little-endian length or count: 0, 1, 2^32 - 1 or a nearby value
            at = min(at, len(b) - 4)
            v = rng.choice([0, 1, 0xffffffff, int.from_bytes(b[at:at + 4], 'little') + rng.randint(-2, 2)])
            b[at:at + 4] = (v & 0xffffffff).to_bytes(4, 'little')
        elif op == 8:
            # a CompactSize prefix of each width
            b[at:at] = rng.choice([b'\xfd\xfd\x00', b'\xfd\xff\xff', b'\xfe\x00\x00\x01\x00', b'\xff' + bytes(7) + b'\x01',
                                   b'\xfc', b'\x00'])
    return bytes(b[:MAX_INPUT])


def mutate_text(rng, data, other, alphabet):
    b = bytearray(data)
    for _ in range(rng.randint(1, 4)):
        op = rng.randrange(6)
        at = rng.randrange(len(b) + 1)
        if op == 0 and b:
            b[min(at, len(b) - 1)] = ord(rng.choice(alphabet))
        elif op == 1:
            b[at:at] = rng.choice(alphabet).encode()
        elif op == 2 and b:
            del b[at:at + rng.randint(1, 4)]
        elif op == 3 and b:
            b[min(at, len(b) - 1)] = rng.choice(b'0OIl/+ \x00\x7f\xff1')
        elif op == 4:
            b = bytearray(other[:at] + b[at:]) if other else b
        elif op == 5 and b:
            i = min(at, len(b) - 1)
            # ASCII letters only: chr(0xff).swapcase() is not one byte
            b[i] ^= 0x20 if chr(b[i]).isascii() and chr(b[i]).isalpha() else 0
    return bytes(b[:MAX_INPUT])


def mutate_frame(rng, data, other):
    """A frame with its payload changed, and usually its length and
    checksum made to match, so the payload reaches the command check."""
    b = mutate_bytes(rng, data, other)
    if len(b) >= 24 and rng.random() < 0.75:
        payload = b[24:]
        b = b[:16] + len(payload).to_bytes(4, 'little') + P.sha256d(payload)[:4] + payload
    return b


def mutate_invoice(rng, data, other):
    """Change the words, then make the Bech32 checksum again, so the input
    reaches the field reader; or change the raw text."""
    if rng.random() < 0.25:
        return mutate_text(rng, data, other, BECH32)
    try:
        hrp, words = B.bech32_decode(data.decode())
    except (B.Refused, UnicodeDecodeError):
        return mutate_text(rng, data, other, BECH32)
    for _ in range(rng.randint(1, 4)):
        op = rng.randrange(5)
        at = rng.randrange(len(words) + 1)
        if op == 0 and words:
            words[min(at, len(words) - 1)] = rng.randrange(32)
        elif op == 1:
            words[at:at] = [rng.randrange(32) for _ in range(rng.randint(1, 3))]
        elif op == 2 and words:
            del words[at:at + rng.randint(1, 8)]
        elif op == 3 and len(words) > 7:
            # a tagged field's length (two words after its type)
            words[min(at, len(words) - 3) + 1] = rng.randrange(32)
        elif op == 4 and rng.random() < 0.3:
            hrp = rng.choice(['lnbc', 'lntb', 'lnbcrt', 'lntbs', 'lnbc1', 'lnbc2500u', 'lnbc1p', 'lnbc10n', 'lnxx',
                              'lnbc20m', 'lnbc9999999999999999m'])
    return B.bech32_encode(hrp, words).encode()


TARGETS = {
    'tx': (seeds_tx, ref_tx, mutate_bytes),
    'tx_decode': (seeds_tx, ref_tx_decode, mutate_bytes),
    'p2p_frame': (seeds_p2p_frame, ref_p2p_frame, mutate_frame),
    'p2p_version': (seeds_p2p_version, ref_p2p_version, mutate_bytes),
    'psbt': (seeds_psbt, ref_psbt, mutate_bytes),
    'invoice': (seeds_invoice, ref_invoice, mutate_invoice),
    'base58check': (seeds_base58check, ref_base58check, lambda r, d, o: mutate_text(r, d, o, B58)),
}
# Results that mean "more than this harness's arrays hold"; the references
# have no such limit, so these are not compared.
CAPACITY = {'tx': {T.CAPACITY}, 'tx_decode': {T.CAPACITY}, 'psbt': {77}, 'invoice': {14}}


def generate(target, corpus, seed, index):
    rng = random.Random('%s:%d:%d' % (target, seed, index))
    first = corpus[rng.randrange(len(corpus))]
    other = corpus[rng.randrange(len(corpus))]
    return TARGETS[target][2](rng, first, other)


# ---- the native builds
def build(coax, directory, mode):
    label = 'fuzz-' + (mode.strip('-') or 'checked')
    c = directory / (label + '.c')
    r = subprocess.run([coax, 'c', str(TARGETS_FILE), '--library', '--header', str(directory / (label + '.h'))]
                       + ([mode] if mode else []), capture_output=True, text=True)
    if r.returncode:
        sys.exit('coax c failed: ' + r.stderr[-2000:])
    c.write_text(r.stdout)
    lib = directory / (label + '.dylib')
    subprocess.run(['cc', '-std=c11', '-O1', '-shared', '-fPIC', str(c), '-lm', '-o', str(lib)], check=True)
    return lib


class View(C.Structure):
    _fields_ = [('data', C.POINTER(C.c_uint8)), ('count', C.c_int64)]


def load(lib):
    so = C.CDLL(str(lib))
    for name in TARGETS:
        getattr(so, 'fuzz_' + name).restype = C.c_int64
        getattr(so, 'fuzz_' + name).argtypes = [View]
    return so


def call(so, target, data):
    buf = (C.c_uint8 * max(1, len(data))).from_buffer_copy(data or b'\0')
    return getattr(so, 'fuzz_' + target)(View(C.cast(buf, C.POINTER(C.c_uint8)), len(data)))


def check(so, target, data):
    """Coax's result, and None when the reference agrees, else what differed."""
    got = call(so, target, data)
    if got in CAPACITY.get(target, ()):
        return got, None
    want = TARGETS[target][1](data)
    if got == ROUNDTRIP or got != want:
        return got, {'coax': got, 'reference': want}
    return got, None


# ---- the worker: one target, one build, a range of indexes
def worker(args):
    so = load(args.lib)
    corpus = TARGETS[args.target][0]()
    found, results = [], {}
    for i in range(args.start, args.start + args.count):
        data = generate(args.target, corpus, args.seed, i)
        got, diff = check(so, args.target, data)
        results[got] = results.get(got, 0) + 1
        if diff:
            found.append(dict(index=i, input=data.hex(), **diff))
    print(json.dumps({'found': found, 'results': results}))


def run_range(lib, target, seed, start, count):
    """The disagreements in a range, or a crash narrowed to one index."""
    cmd = [sys.executable, '-I', '-B', str(Path(__file__)), '--worker', '--lib', str(lib), '--target', target,
           '--seed', str(seed), '--start', str(start), '--count', str(count)]
    r = subprocess.run(cmd, capture_output=True, text=True)
    if r.returncode == 0:
        out = json.loads(r.stdout)
        return out['found'], [], {int(k): v for k, v in out['results'].items()}
    if r.returncode > 0:
        # a Python error in the harness or a reference, not a crash in Coax
        sys.exit('fuzz.py: the worker failed on %s %d..%d:\n%s' % (target, start, start + count, r.stderr[-1500:]))
    if count == 1:
        return [], [dict(index=start, signal=-r.returncode, stderr=r.stderr[-300:])], {}
    half = count // 2
    a, ca, ra = run_range(lib, target, seed, start, half)
    b, cb, rb = run_range(lib, target, seed, start + half, count - half)
    return a + b, ca + cb, {k: ra.get(k, 0) + rb.get(k, 0) for k in set(ra) | set(rb)}


def replay(libs):
    """Every kept regression input, in both builds: it must now agree."""
    cases = json.loads(REGRESSIONS.read_text())['cases'] if REGRESSIONS.exists() else []
    bad = []
    for mode, lib in libs.items():
        so = load(lib)
        for case in cases:
            data = bytes.fromhex(case['input'])
            got = call(so, case['target'], data)
            if got != case['expected'] or TARGETS[case['target']][1](data) != case['expected']:
                bad.append((mode, case['target'], case['input'][:40], got))
    return len(cases), bad


# ---- the book's example: the same generator and mutations, in Python
EXAMPLE = REPO / 'bitcoin/examples/fuzzing.coax'
FIXTURE = REPO / 'bitcoin/fixtures/fuzzing.out'
SEEDS_FILE = HERE / 'fuzz_seeds.coax'
EXAMPLE_BUDGET = 1000
ROOM = 4096
MASK = (1 << 64) - 1


class XorShift:
    def __init__(self, state):
        self.x = state

    def below(self, n):
        x = self.x
        x ^= (x << 13) & MASK
        x ^= x >> 7
        x ^= (x << 17) & MASK
        self.x = x
        return x % n


def example_mutate(s, seed):
    """fz_mutate in fuzzing.coax, draw for draw."""
    b = bytearray(seed)
    for _ in range(1 + s.below(4)):
        op, at = s.below(6), s.below(len(b) + 1)
        if op == 0 and b:
            b[min(at, len(b) - 1)] ^= 1 << s.below(8)
        elif op == 1 and b:
            b[min(at, len(b) - 1)] = INTERESTING[s.below(8)]
        elif op == 2 and len(b) < ROOM:
            b[at:at] = bytes([s.below(256)])
        elif op == 3:
            d = min(1 + s.below(8), len(b) - at)
            del b[at:at + d]
        elif op == 4:
            del b[at:]
        elif op == 5:
            d = min(1 + s.below(16), len(b) - at, ROOM - len(b))
            b[at + d:at + d] = b[at:at + d]
    return bytes(b)


def example_transcript():
    import make_fuzz_seeds as MS
    assert SEEDS_FILE.read_text() == MS.render(), 'fuzz_seeds.coax differs from make_fuzz_seeds.py'
    names = [t for t, _ in MS.EXAMPLE]
    rows = MS.seeds()
    s = XorShift(119)
    out = ['%d inputs per parser, xorshift64 from 119' % EXAMPLE_BUDGET]
    for k, name in enumerate(names):
        seeds = [b for t, b in rows if t == k]
        results = {}
        for _ in range(EXAMPLE_BUDGET):
            data = example_mutate(s, seeds[s.below(len(seeds))])
            want = TARGETS[name][1](data)
            results[want] = results.get(want, 0) + 1
        accepted = sum(v for c, v in results.items() if c == 0 or 100 <= c <= 104)
        out.append('%s: %d seeds, %d accepted, %d refused, 0 accepted but not written back'
                   % (name, len(seeds), accepted, EXAMPLE_BUDGET - accepted))
        out.append('  results:' + ''.join(' %d:%d' % (c, v) for c, v in sorted(results.items())))
    return out


def example(args):
    lines = example_transcript()
    if args.write:
        FIXTURE.write_text('\n'.join(lines) + '\n')
    if FIXTURE.read_text().splitlines() != lines:
        sys.exit('fuzz.py: the example transcript differs from ' + FIXTURE.name)
    if args.run:
        for mode in ([], ['--release']):
            got = subprocess.run([args.coax, 'run', *mode, str(EXAMPLE)], capture_output=True, text=True, check=True).stdout
            if got.splitlines() != lines:
                sys.exit('fuzz.py: the example differs in ' + (mode[0] if mode else 'debug'))
    print('fuzz.py --example: %d lines agree' % len(lines))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--worker', action='store_true', help=argparse.SUPPRESS)
    ap.add_argument('--lib')
    ap.add_argument('--coax', default=str(REPO / 'coax/bin/coax'))
    ap.add_argument('--target', choices=sorted(TARGETS))
    ap.add_argument('--seed', type=int, default=119)
    ap.add_argument('--start', type=int, default=0)
    ap.add_argument('--count', type=int)
    ap.add_argument('--budget', type=int, default=2000, help='inputs per target and build')
    ap.add_argument('--chunk', type=int, default=1000)
    ap.add_argument('--example', action='store_true', help="compute the book example's transcript in Python")
    ap.add_argument('--run', action='store_true', help='with --example, also run the example in both builds')
    ap.add_argument('--write', action='store_true', help='with --example, rewrite the fixture')
    args = ap.parse_args()
    if args.worker:
        return worker(args)
    if args.example:
        return example(args)
    targets = [args.target] if args.target else list(TARGETS)
    total_failures = 0
    with tempfile.TemporaryDirectory(prefix='coax-fuzz-') as tmp:
        libs = {mode or 'checked': build(args.coax, Path(tmp), mode) for mode in ('', '--release')}
        n, bad = replay(libs)
        print('replayed %d kept inputs in both builds: %s' % (n, 'all agree' if not bad else bad))
        total_failures += len(bad)
        for target in targets:
            corpus = TARGETS[target][0]()
            for mode, lib in libs.items():
                started = time.time()
                diffs, crashes, results = [], [], {}
                for start in range(0, args.budget, args.chunk):
                    d, c, r = run_range(lib, target, args.seed, start, min(args.chunk, args.budget - start))
                    diffs += d
                    crashes += c
                    results = {k: results.get(k, 0) + r.get(k, 0) for k in set(results) | set(r)}
                print('%-12s %-7s %6d inputs from %4d seeds, seed %d: %d crashes, %d disagreements (%.0f s)'
                      % (target, mode, args.budget, len(corpus), args.seed, len(crashes), len(diffs), time.time() - started))
                print('    results: ' + ', '.join('%s %d' % (k, v) for k, v in sorted(results.items())))
                for f in (crashes + diffs)[:5]:
                    print('   ', json.dumps(f)[:300])
                total_failures += len(crashes) + len(diffs)
    if total_failures:
        sys.exit('fuzz.py: %d failures' % total_failures)
    print('fuzz.py: no crash and no disagreement')


if __name__ == '__main__':
    main()
