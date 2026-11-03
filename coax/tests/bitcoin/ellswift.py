#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: BIP-324's own Python reference decides each case outside Coax)
"""The cross-check of Bitcoin chapter 66. It computes every line the
chapter's example prints with BIP-324's reference implementation
(reference.py, pinned in coax/tests/vectors/upstream/bip324/), which
shares no code with Coax, and compares the transcript with the fixture:

  python3 -I coax/tests/bitcoin/ellswift.py                     # compare with the fixture
  python3 -I coax/tests/bitcoin/ellswift.py --coax coax/bin/coax   # also run the example

The decoder, the eight inverse cases and the shared secret are
reference.py's xswiftec, xswiftec_inv and v2_ecdh. The encoder's draws
(u from SHA-256(rnd || i || 0), the case from SHA-256(rnd || i || 1)) are
the chapter's own choice, transcribed here from its text.
"""
import hashlib
import importlib.util
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import make_ellswift_vectors as V  # noqa: E402

REPO = HERE.parents[2]
REFERENCE = HERE.parent / 'vectors/upstream/bip324/reference.py'
REFERENCE_PIN = 'cb074b4e64bcb096ff4a2f1ee3a0580c383bb1d02662b7dccdd4968bc6b7af4f'
FIXTURE = REPO / 'bitcoin/fixtures/ellswift.out'
EXAMPLE = 'bitcoin/examples/ellswift.coax'


def load_reference():
    sys.dont_write_bytecode = True  # keep the pinned directory clean
    assert hashlib.sha256(REFERENCE.read_bytes()).hexdigest() == REFERENCE_PIN, 'reference.py differs from its pin'
    spec = importlib.util.spec_from_file_location('bip324_reference', REFERENCE)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def encode(R, x, rnd):
    """The chapter's encoder: attempt i draws u from SHA-256(rnd || i || 0)
    and the case from SHA-256(rnd || i || 1)."""
    for attempt in range(1024):
        seed = rnd + attempt.to_bytes(4, 'little')
        u = R.FE(int.from_bytes(hashlib.sha256(seed + b'\x00').digest(), 'big'))
        case = hashlib.sha256(seed + b'\x01').digest()[0] & 7
        t = R.xswiftec_inv(x, u, case)
        if t is not None and u != 0:
            return u.to_bytes() + t.to_bytes()
    raise AssertionError('no encoding')


def transcript():
    R = load_reference()
    FE = R.FE
    lines = []
    out = lines.append

    def fe(h):
        return FE(int(h, 16))

    decode = V.rows('ellswift_decode_test_vectors.csv')
    good = sum(1 for r in decode if R.ellswift_decode(bytes.fromhex(r['ellswift'])).hex() == r['x'])
    out(f'decode: {good} of {len(decode)} BIP-324 encodings give their x')
    for k in (0, 1, 75):
        e = bytes.fromhex(decode[k]['ellswift'])
        out(f'  row {k}: u {e[:4].hex()}..., t {e[32:36].hex()}... -> x {R.ellswift_decode(e).hex()}')

    inverse = V.rows('xswiftec_inv_test_vectors.csv')
    found, refused, agree, back = [0] * 8, [0] * 8, 0, 0
    for r in inverse:
        x, u = fe(r['x']), fe(r['u'])
        for case in range(8):
            t = R.xswiftec_inv(x, u, case)
            want = r[f'case{case}_t']
            if t is None:
                refused[case] += 1
                agree += want == ''
            else:
                found[case] += 1
                agree += t.to_bytes().hex() == want
                back += R.xswiftec(u, t) == x
    out(f'inverse: {len(inverse)} rows, {len(inverse) * 8} cases; {agree} agree with BIP-324, '
        f'and all {back} t decode back to x')
    for case in range(8):
        out(f'  case {case}: {found[case]} found, {refused[case]} refused')
    x, u = fe(inverse[0]['x']), fe(inverse[0]['u'])
    out(f'  row 0, x {x.to_bytes().hex()}')
    for case in range(8):
        t = R.xswiftec_inv(x, u, case)
        out(f'    case {case}: {"none" if t is None else t.to_bytes().hex()}')

    packets = V.rows('packet_encoding_test_vectors.csv')
    match = 0
    for r in packets:
        priv = bytes.fromhex(r['in_priv_ours'])
        ours, theirs = bytes.fromhex(r['in_ellswift_ours']), bytes.fromhex(r['in_ellswift_theirs'])
        initiating = r['in_initiating'] == '1'
        ok = (R.ellswift_ecdh_xonly(theirs, priv).hex() == r['mid_x_shared']
              and R.v2_ecdh(priv, theirs, ours, initiating).hex() == r['mid_shared_secret']
              and R.ellswift_decode(ours).hex() == r['mid_x_ours']
              and R.ellswift_decode(theirs).hex() == r['mid_x_theirs'])
        match += ok
    out(f'exchange: {match} of {len(packets)} rows give their x, shared x and shared secret')

    a, b = (1).to_bytes(32, 'big'), (2).to_bytes(32, 'big')
    ea = encode(R, (1 * R.SECP256K1_G).x, bytes(range(32)))
    eb = encode(R, (2 * R.SECP256K1_G).x, bytes(range(32, 64)))
    out(f'initiator, key 1: {ea.hex()}')
    out(f'  decodes to {R.ellswift_decode(ea).hex()} (the generator\'s x)')
    out(f'responder, key 2: {eb.hex()}')
    out(f'  decodes to {R.ellswift_decode(eb).hex()}')
    sa = R.v2_ecdh(a, eb, ea, True)
    sb = R.v2_ecdh(b, ea, eb, False)
    out(f'initiator\'s secret: {sa.hex()}')
    out(f'responder\'s secret: {sb.hex()}')
    out(f'  the same: {"true" if sa == sb else "false"}')
    out(f'  with the roles swapped on one side: {R.v2_ecdh(a, eb, ea, False).hex()}')
    return '\n'.join(lines) + '\n'


def main():
    want = transcript()
    if '--write' in sys.argv:
        FIXTURE.write_text(want)
    assert FIXTURE.read_text() == want, 'the fixture differs from the Python transcript'
    print(f'ellswift fixture: {want.count(chr(10))} lines agree with BIP-324\'s reference.py')
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            run = subprocess.run([coax, 'run', EXAMPLE] + mode, cwd=REPO, capture_output=True, text=True)
            assert run.returncode == 0 and run.stdout == want, f'native {mode or "debug"} output differs'
            print(f'native {"release" if mode else "debug"}: agrees')


if __name__ == '__main__':
    main()
