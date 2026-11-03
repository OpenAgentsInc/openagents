#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: hashlib, the csv module and BIP-340's reference.py decide each line outside Coax)
"""The cross-check of Bitcoin chapter 116, test vectors and their provenance.

It reads the pinned file itself (not the generated table), hashes it with
hashlib, parses it with the csv module, verifies each row with BIP-340's
own reference.py (pinned beside it), and compares the transcript with the
fixture. It also checks that the generated table holds the same bytes:

  python3 -I coax/tests/bitcoin/provenance.py                       # compare with the fixture
  python3 -I coax/tests/bitcoin/provenance.py --coax coax/bin/coax  # also run the example
  python3 -I coax/tests/bitcoin/provenance.py --write               # rewrite the fixture
"""
import csv
import hashlib
import io
import json
import re
import subprocess
import sys
import types
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
VECTORS = REPO / 'coax/tests/vectors'
PATH = 'upstream/bip340/test-vectors.csv'
REF = VECTORS / 'upstream/bip340/reference.py'
REF_PIN = '4b1d4ad9e60820df4a6d239733d52b014bc03e4c47afd757a2fe4d3b586d892c'
TABLE = HERE / 'provenance_vectors.coax'
FIXTURE = REPO / 'bitcoin/fixtures/provenance.out'
EXAMPLE = REPO / 'bitcoin/examples/provenance.coax'


def reference():
    raw = REF.read_bytes()
    assert hashlib.sha256(raw).hexdigest() == REF_PIN, 'bip340/reference.py differs from its pin'
    module = types.ModuleType('bip340_reference')
    exec(compile(raw, str(REF), 'exec'), module.__dict__)
    return module


def rows(data):
    """The data rows, parsed by the csv module from the decoded text."""
    table = list(csv.reader(io.StringIO(data.decode('ascii'), newline='')))
    return table[0], table[1:]


def transcript():
    R = reference()
    pin = next(e for e in json.loads((VECTORS / 'manifest.json').read_text())['files'] if e['path'] == PATH)
    data = (VECTORS / PATH).read_bytes()
    table = TABLE.read_text()
    assert ''.join(re.findall(r'^  "([0-9a-f]+)"', table, re.M)) == data.hex(), 'the generated table differs from the file'
    sha = hashlib.sha256(data).hexdigest()
    matches = lambda b: len(b) == pin['bytes'] and hashlib.sha256(b).hexdigest() == pin['sha256']
    out = ['pinned file: ' + PATH, 'revision: ' + pin['revision'],
           'bytes: %d, the pin says %d' % (len(data), pin['bytes']),
           'SHA-256: ' + sha, 'the pin: ' + pin['sha256'],
           'matches the pin: %s' % str(matches(data)).lower()]
    header, body = rows(data)
    out.append('columns: ' + ','.join(header))
    agree = 0
    for r in body:
        expected = r[6] == 'TRUE'
        try:
            got = R.schnorr_verify(bytes.fromhex(r[4]), bytes.fromhex(r[2]), bytes.fromhex(r[5]))
        except ValueError:
            got = False
        out.append('row %s: expected %s, verifies %s%s' % (r[0], str(expected).lower(), str(got).lower(),
                                                          ': ' + r[7] if r[7] else ''))
        agree += got == expected
    out.append('%d of %d rows give their published result' % (agree, len(body)))
    # One edited byte: the first hex digit of row 0's signature.
    lines = data.split(b'\n')
    at = len(lines[0]) + 1 + sum(len(f) + 1 for f in lines[1].split(b',')[:5])
    edited = bytearray(data)
    out.append("edit byte %d, row 0's signature, %s to F" % (at, chr(edited[at])))
    edited[at] = ord('F')
    out += ['SHA-256: ' + hashlib.sha256(edited).hexdigest(), 'matches the pin: %s' % str(matches(edited)).lower()]
    r0 = rows(bytes(edited))[1][0]
    got = R.schnorr_verify(bytes.fromhex(r0[4]), bytes.fromhex(r0[2]), bytes.fromhex(r0[5]))
    out.append('unpinned, row 0 would read: expected %s, verifies %s' % (r0[6], str(got).lower()))
    unix = data.replace(b'\r', b'')
    out.append('without its %d carriage returns: %d bytes, matches the pin %s'
               % (len(data) - len(unix), len(unix), str(matches(unix)).lower()))
    return out


def main():
    lines = transcript()
    if '--write' in sys.argv:
        FIXTURE.write_text('\n'.join(lines) + '\n')
    want = FIXTURE.read_text().splitlines()
    if lines != want:
        for a, b in zip(lines, want):
            if a != b:
                print('python: ' + a + '\nfixture: ' + b)
                break
        sys.exit('provenance.py: the transcript differs from ' + FIXTURE.name)
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            got = subprocess.run([coax, 'run', *mode, str(EXAMPLE)], capture_output=True, text=True, check=True).stdout
            if got.splitlines() != lines:
                sys.exit('provenance.py: the example differs in ' + (mode[0] if mode else 'debug'))
    print('provenance.py: %d lines agree' % len(lines))


if __name__ == '__main__':
    main()
