#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: BOLT 8's text and a separate Python Noise handshake decide each case outside Coax)
"""The cross-check of Bitcoin chapter 105. It computes every line the
chapter's example prints and compares the transcript with the fixture:

  python3 -I coax/tests/lightning/noise.py                      # compare with the fixture
  python3 -I coax/tests/lightning/noise.py --coax coax/bin/coax   # also run the example

The handshake and transport are this file's own Python, written from BOLT
8's "Handshake Exchange" and "Lightning Message Specification" on
ln_crypto.py's secp256k1, ChaCha20-Poly1305 and HKDF, sharing no code with
Coax. Every handshake vector must give BOLT 8's outputs, keys or error,
and the message vector its six outputs, before the transcript is compared.
"""
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import bolts  # noqa: E402
import ln_crypto as C  # noqa: E402

REPO = HERE.parents[2]
FIXTURE = REPO / 'bitcoin/fixtures/lightning-noise.out'
EXAMPLE = 'bitcoin/examples/lightning-noise.coax'


class Refused(Exception):
    pass


def nonce(n):
    return bytes(4) + n.to_bytes(8, 'little')


def ecdh(k, point):
    return C.sha256(C.compress(C.mul(k, point)))


def pub(k):
    return C.compress(C.mul(k))


class Side:
    """One side of a BOLT 8 handshake."""

    def __init__(self, initiator, s, responder_pub):
        self.initiator, self.s = initiator, s
        self.h = C.sha256(b'Noise_XK_secp256k1_ChaChaPoly_SHA256')
        self.ck = self.h
        self.h = C.sha256(self.h, b'lightning')
        self.h = C.sha256(self.h, responder_pub)
        self.rs = responder_pub if initiator else None

    def mix(self, ss):
        out = C.hkdf(self.ck, ss)
        self.ck, self.temp = out[:32], out[32:]

    def write_e(self, e):
        self.e = e
        epub = pub(e)
        self.h = C.sha256(self.h, epub)
        self.mix(ecdh(e, C.decompress(self.rs) if self.initiator else self.re))
        c = C.seal(self.temp, nonce(0), self.h, b'')
        self.h = C.sha256(self.h, c)
        return b'\0' + epub + c

    def read_e(self, m, secret):
        if len(m) != 50:
            raise Refused('READ_FAILED')
        if m[0] != 0:
            raise Refused('BAD_VERSION')
        re = C.decompress(m[1:34])
        if re is None:
            raise Refused('BAD_PUBKEY')
        self.re = re
        self.h = C.sha256(self.h, m[1:34])
        self.mix(ecdh(secret, re))
        if C.open_(self.temp, nonce(0), self.h, m[34:]) is None:
            raise Refused('BAD_TAG')
        self.h = C.sha256(self.h, m[34:])

    def act_one(self, e):
        return self.write_e(e)

    def read_one(self, m):
        self.read_e(m, self.s)

    def act_two(self, e):
        return self.write_e(e)

    def read_two(self, m):
        self.read_e(m, self.e)

    def split(self):
        out = C.hkdf(self.ck, b'')
        first, second = out[:32], out[32:]
        sk, rk = (first, second) if self.initiator else (second, first)
        return Transport(sk, rk, self.ck)

    def act_three(self):
        c = C.seal(self.temp, nonce(1), self.h, pub(self.s))
        self.h = C.sha256(self.h, c)
        self.mix(ecdh(self.s, self.re))
        t = C.seal(self.temp, nonce(0), self.h, b'')
        return b'\0' + c + t, self.split()

    def read_three(self, m):
        if len(m) != 66:
            raise Refused('READ_FAILED')
        if m[0] != 0:
            raise Refused('BAD_VERSION')
        rs = C.open_(self.temp, nonce(1), self.h, m[1:50])
        if rs is None:
            raise Refused('BAD_CIPHERTEXT')
        point = C.decompress(rs)
        if point is None:
            raise Refused('BAD_PUBKEY')
        self.rs = rs
        self.h = C.sha256(self.h, m[1:50])
        self.mix(ecdh(self.e, point))
        if C.open_(self.temp, nonce(0), self.h, m[50:]) is None:
            raise Refused('BAD_TAG')
        return self.split()


class Transport:
    def __init__(self, sk, rk, ck):
        self.sk, self.rk, self.sck, self.rck, self.sn, self.rn = sk, rk, ck, ck, 0, 0

    def _used(self, send):
        if send:
            self.sn += 1
            if self.sn == 1000:
                out = C.hkdf(self.sck, self.sk)
                self.sck, self.sk, self.sn = out[:32], out[32:], 0
        else:
            self.rn += 1
            if self.rn == 1000:
                out = C.hkdf(self.rck, self.rk)
                self.rck, self.rk, self.rn = out[:32], out[32:], 0

    def encrypt(self, m):
        lc = C.seal(self.sk, nonce(self.sn), b'', len(m).to_bytes(2, 'big'))
        self._used(True)
        c = C.seal(self.sk, nonce(self.sn), b'', m)
        self._used(True)
        return lc + c

    def decrypt_length(self, header):
        got = C.open_(self.rk, nonce(self.rn), b'', header)
        self._used(False)
        return None if got is None else int.from_bytes(got, 'big')

    def decrypt_body(self, body):
        got = C.open_(self.rk, nonce(self.rn), b'', body)
        self._used(False)
        return got


def run_vector(t):
    """BOLT 8's verdict for one vector from its side: (error, keys)."""
    s, e = int(t['ls_priv'], 16), int(t['e_priv'], 16)
    try:
        if t['initiator']:
            side = Side(True, s, bytes.fromhex(t['rs_pub']))
            assert side.act_one(e).hex() == t['act1']
            side.read_two(bytes.fromhex(t['act2']))
            three, tr = side.act_three()
            assert three.hex() == t['act3']
            return 'OK', tr.sk + tr.rk
        side = Side(False, s, pub(s))
        side.read_one(bytes.fromhex(t['act1']))
        assert side.act_two(e).hex() == t['act2']
        tr = side.read_three(bytes.fromhex(t['act3']))
        return 'OK', tr.rk + tr.sk
    except Refused as r:
        return str(r), b''


def transcript():
    out = ['handshake vectors']
    for t in bolts.handshakes():
        code, keys = run_vector(t)
        assert (code == 'OK' and keys.hex() == t['keys']) or code == t['error'], t['name']
        out.append(f'  {t["name"]}: {code}, ' + (f'session keys match: {"true" if keys.hex() == t["keys"] else "false"}'
                                                 if code == 'OK' else f'BOLT 8 says {t["error"]}'))

    out.append('the initiator, step by step')
    t = bolts.handshakes()[0]
    side = Side(True, int(t['ls_priv'], 16), bytes.fromhex(t['rs_pub']))
    out.append(f'  h before act one: {side.h.hex()}')
    one = side.act_one(int(t['e_priv'], 16))
    out.append(f'  act one, 50 bytes: {one.hex()}')
    out.append(f'  h after act one: {side.h.hex()}')
    side.read_two(bytes.fromhex(t['act2']))
    out.append(f'  act two read: OK, ck: {side.ck.hex()}')
    three, tr = side.act_three()
    out.append(f'  act three, 66 bytes: {three.hex()}')
    out.append(f'  sk: {tr.sk.hex()}')
    out.append(f'  rk: {tr.rk.hex()}')

    out.append('message vectors')
    keys, outputs = bolts.messages()
    tm = Transport(bytes.fromhex(keys['sk']), bytes.fromhex(keys['rk']), bytes.fromhex(keys['ck']))
    want = dict(outputs)
    for i in range(1002):
        box = tm.encrypt(b'hello')
        if i in want:
            assert box.hex() == want[i], i
            out.append(f'  message {i}, {len(box)} bytes: true, nonce now {tm.sn}')

    out.append('two nodes')
    a = Side(True, 7, pub(9))
    b = Side(False, 9, pub(9))
    out.append(f'  responder\'s static key: {pub(9).hex()}')
    one = a.act_one(11)
    b.read_one(one)
    two = b.act_two(13)
    a.read_two(two)
    three, ta = a.act_three()
    tb = b.read_three(three)
    out.append('  acts read: OK, OK, OK')
    out.append(f'  the responder learned the initiator\'s static key: {"true" if b.rs == pub(7) else "false"}')
    out.append(f'  initiator sk = responder rk: {"true" if ta.sk == tb.rk else "false"}')
    c = Side(False, 5, pub(5))
    try:
        c.read_one(one)
        verdict = 'OK'
    except Refused as r:
        verdict = str(r)
    out.append(f'  act one read by a node with another key: {verdict}')
    init = bytes.fromhex('0010' '0000' '0002' '4100')   # chapter 104's init with bits 8 and 14
    wire = ta.encrypt(init)
    out.append(f'  init: {len(init)} bytes, {len(wire)} on the wire: {wire.hex()}')
    n = tb.decrypt_length(wire[:18])
    body = tb.decrypt_body(wire[18:])
    assert body == init
    out.append(f'  the responder reads length {n}, body OK, init ok, features 2')
    again = bytearray(ta.encrypt(init))
    again[18 + 3] ^= 1
    probe = Transport(tb.sk, tb.rk, tb.sck)
    probe.rn, probe.rck = tb.rn, tb.rck
    length = probe.decrypt_length(bytes(again[:18]))
    body = probe.decrypt_body(bytes(again[18:]))
    out.append(f'  a changed body byte: length {"OK" if length is not None else "BAD_TAG"}, body {"OK" if body else "BAD_TAG"}')
    out.append(f'  the first message replayed: {"OK" if tb.decrypt_length(wire[:18]) is not None else "BAD_TAG"}')
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
