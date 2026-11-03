#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: a BOLT 11 reader and writer in plain Python, sharing no code with Coax)
"""An independent BOLT 11 invoice reader and writer for Bitcoin chapter 114,
written from BOLT 11 and BIP-173 on ln_crypto.py's secp256k1: Bech32
without the 90-character limit, the human-readable part and its amount,
the tagged fields, and the recoverable ECDSA signature with RFC 6979
nonces. It is slow and not constant-time: an oracle for tests only.
"""
import hashlib
import hmac
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import ln_crypto as C  # noqa: E402

CHARSET = 'qpzry9x8gf2tvdw0s3jn54khce6mua7l'
PREFIXES = {'bc': 'mainnet', 'tb': 'testnet', 'tbs': 'signet', 'bcrt': 'regtest'}
MULTIPLIER = {'m': 10**8, 'u': 10**5, 'n': 10**2}   # msat per unit; p is a tenth of a msat
KNOWN_FEATURES = {8, 14, 16, 24, 36, 48}             # BOLT 9's bits with context 9, even halves
TYPES = {'p': 1, 's': 16, 'd': 13, 'm': 27, 'n': 19, 'h': 23, 'x': 6, 'c': 24, 'f': 9, 'r': 3, '9': 5}


class Refused(Exception):
    pass


# ---- Bech32 (BIP-173), with no length limit

def polymod(values):
    gen = [0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3]
    chk = 1
    for v in values:
        top = chk >> 25
        chk = (chk & 0x1ffffff) << 5 ^ v
        for i in range(5):
            if (top >> i) & 1:
                chk ^= gen[i]
    return chk


def hrp_expand(hrp):
    return [ord(c) >> 5 for c in hrp] + [0] + [ord(c) & 31 for c in hrp]


def bech32_decode(text):
    if any(ord(c) < 33 or ord(c) > 126 for c in text):
        raise Refused('BAD_BECH32')
    if text.lower() != text and text.upper() != text:
        raise Refused('BAD_BECH32')
    text = text.lower()
    sep = text.rfind('1')
    if sep < 1 or len(text) - sep - 1 < 6:
        raise Refused('BAD_BECH32')
    hrp = text[:sep]
    try:
        words = [CHARSET.index(c) for c in text[sep + 1:]]
    except ValueError:
        raise Refused('BAD_BECH32')
    if polymod(hrp_expand(hrp) + words) != 1:
        raise Refused('BAD_BECH32')
    return hrp, words[:-6]


def bech32_encode(hrp, words):
    pm = polymod(hrp_expand(hrp) + words + [0] * 6) ^ 1
    return hrp + '1' + ''.join(CHARSET[w] for w in words + [(pm >> 5 * (5 - i)) & 31 for i in range(6)])


def to_bytes(words):
    """5-bit words to bytes; the leftover bits at the end are dropped."""
    acc = 0
    for w in words:
        acc = acc << 5 | w
    bits = 5 * len(words)
    return (acc >> (bits % 8)).to_bytes(bits // 8, 'big')


def to_words(data):
    """Bytes to 5-bit words, padded with zero bits."""
    bits = 8 * len(data)
    n = (bits + 4) // 5
    acc = int.from_bytes(data, 'big') << (5 * n - bits)
    return [(acc >> 5 * (n - 1 - i)) & 31 for i in range(n)]


def int_words(v):
    out = []
    while v:
        out.insert(0, v & 31)
        v >>= 5
    return out


def word_int(words):
    v = 0
    for w in words:
        v = v << 5 | w
    return v


# ---- ECDSA: RFC 6979 signing, verification and recovery

def rfc6979(key, z):
    x = key.to_bytes(32, 'big')
    h = z.to_bytes(32, 'big')
    k = b'\0' * 32
    v = b'\1' * 32
    k = hmac.new(k, v + b'\0' + x + h, hashlib.sha256).digest()
    v = hmac.new(k, v, hashlib.sha256).digest()
    k = hmac.new(k, v + b'\1' + x + h, hashlib.sha256).digest()
    v = hmac.new(k, v, hashlib.sha256).digest()
    while True:
        v = hmac.new(k, v, hashlib.sha256).digest()
        t = int.from_bytes(v, 'big')
        if 1 <= t < C.N:
            return t
        k = hmac.new(k, v + b'\0', hashlib.sha256).digest()
        v = hmac.new(k, v, hashlib.sha256).digest()


def sign(key, digest):
    """r || s with low s, and the recovery ID."""
    z = int.from_bytes(digest, 'big') % C.N
    k = rfc6979(key, int.from_bytes(digest, 'big') % C.N)
    R = C.mul(k)
    r = R[0] % C.N
    s = pow(k, -1, C.N) * (z + r * key) % C.N
    rid = (R[1] & 1) | (2 if R[0] >= C.N else 0)
    if s > C.N // 2:
        s = C.N - s
        rid ^= 1
    return r.to_bytes(32, 'big') + s.to_bytes(32, 'big'), rid


def recover(digest, sig, rid):
    r = int.from_bytes(sig[:32], 'big')
    s = int.from_bytes(sig[32:], 'big')
    if not (0 < r < C.N and 0 < s < C.N) or rid > 3:
        return None
    x = r + (C.N if rid & 2 else 0)
    if x >= C.P:
        return None
    R = C.decompress(bytes([2 + (rid & 1)]) + x.to_bytes(32, 'big'))
    if R is None:
        return None
    z = int.from_bytes(digest, 'big') % C.N
    rinv = pow(r, -1, C.N)
    Q = C.add(C.mul(s * rinv % C.N, R), C.mul((-z * rinv) % C.N))
    return None if Q is None else C.compress(Q)


def verify(digest, key, sig):
    Q = C.decompress(key)
    r = int.from_bytes(sig[:32], 'big')
    s = int.from_bytes(sig[32:], 'big')
    if Q is None or not (0 < r < C.N and 0 < s < C.N):
        return False
    z = int.from_bytes(digest, 'big') % C.N
    w = pow(s, -1, C.N)
    X = C.add(C.mul(z * w % C.N), C.mul(r * w % C.N, Q))
    return X is not None and X[0] % C.N == r


# ---- base58 and segwit addresses for fallbacks

B58 = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'


def base58check(version, payload):
    data = bytes([version]) + payload
    data += hashlib.sha256(hashlib.sha256(data).digest()).digest()[:4]
    n = int.from_bytes(data, 'big')
    out = ''
    while n:
        n, d = divmod(n, 58)
        out = B58[d] + out
    return '1' * (len(data) - len(data.lstrip(b'\0'))) + out


def segwit_address(hrp, version, program):
    words = [version] + to_words(program)
    pm = polymod(hrp_expand(hrp) + words + [0] * 6) ^ (1 if version == 0 else 0x2bc830a3)
    return hrp + '1' + ''.join(CHARSET[w] for w in words + [(pm >> 5 * (5 - i)) & 31 for i in range(6)])


def fallback_address(network, version, program):
    test = network != 'mainnet'
    if version == 17:
        return base58check(111 if test else 0, program)
    if version == 18:
        return base58check(196 if test else 5, program)
    hrp = {'mainnet': 'bc', 'testnet': 'tb', 'signet': 'tb', 'regtest': 'bcrt'}[network]
    return segwit_address(hrp, version, program)


# ---- the reader

def amount_msat(text):
    if not text:
        return None
    mult = text[-1] if text[-1] in 'munp' else ''
    digits = text[:-1] if mult else text
    if not digits or not digits.isdigit():
        raise Refused('BAD_AMOUNT')
    v = int(digits)
    if v >= 2**64:
        raise Refused('BAD_AMOUNT')
    if mult == 'p':
        if v % 10:
            raise Refused('BAD_AMOUNT')
        return v // 10
    v *= MULTIPLIER[mult] if mult else 10**11
    if v >= 2**64:
        raise Refused('BAD_AMOUNT')
    return v


def utf8_ok(b):
    try:
        b.decode('utf-8', 'strict')
        return True
    except UnicodeDecodeError:
        return False


def decode(text):
    """A dict of what the invoice says, or Refused."""
    hrp, words = bech32_decode(text)
    if not hrp.startswith('ln'):
        raise Refused('BAD_PREFIX')
    rest = hrp[2:]
    i = 0
    while i < len(rest) and not rest[i].isdigit():
        i += 1
    prefix, amount = rest[:i], rest[i:]
    if prefix not in PREFIXES:
        raise Refused('BAD_PREFIX')
    inv = {'network': PREFIXES[prefix], 'amount': amount_msat(amount), 'fields': [], 'unknown': 0,
           'fallbacks': [], 'routes': [], 'expiry': 3600, 'cltv': 18, 'features': 0,
           'metadata': None, 'n': None}
    if len(words) < 7 + 104:
        raise Refused('TOO_SHORT')
    data, sigw = words[:-104], words[-104:]
    inv['timestamp'] = word_int(data[:7])
    at = 7
    seen = {}
    while at < len(data):
        if at + 3 > len(data):
            raise Refused('TRUNCATED')
        t, n = data[at], data[at + 1] * 32 + data[at + 2]
        v = data[at + 3:at + 3 + n]
        if len(v) != n:
            raise Refused('TRUNCATED')
        at += 3 + n
        name = {b: a for a, b in TYPES.items()}.get(t)
        if name is None:
            inv['unknown'] += 1
            inv['fields'].append(('?', t, v))
            continue
        inv['fields'].append((name, t, v))
        seen[name] = seen.get(name, 0) + 1
        if name in 'pshn':
            if n != (53 if name == 'n' else 52):
                raise Refused('BAD_LENGTH')
            if seen[name] > 1:
                raise Refused('REQUIRED')
            inv[name] = to_bytes(v)
        elif name == 'd':
            inv['d'] = to_bytes(v)
            if not utf8_ok(inv['d']):
                raise Refused('BAD_UTF8')
        elif name in 'xc':
            if n == 0 or v[0] == 0:
                raise Refused('NOT_MINIMAL')
            if n > 12:
                raise Refused('BAD_LENGTH')
            inv['expiry' if name == 'x' else 'cltv'] = word_int(v)
        elif name == '9':
            if n == 0 or v[0] == 0:
                raise Refused('NOT_MINIMAL')
            inv['features'] = word_int(v)
        elif name == 'm':
            inv['metadata'] = to_bytes(v)
        elif name == 'f':
            if n == 0:
                raise Refused('BAD_FALLBACK')
            ver, prog = v[0], to_bytes(v[1:])
            if ver > 18:
                inv['unknown'] += 1
                continue
            if ver in (17, 18) and len(prog) != 20 or ver == 0 and len(prog) not in (20, 32) \
                    or 0 < ver <= 16 and not 2 <= len(prog) <= 40:
                raise Refused('BAD_FALLBACK')
            inv['fallbacks'].append(fallback_address(inv['network'], ver, prog))
        elif name == 'r':
            b = to_bytes(v)
            if not b or len(b) % 51:
                raise Refused('BAD_ROUTE')
            hops = []
            for k in range(0, len(b), 51):
                e = b[k:k + 51]
                hops.append((e[:33].hex(), int.from_bytes(e[33:41], 'big'), int.from_bytes(e[41:45], 'big'),
                             int.from_bytes(e[45:49], 'big'), int.from_bytes(e[49:51], 'big')))
            inv['routes'].append(hops)
    for one in 'ps':
        if seen.get(one, 0) != 1:
            raise Refused('REQUIRED')
    if seen.get('d', 0) + seen.get('h', 0) != 1:
        raise Refused('REQUIRED')
    f = inv['features']
    for bit in range(f.bit_length()):
        if f >> bit & 1 and bit % 2 == 0 and bit not in KNOWN_FEATURES:
            raise Refused('BAD_FEATURES')
    if (f >> 16 | f >> 17) & 1 and not (f >> 14 | f >> 15) & 1:
        raise Refused('BAD_FEATURES')
    sig = to_bytes(sigw)
    message = hrp.encode() + bytes_padded(data)
    digest = hashlib.sha256(message).digest()
    inv['message'], inv['digest'], inv['signature'], inv['recovery'] = message, digest, sig[:64], sig[64]
    if 'n' in seen:
        if int.from_bytes(sig[32:64], 'big') > C.N // 2 or not verify(digest, inv['n'], sig[:64]):
            raise Refused('BAD_SIGNATURE')
        inv['payee'] = inv['n']
    else:
        if sig[64] > 3:
            raise Refused('BAD_SIGNATURE')
        key = recover(digest, sig[:64], sig[64])
        if key is None:
            raise Refused('BAD_SIGNATURE')
        inv['payee'] = key
    return inv


def bytes_padded(words):
    """Words to bytes with zero bits appended to a byte boundary."""
    bits = 5 * len(words)
    pad = (-bits) % 8
    return (word_int(words) << pad).to_bytes((bits + pad) // 8, 'big')


# ---- the writer

def encode(hrp, timestamp, fields, key):
    """fields: (type, words) in order. Signs with key and returns the text."""
    data = [(timestamp >> 5 * (6 - i)) & 31 for i in range(7)]
    for t, v in fields:
        data += [t, len(v) >> 5, len(v) & 31] + list(v)
    digest = hashlib.sha256(hrp.encode() + bytes_padded(data)).digest()
    sig, rid = sign(key, digest)
    return bech32_encode(hrp, data + to_words(sig + bytes([rid])))
