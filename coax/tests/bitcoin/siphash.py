#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: the reference vectors, Bitcoin Core's test text and a separate Python SipHash check the Coax hash)
"""SipHash-2-4 against its published vectors, natively and in Wasm (Bitcoin chapter 10).

Builds coax/lib/crypto/siphash.coax as a C library in checked and release
modes and checks, in both:

- the 64 tags of `vectors_sip64` in the pinned reference implementation's
  coax/tests/vectors/upstream/siphash/vectors.h, which must equal the
  `siphash_4_2_testvec` table in the pinned Bitcoin Core hash_tests.cpp;
- Bitcoin Core's one Bitcoin case: k0 = 1, k1 = 2 over the SHA-256d of an
  empty version-1 transaction;
- 2,000 random keys and inputs of 0 to 300 bytes against the separate
  Python SipHash below, written from the paper;
- the refusals, which leave the output unchanged.

Then it compiles the same C to wasm32 and runs the vectors and 500 oracle
cases in Node. Run from the repository root: python3 coax/tests/bitcoin/siphash.py
"""
if not __debug__: raise SystemExit('tests require Python assertions')
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

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
sys.path.insert(0, str(ROOT/'coax/tests/nostr'))
from wasm_toolchain import WASM_CLANG, WASM_SYSROOT, WASM_RESOURCES  # noqa: E402

COMPILER = Path(os.environ.get('COAX_TEST_COMPILER', ROOT/'coax/bin/coax'))
SOURCE = ROOT/'coax/lib/crypto/siphash.coax'
VECTORS = ROOT/'coax/tests/vectors/upstream'
MASK = (1 << 64) - 1


def oracle(key, data):
    """SipHash-2-4 from the paper, independent of the Coax code."""
    rotl = lambda x, b: ((x << b) | (x >> (64 - b))) & MASK
    k0, k1 = int.from_bytes(key[:8], 'little'), int.from_bytes(key[8:], 'little')
    v = [k0 ^ 0x736f6d6570736575, k1 ^ 0x646f72616e646f6d, k0 ^ 0x6c7967656e657261, k1 ^ 0x7465646279746573]

    def rounds(n):
        for _ in range(n):
            v[0] = (v[0] + v[1]) & MASK; v[1] = rotl(v[1], 13) ^ v[0]; v[0] = rotl(v[0], 32)
            v[2] = (v[2] + v[3]) & MASK; v[3] = rotl(v[3], 16) ^ v[2]
            v[0] = (v[0] + v[3]) & MASK; v[3] = rotl(v[3], 21) ^ v[0]
            v[2] = (v[2] + v[1]) & MASK; v[1] = rotl(v[1], 17) ^ v[2]; v[2] = rotl(v[2], 32)
    padded = data + b'\0' * (7 - len(data) % 8) + bytes([len(data) & 255])
    for i in range(0, len(padded), 8):
        m = int.from_bytes(padded[i:i + 8], 'little')
        v[3] ^= m; rounds(2); v[0] ^= m
    v[2] ^= 0xff; rounds(4)
    return (v[0] ^ v[1] ^ v[2] ^ v[3]).to_bytes(8, 'little')


def reference_vectors():
    text = (VECTORS/'siphash/vectors.h').read_text()
    table = re.search(r'vectors_sip64\[64\]\[8\] = \{(.*?)\n\};', text, re.S).group(1)
    tags = [bytes(int(b, 16) for b in re.findall(r'0x([0-9a-f]{2})', row)) for row in re.findall(r'\{([^{}]*)\}', table)]
    assert len(tags) == 64 and all(len(t) == 8 for t in tags)
    core = (VECTORS/'bitcoin-core/hash_tests.cpp').read_text()
    words = re.search(r'siphash_4_2_testvec\[\] = \{(.*?)\};', core, re.S).group(1)
    core_tags = [int(w, 16).to_bytes(8, 'little') for w in re.findall(r'0x([0-9a-f]{16})', words)]
    assert core_tags == tags, 'vectors.h and hash_tests.cpp disagree'
    # Bitcoin Core: PresaltedSipHasher(1, 2)(hash of an empty version-1 tx).
    assert 'PresaltedSipHasher(1, 2)(ss.GetHash()), 0x79751e980c2a0a35ULL' in core
    tx = hashlib.sha256(hashlib.sha256(bytes.fromhex('01000000' '00' '00' '00000000')).digest()).digest()
    bitcoin = ((1).to_bytes(8, 'little') + (2).to_bytes(8, 'little'), tx, (0x79751e980c2a0a35).to_bytes(8, 'little'))
    return [(bytes(range(16)), bytes(range(n)), tags[n]) for n in range(64)] + [bitcoin]


class View(C.Structure):
    _fields_ = [('data', C.POINTER(C.c_uint8)), ('count', C.c_int64)]


def view(buffer, count=None):
    return View(C.cast(buffer, C.POINTER(C.c_uint8)), len(buffer) if count is None else count)


def run(*args):
    r = subprocess.run([str(a) for a in args], capture_output=True, text=True)
    assert r.returncode == 0, f'{args[0]} failed: {r.stderr or r.stdout}'
    return r.stdout


def tag(lib, key, data, out_len=9):
    k = (C.c_uint8 * 16).from_buffer_copy(key)
    src = (C.c_uint8 * max(len(data), 1)).from_buffer_copy(data or b'\0')
    out = (C.c_uint8 * out_len)(*([165] * out_len))
    status = lib.siphash24_checked(view(k), view(src, len(data)), view(out), False)
    assert status == 0, status
    assert bytes(out)[8:] == b'\xa5' * (out_len - 8), 'wrote past the tag'
    return bytes(out)[:8]


def refusals(lib):
    key = (C.c_uint8 * 17)(); src = (C.c_uint8 * 4)(); out = (C.c_uint8 * 8)(*([165] * 8))
    f = lib.siphash24_checked
    assert f(view(key, 15), view(src), view(out), False) == 1
    assert f(view(key, 17), view(src), view(out), False) == 1
    assert f(view(key, 16), view(src, -1), view(out), False) == 2
    assert f(view(key, 16), view(src), view(out, 7), False) == 3
    assert f(view(key, 16), view(src), view(out), True) == 4
    assert bytes(out) == b'\xa5' * 8


def main():
    vectors = reference_vectors()
    rng = random.Random(10)
    cases = [(rng.randbytes(16), rng.randbytes(rng.randrange(301))) for _ in range(2000)]
    with tempfile.TemporaryDirectory(prefix='coax-siphash-') as temp:
        d = Path(temp)
        for mode in ('checked', 'release'):
            c = d/f'{mode}.c'
            c.write_text(run(COMPILER, 'c', SOURCE, '--library', '--header', d/'siphash.h', *(['--release'] if mode == 'release' else [])))
            run('cc', '-std=c11', '-O2', '-shared', '-fPIC', c, '-lm', '-o', d/f'{mode}.so')
            lib = C.CDLL(str(d/f'{mode}.so'))
            lib.siphash24_checked.argtypes = [View, View, View, C.c_bool]; lib.siphash24_checked.restype = C.c_uint32
            for key, data, expected in vectors:
                assert tag(lib, key, data) == expected, (mode, data.hex())
            for key, data in cases:
                assert tag(lib, key, data) == oracle(key, data), (mode, key.hex(), data.hex())
            refusals(lib)
            print(f'{mode}: 64 reference vectors, 1 Bitcoin Core case, {len(cases)} oracle comparisons, 5 refusals')
        fixture = d/'wasm.json'
        fixture.write_text(json.dumps([{'key': k.hex(), 'input': m.hex(), 'tag': t.hex()} for k, m, t in vectors]
                                      + [{'key': k.hex(), 'input': m.hex(), 'tag': oracle(k, m).hex()} for k, m in cases[:500]]))
        for mode in ('checked', 'release'):
            module = d/f'{mode}.wasm'
            run(WASM_CLANG, '--target=wasm32-wasip1', '--sysroot=' + str(WASM_SYSROOT), '-resource-dir=' + str(WASM_RESOURCES),
                '-std=c11', '-O2', '-I' + str(d), '-mexec-model=reactor', d/f'{mode}.c', HERE/'siphash_wasm.c',
                '-Wl,--import-memory,--export-memory,--export=__heap_base,--max-memory=67108864,-z,stack-size=1048576,--gc-sections',
                '-o', module)
            result = json.loads(run('node', HERE/'siphash_wasm.cjs', module, fixture))
            assert result['cases'] == 565, result
            print(f'wasm {mode}: {result["cases"]} cases in {result["engine"]}')


if __name__ == '__main__':
    main()
