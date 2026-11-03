#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: BOLT 3's text and a separate Python shachain decide each case outside Coax)
"""The cross-check of Bitcoin chapter 107. It computes every line the
chapter's example prints and compares the transcript with the fixture:

  python3 -I coax/tests/lightning/shachain.py                      # compare with the fixture
  python3 -I coax/tests/lightning/shachain.py --coax coax/bin/coax   # also run the example

The generator and the 49-slot store are this file's own Python, written
from BOLT 3's "Per-commitment Secret Requirements" and "Efficient
Per-commitment Secret Storage" on hashlib, sharing no code with Coax.
Every generation vector must give BOLT 3's secret, and every storage
vector must refuse at the insert BOLT 3 marks ERROR, before the
transcript is compared.
"""
import hashlib
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import bolts  # noqa: E402

REPO = HERE.parents[2]
FIXTURE = REPO / 'bitcoin/fixtures/lightning-shachain.out'
EXAMPLE = 'bitcoin/examples/lightning-shachain.coax'
MAX = (1 << 48) - 1
COMMITMENTS = 10000


def derive(base, bits, index):
    p = bytearray(base)
    for b in reversed(range(bits)):
        if index >> b & 1:
            p[b // 8] ^= 1 << (b % 8)
            p = bytearray(hashlib.sha256(p).digest())
    return bytes(p)


def generate(seed, index):
    return derive(seed, 48, index)


def where(index):
    return next((b for b in range(48) if index >> b & 1), 48)


class Store:
    def __init__(self):
        self.known = {}          # slot -> (index, secret)
        self.next = MAX

    def insert(self, index, secret):
        if index != self.next or (self.known and min(i for i, _ in self.known.values()) == 0):
            return 'ORDER'
        slot = where(index)
        for b in range(slot):
            i, s = self.known[b]
            if derive(secret, slot, i) != s:
                return 'WRONG_SECRET'
        self.known[slot] = (index, secret)
        self.next = index - 1
        return 'OK'

    def lookup(self, index):
        for b, (i, s) in sorted(self.known.items()):
            if self.next < index <= MAX and index >> b == i >> b:
                return 'OK', derive(s, b, index)
        return 'UNKNOWN', None


def transcript():
    out = ['generation vectors']
    for name, seed, index, want in bolts.generation():
        got = generate(bytes.fromhex(seed), index)
        assert got.hex() == want, name
        out.append(f'  {name}: index {index}, matches: true')

    out.append('index 1, step by step')
    seed = bytearray(bytes.fromhex(bolts.generation()[4][1]))
    seed[0] ^= 1
    out.append(f'  flip bit 0: {seed.hex()}')
    out.append(f'  SHA-256:    {hashlib.sha256(seed).hexdigest()}')

    out.append('storage vectors')
    for name, steps in bolts.storage():
        store, got = Store(), -1
        for k, (index, secret, _) in enumerate(steps):
            if store.insert(index, bytes.fromhex(secret)) != 'OK':
                got = k
                break
        want = next((k for k, (_, _, ok) in enumerate(steps) if not ok), -1)
        assert got == want, name
        out.append(f'  {name}: every insert OK, BOLT 3 agrees: true' if got < 0 else
                   f'  {name}: insert {got + 1} refused, BOLT 3 says insert {want + 1}')

    out.append(f'a channel of {COMMITMENTS} commitments')
    seed = bytes((7 * i + 1) & 255 for i in range(32))
    store = Store()
    refused = sum(store.insert(MAX - n, generate(seed, MAX - n)) != 'OK' for n in range(COMMITMENTS))
    out.append(f'  received {COMMITMENTS - refused}, refused {refused}, slots in use {len(store.known)}, next index {store.next}')
    for n in (0, 1234, 5000, 9999):
        code, secret = store.lookup(MAX - n)
        out.append(f'  commitment {n}: lookup {code}, equals the sender\'s: {"true" if secret == generate(seed, MAX - n) else "false"}')
    out.append(f'  commitment {COMMITMENTS}: {store.lookup(MAX - COMMITMENTS)[0]}')

    out.append('a peer that sends a wrong secret')
    copy = Store()
    copy.known, copy.next = dict(store.known), store.next
    first, second = MAX - COMMITMENTS, MAX - COMMITMENTS - 1
    out.append(f'  commitment {COMMITMENTS}, index {first} in slot {where(first)}: '
               f'{copy.insert(first, generate(bytes([0xab] * 32), first))}')
    out.append(f'  commitment {COMMITMENTS + 1}, index {second} in slot {where(second)}: '
               f'{copy.insert(second, generate(seed, second))}')
    copy.known, copy.next = dict(store.known), store.next
    out.append(f'  commitment {COMMITMENTS} skipped: {copy.insert(second, generate(seed, second))}')
    out.append(f'  storage: {49 * 40} bytes of slots for {COMMITMENTS - refused} secrets of 32 bytes')
    return out


def main():
    lines = transcript()
    want = FIXTURE.read_text().splitlines()
    assert lines == want, '\n'.join(f'{a!r} != {b!r}' for a, b in zip(lines, want) if a != b) or 'line counts differ'
    print(f'{len(lines)} lines agree with {FIXTURE.name}')
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            got = subprocess.run([coax, 'run', EXAMPLE, *mode], cwd=REPO, capture_output=True, text=True, check=True)
            assert got.stdout.splitlines() == lines, f'coax run {" ".join(mode)} differs'
            print(f'coax run {" ".join(mode) or "(debug)"}: {len(lines)} lines agree')


if __name__ == '__main__':
    main()
