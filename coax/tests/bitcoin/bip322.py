#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: bip322_reference.py decides every line outside Coax)
"""The cross-check of Bitcoin chapter 96. It writes the transcript the
example must print from Python: BIP-322's message hash and virtual
transactions for one message, signatures in each format from the
published keys, each one verified, and every case of
make_bip322_vectors.py:

  python3 -I coax/tests/bitcoin/bip322.py                       # compare with the fixture
  python3 -I coax/tests/bitcoin/bip322.py --coax coax/bin/coax  # also run the example
"""
import subprocess, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import bip322_reference as R  # noqa: E402
import make_bip322_vectors as V  # noqa: E402
import tx_reference as T  # noqa: E402

REPO = HERE.parents[2]
FIXTURE = REPO / 'bitcoin/fixtures/bip322.out'
EXAMPLE = 'bitcoin/examples/bip322.coax'
FORMATS = ['legacy', 'smp', 'ful', 'pof', 'bare']


def verdict(address, signature, message):
    state, reason, t, s, fmt = R.verify(address, signature, message)
    name = FORMATS[V.FORMAT[fmt]]
    if state == R.VALID: return f'valid at time {t} and age {s} ({name})'
    if state == R.INVALID: return f'invalid: {R.REASONS[reason]} ({name})'
    return f'inconclusive ({name})'


def transcript():
    hashes, cases, errors, built = V.build()
    lines = []
    p_ = lines.append
    h = hashes[1]
    spk = R.address_script(h['address'])
    spend = R.to_spend(spk, h['message'])
    p_(f'"{h["message"]}" for {h["address"]}:')
    p_(f'  message_hash {R.message_hash(h["message"]).hex()}')
    p_(f'  to_spend {T.sha256d(spend)[::-1].hex()}')
    p_(f'  to_sign  {T.txid(R.to_sign(spend))[::-1].hex()} (unsigned)')

    def show(title, address, signature, message):
        p_(f'{title}: {signature[:44]}... ({len(signature)} characters)')
        p_(f'  {verdict(address, signature, message)}')

    def keys(c): return [R.wif_secret(c['keys'][k:k + 52]) for k in range(0, len(c['keys']), 52)]
    w, tr, leaf, pkh, pof = cases[3], cases[5], cases[13], cases[10], cases[22]
    msg = 'Hello World'
    s = R.sign(w['address'], keys(w), msg, 'smp')
    show('P2WPKH, simple', w['address'], s, msg)
    p_(f'  the published signature: {"the same" if s == w["signature"] else "another"}')
    show('P2WPKH, the same signature under "Hello World!"', w['address'], s, msg + '!')
    show('P2WPKH, full', w['address'], R.sign(w['address'], keys(w), msg, 'ful'), msg)
    show('P2TR key path, simple', tr['address'], R.sign(tr['address'], keys(tr), msg, 'smp'), msg)
    show('P2TR time-lock leaf, full, version 2', leaf['address'],
         R.sign(leaf['address'], keys(leaf), msg, 'ful', bytes.fromhex(leaf['script']), 2, 2016, 2016), msg)
    show('P2PKH, legacy', pkh['address'], R.legacy_sign(keys(pkh)[0], msg), msg)
    show('P2PKH, full', pkh['address'], R.sign(pkh['address'], keys(pkh), msg, 'ful'), msg)
    show('P2TR proof of funds, published', pof['address'], pof['signature'], pof['message'])
    p_('The BIP-322 vectors:')
    p_(f'  hashes: {len(hashes)} of {len(hashes)} match')
    p_(f'  signatures: {len(cases)} of {len(cases)} verify as published')
    signed = [c for c in cases if c['keys']]
    same = sum(1 for c in signed if c['ours'] == (c['signature'] if c['signature'][:3] in ('smp', 'ful') else 'smp' + c['signature']))
    p_(f'  the signer: {len(signed)} of {len(signed)} as the reference signs; {same} are the published ones byte for byte,')
    p_(f'    {len(signed) - same} were published with other nonces (BIP-340, or ECDSA ground to a low R)')
    p_(f'  errors: {len(errors)} of {len(errors)} invalid for their reason')
    states = [sum(1 for c in built if c['state'] == k) for k in range(3)]
    p_(f'  constructed: {len(built)} of {len(built)} as decided: {states[0]} valid, {states[1]} invalid, {states[2]} inconclusive')
    return '\n'.join(lines) + '\n'


def main():
    text = transcript()
    if '--write' in sys.argv:
        FIXTURE.write_text(text)
    assert FIXTURE.read_text() == text, 'the fixture differs from Python'
    print(f'bip322.py: {text.count(chr(10))} lines agree with Python')
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            out = subprocess.run([coax, 'run', EXAMPLE, '--run-steps', '100000000'] + mode, cwd=REPO,
                                 capture_output=True, text=True, check=True).stdout
            assert out == text, f'the example differs from Python ({mode or "debug"})'
            print(f'bip322.coax ({"release" if mode else "debug"}) prints the same transcript')


if __name__ == '__main__':
    main()
