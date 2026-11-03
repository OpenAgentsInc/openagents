#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: BOLT 4's Sphinx, error onions and route blinding in Python, sharing no code with Coax)
"""BOLT 4 written from its text in plain Python, for the cross-check of
Bitcoin chapter 111: the Sphinx packet, its filler, peeling one layer,
error onions and their decryption by the origin, and blinded paths.

It runs on ln_crypto.py's secp256k1 and ChaCha20 and Python's hmac. It is
slow and not constant-time: an oracle for tests only.
"""
import hashlib
import hmac

import ln_crypto as C

PAYLOADS = 1300          # hop_payloads in a payment onion and a small onion message
PACKET = 1 + 33 + PAYLOADS + 32


class Refused(Exception):
    pass


def xor(a, b):
    return bytes(x ^ y for x, y in zip(a, b))


def mac(key, data):
    return hmac.new(key, data, hashlib.sha256).digest()


def key(kind, secret):
    """rho, mu, um, pad, ammag and blinded_node_id: HMAC with the name as key."""
    return mac(kind, secret)


def stream(k, n):
    return C.chacha20(k, 0, bytes(12), bytes(n))


def scalar(b):
    return int.from_bytes(b, 'big')


def ecdh(k, point_bytes):
    p = C.decompress(point_bytes)
    if p is None:
        raise Refused('BAD_POINT')
    return C.sha256(C.compress(C.mul(k, p)))


def pub(k):
    return C.compress(C.mul(k))


def bigsize(b, at):
    first = b[at]
    if first < 0xfd:
        return first, at + 1
    width = {0xfd: 2, 0xfe: 4, 0xff: 8}[first]
    return int.from_bytes(b[at + 1:at + 1 + width], 'big'), at + 1 + width


def bigsize_bytes(v):
    if v < 0xfd:
        return bytes([v])
    if v < 0x10000:
        return b'\xfd' + v.to_bytes(2, 'big')
    if v < 0x100000000:
        return b'\xfe' + v.to_bytes(4, 'big')
    return b'\xff' + v.to_bytes(8, 'big')


def tlvs(b):
    """A TLV stream as a dict of type -> value."""
    out, at = {}, 0
    while at < len(b):
        t, at = bigsize(b, at)
        n, at = bigsize(b, at)
        out[t] = b[at:at + n]
        at += n
    return out


def tlv(records):
    return b''.join(bigsize_bytes(t) + bigsize_bytes(len(v)) + v for t, v in sorted(records.items()))


# ---- the forward packet

def route_secrets(session, pubkeys):
    """For each hop: the ephemeral public key it sees and its shared secret."""
    e, out = session, []
    for p in pubkeys:
        epk = pub(e)
        ss = ecdh(e, p)
        out.append((epk, ss))
        e = e * scalar(C.sha256(epk, ss)) % C.N
    return out


def filler(secrets, sizes):
    f = b''
    for (_, ss), size in zip(secrets[:-1], sizes[:-1]):
        f += bytes(size)
        s = stream(key(b'rho', ss), PAYLOADS + size)
        f = xor(f, s[PAYLOADS + size - len(f):])
    return f


def wrap(session, pubkeys, payloads, ad):
    """The onion for hops with these keys and length-prefixed payloads."""
    secrets = route_secrets(session, pubkeys)
    sizes = [len(p) + 32 for p in payloads]
    if sum(sizes) > PAYLOADS:
        raise Refused('TOO_LONG')
    fill = filler(secrets, sizes)
    hp = stream(key(b'pad', session.to_bytes(32, 'big')), PAYLOADS)
    next_mac = bytes(32)
    for i in reversed(range(len(pubkeys))):
        ss = secrets[i][1]
        hp = payloads[i] + next_mac + hp[:PAYLOADS - sizes[i]]
        hp = xor(hp, stream(key(b'rho', ss), PAYLOADS))
        if i == len(pubkeys) - 1 and fill:
            hp = hp[:PAYLOADS - len(fill)] + fill
        next_mac = mac(key(b'mu', ss), hp + ad)
    return b'\0' + secrets[0][0] + hp + next_mac


def peel(k, onion, ad):
    """One hop's layer: (payload, next onion or None, shared secret)."""
    if len(onion) != PACKET:
        raise Refused('BAD_LENGTH')
    if onion[0] != 0:
        raise Refused('BAD_VERSION')
    epk, hp, got = onion[1:34], onion[34:34 + PAYLOADS], onion[34 + PAYLOADS:]
    ss = ecdh(k, epk)
    if not hmac.compare_digest(mac(key(b'mu', ss), hp + ad), got):
        raise Refused('BAD_HMAC')
    d = xor(hp + bytes(PAYLOADS), stream(key(b'rho', ss), 2 * PAYLOADS))
    n, at = bigsize(d, 0)
    if n == 0 or at + n + 32 > PAYLOADS:
        raise Refused('BAD_PAYLOAD')
    payload, next_mac = d[at:at + n], d[at + n:at + n + 32]
    if next_mac == bytes(32):
        return payload, None, ss
    nexte = C.compress(C.mul(scalar(C.sha256(epk, ss)), C.decompress(epk)))
    return payload, b'\0' + nexte + d[at + n + 32:at + n + 32 + PAYLOADS] + next_mac, ss


# ---- errors

def error_packet(ss, failure, pad_to=256):
    pad = max(pad_to - len(failure), 0)
    body = len(failure).to_bytes(2, 'big') + failure + pad.to_bytes(2, 'big') + bytes(pad)
    return mac(key(b'um', ss), body) + body


def error_wrap(ss, packet):
    return xor(packet, stream(key(b'ammag', ss), len(packet)))


def error_open(secrets, packet):
    """The origin: (hop index, failure message), or (None, None)."""
    for i, ss in enumerate(secrets):
        packet = error_wrap(ss, packet)
        body = packet[32:]
        if hmac.compare_digest(mac(key(b'um', ss), body), packet[:32]):
            n = int.from_bytes(body[:2], 'big')
            return i, body[2:2 + n]
    return None, None


# ---- route blinding

def blind(session_keys, node_ids, data):
    """A blinded path. session_keys[i] is None to continue the path, or a
    new secret where another path is joined. Returns per hop
    (path_key, blinded_node_id, encrypted_data, shared secret)."""
    out, e = [], None
    for start, node, plain in zip(session_keys, node_ids, data):
        if start is not None:
            e = start
        path_key = pub(e)
        ss = ecdh(e, node)
        blinded = C.compress(C.mul(scalar(key(b'blinded_node_id', ss)), C.decompress(node)))
        box = C.seal(key(b'rho', ss), bytes(12), b'', plain)
        out.append((path_key, blinded, box, ss))
        e = e * scalar(C.sha256(path_key, ss)) % C.N
    return out


def unblind(k, path_key, box):
    """A node on the path: (blinded private key, plaintext, next path key)."""
    ss = ecdh(k, path_key)
    priv = k * scalar(key(b'blinded_node_id', ss)) % C.N
    plain = C.open_(key(b'rho', ss), bytes(12), b'', box)
    if plain is None:
        raise Refused('BAD_TAG')
    nxt = C.compress(C.mul(scalar(C.sha256(path_key, ss)), C.decompress(path_key)))
    override = tlvs(plain).get(8)
    return priv, plain, override if override else nxt
