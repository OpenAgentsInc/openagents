#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: reads the published PSBT cases from the pinned BIPs and Bitcoin Core's rpc_psbt.json)
"""The published PSBT cases of Bitcoin chapter 94, read from the pinned
BIP-174, BIP-370 and BIP-371 texts and Bitcoin Core's rpc_psbt.json, each
checked against its pinned SHA-256 in manifest.json. Imported by
make_psbt_vectors.py and psbt.py."""
import hashlib, json, re
from pathlib import Path

HERE = Path(__file__).resolve().parent
UP = HERE.parent / 'vectors' / 'upstream'


def pinned(name):
    b = (UP / name).read_bytes()
    m = json.loads((UP.parent / 'manifest.json').read_text())
    pin = next(e['sha256'] for e in m['files'] if e['path'] == 'upstream/' + name)
    assert hashlib.sha256(b).hexdigest() == pin, name
    return b


def bip_cases(name):
    """(kind, title, bytes) for each case of a BIP's test vectors, kind one of
    invalid, valid, signer (valid, but its signer checks refuse) and
    locktime=N (N is the lock time, or none). The PSBT is its base64 text."""
    text = pinned(name).decode()
    text = text[text.index('==Test Vectors=='):]
    text = text[:text.index('==Rationale==')]
    out, kind, title = [], None, None
    for line in text.split('\n'):
        if line.startswith('The following are invalid'): kind = 'invalid'
        elif line.startswith('The following are valid'): kind = 'valid'
        elif line.startswith('Fails Signer checks'): kind = 'signer'
        elif line.startswith('The private keys in the tests below'): kind = None
        elif line.startswith('The timelock for the following PSBTs should be computed to be'):
            kind = 'locktime=' + re.search(r'computed to be (\d+)', line).group(1)
        elif line.startswith('The timelock for the following PSBTs cannot be computed'): kind = 'locktime=none'
        elif line.startswith('* Case:'): title = line[len('* Case:'):].strip()
        m = re.search(r'Base64 String: <pre>([^<]+)</pre>', line)
        if m and kind is not None:
            out.append((kind, title, m.group(1).strip()))
    return out


def core_cases():
    """rpc_psbt.json: (kind, message, base64 text) for invalid, invalid_with_msg and valid."""
    d = json.loads(pinned('bitcoin-core/rpc_psbt.json'))
    out = [('invalid', None, x) for x in d['invalid']]
    out += [('invalid', msg, x) for x, msg in d['invalid_with_msg']]
    out += [('valid', None, x) for x in d['valid']]
    return out, d


def bip174_workflow():
    """BIP-174's worked example after "The private keys in the tests
    below": the master key, the creator's outputs and inputs, the updater's
    scripts, transactions and key paths, the two signers' keys, and each
    role's published result (hex, in the order the text gives them)."""
    text = pinned('bip174/bip-0174.mediawiki').decode()
    text = text[text.index('The private keys in the tests below'):text.index('==Rationale==')]
    tt = lambda s: re.findall(r'<tt>([^<]*)</tt>', s)
    part = lambda a, b: text[text.index(a):text.index(b)]
    w = dict(tprv=re.search(r'Extended Private Key: <pre>(\w+)</pre>', text).group(1))
    creator = part('A creator creating', 'must create this PSBT')
    vals = tt(creator)
    w['outputs'] = [(vals[0], vals[1]), (vals[2], vals[3])]
    w['inputs'] = [(vals[4], int(vals[5])), (vals[6], int(vals[7]))]
    up = part('an updater with only the following', 'Must create this PSBT')
    w['redeem'] = tt(part('* Redeem Scripts:', '* Witness Scripts:'))
    w['witness'] = tt(part('* Witness Scripts:', '* Previous Transactions:'))
    w['prev'] = re.findall(r'<pre>([0-9a-f]+)</pre>', part('* Previous Transactions:', '* Public Keys'))
    keys = tt(up[up.index('* Public Keys'):])
    w['paths'] = list(zip(keys[0::2], keys[1::2]))
    s1 = part('a signer that supports SIGHASH_ALL', 'Given the above updated PSBT, a signer with the following keys')
    s2 = part('Given the above updated PSBT, a signer with the following keys', 'Given both of the above PSBTs')
    w['signers'] = [list(zip(tt(s)[0::2], tt(s)[1::2])) for s in (s1, s2)]
    w['results'] = re.findall(r'Bytes in Hex: <pre>([0-9a-f]+)</pre>', text)
    return w


def all_cases():
    """Every distinct published PSBT once, with where it is published:
    (kind, source, title or Core's message, base64 text)."""
    seen, out = {}, []
    for name, src in (('bip174/bip-0174.mediawiki', 'BIP-174'), ('bip370/bip-0370.mediawiki', 'BIP-370'),
                      ('bip371/bip-0371.mediawiki', 'BIP-371')):
        for kind, title, b in bip_cases(name):
            if b not in seen:
                seen[b] = len(out)
                out.append([kind, src, title, b])
    core, _ = core_cases()
    for kind, msg, b in core:
        if b in seen:
            k = seen[b]
            want = 'invalid' if kind == 'invalid' else ('valid', 'signer')
            assert out[k][0] == kind or out[k][0] in want or out[k][0].startswith('locktime'), (out[k][:3], kind)
            if msg: out[k][2] = msg; out[k][1] += ' and Core'
        else:
            seen[b] = len(out)
            out.append([kind, 'Core', msg, b])
    return out
