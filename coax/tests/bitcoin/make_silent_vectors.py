#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: decides every BIP-352 vector with silent_reference.py, outside Coax)
"""Writes coax/tests/bitcoin/silent_vectors.coax from BIP-352's pinned
send_and_receive_test_vectors.json (coax/tests/vectors/upstream/bip352/,
bips revision 5aadddb5), for Bitcoin chapter 97:

  python3 -I coax/tests/bitcoin/make_silent_vectors.py           # check the file is current
  python3 -I coax/tests/bitcoin/make_silent_vectors.py --write   # rewrite it

Every published case is decided by silent_reference.py and checked
against the published result: the public key each input gives (or that
it gives none), the sender's key sum, shared secrets and outputs (one of
the published sets), and the receiver's addresses, input key sum, tweak,
shared secret, found outputs with their secret tweaks, and the BIP-340
signature each found output's key makes. coax/tests/bitcoin/silent.py
also runs the BIP's own reference.py over the same file. Then it
constructs addresses for the version rules the vectors don't reach, keys
from a seed by a BIP-32 derivation, labels and a scan.
"""
import hashlib, json, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import silent_reference as R  # noqa: E402

VEC = HERE.parent / 'vectors'
FILE = 'upstream/bip352/send_and_receive_test_vectors.json'
OUT = HERE / 'silent_vectors.coax'
MSG = hashlib.sha256(b'message').digest()
AUX = hashlib.sha256(b'random auxiliary data').digest()


def published():
    raw = (VEC / FILE).read_bytes()
    pin = next(e['sha256'] for e in json.loads((VEC / 'manifest.json').read_text())['files'] if e['path'] == FILE)
    assert hashlib.sha256(raw).hexdigest() == pin, FILE
    return json.loads(raw)


def h(b): return b.hex() if b is not None else ''


def build():
    inputs, cases, recipients, receives = [], [], [], []
    for c in published():
        assert len(c['sending']) == 1
        s = c['sending'][0]; given, want = s['given'], s['expected']
        first = len(inputs); keys, outpoints, pubs = [], [], []
        for v in given['vin']:
            spk, sig, wit = (bytes.fromhex(x) for x in (v['prevout']['scriptPubKey']['hex'], v['scriptSig'], v['txinwitness']))
            pub = R.input_key(sig, wit, spk)
            outpoints.append(R.outpoint(v['txid'], v['vout']))
            if pub is not None:
                pubs.append(R.ser_p(pub)); keys.append((int(v['private_key'], 16), R.is_taproot(spk)))
            inputs.append(dict(txid=v['txid'], vout=v['vout'], script_sig=v['scriptSig'], witness=v['txinwitness'],
                               prevout=v['prevout']['scriptPubKey']['hex'], key=v['private_key'], pub=h(R.ser_p(pub) if pub else None)))
        assert [p.hex() for p in pubs] == want['input_pub_keys']
        rfirst = len(recipients); addresses = []
        for r in given['recipients']:
            reason, _, _, scan, spend = R.decode(r['address'])
            assert reason == 0 and scan.hex() == r['scan_pub_key'] and spend.hex() == r['spend_pub_key']
            recipients.append(dict(address=r['address'], scan=r['scan_pub_key'], spend=r['spend_pub_key'], count=r.get('count', 1)))
            addresses += [r['address']] * r.get('count', 1)
        sent = R.send(keys, outpoints, addresses)
        sets = [''.join(x) for x in want['outputs']]
        if sent is None:
            assert want['outputs'] == [[]]
            outputs, a = [], 0
        else:
            outputs, secrets, a = sent
            assert any(sorted(o.hex() for o in outputs) == sorted(x) for x in want['outputs']), c['comment']
            for k, r in enumerate(given['recipients']):
                assert secrets[bytes.fromhex(r['scan_pub_key'])].hex() == want['shared_secrets'][k]
            assert '%064x' % a == want['input_private_key_sum']
        cases.append(dict(comment=c['comment'], first=first, inputs=len(given['vin']), rfirst=rfirst,
                          recipients=len(given['recipients']), sets=' '.join(sets),
                          secrets=''.join(x or '' for x in want['shared_secrets']),
                          a_sum=want.get('input_private_key_sum', ''), sent=len(outputs)))
        for r in c['receiving']:
            g, e = r['given'], r['expected']
            for v, w in zip(g['vin'], given['vin']):
                public = lambda d: {k: x for k, x in d.items() if k != 'private_key'}
                assert public(w) == public(v)
            b_scan, b_spend = int(g['key_material']['scan_priv_key'], 16), int(g['key_material']['spend_priv_key'], 16)
            scan_pt, spend_pt = R.base(b_scan), R.base(b_spend)
            addrs = [R.encode(R.ser_p(scan_pt), R.ser_p(spend_pt))]
            addrs += [R.encode(R.ser_p(scan_pt), R.ser_p(R.labeled_spend(b_scan, spend_pt, m))) for m in g['labels']]
            assert addrs == e['addresses']
            outs = [bytes.fromhex(o) for o in g['outputs']]
            a_point = None
            for p in pubs: a_point = R.add(a_point, R.decompress(p))
            tweak = shared = None; found = []
            if a_point is not None:
                assert R.ser_p(a_point).hex() == e['input_pub_key_sum']
                ih = R.input_hash(outpoints, a_point)
                tweak, shared, found = R.scan(b_scan, spend_pt, a_point, ih, outs, g['labels'])
            assert h(R.ser_p(tweak) if tweak else None) == (e['tweak'] or '')
            assert h(R.ser_p(shared) if shared else None) == (e['shared_secret'] or '')
            rows = []
            for key, t in found:
                sig = R.sign((b_spend + t) % R.N, MSG, AUX)
                assert R.verify(key, MSG, sig)
                rows.append(dict(pub_key=key.hex(), priv_key_tweak='%064x' % t, signature=sig.hex()))
            if 'outputs' in e:
                by_key = lambda o: o['pub_key']
                assert sorted(rows, key=by_key) == sorted(e['outputs'], key=by_key), c['comment']
                n = len(e['outputs'])
            else:
                assert len(rows) == e['n_outputs']
                n = e['n_outputs']
            receives.append(dict(case=len(cases) - 1, scan_key=g['key_material']['scan_priv_key'],
                                 spend_key=g['key_material']['spend_priv_key'],
                                 labels=''.join('%08x' % m for m in g['labels']), addresses=' '.join(e['addresses']),
                                 outputs=''.join(g['outputs']), tweak=e['tweak'] or '', shared=e['shared_secret'] or '',
                                 a_point=e.get('input_pub_key_sum', ''), n=n,
                                 found=''.join(o['pub_key'] + o['priv_key_tweak'] + o['signature'] for o in e.get('outputs', []))))
    return inputs, cases, recipients, receives


def constructed():
    """Addresses for the version rules: (what, address, reason, testnet,
    version, scan, spend), the reason silent_reference.decode gives."""
    scan, spend = R.ser_p(R.base(0x51)), R.ser_p(R.base(0x352))
    body = scan + spend
    off = bytes([2]) + (5).to_bytes(32, 'big')        # x = 5 is not on the curve
    assert R.decompress(off) is None
    flip = lambda a: a[:20] + ('q' if a[20] != 'q' else 'p') + a[21:]
    rows = [('version 0', R.raw('sp', 0, body)), ('version 0 on a testnet', R.raw('tsp', 0, body)),
            ('upper case', R.raw('sp', 0, body).upper()),
            ('version 1 with a byte more', R.raw('sp', 1, body + b'\x01')),
            ('version 30 with 34 bytes more', R.raw('sp', 30, body + bytes(34))),
            ('version 31', R.raw('sp', 31, body)), ('version 0 with a byte more', R.raw('sp', 0, body + b'\x00')),
            ('version 0 with a byte less', R.raw('sp', 0, body[:65])), ('version 1 with a byte less', R.raw('sp', 1, body[:65])),
            ('a Bech32 checksum', R.raw('sp', 0, body, 1)), ('the segwit prefix bc', R.raw('bc', 0, body)),
            ('mixed case', R.raw('sp', 0, body)[:10] + R.raw('sp', 0, body)[10:].upper()),
            ('a character changed', flip(R.raw('sp', 0, body))),
            ('a scan key off the curve', R.raw('sp', 0, off + spend)),
            ('over 1023 characters', R.raw('sp', 1, body + bytes(600))),
            ('exactly 1023 characters', R.raw('sp', 1, body + bytes(567))),
            ('1024 characters on a testnet', R.raw('tsp', 1, body + bytes(567))),
            ('no data', words('sp', [])), ('version 31 alone', words('sp', [31])),
            ('version 0 with a spare zero group', words('sp', [0] + R._regroup(body, 8, 5, True) + [0])),
            ('version 1 with five spare zero bits', words('sp', [1] + R._regroup(body + bytes(4), 8, 5, True) + [0])),
            ('version 0 with a padding bit set', words('sp', [0] + R._regroup(body, 8, 5, True)[:-1] + [R._regroup(body, 8, 5, True)[-1] | 2]))]
    assert len(rows[-7][1]) == 1023 and len(rows[-6][1]) == 1024
    out = []
    for what, text in rows:
        reason, testnet, version, s, m = R.decode(text)
        out.append(dict(what=what, address=text, reason=reason, testnet=testnet, version=version, scan=s.hex(), spend=m.hex()))
    return out


def words(hrp, data):
    """Bech32m of 5-bit groups as they are, padding and all."""
    c = R._polymod(R._expand(hrp) + data + [0] * 6) ^ R.BECH32M
    return hrp + '1' + ''.join(R.CHARSET[d] for d in data + [c >> 5 * (5 - i) & 31 for i in range(6)])


def bip32_key(seed, path):
    """BIP-32's private derivation from a seed along path."""
    import hmac
    I = hmac.new(b'Bitcoin seed', seed, hashlib.sha512).digest()
    k, c = int.from_bytes(I[:32], 'big'), I[32:]
    for i in path:
        data = (b'\0' + k.to_bytes(32, 'big') if i >= 2 ** 31 else R.ser_p(R.base(k))) + i.to_bytes(4, 'big')
        I = hmac.new(c, data, hashlib.sha512).digest()
        k, c = (int.from_bytes(I[:32], 'big') + k) % R.N, I[32:]
    return k.to_bytes(32, 'big')


def keys_labels_scan():
    """Constructed known answers the published vectors don't reach: keys
    from a seed at BIP-352's paths, labels with m's high bytes set, and a
    scan whose outputs are found out of order and far apart."""
    H = 2 ** 31
    keys = []
    for seed, coin, account in ((bytes(range(16)), 1, 2), (bytes([0x51]) * 32, 0, H - 1)):
        keys.append(dict(seed=seed.hex(), coin=coin, account=account,
                         scan=bip32_key(seed, [H + 352, H + coin, H + account, H + 1, 0]).hex(),
                         spend=bip32_key(seed, [H + 352, H + coin, H + account, H, 0]).hex()))
    b_scan = int.from_bytes(hashlib.sha256(b'silent scan key').digest(), 'big') % R.N
    labels = []
    for m in (0x01020304, 0xffffffff, 7):
        t = R.label(b_scan, m)
        labels.append(dict(m=m, tweak=t.to_bytes(32, 'big').hex(), point=R.ser_p(R.base(t)).hex()))
    b_spend = int.from_bytes(hashlib.sha256(b'silent spend key').digest(), 'big') % R.N
    tw = R.base(int.from_bytes(hashlib.sha256(b'silent tweak').digest(), 'big') % R.N)
    ecdh = R.mul(b_scan, tw)
    t = [R.shared_tweak(ecdh, k) for k in range(7)]
    pk = [R.add(R.base(b_spend), R.base(x)) for x in t]
    # The outputs found out of order and far apart, in a list of 400.
    at = [0, 17, 9, 8, 16, 18, 370]
    outs = [bytes(32)] * 400
    for k, i in enumerate(at): outs[i] = R.xonly(pk[k])
    ih = 1  # the tweak is tw itself: input_hash A with A = tw, input_hash 1
    assert R.scan(b_scan, R.base(b_spend), tw, ih, outs, [])[2] == [(R.xonly(pk[k]), t[k]) for k in range(7)]
    # A labeled output first, then an unlabeled one.
    lab = R.label(b_scan, 7)
    two = [R.xonly(R.add(pk[0], R.base(lab))), R.xonly(pk[1])]
    found = R.scan(b_scan, R.base(b_spend), tw, ih, two, [7])[2]
    assert found == [(two[0], (t[0] + lab) % R.N), (two[1], t[1])]
    scan = dict(b_scan=b_scan.to_bytes(32, 'big').hex(), spend=R.ser_p(R.base(b_spend)).hex(), tweak=R.ser_p(tw).hex(),
                keys=''.join(R.xonly(p).hex() for p in pk), tweaks=''.join(x.to_bytes(32, 'big').hex() for x in t),
                labeled=two[0].hex(), labeled_tweak=found[0][1].to_bytes(32, 'big').hex())
    return keys, labels, scan


def render(inputs, cases, recipients, receives, addresses, extra):
    L = ['// Generated by make_silent_vectors.py from BIP-352\'s pinned',
         '// send_and_receive_test_vectors.json (coax/tests/vectors/upstream/bip352/,',
         '// listed in SOURCES.md) and from addresses it constructs. Published values,',
         '// copied; silent_reference.py agrees with each. Test keys only.',
         '// An input: its outpoint (the txid as displayed), scriptSig, serialized',
         '// witness stack, the scriptPubKey it spends, its secret key, and the',
         '// compressed public key BIP-352 takes from it, empty when it takes none.',
         'SlInput :: struct { txid: string; vout: int; script_sig: string; witness: string; prevout: string; key: string; pub: string; }',
         f'SL_INPUTS :: [{len(inputs)}] SlInput.[']
    for i in inputs:
        L.append(f'  SlInput.{{txid="{i["txid"]}", vout={i["vout"]}, script_sig="{i["script_sig"]}", witness="{i["witness"]}", '
                 f'prevout="{i["prevout"]}", key="{i["key"]}", pub="{i["pub"]}"}},')
    L += ['];', '// A recipient: its address, the keys in it, and how many outputs it gets.',
          'SlRecipient :: struct { address: string; scan: string; spend: string; count: int; }',
          f'SL_RECIPIENTS :: [{len(recipients)}] SlRecipient.[']
    for r in recipients:
        L.append(f'  SlRecipient.{{address="{r["address"]}", scan="{r["scan"]}", spend="{r["spend"]}", count={r["count"]}}},')
    L += ['];', '// A case: its inputs and recipients (runs of the tables above), the output',
          '// sets the sender may make (x-only keys, one set per space; one empty set when',
          '// sending fails), the shared secret per recipient, the key sum a, and how',
          '// many outputs the sender makes.',
          'SlCase :: struct { comment: string; first: int; inputs: int; rfirst: int; recipients: int; sets: string; secrets: string; a_sum: string; sent: int; }',
          f'SL_CASES :: [{len(cases)}] SlCase.[']
    for c in cases:
        L.append(f'  SlCase.{{comment="{c["comment"]}", first={c["first"]}, inputs={c["inputs"]}, rfirst={c["rfirst"]}, '
                 f'recipients={c["recipients"]}, sets="{c["sets"]}", secrets="{c["secrets"]}", a_sum="{c["a_sum"]}", sent={c["sent"]}}},')
    L += ['];', '// A receiver of a case: its secret keys, labels (4 bytes each), addresses',
          '// (unlabeled first, one per space), the transaction\'s Taproot outputs, the',
          '// tweak input_hash A, the shared secret, A, how many outputs it finds, and',
          '// each one found: its x-only key, secret tweak and BIP-340 signature.',
          'SlReceive :: struct { case_: int; scan_key: string; spend_key: string; labels: string; addresses: string; outputs: string;',
          '  tweak: string; shared: string; a_point: string; n: int; found: string; }',
          f'SL_RECEIVES :: [{len(receives)}] SlReceive.[']
    for r in receives:
        L.append(f'  SlReceive.{{case_={r["case"]}, scan_key="{r["scan_key"]}", spend_key="{r["spend_key"]}", labels="{r["labels"]}", '
                 f'addresses="{r["addresses"]}", outputs="{r["outputs"]}", tweak="{r["tweak"]}", shared="{r["shared"]}", '
                 f'a_point="{r["a_point"]}", n={r["n"]}, found="{r["found"]}"}},')
    L += ['];', '// A constructed address and what decoding it gives: the reason (an SLA_*',
          '// code), the network, the version and the two keys.',
          'SlAddressCase :: struct { what: string; address: string; reason: int; testnet: bool; version: int; scan: string; spend: string; }',
          f'SL_ADDRESSES :: [{len(addresses)}] SlAddressCase.[']
    for a in addresses:
        L.append(f'  SlAddressCase.{{what="{a["what"]}", address="{a["address"]}", reason={a["reason"]}, '
                 f'testnet={"true" if a["testnet"] else "false"}, version={a["version"]}, scan="{a["scan"]}", spend="{a["spend"]}"}},')
    keys, labels, scan = extra
    L += ['];', '// Keys from a seed at BIP-352\'s paths, by a BIP-32 derivation in Python:',
          '// b_scan at m/352\'/coin\'/account\'/1\'/0, b_spend at m/352\'/coin\'/account\'/0\'/0.',
          'SlKeysCase :: struct { seed: string; coin: int; account: int; scan: string; spend: string; }',
          f'SL_KEYS :: [{len(keys)}] SlKeysCase.[']
    for k in keys:
        L.append(f'  SlKeysCase.{{seed="{k["seed"]}", coin={k["coin"]}, account={k["account"]}, scan="{k["scan"]}", spend="{k["spend"]}"}},')
    L += ['];', '// Labels of the scan key in SL_SCAN: m, the tweak and its point.',
          'SlLabelCase :: struct { m: int; tweak: string; point: string; }',
          f'SL_LABELS :: [{len(labels)}] SlLabelCase.[']
    for l in labels:
        L.append(f'  SlLabelCase.{{m={l["m"]}, tweak="{l["tweak"]}", point="{l["point"]}"}},')
    L += ['];', '// A constructed receiver: b_scan, B_spend, the tweak, P_0 to P_6 (x-only)',
          '// and t_0 to t_6, and P_0 plus label 7\'s point (x-only) with its tweak.',
          'SlScanCase :: struct { b_scan: string; spend: string; tweak: string; keys: string; tweaks: string; labeled: string; labeled_tweak: string; }',
          f'SL_SCAN :: SlScanCase.{{b_scan="{scan["b_scan"]}", spend="{scan["spend"]}", tweak="{scan["tweak"]}", keys="{scan["keys"]}", '
          f'tweaks="{scan["tweaks"]}", labeled="{scan["labeled"]}", labeled_tweak="{scan["labeled_tweak"]}"}};']
    return '\n'.join(L) + '\n'


def main():
    inputs, cases, recipients, receives = build()
    addresses = constructed()
    text = render(inputs, cases, recipients, receives, addresses, keys_labels_scan())
    if '--write' in sys.argv: OUT.write_text(text)
    assert OUT.read_text() == text, 'silent_vectors.coax is not current: run with --write'
    print(f'silent_vectors.coax matches: {len(cases)} cases, {len(inputs)} inputs, {len(recipients)} recipients, '
          f'{len(receives)} receivers, {len(addresses)} constructed addresses')


if __name__ == '__main__':
    main()
