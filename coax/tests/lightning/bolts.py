#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: reads the BOLTs' published test vectors outside Coax)
"""The pinned BOLT texts for Bitcoin chapters 104 to 108, 111, 112, 114 and 115, read and checked.

The BOLTs at lightning/bolts revision 19146282 are pinned in
coax/tests/vectors/upstream/bolts/. Every read checks the file's SHA-256
against its pin, so a changed text cannot change a vector silently.
"""
import hashlib
import json
import re
from pathlib import Path

HERE = Path(__file__).resolve().parent
UP = HERE.parent / 'vectors/upstream/bolts'
PINS = {
    '01-messaging.md': '5801219aabcf854cf0a6dd4e98759b4351cbca238588061c680574912f6ef918',
    '03-transactions.md': '27ffbf1ae436f257bff3f255b4d1e4d4bbf8597fb1e2533d7ff9b66f4fe38d43',
    '04-onion-routing.md': '79f4e06d334a200f62b6053cc891f2c4dd710e7a43e1c7b3691848dc4c460070',
    'bolt04/blinded-onion-message-onion-test.json': 'e5e8ac8a42d3d0ce3bb70c12c5bbd76408e022a63322dcf32b00ddd97d051f96',
    'bolt04/blinded-payment-onion-test.json': 'ce74ab58d12031eebf080c3fde4e259baf9c66bbd1b1cf45f098669ad58b5dab',
    'bolt04/onion-error-test.json': '374800b8d0dd21920299f4a998c4278f9d66f3a00b5e7b396765d38a50f2ea11',
    'bolt04/onion-test.json': 'bff01ffc728c3fecbee133eedfdaf39a26984f35cf7e76ed80f14d6e78cb8d56',
    'bolt04/route-blinding-test.json': 'd2c32e0260e6cb189246ec93bfc606459ef7c8abcad666fab4c496fe9644ef66',
    '07-routing-gossip.md': '4b7cb122c9ba5c065d779671b52b9d2ec6be18cf34dddd046a8714a12af4a5c8',
    'bolt07/extended-queries.json': '9927ae2c9e1a68ad0f99830f2e1c318a50ff1b17d51e37eb0111cdc56ba16f79',
    '08-transport.md': '85a1d6402adb653416c5a48f3b4c8eac2d8964621c41cb1bb6e09b8e06fbde46',
    '09-features.md': '39ead9fa5743104a3e66e0f6c342e34d64966dc943dcbcc337969e543e3e3cbe',
    '11-payment-encoding.md': 'c659da1bc08091f7fe691cd4ce661e578dfbe1f77a0b9cd9c53d1839010ae029',
    'bolt11/invoice-test.json': '5d6af32b7dfb84722e4e8a94d633d1597b9504a836c32046219b82c1edb7824a',
    '12-offer-encoding.md': 'f85fbc0577536c82d94f713076221c76c32218b7728ccfed56962d59ccdcee89',
    'bolt12/format-string-test.json': 'a88e3098b3afee8cd31bf1091d76f2f265f5f0e2e397f56fc2eef8da7e3f131d',
    'bolt12/offers-test.json': 'd30991bbd31105f87b583e1d4168a9beb8b7f9cf2b23c729597428e0dce832c7',
    'bolt12/payer-proof-test.json': 'bad5f365f02c3bd82fcb5d600ba941592b211e715103b134f4deed599a650ad9',
    'bolt12/signature-test.json': '03ddd14f5593bea0a97e5b3cbe3af9fe215d38443ae192a01bb72a840858d6a4',
}


def text(name):
    data = (UP / name).read_bytes()
    assert hashlib.sha256(data).hexdigest() == PINS[name], f'{name} differs from its pin'
    return data.decode()


def section(body, start, end):
    """The text from the heading line `start` up to the heading line `end`."""
    a = body.index(start)
    b = body.index(end, a + len(start)) if end else len(body)
    return body[a:b]


# ---- BOLT 1

def bigsize():
    """Appendix A: the decoding rows, each marked if it is also an encoding row."""
    body = text('01-messaging.md')
    blocks = re.findall(r'```json\n(.*?)\n```', section(body, '## Appendix A', '## Appendix B'), re.S)
    decode, encode = json.loads(blocks[0]), json.loads(blocks[1])
    encoded = {(r['value'], r['bytes']) for r in encode}
    for r in decode:
        r['encode'] = (r['value'], r['bytes']) in encoded and 'exp_error' not in r
    assert all((r['value'], r['bytes']) in {(d['value'], d['bytes']) for d in decode} for r in encode)
    return decode


def _streams(chunk):
    out = []
    for m in re.finditer(r'1\. (Valid|Invalid) stream: 0x([0-9a-f ]*)\n2\. (?:Reason|Explanation|Values): ([^\n]*)', chunk):
        out.append((m.group(1) == 'Valid', m.group(2).replace(' ', ''), m.group(3).strip()))
    return out


def _values(text):
    """`tlv1` `amount_msat`=0 and the like, as a dict."""
    fields = dict(re.findall(r'`(\w+)`=(\S+)', text))
    record = int(re.match(r'`tlv(\d)`', text).group(1))
    return record, fields


def tlv():
    """Appendix B: (stream, namespace, ok, reason, record, fields). Namespace
    0 is every namespace."""
    body = section(text('01-messaging.md'), '## Appendix B', '## Appendix C')
    rows = []
    groups = re.split(r'\n(?=[^\n]*following TLV stream)', body)
    for g in groups:
        head = ' '.join(g.split('\n')[:2])
        if 'namespace `n1`' in head or '`n1` namespace' in head:
            ns = 1
        elif 'namespace `n2`' in head:
            ns = 2
        elif 'any namespace' in head or 'either namespace' in head:
            ns = 0
        else:
            continue
        for ok, stream, why in _streams(g.split('\n### ')[0]):
            record, fields = (_values(why) if ok and ns == 1 else (0, {}))
            rows.append((stream, ns, ok, why, record, fields))
    return rows


def init_extension():
    """Appendix C: (message, ok, why)."""
    body = section(text('01-messaging.md'), '## Appendix C', '## Appendix D')
    valid, invalid = body.split('The following `init` messages are invalid:')
    rows = []
    for part, ok in ((valid, True), (invalid, False)):
        for m in re.finditer(r'- `0x([0-9a-f]+)`: ([^\n]*)', part):
            rows.append((m.group(1), ok, m.group(2)))
    return rows


def features():
    """BOLT 9's table: bit pair -> name."""
    names = {}
    for m in re.finditer(r'^\| (\d+)/(\d+) +\| `(\w+)`', text('09-features.md'), re.M):
        names[int(m.group(1)) // 2] = m.group(3)
    return names


def scid(text):
    block, tx, out = (int(x) for x in text.split('x'))
    return (block << 40) | (tx << 16) | out


# ---- BOLT 8

def _hex(value):
    return value.strip().removeprefix('0x').lower()


def handshakes():
    """Appendix A's initiator and responder tests: one dict per test with
    name, initiator, rs_pub, ls_priv, e_priv, act1, act2, act3, keys (the
    64 bytes sk || rk for the initiator, rk || sk for the responder) and
    error (BOLT 8's code without its ACTn_ prefix)."""
    body = section(text('08-transport.md'), '## Initiator Tests', '## Message Encryption Tests')
    tests = []
    for side, chunk in (('initiator', section(body, '## Initiator Tests', '## Responder Tests')),
                        ('responder', section(body, '## Responder Tests', None))):
        for block in re.split(r'\n\s*name: ', chunk)[1:]:
            lines = block.split('\n')
            t = {'name': lines[0].strip(), 'initiator': side == 'initiator', 'rs_pub': '', 'act1': '',
                 'act2': '', 'act3': '', 'keys': '', 'error': ''}
            act = 0
            for line in lines[1:]:
                line = line.strip()
                if line.startswith('# Act One'):
                    act = 1
                elif line.startswith('# Act Two'):
                    act = 2
                elif line.startswith('# Act Three'):
                    act = 3
                m = re.match(r'^(rs\.pub|ls\.priv|e\.priv|input|output)\s*[:=]\s*(.*)$', line)
                if not m:
                    continue
                key, value = m.groups()
                if key == 'rs.pub':
                    t['rs_pub'] = _hex(value)
                elif key == 'ls.priv':
                    t['ls_priv'] = _hex(value)
                elif key == 'e.priv':
                    t['e_priv'] = _hex(value)
                elif value.startswith('ERROR'):
                    t['error'] = re.match(r'ERROR \((ACT\d_)?(\w+)', value).group(2)
                elif value.startswith(('sk,rk=', 'rk,sk=')):
                    a, b = value[6:].split(',')
                    t['keys'] = _hex(a) + _hex(b)
                else:
                    # An initiator's first output is act one, before any "# Act One" line.
                    t[f'act{act or 1}'] = _hex(value)
            tests.append(t)
    return tests


def messages():
    """The message encryption test: ck, sk, rk and the six outputs by index."""
    body = section(text('08-transport.md'), '## Message Encryption Tests', '# Acknowledgments')
    keys = dict(re.findall(r'^\s*(ck|sk|rk)=0x([0-9a-f]+)', body, re.M))
    outputs = [(int(i), h) for i, h in re.findall(r'^\s*output (\d+): 0x([0-9a-f]+)', body, re.M)]
    return keys, outputs


# ---- BOLT 3

def _int(value):
    value = value.strip()
    return int(value, 16) if value.startswith('0x') else int(value)


def generation():
    """Appendix D's generation tests: (name, seed, index, output)."""
    body = section(text('03-transactions.md'), '## Generation Tests', '## Storage Tests')
    return [(name.strip(), _hex(seed), _int(index), _hex(out)) for name, seed, index, out in
            re.findall(r'name: ([^\n]*)\n\s*seed: (\S+)\n\s*I: (\S+)\n\s*output: (\S+)', body)]


def storage():
    """Appendix D's storage tests: (name, [(index, secret, ok), ...])."""
    body = section(text('03-transactions.md'), '## Storage Tests', '# Appendix E')
    tests = []
    for block in re.split(r'\n\s*name: ', body)[1:]:
        name = block.split('\n')[0].strip()
        steps = [(_int(i), _hex(s), out == 'OK') for i, s, out in
                 re.findall(r'I: (\S+)\n\s*secret: (\S+)\n\s*output: (\S+)', block)]
        tests.append((name, steps))
    return tests


def keys():
    """Appendix E: the shared secrets and points, and each derived key."""
    body = section(text('03-transactions.md'), '# Appendix E', '# Appendix F')
    return {k: _hex(v) for k, v in re.findall(
        r'^\s*(base_secret|per_commitment_secret|base_point|per_commitment_point|localpubkey|localprivkey|'
        r'revocationpubkey|revocationprivkey): (0x[0-9a-f]+)', body, re.M)}


# ---- BOLT 4

def bolt4_json(name):
    """One of the bolt04/*.json vector files, parsed."""
    return json.loads(text(f'bolt04/{name}'))


def error_trace():
    """The "Returning Errors" trace of BOLT 4's Test Vector section: the
    failure message and its padded payload, the um key, the packet before
    obfuscation, and for each node from 4 down to 0 its shared secret,
    ammag key and the packet it sends back."""
    body = section(text('04-onion-routing.md'), '## Returning Errors\n\nThe test vectors', '## Returning success')
    one = lambda name: re.search(name + r' = (\w+)', body).group(1)
    hops = [(int(n), ss, ammag, packet) for ss, ammag, n, packet in re.findall(
        r'shared_secret = (\w+)\n\s*ammag_key = (\w+)\n\s*stream = \w+\n\s*error packet for node (\d): (\w+)', body)]
    return {'payload': one('payload'), 'um_key': one('um_key'), 'raw': one('raw_error_packet'), 'hops': hops}


# ---- BOLT 11

def bolt11_json():
    """bolt11/invoice-test.json: every example invoice and whether it is valid."""
    return json.loads(text('bolt11/invoice-test.json'))


def bolt11_examples():
    """What BOLT 11's Examples section states for each invoice: the signing
    key and payment secret every example uses, and for each invoice its
    timestamp, and the signature, recovery flag, signed data and its
    SHA-256 where its "Signature breakdown" gives them."""
    body = section(text('11-payment-encoding.md'), '# Examples\n', '# Examples of Invalid Invoices')
    key = re.search(r'`priv_key`=`(\w+)`', body).group(1)
    secret = re.search(r'`payment_secret`=`(\w+)`', body).group(1)
    out = {}
    for block in body.split('\n> ###')[1:]:
        invoice = re.search(r'^> (ln\w+)$', block, re.M | re.I).group(1)
        one = lambda pattern: (lambda m: m.group(1) if m else '')(re.search(pattern, block))
        out[invoice] = {
            'timestamp': int(one(r'timestamp \((\d+)\)') or 0),
            'signature': one(r'`(\w{128})` hex of signature data'),
            'recovery': int(one(r'`(\d)` \(int\) recovery flag') or -1),
            'data': one(r'`(\w+)` hex of data for signing'),
            'digest': one(r'`(\w{64})` hex of SHA256 of the preimage'),
        }
    return key, secret, out


# ---- BOLT 12

def bolt12_json(name):
    """One of bolt12/format-string-test.json, offers-test.json,
    signature-test.json and payer-proof-test.json."""
    return json.loads(text(f'bolt12/{name}-test.json'))


# ---- BOLT 3 transactions

def _fields(body):
    """`name: value` lines (and `INTERNAL: name: value`) as a dict of strings."""
    out = {}
    for name, value in re.findall(r'^\s*(?:INTERNAL: )?(\w+): (\S+)$', body, re.M):
        out.setdefault(name, value)
    return out


def funding():
    """Appendix B: the coinbase it spends, its key, the funding keys and
    amounts, and the funding transaction."""
    body = section(text('03-transactions.md'), '# Appendix B', '# Appendix C')
    f = _fields(body)
    one = lambda pattern: re.search(pattern, body).group(1)
    return {
        'coinbase': one(r'Block 1 coinbase transaction: (\w+)'),
        'coinbase_privkey': one(r'Block 1 coinbase privkey: (\w{64})01'),
        'local_funding_pubkey': f['local_funding_pubkey'], 'remote_funding_pubkey': f['remote_funding_pubkey'],
        'witness_script': one(r'# funding witness script = (\w+)'),
        'input_txid': one(r'input txid: (\w+)'),
        'input_index': int(one(r'input index: (\d+)')),
        'input_sat': int(one(r'input satoshis: (\d+)')),
        'funding_sat': int(one(r'funding satoshis: (\d+)')),
        'feerate': int(one(r'# feerate_per_kw: (\d+)')),
        'change_sat': int(one(r'change satoshis: (\d+)')),
        'tx': one(r'funding tx: (\w+)'),
        'txid': one(r'# txid: (\w+)'),
    }


def commitment_params():
    """Appendix C's shared parameters: the channel, the seven HTLCs, the
    secrets in its comment and the keys it lists. Keys and secrets are hex;
    secrets drop Bitcoin's trailing 01."""
    body = section(text('03-transactions.md'), '# Appendix C', '    name: simple commitment tx')
    f = _fields(body)
    one = lambda pattern: re.search(pattern, body).group(1)
    htlcs = [{'offered': f'htlc {i} direction: local->remote' in body,
              'amount_msat': int(one(rf'htlc {i} amount_msat: (\d+)')),
              'expiry': int(one(rf'htlc {i} expiry: (\d+)')),
              'preimage': one(rf'htlc {i} payment_preimage: (\w+)')} for i in range(7)]
    secret = lambda k: f[k][:64]
    return {
        'funding_txid': f['funding_tx_id'], 'funding_index': int(f['funding_output_index']),
        'funding_sat': int(f['funding_amount_satoshi']), 'commitment_number': int(f['commitment_number']),
        'to_self_delay': int(f['local_delay']), 'dust_limit': int(f['local_dust_limit_satoshi']),
        'htlcs': htlcs,
        'local_funding_privkey': secret('local_funding_privkey'), 'remote_funding_privkey': secret('remote_funding_privkey'),
        'local_payment_basepoint_secret': secret('local_payment_basepoint_secret'),
        'remote_revocation_basepoint_secret': secret('remote_revocation_basepoint_secret'),
        'local_delayed_payment_basepoint_secret': secret('local_delayed_payment_basepoint_secret'),
        'remote_payment_basepoint_secret': secret('remote_payment_basepoint_secret'),
        'local_per_commitment_secret': secret('x_local_per_commitment_secret'),
        **{k: f[k] for k in ('local_payment_basepoint', 'remote_payment_basepoint', 'local_htlc_basepoint',
                             'remote_htlc_basepoint', 'local_funding_pubkey', 'remote_funding_pubkey',
                             'local_htlcpubkey', 'remote_htlcpubkey', 'local_delayedpubkey',
                             'local_revocation_pubkey', 'local_per_commitment_point')},
    }


def commitments():
    """Appendix C's tests: name, to_local_msat, to_remote_msat, feerate,
    the commitment transaction, and the HTLC transactions in output order
    as (kind, HTLC number, hex)."""
    body = section(text('03-transactions.md'), '    name: simple commitment tx', '# Appendix D')
    tests = []
    for block in re.split(r'\n    name: ', '\n' + body)[1:]:
        one = lambda pattern: re.search(pattern, block).group(1)
        tests.append({
            'name': block.split('\n')[0].strip(),
            'to_local_msat': int(one(r'to_local_msat: (\d+)')),
            'to_remote_msat': int(one(r'to_remote_msat: (\d+)')),
            'feerate': int(one(r'local_feerate_per_kw: (\d+)')),
            'tx': one(r'output commit_tx: (\w+)'),
            'htlc_txs': [(k, int(n), h) for k, n, h in
                         re.findall(r'htlc_(success|timeout)_tx \(htlc #(\d+)\): (\w+)', block)],
        })
    return tests


def anchors():
    """Appendix F's tests, as BOLT 3 lists them in JSON."""
    body = section(text('03-transactions.md'), '# Appendix F', '# Appendix G')
    return json.loads(re.search(r'```yaml\n(.*?)\n```', body, re.S).group(1))


# ---- BOLT 7

def bolt7_aliases():
    """The node_announcement test vectors: aliases with forbidden
    characters, signed with the key 0x11...11."""
    body = section(text('07-routing-gossip.md'), '### Test vectors', '## The `channel_update` Message')
    key = re.search(r'priv=0x([0-9a-f]{64})', body).group(1)
    rows = json.loads(re.search(r'```json\n(.*?)\n```', body, re.S).group(1))
    return int(key, 16), rows


def bolt7_queries():
    """bolt07/extended-queries.json: query and reply messages."""
    return json.loads(text('bolt07/extended-queries.json'))
