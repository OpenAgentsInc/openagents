#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: decides every BIP-322 vector with bip322_reference.py, outside Coax)
"""Writes coax/tests/bitcoin/bip322_vectors.coax from the pinned BIP-322
test vectors (coax/tests/vectors/upstream/bip322/, bips revision
5aadddb5), for Bitcoin chapter 96:

  python3 -I coax/tests/bitcoin/make_bip322_vectors.py           # check the file is current
  python3 -I coax/tests/bitcoin/make_bip322_vectors.py --write   # rewrite it

Every published hash, signature and error is decided by
bip322_reference.py and checked against the published result: each
signature verifies as valid at its time and age, each error is invalid
for the reason its text names. For each signature the reference signer
makes its own from the published keys; for ECDSA it is the published
one byte for byte (RFC 6979 without low-R grinding), for BIP-340 it is
another valid signature (the published ones use btcd's nonces). Then
constructed cases reach the paths the published vectors don't: the
upgradeable rules (inconclusive), Taproot's annex, leaf versions,
OP_SUCCESSx, CHECKSIGADD and unknown key types, and the legacy format.
"""
import hashlib, json, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import bip322_reference as R  # noqa: E402
import der_reference as D  # noqa: E402
import psbt_reference as P  # noqa: E402
import tx_reference as T  # noqa: E402

UP = HERE.parent / 'vectors/upstream/bip322'
OUT = HERE / 'bip322_vectors.coax'
FORMAT = {'legacy': 0, 'smp': 1, 'ful': 2, 'pof': 3, None: 4}
STATE = {R.VALID: 0, R.INVALID: 1, R.INCONCLUSIVE: 2}
AUX = bytes(32)


def bech32_encode(version, prog):
    charset = 'qpzry9x8gf2tvdw0s3jn54khce6mua7l'
    acc, bits, data = 0, 0, [version]
    for b in prog:
        acc, bits = (acc << 8) | b, bits + 8
        while bits >= 5: bits -= 5; data.append((acc >> bits) & 31)
    if bits: data.append((acc << (5 - bits)) & 31)
    gen = [0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3]
    def polymod(v):
        chk = 1
        for x in v:
            top = chk >> 25
            chk = (chk & 0x1ffffff) << 5 ^ x
            for i in range(5): chk ^= gen[i] if (top >> i) & 1 else 0
        return chk
    hrp = [3, 3, 0, 2, 3]   # 'bc' expanded
    pm = polymod(hrp + data + [0] * 6) ^ (1 if version == 0 else 0x2bc830a3)
    return 'bc1' + ''.join(charset[d] for d in data + [(pm >> 5 * (5 - i)) & 31 for i in range(6)])


def b64(b): return R.B64.b64encode(b).decode()
def witness_bytes(items): return T.compact(len(items)) + b''.join(T.compact(len(x)) + x for x in items)


def tap_tree(internal_secret, script, version=0xc0):
    """The address and control block of a tree with one leaf."""
    pt = D.mul(internal_secret, D.G)
    x = pt[0].to_bytes(32, 'big')
    h = R.leaf_hash(script, version)
    even = pt if pt[1] % 2 == 0 else (pt[0], D.P - pt[1])
    t = int.from_bytes(P.tagged('TapTweak', x + h), 'big')
    q = D.add(even, D.mul(t, D.G))
    return bech32_encode(1, q[0].to_bytes(32, 'big')), bytes([version | (q[1] % 2)]) + x


def full_tx(address, message, witness, version=0, locktime=0, sequence=0):
    tx = R.to_sign(R.to_spend(R.address_script(address), message), version, locktime, sequence)
    tx['ins'][0]['witness'] = witness
    return tx


def constructed():
    """(description, address, message, signature) for paths the published vectors miss."""
    out = []
    msg = 'Chapter 96'
    def wsh(script): return bech32_encode(0, hashlib.sha256(script).digest())
    # The upgradeable rules: a NOP reserved for soft forks; a version other than 0 and 2; a witness version above 1.
    nop4 = bytes([0xb3, 0x51])
    out.append(('P2WSH with OP_NOP4: upgradeable', wsh(nop4), msg, 'smp' + b64(witness_bytes([nop4]))))
    one = b'\x51'
    out.append(('P2WSH OP_1 in a version 1 to_sign', wsh(one), msg, 'ful' + b64(T.serialize(full_tx(wsh(one), msg, [one], 1), True))))
    out.append(('P2WSH OP_1, version 2, valid at 500 and age 7', wsh(one), msg,
                'ful' + b64(T.serialize(full_tx(wsh(one), msg, [one], 2, 500, 7), True))))
    out.append(('witness version 2', bech32_encode(2, bytes(range(32))), msg, 'smp' + b64(witness_bytes([b'\x01']))))
    out.append(('P2WSH with OP_CODESEPARATOR', wsh(b'\xab\x51'), msg, 'smp' + b64(witness_bytes([b'\xab\x51']))))
    out.append(('P2WSH whose script leaves two elements', wsh(b'\x51\x51'), msg, 'smp' + b64(witness_bytes([b'\x51\x51']))))
    # Taproot script paths.
    k = 0x96 * 0x0101010101010101010101010101010101010101010101010101010101010101 % D.N
    for desc, script, version in (('P2TR leaf version 0xc2', b'\x51', 0xc2),
                                  ('P2TR leaf with OP_SUCCESS80', b'\x50\x51', 0xc0),
                                  ('P2TR leaf with OP_ROT, an opcode the evaluator leaves out', b'\x51\x51\x51\x7b\x75\x75', 0xc0),
                                  ('P2TR leaf with OP_CODESEPARATOR', b'\xab\x51', 0xc0),
                                  ('P2TR leaf with OP_CHECKMULTISIG', b'\x00\x00\xae', 0xc0),
                                  ('P2TR leaf OP_1', b'\x51', 0xc0),
                                  ('P2TR leaf with a 33-byte key: unknown key type', b'\x00\x21' + bytes([2]) + bytes(32) + b'\xac\x91', 0xc0),
                                  ('P2TR leaf with an empty key', b'\x00\x00\xac\x91', 0xc0),
                                  ('P2TR leaf with a non-minimal IF argument', b'\x63\x51\x68', 0xc0)):
        addr, control = tap_tree(k, script, version)
        stack = [b'\x02'] if script == b'\x63\x51\x68' else []
        out.append((desc, addr, msg, 'smp' + b64(witness_bytes(stack + [script, control]))))
    addr, control = tap_tree(k, b'\x51')
    out.append(('P2TR with an annex', addr, msg, 'smp' + b64(witness_bytes([b'\x51', control, b'\x50\x01']))))
    out.append(('P2TR leaf OP_1 under the wrong control block', addr, msg,
                'smp' + b64(witness_bytes([b'\x51', bytes([control[0] ^ 1]) + control[1:]]))))
    # A 2-of-2 CHECKSIGADD leaf, signed by both keys.
    k1, k2 = 0x9601 * 0x10001 % D.N, 0x9602 * 0x10001 % D.N
    x1, x2 = (D.mul(s, D.G)[0].to_bytes(32, 'big') for s in (k1, k2))
    script = b'\x20' + x1 + b'\xac\x20' + x2 + b'\xba\x52\x9c'
    addr, control = tap_tree(k, script)
    tx = full_tx(addr, msg, [])
    h = R.tap_sighash(tx, 0, [(0, R.address_script(addr))], 0, R.leaf_hash(script))
    sig = lambda d: P.schnorr(d, h, AUX)
    out.append(('P2TR 2-of-2 CHECKSIGADD', addr, msg, 'smp' + b64(witness_bytes([sig(k2), sig(k1), script, control]))))
    out.append(('P2TR 2-of-2 CHECKSIGADD with one signature', addr, msg, 'smp' + b64(witness_bytes([b'', sig(k1), script, control]))))
    out.append(('P2TR 2-of-2 CHECKSIGADD with a 65-byte SIGHASH_ALL signature', addr, msg,
                'smp' + b64(witness_bytes([P.schnorr(k2, R.tap_sighash(tx, 0, [(0, R.address_script(addr))], 1, R.leaf_hash(script)), AUX) + b'\x01',
                                           sig(k1), script, control]))))
    # The legacy format, P2PKH only, and the same signature under a wrong message.
    secret = 0x9603 * 0x10001 % D.N
    p2pkh = P.p2pkh(S_hash160(P.pubkey(secret)))
    address = b58check(b'\x00' + p2pkh[3:23])
    out.append(('legacy P2PKH', address, msg, R.legacy_sign(secret, msg)))
    out.append(('legacy P2PKH, wrong message', address, msg + '!', R.legacy_sign(secret, msg)))
    return out


def constructed_edges():
    """Each rule of the tapscript evaluator, the key path and to_sign's shape
    at its edge: one case either side where a limit has two."""
    out = []
    msg = 'Chapter 96'
    k = 0x96 * 0x0101010101010101010101010101010101010101010101010101010101010101 % D.N
    k1, k2 = 0x9601 * 0x10001 % D.N, 0x9602 * 0x10001 % D.N
    x1 = D.mul(k1, D.G)[0].to_bytes(32, 'big')
    def leaf(desc, script, stack):
        """A one-leaf tree; a callable stack item is a signature, made from
        the leaf's signature hash of the hash type it is given."""
        addr, control = tap_tree(k, script)
        tx = full_tx(addr, msg, [])
        spent = [(0, R.address_script(addr))]
        h = lambda t: R.tap_sighash(tx, 0, spent, t, R.leaf_hash(script))
        items = [x(h) if callable(x) else x for x in stack]
        out.append((desc, addr, msg, 'smp' + b64(witness_bytes(items + [script, control]))))
    sig1 = lambda h: P.schnorr(k1, h(0), AUX)
    sig2 = lambda h: P.schnorr(k2, h(0), AUX)
    key1 = b'\x20' + x1
    leaf('P2TR leaf CHECKSIGVERIFY then OP_1', key1 + b'\xad\x51', [sig1])
    leaf('P2TR leaf CHECKSIGVERIFY, the wrong key\'s signature', key1 + b'\xad\x51', [sig2])
    leaf('P2TR leaf CHECKSIGVERIFY, an empty signature', key1 + b'\xad\x51', [b''])
    leaf('P2TR leaf CHECKSIG, an empty signature is false', key1 + b'\xac\x00\x87', [b''])
    leaf('P2TR leaf CHECKSIG, a 65-byte signature of hash type 2', key1 + b'\xac', [lambda h: P.schnorr(k1, h(2), AUX) + b'\x02'])
    leaf('P2TR leaf CHECKSIG, a 65-byte SIGHASH_ALL signature', key1 + b'\xac', [lambda h: P.schnorr(k1, h(1), AUX) + b'\x01'])
    leaf('P2TR leaf CHECKSIG, a 63-byte signature', key1 + b'\xac', [lambda h: sig1(h)[:63]])
    leaf('P2TR leaf CHECKSIG, a 66-byte signature', key1 + b'\xac', [lambda h: sig1(h) + b'\x01\x01'])
    leaf('P2TR leaf CHECKSIGADD, a count that is not minimal', key1 + b'\xba\x75\x51', [b'', b'\x00'])
    leaf('P2TR leaf CHECKSIGADD, a count of 5 bytes', key1 + b'\xba\x75\x51', [b'', b'\x01\x00\x00\x00\x01'])
    leaf('P2TR leaf CHECKSIGADD, an empty signature adds nothing', key1 + b'\xba\x52\x9c', [b'', b'\x02'])
    leaf('P2TR leaf CHECKSIGADD adds one to the count', key1 + b'\xba\x53\x9c', [sig1, b'\x02'])
    leaf('P2TR leaf CHECKSIG on one item', b'\xac', [b'\x01'])
    leaf('P2TR leaf CHECKSIGADD on two items', b'\xba', [b'\x01', b'\x01'])
    leaf('P2TR leaf IF ELSE ENDIF, the ELSE branch', b'\x63\x00\x67\x51\x68', [b''])
    leaf('P2TR leaf IF ELSE ENDIF, the IF branch', b'\x63\x00\x67\x51\x68', [b'\x01'])
    leaf('P2TR leaf NOTIF on false', b'\x64\x51\x68', [b''])
    leaf('P2TR leaf NOTIF on true', b'\x64\x51\x68', [b'\x01'])
    leaf('P2TR leaf IF on a 2-byte argument', b'\x63\x51\x68', [b'\x01\x00'])
    leaf('P2TR leaf IF on an empty stack', b'\x63\x51\x68', [])
    leaf('P2TR leaf ENDIF without IF', b'\x51\x68', [])
    leaf('P2TR leaf ELSE without IF', b'\x51\x67', [])
    leaf('P2TR leaf IF left open', b'\x51\x63\x51', [])
    leaf('P2TR leaf, OP_ROT in a branch not taken', b'\x00\x63\x7b\x68\x51', [])
    leaf('P2TR leaf, a non-minimal push', b'\x4c\x01\x05\x75\x51', [])
    leaf('P2TR leaf, a non-minimal push in a branch not taken', b'\x00\x63\x4c\x01\x05\x68\x51', [])
    leaf('P2TR leaf, a 521-byte push', b'\x4d\x09\x02' + bytes(521) + b'\x75\x51', [])
    leaf('P2TR leaf, a 520-byte push', b'\x4d\x08\x02' + bytes(520) + b'\x75\x51', [])
    leaf('P2TR leaf, a signature check in a branch not taken', b'\x00\x63\xac\x68\x51', [])
    leaf('P2TR leaf OP_VERIFY on false', b'\x00\x69\x51', [])
    leaf('P2TR leaf OP_RETURN', b'\x6a\x51', [])
    leaf('P2TR leaf OP_RETURN in a branch not taken', b'\x00\x63\x6a\x68\x51', [])
    leaf('P2TR leaf DUP SIZE EQUALVERIFY SWAP DROP', b'\x76\x82\x51\x88\x7c\x75\x75\x51', [b'\x07'])
    leaf('P2TR leaf leaves two elements', b'\x51\x51', [])
    leaf('P2TR leaf leaves a false element', b'\x00', [])
    leaf('P2TR leaf, a 521-byte witness item', b'\x75\x51', [bytes(521)])
    leaf('P2TR leaf, a 520-byte witness item', b'\x75\x51', [bytes(520)])
    leaf('P2TR leaf, a script that runs off its end', b'\x51\x4c', [])
    leaf('P2TR leaf with OP_SUCCESS254', b'\xfe\x51', [])
    # The validation weight: 50 for each signature checked, against 50 and
    # the witness's size. Ten checks after DUP, then one more, and a pad
    # item: 10 bytes of pad leaves exactly 0, 9 bytes leaves -1.
    for pad, desc in ((10, 'exactly the validation weight'), (9, 'one over the validation weight')):
        script = (b'\x76' + key1 + b'\xad') * 10 + key1 + b'\xac\x7c\x75'
        leaf('P2TR leaf, eleven signature checks, ' + desc, script, [bytes(pad), sig1])
    # The key path.
    secret = 0x9604 * 0x10001 % D.N
    q, xq = P.taproot_tweak(secret, b'')
    addr = bech32_encode(1, xq)
    tx = full_tx(addr, msg, [])
    spent = [(0, R.address_script(addr))]
    h = lambda t: R.tap_sighash(tx, 0, spent, t)
    key_path = lambda desc, sig: out.append((desc, addr, msg, 'smp' + b64(witness_bytes([sig]))))
    key_path('P2TR key path', P.schnorr(q, h(0), AUX))
    key_path('P2TR key path, a 65-byte SIGHASH_ALL signature', P.schnorr(q, h(1), AUX) + b'\x01')
    key_path('P2TR key path, a 65-byte signature of hash type 0', P.schnorr(q, h(0), AUX) + b'\x00')
    key_path('P2TR key path, a 65-byte signature of hash type 0x81', P.schnorr(q, h(0x81), AUX) + b'\x81')
    key_path('P2TR key path, SIGHASH_ALL on the default hash', P.schnorr(q, h(0), AUX) + b'\x01')
    key_path('P2TR key path, a 63-byte signature', P.schnorr(q, h(0), AUX)[:63])
    key_path('P2TR key path, the untweaked key\'s signature', P.schnorr(secret, h(0), AUX))
    t = full_tx(addr, msg, [P.schnorr(q, h(0), AUX)]); t['ins'][0]['script'] = b'\x51'
    out.append(('P2TR key path with a scriptSig', addr, msg, 'ful' + b64(T.serialize(t, True))))
    out.append(('P2TR with an empty witness', addr, msg, 'ful' + b64(T.serialize(full_tx(addr, msg, []), True))))
    script_addr, control = tap_tree(k, b'\x51')
    out.append(('P2TR leaf, a control block of 32 bytes', script_addr, msg, 'smp' + b64(witness_bytes([b'\x51', control[:32]]))))
    out.append(('P2TR leaf, a control block of 34 bytes', script_addr, msg, 'smp' + b64(witness_bytes([b'\x51', control + b'\x00']))))
    # to_sign's shape, on a P2WSH OP_1 address.
    one = b'\x51'
    wsh_one = bech32_encode(0, hashlib.sha256(one).digest())
    def shaped(desc, change, version=0):
        t = full_tx(wsh_one, msg, [one], version)
        change(t)
        out.append((desc, wsh_one, msg, 'ful' + b64(T.serialize(t, True))))
    shaped('to_sign in version 3', lambda t: None, 3)
    shaped('to_sign with two outputs', lambda t: t['outs'].append(dict(value=bytes(8), script=b'\x6a')))
    shaped('to_sign paying 1', lambda t: t['outs'][0].update(value=(1).to_bytes(8, 'little')))
    shaped('to_sign paying to OP_1', lambda t: t['outs'][0].update(script=b'\x51'))
    shaped('to_sign paying to OP_RETURN OP_1', lambda t: t['outs'][0].update(script=b'\x6a\x51'))
    shaped('to_sign spending output 1', lambda t: t['ins'][0].update(prevout=t['ins'][0]['prevout'][:32] + (1).to_bytes(4, 'little')))
    shaped('to_sign spending another transaction', lambda t: t['ins'][0].update(prevout=bytes([t['ins'][0]['prevout'][0] ^ 1]) + t['ins'][0]['prevout'][1:]))
    shaped('a full to_sign with two inputs', lambda t: t['ins'].append(dict(t['ins'][0])))
    shaped('a full to_sign with no outputs', lambda t: t['outs'].clear())
    out.append(('a full signature that does not parse', wsh_one, msg, 'ful' + b64(b'\x02\x00\x00\x00\x01')))
    out.append(('a simple signature with a byte after the witness', wsh_one, msg, 'smp' + b64(witness_bytes([one]) + b'\x00')))
    out.append(('a simple signature of one empty item', wsh_one, msg, 'smp' + b64(witness_bytes([b'']))))
    out += constructed_limits(msg, k, k1, key1, leaf, sig1, wsh_one, BOUNDS)
    # P2SH-P2WSH, with and without OP_CODESEPARATOR.
    for desc, ws in (('P2SH-P2WSH OP_1', b'\x51'), ('P2SH-P2WSH with OP_CODESEPARATOR', b'\xab\x51')):
        redeem = b'\x00\x20' + hashlib.sha256(ws).digest()
        address = b58check(b'\x05' + S_hash160(redeem))
        t = full_tx(address, msg, [ws]); t['ins'][0]['script'] = P.push(redeem)
        out.append((desc, address, msg, 'ful' + b64(T.serialize(t, True))))
    return out


def constructed_limits(msg, k, k1, key1, leaf, sig1, wsh_one, bounds):
    """The evaluator's remaining limits, the decoder's sizes and the proof
    of funds' spent outputs."""
    out = []
    # OP_SUCCESSx at each edge of its ranges, after an OP_VERIFY that fails:
    # the scan before the run makes each one inconclusive, any other opcode
    # leaves the failure.
    for op in (79, 80, 81, 97, 98, 99, 125, 126, 127, 128, 129, 130, 131, 134, 135, 136, 137, 138, 139, 140,
               141, 142, 143, 148, 149, 153, 154, 186, 187, 254, 255):
        leaf(f'P2TR leaf, opcode {op} after an OP_VERIFY that fails', b'\x00\x69' + bytes([op]), [])
    leaf('P2TR leaf OP_16', b'\x60', [])
    leaf('P2TR leaf OP_1NEGATE then DROP', b'\x4f\x75\x51', [])
    leaf('P2TR leaf, a 1-byte push by PUSHDATA4', b'\x4e\x01\x00\x00\x00\x05\x75\x51', [])
    leaf('P2TR leaf CHECKSIG, a 1-byte signature', key1 + b'\xac\x00\x87', [b'\x01'])
    leaf('P2TR leaf CHECKSIG, SIGHASH_ALL\'s signature marked hash type 2', key1 + b'\xac',
         [lambda h: P.schnorr(k1, h(1), AUX) + b'\x02'])
    leaf('P2TR leaf IF on true', b'\x63\x51\x68', [b'\x01'])
    leaf('P2TR leaf NOTIF on a 2-byte argument', b'\x64\x51\x68', [b'\x01\x00'])
    leaf('P2TR leaf with OP_CHECKMULTISIGVERIFY', b'\x00\x00\xaf\x51', [])
    # Seven empty signatures cost nothing; charged 50 each they would be
    # one over the validation weight.
    leaf('P2TR leaf, seven checks of empty signatures', (b'\x00' + key1 + b'\xac\x75') * 7 + b'\x51', [])
    # The stack's 1000 elements: from the witness, and while the script runs.
    leaf('P2TR leaf, 1000 witness items', b'\x75' * 999, [b'\x01'] + [b''] * 999)
    leaf('P2TR leaf, 1001 witness items', b'\x75' * 1000, [b'\x01'] + [b''] * 1000)
    leaf('P2TR leaf, 999 witness items and a push to 1000', b'\x51' + b'\x75' * 999, [b'\x01'] + [b''] * 998)
    leaf('P2TR leaf, 1000 witness items and a push to 1001', b'\x51' + b'\x75' * 1000, [b'\x01'] + [b''] * 999)
    # The annex is a last item of 2 or more starting 0x50.
    script_addr, control = tap_tree(k, b'\x51')
    out.append(('P2TR, two items, the last 0x50 alone', script_addr, msg, 'smp' + b64(witness_bytes([b'\x51', b'\x50']))))
    out.append(('P2TR, two items, the last empty', script_addr, msg, 'smp' + b64(witness_bytes([b'\x51', b'']))))
    out.append(('P2TR key path, a signature starting 0x50', script_addr, msg, 'smp' + b64(witness_bytes([b'\x50' + bytes(63)]))))
    # The decoder: the prefix alone, bytes that aren't a PSBT, a witness that
    # doesn't parse, an empty stack and a missing item for witness version 2.
    v2 = bech32_encode(2, bytes(range(32)))
    out.append(('the prefix alone', wsh_one, msg, 'smp'))
    out.append(('a proof of funds that is not a PSBT', wsh_one, msg, 'pof' + b64(b'not a psbt')))
    out.append(('an unprefixed signature whose witness does not parse', wsh_one, msg, b64(b'\x02\x01')))
    out.append(('witness version 2, an empty stack', v2, msg, 'smp' + b64(b'\x00')))
    out.append(('witness version 2, a stack of one missing item', v2, msg, 'smp' + b64(b'\x01')))
    out.append(('a full to_sign with no inputs and no outputs', wsh_one, msg, 'ful' + b64(bytes.fromhex('02000000000000000000'))))
    # The decoder's sizes: a simple signature of 8129 bytes makes to_sign
    # 8192 bytes, the most it holds; a full signature of 8192 bytes fits.
    big = lambda n: (b'\x4d\x08\x02' + bytes(520) + b'\x75') * (n // 524) + b'\x4d' + (n % 524 - 4).to_bytes(2, 'little') + bytes(n % 524 - 4) + b'\x75'
    ws = big(8125 - 1) + b'\x51'
    assert len(witness_bytes([ws])) == 8129
    addr = bech32_encode(0, hashlib.sha256(ws).digest())
    out.append(('a simple signature of 8129 bytes', addr, msg, 'smp' + b64(witness_bytes([ws]))))
    for n in range(8000, 8200):
        ws3 = big(n - 1) + b'\x51'
        a3 = bech32_encode(0, hashlib.sha256(ws3).digest())
        full = T.serialize(full_tx(a3, msg, [ws3]), True)
        if len(full) == 8192: break
    assert len(full) == 8192
    out.append(('a full signature of 8192 bytes', a3, msg, 'ful' + b64(full)))
    # Past what the reference limits: to_sign of 8193 bytes and more, and a
    # signature of more than 8192 bytes.
    for n, reason in ((8130, R.R_SCRIPT), (8192, R.R_SCRIPT), (8193, R.R_BASE64)):
        ws4 = big(n - 4 - 1) + b'\x51'
        assert len(witness_bytes([ws4])) == n
        bounds.append((f'a simple signature of {n} bytes', bech32_encode(0, hashlib.sha256(ws4).digest()), msg,
                       'smp' + b64(witness_bytes([ws4])), R.INVALID, reason, 'smp'))
    out += proofs_of_funds(msg, bounds)
    out.append(('P2WPKH, a key that reads as a script with OP_CODESEPARATOR', *codesep_key(msg)))
    return out


def p2wpkh_spk(secret): return b'\x00\x14' + S_hash160(P.pubkey(secret))


def codesep_key(msg):
    """A P2WPKH address whose public key, read as a script, parses and holds
    OP_CODESEPARATOR: only a P2WSH witnessScript is scanned for it."""
    for d in range(0x9700, 0x9800):
        secret = d * 0x10001 % D.N
        key = P.pubkey(secret)
        try: ops = [op for op, _ in R.iter_ops(key)]
        except R.S.Fail: continue
        if 0xab in ops: break
    address = bech32_encode(0, S_hash160(key))
    return address, msg, R.sign(address, [secret], msg, 'smp')


def proofs_of_funds(msg, bounds):
    """Proofs of funds whose inputs reach their spent outputs each way:
    their own non-witness or witness UTXO, or an earlier input's
    non-witness UTXO of the same transaction."""
    out = []
    s0 = 0x9801 * 0x10001 % D.N
    address = bech32_encode(0, S_hash160(P.pubkey(s0)))
    spk0 = R.address_script(address)
    spend = R.to_spend(spk0, msg)
    def prev_tx(outs, tag):
        tx = dict(version=(2).to_bytes(4, 'little'), locktime=bytes(4),
                  ins=[dict(prevout=bytes([tag]) * 32 + bytes(4), script=b'', sequence=b'\xff' * 4, witness=[b'\x01'])],
                  outs=[dict(value=v.to_bytes(8, 'little'), script=spk) for v, spk in outs])
        return T.serialize(tx, True)
    def build(inputs, final=True):
        """inputs: (prev bytes, index, secret or a final scriptSig, give its own UTXO: 'nw', 'w' or '')."""
        tx = R.to_sign(spend, 2, 0, 0)
        for prev, index, _, _ in inputs:
            tx['ins'].append(dict(prevout=T.txid(T.unserialize(prev, True)) + index.to_bytes(4, 'little'), script=b'', sequence=bytes(4), witness=[]))
        m = P.create(T.serialize(tx, False))
        P.put(m['ins'][0], b'\x01', bytes(8) + T.compact(len(spk0)) + spk0)
        for n, (prev, index, _, how) in enumerate(inputs, 1):
            P.put(m['ins'][n], b'\x00', prev)
            if how == 'w':
                o = T.unserialize(prev, True)['outs'][index]
                m['ins'][n] = [(b'\x01', o['value'] + T.compact(len(o['script'])) + o['script'])]
        assert P.sign(m, 0, s0) == P.SC['OK']
        for n, (prev, index, secret, how) in enumerate(inputs, 1):
            if isinstance(secret, bytes): P.put(m['ins'][n], b'\x07', secret)
            else: assert P.sign(m, n, secret) == P.SC['OK']
        if final: assert P.finalize(m)
        for n, (prev, index, secret, how) in enumerate(inputs, 1):
            if how == '': m['ins'][n] = [kv for kv in m['ins'][n] if kv[0] != b'\x00']
        return 'pof' + b64(P.write(m))
    keys = [(0x9810 + j) * 0x10001 % D.N for j in range(20)]
    prev = prev_tx([(50000, p2wpkh_spk(keys[0])), (60000, P.p2pkh(S_hash160(P.pubkey(keys[1])))), (70000, p2wpkh_spk(keys[2]))], 1)
    other = prev_tx([(40000, p2wpkh_spk(keys[3]))], 2)
    out.append(('a proof of funds: two inputs reach their outputs through an earlier input\'s UTXO', address, msg,
                build([(other, 0, keys[3], 'nw'), (prev, 0, keys[0], 'nw'), (prev, 1, keys[1], ''), (prev, 2, keys[2], '')])))
    out.append(('a proof of funds: an input with no UTXO and none before it', address, msg,
                build([(prev, 1, keys[1], ''), (prev, 0, keys[0], 'nw')])))
    for size in (64,):
        data = bytes([0x5a]) * (size - 2)
        spk = bytes([len(data)]) + data + b'\x87'
        held = prev_tx([(30000, spk)], 3)
        out.append((f'a proof of funds: a spent scriptPubKey of {size} bytes', address, msg,
                    build([(held, 0, bytes([len(data)]) + data, 'w')])))
    # Past what the reference checks: an output index past the earlier
    # input's transaction, a spent scriptPubKey of 65 bytes, and a proof
    # that isn't finalized.
    bounds.append(('a proof of funds: an output index past the earlier input\'s transaction', address, msg,
                   build([(prev, 0, keys[0], 'nw'), (prev, 5, b'\x51', '')]), R.INVALID, R.R_SHAPE, 'pof'))
    data = bytes([0x5a]) * 63
    held = prev_tx([(30000, bytes([63]) + data + b'\x87')], 3)
    bounds.append(('a proof of funds: a spent scriptPubKey of 65 bytes', address, msg,
                   build([(held, 0, bytes([63]) + data, 'w')]), R.INVALID, R.R_SHAPE, 'pof'))
    bounds.append(('a proof of funds that is not finalized', address, msg,
                   build([(prev, 0, keys[0], 'nw')], final=False), R.INVALID, R.R_FULL, 'pof'))
    many = prev_tx([(10000 + j, p2wpkh_spk(keys[4 + j])) for j in range(15)], 4)
    out.append(('a proof of funds of 16 inputs', address, msg, build([(many, j, keys[4 + j], 'w') for j in range(15)])))
    return out


BOUNDS = []


def S_hash160(b): return R.S.ripemd160(hashlib.sha256(b).digest())


def b58check(payload):
    alphabet = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'
    raw = payload + T.sha256d(payload)[:4]
    n = int.from_bytes(raw, 'big')
    s = ''
    while n: n, r = divmod(n, 58); s = alphabet[r] + s
    return '1' * (len(raw) - len(raw.lstrip(b'\0'))) + s


def esc(s): return s.replace('\\', '\\\\').replace('"', '\\"')


def build():
    basic = json.loads((UP / 'basic-test-vectors.json').read_text())
    generated = json.loads((UP / 'generated-test-vectors.json').read_text())
    hashes = []
    for v in basic['tx_hashes']:
        spk = R.address_script(v['address'])
        spend = R.to_spend(spk, v['message'])
        assert R.message_hash(v['message']).hex() == v['message_hash']
        assert T.sha256d(spend)[::-1].hex() == v['to_spend_tx_hash']
        assert T.txid(R.to_sign(spend))[::-1].hex() == v['to_sign_tx_hash']
        hashes.append(v)
    cases = []
    for source, doc, kind in (('basic', basic, 'simple'), ('generated', generated, 'simple'),
                              ('generated', generated, 'full'), ('generated', generated, 'proof_of_funds')):
        for v in doc[kind]:
            script = v['witness_script'] or v.get('sig_script', '')
            for s in v['bip322_signatures']:
                state, reason, lt, seq, fmt = R.verify(v['address'], s, v['message'])
                assert state == R.VALID and lt == v.get('lock_time', 0) and seq == v.get('sequence', 0), v
                ours = ''
                if kind != 'proof_of_funds':
                    secrets = [R.wif_secret(x) for x in v['private_keys']]
                    ours = R.sign(v['address'], secrets, v['message'], 'smp' if kind == 'simple' else 'ful',
                                  bytes.fromhex(script), v.get('tx_version', 0), v.get('lock_time', 0), v.get('sequence', 0))
                    assert R.verify(v['address'], ours, v['message'])[0] == R.VALID
                    if 'p2tr' not in v['type']:
                        published = [x if x[:3] in ('smp', 'ful') else 'smp' + x for x in v['bip322_signatures']]
                        assert ours in published, (v['type'], ours)
                cases.append(dict(source=source + ' ' + kind, type=v['type'], address=v['address'], message=v['message'],
                                  keys=''.join(v['private_keys']) if ours else '', script=script, version=v.get('tx_version', 0),
                                  locktime=v.get('lock_time', 0), sequence=v.get('sequence', 0), signature=s, ours=ours,
                                  state=0, reason=0, format=FORMAT[fmt], t=lt, s=seq))
    errors = []
    allowed = {'invalid signature': (R.R_SCRIPT, R.R_SHAPE)}
    for source, doc in (('basic', basic), ('generated', generated)):
        for v in doc['error']:
            state, reason, _, _, fmt = R.verify(v['address'], v['signature'], v['message'])
            assert state == R.INVALID and reason in allowed.get(v['error_substr'], (R.ERROR_REASONS[v['error_substr']],)), v
            errors.append(dict(source=source, description=v['description'], address=v['address'], message=v['message'],
                               signature=v['signature'], reason=reason, format=FORMAT[fmt]))
    built, edges = [], []
    BOUNDS.clear()
    for rows, made in ((built, constructed()), (edges, constructed_edges())):
        for desc, address, message, signature in made:
            state, reason, lt, seq, fmt = R.verify(address, signature, message)
            rows.append(dict(source='constructed', type=desc, address=address, message=message, keys='', script='',
                             version=0, locktime=0, sequence=0, signature=signature, ours='', state=STATE[state],
                             reason=reason, format=FORMAT[fmt], t=lt, s=seq))
    bounds = [dict(source='constructed', type=desc, address=address, message=message, keys='', script='', version=0,
                   locktime=0, sequence=0, signature=signature, ours='', state=STATE[state], reason=reason,
                   format=FORMAT[fmt], t=0, s=0) for desc, address, message, signature, state, reason, fmt in BOUNDS]
    return hashes, cases, errors, built, edges, bounds


def render(hashes, cases, errors, built, edges, bounds):
    lines = ['// Generated by make_bip322_vectors.py from the pinned BIP-322 test vectors',
             '// (coax/tests/vectors/upstream/bip322/, listed in SOURCES.md) and from',
             '// cases it constructs. Published values, copied; each state and reason is',
             "// bip322_reference.py's verdict, which agrees with the published one. ours",
             "// is the reference signer's signature from the published keys (WIF,",
             '// 52 characters each). Test keys only.',
             '// A published message: its hash, and the txids of to_spend and of to_sign',
             '// before it is signed (displayed, so byte-reversed).',
             'B322Hash :: struct { message: string; address: string; message_hash: string; to_spend: string; to_sign: string; }',
             f'B322_HASHES :: [{len(hashes)}] B322Hash.[']
    for v in hashes:
        lines.append(f'  B322Hash.{{message="{esc(v["message"])}", address="{v["address"]}", message_hash="{v["message_hash"]}", '
                     f'to_spend="{v["to_spend_tx_hash"]}", to_sign="{v["to_sign_tx_hash"]}"}},')
    lines.append('];')
    lines += ['// A signature: where it comes from, the address type, what signs it, to_sign\'s',
              '// version, lock time and first sequence, the signature, the reference signer\'s',
              '// own (empty for a proof of funds and a constructed case), and the verdict:',
              '// the state, the reason, the format and, when valid, the time and age.',
              'B322Case :: struct { source: string; type: string; address: string; message: string; keys: string; script: string;',
              '  version: int; locktime: int; sequence: int; signature: string; ours: string; state: int; reason: int; format: int; t: int; s: int; }']
    for name, rows in (('B322_CASES', cases), ('B322_BUILT', built), ('B322_EDGES', edges), ('B322_BOUNDS', bounds)):
        if name == 'B322_BOUNDS':
            lines += ['// Past the reference\'s reach: the sizes this verifier holds, and proofs of',
                      '// funds the reference does not check. Each verdict is set by the generator.']
        lines.append(f'{name} :: [{len(rows)}] B322Case.[')
        for c in rows:
            lines.append(f'  B322Case.{{source="{c["source"]}", type="{esc(c["type"])}", address="{c["address"]}", '
                         f'message="{esc(c["message"])}", keys="{c["keys"]}", script="{c["script"]}", version={c["version"]}, '
                         f'locktime={c["locktime"]}, sequence={c["sequence"]}, signature="{c["signature"]}", ours="{c["ours"]}", '
                         f'state={c["state"]}, reason={c["reason"]}, format={c["format"]}, t={c["t"]}, s={c["s"]}}},')
        lines.append('];')
    lines += ['// A published error: the signature is invalid, for the reason given.',
              'B322Error :: struct { source: string; description: string; address: string; message: string; signature: string; reason: int; format: int; }',
              f'B322_ERRORS :: [{len(errors)}] B322Error.[']
    for e in errors:
        lines.append(f'  B322Error.{{source="{e["source"]}", description="{esc(e["description"])}", address="{e["address"]}", '
                     f'message="{esc(e["message"])}", signature="{e["signature"]}", reason={e["reason"]}, format={e["format"]}}},')
    lines.append('];')
    return '\n'.join(lines) + '\n'


def main():
    hashes, cases, errors, built, edges, bounds = build()
    text = render(hashes, cases, errors, built, edges, bounds)
    if '--write' in sys.argv: OUT.write_text(text)
    assert OUT.read_text() == text, 'bip322_vectors.coax is not current: run with --write'
    states = [sum(1 for c in built if c['state'] == k) for k in range(3)]
    print(f'bip322_vectors.coax matches: {len(hashes)} hashes, {len(cases)} signatures, {len(errors)} errors, '
          f'{len(built)} constructed ({states[0]} valid, {states[1]} invalid, {states[2]} inconclusive), '
          f'{len(edges)} at the edges, {len(bounds)} past the reference')


if __name__ == '__main__':
    main()
