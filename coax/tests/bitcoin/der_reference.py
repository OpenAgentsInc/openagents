# Firm reason: 2 (independent oracle: Python decides each case outside Coax)
"""A Python reference for Bitcoin chapter 21, written from Bitcoin Core's
interpreter.cpp and pubkey.cpp (pinned in coax/tests/vectors/upstream/bitcoin-core/):
IsValidSignatureEncoding, IsLowDERSignature, IsDefinedHashtypeSignature,
CheckSignatureEncoding, ecdsa_signature_parse_der_lax, and CHECKSIG and
CHECKMULTISIG as they use them. It shares no code with the Coax library.
Imported by der.py."""

P = 2**256 - 2**32 - 977
N = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141
G = (0x79BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798,
     0x483ADA7726A3C4655DA4FBFC0E1108A8FD17B448A68554199C47D08FFB10D4B8)
DERSIG, LOW_S, STRICTENC, NULLFAIL, NULLDUMMY = 1, 2, 4, 8, 16


def valid_encoding(sig):
    """IsValidSignatureEncoding, rule by rule, on a signature with its sighash byte."""
    n = len(sig)
    if n < 9 or n > 73 or sig[0] != 0x30 or sig[1] != n - 3: return False
    len_r = sig[3]
    if 5 + len_r >= n: return False
    len_s = sig[5 + len_r]
    if len_r + len_s + 7 != n or sig[2] != 2 or len_r == 0 or sig[4] & 0x80: return False
    if len_r > 1 and sig[4] == 0 and not sig[5] & 0x80: return False
    if sig[len_r + 4] != 2 or len_s == 0 or sig[len_r + 6] & 0x80: return False
    return not (len_s > 1 and sig[len_r + 6] == 0 and not sig[len_r + 7] & 0x80)


def lax(der):
    """ecdsa_signature_parse_der_lax: None when it returns 0, else (r, s),
    which is (0, 0) when either value overflows."""
    n, pos = len(der), 0
    if pos == n or der[pos] != 0x30: return None
    pos += 1
    if pos == n: return None
    length = der[pos]; pos += 1
    if length & 0x80:
        length -= 0x80
        if length > n - pos: return None
        pos += length
    spans = []
    for _ in range(2):
        if pos == n or der[pos] != 2: return None
        pos += 1
        if pos == n: return None
        length = der[pos]; pos += 1
        if length & 0x80:
            length -= 0x80
            if length > n - pos: return None
            while length > 0 and der[pos] == 0:
                pos += 1; length -= 1
            if length >= 4: return None
            value = 0
            while length > 0:
                value = (value << 8) + der[pos]; pos += 1; length -= 1
            length = value
        if length > n - pos: return None
        spans.append((pos, length))
        pos += length
    values = []
    for start, length in spans:
        body = der[start:start + length].lstrip(b'\0')
        values.append(None if len(body) > 32 else int.from_bytes(body, 'big'))
    if None in values or values[0] >= N or values[1] >= N: return (0, 0)
    return tuple(values)


def low_s(sig):
    """CheckLowS on the signature without its sighash byte (after the encoding check)."""
    rs = lax(sig[:-1])
    return rs is not None and rs[1] <= N // 2


def defined_hashtype(sig):
    return len(sig) > 0 and 1 <= (sig[-1] & 0x7f) <= 3


def signature_error(sig, flags):
    """CheckSignatureEncoding: None, or the script error."""
    if not sig: return None
    if flags & (DERSIG | LOW_S | STRICTENC) and not valid_encoding(sig): return 'SIG_DER'
    if flags & LOW_S and not low_s(sig): return 'SIG_HIGH_S'
    if flags & STRICTENC and not defined_hashtype(sig): return 'SIG_HASHTYPE'
    return None


# ---- verification
def add(a, b):
    if a is None: return b
    if b is None: return a
    if a[0] == b[0] and (a[1] + b[1]) % P == 0: return None
    if a == b: slope = 3 * a[0] * a[0] * pow(2 * a[1], -1, P) % P
    else: slope = (b[1] - a[1]) * pow(b[0] - a[0], -1, P) % P
    x = (slope * slope - a[0] - b[0]) % P
    return x, (slope * (a[0] - x) - a[1]) % P


def mul(k, point):
    result = None
    while k:
        if k & 1: result = add(result, point)
        point, k = add(point, point), k >> 1
    return result


def point(key):
    """A SEC 1 key (02, 03 or 04 forms), or None."""
    if len(key) == 33 and key[0] in (2, 3):
        x = int.from_bytes(key[1:], 'big')
        if x >= P: return None
        y = pow((x**3 + 7) % P, (P + 1) // 4, P)
        if y * y % P != (x**3 + 7) % P: return None
        return (x, y if y % 2 == key[0] % 2 else P - y)
    if len(key) == 65 and key[0] == 4:
        x, y = int.from_bytes(key[1:33], 'big'), int.from_bytes(key[33:], 'big')
        return (x, y) if x < P and y < P and (y * y - x**3 - 7) % P == 0 else None
    return None


def checksig(sig, key, digest):
    """CheckECDSASignature: lax parse, normalize to low S, verify."""
    q = point(key)
    if q is None or not sig: return False
    rs = lax(sig[:-1])
    if rs is None: return False
    r, s = rs
    if not (0 < r < N and 0 < s < N): return False
    w = pow(s, -1, N)   # s and n - s verify alike
    z = int.from_bytes(digest, 'big') % N
    R = add(mul(z * w % N, G), mul(r * w % N, q))
    return R is not None and R[0] % N == r


def key_shape(key):
    return (len(key) == 33 and key[0] in (2, 3)) or (len(key) == 65 and key[0] == 4)


def scriptnum(b):
    if not b: return 0
    v = int.from_bytes(b, 'little')
    if b[-1] & 0x80: return -(v & ~(0x80 << (8 * (len(b) - 1))))
    return v


def run_row(stack, hashes, flags, multisig, negate):
    """The script result: 'OK', 'EVAL_FALSE' or an error name."""
    stack = list(stack)
    if not multisig:
        sig, key = stack[-2], stack[-1]
        error = signature_error(sig, flags)
        if error: return error
        if flags & STRICTENC and not key_shape(key): return 'PUBKEYTYPE'
        ok = checksig(sig, key, hashes[len(stack) - 2])
        if not ok and flags & NULLFAIL and sig: return 'NULLFAIL'
        del stack[-2:]
    else:
        i = 1
        keys = scriptnum(stack[-i]); ikey = i + 1; ikey2 = keys + 2; i += keys + 1
        sigs = scriptnum(stack[-i]); isig = i + 1; i += sigs + 1
        assert 0 <= sigs <= keys and len(stack) >= i
        ok = True
        while ok and sigs > 0:
            sig, key = stack[-isig], stack[-ikey]
            error = signature_error(sig, flags)
            if error: return error
            if flags & STRICTENC and not key_shape(key): return 'PUBKEYTYPE'
            if checksig(sig, key, hashes[len(stack) - isig]):
                isig += 1; sigs -= 1
            ikey += 1; keys -= 1
            if sigs > keys: ok = False
        while i > 1:
            i -= 1
            if not ok and flags & NULLFAIL and not ikey2 and stack[-1]: return 'NULLFAIL'
            if ikey2 > 0: ikey2 -= 1
            stack.pop()
        if flags & NULLDUMMY and stack[-1]: return 'SIG_NULLDUMMY'
        stack.pop()
    if negate: ok = not ok
    return 'OK' if ok else 'EVAL_FALSE'
