#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: BOLT 3's transactions built and signed in plain Python, sharing no code with Coax)
"""BOLT 3's funding, commitment and HTLC transactions in plain Python, for
the cross-checks of Bitcoin chapters 108 and 109.

Written from BOLT 3's "Transactions" and "Fees" sections and BIP-143, on
ln_crypto.py's secp256k1, with its own RFC 6979 ECDSA (low-S, DER) and
transaction serialization. It is slow and not constant-time: an oracle for
tests only.
"""
import hashlib
import hmac
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import ln_crypto as C  # noqa: E402
import keys as K  # noqa: E402

ANCHOR_SAT = 330


def sha256d(b):
    return hashlib.sha256(hashlib.sha256(b).digest()).digest()


def ripemd160(b):
    return hashlib.new('ripemd160', b).digest()


def hash160(b):
    return ripemd160(hashlib.sha256(b).digest())


def le(v, n):
    return v.to_bytes(n, 'little')


def compact(n):
    if n < 0xfd:
        return bytes([n])
    if n <= 0xffff:
        return b'\xfd' + le(n, 2)
    return b'\xfe' + le(n, 4)


def push(b):
    assert len(b) < 76
    return bytes([len(b)]) + b


def number(n):
    """A script number: OP_0, OP_1 to OP_16, or the minimal push."""
    if n == 0:
        return b'\x00'
    if 1 <= n <= 16:
        return bytes([0x50 + n])
    out = bytearray()
    while n:
        out.append(n & 0xff)
        n >>= 8
    if out[-1] & 0x80:
        out.append(0)
    return push(bytes(out))


# ---- RFC 6979 ECDSA, low-S, DER

def sign(key, digest):
    d = int.from_bytes(key, 'big')
    x = key
    h1 = (int.from_bytes(digest, 'big') % C.N).to_bytes(32, 'big')
    v, k_ = b'\x01' * 32, b'\x00' * 32
    k_ = hmac.new(k_, v + b'\x00' + x + h1, 'sha256').digest()
    v = hmac.new(k_, v, 'sha256').digest()
    k_ = hmac.new(k_, v + b'\x01' + x + h1, 'sha256').digest()
    v = hmac.new(k_, v, 'sha256').digest()
    while True:
        v = hmac.new(k_, v, 'sha256').digest()
        k = int.from_bytes(v, 'big')
        if 1 <= k < C.N:
            r = C.mul(k)[0] % C.N
            s = pow(k, -1, C.N) * (int.from_bytes(digest, 'big') + r * d) % C.N
            if r and s:
                return r, min(s, C.N - s)
        k_ = hmac.new(k_, v + b'\x00', 'sha256').digest()
        v = hmac.new(k_, v, 'sha256').digest()


def der(r, s):
    def integer(v):
        b = v.to_bytes(32, 'big').lstrip(b'\x00')
        if b[0] & 0x80:
            b = b'\x00' + b
        return b'\x02' + bytes([len(b)]) + b
    body = integer(r) + integer(s)
    return b'\x30' + bytes([len(body)]) + body


def der_sign(key, digest):
    return der(*sign(key, digest))


# ---- transactions

def serialize(version, inputs, outputs, locktime, witnesses=None):
    """inputs: (outpoint 36 bytes, scriptSig, sequence); outputs: (sat, spk)."""
    out = le(version, 4)
    if witnesses:
        out += b'\x00\x01'
    out += compact(len(inputs))
    for outpoint, script, seq in inputs:
        out += outpoint + compact(len(script)) + script + le(seq, 4)
    out += compact(len(outputs))
    for sat, spk in outputs:
        out += le(sat, 8) + compact(len(spk)) + spk
    if witnesses:
        for stack in witnesses:
            out += compact(len(stack)) + b''.join(compact(len(x)) + x for x in stack)
    return out + le(locktime, 4)


def bip143(version, inputs, outputs, locktime, index, code, amount, hashtype):
    anyone, base = hashtype & 0x80, hashtype & 0x1f
    zero = b'\x00' * 32
    prevouts = zero if anyone else sha256d(b''.join(i[0] for i in inputs))
    seqs = zero if anyone or base in (2, 3) else sha256d(b''.join(le(i[2], 4) for i in inputs))
    if base not in (2, 3):
        outs = sha256d(b''.join(le(s, 8) + compact(len(p)) + p for s, p in outputs))
    elif base == 3 and index < len(outputs):
        s, p = outputs[index]
        outs = sha256d(le(s, 8) + compact(len(p)) + p)
    else:
        outs = zero
    i = inputs[index]
    msg = (le(version, 4) + prevouts + seqs + i[0] + compact(len(code)) + code + le(amount, 8) + le(i[2], 4)
           + outs + le(locktime, 4) + le(hashtype, 4))
    return sha256d(msg)


def p2wsh(script):
    return b'\x00\x20' + hashlib.sha256(script).digest()


def p2wpkh(key):
    return b'\x00\x14' + hash160(key)


# ---- BOLT 3 scripts

def funding_script(a, b):
    k1, k2 = sorted([a, b])
    return b'\x52' + push(k1) + push(k2) + b'\x52\xae'


def to_local_script(revocation, delay, delayed):
    return b'\x63' + push(revocation) + b'\x67' + number(delay) + b'\xb2\x75' + push(delayed) + b'\x68\xac'


def to_remote_anchor_script(key):
    return push(key) + b'\xad\x51\xb2'


def anchor_script(key):
    return push(key) + b'\xac\x73\x64\x60\xb2\x68'


def offered_script(revocation, remote_htlc, local_htlc, payment_hash, anchors):
    s = (b'\x76\xa9' + push(hash160(revocation)) + b'\x87\x63\xac\x67' + push(remote_htlc)
         + b'\x7c\x82\x01\x20\x87\x64\x75\x52\x7c' + push(local_htlc) + b'\x52\xae\x67\xa9'
         + push(ripemd160(payment_hash)) + b'\x88\xac\x68')
    return s + (b'\x51\xb2\x75' if anchors else b'') + b'\x68'


def received_script(revocation, remote_htlc, local_htlc, payment_hash, expiry, anchors):
    s = (b'\x76\xa9' + push(hash160(revocation)) + b'\x87\x63\xac\x67' + push(remote_htlc)
         + b'\x7c\x82\x01\x20\x87\x63\xa9' + push(ripemd160(payment_hash)) + b'\x88\x52\x7c' + push(local_htlc)
         + b'\x52\xae\x67\x75' + number(expiry) + b'\xb1\x75\xac\x68')
    return s + (b'\x51\xb2\x75' if anchors else b'') + b'\x68'


# ---- fees and trimming

def htlc_fee(feerate, offered, anchors):
    if anchors:
        return 0
    return feerate * (663 if offered else 703) // 1000


def trimmed(h, feerate, dust, anchors):
    return h['amount_msat'] // 1000 < dust + htlc_fee(feerate, h['offered'], anchors)


def commit_fee(feerate, untrimmed, anchors):
    return feerate * ((1124 if anchors else 724) + 172 * untrimmed) // 1000


def obscured(number, opener_payment_basepoint, accepter_payment_basepoint):
    mask = int.from_bytes(hashlib.sha256(opener_payment_basepoint + accepter_payment_basepoint).digest()[-6:], 'big')
    return number ^ mask


class Channel:
    """One side's view of a commitment: its keys and the shared parameters."""

    def __init__(self, funding_txid, funding_index, funding_sat, local_funding, remote_funding,
                 revocation, local_delayed, remotepubkey, local_htlc, remote_htlc, to_self_delay,
                 dust_limit, obscured_number, anchors, local_is_funder=True):
        self.__dict__.update(locals())
        del self.__dict__['self']

    def outputs(self, to_local_msat, to_remote_msat, feerate, htlcs):
        """The sorted outputs: (sat, spk, witness script, kind, htlc, expiry), and the fee."""
        a = self.anchors
        kept = [h for h in htlcs if not trimmed(h, feerate, self.dust_limit, a)]
        fee = commit_fee(feerate, len(kept), a)
        local, remote = to_local_msat // 1000, to_remote_msat // 1000
        charge = fee + (2 * ANCHOR_SAT if a else 0)
        if self.local_is_funder:
            local -= charge
        else:
            remote -= charge
        outs = []
        for h in kept:
            ph = hashlib.sha256(h['preimage']).digest() if 'preimage' in h else h['payment_hash']
            if h['offered']:
                ws = offered_script(self.revocation, self.remote_htlc, self.local_htlc, ph, a)
            else:
                ws = received_script(self.revocation, self.remote_htlc, self.local_htlc, ph, h['expiry'], a)
            outs.append((h['amount_msat'] // 1000, p2wsh(ws), ws, 'offered' if h['offered'] else 'received', h, h['expiry']))
        if local >= self.dust_limit:
            ws = to_local_script(self.revocation, self.to_self_delay, self.local_delayed)
            outs.append((local, p2wsh(ws), ws, 'to_local', None, 0))
        if remote >= self.dust_limit:
            if a:
                ws = to_remote_anchor_script(self.remotepubkey)
                outs.append((remote, p2wsh(ws), ws, 'to_remote', None, 0))
            else:
                outs.append((remote, p2wpkh(self.remotepubkey), b'', 'to_remote', None, 0))
        if a:
            if local >= self.dust_limit or kept:
                ws = anchor_script(self.local_funding)
                outs.append((ANCHOR_SAT, p2wsh(ws), ws, 'local_anchor', None, 0))
            if remote >= self.dust_limit or kept:
                ws = anchor_script(self.remote_funding)
                outs.append((ANCHOR_SAT, p2wsh(ws), ws, 'remote_anchor', None, 0))
        outs.sort(key=lambda o: (o[0], o[1], o[5]))
        return outs, fee

    def unsigned(self, outs):
        o = self.obscured_number
        inputs = [(self.funding_txid + le(self.funding_index, 4), b'', 0x80 << 24 | (o >> 24) & 0xffffff)]
        return 2, inputs, [(s, p) for s, p, *_ in outs], 0x20 << 24 | o & 0xffffff

    def funding_script(self):
        return funding_script(self.local_funding, self.remote_funding)

    def commitment_sighash(self, outs):
        v, ins, os_, lt = self.unsigned(outs)
        return bip143(v, ins, os_, lt, 0, self.funding_script(), self.funding_sat, 1)

    def signed(self, outs, local_sig, remote_sig):
        """The commitment with its witness: the signatures in the script's key order."""
        v, ins, os_, lt = self.unsigned(outs)
        sigs = [local_sig + b'\x01', remote_sig + b'\x01']
        if self.remote_funding < self.local_funding:
            sigs.reverse()
        return serialize(v, ins, os_, lt, [[b''] + sigs + [self.funding_script()]])

    def txid(self, outs):
        return sha256d(serialize(*self.unsigned(outs)))

    def htlc_tx(self, outs, index, feerate):
        """The unsigned HTLC-timeout or HTLC-success transaction for output index."""
        sat, _, ws, kind, h, expiry = outs[index]
        offered = kind == 'offered'
        amount = sat - htlc_fee(feerate, offered, self.anchors)
        out = to_local_script(self.revocation, self.to_self_delay, self.local_delayed)
        inputs = [(self.txid(outs) + le(index, 4), b'', 1 if self.anchors else 0)]
        return 2, inputs, [(amount, p2wsh(out))], expiry if offered else 0

    def htlc_sighashes(self, outs, index, feerate):
        """(remote's, local's) BIP-143 hashes for an HTLC transaction."""
        v, ins, os_, lt = self.htlc_tx(outs, index, feerate)
        sat, ws = outs[index][0], outs[index][2]
        remote_type = 0x83 if self.anchors else 1
        return (bip143(v, ins, os_, lt, 0, ws, sat, remote_type), bip143(v, ins, os_, lt, 0, ws, sat, 1)), remote_type

    def htlc_signed(self, outs, index, feerate, remote_sig, local_sig, remote_type):
        v, ins, os_, lt = self.htlc_tx(outs, index, feerate)
        h, ws = outs[index][4], outs[index][2]
        last = h['preimage'] if outs[index][3] == 'received' else b''
        stack = [b'', remote_sig + bytes([remote_type]), local_sig + b'\x01', last, ws]
        return serialize(v, ins, os_, lt, [stack])


# ---- Appendix B's funding transaction

def funding_tx(f):
    """The funding transaction of Appendix B: one P2PKH coinbase input,
    the funding output and a P2WPKH change output in BIP-69 order, with
    the legacy SIGHASH_ALL signature."""
    key = bytes.fromhex(f['coinbase_privkey'])
    pub = C.compress(C.mul(int.from_bytes(key, 'big')))
    spk = b'\x76\xa9' + push(hash160(pub)) + b'\x88\xac'
    script = funding_script(bytes.fromhex(f['local_funding_pubkey']), bytes.fromhex(f['remote_funding_pubkey']))
    fee = f['feerate'] * 928 // 1000        # BOLT 3's Appendix B: a 928-weight funding transaction
    outputs = sorted([(f['funding_sat'], p2wsh(script)), (f['input_sat'] - f['funding_sat'] - fee, p2wpkh(pub))])
    outpoint = bytes.fromhex(f['input_txid'])[::-1] + le(f['input_index'], 4)
    digest = sha256d(serialize(2, [(outpoint, spk, 0xffffffff)], outputs, 0) + le(1, 4))
    script_sig = push(der_sign(key, digest) + b'\x01') + push(pub)
    tx = serialize(2, [(outpoint, script_sig, 0xffffffff)], outputs, 0)
    return tx, sha256d(tx)[::-1].hex(), script, outputs


# ---- Appendix C's channel

def vector_channel(p, anchors):
    """The channel of BOLT 3's Appendix C, every key derived from its secrets."""
    pcp = bytes.fromhex(p['local_per_commitment_point'])
    lsec, rsec = (int(p[k], 16) for k in ('local_payment_basepoint_secret', 'remote_payment_basepoint_secret'))
    local_pay, remote_pay = K.point(lsec), K.point(rsec)
    revocation = K.revocation_pubkey(K.point(int(p['remote_revocation_basepoint_secret'], 16)), pcp)
    delayed = K.pubkey(K.point(int(p['local_delayed_payment_basepoint_secret'], 16)), pcp)
    ch = Channel(bytes.fromhex(p['funding_txid'])[::-1], p['funding_index'], p['funding_sat'],
                 K.point(int(p['local_funding_privkey'], 16)), K.point(int(p['remote_funding_privkey'], 16)),
                 revocation, delayed, remote_pay, K.pubkey(local_pay, pcp), K.pubkey(remote_pay, pcp),
                 p['to_self_delay'], p['dust_limit'], obscured(p['commitment_number'], local_pay, remote_pay), anchors)
    secrets = {
        'local_funding': bytes.fromhex(p['local_funding_privkey']),
        'remote_funding': bytes.fromhex(p['remote_funding_privkey']),
        'local_htlc': K.privkey(lsec, pcp).to_bytes(32, 'big'),
        'remote_htlc': K.privkey(rsec, pcp).to_bytes(32, 'big'),
    }
    return ch, secrets


def vector_htlcs(p, which):
    return [dict(p['htlcs'][i], preimage=bytes.fromhex(p['htlcs'][i]['preimage']), number=i) for i in which]


def build_vector(ch, secrets, to_local_msat, to_remote_msat, feerate, htlcs):
    """The signed commitment and its signed HTLC transactions in output order."""
    outs, fee = ch.outputs(to_local_msat, to_remote_msat, feerate, htlcs)
    digest = ch.commitment_sighash(outs)
    tx = ch.signed(outs, der_sign(secrets['local_funding'], digest), der_sign(secrets['remote_funding'], digest))
    htlc_txs = []
    for i, o in enumerate(outs):
        if o[3] in ('offered', 'received'):
            (rh, lh), rt = ch.htlc_sighashes(outs, i, feerate)
            htlc_txs.append((o[3], o[4]['number'], ch.htlc_signed(outs, i, feerate, der_sign(secrets['remote_htlc'], rh),
                                                                  der_sign(secrets['local_htlc'], lh), rt)))
    return outs, fee, tx, htlc_txs
