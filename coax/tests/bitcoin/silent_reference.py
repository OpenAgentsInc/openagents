#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: a second BIP-352 implementation, outside Coax and apart from the BIP's reference.py)
"""BIP-352 silent payments in plain Python, for Bitcoin chapter 97. Its
own secp256k1 arithmetic (Jacobian coordinates), Bech32m and input
parsing; it imports neither secp256k1lab nor the BIP's reference.py.
make_silent_vectors.py decides every published vector with it and with
the pinned reference.py, and requires the two to agree.

  python3 -I coax/tests/bitcoin/silent_reference.py   # its own self-checks
"""
import hashlib, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import script_reference as S  # noqa: E402  (ripemd160)

P = 2 ** 256 - 2 ** 32 - 977
N = 0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141
G = (0x79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798,
     0x483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8)
K_MAX = 2323
# BIP-341's NUMS point H: its x is SHA-256 of G's uncompressed encoding.
NUMS = bytes.fromhex('50929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0')
CHARSET = 'qpzry9x8gf2tvdw0s3jn54khce6mua7l'
BECH32M = 0x2bc830a3


# ---- the curve: affine points as (x, y), None for infinity
def _jadd(a, b):
    if a is None: return b
    if b is None: return a
    x1, y1, z1 = a; x2, y2, z2 = b
    z1s, z2s = z1 * z1 % P, z2 * z2 % P
    u1, u2 = x1 * z2s % P, x2 * z1s % P
    s1, s2 = y1 * z2s * z2 % P, y2 * z1s * z1 % P
    if u1 == u2:
        if s1 != s2: return None
        return _jdouble(a)
    h, r = (u2 - u1) % P, (s2 - s1) % P
    h2 = h * h % P; h3 = h2 * h % P
    x3 = (r * r - h3 - 2 * u1 * h2) % P
    return (x3, (r * (u1 * h2 - x3) - s1 * h3) % P, h * z1 * z2 % P)


def _jdouble(a):
    if a is None: return None
    x, y, z = a
    if y == 0: return None
    s = 4 * x * y * y % P; m = 3 * x * x % P
    x3 = (m * m - 2 * s) % P
    return (x3, (m * (s - x3) - 8 * pow(y, 4, P)) % P, 2 * y * z % P)


def _affine(j):
    if j is None: return None
    zi = pow(j[2], -1, P)
    return (j[0] * zi * zi % P, j[1] * zi * zi * zi % P)


def mul(k, pt):
    k %= N
    if pt is None or k == 0: return None
    r, q = None, (pt[0], pt[1], 1)
    while k:
        if k & 1: r = _jadd(r, q)
        q = _jdouble(q); k >>= 1
    return _affine(r)


def add(a, b):
    return _affine(_jadd(None if a is None else (a[0], a[1], 1), None if b is None else (b[0], b[1], 1)))


def neg(a): return None if a is None else (a[0], (P - a[1]) % P)


def base(k): return mul(k, G)


def lift_x(xb):
    x = int.from_bytes(xb, 'big')
    if x >= P: return None
    c = (x * x * x + 7) % P
    y = pow(c, (P + 1) // 4, P)
    if y * y % P != c: return None
    return (x, y if y % 2 == 0 else P - y)


def decompress(b):
    if len(b) != 33 or b[0] not in (2, 3): return None
    pt = lift_x(b[1:])
    if pt is None: return None
    return pt if pt[1] % 2 == b[0] % 2 else neg(pt)


def ser_p(pt): return bytes([2 | (pt[1] & 1)]) + pt[0].to_bytes(32, 'big')


def xonly(pt): return pt[0].to_bytes(32, 'big')


def tagged(tag, b):
    t = hashlib.sha256(tag.encode()).digest()
    return hashlib.sha256(t + t + b).digest()


def hash160(b): return S.ripemd160(hashlib.sha256(b).digest())


def scalar(b):
    """A 32-byte hash as a scalar, or None when it is 0 or not below n."""
    v = int.from_bytes(b, 'big')
    return v if 0 < v < N else None


# ---- addresses
def _polymod(values):
    gen = [0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3]
    c = 1
    for v in values:
        top = c >> 25
        c = (c & 0x1ffffff) << 5 ^ v
        for i in range(5):
            if (top >> i) & 1: c ^= gen[i]
    return c


def _expand(hrp): return [ord(x) >> 5 for x in hrp] + [0] + [ord(x) & 31 for x in hrp]


def _regroup(data, frombits, tobits, pad):
    acc = bits = 0; out = []
    for v in data:
        acc = acc << frombits | v; bits += frombits
        while bits >= tobits:
            bits -= tobits; out.append(acc >> bits & (1 << tobits) - 1)
    if pad:
        if bits: out.append(acc << (tobits - bits) & (1 << tobits) - 1)
    elif bits >= frombits or acc << (tobits - bits) & (1 << tobits) - 1:
        return None
    return out


def raw(hrp, version, body, constant=BECH32M):
    """Bech32m (or, with constant 1, Bech32) of a version word and bytes."""
    data = [version] + _regroup(body, 8, 5, True)
    c = _polymod(_expand(hrp) + data + [0] * 6) ^ constant
    return hrp + '1' + ''.join(CHARSET[d] for d in data + [c >> 5 * (5 - i) & 31 for i in range(6)])


def encode(scan, spend, testnet=False):
    """sp1q... (or tsp1q...): Bech32m of version 0 and ser_P(B_scan) ||
    ser_P(B_m)."""
    return raw('tsp' if testnet else 'sp', 0, scan + spend)


# The reasons an address is refused, in silent.coax's order.
ADDRESS_REASONS = ['ok', 'not Bech32m', 'not sp or tsp', 'version 31', 'not 66 bytes', 'not a public key']


def decode(text):
    """(reason, testnet, version, B_scan bytes, B_m bytes). Version 0 is
    exactly 66 bytes; 1 to 30 read the first 66 and ignore the rest; 31
    is reserved and refused. At most 1023 characters."""
    bad = lambda r: (r, False, 0, b'', b'')
    if not 8 <= len(text) <= 1023 or any(ord(c) < 33 or ord(c) > 126 for c in text): return bad(1)
    if text.lower() != text and text.upper() != text: return bad(1)
    text = text.lower(); at = text.rfind('1')
    if at < 1 or at + 7 > len(text) or any(c not in CHARSET for c in text[at + 1:]): return bad(1)
    hrp, data = text[:at], [CHARSET.index(c) for c in text[at + 1:]]
    if _polymod(_expand(hrp) + data) != BECH32M: return bad(1)
    if hrp not in ('sp', 'tsp'): return bad(2)
    data = data[:-6]
    if not data: return bad(4)
    version = data[0]
    if version == 31: return bad(3)
    body = _regroup(data[1:], 5, 8, False)
    if body is None or len(body) < 66 or (version == 0 and len(body) != 66): return bad(4)
    body = bytes(body[:66])
    if decompress(body[:33]) is None or decompress(body[33:]) is None: return bad(5)
    return (0, hrp == 'tsp', version, body[:33], body[33:])


def label(b_scan, m):
    """hash_BIP0352/Label(ser_256(b_scan) || ser_32(m)), as a scalar."""
    return scalar(tagged('BIP0352/Label', b_scan.to_bytes(32, 'big') + m.to_bytes(4, 'big')))


def labeled_spend(b_scan, b_spend_point, m):
    return add(b_spend_point, base(label(b_scan, m)))


# ---- inputs
def witness_items(w):
    """A serialized witness stack: a count, then each item with its length."""
    def compact(at):
        f = w[at]
        if f < 0xfd: return f, at + 1
        n = {0xfd: 2, 0xfe: 4, 0xff: 8}[f]
        return int.from_bytes(w[at + 1:at + 1 + n], 'little'), at + 1 + n
    if not w: return []
    count, at = compact(0); items = []
    for _ in range(count):
        n, at = compact(at); items.append(w[at:at + n]); at += n
    return items


def input_key(script_sig, witness, prevout):
    """The input's public key, or None when it is not one of BIP-352's
    inputs (or its key is uncompressed, or it uses H)."""
    items = witness_items(witness)
    if len(prevout) == 25 and prevout[:3] == b'\x76\xa9\x14' and prevout[23:] == b'\x88\xac':
        # P2PKH: the last 33 bytes of the scriptSig that hash to the key
        # hash, read from the end, even in a non-standard scriptSig.
        for end in range(len(script_sig), 32, -1):
            key = script_sig[end - 33:end]
            if hash160(key) == prevout[3:23] and decompress(key) is not None: return decompress(key)
        return None
    if len(prevout) == 23 and prevout[:2] == b'\xa9\x14' and prevout[22] == 0x87:
        # P2SH-P2WPKH: the scriptSig pushes a P2WPKH program; the key is
        # the last witness item.
        redeem = script_sig[1:]
        if len(redeem) == 22 and redeem[:2] == b'\x00\x14' and items: return decompress(items[-1])
        return None
    if len(prevout) == 22 and prevout[:2] == b'\x00\x14':
        return decompress(items[-1]) if items else None
    if len(prevout) == 34 and prevout[:2] == b'\x51\x20':
        if not items: return None
        if len(items) > 1 and items[-1][:1] == b'\x50': items = items[:-1]   # the annex
        if len(items) > 1 and items[-1][1:33] == NUMS: return None         # a script path from H
        return lift_x(prevout[2:])
    return None


def is_taproot(prevout): return len(prevout) == 34 and prevout[:2] == b'\x51\x20'


def outpoint(txid_hex, vout): return bytes.fromhex(txid_hex)[::-1] + vout.to_bytes(4, 'little')


def input_hash(outpoints, a_point):
    """hash_BIP0352/Inputs(outpoint_L || ser_P(A)), outpoint_L the smallest."""
    return scalar(tagged('BIP0352/Inputs', min(outpoints) + ser_p(a_point)))


def shared_tweak(ecdh, k):
    return scalar(tagged('BIP0352/SharedSecret', ser_p(ecdh) + k.to_bytes(4, 'big')))


# ---- the sender
def send(keys, outpoints, recipients):
    """keys: (secret, is_taproot) for each of BIP-352's inputs; recipients:
    addresses. Returns (outputs in order, shared secrets by B_scan, a) or
    None when sending fails: no inputs, a = 0 or a group over K_max."""
    if not keys: return None
    a = 0
    for d, taproot in keys:
        if taproot and base(d)[1] % 2: d = N - d
        a = (a + d) % N
    if a == 0: return None
    ih = input_hash(outpoints, base(a))
    groups = {}
    for text in recipients:
        reason, _, _, scan, spend = decode(text)
        assert reason == 0
        groups.setdefault(scan, []).append(spend)
    if any(len(g) > K_MAX for g in groups.values()): return None
    outputs, secrets = [], {}
    for scan, spends in groups.items():
        ecdh = mul(ih * a % N, decompress(scan))
        secrets[scan] = ser_p(ecdh)
        for k, spend in enumerate(spends):
            outputs.append(xonly(add(decompress(spend), base(shared_tweak(ecdh, k)))))
    return outputs, secrets, a


# ---- the receiver
def scan(b_scan, b_spend_point, a_point, ih, outputs, labels):
    """Returns (tweak point, shared secret, found): found lists (x-only key,
    secret tweak) in the order found. labels: the m values the wallet
    uses, which BIP-352 asks to include the change label 0.
    Follows BIP-352's loop: for each output, P_k, then output - P_k and
    -output - P_k among the labels."""
    table = {}
    for m in labels:
        t = label(b_scan, m); table[ser_p(base(t))] = t
    tweak = mul(ih, a_point)
    ecdh = mul(b_scan, tweak)
    left = list(outputs); found = []; k = 0
    while k < K_MAX:
        t = shared_tweak(ecdh, k)
        pk = add(b_spend_point, base(t))
        hit = None
        for out in left:
            if xonly(pk) == out: hit = (out, out, t); break
            o = lift_x(out)
            for cand in (add(o, neg(pk)), add(neg(o), neg(pk))):
                if cand is not None and ser_p(cand) in table:
                    hit = (out, xonly(add(pk, cand)), (t + table[ser_p(cand)]) % N); break
            if hit: break
        if hit is None: break
        left.remove(hit[0]); found.append((hit[1], hit[2])); k += 1
    return tweak, ecdh, found


def eligible(prevouts, outputs_spk):
    """Whether a transaction must be scanned: a Taproot output, one of
    BIP-352's inputs, and no input spending a witness version above 1."""
    for spk in prevouts:
        if 4 <= len(spk) <= 42 and 0x52 <= spk[0] <= 0x60 and spk[1] == len(spk) - 2: return False
    return any(is_taproot(s) for s in outputs_spk)


# ---- BIP-340 signing, for the spend
def sign(d, msg, aux):
    pt = base(d)
    if pt[1] % 2: d = N - d
    t = (d ^ int.from_bytes(tagged('BIP0340/aux', aux), 'big')).to_bytes(32, 'big')
    k0 = int.from_bytes(tagged('BIP0340/nonce', t + xonly(pt) + msg), 'big') % N
    r = base(k0)
    k = k0 if r[1] % 2 == 0 else N - k0
    e = int.from_bytes(tagged('BIP0340/challenge', xonly(r) + xonly(pt) + msg), 'big') % N
    return xonly(r) + ((k + e * d) % N).to_bytes(32, 'big')


def verify(pub, msg, sig):
    pt = lift_x(pub); r = int.from_bytes(sig[:32], 'big'); s = int.from_bytes(sig[32:], 'big')
    if pt is None or r >= P or s >= N: return False
    e = int.from_bytes(tagged('BIP0340/challenge', sig[:32] + pub + msg), 'big') % N
    rr = add(base(s), neg(mul(e, pt)))
    return rr is not None and rr[1] % 2 == 0 and rr[0] == r


if __name__ == '__main__':
    assert ser_p(base(1)) == bytes.fromhex('0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798')
    assert add(base(2), base(3)) == base(5) and add(base(5), neg(base(5))) is None and mul(N - 1, G) == neg(G)
    assert hashlib.sha256(bytes.fromhex('04' + '%064x%064x' % G)).digest() == NUMS
    s, v = 7, 9
    a = encode(ser_p(base(s)), ser_p(base(v)))
    assert decode(a) == (0, False, 0, ser_p(base(s)), ser_p(base(v))) and len(a) == 116
    assert len(encode(ser_p(base(s)), ser_p(base(v)), True)) == 117
    print('silent_reference.py: self-checks pass')
