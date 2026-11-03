#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: BOLT 12's vectors and a separate Python reader and writer decide each case outside Coax)
"""The cross-check of Bitcoin chapter 115. It runs every BOLT 12 vector on
bolt12.py, computes every line the chapter's example prints, and compares
the transcript with the fixture:

  python3 -I coax/tests/lightning/offers.py                        # compare with the fixture
  python3 -I coax/tests/lightning/offers.py --coax coax/bin/coax   # also run the example

bolt12.py is this cross-check's own BOLT 12 reader and writer, Merkle tree
and BIP-340 signer and verifier, written from BOLT 12 and BIP-340 on
ln_crypto.py's secp256k1, sharing no code with Coax; sphinx.py's route
blinding builds the example's blinded path. Every format string and offer
in bolt12/*.json must be read or refused as stated, each invalid offer
and payer proof for the reason make_offer_vectors.py names, the Merkle
roots and signature of signature-test.json must match, and each payer
proof must be written again byte for byte, before the transcript is
compared.
"""
import hashlib
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import bolt12 as B  # noqa: E402
import bolts  # noqa: E402
import ln_crypto as C  # noqa: E402
import make_offer_vectors as V  # noqa: E402
import onion as O  # noqa: E402
import sphinx as S  # noqa: E402

REPO = HERE.parents[2]
FIXTURE = REPO / 'bitcoin/fixtures/lightning-offers.out'
EXAMPLE = 'bitcoin/examples/lightning-offers.coax'
RUN_STEPS = '250000000'
NAMES = {0: 'invreq_metadata', 2: 'offer_chains', 4: 'offer_metadata', 6: 'offer_currency', 8: 'offer_amount',
         10: 'offer_description', 12: 'offer_features', 14: 'offer_absolute_expiry', 16: 'offer_paths',
         18: 'offer_issuer', 20: 'offer_quantity_max', 22: 'offer_issuer_id', 80: 'invreq_chain',
         82: 'invreq_amount', 86: 'invreq_quantity', 88: 'invreq_payer_id', 89: 'invreq_payer_note',
         160: 'invoice_paths', 162: 'invoice_blindedpay', 164: 'invoice_created_at', 168: 'invoice_payment_hash',
         170: 'invoice_amount', 176: 'invoice_node_id', 240: 'signature', 241: 'proof_signature',
         1001: 'proof_preimage', 1002: 'proof_omitted_tlvs', 1003: 'proof_missing_hashes',
         1004: 'proof_leaf_hashes', 1005: 'proof_note'}


def code(text):
    try:
        B.decode(text)
        return 'OK'
    except B.Refused as e:
        return str(e)


# ---- BOLT 12's vectors

def check_vectors():
    for f in bolts.bolt12_json('format-string'):
        try:
            B.text_decode(f['string'])
            ok = True
        except B.Refused:
            ok = False
        assert ok == f['valid'], f['comment']
    for o in bolts.bolt12_json('offers'):
        want = 'OK' if o['valid'] else V.REASONS[o['description']]
        assert code(o['bolt12']) == want, o['description']
        if o['valid']:
            _, rs, _ = B.decode(o['bolt12'])
            assert [(t, v.hex()) for t, v, _ in rs] == [(x['type'], x['hex']) for x in o['fields']]
            assert B.text_encode('offer', b''.join(r for _, _, r in rs)) == o['bolt12'].lower()
    sig = bolts.bolt12_json('signature')
    t1, t2 = '010203e8', '02080000010000020003'
    t3 = '0331' + '0266e4598d1d3c415f572a8488830b60f7e744ed9235eb0b1ba93283b315c03518' + '00' * 7 + '01' + '00' * 7 + '02'
    for s, v in zip([t1, t1 + t2, t1 + t2 + t3], sig[:3]):
        assert B.merkle(B.records(bytes.fromhex(s))).hex() == v['merkle']
    r = sig[3]
    _, rs, f = B.decode(r['bolt12'])
    assert B.merkle(rs).hex() == r['merkle']
    assert B.sig_hash('invoice_request', 'signature', B.merkle(rs)).hex() == r['H(signature_tag,merkle)']
    assert B.schnorr_sign(int('42' * 32, 16), bytes.fromhex(r['H(signature_tag,merkle)'])).hex() == r['signature']
    pp = bolts.bolt12_json('payer-proof')
    for v in pp['valid_vectors']:
        assert code(v['input']['invoice']) == 'OK' and code(v['result']['bech32']) == 'OK'
        _, rs, f = B.decode(v['input']['invoice'])
        assert B.merkle(rs).hex() == v['working']['invoice_merkle_root']
        _, prs, pf = B.decode(v['result']['bech32'])
        assert B.merkle(prs).hex() == v['working']['proof_merkle_root']
        keep = {x['type'] for x in v['input']['invoice_fields'] if x['included'] and not 240 <= x['type'] <= 1000}
        note = pf[1005].decode() if 1005 in pf else None
        empty = 1002 in pf and not v['working']['proof_omitted_tlvs']
        again = B.proof_write(v['input']['invoice'], bytes.fromhex(v['input']['preimage']), keep,
                              int(pp['payer_secret'], 16), note, empty)
        assert again == v['result']['bech32'], v['name']
    for v in pp['invalid_vectors']:
        assert code(v['bech32']) == V.proof_reason(v['reason']), v['reason']


# ---- the example's transcript

def secret(k, c):
    return bytes((k * i + c) & 255 for i in range(32))


def pub(s):
    return C.compress(C.mul(int.from_bytes(s, 'big')))


def fields(rs):
    return [f'  {t} {NAMES.get(t, "unknown")}: {len(v)} bytes' for t, v, _ in rs]


def split(text):
    return '+\n'.join(text[i:i + 64] for i in range(0, len(text), 64))


def write(kind, pairs):
    return B.text_encode(kind, B.stream(pairs))


def matches(req_rs, inv_rs):
    mine = {t: raw for t, _, raw in inv_rs if t <= 159 or 10**9 <= t <= 2999999999}
    theirs = {t: raw for t, _, raw in req_rs if not 240 <= t <= 1000}
    return mine == theirs


def transcript():
    check_vectors()
    lines = ["BOLT 12's vectors"]
    formats = bolts.bolt12_json('format-string')
    ok = sum(f['valid'] for f in formats)
    lines.append(f'  format strings: {ok} read, {len(formats) - ok} refused')
    offers = bolts.bolt12_json('offers')
    lines.append('  offers refused: ' + ' '.join(code(o['bolt12']) for o in offers if not o['valid']))
    lines.append(f'  {sum(code(o["bolt12"]) == "OK" for o in offers)} of 20 valid offers read')
    pp = bolts.bolt12_json('payer-proof')
    lines.append(f'  {sum(code(v["result"]["bech32"]) == "OK" for v in pp["valid_vectors"])} of 5 payer proofs check; '
                 'invalid ones refused: ' + ' '.join(code(v['bech32']) for v in pp['invalid_vectors']))

    lines.append('an offer priced in dollars')
    _, rs, f = B.decode(offers[7]['bolt12'])
    lines += fields(rs)
    lines.append(f'  {f[6].decode()} {B.tu(f[8])} cents, "{f[10].decode()}"')

    lines.append("BOLT 12's invoice request")
    r = bolts.bolt12_json('signature')[3]
    _, rs, f = B.decode(r['bolt12'])
    lines += fields(rs)
    root = B.merkle(rs)
    lines.append(f'  Merkle root {root.hex()}')
    lines.append(f'  signed hash {B.sig_hash("invoice_request", "signature", root).hex()}')
    lines.append(f'  the signature checks under invreq_payer_id: {code(r["bolt12"])}')

    # ---- Dave's offer, behind a blinded path that starts at Carol
    lines.append("Dave's offer, behind a blinded path from Carol")
    carol, dave, erin = secret(7, 3), secret(11, 5), secret(13, 1)
    carol_id, dave_id = pub(carol), pub(dave)
    e, path_id = secret(17, 9), secret(19, 2)
    made = S.blind([int.from_bytes(e, 'big'), None], [carol_id, dave_id],
                   [bytes([4, 33]) + dave_id, bytes([6, 32]) + path_id])
    (carol_key, carol_blinded, carol_box, _), (dave_key, dave_blinded, dave_box, _) = made
    path = (carol_id + carol_key + bytes([2]) + carol_blinded + len(carol_box).to_bytes(2, 'big') + carol_box
            + dave_blinded + len(dave_box).to_bytes(2, 'big') + dave_box)
    offer = write('offer', [(10, b'Bitcoin From Scratch, chapter 115'), (8, B.put_tu(50000)), (16, path),
                            (18, b'Dave'), (20, B.put_tu(10))])
    lines.append(f'  {len(offer)} characters, {code(offer)}')
    _, ors, of = B.decode(offer)
    lines += fields(ors)
    joined = split(offer)
    lines.append(f"  split with '+' and newlines: {code(joined)}, same bytes: "
                 f"{'true' if B.text_decode(joined) == B.text_decode(offer) else 'false'}")
    final = B.paths(of[16])[0][2][-1][0]
    lines.append(f'  no offer_issuer_id; the path ends at blinded node {final.hex()}')
    lines.append(f"  is that Dave's node ID: {'true' if final == dave_id else 'false'}")

    # ---- Erin asks for three
    lines.append("Erin's invoice request")
    metadata = secret(23, 4)[:16]
    request = B.request(offer, metadata, int.from_bytes(erin, 'big'), quantity=3)
    lines.append(f'  {len(request)} characters, {code(request)}')
    _, rrs, rf = B.decode(request)
    lines += [f'  added {t} {NAMES[t]}: {len(v)} bytes' for t, v, _ in rrs if t >= 80 or t == 0]
    lines.append(f"  invreq_metadata is first, so it seeds every nonce: {'true' if rrs[0][0] == 0 else 'false'}")

    # ---- Dave's invoice, signed with his key for the path
    lines.append("Dave's invoice")
    blinded = O.blinded_secret(int.from_bytes(dave, 'big'), dave_key)
    key = C.compress(C.mul(blinded))
    lines.append(f"  Dave's blinded secret gives the path's last key: {'true' if key == final else 'false'}")
    preimage = secret(29, 6)
    payment_hash = hashlib.sha256(preimage).digest()
    payinfo = ((1000).to_bytes(4, 'big') + (100).to_bytes(4, 'big') + (144).to_bytes(2, 'big')
               + (1).to_bytes(8, 'big') + (10**10).to_bytes(8, 'big') + bytes(2))
    amount = B.tu(of[8]) * B.tu(rf[86])
    invoice = B.invoice(request, blinded, path, 1762156800, payment_hash, amount, payinfo)
    _, irs, inf = B.decode(invoice)
    lines.append(f'  {len(invoice)} characters, {code(invoice)}, {B.tu(inf[170])} msat for 3')
    lines += [f'  added {t} {NAMES[t]}: {len(v)} bytes' for t, v, _ in irs if t >= 160]
    lines.append(f"  Erin: it repeats her request: {'true' if matches(rrs, irs) else 'false'}")
    lines.append(f"  Erin: invoice_node_id is the path's last key: {'true' if inf[176] == final else 'false'}")
    expired = 1762156800 + 7201 > B.tu(inf[164]) + 7200
    lines.append(f"  Erin: expired two hours and a second later: {'true' if expired else 'false'}")

    # ---- Erin's payer proof, after she pays
    lines.append("Erin's payer proof")
    proof = B.proof_write(invoice, preimage, {10, 88, 168, 170, 176}, int.from_bytes(erin, 'big'), 'paid')
    lines.append(f'  {len(proof)} characters, {code(proof)}')
    _, prs, pf = B.decode(proof)
    lines += fields(prs)
    marks = []
    at = 0
    while at < len(pf[1002]):
        m, at = B.bigsize(pf[1002], at)
        marks.append(m)
    lines.append('  proof_omitted_tlvs: ' + ' '.join(str(m) for m in marks))
    lines.append(f'  {len(pf[1003]) // 32} hashes stand for the {len(marks) + 1} hidden fields')

    # ---- changes
    lines.append('changing them')
    pairs = [(t, v) for t, v, _ in irs if t != 170] + [(170, B.put_tu(1))]
    lines.append(f"  the invoice's amount set to 1 msat: {code(write('invoice', pairs))}")
    changed = list(invoice)
    changed[100] = 'p' if changed[100] == 'q' else 'q'
    lines.append(f"  one character of the invoice changed: {code(''.join(changed))}")
    other = B.invoice(request, int.from_bytes(dave, 'big'), path, 1762156800, payment_hash, amount, payinfo)
    lines.append(f"  the invoice signed with Dave's node key: {code(other)}, but the path's key: "
                 f"{'true' if B.decode(other)[2][176] == final else 'false'}")
    pairs = [(t, v) for t, v, _ in prs if t != 1001] + [(1001, payment_hash)]
    lines.append(f"  the proof with another preimage: {code(write('payer_proof', pairs))}")
    pairs = [(t, v) for t, v, _ in prs if t != 1005] + [(1005, b'not paid')]
    lines.append(f"  the proof with another note: {code(write('payer_proof', pairs))}")
    lines.append(f"  a request for 11 of at most 10: "
                 f"{code(B.request(offer, metadata, int.from_bytes(erin, 'big'), quantity=11))}")
    lines.append(f"  a request for 3 that pays 149999 msat: "
                 f"{code(B.request(offer, metadata, int.from_bytes(erin, 'big'), amount=149999, quantity=3))}")
    return lines


def main():
    lines = transcript()
    want = FIXTURE.read_text().splitlines()
    assert lines == want, '\n'.join(f'{a!r} != {b!r}' for a, b in zip(lines, want) if a != b) or 'line counts differ'
    print(f'every BOLT 12 vector passes; {len(lines)} lines agree with {FIXTURE.name}')
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
