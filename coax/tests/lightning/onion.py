#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: BOLT 4's vectors and a separate Python Sphinx decide each case outside Coax)
"""The cross-check of Bitcoin chapter 111. It runs every BOLT 4 vector on
sphinx.py, computes every line the chapter's example prints, and compares
the transcript with the fixture:

  python3 -I coax/tests/lightning/onion.py                        # compare with the fixture
  python3 -I coax/tests/lightning/onion.py --coax coax/bin/coax   # also run the example

sphinx.py is this cross-check's own Sphinx, error onion and route
blinding, written from BOLT 4 on ln_crypto.py's secp256k1 and ChaCha20,
sharing no code with Coax. The five bolt04/*.json files and the "Returning
Errors" trace must all give BOLT 4's bytes before the transcript is
compared.
"""
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import bolts  # noqa: E402
import ln_crypto as C  # noqa: E402
import sphinx as S  # noqa: E402

REPO = HERE.parents[2]
FIXTURE = REPO / 'bitcoin/fixtures/lightning-onion.out'
EXAMPLE = 'bitcoin/examples/lightning-onion.coax'
h = bytes.fromhex


def strip(payload):
    """A length-prefixed payload without its length."""
    _, at = S.bigsize(payload, 0)
    return payload[at:]


def blinded_secret(k, path_key):
    return (k * S.scalar(S.key(b'blinded_node_id', S.ecdh(k, path_key)))) % C.N


# ---- BOLT 4's vectors

def check_vectors():
    d = bolts.bolt4_json('onion-test.json')
    g = d['generate']
    ad = h(g['associated_data'])
    pls = [h(x['payload']) for x in g['hops']]
    onion = S.wrap(S.scalar(h(g['session_key'])), [h(x['pubkey']) for x in g['hops']], pls, ad)
    assert onion.hex() == d['onion']
    for i, k in enumerate(d['decode']):
        p, onion, _ = S.peel(S.scalar(h(k)), onion, ad)
        assert p == strip(pls[i]) and (onion is None) == (i == 4)

    e = bolts.bolt4_json('onion-error-test.json')
    g = e['generate']
    secrets = [ss for _, ss in S.route_secrets(S.scalar(h(g['session_key'])), [h(x['pubkey']) for x in g['hops']])]
    for ss, x in zip(secrets, g['hops']):
        assert ss.hex() == x['hop_shared_secret'] and S.key(b'ammag', ss).hex() == x['ammag_key']
    assert S.key(b'um', secrets[4]).hex() == g['hops'][4]['um_key']
    packet = S.error_packet(secrets[4], h(g['failure_message']))
    assert packet[32:].hex() == g['hops'][4]['payload']
    for i in reversed(range(5)):
        packet = S.error_wrap(secrets[i], packet)
    assert packet.hex() == e['errorpacket'] and S.error_open(secrets, packet) == (4, h(g['failure_message']))

    trace = bolts.error_trace()
    body = h(trace['payload'])
    failure = body[2:2 + int.from_bytes(body[:2], 'big')]
    packet = S.error_packet(secrets[4], failure, 1024)
    assert packet.hex() == trace['raw'] and packet[32:] == body
    for n, ss, ammag, want in trace['hops']:
        assert ss == secrets[n].hex() and ammag == S.key(b'ammag', secrets[n]).hex()
        packet = S.error_wrap(secrets[n], packet)
        assert packet.hex() == want, n
    assert S.error_open(secrets, packet) == (4, failure)

    b = bolts.bolt4_json('route-blinding-test.json')
    gen = b['generate']['hops']
    made = S.blind([S.scalar(h(x['session_key'])) if 'session_key' in x else None for x in gen],
                   [h(x['node_id']) for x in gen], [h(x['encoded_tlvs']) for x in gen])
    for (pk, bid, box, ss), x, u in zip(made, gen, b['unblind']['hops']):
        assert (pk.hex(), bid.hex(), box.hex(), ss.hex()) == (x['path_key'], x['blinded_node_id'], x['encrypted_data'],
                                                              x['shared_secret'])
        assert S.key(b'rho', ss).hex() == x['rho']
        priv, plain, nxt = S.unblind(S.scalar(h(u['node_privkey'])), pk, box)
        assert f'{priv:064x}' == u['blinded_privkey'] and plain.hex() == u['decrypted_data'] and S.pub(priv) == bid
        assert nxt.hex() == u.get('next_path_key_override', u['next_path_key'])

    p = bolts.bolt4_json('blinded-payment-onion-test.json')
    g = p['generate']
    ad, full = h(g['associated_data']), g['full_route']['hops']
    onion = S.wrap(S.scalar(h(g['session_key'])), [h(x['pubkey']) for x in full], [h(x['payload']) for x in full], ad)
    assert onion.hex() == g['onion']
    path_key = None
    for i, x in enumerate(p['decrypt']['hops']):
        assert onion.hex() == x['onion']
        k = S.scalar(h(x['node_privkey']))
        payload, onion, _ = S.peel(blinded_secret(k, path_key) if path_key else k, onion, ad)
        t = S.tlvs(payload)
        path_key = t.get(12, path_key)
        if 10 in t:
            _, _, path_key = S.unblind(k, path_key, t[10])
            assert path_key.hex() == x['next_path_key']
    assert onion is None

    m = bolts.bolt4_json('blinded-onion-message-onion-test.json')
    hops, dec = m['generate']['hops'], m['decrypt']['hops']
    starts = [S.scalar(h(hops[0]['path_key_secret'])), S.scalar(h(hops[1]['path_key_secret'])), None, None]
    made = S.blind(starts, [S.pub(S.scalar(h(x['privkey']))) for x in dec], [h(x['encrypted_data_tlv']) for x in hops])
    pls = []
    for i, ((pk, bid, box, ss), x) in enumerate(zip(made, hops)):
        assert (pk.hex(), bid.hex(), box.hex(), ss.hex()) == (x['E'], x['blinded_node_id'], x['encrypted_recipient_data'], x['ss'])
        body = S.tlv({4: box, **({1: h(m['onionmessage']['unknown_tag_1'])} if i == 3 else {})})
        pls.append(S.bigsize_bytes(len(body)) + body)
    packet = S.wrap(S.scalar(h(m['generate']['session_key'])), [x[1] for x in made], pls, b'')
    assert packet.hex() == m['onionmessage']['onion_message_packet']
    message = h(dec[0]['onion_message'])
    for i, x in enumerate(dec):
        assert message.hex() == x['onion_message']
        k = S.scalar(h(x['privkey']))
        pk, packet = message[2:35], message[37:37 + int.from_bytes(message[35:37], 'big')]
        payload, nxt, _ = S.peel(blinded_secret(k, pk), packet, b'')
        t = S.tlvs(payload)
        _, plain, next_key = S.unblind(k, pk, t[4])
        if nxt is None:
            assert i == 3 and t[1] == h(m['onionmessage']['unknown_tag_1'])
            break
        assert S.tlvs(plain)[4].hex() == x['next_node_id']
        message = b'\x02\x01' + next_key + len(nxt).to_bytes(2, 'big') + nxt
    return True


# ---- the example

def secret(k):
    return bytes((k + i) & 255 for i in range(32))


def node_id(k):
    return S.pub(S.scalar(secret(k)))


def scid(block, tx, out):
    return ((block << 40) | (tx << 16) | out).to_bytes(8, 'big')


def scid_text(b):
    v = int.from_bytes(b, 'big')
    return f'{v >> 40}x{(v >> 16) & 0xffffff}x{v & 0xffff}'


def tu(v):
    return v.to_bytes(8, 'big').lstrip(b'\0')


def forward(amount, cltv, channel=None):
    records = {2: tu(amount), 4: tu(cltv)}
    if channel:
        records[6] = channel
    return S.tlv(records)


def prefixed(body):
    return S.bigsize_bytes(len(body)) + body


def report(name, payload, last):
    t = S.tlvs(payload)
    line = f'  {name}: {int.from_bytes(t[2], "big")} msat, cltv {int.from_bytes(t[4], "big")}'
    if 6 in t:
        line += f', forward over {scid_text(t[6])}'
    return line + (', last hop' if last else '')


def code(f):
    try:
        f()
        return 'OK'
    except S.Refused as r:
        return {'BAD_HMAC': 'BAD_HMAC', 'BAD_TAG': 'BAD_TAG'}[r.args[0]]


def transcript():
    out = ['BOLT 4 vectors']
    d = bolts.bolt4_json('onion-test.json')
    e = bolts.bolt4_json('onion-error-test.json')
    ok = check_vectors()
    out += [f'  onion-test.json: the onion matches: {str(ok).lower()}',
            f'  hops that read their own payload: {len(d["decode"])} of 5, the last is hop 4',
            f'  onion-error-test.json: the error packet matches: {str(ok).lower()}',
            f'  the origin finds hop 4, failure {e["generate"]["failure_message"]}']

    out.append('Alice pays Dave through Bob and Carol')
    bob, carol, dave = (S.scalar(secret(k)) for k in (21, 41, 61))
    ids = [node_id(21), node_id(41), node_id(61)]
    session, payment_hash = S.scalar(secret(81)), secret(101)
    bob_carol, carol_dave = scid(800000, 1, 0), scid(800001, 2, 1)
    pls = [prefixed(forward(1002000, 800180, bob_carol)), prefixed(forward(1000000, 800140, carol_dave)),
           prefixed(forward(1000000, 800100))]
    first = S.wrap(session, ids, pls, payment_hash)
    out.append(f'  onion: {len(first)} bytes; payloads and HMACs use {sum(len(p) + 32 for p in pls)} of 1300')
    onion = first
    for name, k in zip(['Bob', 'Carol', 'Dave'], [bob, carol, dave]):
        out.append(f'  {name} sees ephemeral key {onion[1:34].hex()}')
        payload, onion, _ = S.peel(k, onion, payment_hash)
        out.append(report(name, payload, onion is None))
    changed = bytearray(first)
    changed[500] ^= 1
    out.append(f'  Bob given a changed byte: {code(lambda: S.peel(bob, bytes(changed), payment_hash))}')
    out.append(f'  Bob given another payment hash: {code(lambda: S.peel(bob, first, secret(102)))}')

    out.append('Carol fails the payment')
    secrets = [ss for _, ss in S.route_secrets(session, ids)]
    failure = bytes([0x10, 0x07, 0, 0])
    packet = S.error_wrap(secrets[0], S.error_wrap(secrets[1], S.error_packet(secrets[1], failure)))
    out.append(f'  the packet Alice receives: {len(packet)} bytes')
    hop, got = S.error_open(secrets, packet)
    out.append(f'  Alice finds hop {hop} ({["Bob", "Carol", "Dave"][hop]}), failure {got.hex()}')
    bad = bytearray(packet)
    bad[100] ^= 1
    out.append(f'  a packet changed on the way: {"UNKNOWN" if S.error_open(secrets, bytes(bad))[0] is None else "OK"}')

    out.append("Dave's blinded path from Carol")
    path_id = secret(141)
    made = S.blind([S.scalar(secret(121)), None], ids[1:], [S.tlv({2: carol_dave}), S.tlv({6: path_id})])
    (carol_key, carol_blinded, carol_box, _), (dave_key, dave_blinded, dave_box, _) = made
    out += [f"  Carol's blinded node ID is her node ID: {str(carol_blinded == ids[1]).lower()}",
            f"  Dave's blinded node ID: {dave_blinded.hex()}",
            f"  Dave's blinded node ID is his node ID: {str(dave_blinded == ids[2]).lower()}"]
    pls = [prefixed(forward(1002000, 800180, bob_carol)), prefixed(S.tlv({10: carol_box, 12: carol_key})),
           prefixed(S.tlv({2: tu(1000000), 4: tu(800100), 10: dave_box, 18: tu(1000000)}))]
    onion = S.wrap(S.scalar(secret(161)), [ids[0], ids[1], dave_blinded], pls, payment_hash)
    payload, onion, _ = S.peel(bob, onion, payment_hash)
    out.append(report('Bob', payload, False))
    payload, onion, _ = S.peel(carol, onion, payment_hash)
    t = S.tlvs(payload)
    _, plain, next_key = S.unblind(carol, t[12], t[10])
    out.append(f'  Carol: introduction node, her data says forward over {scid_text(S.tlvs(plain)[2])}')
    out.append(f"  Carol passes on the path key Dave's hop was made with: {str(next_key == dave_key).lower()}")
    dave_onion = onion
    payload, onion, _ = S.peel(blinded_secret(dave, next_key), onion, payment_hash)
    out.append(report('Dave', payload, onion is None))
    t = S.tlvs(payload)
    _, plain, _ = S.unblind(dave, next_key, t[10])
    out.append(f"  Dave's data holds his path_id: {str(S.tlvs(plain)[6] == path_id).lower()}")
    out.append(f"  Bob opening Dave's data: {code(lambda: S.unblind(bob, next_key, t[10]))}")
    out.append(f"  Dave's own key opening the onion: {code(lambda: S.peel(dave, dave_onion, payment_hash))}")
    return out


def main():
    lines = transcript()
    want = FIXTURE.read_text().splitlines()
    assert lines == want, '\n'.join(f'{a!r} != {b!r}' for a, b in zip(lines, want) if a != b) or 'line counts differ'
    print(f'every BOLT 4 vector passes; {len(lines)} lines agree with {FIXTURE.name}')
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            got = subprocess.run([coax, 'run', EXAMPLE, *mode], cwd=REPO, capture_output=True, text=True, check=True)
            assert got.stdout.splitlines() == lines, f'coax run {" ".join(mode)} differs'
            print(f'coax run {" ".join(mode) or "(debug)"}: {len(lines)} lines agree')


if __name__ == '__main__':
    main()
