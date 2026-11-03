#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: textbook secp256k1, ChaCha20-Poly1305 and HKDF in Python, sharing no code with Coax)
"""The cryptography the Lightning cross-checks need, written from the
standards in plain Python: secp256k1 in affine coordinates (SEC 2), the
ChaCha20-Poly1305 AEAD (RFC 8439) and HKDF-SHA256 (RFC 5869, on Python's
hmac). It is slow and not constant-time: an oracle for tests only.
"""
import hashlib
import hmac

P = 2**256 - 2**32 - 977
N = 0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141
G = (0x79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798,
     0x483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8)


def add(a, b):
    if a is None:
        return b
    if b is None:
        return a
    if a[0] == b[0] and (a[1] + b[1]) % P == 0:
        return None
    if a == b:
        m = 3 * a[0] * a[0] * pow(2 * a[1], -1, P)
    else:
        m = (b[1] - a[1]) * pow(b[0] - a[0], -1, P)
    x = (m * m - a[0] - b[0]) % P
    return x, (m * (a[0] - x) - a[1]) % P


def mul(k, point=G):
    result = None
    while k:
        if k & 1:
            result = add(result, point)
        point = add(point, point)
        k >>= 1
    return result


def compress(point):
    return bytes([2 + (point[1] & 1)]) + point[0].to_bytes(32, 'big')


def decompress(b):
    """A 33-byte compressed point, or None."""
    if len(b) != 33 or b[0] not in (2, 3):
        return None
    x = int.from_bytes(b[1:], 'big')
    if x >= P:
        return None
    y = pow((pow(x, 3, P) + 7) % P, (P + 1) // 4, P)
    if (y * y - x ** 3 - 7) % P:
        return None
    return (x, y if y & 1 == b[0] & 1 else P - y)


def sha256(*parts):
    return hashlib.sha256(b''.join(parts)).digest()


def hkdf(salt, ikm, length=64):
    prk = hmac.new(salt, ikm, hashlib.sha256).digest()
    out, block = b'', b''
    for i in range((length + 31) // 32):
        block = hmac.new(prk, block + bytes([i + 1]), hashlib.sha256).digest()
        out += block
    return out[:length]


def _rotl(v, c):
    return ((v << c) & 0xffffffff) | (v >> (32 - c))


def _quarter(s, a, b, c, d):
    s[a] = (s[a] + s[b]) & 0xffffffff; s[d] = _rotl(s[d] ^ s[a], 16)
    s[c] = (s[c] + s[d]) & 0xffffffff; s[b] = _rotl(s[b] ^ s[c], 12)
    s[a] = (s[a] + s[b]) & 0xffffffff; s[d] = _rotl(s[d] ^ s[a], 8)
    s[c] = (s[c] + s[d]) & 0xffffffff; s[b] = _rotl(s[b] ^ s[c], 7)


def chacha20_block(key, counter, nonce):
    state = [0x61707865, 0x3320646e, 0x79622d32, 0x6b206574]
    state += [int.from_bytes(key[i:i + 4], 'little') for i in range(0, 32, 4)]
    state += [counter] + [int.from_bytes(nonce[i:i + 4], 'little') for i in range(0, 12, 4)]
    s = list(state)
    for _ in range(10):
        _quarter(s, 0, 4, 8, 12); _quarter(s, 1, 5, 9, 13); _quarter(s, 2, 6, 10, 14); _quarter(s, 3, 7, 11, 15)
        _quarter(s, 0, 5, 10, 15); _quarter(s, 1, 6, 11, 12); _quarter(s, 2, 7, 8, 13); _quarter(s, 3, 4, 9, 14)
    return b''.join(((s[i] + state[i]) & 0xffffffff).to_bytes(4, 'little') for i in range(16))


def chacha20(key, counter, nonce, data):
    out = bytearray()
    for i in range(0, len(data), 64):
        block = chacha20_block(key, counter + i // 64, nonce)
        out += bytes(x ^ y for x, y in zip(data[i:i + 64], block))
    return bytes(out)


def poly1305(key, msg):
    r = int.from_bytes(key[:16], 'little') & 0x0ffffffc0ffffffc0ffffffc0fffffff
    s = int.from_bytes(key[16:], 'little')
    acc, p = 0, (1 << 130) - 5
    for i in range(0, len(msg), 16):
        acc = (acc + int.from_bytes(msg[i:i + 16] + b'\x01', 'little')) * r % p
    return ((acc + s) & ((1 << 128) - 1)).to_bytes(16, 'little')


def _pad(b):
    return bytes((16 - len(b) % 16) % 16)


def seal(key, nonce, aad, plain):
    otk = chacha20_block(key, 0, nonce)[:32]
    c = chacha20(key, 1, nonce, plain)
    mac = aad + _pad(aad) + c + _pad(c) + len(aad).to_bytes(8, 'little') + len(c).to_bytes(8, 'little')
    return c + poly1305(otk, mac)


def open_(key, nonce, aad, box):
    """The plaintext, or None when the tag does not check."""
    c, tag = box[:-16], box[-16:]
    otk = chacha20_block(key, 0, nonce)[:32]
    mac = aad + _pad(aad) + c + _pad(c) + len(aad).to_bytes(8, 'little') + len(c).to_bytes(8, 'little')
    if not hmac.compare_digest(poly1305(otk, mac), tag):
        return None
    return chacha20(key, 1, nonce, c)
