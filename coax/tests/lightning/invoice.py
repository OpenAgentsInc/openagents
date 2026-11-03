#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: BOLT 11's vectors and a separate Python reader and writer decide each case outside Coax)
"""The cross-check of Bitcoin chapter 114. It runs every BOLT 11 vector on
bolt11.py, computes every line the chapter's example prints, and compares
the transcript with the fixture:

  python3 -I coax/tests/lightning/invoice.py                        # compare with the fixture
  python3 -I coax/tests/lightning/invoice.py --coax coax/bin/coax   # also run the example

bolt11.py is this cross-check's own Bech32, invoice reader and writer and
recoverable ECDSA with RFC 6979 nonces, written from BOLT 11 and BIP-173
on ln_crypto.py's secp256k1, sharing no code with Coax. Every example in
bolt11/invoice-test.json must be valid or invalid as stated, and the
signature, recovery flag, signed data and digest of every "Signature
breakdown" in 11-payment-encoding.md must match, before the transcript is
compared.
"""
import hashlib
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import bolt11 as B  # noqa: E402
import bolts  # noqa: E402
import ln_crypto as C  # noqa: E402

REPO = HERE.parents[2]
FIXTURE = REPO / 'bitcoin/fixtures/lightning-invoice.out'
EXAMPLE = 'bitcoin/examples/lightning-invoice.coax'
RUN_STEPS = '250000000'
h = bytes.fromhex
LETTER = {v: k for k, v in B.TYPES.items()}


def pub(k):
    return C.compress(C.mul(k))


def code(text):
    try:
        B.decode(text)
        return 'OK'
    except B.Refused as e:
        return str(e)


# ---- BOLT 11's vectors

def check_vectors():
    rows = bolts.bolt11_json()
    key, secret, stated = bolts.bolt11_examples()
    payee = pub(int(key, 16))
    for i, row in enumerate(rows):
        assert (code(row['invoice']) == 'OK') == row['valid'], row['description']
        if not row['valid']:
            continue
        inv = B.decode(row['invoice'])
        assert inv['s'].hex() == secret
        s = stated[row['invoice']]
        if s['timestamp']:
            assert inv['timestamp'] == s['timestamp']
        if s['data']:
            assert inv['message'].hex() == s['data'] and inv['digest'].hex() == s['digest']
            assert inv['signature'].hex() == s['signature'] and inv['recovery'] == s['recovery']
        high_s = int.from_bytes(inv['signature'][32:], 'big') > C.N // 2
        assert (inv['payee'] == payee) == (not high_s)
        hrp = row['invoice'].lower()[:row['invoice'].lower().rfind('1')]
        again = B.encode(hrp, inv['timestamp'], [(t, v) for _, t, v in inv['fields']], int(key, 16))
        assert (again == row['invoice'].lower()) == (not high_s)
    return rows, key


# ---- the example's transcript

def report(inv, text):
    hrp = text[:text.rfind('1')]
    out = [f"  {inv['network']}, " + (f"{inv['amount']} msat" if inv['amount'] is not None else 'any amount')
           + f", timestamp {inv['timestamp']}"]
    fallback = hint = 0
    for name, t, v in inv['fields']:
        line = f'  {B.CHARSET[t]}: {len(v)} words'
        b = B.to_bytes(v)
        if name == 'p':
            line += f', payment_hash {b.hex()}'
        elif name == 's':
            line += f', payment_secret {b.hex()}'
        elif name == 'd':
            line += f', description "{b.decode()}"'
        elif name == 'h':
            line += f', description_hash {b.hex()}'
        elif name == 'n':
            line += f', payee {b.hex()}'
        elif name == 'x':
            line += f", expiry {B.word_int(v)} s"
        elif name == 'c':
            line += f", min_final_cltv_expiry_delta {B.word_int(v)}"
        elif name == 'm':
            line += f', payment_metadata {b.hex()}'
        elif name == '9':
            f = B.word_int(v)
            line += ', features' + ''.join(f' {bit}' for bit in range(f.bit_length()) if f >> bit & 1)
        elif name == 'f':
            line += f', fallback version {v[0]}'
            if v[0] <= 18:
                line += ' ' + inv['fallbacks'][fallback]
                fallback += 1
        elif name == 'r':
            for pk, scid, base, ppm, cltv in inv['routes'][hint]:
                line += (f'\n    hop {pk[:8]}... over {scid >> 40}x{(scid >> 16) & 0xffffff}x{scid & 0xffff}, '
                         f'fee {base} msat + {ppm} ppm, cltv delta {cltv}')
            hint += 1
        out.append(line)
    out.append(f"  signature recovery ID {inv['recovery']}, payee {inv['payee'].hex()}"
               + (' (from n)' if inv['n'] else ' (recovered)'))
    assert hrp == hrp
    return out


def transcript():
    rows, key = check_vectors()
    payee = pub(int(key, 16))
    lines = ['BOLT 11 examples']
    good = [r for r in rows if r['valid']]
    lines.append(f'  {len(good)} of 16 valid invoices decode, '
                 f'{sum(B.decode(r["invoice"])["payee"] == payee for r in good)} to the key that signed them')
    bad = [code(r['invoice']) for r in rows if not r['valid']]
    lines.append(f'  {sum(c != "OK" for c in bad)} of 15 invalid invoices refuse: ' + ' '.join(bad))

    lines.append('$3 for a cup of coffee')
    text = rows[1]['invoice']
    inv = B.decode(text)
    lines += report(inv, text)
    lines.append(f"  signed: {len(inv['message'])} bytes, SHA-256 {inv['digest'].hex()}")
    lines.append('a fallback address and two route hints')
    lines += report(B.decode(rows[5]['invoice']), rows[5]['invoice'])
    lines.append('an amount in pico-bitcoin and fields to skip')
    t = rows[10]['invoice']
    lines.append(f"  {B.decode(t)['amount']} msat from {t[:t.rfind('1')]}")
    inv = B.decode(rows[13]['invoice'])
    lines.append(f"  {inv['unknown']} unknown fields skipped, {len(inv['fallbacks'])} fallbacks kept")

    # ---- an invoice we write, on regtest
    lines.append('an invoice we write')
    node = int.from_bytes(bytes((7 * i + 3) & 255 for i in range(32)), 'big')
    peer = int.from_bytes(bytes((11 * i + 5) & 255 for i in range(32)), 'big')
    payment_hash = hashlib.sha256(b'\x42' * 32).digest()
    secret = bytes(255 - i for i in range(32))
    features = (1 << 8) | (1 << 14) | (1 << 17)
    desc = b'Bitcoin From Scratch, chapter 114'
    tap = bytes(range(1, 33))
    scid = (800000 << 40) | (7 << 16) | 1
    hop = pub(peer) + scid.to_bytes(8, 'big') + (1000).to_bytes(4, 'big') + (100).to_bytes(4, 'big') + (144).to_bytes(2, 'big')
    fields = [(1, B.to_words(payment_hash)), (16, B.to_words(secret)), (13, B.to_words(desc)),
              (6, B.int_words(600)), (24, B.int_words(80)), (9, [1] + B.to_words(tap)), (3, B.to_words(hop)),
              (5, B.int_words(features))]
    text = B.encode('lnbcrt1500u', 1762156800, fields, node)
    lines.append(f'  {len(text)} characters: {text}')
    lines.append(f'  decodes: {code(text)}')
    inv = B.decode(text)
    lines += report(inv, text)
    lines.append(f"  the payee is our node: {'true' if inv['payee'] == pub(node) else 'false'}")

    lines.append('changing it')
    changed = text[:60] + ('p' if text[60] == 'q' else 'q') + text[61:]
    lines.append(f'  one character changed: {code(changed)}')
    words = [B.CHARSET.index(c) for c in text[text.rfind('1') + 1:-6]]
    moved = B.bech32_encode('lnbcrt1500m', words)
    inv = B.decode(moved)
    lines.append(f"  {inv['amount']} msat with the checksum made again: {code(moved)}, payee {inv['payee'].hex()}")
    lines.append(f"  is it our node: {'true' if inv['payee'] == pub(node) else 'false'}")
    fields = fields[:3] + [(19, B.to_words(pub(node)))]
    text = B.encode('lnbcrt1500u', 1762156800, fields, node)
    lines.append(f'  with an n field: {len(text)} characters, {code(text)}')
    words = [B.CHARSET.index(c) for c in text[text.rfind('1') + 1:-6]]
    lines.append(f"  its amount changed: {code(B.bech32_encode('lnbcrt1500m', words))}")
    return '\n'.join(lines).split('\n')


def main():
    lines = transcript()
    want = FIXTURE.read_text().splitlines()
    assert lines == want, '\n'.join(f'{a!r} != {b!r}' for a, b in zip(lines, want) if a != b) or 'line counts differ'
    print(f'every BOLT 11 vector passes; {len(lines)} lines agree with {FIXTURE.name}')
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            got = subprocess.run([coax, 'run', EXAMPLE, *mode, '--run-steps', RUN_STEPS], cwd=REPO,
                                 capture_output=True, text=True, check=True)
            assert got.stdout.splitlines() == lines, f'coax run {" ".join(mode)} differs'
            print(f'coax run {" ".join(mode) or "(debug)"}: {len(lines)} lines agree')


if __name__ == '__main__':
    if '--print' in sys.argv:
        print('\n'.join(transcript()))
    else:
        main()
