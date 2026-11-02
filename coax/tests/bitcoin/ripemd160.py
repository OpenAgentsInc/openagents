#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: the paper's text, the BIP-32 text and Python's hashlib check the Coax hashes)
"""RIPEMD-160 and HASH160 against their published values (Bitcoin chapter 6).

Builds coax/lib/bitcoin/hash160.coax (which loads coax/lib/crypto/ripemd160.coax)
as a C library in checked and release modes and checks, in both:

- the nine RIPEMD-160 test values of the paper's Appendix B, read from the
  pinned coax/tests/vectors/upstream/ripemd160/AB-9601.pdf, including one
  million "a" bytes;
- every input length from 0 to 300 bytes, and 20 random inputs up to 1 MiB,
  against Python's hashlib ripemd160 (OpenSSL) when it is available;
- every parent/child pair of extended public keys in the pinned BIP-32 text:
  HASH160 of the parent's key starts with the child's parent fingerprint.

Run from the repository root: python3 coax/tests/bitcoin/ripemd160.py
"""
if not __debug__: raise SystemExit('tests require Python assertions')
import ctypes as C
import hashlib
import os
from pathlib import Path
import random
import re
import subprocess
import tempfile
import zlib

ROOT = Path(__file__).resolve().parents[3]
COMPILER = Path(os.environ.get('COAX_TEST_COMPILER', ROOT/'coax/bin/coax'))
SOURCE = ROOT/'coax/lib/bitcoin/hash160.coax'
VECTORS = ROOT/'coax/tests/vectors/upstream'
MILLION_A = '1 million times "a"'


class View(C.Structure):
    _fields_ = [('data', C.POINTER(C.c_uint8)), ('count', C.c_int64)]


def view(buffer):
    return View(C.cast(buffer, C.POINTER(C.c_uint8)), len(buffer))


def run(*args):
    r = subprocess.run([str(a) for a in args], capture_output=True, text=True)
    assert r.returncode == 0, f'{args[0]} failed: {r.stderr or r.stdout}'
    return r.stdout


def paper_values():
    """The RIPEMD-160 block of Appendix B, from the PDF's compressed text."""
    pdf = (VECTORS/'ripemd160/AB-9601.pdf').read_bytes()
    text = b''
    for m in re.finditer(rb'stream\r?\n(.*?)endstream', pdf, re.S):
        try:
            text += zlib.decompress(m.group(1))
        except zlib.error:
            pass
    # The appendix heading "B Test Values", typeset with kerning.
    heading = re.search(rb'\[\(B\)[^\]]*\(est\)[^\]]*\(alues\)\]TJ', text)
    assert heading, 'Appendix B heading not found'
    start = text.index(b'(RIPEMD-160:)', heading.end()); end = text.index(b'(RIPEMD-128:)', start)
    lines = []
    for array in re.findall(rb'\[(.*?)\]TJ', text[start:end], re.S):
        # A TJ array is strings with kerning between them; a gap wider than
        # 200 units is a word space.
        parts = re.findall(rb'\(([^)]*)\)|(-?\d+(?:\.\d+)?)', array)
        line = ''.join(s.decode('latin-1') if s else (' ' if float(k) < -200 else '') for s, k in parts)
        lines.append(line)
    assert len(lines) == 18, lines
    pairs = list(zip(lines[0::2], lines[1::2]))
    messages = {'""': b'', '"a"': b'a', '"abc"': b'abc', '"message digest"': b'message digest',
                '"abcdefghijklmnopqrstuvwxyz"': b'abcdefghijklmnopqrstuvwxyz',
                '"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"': b'abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq',
                '"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"': b'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789',
                '8 times "1234567890"': b'1234567890' * 8, MILLION_A: b'a' * 1000000}
    assert [p[0] for p in pairs] == list(messages), [p[0] for p in pairs]
    for _, digest in pairs:
        assert re.fullmatch(r'[0-9a-f]{40}', digest), digest
    return [(messages[label], label, bytes.fromhex(digest)) for label, digest in pairs]


B58 = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'


def base58check(text):
    n = 0
    for ch in text:
        n = n * 58 + B58.index(ch)
    raw = n.to_bytes(82, 'big')
    payload, check = raw[:-4], raw[-4:]
    assert hashlib.sha256(hashlib.sha256(payload).digest()).digest()[:4] == check, text
    return payload


def bip32_pairs():
    """(parent key, child's parent fingerprint, child path) for every chain step."""
    text = (VECTORS/'bip32/bip-0032.mediawiki').read_text()
    pairs = []
    for vector in re.split(r'===Test vector \d+===', text)[1:4]:
        chain = re.findall(r'\* Chain (\S+)\n\*\* ext pub: (\w+)', vector)
        assert chain, vector[:80]
        keys = [(path, base58check(xpub)) for path, xpub in chain]
        for (_, parent), (path, child) in zip(keys, keys[1:]):
            assert child[4] == parent[4] + 1
            pairs.append((parent[45:78], child[5:9], path))
    return pairs


def library(directory, mode):
    c = directory/f'{mode}.c'
    c.write_text(run(COMPILER, 'c', SOURCE, '--library', *(['--release'] if mode == 'release' else [])))
    shared = directory/f'{mode}.so'
    run('cc', '-std=c11', '-O2', '-shared', '-fPIC', c, '-lm', '-o', shared)
    lib = C.CDLL(str(shared))
    for name in ('ripemd160_hash_checked', 'hash160_checked'):
        f = getattr(lib, name); f.argtypes = [View, View, C.c_bool]; f.restype = C.c_uint32
    return lib


def digest(f, data):
    src = (C.c_uint8 * max(len(data), 1)).from_buffer_copy(data or b'\0')
    out = (C.c_uint8 * 20)()
    status = f(View(C.cast(src, C.POINTER(C.c_uint8)), len(data)), view(out), False)
    assert status == 0, status
    return bytes(out)


def main():
    values = paper_values()
    pairs = bip32_pairs()
    try:
        oracle = lambda data: hashlib.new('ripemd160', data).digest()
        oracle(b'')
    except ValueError:
        oracle = None
    rng = random.Random(6)
    with tempfile.TemporaryDirectory(prefix='coax-ripemd160-') as temp:
        for mode in ('checked', 'release'):
            lib = library(Path(temp), mode)
            for message, label, expected in values:
                assert digest(lib.ripemd160_hash_checked, message) == expected, (mode, label)
            for parent, fingerprint, path in pairs:
                assert digest(lib.hash160_checked, parent)[:4] == fingerprint, (mode, path)
            first = pairs[0]
            assert first[1].hex() == '3442193e' and first[2] == 'm/0<sub>H</sub>', first
            checked = 0
            if oracle:
                for n in range(301):
                    data = bytes((i * 7 + n) % 256 for i in range(n))
                    assert digest(lib.ripemd160_hash_checked, data) == oracle(data), (mode, n)
                    assert digest(lib.hash160_checked, data) == oracle(hashlib.sha256(data).digest()), (mode, n)
                    checked += 1
                for _ in range(20):
                    data = rng.randbytes(rng.randrange(1, 1 << 20))
                    assert digest(lib.ripemd160_hash_checked, data) == oracle(data), (mode, len(data))
                    checked += 1
            print(f'{mode}: {len(values)} paper values, {len(pairs)} BIP-32 fingerprints, '
                  f'{checked} hashlib comparisons{"" if oracle else " (hashlib has no ripemd160 here)"}')


if __name__ == '__main__':
    main()
