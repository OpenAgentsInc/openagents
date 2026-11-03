#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: BIP-322's signer and verifier in Python, outside Coax)
"""BIP-322's generic signed messages in Python, for Bitcoin chapter 96.

It follows the specification in
coax/tests/vectors/upstream/bip322/bip-0322.mediawiki (bips revision
5aadddb5): the message hash, the to_spend and to_sign virtual
transactions, the simple (smp), full (ful) and proof-of-funds (pof)
encodings, and the verification process with its three states: valid
at time T and age S, inconclusive, and invalid. Scripts run through
script_reference.py (chapter 36's interpreter in Python) with the
required rules as flags, then again with the upgradeable rules.
Taproot, which chapter 36 leaves to chapters 43 to 45, is checked here:
the key path with BIP-340, and the script path through a control block
and a bounded tapscript evaluator that answers inconclusive for any
opcode it doesn't know, as BIP-322's step 2 allows. The legacy format
is the Bitcoin Signed Message of chapter 22, for P2PKH only.

It shares no code with coax/lib/bitcoin/bip322.coax.
"""
import base64 as B64
import hashlib

import der_reference as D
import psbt_reference as P
import script_reference as S
import tx_reference as T

VALID, INVALID, INCONCLUSIVE = 'valid', 'invalid', 'inconclusive'
# Why a signature is invalid, as bip322.coax numbers them.
REASONS = ['ok', 'base64', 'signature too short', 'full variant does not parse', 'not a BIP-322 transaction',
           'script failed', 'address', 'legacy signature']
R_OK, R_BASE64, R_SHORT, R_FULL, R_SHAPE, R_SCRIPT, R_ADDRESS, R_LEGACY = range(8)
# The errors the published vectors give, and the reason each one is.
ERROR_REASONS = {'base64': R_BASE64, 'error decoding signature as base64': R_BASE64,
                 'signature too short': R_SHORT, 'error parsing signature as full variant': R_FULL,
                 'invalid signature': R_SCRIPT}

F = S.F
REQUIRED = (F['P2SH'] | F['STRICTENC'] | F['DERSIG'] | F['LOW_S'] | F['NULLDUMMY'] | F['MINIMALDATA'] | F['CLEANSTACK']
            | F['CHECKLOCKTIMEVERIFY'] | F['CHECKSEQUENCEVERIFY'] | F['WITNESS'] | F['MINIMALIF'] | F['NULLFAIL']
            | F['CONST_SCRIPTCODE'])
UPGRADEABLE = F['DISCOURAGE_UPGRADABLE_NOPS'] | F['DISCOURAGE_UPGRADABLE_WITNESS_PROGRAM']


# ---- addresses and keys
def b58decode(text):
    alphabet = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'
    n = 0
    for c in text: n = n * 58 + alphabet.index(c)
    raw = n.to_bytes((n.bit_length() + 7) // 8, 'big')
    raw = b'\0' * (len(text) - len(text.lstrip('1'))) + raw
    body, check = raw[:-4], raw[-4:]
    assert T.sha256d(body)[:4] == check
    return body


def bech32_decode(text):
    charset = 'qpzry9x8gf2tvdw0s3jn54khce6mua7l'
    hrp, data = text.lower().rsplit('1', 1)
    v = [charset.index(c) for c in data]
    gen = [0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3]
    chk = 1
    for x in [ord(c) >> 5 for c in hrp] + [0] + [ord(c) & 31 for c in hrp] + v:
        top = chk >> 25
        chk = (chk & 0x1ffffff) << 5 ^ x
        for i in range(5): chk ^= gen[i] if (top >> i) & 1 else 0
    version = v[0]
    assert chk == (1 if version == 0 else 0x2bc830a3)
    acc, bits, prog = 0, 0, []
    for x in v[1:-6]:
        acc, bits = (acc << 5) | x, bits + 5
        while bits >= 8: bits -= 8; prog.append((acc >> bits) & 255)
    return version, bytes(prog)


def address_script(address):
    """The mainnet scriptPubKey of an address."""
    if address.lower().startswith('bc1'):
        version, prog = bech32_decode(address)
        return bytes([0 if version == 0 else 0x50 + version, len(prog)]) + prog
    body = b58decode(address)
    if body[0] == 0: return b'\x76\xa9\x14' + body[1:] + b'\x88\xac'
    assert body[0] == 5
    return b'\xa9\x14' + body[1:] + b'\x87'


def wif_secret(text):
    return int.from_bytes(b58decode(text)[1:33], 'big')


# ---- the virtual transactions
def message_hash(message):
    return P.tagged('BIP0322-signed-message', message.encode())


def to_spend(challenge, message):
    """to_spend's bytes: version 0, one input from 000...000:FFFFFFFF whose
    scriptSig is OP_0 PUSH32[message_hash], one output of 0 to the challenge."""
    tx = dict(version=bytes(4), locktime=bytes(4),
              ins=[dict(prevout=bytes(32) + b'\xff' * 4, script=b'\x00\x20' + message_hash(message),
                        sequence=bytes(4), witness=[])],
              outs=[dict(value=bytes(8), script=challenge)])
    return T.serialize(tx, True)


def to_sign(spend, version=0, locktime=0, sequence=0):
    """to_sign as a dict: one input spending to_spend's output 0, one output of 0 to OP_RETURN."""
    return dict(version=version.to_bytes(4, 'little'), locktime=locktime.to_bytes(4, 'little'),
                ins=[dict(prevout=T.sha256d(spend) + bytes(4), script=b'', sequence=sequence.to_bytes(4, 'little'),
                          witness=[])],
                outs=[dict(value=bytes(8), script=b'\x6a')])


# ---- Taproot
def tap_sighash(tx, n_in, spent, hashtype, leaf=None):
    """BIP-341's signature hash, with BIP-342's extension for a leaf."""
    sha = lambda b: hashlib.sha256(b).digest()
    ser = lambda o: o['value'] + T.compact(len(o['script'])) + o['script']
    anyone, out_type = hashtype & 0x80, hashtype & 3
    m = bytes([0, hashtype]) + tx['version'] + tx['locktime']
    if not anyone:
        m += sha(b''.join(i['prevout'] for i in tx['ins']))
        m += sha(b''.join(a.to_bytes(8, 'little') for a, _ in spent))
        m += sha(b''.join(T.compact(len(s)) + s for _, s in spent))
        m += sha(b''.join(i['sequence'] for i in tx['ins']))
    if out_type not in (2, 3): m += sha(b''.join(ser(o) for o in tx['outs']))
    m += bytes([2 if leaf else 0])
    if anyone:
        a, s = spent[n_in]
        m += tx['ins'][n_in]['prevout'] + a.to_bytes(8, 'little') + T.compact(len(s)) + s + tx['ins'][n_in]['sequence']
    else:
        m += n_in.to_bytes(4, 'little')
    if out_type == 3: m += sha(ser(tx['outs'][n_in]))
    if leaf: m += leaf + b'\x00' + b'\xff\xff\xff\xff'
    return P.tagged('TapSighash', m)


def leaf_hash(script, version=0xc0):
    return P.tagged('TapLeaf', bytes([version]) + T.compact(len(script)) + script)


def control_ok(control, script, program):
    """The control block commits script to the output key program."""
    h = leaf_hash(script, control[0] & 0xfe)
    for k in range(33, len(control), 32):
        e = control[k:k + 32]
        h = P.tagged('TapBranch', min(h, e) + max(h, e))
    internal = P.lift_x(int.from_bytes(control[1:33], 'big'))
    if internal is None: return False
    t = int.from_bytes(P.tagged('TapTweak', control[1:33] + h), 'big')
    if t >= D.N: return False
    q = D.add(internal, D.mul(t, D.G))
    return q[0].to_bytes(32, 'big') == program and q[1] % 2 == control[0] & 1


class Unknown(Exception):
    """A script the bounded evaluator doesn't understand: inconclusive."""


SUCCESS = {80, 98, 126, 127, 128, 129, 131, 132, 133, 134, 137, 138, 141, 142, 149, 150, 151, 152, 153} | set(range(187, 255))
TAP_KNOWN = {0x4f, 0x61, 0x63, 0x64, 0x67, 0x68, 0x69, 0x6a, 0x75, 0x76, 0x7c, 0x82, 0x87, 0x88, 0x9c, 0x9d,
             0xa8, 0xa9, 0xac, 0xad, 0xb1, 0xb2, 0xba} | set(range(0x51, 0x61))


def tap_schnorr(sig, key, tx, n_in, spent, leaf):
    """A tapscript signature check: False for an empty signature, an
    exception (invalid) for a failing non-empty one."""
    if len(key) == 0: raise S.Fail(S.E['TAPSCRIPT_EMPTY_PUBKEY'])
    if len(key) != 32: raise Unknown()                           # an unknown public key type
    if not sig: return False
    if len(sig) not in (64, 65): raise S.Fail(S.E['SCHNORR_SIG_SIZE'])
    hashtype = sig[64] if len(sig) == 65 else 0
    if len(sig) == 65 and hashtype != 1: raise S.Fail(S.E['SIG_HASHTYPE'])   # BIP-322: ALL or DEFAULT
    if not P.schnorr_verify(key, tap_sighash(tx, n_in, spent, hashtype, leaf), sig[:64]): raise S.Fail(S.E['SCHNORR_SIG'])
    return True


def tapscript(stack, script, tx, n_in, spent, budget):
    """A bounded tapscript evaluator: the opcodes in TAP_KNOWN, with
    BIP-342's rules (MINIMALIF, CHECKSIGADD, the validation weight)."""
    for o in iter_ops(script):
        if o[0] in SUCCESS: raise Unknown()
    vf = []                                    # the open conditionals' values
    num = lambda b: S.num(b, True, 4)
    checker = S.Checker(tx, n_in, spent[n_in][0])
    pc = 0
    while pc < len(script):
        ok, op, data, pc = S.get_op(script, pc)
        if not ok: raise S.Fail(S.E['BAD_OPCODE'])
        running = all(vf)
        if op <= 0x4e:
            if running:
                if len(data) > 520: raise S.Fail(S.E['PUSH_SIZE'])
                if not S.minimal_push(data, op): raise S.Fail(S.E['MINIMALDATA'])
                stack.append(data)
        elif op in (0x63, 0x64):
            v = False
            if running:
                if not stack: raise S.Fail(S.E['UNBALANCED_CONDITIONAL'])
                top = stack.pop()
                if top not in (b'', b'\x01'): raise S.Fail(S.E['TAPSCRIPT_MINIMALIF'])
                v = (top == b'\x01') != (op == 0x64)
            vf.append(v)
        elif op == 0x67:
            if not vf: raise S.Fail(S.E['UNBALANCED_CONDITIONAL'])
            vf[-1] = not vf[-1]
        elif op == 0x68:
            if not vf: raise S.Fail(S.E['UNBALANCED_CONDITIONAL'])
            vf.pop()
        elif op in (0xab,): raise S.Fail(S.E['OP_CODESEPARATOR'])     # BIP-322 forbids it
        elif op in (0xae, 0xaf): raise S.Fail(S.E['TAPSCRIPT_CHECKMULTISIG'])
        elif op not in TAP_KNOWN: raise Unknown()
        elif not running: pass
        elif op == 0x4f or 0x51 <= op <= 0x60: stack.append(S.num_serialize(-1 if op == 0x4f else op - 0x50))
        elif op == 0x61: pass
        elif op in (0xb1, 0xb2):
            if not stack: raise S.Fail(S.E['INVALID_STACK_OPERATION'])
            n = S.num(stack[-1], True, 5)
            if n < 0: raise S.Fail(S.E['NEGATIVE_LOCKTIME'])
            ok = checker.locktime(n) if op == 0xb1 else (n & (1 << 31)) or checker.sequence(n)
            if not ok: raise S.Fail(S.E['UNSATISFIED_LOCKTIME'])
        else:
            need = {0x69: 1, 0x6a: 0, 0x75: 1, 0x76: 1, 0x7c: 2, 0x82: 1, 0x87: 2, 0x88: 2, 0x9c: 2, 0x9d: 2,
                    0xa8: 1, 0xa9: 1, 0xac: 2, 0xad: 2, 0xba: 3}[op]
            if len(stack) < need: raise S.Fail(S.E['INVALID_STACK_OPERATION'])
            if op == 0x69:
                if not S.cast_bool(stack.pop()): raise S.Fail(S.E['VERIFY'])
            elif op == 0x6a: raise S.Fail(S.E['OP_RETURN'])
            elif op == 0x75: stack.pop()
            elif op == 0x76: stack.append(stack[-1])
            elif op == 0x7c: stack[-1], stack[-2] = stack[-2], stack[-1]
            elif op == 0x82: stack.append(S.num_serialize(len(stack[-1])))
            elif op in (0x87, 0x88):
                eq = stack.pop() == stack.pop()
                if op == 0x88:
                    if not eq: raise S.Fail(S.E['EQUALVERIFY'])
                else: stack.append(b'\x01' if eq else b'')
            elif op in (0x9c, 0x9d):
                eq = num(stack.pop()) == num(stack.pop())
                if op == 0x9d:
                    if not eq: raise S.Fail(S.E['NUMEQUALVERIFY'])
                else: stack.append(b'\x01' if eq else b'')
            elif op == 0xa8: stack.append(hashlib.sha256(stack.pop()).digest())
            elif op == 0xa9: stack.append(S.ripemd160(hashlib.sha256(stack.pop()).digest()))
            else:
                key = stack.pop()
                n = num(stack.pop()) if op == 0xba else None
                sig = stack.pop()
                if sig:
                    budget[0] -= 50
                    if budget[0] < 0: raise S.Fail(S.E['TAPSCRIPT_VALIDATION_WEIGHT'])
                ok = tap_schnorr(sig, key, tx, n_in, spent, leaf_hash(script))
                if op == 0xba: stack.append(S.num_serialize(n + (1 if ok else 0)))
                elif op == 0xad:
                    if not ok: raise S.Fail(S.E['CHECKSIGVERIFY'])
                else: stack.append(b'\x01' if ok else b'')
        if len(stack) > 1000: raise S.Fail(S.E['STACK_SIZE'])
    if vf: raise S.Fail(S.E['UNBALANCED_CONDITIONAL'])


def iter_ops(script):
    pc = 0
    while pc < len(script):
        ok, op, data, pc = S.get_op(script, pc)
        if not ok: raise S.Fail(S.E['BAD_OPCODE'])
        yield op, data


def taproot_verify(tx, n_in, spent, program):
    """A P2TR input: VALID, INVALID or INCONCLUSIVE."""
    i = tx['ins'][n_in]
    if i['script']: return INVALID
    stack = list(i['witness'])
    if not stack: return INVALID
    if len(stack) >= 2 and stack[-1][:1] == b'\x50': return INCONCLUSIVE        # the annex
    try:
        if len(stack) == 1:
            sig = stack[0]
            if len(sig) not in (64, 65) or (len(sig) == 65 and sig[64] != 1): return INVALID
            ok = P.schnorr_verify(program, tap_sighash(tx, n_in, spent, sig[64] if len(sig) == 65 else 0), sig[:64])
            return VALID if ok else INVALID
        control, script = stack.pop(), stack.pop()
        if len(control) < 33 or len(control) > 33 + 128 * 32 or (len(control) - 33) % 32: return INVALID
        if not control_ok(control, script, program): return INVALID
        if control[0] & 0xfe != 0xc0: return INCONCLUSIVE                          # an unknown leaf version
        if len(stack) > 1000 or any(len(x) > 520 for x in stack): return INVALID
        witness_size = len(T.compact(len(i['witness']))) + sum(len(T.compact(len(x))) + len(x) for x in i['witness'])
        tapscript(stack, script, tx, n_in, spent, [witness_size + 50])
        return VALID if len(stack) == 1 and S.cast_bool(stack[0]) else INVALID
    except Unknown:
        return INCONCLUSIVE
    except S.Fail:
        return INVALID


# ---- verification
def witness_codesep(i, spk):
    """A P2WSH witnessScript (bare or under P2SH) with OP_CODESEPARATOR,
    which BIP-322 forbids and CONST_SCRIPTCODE refuses only in legacy script."""
    wsh = len(spk) == 34 and spk[:2] == b'\x00\x20'
    nested = len(spk) == 23 and spk[0] == 0xa9 and len(i['script']) == 35 and i['script'][:3] == b'\x22\x00\x20'
    if not (wsh or nested) or not i['witness']: return False
    try: return any(op == 0xab for op, _ in iter_ops(i['witness'][-1]))
    except S.Fail: return False


def verify_input(tx, n_in, spent):
    amount, spk = spent[n_in]
    if len(spk) == 34 and spk[0] == 0x51 and spk[1] == 32: return taproot_verify(tx, n_in, spent, spk[2:])
    i = tx['ins'][n_in]
    if witness_codesep(i, spk): return INVALID
    if S.verify_script(i['script'], spk, i['witness'], REQUIRED, S.Checker(tx, n_in, amount)): return INVALID
    if S.verify_script(i['script'], spk, i['witness'], REQUIRED | UPGRADEABLE, S.Checker(tx, n_in, amount)): return INCONCLUSIVE
    return VALID


def read_witness(b):
    r = T.Reader(b, (1 << 62,) * 3)
    items = [r.bytes() for _ in range(r.compact())]
    if r.at != len(b): raise T.Refused(T.TRAILING, r.at)
    return items


def unbase64(text):
    if len(text) % 4: return None
    try: raw = B64.b64decode(text, validate=True)
    except Exception: return None
    return raw if B64.b64encode(raw).decode() == text else None


def legacy_verify(spk, signature, message):
    """Chapter 22's Bitcoin Signed Message, P2PKH only."""
    sig = unbase64(signature)
    if sig is None or len(sig) != 65 or not 27 <= sig[0] <= 34: return False
    m = b'\x18Bitcoin Signed Message:\n' + T.compact(len(message.encode())) + message.encode()
    z = int.from_bytes(T.sha256d(m), 'big') % D.N
    header = sig[0] - 27
    r, s = int.from_bytes(sig[1:33], 'big'), int.from_bytes(sig[33:], 'big')
    if not (0 < r < D.N and 0 < s < D.N): return False
    x = r + (D.N if header & 2 else 0)
    Rp = P.lift_x(x) if x < D.P else None
    if Rp is None: return False
    if Rp[1] % 2 != header & 1: Rp = (Rp[0], D.P - Rp[1])
    rinv = pow(r, -1, D.N)
    Q = D.add(D.mul(s * rinv % D.N, Rp), D.mul((D.N - z) * rinv % D.N, D.G))
    if Q is None: return False
    key = P.compress(Q) if header & 4 else b'\x04' + Q[0].to_bytes(32, 'big') + Q[1].to_bytes(32, 'big')
    return spk == b'\x76\xa9\x14' + S.ripemd160(hashlib.sha256(key).digest()) + b'\x88\xac'


def verify(address, signature, message):
    """(state, reason, nLockTime, nSequence, format)."""
    try: spk = address_script(address)
    except Exception: return INVALID, R_ADDRESS, 0, 0, None
    fmt = signature[:3] if signature[:3] in ('smp', 'ful', 'pof') else None
    body = signature[3:] if fmt else signature
    if fmt is None and len(spk) == 25 and spk[0] == 0x76 and len(body) == 88:
        ok = legacy_verify(spk, body, message)
        return (VALID if ok else INVALID), (R_OK if ok else R_LEGACY), 0, 0, 'legacy'
    raw = unbase64(body)
    if raw is None: return INVALID, R_BASE64, 0, 0, fmt
    if not raw: return INVALID, R_SHORT, 0, 0, fmt
    spend = to_spend(spk, message)
    spent = [(0, spk)]
    if fmt == 'pof':
        m = P.maps(raw) if P.refusal(raw) is None else None
        if m is None: return INVALID, R_FULL, 0, 0, fmt
        tx = T.unserialize(P.extract(m), True)
        for n in range(1, len(tx['ins'])):
            o = P.spent_output(m, P.Stream(P.unsigned_tx(m)).tx(False), n)
            if o is None:   # a non-witness UTXO given once for the inputs of one transaction
                prev = next((P.get(m['ins'][k], b'\x00') for k in range(1, n)
                             if tx['ins'][k]['prevout'][:32] == tx['ins'][n]['prevout'][:32]
                             and P.get(m['ins'][k], b'\x00') is not None), None)
                if prev is None: return INVALID, R_SHAPE, 0, 0, fmt
                o2 = P.Stream(prev).tx(True)['outs'][int.from_bytes(tx['ins'][n]['prevout'][32:], 'little')]
                o = (int.from_bytes(o2['value'], 'little'), o2['script'], False)
            spent.append((o[0], o[1]))
    elif fmt == 'ful':
        try: tx = T.unserialize(raw, True)
        except T.Refused: return INVALID, R_FULL, 0, 0, fmt
    else:
        try: witness = read_witness(raw)
        except T.Refused: return INVALID, R_SCRIPT, 0, 0, fmt
        tx = to_sign(spend)
        tx['ins'][0]['witness'] = witness
    if (not tx['ins'] or tx['ins'][0]['prevout'] != T.sha256d(spend) + bytes(4)
            or (fmt != 'pof' and len(tx['ins']) != 1) or len(tx['outs']) != 1
            or tx['outs'][0]['value'] != bytes(8) or tx['outs'][0]['script'] != b'\x6a'):
        return INVALID, R_SHAPE, 0, 0, fmt
    states = [verify_input(tx, n, spent) for n in range(len(tx['ins']))]
    if INVALID in states: return INVALID, R_SCRIPT, 0, 0, fmt
    if INCONCLUSIVE in states or int.from_bytes(tx['version'], 'little') not in (0, 2): return INCONCLUSIVE, R_OK, 0, 0, fmt
    return VALID, R_OK, int.from_bytes(tx['locktime'], 'little'), int.from_bytes(tx['ins'][0]['sequence'], 'little'), fmt


# ---- signing
def sign(address, secrets, message, fmt, witness_script=b'', version=0, locktime=0, sequence=0, grind=False, aux=bytes(32)):
    """A simple or full signature from the secrets, in the input's script
    order. Time-lock scripts take their ELSE branch; a P2TR address
    without a witness script is a key path."""
    spk = address_script(address)
    spend = to_spend(spk, message)
    tx = to_sign(spend, version, locktime, sequence)
    i = tx['ins'][0]
    timelock = witness_script[:1] == b'\x63'
    def ecdsa(code, v0):
        out = []
        for d in secrets:
            digest = S.sighash_v0(code, tx, 0, 1, 0) if v0 else S.sighash_legacy(code, tx, 0, 1)
            out.append(P.ecdsa(d, digest, grind) + b'\x01')
        return out
    if spk[0] == 0x51:
        if not witness_script:
            q, _ = P.taproot_tweak(secrets[0], b'')
            i['witness'] = [P.schnorr(q, tap_sighash(tx, 0, [(0, spk)], 0), aux)]
        else:
            internal = D.mul(secrets[0], D.G)
            control = bytes([0xc0 | (taproot_parity(internal, witness_script))]) + internal[0].to_bytes(32, 'big')
            d = secrets[0]
            sig = P.schnorr(d, tap_sighash(tx, 0, [(0, spk)], 0, leaf_hash(witness_script)), aux)
            i['witness'] = [sig, b'', witness_script, control]
    elif len(spk) == 22:
        key = P.pubkey(secrets[0])
        i['witness'] = ecdsa(P.p2pkh(S.ripemd160(hashlib.sha256(key).digest())), True) + [key]
    elif len(spk) == 34:
        sigs = ecdsa(witness_script, True)
        i['witness'] = ([sigs[0], b''] if timelock else [b''] + sigs) + [witness_script]
    elif len(spk) == 25:
        key = P.pubkey(secrets[0])
        i['script'] = P.push(ecdsa(spk, False)[0]) + P.push(key)
    else:   # P2SH: the script is the redeemScript, or a P2SH-P2WSH's witnessScript
        if S.ripemd160(hashlib.sha256(witness_script).digest()) == spk[2:22]:
            redeem = witness_script
            if redeem[:2] == b'\x00\x14':
                key = P.pubkey(secrets[0])
                i['witness'] = ecdsa(P.p2pkh(redeem[2:]), True) + [key]
            else:
                i['script'] = P.push(b'') + b''.join(P.push(s) for s in ecdsa(redeem, False))
            i['script'] += P.push(redeem)
        else:
            redeem = b'\x00\x20' + hashlib.sha256(witness_script).digest()
            assert S.ripemd160(hashlib.sha256(redeem).digest()) == spk[2:22]
            i['witness'] = [b''] + ecdsa(witness_script, True) + [witness_script]
            i['script'] = P.push(redeem)
    if fmt == 'smp':
        w = i['witness']
        return 'smp' + B64.b64encode(T.compact(len(w)) + b''.join(T.compact(len(x)) + x for x in w)).decode()
    return 'ful' + B64.b64encode(T.serialize(tx, True)).decode()


def legacy_sign(secret, message):
    """Chapter 22's signmessage: a recoverable signature, compressed key."""
    m = b'\x18Bitcoin Signed Message:\n' + T.compact(len(message.encode())) + message.encode()
    digest = T.sha256d(m)
    z = int.from_bytes(digest, 'big') % D.N
    k = P.rfc6979(secret, digest)
    R = D.mul(k, D.G)
    r = R[0] % D.N
    s = pow(k, -1, D.N) * (z + r * secret) % D.N
    recid = (R[1] & 1) | (2 if R[0] >= D.N else 0)
    if s > D.N // 2: s, recid = D.N - s, recid ^ 1
    return B64.b64encode(bytes([27 + 4 + recid]) + r.to_bytes(32, 'big') + s.to_bytes(32, 'big')).decode()


def taproot_parity(internal, script):
    t = int.from_bytes(P.tagged('TapTweak', internal[0].to_bytes(32, 'big') + leaf_hash(script)), 'big')
    p = internal if internal[1] % 2 == 0 else (internal[0], D.P - internal[1])
    return D.add(p, D.mul(t, D.G))[1] % 2
