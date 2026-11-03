#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: a BOLT 12 reader and writer in plain Python, sharing no code with Coax)
"""An independent BOLT 12 reader and writer for Bitcoin chapter 115,
written from BOLT 12, BOLT 4's blinded paths and BIP-340 on ln_crypto.py's
secp256k1: the checksum-free Bech32 text with '+' joins, TLV streams of
offers (lno), invoice requests (lnr), invoices (lni) and payer proofs
(lnp), the Merkle tree with its tagged leaves, nonces and sorted branches,
Schnorr signatures, and payer proofs that leave fields out. It is slow and
not constant-time: an oracle for tests only.
"""
import hashlib
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import ln_crypto as C  # noqa: E402

CHARSET = 'qpzry9x8gf2tvdw0s3jn54khce6mua7l'
KINDS = {'lno': 'offer', 'lnr': 'invoice_request', 'lni': 'invoice', 'lnp': 'payer_proof'}
BITCOIN = bytes.fromhex('6fe28c0ab6f1b372c1a6a246ae63f74f931e8365e15a089c68d6190000000000')

OFFER_TYPES = {2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22}
INVREQ_TYPES = OFFER_TYPES | {0, 80, 82, 84, 86, 88, 89, 90, 91, 240}
INVOICE_TYPES = INVREQ_TYPES | {160, 162, 164, 166, 168, 170, 172, 174, 176}
PROOF_TYPES = (INVOICE_TYPES - {0}) | {241, 1001, 1002, 1003, 1004, 1005}
KNOWN = {'offer': OFFER_TYPES, 'invoice_request': INVREQ_TYPES, 'invoice': INVOICE_TYPES,
         'payer_proof': PROOF_TYPES}
RANGES = {'offer': [(1, 79), (10**9, 1999999999)],
          'invoice_request': [(0, 159), (240, 1000), (10**9, 2999999999)],
          'invoice': [(0, 239), (240, 1000), (10**9, 3999999999)],
          'payer_proof': [(1, 239), (240, 1000), (1001, 1005), (10**9, 3999999999)]}
INVOICE_FEATURES = {16}   # MPP/compulsory; 17 is its odd half


class Refused(Exception):
    pass


def th(tag, *parts):
    t = hashlib.sha256(tag).digest()
    return hashlib.sha256(t + t + b''.join(parts)).digest()


# ---- the text: Bech32 characters, no checksum, '+' joins

def text_decode(s):
    """(kind, bytes) or Refused('BAD_TEXT') / Refused('BAD_PREFIX')."""
    if s != s.lower() and s != s.upper():
        raise Refused('BAD_TEXT')
    s = s.lower()
    out = []
    i = 0
    while i < len(s):
        c = s[i]
        if c == '+':
            if not out or not out[-1].isalnum():
                raise Refused('BAD_TEXT')
            i += 1
            while i < len(s) and s[i] in ' \t\r\n':
                i += 1
            if i == len(s) or not s[i].isalnum():
                raise Refused('BAD_TEXT')
            continue
        if not c.isalnum():
            raise Refused('BAD_TEXT')
        out.append(c)
        i += 1
    s = ''.join(out)
    if '1' not in s:
        raise Refused('BAD_TEXT')
    hrp, data = s[:s.rindex('1')], s[s.rindex('1') + 1:]
    if hrp not in KINDS:
        raise Refused('BAD_PREFIX')
    acc = bits = 0
    b = bytearray()
    for c in data:
        if c not in CHARSET:
            raise Refused('BAD_TEXT')
        acc = (acc << 5) | CHARSET.index(c)
        bits += 5
        if bits >= 8:
            bits -= 8
            b.append((acc >> bits) & 255)
    if bits > 4 or acc & ((1 << bits) - 1):
        raise Refused('BAD_TEXT')
    return KINDS[hrp], bytes(b)


def text_encode(kind, data):
    hrp = {v: k for k, v in KINDS.items()}[kind]
    acc = bits = 0
    out = []
    for x in data:
        acc = (acc << 8) | x
        bits += 8
        while bits >= 5:
            bits -= 5
            out.append(CHARSET[(acc >> bits) & 31])
    if bits:
        out.append(CHARSET[(acc << (5 - bits)) & 31])
    return hrp + '1' + ''.join(out)


# ---- TLV streams

def bigsize(b, at):
    if at >= len(b):
        raise Refused('TRUNCATED')
    f = b[at]
    if f < 0xfd:
        return f, at + 1
    n = {0xfd: 2, 0xfe: 4, 0xff: 8}[f]
    if at + 1 + n > len(b):
        raise Refused('TRUNCATED')
    v = int.from_bytes(b[at + 1:at + 1 + n], 'big')
    if v < {2: 0xfd, 4: 0x10000, 8: 0x100000000}[n]:
        raise Refused('NOT_MINIMAL')
    return v, at + 1 + n


def put_bigsize(v):
    if v < 0xfd:
        return bytes([v])
    if v < 0x10000:
        return b'\xfd' + v.to_bytes(2, 'big')
    if v < 0x100000000:
        return b'\xfe' + v.to_bytes(4, 'big')
    return b'\xff' + v.to_bytes(8, 'big')


def records(b):
    """[(type, value, raw record bytes)] in order, or Refused."""
    out = []
    at = 0
    while at < len(b):
        start = at
        t, at = bigsize(b, at)
        if out and t <= out[-1][0]:
            raise Refused('ORDER')
        n, at = bigsize(b, at)
        if at + n > len(b):
            raise Refused('TRUNCATED')
        out.append((t, b[at:at + n], b[start:at + n]))
        at += n
    return out


def record(t, v):
    return put_bigsize(t) + put_bigsize(len(v)) + v


def tu(v, width=8):
    if len(v) > width:
        raise Refused('BAD_LENGTH')
    if v and v[0] == 0:
        raise Refused('NOT_MINIMAL')
    return int.from_bytes(v, 'big')


def put_tu(n):
    return n.to_bytes((n.bit_length() + 7) // 8, 'big')


def utf8(v):
    try:
        v.decode('utf-8', 'strict')
    except UnicodeDecodeError:
        raise Refused('BAD_UTF8')
    return v.decode()


def point(v):
    if C.decompress(v) is None:
        raise Refused('BAD_POINT')
    return v


def features(v, known):
    """The set bits, or Refused: not minimal, or an unknown even bit."""
    if v and v[0] == 0:
        raise Refused('NOT_MINIMAL')
    n = int.from_bytes(v, 'big')
    bits = {i for i in range(8 * len(v)) if n >> i & 1}
    if any(i % 2 == 0 and i not in known and i + 1 not in known for i in bits):
        raise Refused('BAD_FEATURES')
    return bits


def paths(v):
    """BOLT 4's blinded paths: [(first, path_key, [(blinded_id, data)])]."""
    out = []
    at = 0
    try:
        while at < len(v):
            if v[at] in (0, 1):
                first = v[at:at + 9]
                if len(first) < 9:
                    raise Refused('BAD_PATH')
                at += 9
            else:
                first = v[at:at + 33]
                if C.decompress(first) is None:
                    raise Refused('BAD_PATH')
                at += 33
            key = v[at:at + 33]
            if C.decompress(key) is None:
                raise Refused('BAD_PATH')
            at += 33
            if at >= len(v):
                raise Refused('BAD_PATH')
            hops = v[at]
            at += 1
            if hops == 0:
                raise Refused('BAD_PATH')
            hl = []
            for _ in range(hops):
                bid = v[at:at + 33]
                if C.decompress(bid) is None:
                    raise Refused('BAD_PATH')
                if at + 35 > len(v):
                    raise Refused('BAD_PATH')
                n = int.from_bytes(v[at + 33:at + 35], 'big')
                if at + 35 + n > len(v):
                    raise Refused('BAD_PATH')
                hl.append((bid, v[at + 35:at + 35 + n]))
                at += 35 + n
            out.append((first, key, hl))
    except IndexError:
        raise Refused('BAD_PATH')
    if not out:
        raise Refused('BAD_PATH')
    return out


def payinfo(v):
    out = []
    at = 0
    while at < len(v):
        if at + 28 > len(v):
            raise Refused('BAD_LENGTH')
        flen = int.from_bytes(v[at + 26:at + 28], 'big')
        if at + 28 + flen > len(v):
            raise Refused('BAD_LENGTH')
        f = v[at + 28:at + 28 + flen]
        out.append({'fee_base': int.from_bytes(v[at:at + 4], 'big'),
                    'fee_ppm': int.from_bytes(v[at + 4:at + 8], 'big'),
                    'cltv': int.from_bytes(v[at + 8:at + 10], 'big'),
                    'min': int.from_bytes(v[at + 10:at + 18], 'big'),
                    'max': int.from_bytes(v[at + 18:at + 26], 'big'),
                    'usable': not any(i % 2 == 0 for i in range(8 * flen)
                                      if int.from_bytes(f, 'big') >> i & 1)})
        at += 28 + flen
    return out


def fallbacks(v):
    out = []
    at = 0
    while at < len(v):
        if at + 3 > len(v):
            raise Refused('BAD_LENGTH')
        n = int.from_bytes(v[at + 1:at + 3], 'big')
        if at + 3 + n > len(v):
            raise Refused('BAD_LENGTH')
        out.append((v[at], v[at + 3:at + 3 + n]))
        at += 3 + n
    return out


# ---- the Merkle tree and signatures

def leaf_branch(raw, nonce):
    leaf = th(b'LnLeaf', raw)
    return branch(leaf, nonce)


def branch(a, b):
    return th(b'LnBranch', min(a, b) + max(a, b))


def split(n):
    """The root of n leaves joins the first 2^k (the largest power of two below n) and the rest."""
    k = 1
    while k * 2 < n:
        k *= 2
    return k


def tree(nodes):
    if len(nodes) == 1:
        return nodes[0]
    k = split(len(nodes))
    return branch(tree(nodes[:k]), tree(nodes[k:]))


def signed(rs):
    return [r for r in rs if not 240 <= r[0] <= 1000]


def merkle(rs):
    """The Merkle root of a stream's records outside the signature range."""
    rs = signed(rs)
    first = rs[0][2]
    return tree([leaf_branch(raw, th(b'LnNonce' + first, put_bigsize(t))) for t, _, raw in rs])


def sig_hash(kind, field, root):
    return th(b'lightning' + kind.encode() + field.encode(), root)


def lift(xonly):
    x = int.from_bytes(xonly, 'big')
    if x >= C.P:
        return None
    y = pow((pow(x, 3, C.P) + 7) % C.P, (C.P + 1) // 4, C.P)
    if (y * y - x ** 3 - 7) % C.P:
        return None
    return x, y if y % 2 == 0 else C.P - y


def schnorr_verify(msg, key33, sig):
    P = lift(key33[1:])
    r, s = int.from_bytes(sig[:32], 'big'), int.from_bytes(sig[32:], 'big')
    if P is None or r >= C.P or s >= C.N:
        return False
    e = int.from_bytes(th(b'BIP0340/challenge', sig[:32], key33[1:], msg), 'big') % C.N
    R = C.add(C.mul(s), C.mul(C.N - e, P))
    return R is not None and R[1] % 2 == 0 and R[0] == r


def schnorr_sign(k, msg, aux=bytes(32)):
    P = C.mul(k)
    d = k if P[1] % 2 == 0 else C.N - k
    t = (d ^ int.from_bytes(th(b'BIP0340/aux', aux), 'big')).to_bytes(32, 'big')
    n = int.from_bytes(th(b'BIP0340/nonce', t, P[0].to_bytes(32, 'big'), msg), 'big') % C.N
    R = C.mul(n)
    n = n if R[1] % 2 == 0 else C.N - n
    e = int.from_bytes(th(b'BIP0340/challenge', R[0].to_bytes(32, 'big'), P[0].to_bytes(32, 'big'), msg),
                       'big') % C.N
    return R[0].to_bytes(32, 'big') + ((n + e * d) % C.N).to_bytes(32, 'big')


# ---- reading

def fields(kind, data):
    """Every record checked for its range, the odd rule and its format: a dict by type."""
    rs = records(data)
    f = {}
    for t, v, raw in rs:
        if not any(a <= t <= b for a, b in RANGES[kind]):
            raise Refused('RANGE')
        if t not in KNOWN[kind]:
            if t % 2 == 0:
                raise Refused('UNKNOWN_EVEN')
            f[t] = v
            continue
        if t in (2,):
            if len(v) == 0 or len(v) % 32:
                raise Refused('BAD_LENGTH')
        elif t in (6, 10, 18, 89, 1005):
            utf8(v)
        elif t in (8, 14, 20, 82, 86, 164, 170):
            tu(v)
        elif t == 166:
            tu(v, 4)
        elif t == 12:
            features(v, set())
        elif t == 84:
            features(v, set())
        elif t == 174:
            features(v, INVOICE_FEATURES)
        elif t in (16, 90, 160):
            paths(v)
        elif t in (22, 88, 176):
            point(v)
        elif t in (80, 168):
            if len(v) != 32:
                raise Refused('BAD_LENGTH')
        elif t == 91:
            if len(v) < 1 or len(v) < 2 + v[0] or len(v) != 2 + v[0] + v[1 + v[0]]:
                raise Refused('BAD_LENGTH')
            ok = set(b'0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ-_.')
            if not set(v[1:1 + v[0]] + v[2 + v[0]:]) <= ok:
                raise Refused('BAD_VALUE')
        elif t == 162:
            payinfo(v)
        elif t == 172:
            fallbacks(v)
        elif t in (240, 241):
            if len(v) != 64:
                raise Refused('BAD_LENGTH')
        elif t == 1001:
            if len(v) != 32:
                raise Refused('BAD_LENGTH')
        elif t == 1002:
            at = 0
            while at < len(v):
                _, at = bigsize(v, at)
        elif t in (1003, 1004):
            if len(v) % 32:
                raise Refused('BAD_LENGTH')
        f[t] = v
    return rs, f


def check_offer(f):
    if 8 in f and tu(f[8]) == 0:
        raise Refused('BAD_AMOUNT')
    if 8 in f and 10 not in f:
        raise Refused('REQUIRED')
    if 6 in f and 8 not in f:
        raise Refused('REQUIRED')
    if 22 not in f and 16 not in f:
        raise Refused('REQUIRED')


def check_request(rs, f):
    if 0 not in f or 88 not in f:
        raise Refused('REQUIRED')
    if 240 not in f:
        raise Refused('REQUIRED')
    if not schnorr_verify(sig_hash('invoice_request', 'signature', merkle(rs)), f[88], f[240]):
        raise Refused('BAD_SIGNATURE')
    if 22 in f or 16 in f:
        check_offer(f)
        if 20 in f:
            if 86 not in f:
                raise Refused('BAD_QUANTITY')
            q, m = tu(f[86]), tu(f[20])
            if q == 0 or (m and q > m):
                raise Refused('BAD_QUANTITY')
        elif 86 in f:
            raise Refused('BAD_QUANTITY')
        if 8 not in f and 82 not in f:
            raise Refused('REQUIRED')
        if 8 in f and 6 not in f and 82 in f:
            if tu(f[82]) < tu(f[8]) * (tu(f[86]) if 86 in f else 1):
                raise Refused('BAD_AMOUNT')
    else:
        if 2 in f or 12 in f or 20 in f:
            raise Refused('FORBIDDEN')
        if 82 not in f:
            raise Refused('REQUIRED')


def check_invoice(rs, f):
    for t in (170, 164, 168, 176):
        if t not in f:
            raise Refused('REQUIRED')
    if 160 not in f or 162 not in f:
        raise Refused('REQUIRED')
    ps, pi = paths(f[160]), payinfo(f[162])
    if len(ps) != len(pi):
        raise Refused('BAD_PATH')
    if not any(p['usable'] for p in pi):
        raise Refused('BAD_PATH')
    if 240 not in f:
        raise Refused('REQUIRED')
    if not schnorr_verify(sig_hash('invoice', 'signature', merkle(rs)), f[176], f[240]):
        raise Refused('BAD_SIGNATURE')
    if 22 in f and f[22] != f[176]:
        raise Refused('WRONG_NODE')
    if 82 in f and tu(f[82]) != tu(f[170]):
        raise Refused('BAD_AMOUNT')


def decode(text):
    """(kind, records, fields) of a valid string, or Refused(code)."""
    kind, data = text_decode(text)
    rs, f = fields(kind, data)
    if kind == 'offer':
        check_offer(f)
    elif kind == 'invoice_request':
        check_request(rs, f)
    elif kind == 'invoice':
        check_invoice(rs, f)
    else:
        check_proof(rs, f)
    return kind, rs, f


# ---- payer proofs

def markers(types, included):
    """proof_omitted_tlvs for the invoice's non-signature types, 0 left out."""
    out = []
    prev_included = None
    for t in types:
        if t in included:
            prev_included = t
            continue
        if t == 0:
            continue
        if prev_included is not None:
            m = prev_included + 1
        elif not out:
            m = 1
        elif out[-1] == 239:
            m = 10**9
        else:
            m = out[-1] + 1
        out.append(m)
        prev_included = None
    return out


def proof_tree(nodes, missing, take):
    """nodes: a branch hash for an included leaf, or (None, hash) for an
    omitted one (the writer knows it) or None (the reader does not). Returns
    (hash or None, whether anything below is included). An omitted side next
    to an included one is appended to missing (writer) or taken from it (reader)."""
    if len(nodes) == 1:
        n = nodes[0]
        if isinstance(n, bytes):
            return n, True
        return (n[1] if n else None), False
    k = split(len(nodes))
    a, ina = proof_tree(nodes[:k], missing, take)
    b, inb = proof_tree(nodes[k:], missing, take)
    if ina and not inb:
        b = take() if take else (missing.append(b) or b)
    elif inb and not ina:
        a = take() if take else (missing.append(a) or a)
    if a is None or b is None:
        return None, ina or inb
    return branch(a, b), ina or inb


def proof_write(invoice_text, preimage, include, payer_key, note=None, explicit_empty=False):
    kind, rs, f = decode(invoice_text)
    assert kind == 'invoice' and 0 in f and 0 not in include
    first = rs[0][2]
    leaves, out_rs, omitted_types = [], [], []
    nonces = []
    for t, v, raw in signed(rs):
        nonce = th(b'LnNonce' + first, put_bigsize(t))
        if t in include:
            leaves.append(leaf_branch(raw, nonce))
            nonces.append(nonce)
            out_rs.append((t, v))
        else:
            leaves.append((None, leaf_branch(raw, nonce)))
    missing = []
    root, _ = proof_tree(leaves, missing, None)
    assert root == merkle(rs)
    om = markers([t for t, _, _ in signed(rs)], include)
    body = [(t, v) for t, v in out_rs] + [(240, f[240]), (1001, preimage)]
    if om or explicit_empty:
        body.append((1002, b''.join(put_bigsize(m) for m in om)))
    body += [(1003, b''.join(missing)), (1004, b''.join(nonces))]
    if note is not None:
        body.append((1005, note.encode()))
    body.sort()
    data = b''.join(record(t, v) for t, v in body)
    proof_rs = records(data)
    sig = schnorr_sign(payer_key, sig_hash('payer_proof', 'proof_signature', merkle(proof_rs)))
    body.append((241, sig))
    body.sort()
    return text_encode('payer_proof', b''.join(record(t, v) for t, v in body))


def check_proof(rs, f):
    for t in (88, 168, 176, 240, 1001, 1003, 1004, 241):
        if t not in f:
            raise Refused('REQUIRED')
    if hashlib.sha256(f[1001]).digest() != f[168]:
        raise Refused('BAD_PREIMAGE')
    om = []
    at = 0
    while at < len(f.get(1002, b'')):
        m, at = bigsize(f[1002], at)
        om.append(m)
    included = [t for t, _, _ in rs if t < 240 or 10**9 <= t]
    prev = 0
    for i, m in enumerate(om):
        if m == 0 or not (1 <= m <= 239 or 10**9 <= m <= 3999999999) or m in included:
            raise Refused('BAD_OMITTED')
        if i and m <= om[i - 1]:
            raise Refused('BAD_OMITTED')
        ok = (m - 1 in included) or m == prev + 1 or (prev == 239 and m == 10**9)
        if not ok:
            raise Refused('BAD_OMITTED')
        prev = m
    nonces = [f[1004][i:i + 32] for i in range(0, len(f[1004]), 32)]
    if len(nonces) != len(included):
        raise Refused('BAD_LEAVES')
    missing = [f[1003][i:i + 32] for i in range(0, len(f[1003]), 32)]
    raw = {t: r for t, _, r in rs}
    order = sorted([(t, 'in') for t in included] + [(0, 'out')] + [(m, 'out') for m in om])
    nodes = []
    j = 0
    for t, what in order:
        if what == 'in':
            nodes.append(leaf_branch(raw[t], nonces[j]))
            j += 1
        else:
            nodes.append(None)
    pool = list(missing)

    def take():
        if not pool:
            raise Refused('BAD_MISSING')
        return pool.pop(0)
    root, _ = proof_tree(nodes, None, take)
    if pool or root is None:
        raise Refused('BAD_MISSING')
    if not schnorr_verify(sig_hash('invoice', 'signature', root), f[176], f[240]):
        raise Refused('BAD_SIGNATURE')
    if not schnorr_verify(sig_hash('payer_proof', 'proof_signature', merkle(rs)), f[88], f[241]):
        raise Refused('BAD_SIGNATURE')


# ---- writing offers, requests and invoices

def stream(pairs):
    return b''.join(record(t, v) for t, v in sorted(pairs))


def sign_stream(kind, pairs, key):
    data = stream(pairs)
    sig = schnorr_sign(key, sig_hash(kind, 'signature', merkle(records(data))))
    return stream(list(pairs) + [(240, sig)])


def request(offer_text, metadata, payer_key, amount=None, quantity=None, note=None):
    kind, rs, f = decode(offer_text)
    assert kind == 'offer'
    pairs = [(t, v) for t, v, _ in rs] + [(0, metadata), (88, C.compress(C.mul(payer_key)))]
    if amount is not None:
        pairs.append((82, put_tu(amount)))
    if quantity is not None:
        pairs.append((86, put_tu(quantity)))
    if note is not None:
        pairs.append((89, note.encode()))
    return text_encode('invoice_request', sign_stream('invoice_request', pairs, payer_key))


def invoice(request_text, node_key, path, created_at, payment_hash, amount, payinfo_bytes):
    kind, rs, f = decode(request_text)
    assert kind == 'invoice_request'
    pairs = [(t, v) for t, v, _ in rs if t != 240]
    pairs += [(160, path), (162, payinfo_bytes), (164, put_tu(created_at)), (168, payment_hash),
              (170, put_tu(amount)), (176, C.compress(C.mul(node_key)))]
    return text_encode('invoice', sign_stream('invoice', pairs, node_key))
