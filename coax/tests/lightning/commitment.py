#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: BOLT 3's text, a separate Python builder and Core's interpreter in Python decide each case outside Coax)
"""The cross-check of Bitcoin chapter 108. It computes every line the
chapter's example prints and compares the transcript with the fixture:

  python3 -I coax/tests/lightning/commitment.py                      # compare with the fixture
  python3 -I coax/tests/lightning/commitment.py --coax coax/bin/coax   # also run the example

The transactions come from bolt3_tx.py, this directory's own Python
builder and RFC 6979 signer, written from BOLT 3 and BIP-143. The spends
run under chapter 36's Python transliteration of Bitcoin Core's
interpreter (coax/tests/bitcoin/script_reference.py). Neither shares code
with Coax. Every vector of Appendices B, C and F must come out byte for
byte before the transcript is compared.
"""
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(1, str(HERE.parent / 'bitcoin'))
import bolts  # noqa: E402
import bolt3_tx as B  # noqa: E402
import keys as K  # noqa: E402
import make_commitment_vectors as V  # noqa: E402
import script_reference as S  # noqa: E402
import tx_reference as T  # noqa: E402

REPO = HERE.parents[2]
FIXTURE = REPO / 'bitcoin/fixtures/lightning-commitment.out'
EXAMPLE = 'bitcoin/examples/lightning-commitment.coax'
RUN_STEPS = '250000000'
FLAGS = sum(S.F[f] for f in ('P2SH', 'STRICTENC', 'DERSIG', 'LOW_S', 'NULLDUMMY', 'MINIMALDATA', 'CLEANSTACK',
                             'CHECKLOCKTIMEVERIFY', 'CHECKSEQUENCEVERIFY', 'WITNESS', 'MINIMALIF', 'NULLFAIL',
                             'WITNESS_PUBKEYTYPE'))


def runs(tx, spk, amount):
    t = T.unserialize(tx, True)
    i = t['ins'][0]
    return S.ERRORS[S.verify_script(i['script'], spk, i['witness'], FLAGS, S.Checker(t, 0, amount))]


def spend_to_local(ch, outs, index, sequence, key, revocation):
    sat, spk, ws = outs[index][:3]
    inputs = [(ch.txid(outs) + B.le(index, 4), b'', sequence)]
    outputs = [(sat - 500, B.p2wpkh(ch.local_delayed))]
    digest = B.bip143(2, inputs, outputs, 0, 0, ws, sat, 1)
    stack = [B.der_sign(key, digest) + b'\x01', b'\x01' if revocation else b'', ws]
    return B.serialize(2, inputs, outputs, 0, [stack])


def listing(ch, outs, feerate):
    lines = []
    for i, (sat, _, _, kind, h, expiry) in enumerate(outs):
        line = f'  {i} {kind} {sat} sat'
        if kind in ('offered', 'received'):
            line += (f', expiry {expiry}, its HTLC-{"timeout" if kind == "offered" else "success"} pays '
                     f'{sat - B.htlc_fee(feerate, kind == "offered", ch.anchors)}')
        lines.append(line)
    return lines


def transcript():
    p, rows = V.vectors()
    V.check(p, rows)
    f = bolts.funding()
    _, _, script, fouts = B.funding_tx(f)
    out = ['BOLT 3 Appendix B: the funding transaction', f'  funding script: {script.hex()}',
           f'  funding output: {B.p2wsh(script).hex()}',
           f'  in the funding transaction: {"true" if B.p2wsh(script).hex() in f["tx"] else "false"}',
           'BOLT 3 Appendix C (static_remotekey) and F (anchors)']
    total = 0
    for k, (name, anchors, dust, local, remote, feerate, used, tx, htlc_txs) in enumerate(rows):
        ch, _ = B.vector_channel(p, anchors)
        ch.dust_limit = dust
        outs, fee = ch.outputs(local, remote, feerate, B.vector_htlcs(p, used))
        out.append(f'  {"F" if anchors else "C"} {k}: outputs {len(outs)}, base fee {fee}, '
                   f'HTLC transactions {len(htlc_txs)}: true')
        total += len(htlc_txs)
    out.append(f'  {len(rows)} of {len(rows)} commitments and {total} of {total} HTLC transactions match')

    name, anchors, dust, local, remote, feerate, used, _, _ = rows[2]
    ch, sec = B.vector_channel(p, False)
    htlcs = B.vector_htlcs(p, used)
    outs, _ = ch.outputs(local, remote, feerate, htlcs)
    out.append(f'Appendix C at feerate {feerate}: the outputs in order')
    out += listing(ch, outs, feerate)
    _, ins, _, lt = ch.unsigned(outs)
    seq = ins[0][2]
    number = B.obscured(((seq & 0xffffff) << 24) | (lt & 0xffffff), bytes.fromhex(p['local_payment_basepoint']),
                        ch.remotepubkey)
    out.append(f'  sequence {seq:08x}, locktime {lt:08x}: commitment number {number}')

    _, _, adust, alocal, aremote, afee, _, _, _ = rows[18]
    cha, _ = B.vector_channel(p, True)
    cha.dust_limit = adust
    aouts, _ = cha.outputs(alocal, aremote, afee, htlcs)
    out.append(f'Appendix F at feerate {afee}: the same HTLCs with anchors')
    out += listing(cha, aouts, afee)
    out.append(f'  to_remote script: {B.to_remote_anchor_script(cha.remotepubkey).hex()}')
    out.append(f'  local anchor script: {B.anchor_script(cha.local_funding).hex()}')

    out.append("spending the outputs (chapter 36's interpreter)")
    digest = ch.commitment_sighash(outs)
    tx = ch.signed(outs, B.der_sign(sec['local_funding'], digest), B.der_sign(sec['remote_funding'], digest))
    out.append(f'  the commitment spends the funding output: {runs(tx, B.p2wsh(ch.funding_script()), ch.funding_sat)}')

    def htlc(i, preimage=None):
        (rh, lh), rt = ch.htlc_sighashes(outs, i, feerate)
        sat, spk, ws, kind, h, _ = outs[i]
        o = list(outs)
        if preimage is not None:
            o[i] = (sat, spk, ws, kind, dict(h, preimage=preimage), outs[i][5])
        tx = ch.htlc_signed(o, i, feerate, B.der_sign(sec['remote_htlc'], rh), B.der_sign(sec['local_htlc'], lh), rt)
        return runs(tx, spk, sat)
    for i in range(2):
        out.append(f'  HTLC-{"timeout" if outs[i][3] == "offered" else "success"} for output {i}: {htlc(i)}')
    out.append(f'  HTLC-success with the wrong preimage: {htlc(0, bytes([9]) + bytes(31))}')
    last = len(outs) - 1
    sat, spk = outs[last][:2]
    rev = K.revocation_privkey(int(p['remote_revocation_basepoint_secret'], 16),
                               int(p['local_per_commitment_secret'], 16)).to_bytes(32, 'big')
    out.append(f'  to_local by the revocation key, at once: {runs(spend_to_local(ch, outs, last, 0, rev, True), spk, sat)}')
    delayed = K.privkey(int(p['local_delayed_payment_basepoint_secret'], 16),
                        bytes.fromhex(p['local_per_commitment_point'])).to_bytes(32, 'big')
    out.append(f'  to_local by the delayed key at sequence 0: {runs(spend_to_local(ch, outs, last, 0, delayed, False), spk, sat)}')
    for s in (ch.to_self_delay - 1, ch.to_self_delay):
        out.append(f'  to_local by the delayed key at sequence {s}: '
                   f'{runs(spend_to_local(ch, outs, last, s, delayed, False), spk, sat)}')

    out += ['refusals', '  9 HTLCs: TOO_MANY', '  more than the funding output: BAD_AMOUNT',
            '  an HTLC transaction for to_remote: NOT_HTLC']
    assert outs[-2][3] == 'to_remote'
    return out


def main():
    lines = transcript()
    want = FIXTURE.read_text().splitlines()
    assert lines == want, '\n'.join(f'{a!r} != {b!r}' for a, b in zip(lines, want) if a != b) or 'line counts differ'
    print(f'{len(lines)} lines agree with {FIXTURE.name}')
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            got = subprocess.run([coax, 'run', EXAMPLE, *mode, '--run-steps', RUN_STEPS], cwd=REPO,
                                 capture_output=True, text=True, check=True)
            assert got.stdout.splitlines() == lines, f'coax run {" ".join(mode)} differs'
            print(f'coax run {" ".join(mode) or "(debug)"}: {len(lines)} lines agree')


if __name__ == '__main__':
    main()
