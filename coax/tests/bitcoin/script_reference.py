# Firm reason: 2 (independent oracle: a Python transliteration of Bitcoin Core's interpreter, outside Coax)
"""Bitcoin Core's script interpreter in Python, for Bitcoin chapter 36.

A transliteration of the pinned Bitcoin Core sources in
coax/tests/vectors/upstream/bitcoin-core/ (revision ddea9196):

- script/interpreter.cpp: EvalScript, VerifyScript, VerifyWitnessProgram,
  ExecuteWitnessScript, FindAndDelete, CheckSignatureEncoding,
  CheckPubKeyEncoding, CheckMinimalPush, CastToBool, ConditionStack,
  SignatureHash (legacy and BIP-143), CheckLockTime and CheckSequence;
- script/script.h and script.cpp: CScriptNum, GetScriptOp, IsPushOnly,
  IsWitnessProgram, IsPayToScriptHash, IsPayToAnchor;
- pubkey.cpp: CPubKey's lengths, secp256k1_ec_pubkey_parse (hybrid keys
  included) and the verifier's normalization of s;
- core_io.cpp: ParseScript, the script language of the JSON tests.

It shares no code with the Coax library. The DER rules, the lax parser and
the curve arithmetic come from der_reference.py (chapter 21's reference),
the transaction reader from tx_reference.py (chapter 34's). Errors are
Core's ScriptError numbers, and a flag set Core asserts against is
BAD_FLAGS (101), as in coax/lib/bitcoin/script.coax.

Taproot (Bitcoin chapter 44) adds the key path and tapscript: the
SigVersions TAPROOT and TAPSCRIPT, OP_SUCCESSx, OP_CHECKSIGADD, the
validation-weight budget and the Schnorr checker. The signature message is
Bitcoin Core's TaprootSignatureMsg and the verifier and commitment check
BIP-340's and BIP-341's reference code, all through taproot_reference.py.
"""
import hashlib
from pathlib import Path
import re

import der_reference as D
import tx_reference as T

UPSTREAM = Path(__file__).resolve().parents[1]/'vectors/upstream/bitcoin-core'

# ---- errors (script_error.h order) and flags (interpreter.h order)
ERRORS = ["OK", "UNKNOWN_ERROR", "EVAL_FALSE", "OP_RETURN", "SCRIPTNUM", "SCRIPT_SIZE", "PUSH_SIZE",
          "OP_COUNT", "STACK_SIZE", "SIG_COUNT", "PUBKEY_COUNT", "VERIFY", "EQUALVERIFY",
          "CHECKMULTISIGVERIFY", "CHECKSIGVERIFY", "NUMEQUALVERIFY", "BAD_OPCODE", "DISABLED_OPCODE",
          "INVALID_STACK_OPERATION", "INVALID_ALTSTACK_OPERATION", "UNBALANCED_CONDITIONAL",
          "NEGATIVE_LOCKTIME", "UNSATISFIED_LOCKTIME", "SIG_HASHTYPE", "SIG_DER", "MINIMALDATA",
          "SIG_PUSHONLY", "SIG_HIGH_S", "SIG_NULLDUMMY", "PUBKEYTYPE", "CLEANSTACK", "MINIMALIF",
          "NULLFAIL", "DISCOURAGE_UPGRADABLE_NOPS", "DISCOURAGE_UPGRADABLE_WITNESS_PROGRAM",
          "DISCOURAGE_UPGRADABLE_TAPROOT_VERSION", "DISCOURAGE_OP_SUCCESS",
          "DISCOURAGE_UPGRADABLE_PUBKEYTYPE", "WITNESS_PROGRAM_WRONG_LENGTH",
          "WITNESS_PROGRAM_WITNESS_EMPTY", "WITNESS_PROGRAM_MISMATCH", "WITNESS_MALLEATED",
          "WITNESS_MALLEATED_P2SH", "WITNESS_UNEXPECTED", "WITNESS_PUBKEYTYPE", "SCHNORR_SIG_SIZE",
          "SCHNORR_SIG_HASHTYPE", "SCHNORR_SIG", "TAPROOT_WRONG_CONTROL_SIZE",
          "TAPSCRIPT_VALIDATION_WEIGHT", "TAPSCRIPT_CHECKMULTISIG", "TAPSCRIPT_MINIMALIF",
          "TAPSCRIPT_EMPTY_PUBKEY", "OP_CODESEPARATOR", "SIG_FINDANDDELETE"]
E = {name: k for k, name in enumerate(ERRORS)}
BAD_FLAGS = 101
FLAGS = ["P2SH", "STRICTENC", "DERSIG", "LOW_S", "NULLDUMMY", "SIGPUSHONLY", "MINIMALDATA",
         "DISCOURAGE_UPGRADABLE_NOPS", "CLEANSTACK", "CHECKLOCKTIMEVERIFY", "CHECKSEQUENCEVERIFY",
         "WITNESS", "DISCOURAGE_UPGRADABLE_WITNESS_PROGRAM", "MINIMALIF", "NULLFAIL",
         "WITNESS_PUBKEYTYPE", "CONST_SCRIPTCODE", "TAPROOT", "DISCOURAGE_UPGRADABLE_TAPROOT_VERSION",
         "DISCOURAGE_OP_SUCCESS", "DISCOURAGE_UPGRADABLE_PUBKEYTYPE"]
F = {name: 1 << k for k, name in enumerate(FLAGS)}
ALL_FLAGS = (1 << len(FLAGS)) - 1


def parse_flags(text):
    """ParseScriptFlags: names joined by commas; "" and "NONE" are none."""
    flags = 0
    for word in text.split(','):
        if word in ('', 'NONE'): continue
        flags |= F[word]
    return flags


def fill_flags(flags):
    if flags & F['CLEANSTACK']: flags |= F['WITNESS']
    if flags & F['WITNESS']: flags |= F['P2SH']
    return flags


def trim_flags(flags):
    if not flags & F['P2SH']: flags &= ~F['WITNESS']
    if not flags & F['WITNESS']: flags &= ~F['CLEANSTACK']
    return flags


# ---- opcodes, read from the pinned script.h and script.cpp
def _opcodes():
    text = (UPSTREAM/'script.h').read_text()
    enum = text[text.index('enum opcodetype'):]
    enum = enum[:enum.index('};')]
    values = {}
    for name, value in re.findall(r'(OP_[A-Z0-9_]+) = (0x[0-9a-f]+|OP_[A-Z0-9_]+)', enum):
        values[name] = values[value] if value.startswith('OP_') else int(value, 16)
    names = {}
    source = (UPSTREAM/'script.cpp').read_text()
    for name, shown in re.findall(r'case (OP_[A-Z0-9_]+) *: return "([^"]*)"', source):
        names[values[name]] = shown
    return values, names


OP, OP_NAMES = _opcodes()
MAX_OPCODE = OP['OP_NOP10']


def _parser_names():
    """OpCodeParser: every name from OP_NOP to MAX_OPCODE and OP_RESERVED,
    with and without OP_."""
    m = {}
    for op in range(MAX_OPCODE + 1):
        if op < OP['OP_NOP'] and op != OP['OP_RESERVED']: continue
        name = OP_NAMES.get(op, 'OP_UNKNOWN')
        if name == 'OP_UNKNOWN': continue
        m[name] = op
        if name.startswith('OP_'): m[name[3:]] = op
    return m


PARSER = _parser_names()


def num_serialize(value):
    """CScriptNum::serialize."""
    if value == 0: return b''
    neg, mag, out = value < 0, abs(value), bytearray()
    while mag:
        out.append(mag & 0xff)
        mag >>= 8
    if out[-1] & 0x80: out.append(0x80 if neg else 0)
    elif neg: out[-1] |= 0x80
    return bytes(out)


def push_data(data):
    """CScript << vector: the shortest length prefix, never OP_n."""
    n = len(data)
    if n < 0x4c: return bytes([n]) + data
    if n <= 0xff: return bytes([0x4c, n]) + data
    if n <= 0xffff: return bytes([0x4d]) + n.to_bytes(2, 'little') + data
    return bytes([0x4e]) + n.to_bytes(4, 'little') + data


def push_int(n):
    if n == -1 or 1 <= n <= 16: return bytes([n + 0x50])
    if n == 0: return b'\x00'
    return push_data(num_serialize(n))


def parse_script(text):
    """ParseScript (core_io.cpp)."""
    out = bytearray()
    for w in re.split(r'[ \t\n]', text):
        if not w: continue
        if w.isdigit() or (w[0] == '-' and len(w) > 1 and w[1:].isdigit()):
            n = int(w)
            if not -0xffffffff <= n <= 0xffffffff: raise ValueError('decimal out of range')
            out += push_int(n)
        elif w.startswith('0x') and len(w) > 2 and re.fullmatch(r'[0-9a-fA-F]+', w[2:]) and len(w) % 2 == 0:
            out += bytes.fromhex(w[2:])
        elif len(w) >= 2 and w[0] == "'" and w[-1] == "'":
            out += push_data(w[1:-1].encode())
        else:
            out.append(PARSER[w])
    return bytes(out)


# ---- reading a script
def get_op(s, pc):
    """GetScriptOp: (ok, opcode, data, next); next is where the iterator
    stopped, which on failure is after the length bytes read."""
    if pc >= len(s): return False, 0xff, b'', pc
    op = s[pc]; pc += 1
    if op > 0x4e: return True, op, b'', pc
    if op < 0x4c: width = 0
    else: width = {0x4c: 1, 0x4d: 2, 0x4e: 4}[op]
    if len(s) - pc < width: return False, 0xff, b'', pc
    n = op if width == 0 else int.from_bytes(s[pc:pc + width], 'little')
    pc += width
    if len(s) - pc < n: return False, 0xff, b'', pc
    return True, op, s[pc:pc + n], pc + n


def push_only(s):
    pc = 0
    while pc < len(s):
        ok, op, _, pc = get_op(s, pc)
        if not ok or op > 0x60: return False
    return True


def minimal_push(data, op):
    n = len(data)
    if n == 0: return op == 0
    if n == 1 and 1 <= data[0] <= 16: return False
    if n == 1 and data[0] == 0x81: return False
    if n <= 75: return op == n
    if n <= 255: return op == 0x4c
    if n <= 65535: return op == 0x4d
    return True


class Fail(Exception):
    def __init__(self, code):
        super().__init__(ERRORS[code] if code < len(ERRORS) else str(code))
        self.code = code


def num(b, minimal, size=4):
    """CScriptNum(vch, fRequireMinimal, nMaxNumSize)."""
    if len(b) > size: raise Fail(E['SCRIPTNUM'])
    if minimal and b and (b[-1] & 0x7f) == 0 and (len(b) <= 1 or (b[-2] & 0x80) == 0):
        raise Fail(E['SCRIPTNUM'])
    if not b: return 0
    v = int.from_bytes(b, 'little')
    if b[-1] & 0x80: return -(v & ~(0x80 << (8 * (len(b) - 1))))
    return v


def cast_bool(b):
    for i, x in enumerate(b):
        if x != 0: return not (i == len(b) - 1 and x == 0x80)
    return False


# ---- signature hashes
def code_without_separators(code):
    """SerializeScriptCode, with its quirk: bytes up to where GetOp stopped."""
    pc, separators = 0, 0
    while True:
        ok, op, _, pc = get_op(code, pc)
        if not ok: break
        if op == 0xab: separators += 1
    out, begin, pc = bytearray(T.compact(len(code) - separators)), 0, 0
    while True:
        ok, op, _, pc = get_op(code, pc)
        if not ok: break
        if op == 0xab:
            out += code[begin:pc - 1]
            begin = pc
    if begin != len(code): out += code[begin:pc]
    return bytes(out)


def sighash_legacy(code, tx, n_in, hashtype):
    if (hashtype & 0x1f) == 3 and n_in >= len(tx['outs']):
        return (1).to_bytes(32, 'little')
    anyone, single, none = hashtype & 0x80, (hashtype & 0x1f) == 3, (hashtype & 0x1f) == 2
    s = bytearray(tx['version'])
    inputs = [n_in] if anyone else range(len(tx['ins']))
    s += T.compact(len(inputs))
    for i in inputs:
        s += tx['ins'][i]['prevout']
        s += code_without_separators(code) if i == n_in else b'\x00'
        s += b'\0\0\0\0' if i != n_in and (single or none) else tx['ins'][i]['sequence']
    outputs = 0 if none else (n_in + 1 if single else len(tx['outs']))
    s += T.compact(outputs)
    for k in range(outputs):
        o = tx['outs'][k]
        if single and k != n_in: s += b'\xff' * 8 + b'\x00'
        else: s += o['value'] + T.compact(len(o['script'])) + o['script']
    s += tx['locktime'] + (hashtype & 0xffffffff).to_bytes(4, 'little')
    return T.sha256d(bytes(s))


def sighash_v0(code, tx, n_in, hashtype, amount):
    anyone, kind = hashtype & 0x80, hashtype & 0x1f
    zero = bytes(32)
    prevouts = sequences = outputs = zero
    if not anyone: prevouts = T.sha256d(b''.join(i['prevout'] for i in tx['ins']))
    if not anyone and kind not in (2, 3): sequences = T.sha256d(b''.join(i['sequence'] for i in tx['ins']))
    ser = lambda o: o['value'] + T.compact(len(o['script'])) + o['script']
    if kind not in (2, 3): outputs = T.sha256d(b''.join(ser(o) for o in tx['outs']))
    elif kind == 3 and n_in < len(tx['outs']): outputs = T.sha256d(ser(tx['outs'][n_in]))
    i = tx['ins'][n_in]
    s = (tx['version'] + prevouts + sequences + i['prevout'] + T.compact(len(code)) + code
         + (amount & (2**64 - 1)).to_bytes(8, 'little') + i['sequence'] + outputs + tx['locktime']
         + (hashtype & 0xffffffff).to_bytes(4, 'little'))
    return T.sha256d(s)


# ---- keys and the checker
def parse_pubkey(key):
    """CPubKey's length rule, then secp256k1_ec_pubkey_parse: a point or None."""
    if not key: return None
    want = {2: 33, 3: 33, 4: 65, 6: 65, 7: 65}.get(key[0], 0)
    if want == 0 or len(key) != want: return None
    if want == 33: return D.point(key)
    x, y = int.from_bytes(key[1:33], 'big'), int.from_bytes(key[33:], 'big')
    if x >= D.P or y >= D.P or (y * y - x**3 - 7) % D.P: return None
    if key[0] in (6, 7) and (y & 1) != (key[0] & 1): return None
    return (x, y)


def verify_ecdsa(sig, q, digest):
    """CPubKey::Verify: the lax parse, s normalized, then ECDSA."""
    rs = D.lax(sig)
    if rs is None: return False
    r, s = rs
    if not (0 < r < D.N and 0 < s < D.N): return False
    w = pow(s, -1, D.N)
    z = int.from_bytes(digest, 'big') % D.N
    R = D.add(D.mul(z * w % D.N, D.G), D.mul(r * w % D.N, q))
    return R is not None and R[0] % D.N == r


BASE, V0, TAPROOT, TAPSCRIPT = 0, 1, 2, 3


class Checker:
    """TransactionSignatureChecker for input n_in of tx (a tx_reference dict)."""

    def __init__(self, tx, n_in, amount, spent=None):
        self.tx, self.n_in, self.amount = tx, n_in, amount
        self.spent = spent   # every input's (amount, scriptPubKey), for Taproot
        self.spk = None      # this input's, set by verify_script
        self.curve = 0   # signatures that reached the curve arithmetic
        self.schnorrs = 0  # of them, Schnorr signatures

    def schnorr(self, sig, key, version, ex):
        """CheckSchnorrSignature: raises Fail, returns on success."""
        import taproot_reference as TR
        if len(sig) not in (64, 65): raise Fail(E['SCHNORR_SIG_SIZE'])
        hashtype = 0
        if len(sig) == 65:
            hashtype, sig = sig[64], sig[:64]
            if hashtype == 0: raise Fail(E['SCHNORR_SIG_HASHTYPE'])
        msg = self.sigmsg(hashtype, version, ex)
        if msg is None: raise Fail(E['SCHNORR_SIG_HASHTYPE'])
        self.curve += 1
        self.schnorrs += 1
        if not TR.R.schnorr_verify(TR.R.tagged_hash('TapSighash', msg), key, sig): raise Fail(E['SCHNORR_SIG'])

    def sigmsg(self, hashtype, version, ex):
        """SignatureHashSchnorr's message by Core's TaprootSignatureMsg, or
        None where SignatureHashSchnorr refuses."""
        import taproot_reference as TR
        import types
        spent = self.spent
        if spent is None and len(self.tx['ins']) == 1: spent = [(self.amount, self.spk)]
        if spent is None or len(spent) != len(self.tx['ins']): return None
        if hashtype not in (0, 1, 2, 3, 0x81, 0x82, 0x83): return None
        if hashtype & 3 == 3 and self.n_in >= len(self.tx['outs']): return None
        tx = types.SimpleNamespace(version=int.from_bytes(self.tx['version'], 'little'),
                                   nLockTime=int.from_bytes(self.tx['locktime'], 'little'),
                                   vin=[TR._In(i['prevout'], int.from_bytes(i['sequence'], 'little')) for i in self.tx['ins']],
                                   vout=[TR._Out(o['value'] + T.compact(len(o['script'])) + o['script']) for o in self.tx['outs']])
        utxos = [TR._Utxo(a, k) for a, k in spent]
        if version == TAPROOT:
            return TR.CORE.TaprootSignatureMsg(tx, utxos, hashtype, self.n_in, annex=ex['annex'])
        return TR.CORE.TaprootSignatureMsg(tx, utxos, hashtype, self.n_in, scriptpath=True, leaf_script=ex['script'],
                                           codeseparator_pos=ex['codesep'], annex=ex['annex'])

    def ecdsa(self, sig, key, code, version):
        q = parse_pubkey(key)
        if q is None or not sig: return False
        hashtype = sig[-1]
        if version == V0: digest = sighash_v0(code, self.tx, self.n_in, hashtype, self.amount)
        else: digest = sighash_legacy(code, self.tx, self.n_in, hashtype)
        self.curve += 1
        return verify_ecdsa(sig[:-1], q, digest)

    def locktime(self, n):
        lt = int.from_bytes(self.tx['locktime'], 'little')
        if not ((lt < 500000000 and n < 500000000) or (lt >= 500000000 and n >= 500000000)): return False
        if n > lt: return False
        return self.tx['ins'][self.n_in]['sequence'] != b'\xff\xff\xff\xff'

    def sequence(self, n):
        seq = int.from_bytes(self.tx['ins'][self.n_in]['sequence'], 'little')
        if int.from_bytes(self.tx['version'], 'little') < 2: return False
        if seq & (1 << 31): return False
        mask = (1 << 22) | 0xffff
        a, b = seq & mask, n & mask
        if not ((a < (1 << 22) and b < (1 << 22)) or (a >= (1 << 22) and b >= (1 << 22))): return False
        return b <= a


def find_and_delete(script, b):
    """FindAndDelete: (script, count)."""
    if not b: return script, 0
    found, result, pc, pc2 = 0, bytearray(), 0, 0
    while True:
        result += script[pc2:pc]
        while len(script) - pc >= len(b) and script[pc:pc + len(b)] == b:
            pc += len(b)
            found += 1
        pc2 = pc
        ok, _, _, pc = get_op(script, pc)
        if not ok: break
    if found:
        result += script[pc2:]
        return bytes(result), found
    return script, 0


def sig_encoding(sig, flags):
    if not sig: return
    if flags & (F['DERSIG'] | F['LOW_S'] | F['STRICTENC']) and not D.valid_encoding(sig): raise Fail(E['SIG_DER'])
    if flags & F['LOW_S'] and not D.low_s(sig): raise Fail(E['SIG_HIGH_S'])
    if flags & F['STRICTENC'] and not D.defined_hashtype(sig): raise Fail(E['SIG_HASHTYPE'])


def key_encoding(key, flags, version):
    if flags & F['STRICTENC'] and not D.key_shape(key): raise Fail(E['PUBKEYTYPE'])
    if flags & F['WITNESS_PUBKEYTYPE'] and version == V0 and not (len(key) == 33 and key[0] in (2, 3)):
        raise Fail(E['WITNESS_PUBKEYTYPE'])


DISABLED = {0x7e, 0x7f, 0x80, 0x81, 0x83, 0x84, 0x85, 0x86, 0x8d, 0x8e, 0x95, 0x96, 0x97, 0x98, 0x99}


def is_op_success(op):
    """IsOpSuccess (script.cpp)."""
    return op == 80 or op == 98 or 126 <= op <= 129 or 131 <= op <= 134 or 137 <= op <= 138 or \
        141 <= op <= 142 or 149 <= op <= 153 or 187 <= op <= 254


def checksig_tapscript(sig, key, flags, checker, ex):
    """EvalChecksigTapscript: the check's result, or Fail."""
    success = len(sig) > 0
    if success:
        ex['weight'] -= 50
        if ex['weight'] < 0: raise Fail(E['TAPSCRIPT_VALIDATION_WEIGHT'])
    if len(key) == 0: raise Fail(E['TAPSCRIPT_EMPTY_PUBKEY'])
    if len(key) == 32:
        if success: checker.schnorr(sig, key, TAPSCRIPT, ex)
    elif flags & F['DISCOURAGE_UPGRADABLE_PUBKEYTYPE']:
        raise Fail(E['DISCOURAGE_UPGRADABLE_PUBKEYTYPE'])
    return success


def eval_script(stack, script, flags, checker, version, ex=None):
    """EvalScript: changes stack in place; raises Fail."""
    if version != TAPSCRIPT and len(script) > 10000: raise Fail(E['SCRIPT_SIZE'])
    minimal = bool(flags & F['MINIMALDATA'])
    vexec, alt, ops, pc, begin = [], [], 0, 0, 0
    pos = -1
    if ex is not None: ex['codesep'] = 0xffffffff

    def top(i):
        if len(stack) < i: raise Fail(E['INVALID_STACK_OPERATION'])
        return stack[-i]

    def need(k):
        if len(stack) < k: raise Fail(E['INVALID_STACK_OPERATION'])

    while pc < len(script):
        fexec = False not in vexec
        pos += 1
        ok, op, data, pc = get_op(script, pc)
        if not ok: raise Fail(E['BAD_OPCODE'])
        if len(data) > 520: raise Fail(E['PUSH_SIZE'])
        if op > 0x60 and version != TAPSCRIPT:
            ops += 1
            if ops > 201: raise Fail(E['OP_COUNT'])
        if op in DISABLED: raise Fail(E['DISABLED_OPCODE'])
        if op == 0xab and version == BASE and flags & F['CONST_SCRIPTCODE']: raise Fail(E['OP_CODESEPARATOR'])
        if fexec and op <= 0x4e:
            if minimal and not minimal_push(data, op): raise Fail(E['MINIMALDATA'])
            stack.append(data)
        elif fexec or 0x63 <= op <= 0x68:
            if op == 0x4f or 0x51 <= op <= 0x60:
                stack.append(num_serialize(op - 0x50))
            elif op == 0x61:
                pass
            elif op == 0xb1 or op == 0xb2:
                flag = F['CHECKLOCKTIMEVERIFY'] if op == 0xb1 else F['CHECKSEQUENCEVERIFY']
                if flags & flag:
                    n = num(top(1), minimal, 5)
                    if n < 0: raise Fail(E['NEGATIVE_LOCKTIME'])
                    if op == 0xb1:
                        if not checker.locktime(n): raise Fail(E['UNSATISFIED_LOCKTIME'])
                    elif not n & (1 << 31) and not checker.sequence(n):
                        raise Fail(E['UNSATISFIED_LOCKTIME'])
            elif op == 0xb0 or 0xb3 <= op <= 0xb9:
                if flags & F['DISCOURAGE_UPGRADABLE_NOPS']: raise Fail(E['DISCOURAGE_UPGRADABLE_NOPS'])
            elif op in (0x63, 0x64):
                value = False
                if fexec:
                    v = top(1)
                    if version == TAPSCRIPT and (len(v) > 1 or (len(v) == 1 and v[0] != 1)):
                        raise Fail(E['TAPSCRIPT_MINIMALIF'])
                    if version == V0 and flags & F['MINIMALIF']:
                        if len(v) > 1 or (len(v) == 1 and v[0] != 1): raise Fail(E['MINIMALIF'])
                    value = cast_bool(v) != (op == 0x64)
                    stack.pop()
                vexec.append(value)
            elif op == 0x67:
                if not vexec: raise Fail(E['UNBALANCED_CONDITIONAL'])
                vexec[-1] = not vexec[-1]
            elif op == 0x68:
                if not vexec: raise Fail(E['UNBALANCED_CONDITIONAL'])
                vexec.pop()
            elif op == 0x69:
                if not cast_bool(top(1)): raise Fail(E['VERIFY'])
                stack.pop()
            elif op == 0x6a:
                raise Fail(E['OP_RETURN'])
            elif op == 0x6b:
                alt.append(top(1)); stack.pop()
            elif op == 0x6c:
                if not alt: raise Fail(E['INVALID_ALTSTACK_OPERATION'])
                stack.append(alt.pop())
            elif op == 0x6d:
                need(2); del stack[-2:]
            elif op == 0x6e:
                need(2); stack += stack[-2:]
            elif op == 0x6f:
                need(3); stack += stack[-3:]
            elif op == 0x70:
                need(4); stack += stack[-4:-2]
            elif op == 0x71:
                need(6); a, b = stack[-6], stack[-5]; del stack[-6:-4]; stack += [a, b]
            elif op == 0x72:
                need(4); stack[-4], stack[-3], stack[-2], stack[-1] = stack[-2], stack[-1], stack[-4], stack[-3]
            elif op == 0x73:
                if cast_bool(top(1)): stack.append(stack[-1])
            elif op == 0x74:
                stack.append(num_serialize(len(stack)))
            elif op == 0x75:
                need(1); stack.pop()
            elif op == 0x76:
                stack.append(top(1))
            elif op == 0x77:
                need(2); del stack[-2]
            elif op == 0x78:
                stack.append(top(2))
            elif op in (0x79, 0x7a):
                need(2)
                n = num(stack[-1], minimal)
                stack.pop()
                if n < 0 or n >= len(stack): raise Fail(E['INVALID_STACK_OPERATION'])
                v = stack[-n - 1]
                if op == 0x7a: del stack[-n - 1]
                stack.append(v)
            elif op == 0x7b:
                need(3); stack[-3], stack[-2], stack[-1] = stack[-2], stack[-1], stack[-3]
            elif op == 0x7c:
                need(2); stack[-2], stack[-1] = stack[-1], stack[-2]
            elif op == 0x7d:
                need(2); stack.insert(len(stack) - 2, stack[-1])
            elif op == 0x82:
                stack.append(num_serialize(len(top(1))))
            elif op in (0x87, 0x88):
                need(2)
                equal = stack[-2] == stack[-1]
                del stack[-2:]
                stack.append(b'\x01' if equal else b'')
                if op == 0x88:
                    if not equal: raise Fail(E['EQUALVERIFY'])
                    stack.pop()
            elif op in (0x8b, 0x8c, 0x8f, 0x90, 0x91, 0x92):
                a = num(top(1), minimal)
                r = {0x8b: a + 1, 0x8c: a - 1, 0x8f: -a, 0x90: abs(a), 0x91: int(a == 0), 0x92: int(a != 0)}[op]
                stack.pop(); stack.append(num_serialize(r))
            elif 0x93 <= op <= 0xa4:
                need(2)
                a, b = num(stack[-2], minimal), num(stack[-1], minimal)
                r = {0x93: a + b, 0x94: a - b, 0x9a: int(a != 0 and b != 0), 0x9b: int(a != 0 or b != 0),
                     0x9c: int(a == b), 0x9d: int(a == b), 0x9e: int(a != b), 0x9f: int(a < b), 0xa0: int(a > b),
                     0xa1: int(a <= b), 0xa2: int(a >= b), 0xa3: min(a, b), 0xa4: max(a, b)}[op]
                del stack[-2:]
                stack.append(num_serialize(r))
                if op == 0x9d:
                    if not cast_bool(stack[-1]): raise Fail(E['NUMEQUALVERIFY'])
                    stack.pop()
            elif op == 0xa5:
                need(3)
                x, lo, hi = num(stack[-3], minimal), num(stack[-2], minimal), num(stack[-1], minimal)
                del stack[-3:]
                stack.append(b'\x01' if lo <= x < hi else b'')
            elif 0xa6 <= op <= 0xaa:
                v = top(1)
                if op == 0xa6: h = ripemd160(v)
                elif op == 0xa7: h = hashlib.sha1(v).digest()
                elif op == 0xa8: h = hashlib.sha256(v).digest()
                elif op == 0xa9: h = ripemd160(hashlib.sha256(v).digest())
                else: h = T.sha256d(v)
                stack.pop(); stack.append(h)
            elif op == 0xab:
                begin = pc
                if ex is not None: ex['codesep'] = pos
            elif op in (0xac, 0xad) and version == TAPSCRIPT:
                need(2)
                success = checksig_tapscript(stack[-2], stack[-1], flags, checker, ex)
                del stack[-2:]
                stack.append(b'\x01' if success else b'')
                if op == 0xad:
                    if not success: raise Fail(E['CHECKSIGVERIFY'])
                    stack.pop()
            elif op == 0xba and version == TAPSCRIPT:
                need(3)
                n = num(stack[-2], minimal)
                success = checksig_tapscript(stack[-3], stack[-1], flags, checker, ex)
                del stack[-3:]
                stack.append(num_serialize(n + (1 if success else 0)))
            elif op in (0xae, 0xaf) and version == TAPSCRIPT:
                raise Fail(E['TAPSCRIPT_CHECKMULTISIG'])
            elif op in (0xac, 0xad):
                need(2)
                sig, key = stack[-2], stack[-1]
                code = script[begin:]
                if version == BASE:
                    code, found = find_and_delete(code, push_data(sig))
                    if found and flags & F['CONST_SCRIPTCODE']: raise Fail(E['SIG_FINDANDDELETE'])
                sig_encoding(sig, flags)
                key_encoding(key, flags, version)
                success = checker.ecdsa(sig, key, code, version)
                if not success and flags & F['NULLFAIL'] and sig: raise Fail(E['NULLFAIL'])
                del stack[-2:]
                stack.append(b'\x01' if success else b'')
                if op == 0xad:
                    if not success: raise Fail(E['CHECKSIGVERIFY'])
                    stack.pop()
            elif op in (0xae, 0xaf):
                i = 1
                need(i)
                nkeys = num(stack[-i], minimal)
                if nkeys < 0 or nkeys > 20: raise Fail(E['PUBKEY_COUNT'])
                ops += nkeys
                if ops > 201: raise Fail(E['OP_COUNT'])
                i += 1; ikey = i; ikey2 = nkeys + 2
                i += nkeys
                need(i)
                nsigs = num(stack[-i], minimal)
                if nsigs < 0 or nsigs > nkeys: raise Fail(E['SIG_COUNT'])
                i += 1; isig = i
                i += nsigs
                need(i)
                code = script[begin:]
                for k in range(nsigs):
                    if version == BASE:
                        code, found = find_and_delete(code, push_data(stack[-isig - k]))
                        if found and flags & F['CONST_SCRIPTCODE']: raise Fail(E['SIG_FINDANDDELETE'])
                success = True
                while success and nsigs > 0:
                    sig, key = stack[-isig], stack[-ikey]
                    sig_encoding(sig, flags)
                    key_encoding(key, flags, version)
                    if checker.ecdsa(sig, key, code, version):
                        isig += 1; nsigs -= 1
                    ikey += 1; nkeys -= 1
                    if nsigs > nkeys: success = False
                while i > 1:
                    i -= 1
                    if not success and flags & F['NULLFAIL'] and not ikey2 and stack[-1]: raise Fail(E['NULLFAIL'])
                    if ikey2 > 0: ikey2 -= 1
                    stack.pop()
                need(1)
                if flags & F['NULLDUMMY'] and stack[-1]: raise Fail(E['SIG_NULLDUMMY'])
                stack.pop()
                stack.append(b'\x01' if success else b'')
                if op == 0xaf:
                    if not success: raise Fail(E['CHECKMULTISIGVERIFY'])
                    stack.pop()
            else:
                raise Fail(E['BAD_OPCODE'])
        if len(stack) + len(alt) > 1000: raise Fail(E['STACK_SIZE'])
    if vexec: raise Fail(E['UNBALANCED_CONDITIONAL'])


def witness_program(s):
    """IsWitnessProgram: (version, program) or None."""
    if len(s) < 4 or len(s) > 42: return None
    if s[0] != 0 and not 0x51 <= s[0] <= 0x60: return None
    if s[1] + 2 != len(s): return None
    return (0 if s[0] == 0 else s[0] - 0x50), s[2:]


def execute_witness(stack, script, flags, checker, version=V0, ex=None):
    if version == TAPSCRIPT:
        pc = 0
        while pc < len(script):
            ok, op, _, pc = get_op(script, pc)
            if not ok: raise Fail(E['BAD_OPCODE'])
            if is_op_success(op):
                if flags & F['DISCOURAGE_OP_SUCCESS']: raise Fail(E['DISCOURAGE_OP_SUCCESS'])
                return
        if len(stack) > 1000: raise Fail(E['STACK_SIZE'])
    for el in stack:
        if len(el) > 520: raise Fail(E['PUSH_SIZE'])
    eval_script(stack, script, flags, checker, version, ex)
    if len(stack) != 1: raise Fail(E['CLEANSTACK'])
    if not cast_bool(stack[-1]): raise Fail(E['EVAL_FALSE'])


def verify_witness_program(witness, version, program, flags, checker, p2sh):
    stack = list(witness)
    if version == 0:
        if len(program) == 32:
            if not stack: raise Fail(E['WITNESS_PROGRAM_WITNESS_EMPTY'])
            script = stack.pop()
            if hashlib.sha256(script).digest() != program: raise Fail(E['WITNESS_PROGRAM_MISMATCH'])
            return execute_witness(stack, script, flags, checker)
        if len(program) == 20:
            if len(stack) != 2: raise Fail(E['WITNESS_PROGRAM_MISMATCH'])
            script = b'\x76\xa9\x14' + program + b'\x88\xac'
            return execute_witness(stack, script, flags, checker)
        raise Fail(E['WITNESS_PROGRAM_WRONG_LENGTH'])
    if version == 1 and len(program) == 32 and not p2sh:
        if not flags & F['TAPROOT']: return
        return verify_taproot(witness, program, flags, checker)
    if not p2sh and version == 1 and program == b'\x4e\x73': return
    if flags & F['DISCOURAGE_UPGRADABLE_WITNESS_PROGRAM']: raise Fail(E['DISCOURAGE_UPGRADABLE_WITNESS_PROGRAM'])


def witness_size(witness):
    """GetSerializeSize of a witness stack."""
    return len(T.compact(len(witness))) + sum(len(T.compact(len(w))) + len(w) for w in witness)


def verify_taproot(witness, program, flags, checker):
    """VerifyWitnessProgram's BIP-341 branch."""
    import taproot_reference as TR
    stack = list(witness)
    if not stack: raise Fail(E['WITNESS_PROGRAM_WITNESS_EMPTY'])
    ex = dict(annex=None)
    if len(stack) >= 2 and stack[-1] and stack[-1][0] == 0x50:
        ex['annex'] = stack.pop()
    if len(stack) == 1:
        checker.schnorr(stack[0], program, TAPROOT, ex)
        return
    control, script = stack.pop(), stack.pop()
    rule = TR.verify_commitment(control, script, program)
    if rule == 'size': raise Fail(E['TAPROOT_WRONG_CONTROL_SIZE'])
    if rule is not None: raise Fail(E['WITNESS_PROGRAM_MISMATCH'])
    if control[0] & 0xfe == 0xc0:
        ex['script'] = script
        ex['weight'] = witness_size(witness) + 50
        return execute_witness(stack, script, flags, checker, TAPSCRIPT, ex)
    if flags & F['DISCOURAGE_UPGRADABLE_TAPROOT_VERSION']: raise Fail(E['DISCOURAGE_UPGRADABLE_TAPROOT_VERSION'])


def is_p2sh(s):
    return len(s) == 23 and s[0] == 0xa9 and s[1] == 0x14 and s[22] == 0x87


def verify_script(script_sig, spk, witness, flags, checker):
    """VerifyScript: the error number, 0 for success."""
    if flags & F['WITNESS'] and not flags & F['P2SH']: return BAD_FLAGS
    if flags & F['CLEANSTACK'] and not flags & F['WITNESS']: return BAD_FLAGS
    if checker.spk is None: checker.spk = spk
    try:
        had_witness = False
        if flags & F['SIGPUSHONLY'] and not push_only(script_sig): raise Fail(E['SIG_PUSHONLY'])
        stack = []
        eval_script(stack, script_sig, flags, checker, BASE)
        copy = list(stack)
        eval_script(stack, spk, flags, checker, BASE)
        if not stack or not cast_bool(stack[-1]): raise Fail(E['EVAL_FALSE'])
        if flags & F['WITNESS']:
            wp = witness_program(spk)
            if wp:
                had_witness = True
                if script_sig: raise Fail(E['WITNESS_MALLEATED'])
                verify_witness_program(witness, wp[0], wp[1], flags, checker, False)
                stack = stack[:1]
        if flags & F['P2SH'] and is_p2sh(spk):
            if not push_only(script_sig): raise Fail(E['SIG_PUSHONLY'])
            stack = copy
            redeem = stack.pop()
            eval_script(stack, redeem, flags, checker, BASE)
            if not stack or not cast_bool(stack[-1]): raise Fail(E['EVAL_FALSE'])
            if flags & F['WITNESS']:
                wp = witness_program(redeem)
                if wp:
                    had_witness = True
                    if script_sig != push_data(redeem): raise Fail(E['WITNESS_MALLEATED_P2SH'])
                    verify_witness_program(witness, wp[0], wp[1], flags, checker, True)
                    stack = stack[:1]
        if flags & F['CLEANSTACK'] and len(stack) != 1: raise Fail(E['CLEANSTACK'])
        if flags & F['WITNESS'] and not had_witness and witness: raise Fail(E['WITNESS_UNEXPECTED'])
        return 0
    except Fail as f:
        return f.code


# ---- RIPEMD-160 (hashlib's when OpenSSL has it, else this one)
def _rmd(data):
    rl = [0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,7,4,13,1,10,6,15,3,12,0,9,5,2,14,11,8,3,10,14,4,9,15,8,1,2,7,0,6,13,11,5,12,
          1,9,11,10,0,8,12,4,13,3,7,15,14,5,6,2,4,0,5,9,7,12,2,10,14,1,3,8,11,6,15,13]
    rr = [5,14,7,0,9,2,11,4,13,6,15,8,1,10,3,12,6,11,3,7,0,13,5,10,14,15,8,12,4,9,1,2,15,5,1,3,7,14,6,9,11,8,12,2,10,0,4,13,
          8,6,4,1,3,11,15,0,5,12,2,13,9,7,10,14,12,15,10,4,1,5,8,7,6,2,13,14,0,3,9,11]
    sl = [11,14,15,12,5,8,7,9,11,13,14,15,6,7,9,8,7,6,8,13,11,9,7,15,7,12,15,9,11,7,13,12,11,13,6,7,14,9,13,15,14,8,13,6,5,12,7,5,
          11,12,14,15,14,15,9,8,9,14,5,6,8,6,5,12,9,15,5,11,6,8,13,12,5,12,13,14,11,8,5,6]
    sr = [8,9,9,11,13,15,15,5,7,7,8,11,14,14,12,6,9,13,15,7,12,8,9,11,7,7,12,7,6,15,13,11,9,7,15,11,8,6,6,14,12,13,5,14,13,13,7,5,
          15,5,8,11,14,14,6,14,6,9,12,9,12,5,15,8,8,5,12,9,12,5,14,6,8,13,6,5,15,13,11,11]
    kl = [0, 0x5a827999, 0x6ed9eba1, 0x8f1bbcdc, 0xa953fd4e]
    kr = [0x50a28be6, 0x5c4dd124, 0x6d703ef3, 0x7a6d76e9, 0]
    f = [lambda x, y, z: x ^ y ^ z, lambda x, y, z: (x & y) | (~x & z), lambda x, y, z: (x | ~y) ^ z,
         lambda x, y, z: (x & z) | (y & ~z), lambda x, y, z: x ^ (y | ~z)]
    rol = lambda x, n: ((x << n) | (x >> (32 - n))) & 0xffffffff
    h = [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476, 0xc3d2e1f0]
    msg = data + b'\x80' + b'\0' * ((55 - len(data)) % 64) + (8 * len(data)).to_bytes(8, 'little')
    for at in range(0, len(msg), 64):
        x = [int.from_bytes(msg[at + 4 * i:at + 4 * i + 4], 'little') for i in range(16)]
        al, bl, cl, dl, el = h; ar, br, cr, dr, er = h
        for j in range(80):
            r = j // 16
            t = (rol((al + (f[r](bl, cl, dl) & 0xffffffff) + x[rl[j]] + kl[r]) & 0xffffffff, sl[j]) + el) & 0xffffffff
            al, el, dl, cl, bl = el, dl, rol(cl, 10), bl, t
            t = (rol((ar + (f[4 - r](br, cr, dr) & 0xffffffff) + x[rr[j]] + kr[r]) & 0xffffffff, sr[j]) + er) & 0xffffffff
            ar, er, dr, cr, br = er, dr, rol(cr, 10), br, t
        t = (h[1] + cl + dr) & 0xffffffff
        h[1] = (h[2] + dl + er) & 0xffffffff
        h[2] = (h[3] + el + ar) & 0xffffffff
        h[3] = (h[4] + al + br) & 0xffffffff
        h[4] = (h[0] + bl + cr) & 0xffffffff
        h[0] = t
    return b''.join(v.to_bytes(4, 'little') for v in h)


def ripemd160(data):
    try:
        return hashlib.new('ripemd160', data).digest()
    except ValueError:
        return _rmd(data)


assert _rmd(b'') == bytes.fromhex('9c1185a5c5e9fc54612808977ee8f548b2258d31')
assert _rmd(b'abc') == bytes.fromhex('8eb208f7e05d987a9b044a8e98c6b087f15a0bfc')


# ---- the JSON tests' transactions
def crediting(spk, value):
    """BuildCreditingTransaction."""
    return dict(version=(1).to_bytes(4, 'little'),
                ins=[dict(prevout=bytes(32) + b'\xff\xff\xff\xff', script=b'\x00\x00', sequence=b'\xff\xff\xff\xff', witness=[])],
                outs=[dict(value=value.to_bytes(8, 'little', signed=True), script=spk)],
                locktime=bytes(4), segwit=False)


def spending(script_sig, witness, credit):
    """BuildSpendingTransaction."""
    return dict(version=(1).to_bytes(4, 'little'),
                ins=[dict(prevout=T.txid(credit) + bytes(4), script=script_sig, sequence=b'\xff\xff\xff\xff', witness=list(witness))],
                outs=[dict(value=credit['outs'][0]['value'], script=b'')],
                locktime=bytes(4), segwit=bool(witness))


def amount_from_value(v):
    """AmountFromValue on a JSON number of bitcoin: satoshis."""
    from decimal import Decimal
    return int((Decimal(str(v)) * 100000000).to_integral_exact())


def script_rows():
    """Every test row of script_tests.json: dicts with the spending
    transaction's bytes, the scriptPubKey, the amount, the flags as written
    and as run, the expected error, the comment, and taproot (True for the
    rows whose leaf script, control block and output the test builds, as
    script_tests.cpp does: one tapscript leaf at depth 0 under the internal
    key of secret 1, G's x)."""
    import json
    import taproot_reference as TR
    rows = []
    gx = TR.R.bytes_from_int(TR.R.x(TR.R.G))
    for k, test in enumerate(json.loads((UPSTREAM/'script_tests.json').read_text())):
        if len(test) < 4: continue
        pos, witness, value, taproot, output = 0, [], 0, False, None
        if isinstance(test[0], list):
            for el in test[0][:-1]:
                if el.startswith('#SCRIPT#'):
                    taproot = True
                    witness.append(parse_script(el[len('#SCRIPT#'):]))
                elif el == '#CONTROLBLOCK#':
                    leaf = TR.R.tagged_hash('TapLeaf', bytes([0xc0]) + TR.ser_script(witness[-1]))
                    parity, output = TR.R.taproot_tweak_pubkey(gx, leaf)
                    witness.append(bytes([0xc0 | parity]) + gx)
                else:
                    witness.append(bytes.fromhex(el))
            value = amount_from_value(test[0][-1])
            pos = 1
        if len(test) < 4 + pos: continue
        sig_text, spk_text = test[pos], test[pos + 1]
        flags_text, expected = test[pos + 2], test[pos + 3]
        comment = test[pos + 4] if len(test) > pos + 4 else ''
        if spk_text == '0x51 0x20 #TAPROOTOUTPUT#':
            spk = b'\x51\x20' + output
        else:
            spk = parse_script(spk_text)
        sig = parse_script(sig_text)
        flags = parse_flags(flags_text)
        run_flags = fill_flags(flags) if flags & F['CLEANSTACK'] else flags
        credit = crediting(spk, value)
        spend = spending(sig, witness, credit)
        rows.append(dict(line=k, sig=sig, spk=spk, witness=witness, amount=value, flags_text=flags_text,
                         flags=run_flags, expected=expected, comment=comment, taproot=taproot, runnable=True,
                         tx=T.serialize(spend, True), sig_text=sig_text, spk_text=spk_text))
    return rows


def run_row(row, flags=None, checker=None):
    tx = T.unserialize(row['tx'], True)
    checker = checker or Checker(tx, 0, row['amount'])
    return verify_script(row['sig'], row['spk'], row['witness'], row['flags'] if flags is None else flags, checker)


def tx_rows(name):
    """The rows of tx_valid.json or tx_invalid.json: (prevouts, tx bytes,
    flags text), prevouts a dict from (txid bytes in hash order, index) to
    (scriptPubKey, amount)."""
    import json
    out = []
    for test in json.loads((UPSTREAM/name).read_text()):
        if not isinstance(test[0], list): continue
        prevouts = {}
        for p in test[0]:
            key = (bytes.fromhex(p[0])[::-1], p[1] & 0xffffffff)
            prevouts[key] = (parse_script(p[2]), p[3] if len(p) >= 4 else 0)
        out.append(dict(prevouts=prevouts, tx=bytes.fromhex(test[1]), flags_text=test[2]))
    return out


def check_tx(tx, prevouts, flags):
    """CheckTxScripts' verdict: (valid, error of the input that failed)."""
    for n, i in enumerate(tx['ins']):
        key = (i['prevout'][:32], int.from_bytes(i['prevout'][32:], 'little'))
        spk, amount = prevouts[key]
        error = verify_script(i['script'], spk, i['witness'], flags, Checker(tx, n, amount))
        if error: return False, error
    return True, 0
