#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: a Python transliteration of Bitcoin Core's proof-of-work code decides each case outside Coax)
"""Bitcoin Core's header and proof-of-work code, transliterated to Python.

Bitcoin From Scratch, chapter 47. From the pinned sources in
coax/tests/vectors/upstream/bitcoin-core/ at revision ddea9196:

- arith_uint256.cpp: base_uint<256> as eight 32-bit limbs, lowest first,
  with its shifts, bits(), operator/= (shift and subtract), SetCompact
  with its negative and overflow flags, and GetCompact;
- pow.cpp: DeriveTarget and CheckProofOfWorkImpl;
- chain.cpp: GetBitsProof, the work of one header, 2^256 / (target + 1);
- primitives/block.h: the 80-byte header and its hash.

The limbs are Python lists, so every shift and carry happens the way Core's
does; nothing here shares code with Coax. `proof_direct` computes the same
work with Python's own big integers, as a second opinion.
"""
import hashlib
import struct

WIDTH = 8
MASK32 = 0xFFFFFFFF
# chainparams.cpp, CMainParams: consensus.powLimit.
POW_LIMIT_MAIN = int("00000000ffffffffffffffffffffffffffffffffffffffffffffffffffffffff", 16)
# chainparams.cpp, CRegTestParams: consensus.powLimit.
POW_LIMIT_REGTEST = int("7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff", 16)

# The chapter's result codes (pow.coax).
OK, NEGATIVE, ZERO, OVERFLOW, ABOVE_LIMIT, HIGH_HASH, CAPACITY = range(7)


def sha256d(b):
    return hashlib.sha256(hashlib.sha256(b).digest()).digest()


# ---- base_uint<256>
def limbs(v):
    return [(v >> (32 * i)) & MASK32 for i in range(WIDTH)]


def value(pn):
    return sum(x << (32 * i) for i, x in enumerate(pn))


def shl(pn, shift):
    """base_uint::operator<<=: bits past the top are lost."""
    a = list(pn)
    out = [0] * WIDTH
    k = shift // 32
    shift = shift % 32
    for i in range(WIDTH):
        if i + k + 1 < WIDTH and shift != 0:
            out[i + k + 1] |= a[i] >> (32 - shift)
        if i + k < WIDTH:
            out[i + k] |= (a[i] << shift) & MASK32
    return out


def shr(pn, shift):
    """base_uint::operator>>=."""
    a = list(pn)
    out = [0] * WIDTH
    k = shift // 32
    shift = shift % 32
    for i in range(WIDTH):
        if i - k - 1 >= 0 and shift != 0:
            out[i - k - 1] |= (a[i] << (32 - shift)) & MASK32
        if i - k >= 0:
            out[i - k] |= a[i] >> shift
    return out


def bits(pn):
    """base_uint::bits: the position of the highest set bit, plus one."""
    for pos in range(WIDTH - 1, -1, -1):
        if pn[pos]:
            for nbits in range(31, 0, -1):
                if pn[pos] & (1 << nbits):
                    return 32 * pos + nbits + 1
            return 32 * pos + 1
    return 0


def compare(a, b):
    for i in range(WIDTH - 1, -1, -1):
        if a[i] < b[i]:
            return -1
        if a[i] > b[i]:
            return 1
    return 0


def add(a, b):
    out, carry = [0] * WIDTH, 0
    for i in range(WIDTH):
        n = carry + a[i] + b[i]
        out[i] = n & MASK32
        carry = n >> 32
    return out


def neg(a):
    """base_uint::operator-: two's complement, ~a + 1."""
    return add([x ^ MASK32 for x in a], limbs(1))


def sub(a, b):
    return add(a, neg(b))


def div(a, b):
    """base_uint::operator/=: None for a zero divisor (Core throws)."""
    num, d = list(a), list(b)
    q = [0] * WIDTH
    num_bits, div_bits = bits(num), bits(d)
    if div_bits == 0:
        return None
    if div_bits > num_bits:
        return q
    shift = num_bits - div_bits
    d = shl(d, shift)
    while shift >= 0:
        if compare(num, d) >= 0:
            num = sub(num, d)
            q[shift // 32] |= 1 << (shift & 31)
        d = shr(d, 1)
        shift -= 1
    return q


def low64(pn):
    return pn[0] | (pn[1] << 32)


# ---- compact targets
def set_compact(n_compact):
    """arith_uint256::SetCompact: (target, negative, overflow)."""
    n_size = n_compact >> 24
    n_word = n_compact & 0x007FFFFF
    if n_size <= 3:
        n_word >>= 8 * (3 - n_size)
        t = limbs(n_word)
    else:
        t = shl(limbs(n_word), 8 * (n_size - 3))
    negative = n_word != 0 and (n_compact & 0x00800000) != 0
    overflow = n_word != 0 and (n_size > 34 or (n_word > 0xFF and n_size > 33) or (n_word > 0xFFFF and n_size > 32))
    return value(t), negative, overflow


def get_compact(v, negative=False):
    """arith_uint256::GetCompact."""
    pn = limbs(v)
    n_size = (bits(pn) + 7) // 8
    if n_size <= 3:
        n_compact = (low64(pn) << 8 * (3 - n_size)) & MASK32
    else:
        n_compact = low64(shr(pn, 8 * (n_size - 3))) & MASK32
    if n_compact & 0x00800000:
        n_compact >>= 8
        n_size += 1
    assert n_compact & ~0x007FFFFF & MASK32 == 0 and n_size < 256
    n_compact |= n_size << 24
    if negative and n_compact & 0x007FFFFF:
        n_compact |= 0x00800000
    return n_compact


def derive_target(n_bits, pow_limit):
    """pow.cpp DeriveTarget, with the reason it refuses in its order."""
    t, negative, overflow = set_compact(n_bits)
    if negative:
        return NEGATIVE, t
    if t == 0:
        return ZERO, t
    if overflow:
        return OVERFLOW, t
    if t > pow_limit:
        return ABOVE_LIMIT, t
    return OK, t


def check_pow(hash_bytes, n_bits, pow_limit):
    """pow.cpp CheckProofOfWorkImpl; the hash in the order SHA-256d gives it."""
    code, t = derive_target(n_bits, pow_limit)
    if code != OK:
        return code
    return HIGH_HASH if int.from_bytes(hash_bytes, "little") > t else OK


def proof(n_bits):
    """chain.cpp GetBitsProof: ~target / (target + 1) + 1, or 0."""
    t, negative, overflow = set_compact(n_bits)
    if negative or overflow or t == 0:
        return 0
    pn = limbs(t)
    q = div([x ^ MASK32 for x in pn], add(pn, limbs(1)))
    return value(add(q, limbs(1)))


def proof_direct(n_bits):
    t, negative, overflow = set_compact(n_bits)
    return 0 if negative or overflow or t == 0 else (1 << 256) // (t + 1)


# ---- headers
def read_header(b):
    """CBlockHeader's serialization: version (int32), prev, merkle root,
    time, bits, nonce (uint32), little-endian."""
    if len(b) != 80:
        return None
    version, = struct.unpack_from("<i", b, 0)
    time, n_bits, nonce = struct.unpack_from("<III", b, 68)
    return dict(version=version, prev=b[4:36], merkle_root=b[36:68], time=time, bits=n_bits, nonce=nonce)


def write_header(h):
    return struct.pack("<i", h["version"]) + h["prev"] + h["merkle_root"] + struct.pack("<III", h["time"], h["bits"], h["nonce"])


def check_header(b, pow_limit):
    """validation.cpp CheckBlockHeader: "high-hash" unless the hash meets nBits."""
    h = read_header(b)
    if h is None:
        return CAPACITY
    return check_pow(sha256d(b), h["bits"], pow_limit)
