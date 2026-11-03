#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: transcribes BOLT 12's published test vectors outside Coax)
"""Writes coax/tests/lightning/offer_vectors.coax for Bitcoin chapter 115:

  python3 -I coax/tests/lightning/make_offer_vectors.py

From BOLT 12, pinned in coax/tests/vectors/upstream/bolts/:

- OFFER_FORMATS (bolt12/format-string-test.json): strings with '+' joins,
  whitespace and case, valid or not.
- OFFER_ROWS (bolt12/offers-test.json): every offer, valid or invalid,
  with the code the chapter's reader must give. REASONS below is the
  reason each description names. Seven invalid vectors are malformed
  earlier than their description says, and their reason is the earlier
  one: five blinded-path vectors leave out the offer_paths length, so the
  next bytes read as records out of order, and two type-range vectors
  write a 4-byte type after 0xfd, which reads as a 2-byte type and then
  runs out of bytes.
- OFFER_MERKLE (signature-test.json): the three TLV streams of BOLT 1's
  n1 namespace and their Merkle roots, and the invoice request with its
  root, tagged hash and signature.
- PROOF_VALID and PROOF_INVALID (payer-proof-test.json): each payer proof
  with the invoice it comes from, the fields it keeps and the roots, and
  each invalid proof with its reason.

Every pinned file's SHA-256 must equal its pin.
"""
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import bolt12 as B  # noqa: E402
import bolts  # noqa: E402

OUT = HERE / 'offer_vectors.coax'

CODES = ['OK', 'BAD_TEXT', 'BAD_PREFIX', 'TRUNCATED', 'NOT_MINIMAL', 'ORDER', 'RANGE', 'UNKNOWN_EVEN',
         'BAD_LENGTH', 'BAD_UTF8', 'BAD_POINT', 'BAD_PATH', 'BAD_FEATURES', 'BAD_VALUE', 'REQUIRED',
         'BAD_AMOUNT', 'BAD_QUANTITY', 'FORBIDDEN', 'BAD_SIGNATURE', 'WRONG_NODE', 'BAD_PREIMAGE',
         'BAD_OMITTED', 'BAD_LEAVES', 'BAD_MISSING', 'CAPACITY']

REASONS = {
    'Malformed: fields out of order': 'ORDER',
    'Malformed: unknown even TLV type 78': 'UNKNOWN_EVEN',
    'Malformed: empty': 'REQUIRED',
    'Malformed: truncated at type': 'TRUNCATED',
    'Malformed: truncated in length': 'TRUNCATED',
    'Malformed: truncated after length': 'TRUNCATED',
    'Malformed: truncated in description': 'TRUNCATED',
    'Malformed: invalid offer_chains length': 'BAD_LENGTH',
    'Malformed: truncated currency UTF-8': 'BAD_UTF8',
    'Malformed: invalid currency UTF-8': 'BAD_UTF8',
    'Malformed: truncated description UTF-8': 'BAD_UTF8',
    'Malformed: invalid description UTF-8': 'BAD_UTF8',
    'Malformed: truncated offer_paths': 'BAD_PATH',
    'Malformed: zero num_hops in blinded_path': 'ORDER',             # no offer_paths length
    'Malformed: truncated onionmsg_hop in blinded_path': 'ORDER',    # no offer_paths length
    'Malformed: bad first_node_id in blinded_path': 'ORDER',         # no offer_paths length
    'Malformed: bad path_key in blinded_path': 'ORDER',              # no offer_paths length
    'Malformed: bad blinded_node_id in onionmsg_hop': 'ORDER',       # no offer_paths length
    'Malformed: truncated issuer UTF-8': 'BAD_UTF8',
    'Malformed: invalid issuer UTF-8': 'BAD_UTF8',
    'Malformed: invalid offer_issuer_id': 'BAD_POINT',
    'Contains type >= 80': 'RANGE',
    'Contains type > 1999999999': 'TRUNCATED',                       # 0xfd before a 4-byte type
    'Contains unknown even type (1000000002)': 'TRUNCATED',          # 0xfd before a 4-byte type
    'Contains unknown feature 122': 'BAD_FEATURES',
    'Missing offer_description, but has offer_amount': 'REQUIRED',
    'Missing offer_amount with offer_currency': 'REQUIRED',
    'Invalid: zero offer_amount': 'BAD_AMOUNT',
    'Invalid: zero offer_amount with currency': 'BAD_AMOUNT',
    'Missing offer_issuer_id and no offer_path': 'REQUIRED',
    'Second offer_path is empty': 'BAD_PATH',
    'offer_chains with zero entries': 'BAD_LENGTH',
    'Bech32 padding exceeds 4-bit limit': 'BAD_TEXT',
}
PROOF_REASONS = {
    'wrong_proof_preimage': 'BAD_PREIMAGE', 'proof_leaf_hashes_too_few': 'BAD_LEAVES',
    'proof_leaf_hashes_too_many': 'BAD_LEAVES', 'proof_missing_hashes_too_few': 'BAD_MISSING',
    'proof_missing_hashes_too_many': 'BAD_MISSING', 'wrong_invoice_signature': 'BAD_SIGNATURE',
    'wrong_proof_signature': 'BAD_SIGNATURE',
    'contains_invreq_metadata': 'RANGE',   # type 0 is outside a payer proof's ranges
}


def proof_reason(r):
    if r.startswith('missing_'):
        return 'REQUIRED'
    if r.startswith('proof_omitted_tlvs_'):
        return 'BAD_OMITTED'
    return PROOF_REASONS[r]


def code(text):
    try:
        B.decode(text)
        return 'OK'
    except B.Refused as e:
        return str(e)


def esc(s):
    return s.replace('\\', '\\\\').replace('"', '\\"').replace('\n', '\\n').replace('\r', '\\r').replace('\t', '\\t')


def main():
    formats = bolts.bolt12_json('format-string')
    offers = bolts.bolt12_json('offers')
    sig = bolts.bolt12_json('signature')
    pp = bolts.bolt12_json('payer-proof')
    assert len(offers) == 53 and sum(o['valid'] for o in offers) == 20
    text = [
        "// Generated by make_offer_vectors.py from BOLT 12's test vectors, pinned in",
        '// coax/tests/vectors/upstream/bolts/ (see SOURCES.md there). Test data only.',
        'OfferFormat :: struct { text: string; valid: bool; }',
        f'OFFER_FORMATS :: [{len(formats)}] OfferFormat.[',
    ]
    for f in formats:
        text.append(f'  OfferFormat.{{text="{esc(f["string"])}", valid={"true" if f["valid"] else "false"}}},')
    text += ['];', '// code: the OFR_ code the reader gives, 0 for a valid offer.',
             'OfferRow :: struct { text: string; valid: bool; code: int; }',
             f'OFFER_ROWS :: [{len(offers)}] OfferRow.[']
    for o in offers:
        want = 'OK' if o['valid'] else REASONS[o['description']]
        assert code(o['bolt12']) == want, o['description']
        text.append(f'  OfferRow.{{text="{o["bolt12"]}", valid={"true" if o["valid"] else "false"}, '
                    f'code={CODES.index(want)}}},  // {o["description"][:60]}')
    text += ['];']
    # The n1 streams: tlv1 = 1000, tlv2 = 1x2x3, tlv3 = (node_id, 1, 2).
    t1 = '010203e8'
    t2 = '02080000010000020003'
    t3 = '0331' + '0266e4598d1d3c415f572a8488830b60f7e744ed9235eb0b1ba93283b315c03518' + '00' * 7 + '01' + '00' * 7 + '02'
    streams = [t1, t1 + t2, t1 + t2 + t3]
    text += ['MerkleRow :: struct { stream: string; root: string; }', 'OFFER_MERKLE :: [3] MerkleRow.[']
    for s, v in zip(streams, sig[:3]):
        assert B.merkle(B.records(bytes.fromhex(s))).hex() == v['merkle']
        text.append(f'  MerkleRow.{{stream="{s}", root="{v["merkle"]}"}},')
    r = sig[3]
    assert code(r['bolt12']) == 'OK'
    text += ['];',
             f'OFFER_REQUEST :: "{r["bolt12"]}";',
             f'OFFER_REQUEST_ROOT :: "{r["merkle"]}";',
             f'OFFER_REQUEST_HASH :: "{r["H(signature_tag,merkle)"]}";',
             f'OFFER_REQUEST_SIGNATURE :: "{r["signature"]}";',
             '// Payer proofs: include lists the invoice types kept, note is proof_note,',
             '// and empty_list marks a proof with an empty proof_omitted_tlvs written out.',
             f'PROOF_PAYER_KEY :: "{pp["payer_secret"]}";',
             f'PROOF_NODE_KEY :: "{pp["keys"]["invoice_node_id"]["secret"]}";',
             'ProofRow :: struct { name: string; invoice: string; preimage: string; include: [16] u64; '
             'include_n: int; note: string; empty_list: bool; proof: string; invoice_root: string; '
             'proof_root: string; omitted: int; missing: int; }',
             f'PROOF_VALID :: [{len(pp["valid_vectors"])}] ProofRow.[']
    for v in pp['valid_vectors']:
        inc = [x['type'] for x in v['input']['invoice_fields'] if x['included'] and not 240 <= x['type'] <= 1000]
        _, rs, f = B.decode(v['result']['bech32'])
        note = f[1005].decode() if 1005 in f else ''
        explicit = 1002 in f and not v['working']['proof_omitted_tlvs']
        w = B.proof_write(v['input']['invoice'], bytes.fromhex(v['input']['preimage']), set(inc),
                          int(pp['payer_secret'], 16), note or None, explicit)
        assert w == v['result']['bech32'], v['name']
        assert code(v['input']['invoice']) == 'OK'
        assert len(inc) <= 16
        incs = ', '.join(str(t) for t in inc + [0] * (16 - len(inc)))
        text.append(f'  ProofRow.{{name="{v["name"]}", invoice="{v["input"]["invoice"]}", '
                    f'preimage="{v["input"]["preimage"]}", include=[16] u64.[{incs}], include_n={len(inc)}, '
                    f'note="{esc(note)}", empty_list={"true" if explicit else "false"}, '
                    f'proof="{v["result"]["bech32"]}", invoice_root="{v["working"]["invoice_merkle_root"]}", '
                    f'proof_root="{v["working"]["proof_merkle_root"]}", '
                    f'omitted={len(v["working"]["proof_omitted_tlvs"])}, '
                    f'missing={len(v["working"]["proof_missing_hashes"])}}},')
    text += ['];', 'ProofBad :: struct { reason: string; text: string; code: int; }',
             f'PROOF_INVALID :: [{len(pp["invalid_vectors"])}] ProofBad.[']
    for v in pp['invalid_vectors']:
        want = proof_reason(v['reason'])
        assert code(v['bech32']) == want, v['reason']
        text.append(f'  ProofBad.{{reason="{v["reason"]}", text="{v["bech32"]}", code={CODES.index(want)}}},')
    text += ['];', '']
    OUT.write_text('\n'.join(text))
    print(f'wrote {OUT.name}: {len(formats)} formats, {len(offers)} offers, 4 signature vectors, '
          f'{len(pp["valid_vectors"])} + {len(pp["invalid_vectors"])} payer proofs')


if __name__ == '__main__':
    main()
