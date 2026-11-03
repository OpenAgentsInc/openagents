#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Bitcoin Core's pinned policy.cpp, solver.cpp and validation.cpp decide each case outside Coax)
"""The cross-check of Bitcoin chapter 81. It builds the example's
transactions again, computes every line the example prints in Python that
shares no code with Coax, and compares the transcript with the fixture:

  python3 -I coax/tests/bitcoin/policy.py                     # compare with the fixture
  python3 -I coax/tests/bitcoin/policy.py --coax coax/bin/coax   # also run the example

It transliterates Solver (script/solver.cpp); GetDustThreshold, IsStandard,
IsStandardTx, ValidateInputsStandardness with CheckSigopsBIP54, and
IsWitnessStandard (policy/policy.cpp); the order of PreChecks
(validation.cpp); and GetTransactionSigOpCost. For "a block", it checks
CheckTransaction's rules, that the inputs pay for the outputs, and runs a
small script interpreter with the opcodes these transactions use, under
P2SH, BIP-141 and the consensus rules for undefined witness programs.
The constants and reason strings are read from the pinned policy.h and
policy.cpp, so a change there shows up here.
"""
import hashlib
import json
import math
import re
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent.parent
CORE = HERE.parent / 'vectors' / 'upstream' / 'bitcoin-core'
FIXTURE = REPO / 'bitcoin' / 'fixtures' / 'standardness.out'
EXAMPLE = REPO / 'bitcoin' / 'examples' / 'standardness.coax'


def pinned(name):
    manifest = json.loads((HERE.parent / 'vectors' / 'manifest.json').read_text())
    pin = {e['path']: e['sha256'] for e in manifest['files']}['upstream/bitcoin-core/' + name]
    data = (CORE / name).read_bytes()
    assert hashlib.sha256(data).hexdigest() == pin, name
    return data.decode()


POLICY_H = pinned('policy.h')
POLICY_CPP = pinned('policy.cpp')


def const(name):
    m = re.search(name + r"\{([0-9']+)\}", POLICY_H)
    return int(m.group(1).replace("'", ''))


MAX_STANDARD_TX_WEIGHT = const('MAX_STANDARD_TX_WEIGHT')
MIN_STANDARD_TX_NONWITNESS_SIZE = const('MIN_STANDARD_TX_NONWITNESS_SIZE')
MAX_P2SH_SIGOPS = const('MAX_P2SH_SIGOPS')
MAX_TX_LEGACY_SIGOPS = const('MAX_TX_LEGACY_SIGOPS')
MAX_STANDARD_P2WSH_STACK_ITEMS = const('MAX_STANDARD_P2WSH_STACK_ITEMS')
MAX_STANDARD_P2WSH_STACK_ITEM_SIZE = const('MAX_STANDARD_P2WSH_STACK_ITEM_SIZE')
MAX_STANDARD_TAPSCRIPT_STACK_ITEM_SIZE = const('MAX_STANDARD_TAPSCRIPT_STACK_ITEM_SIZE')
MAX_STANDARD_P2WSH_SCRIPT_SIZE = const('MAX_STANDARD_P2WSH_SCRIPT_SIZE')
MAX_STANDARD_SCRIPTSIG_SIZE = const('MAX_STANDARD_SCRIPTSIG_SIZE')
DUST_RELAY_TX_FEE = const('DUST_RELAY_TX_FEE')
MAX_DUST_OUTPUTS_PER_TX = const('MAX_DUST_OUTPUTS_PER_TX')
TX_MIN_STANDARD_VERSION = const('TX_MIN_STANDARD_VERSION')
TX_MAX_STANDARD_VERSION = const('TX_MAX_STANDARD_VERSION')
MAX_BLOCK_SIGOPS_COST = 80000
MAX_STANDARD_TX_SIGOPS_COST = MAX_BLOCK_SIGOPS_COST // 5
MAX_OP_RETURN_RELAY = MAX_STANDARD_TX_WEIGHT // 4
MAX_BLOCK_WEIGHT = 4000000
assert (MAX_STANDARD_TX_WEIGHT, MIN_STANDARD_TX_NONWITNESS_SIZE, DUST_RELAY_TX_FEE) == (400000, 65, 3000)
for reason in ['version', 'tx-size', 'scriptsig-size', 'scriptsig-not-pushonly', 'scriptpubkey', 'datacarrier',
               'bare-multisig', 'dust', 'bad-txns-nonstandard-inputs', 'non-witness sigops exceed bip54 limit',
               'input %u script unknown', 'input %u witness program is undefined',
               'p2sh redeemscript sigops exceed limit (input %u: %u > %u)']:
    assert '"' + reason + '"' in POLICY_CPP, reason

OP_0, OP_PUSHDATA1, OP_PUSHDATA2, OP_PUSHDATA4, OP_1NEGATE, OP_RESERVED = 0, 0x4c, 0x4d, 0x4e, 0x4f, 0x50
OP_1, OP_16, OP_NOP, OP_RETURN, OP_2DROP, OP_DROP, OP_DUP = 0x51, 0x60, 0x61, 0x6a, 0x6d, 0x75, 0x76
OP_EQUAL, OP_EQUALVERIFY, OP_HASH160 = 0x87, 0x88, 0xa9
OP_CHECKSIG, OP_CHECKSIGVERIFY, OP_CHECKMULTISIG, OP_CHECKMULTISIGVERIFY = 0xac, 0xad, 0xae, 0xaf


def sha256(b):
    return hashlib.sha256(b).digest()


def hash160(b):
    return hashlib.new('ripemd160', sha256(b)).digest()


def sha256d(b):
    return sha256(sha256(b))


def compact(n):
    if n < 0xfd:
        return bytes([n])
    if n <= 0xffff:
        return b'\xfd' + n.to_bytes(2, 'little')
    if n <= 0xffffffff:
        return b'\xfe' + n.to_bytes(4, 'little')
    return b'\xff' + n.to_bytes(8, 'little')


def push(data):
    """CScript << data: the shortest push."""
    n = len(data)
    if n < OP_PUSHDATA1:
        return bytes([n]) + data
    if n <= 0xff:
        return bytes([OP_PUSHDATA1, n]) + data
    if n <= 0xffff:
        return bytes([OP_PUSHDATA2]) + n.to_bytes(2, 'little') + data
    return bytes([OP_PUSHDATA4]) + n.to_bytes(4, 'little') + data


def get_op(s, pc):
    """CScript::GetOp: (opcode, data, next), or None for a push that runs past the end."""
    if pc >= len(s):
        return None
    op = s[pc]
    pc += 1
    if op > OP_PUSHDATA4:
        return op, b'', pc
    width = {OP_PUSHDATA1: 1, OP_PUSHDATA2: 2, OP_PUSHDATA4: 4}.get(op, 0)
    if len(s) - pc < width:
        return None
    n = op if width == 0 else int.from_bytes(s[pc:pc + width], 'little')
    pc += width
    if len(s) - pc < n:
        return None
    return op, s[pc:pc + n], pc + n


def push_only(s):
    pc = 0
    while pc < len(s):
        r = get_op(s, pc)
        if r is None or r[0] > OP_16:
            return False
        pc = r[2]
    return True


# ---- Solver (script/solver.cpp)
def valid_size(k):
    if not k:
        return False
    return len(k) == {2: 33, 3: 33, 4: 65, 6: 65, 7: 65}.get(k[0], 0)


def witness_program(s):
    if len(s) < 4 or len(s) > 42:
        return None
    if s[0] != OP_0 and not OP_1 <= s[0] <= OP_16:
        return None
    if s[1] + 2 != len(s):
        return None
    return (0 if s[0] == OP_0 else s[0] - 0x50), s[2:]


def is_p2sh(s):
    return len(s) == 23 and s[0] == OP_HASH160 and s[1] == 20 and s[22] == OP_EQUAL


def is_anchor(s):
    return s == bytes([OP_1, 2, 0x4e, 0x73])


def minimal_push(data, op):
    n = len(data)
    if n == 0:
        return op == OP_0
    if n == 1 and 1 <= data[0] <= 16:
        return False
    if n == 1 and data[0] == 0x81:
        return False
    if n <= 75:
        return op == n
    if n <= 255:
        return op == OP_PUSHDATA1
    if n <= 65535:
        return op == OP_PUSHDATA2
    return True


def script_number(op, data, lo, hi):
    if OP_1 <= op <= OP_16:
        v = op - 0x50
    elif op <= OP_PUSHDATA4:
        if not minimal_push(data, op) or len(data) > 4:
            return None
        if data and (data[-1] & 0x7f) == 0 and (len(data) == 1 or not data[-2] & 0x80):
            return None
        v = int.from_bytes(data, 'little') if data else 0
        if data and data[-1] & 0x80:
            v = -(v & ~(0x80 << (8 * (len(data) - 1))))
    else:
        return None
    return v if lo <= v <= hi else None


def match_multisig(s):
    if not s or s[-1] != OP_CHECKMULTISIG:
        return None
    r = get_op(s, 0)
    if r is None:
        return None
    m = script_number(r[0], r[1], 1, 20)
    if m is None:
        return None
    keys = 0
    r = get_op(s, r[2])
    while r is not None and r[0] <= OP_PUSHDATA4 and valid_size(r[1]):
        keys += 1
        r = get_op(s, r[2])
    if r is None:
        return None
    n = script_number(r[0], r[1], m, 20)
    if n is None or n != keys or r[2] + 1 != len(s):
        return None
    return m, n


def solver(s):
    if is_p2sh(s):
        return 'scripthash', None
    wp = witness_program(s)
    if wp:
        v, prog = wp
        if v == 0 and len(prog) == 20:
            return 'witness_v0_keyhash', None
        if v == 0 and len(prog) == 32:
            return 'witness_v0_scripthash', None
        if v == 1 and len(prog) == 32:
            return 'witness_v1_taproot', None
        if is_anchor(s):
            return 'anchor', None
        if v != 0:
            return 'witness_unknown', None
        return 'nonstandard', None
    if len(s) >= 1 and s[0] == OP_RETURN and push_only(s[1:]):
        return 'nulldata', None
    if len(s) == 67 and s[0] == 65 and s[-1] == OP_CHECKSIG and valid_size(s[1:66]):
        return 'pubkey', None
    if len(s) == 35 and s[0] == 33 and s[-1] == OP_CHECKSIG and valid_size(s[1:34]):
        return 'pubkey', None
    if len(s) == 25 and s[:3] == bytes([OP_DUP, OP_HASH160, 20]) and s[23:] == bytes([OP_EQUALVERIFY, OP_CHECKSIG]):
        return 'pubkeyhash', None
    ms = match_multisig(s)
    if ms:
        return 'multisig', ms
    return 'nonstandard', None


def is_standard(s):
    t, ms = solver(s)
    if t == 'nonstandard':
        return False, t
    if t == 'multisig':
        m, n = ms
        if n < 1 or n > 3 or m < 1 or m > n:
            return False, t
    return True, t


def dust_threshold(spk, rate):
    if (spk and spk[0] == OP_RETURN) or len(spk) > 10000:
        return 0
    size = 8 + len(compact(len(spk))) + len(spk)
    size += (32 + 4 + 1 + 107 // 4 + 4) if witness_program(spk) else (32 + 4 + 1 + 107 + 4)
    return math.ceil(rate * size / 1000)     # CFeeRate::GetFee rounds up


# ---- transactions
class Tx:
    def __init__(self, version, ins, outs):
        self.version, self.ins, self.outs = version, ins, outs   # ins: (index, scriptSig, witness items)

    def stripped(self):
        b = self.version.to_bytes(4, 'little') + compact(len(self.ins))
        for index, sig, _ in self.ins:
            b += bytes([0x11]) * 32 + index.to_bytes(4, 'little') + compact(len(sig)) + sig + b'\xff\xff\xff\xff'
        b += compact(len(self.outs))
        for value, spk in self.outs:
            b += value.to_bytes(8, 'little') + compact(len(spk)) + spk
        return b + bytes(4)

    def has_witness(self):
        return any(w for _, _, w in self.ins)

    def full(self):
        s = self.stripped()
        if not self.has_witness():
            return s
        wit = b''.join(compact(len(w)) + b''.join(compact(len(x)) + x for x in w) for _, _, w in self.ins)
        return s[:4] + b'\x00\x01' + s[4:-4] + wit + s[-4:]

    def weight(self):
        return len(self.stripped()) * 3 + len(self.full())


def p2wsh(script):
    return bytes([OP_0, 32]) + sha256(script)


def p2sh(script):
    return bytes([OP_HASH160, 20]) + hash160(script) + bytes([OP_EQUAL])


COIN = 1000000
P2WPKH = bytes.fromhex('0014751e76e8199196d454941c45d1b3a323f1433bd6')
KEY = bytes.fromhex('0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798')


def multisig(m, n):
    return bytes([0x50 + m]) + b''.join(push(KEY) for _ in range(n)) + bytes([0x50 + n, OP_CHECKMULTISIG])


def sigops_redeem(n):
    return bytes([OP_0, OP_0, OP_CHECKSIG, OP_DROP]) * n + bytes([OP_1])


CASES = [
    ('a P2WSH spend to P2WPKH', 'standard'), ('version 0', 'version'), ('version 4', 'version'),
    ('an OP_RETURN of 99,990 bytes', 'tx-size'), ('a scriptSig of 2,014 bytes', 'scriptsig-size'),
    ('a scriptSig with OP_NOP', 'scriptsig-not-pushonly'), ('an output of OP_1', 'scriptpubkey'),
    ('a 2-of-4 bare multisig output', 'scriptpubkey'), ('an 84-byte OP_RETURN, -datacarriersize=83', 'datacarrier'),
    ('a 1-of-1 bare multisig output, -permitbaremultisig=0', 'bare-multisig'), ('two dust outputs', 'dust'),
    ('61 bytes without the witness', 'tx-size-small'), ('a spend of a bare OP_1', 'bad-txns-nonstandard-inputs'),
    ('a spend of a version 2 program', 'bad-txns-nonstandard-inputs'),
    ('a redeem script with 16 sigops', 'bad-txns-nonstandard-inputs'),
    ('167 redeem scripts with 15 sigops each', 'bad-txns-nonstandard-inputs'),
    ('an 81-byte P2WSH item', 'bad-witness-nonstandard'), ('101 P2WSH items', 'bad-witness-nonstandard'),
    ('a P2WSH script of 3,631 bytes', 'bad-witness-nonstandard'), ('a witness on a P2A output', 'bad-witness-nonstandard'),
    ('201 bare multisig outputs', 'bad-txns-too-many-sigops'),
]


def case(k):
    """Case k: the transaction, the outputs it spends (value, scriptPubKey) and the options."""
    opts = {'datacarrier': 83 if k == 8 else MAX_OP_RETURN_RELAY, 'bare': k != 9, 'dust': DUST_RELAY_TX_FEE}
    one = bytes([OP_1])
    if k <= 3 or 6 <= k <= 11:
        version = {1: 0, 2: 4}.get(k, 2)
        outs = {
            3: [(0, bytes([OP_RETURN]) + push(bytes(99990)))],
            6: [(900000, one)],
            7: [(900000, multisig(2, 4))],
            8: [(0, bytes([OP_RETURN]) + push(b'\x42' * 81))],
            9: [(900000, multisig(1, 1))],
            10: [(900000, P2WPKH), (293, P2WPKH), (0, P2WPKH)],
            11: [(0, bytes([OP_RETURN]))],
        }.get(k, [(900000, P2WPKH)])
        return Tx(version, [(k, b'', [one])], outs), [(COIN, p2wsh(one))], opts
    pay = [(900000, P2WPKH)]
    if k == 4:
        sig = b''.join(push(bytes(500)) for _ in range(4)) + push(one)
        return Tx(2, [(k, sig, [])], pay), [(COIN, p2sh(one))], opts
    if k == 5:
        return Tx(2, [(k, bytes([OP_NOP]), [])], pay), [(COIN, one)], opts
    if k == 12:
        return Tx(2, [(k, b'', [])], pay), [(COIN, one)], opts
    if k == 13:
        return Tx(2, [(k, b'', [])], pay), [(COIN, bytes.fromhex('5202beef'))], opts
    if k == 14:
        r = sigops_redeem(16)
        return Tx(2, [(k, push(r), [])], pay), [(COIN, p2sh(r))], opts
    if k == 15:
        r = sigops_redeem(15)
        return Tx(2, [(i, push(r), []) for i in range(167)], pay), [(COIN, p2sh(r))] * 167, opts
    if k == 20:
        return Tx(2, [(k, b'', [one])], [(1000, multisig(1, 1))] * 201), [(COIN, p2wsh(one))], opts
    if k == 16:
        script = bytes([OP_DROP, OP_1]); items = [bytes(81)]
    if k == 17:
        script = bytes([OP_DROP]) * 101 + one; items = [b''] * 101
    if k == 18:
        script = b''.join(push(bytes(515)) for _ in range(7)) + bytes([OP_2DROP] * 3 + [OP_DROP, OP_1]); items = []
        assert len(script) == 3631
    if k == 19:
        return Tx(2, [(k, b'', [b'\xaa'])], pay), [(COIN, bytes.fromhex('51024e73'))], opts
    return Tx(2, [(k, b'', items + [script])], pay), [(COIN, p2wsh(script))], opts


# ---- policy (policy/policy.cpp, validation.cpp PreChecks)
def eval_push_only(sig):
    """EvalScript with SCRIPT_VERIFY_NONE on a push-only scriptSig: the stack, or an error."""
    stack = []
    if len(sig) > 10000:
        return 'SCRIPT_SIZE'
    pc = 0
    while pc < len(sig):
        r = get_op(sig, pc)
        if r is None:
            return 'BAD_OPCODE'
        op, data, pc = r
        if len(data) > 520:
            return 'PUSH_SIZE'
        if op <= OP_PUSHDATA4:
            stack.append(data)
        elif op == OP_1NEGATE:
            stack.append(b'\x81')
        elif OP_1 <= op <= OP_16:
            stack.append(bytes([op - 0x50]))
        else:
            return 'BAD_OPCODE'
        if len(stack) > 1000:
            return 'STACK_SIZE'
    return stack


def sigop_count(s, accurate):
    n, last, pc = 0, 0xff, 0
    while pc < len(s):
        r = get_op(s, pc)
        if r is None:
            break
        op = r[0]
        if op in (OP_CHECKSIG, OP_CHECKSIGVERIFY):
            n += 1
        elif op in (OP_CHECKMULTISIG, OP_CHECKMULTISIGVERIFY):
            n += (last - 0x50) if accurate and OP_1 <= last <= OP_16 else 20
        last, pc = op, r[2]
    return n


def p2sh_sigops(spk, sig):
    """CScript::GetSigOpCount(scriptSig)."""
    if not is_p2sh(spk):
        return sigop_count(spk, True)
    pc, data = 0, b''
    while pc < len(sig):
        r = get_op(sig, pc)
        if r is None or r[0] > OP_16:
            return 0
        data, pc = r[1], r[2]
    return sigop_count(data, True)


def witness_sigops(spk, sig, wit):
    wp = witness_program(spk)
    if wp is None and is_p2sh(spk) and push_only(sig):
        stack = eval_push_only(sig)
        wp = witness_program(stack[-1]) if isinstance(stack, list) and stack else None
    if wp is None or wp[0] != 0:
        return 0
    if len(wp[1]) == 20:
        return 1
    if len(wp[1]) == 32 and wit:
        return sigop_count(wit[-1], True)
    return 0


def sigop_cost(tx, spent):
    cost = sum(sigop_count(sig, False) for _, sig, _ in tx.ins) * 4
    cost += sum(sigop_count(spk, False) for _, spk in tx.outs) * 4
    for (_, sig, wit), (_, spk) in zip(tx.ins, spent):
        if is_p2sh(spk):
            cost += p2sh_sigops(spk, sig) * 4
        cost += witness_sigops(spk, sig, wit)
    return cost


def is_standard_tx(tx, o):
    if not TX_MIN_STANDARD_VERSION <= tx.version <= TX_MAX_STANDARD_VERSION:
        return 'version'
    if tx.weight() > MAX_STANDARD_TX_WEIGHT:
        return 'tx-size'
    for _, sig, _ in tx.ins:
        if len(sig) > MAX_STANDARD_SCRIPTSIG_SIZE:
            return 'scriptsig-size'
        if not push_only(sig):
            return 'scriptsig-not-pushonly'
    left = o['datacarrier']
    for _, spk in tx.outs:
        ok, t = is_standard(spk)
        if not ok:
            return 'scriptpubkey'
        if t == 'nulldata':
            if len(spk) > left:
                return 'datacarrier'
            left -= len(spk)
        elif t == 'multisig' and not o['bare']:
            return 'bare-multisig'
    if sum(1 for v, spk in tx.outs if v < dust_threshold(spk, o['dust'])) > MAX_DUST_OUTPUTS_PER_TX:
        return 'dust'
    return None


def inputs_standard(tx, spent):
    sigops = 0
    for (_, sig, _), (_, spk) in zip(tx.ins, spent):
        sigops += sigop_count(sig, True) + p2sh_sigops(spk, sig)
        if sigops > MAX_TX_LEGACY_SIGOPS:
            return 'non-witness sigops exceed bip54 limit'
    for i, ((_, sig, _), (_, spk)) in enumerate(zip(tx.ins, spent)):
        t, _ = solver(spk)
        if t == 'nonstandard':
            return 'input %d script unknown' % i
        if t == 'witness_unknown':
            return 'input %d witness program is undefined' % i
        if t == 'scripthash':
            stack = eval_push_only(sig)
            if not isinstance(stack, list):
                return 'p2sh scriptsig malformed (input %d: %s)' % (i, stack)
            if not stack:
                return 'input %d P2SH redeemscript missing' % i
            n = sigop_count(stack[-1], True)
            if n > MAX_P2SH_SIGOPS:
                return 'p2sh redeemscript sigops exceed limit (input %d: %d > %d)' % (i, n, MAX_P2SH_SIGOPS)
    return None


def witness_standard(tx, spent):
    for (_, sig, wit), (_, spk) in zip(tx.ins, spent):
        if not wit:
            continue
        if is_anchor(spk):
            return False
        script, wrapped = spk, False
        if is_p2sh(spk):
            stack = eval_push_only(sig)
            if not isinstance(stack, list) or not stack:
                return False
            script, wrapped = stack[-1], True
        wp = witness_program(script)
        if wp is None:
            return False
        v, prog = wp
        if v == 0 and len(prog) == 32:
            if len(wit[-1]) > MAX_STANDARD_P2WSH_SCRIPT_SIZE or len(wit) - 1 > MAX_STANDARD_P2WSH_STACK_ITEMS:
                return False
            if any(len(x) > MAX_STANDARD_P2WSH_STACK_ITEM_SIZE for x in wit[:-1]):
                return False
        if v == 1 and len(prog) == 32 and not wrapped:
            if len(wit) >= 2 and wit[-1] and wit[-1][0] == 0x50:
                return False
            if len(wit) >= 2:
                if not wit[-1]:
                    return False
                if wit[-1][0] & 0xfe == 0xc0 and any(len(x) > MAX_STANDARD_TAPSCRIPT_STACK_ITEM_SIZE for x in wit[:-2]):
                    return False
    return True


def policy(tx, spent, o):
    """PreChecks' standardness steps, in order: the reason and the debug message."""
    r = is_standard_tx(tx, o)
    if r:
        return r, None
    if len(tx.stripped()) < MIN_STANDARD_TX_NONWITNESS_SIZE:
        return 'tx-size-small', None
    r = inputs_standard(tx, spent)
    if r:
        return 'bad-txns-nonstandard-inputs', r
    if tx.has_witness() and not witness_standard(tx, spent):
        return 'bad-witness-nonstandard', None
    cost = sigop_cost(tx, spent)
    if cost > MAX_STANDARD_TX_SIGOPS_COST:
        return 'bad-txns-too-many-sigops', str(cost)
    return 'standard', None


# ---- what a block checks
def run_script(script, stack):
    """The opcodes these transactions use; False on a failure."""
    pc = 0
    while pc < len(script):
        r = get_op(script, pc)
        if r is None:
            return False
        op, data, pc = r
        if len(data) > 520:
            return False
        if op <= OP_PUSHDATA4:
            stack.append(data)
        elif OP_1 <= op <= OP_16:
            stack.append(bytes([op - 0x50]))
        elif op == OP_NOP:
            pass
        elif op == OP_DROP:
            if not stack:
                return False
            stack.pop()
        elif op == OP_2DROP:
            if len(stack) < 2:
                return False
            del stack[-2:]
        elif op == OP_CHECKSIG:      # an empty signature is false, with no error
            if len(stack) < 2:
                return False
            sig = stack[-2]
            del stack[-2:]
            assert sig == b''
            stack.append(b'')
        elif op in (OP_EQUAL, OP_HASH160):
            if op == OP_HASH160:
                stack.append(hash160(stack.pop()))
            else:
                a, b = stack.pop(), stack.pop()
                stack.append(b'\x01' if a == b else b'')
        else:
            return False
    return True


def true(x):
    return any(x[:-1]) or (x and x[-1] not in (0, 0x80))


def verify(spk, sig, wit):
    """VerifyScript under P2SH and WITNESS for the shapes these transactions spend."""
    stack = []
    if not run_script(sig, stack):
        return False
    saved = list(stack)
    if not run_script(spk, stack) or not stack or not true(stack[-1]):
        return False
    witnessed = False

    def program(v, prog):
        if v == 0 and len(prog) == 32:
            if not wit or sha256(wit[-1]) != prog:
                return False
            st = list(wit[:-1])
            if any(len(x) > 520 for x in st) or not run_script(wit[-1], st):
                return False
            return len(st) == 1 and true(st[0])
        assert v != 0 and not (v == 1 and len(prog) == 32)    # no P2WPKH or Taproot spends here
        return True                                            # undefined programs and P2A succeed

    wp = witness_program(spk)
    if wp:
        if sig:
            return False
        witnessed = True
        if not program(*wp):
            return False
    if is_p2sh(spk):
        if not push_only(sig):
            return False
        stack = saved
        redeem = stack.pop()
        if not run_script(redeem, stack) or not stack or not true(stack[-1]):
            return False
        assert witness_program(redeem) is None
    if wit and not witnessed:
        return False
    return True


def block(tx, spent):
    outs = [v for v, _ in tx.outs]
    check = (tx.ins and tx.outs and len(tx.stripped()) * 4 <= MAX_BLOCK_WEIGHT
             and all(0 <= v <= 21000000 * 10 ** 8 for v in outs) and sum(outs) <= 21000000 * 10 ** 8
             and len({i for i, _, _ in tx.ins}) == len(tx.ins))
    funded = sum(v for v, _ in spent) >= sum(outs)
    scripts = all(verify(spk, sig, wit) for (_, sig, wit), (_, spk) in zip(tx.ins, spent))
    weight, cost = tx.weight(), sigop_cost(tx, spent)
    return bool(check and funded and scripts and weight <= MAX_BLOCK_WEIGHT and cost <= MAX_BLOCK_SIGOPS_COST), weight, cost


# ---- the transcript
OUTPUTS = [
    ('P2PK, compressed key', '210279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798ac'),
    ('P2PK, uncompressed key', '410479be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8ac'),
    ('P2PKH', '76a914751e76e8199196d454941c45d1b3a323f1433bd688ac'),
    ('P2SH', 'a914da1745e9b549bd0bfa1a569971c77eba30cd5a4b87'),
    ('P2WPKH', '0014751e76e8199196d454941c45d1b3a323f1433bd6'),
    ('P2WSH', '00204ae81572f06e1b88fd5ced7a1a000945432e83e1551e6f721ee9c00b8cc33260'),
    ('P2TR', '51200000000000000000000000000000000000000000000000000000000000000001'),
    ('a version 2 program', '5202beef'), ('P2A', '51024e73'), ('OP_RETURN', '6a0568656c6c6f'),
    ('bare 2-of-4 multisig', multisig(2, 4).hex()), ('OP_1', '51'),
]


def transcript():
    out = ['output types, and dust at -dustrelayfee=3000 sat/kvB']
    for name, h in OUTPUTS:
        s = bytes.fromhex(h)
        ok, t = is_standard(s)
        line = '  %s: %d byte%s, %s' % (name, len(s), '' if len(s) == 1 else 's', t)
        if not ok:
            line += ', not standard'
        else:
            d = dust_threshold(s, DUST_RELAY_TX_FEE)
            line += ', never dust' if d == 0 else ', dust below %d sat' % d
        out.append(line)
    out.append('one transaction per rule: the reason Bitcoin Core refuses it, then what a block checks')
    valid = standard = 0
    for k, (name, want) in enumerate(CASES):
        tx, spent, o = case(k)
        reason, message = policy(tx, spent, o)
        assert reason == want, (k, reason)
        out.append('  %s %s' % (sha256d(tx.full())[::-1].hex()[:16], name))
        out.append('    policy: ' + reason + (' (%s)' % message if message else ''))
        ok, weight, cost = block(tx, spent)
        n, m = len(tx.ins), len(tx.outs)
        out.append('    a block: %s, %d input%s, %d output%s, weight %d, sigop cost %d' % (
            'valid' if ok else 'invalid', n, '' if n == 1 else 's', m, '' if m == 1 else 's', weight, cost))
        valid += ok
        standard += reason == 'standard'
    out.append('%d of %d transactions are valid in a block; %d of them is standard' % (valid, len(CASES), standard))
    return out


def main():
    lines = transcript()
    want = FIXTURE.read_text().splitlines()
    if lines != want:
        for a, b in zip(lines, want):
            if a != b:
                print('python: ' + a + '\nfixture: ' + b)
                break
        sys.exit('policy.py: the transcript differs from ' + FIXTURE.name)
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            got = subprocess.run([coax, 'run', *mode, str(EXAMPLE)], capture_output=True, text=True, check=True).stdout
            if got.splitlines() != lines:
                sys.exit('policy.py: the example differs in ' + (mode[0] if mode else 'debug'))
    print('policy.py: %d lines agree' % len(lines))


if __name__ == '__main__':
    main()
