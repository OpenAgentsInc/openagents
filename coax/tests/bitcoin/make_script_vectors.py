#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: copies published vectors and decides expected results outside Coax)
"""Writes coax/tests/bitcoin/script_vectors.coax and script_lib_vectors.coax
for Bitcoin chapter 36 from files pinned in coax/tests/vectors/upstream/:

  python3 -I coax/tests/bitcoin/make_script_vectors.py

script_vectors.coax (the example and its checks):

- every row of Bitcoin Core's script_tests.json: the spending transaction
  that script_tests.cpp builds for it (BuildCreditingTransaction and
  BuildSpendingTransaction), the scriptPubKey, the amount, the flags it
  runs under and the error it expects, all copied or rebuilt as Core's
  test does. The 7 Taproot rows get the leaf script, control block and
  output key script_tests.cpp builds for them (chapter 44), and their
  lines are listed apart;
- every row of tx_valid.json and tx_invalid.json except the BADTX ones
  (CheckTransaction, chapter 52): the transaction, each input's spent
  scriptPubKey and amount, and the flag sets transaction_tests.cpp runs it
  under, with the verdict each must give: the row's own flags, each flag
  removed (valid rows) or added (invalid rows), and each flag the other way
  (the maximality and minimality checks). Core also tries random flag
  sets; script.py does, against the Python reference;
- every row of sighash.json: the transaction, the script, the input, the
  hash type and the published legacy signature hash.

script_lib_vectors.coax (the library's own checks): hand-picked cases, each
a script, flags and a transaction built the same way, whose expected error
is decided by script_reference.py (a transliteration of Core's
interpreter.cpp that shares no code with Coax), and two spends signed here
with test keys derived from fixed labels.

Every input's SHA-256 must equal its pin below.
"""
import hashlib
import json
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import der_reference as D  # noqa: E402
import script_reference as R  # noqa: E402
import tx_reference as T  # noqa: E402

UPSTREAM = HERE.parent/'vectors/upstream/bitcoin-core'
PINS = {
    'script_tests.json': 'ccfadc13f9b06888faf57a4a2f86dd8feff35c80aa710cbf3eb421f326b8d8b7',
    'tx_valid.json': '0abeb1a6e4b942e4e43ef8d49b7356c1f6204bcb14163f24153f4d0ec5418011',
    'tx_invalid.json': '0c02ce44ff3a880458f9569a25589315a07f924fcaadac828613d4615776ca52',
    'sighash.json': '52cf23c2076e7f129c71d5508631d3e5ae3be1b1cb0585c0e23bbb4bb373e924',
    'script.h': '43e4eb10ee85ac31996e257d0c5dc45cbe3b29b2d61d1a72d101453ba64ded01',
    'script.cpp': '210b1720acce295de54c6a08c76b97fa4cfe835f64e0ff7e0730c57ebebb8c55',
}
for name, pin in PINS.items():
    assert hashlib.sha256((UPSTREAM/name).read_bytes()).hexdigest() == pin, name


def text(s):
    """A Coax string literal's contents: no quotes or backslashes."""
    return s.replace('\\', '/').replace('"', "'")


# ---- script_tests.json
rows = R.script_rows()
assert len(rows) == 1263
run = rows
taproot_lines = [r['line'] for r in rows if r['taproot']]
assert len(taproot_lines) == 7 and all(r['flags'] & R.F['TAPROOT'] for r in rows if r['taproot'])
for r in run:
    checker = R.Checker(T.unserialize(r['tx'], True), 0, r['amount'])
    got = R.run_row(r, checker=checker)
    r['curve'] = checker.curve
    assert R.ERRORS[got] == r['expected'], (r['line'], got, r['expected'])

# ---- tx_valid.json and tx_invalid.json
txs, prevs, cases = [], [], []
for name, valid in (('tx_valid.json', True), ('tx_invalid.json', False)):
    for row in R.tx_rows(name):
        if row['flags_text'] == 'BADTX': continue
        tx = T.unserialize(row['tx'], True)
        listed = R.parse_flags(row['flags_text'])
        first = len(prevs)
        for i in tx['ins']:
            spk, amount = row['prevouts'][(i['prevout'][:32], int.from_bytes(i['prevout'][32:], 'little'))]
            prevs.append((spk, amount))
        k = len(txs)
        txs.append(dict(tx=row['tx'], valid=valid, first=first, count=len(tx['ins'])))
        sets = []
        if valid:
            sets.append((R.ALL_FLAGS & ~listed, True))
            for f in R.F.values():
                sets.append((R.trim_flags(R.ALL_FLAGS & ~(listed | f)), True))
            for f in R.F.values():
                one = R.trim_flags(listed & ~f)
                if one != listed: sets.append((R.ALL_FLAGS & ~one, False))
        else:
            sets.append((listed, False))
            for f in R.F.values():
                sets.append((R.fill_flags(listed | f), False))
            for f in R.F.values():
                one = R.trim_flags(listed & ~f)
                if one != listed: sets.append((one, True))
        seen = set()
        for flags, verdict in sets:
            if flags in seen: continue
            seen.add(flags)
            got, _ = R.check_tx(tx, row['prevouts'], flags)
            assert got == verdict, (name, row['tx'].hex()[:40], flags)
            cases.append((k, flags, verdict))
assert len(txs) == 206

# ---- sighash.json
sighash = []
for row in json.loads((UPSTREAM/'sighash.json').read_text())[1:]:
    tx = T.unserialize(bytes.fromhex(row[0]), True)
    h = R.sighash_legacy(bytes.fromhex(row[1]), tx, row[2], row[3])
    assert h[::-1].hex() == row[4]
    sighash.append((row[0], row[1], row[2], row[3], row[4]))
assert len(sighash) == 500


# ---- the library's cases
def spend(sig, witness, spk, amount=0, version=1, locktime=0, sequence=0xffffffff):
    """Core's test transactions, with the version, locktime and sequence the
    lock-time checks need."""
    credit = R.crediting(spk, amount)
    s = R.spending(sig, witness, credit)
    s['version'] = version.to_bytes(4, 'little')
    s['locktime'] = locktime.to_bytes(4, 'little')
    s['ins'][0]['sequence'] = sequence.to_bytes(4, 'little')
    return s


def wsh(script):
    return b'\x00\x20' + hashlib.sha256(script).digest()


def sh(script):
    return b'\xa9\x14' + R.ripemd160(hashlib.sha256(script).digest()) + b'\x87'


P = R.parse_script
KEY05 = '0x21 0x05' + '11' * 32           # a key no parser takes, rejected before any curve work
UNCOMPRESSED = '0x41 0x04' + '22' * 64    # the right shape for STRICTENC, not a point
SIG11 = '0x09 0x300602010102010101'       # DER r = 1, s = 1, SIGHASH_ALL
HIGH_S = '0x29 0x3026020101022100' + format(D.N - 1, '064x') + '01'
lib = []
schnorr = []   # the Taproot spends that reach the curve (a signature or the commitment), run apart for their cost


def case(label, sig, spk, flags='', witness=(), amount=0, **tx):
    sig_b = P(sig) if isinstance(sig, str) else sig
    spk_b = P(spk) if isinstance(spk, str) else spk
    s = spend(sig_b, list(witness), spk_b, amount, **tx)
    flags_n = R.parse_flags(flags)
    checker = R.Checker(s, 0, amount)
    error = R.verify_script(sig_b, spk_b, list(witness), flags_n, checker)
    (schnorr if checker.schnorrs or (flags_n & R.F['TAPROOT'] and spk_b[:2] == b'\x51\x20' and witness) else lib).append(dict(label=label, tx=T.serialize(s, True), spk=spk_b, amount=amount, flags=flags_n, error=error))


# Values and branches.
case('true', '1', '')
case('false', '', '0')
case('nothing left', '', '')
case('ADD', '1 2', 'ADD 3 EQUAL')
case('IF taken', '1', 'IF 1 ELSE 0 ENDIF')
case('ELSE taken', '0', 'IF 0 ELSE 1 ENDIF')
case('NOTIF', '0', 'NOTIF 1 ENDIF')
case('nested, outer false', '0 1', 'IF IF 0 ENDIF ELSE 1 ENDIF')
case('nested, inner false', '1 0', 'IF IF 0 ELSE 1 ENDIF ENDIF')
case('ELSE ELSE flips twice', '1', 'IF 1 ELSE 0 ELSE 1 ENDIF')
case('ELSE without IF', '1', 'ELSE')
case('ENDIF without IF', '1', 'ENDIF')
case('IF without ENDIF', '1', 'IF 1')
case('IF on an empty stack', '', 'IF 1 ENDIF')
case('IF of negative zero', '0x01 0x80', 'IF 0 ELSE 1 ENDIF')
case('VERIF unexecuted', '0', 'IF VERIF ENDIF 1')
case('VERNOTIF unexecuted', '0', 'IF VERNOTIF ENDIF 1')
case('RESERVED unexecuted', '0', 'IF RESERVED ENDIF 1')
case('RESERVED executed', '1', 'RESERVED')
case('VER executed', '1', 'VER')
case('0xba executed', '1', '0xba')
case('0xff executed', '1', '0xff')
case('disabled, unexecuted', '0', 'IF 2MUL ENDIF 1')
for op in ('CAT', 'SUBSTR', 'LEFT', 'RIGHT', 'INVERT', 'AND', 'OR', 'XOR', '2MUL', '2DIV', 'MUL', 'DIV', 'MOD', 'LSHIFT', 'RSHIFT'):
    case('disabled ' + op, '1', '0 IF ' + op + ' ENDIF')
case('VERIFY true', '1', 'VERIFY 1')
case('VERIFY false', '0', 'VERIFY 1')
case('VERIFY empty', '', 'VERIFY 1')
case('RETURN', '1', 'RETURN')
case('RETURN unexecuted', '0', 'IF RETURN ENDIF 1')
case('NOP', '', 'NOP 1')
case('a push past the end', '', '1 0x4c')
case('a push of 520', '', '0x4d 0x0802 0x' + '00' * 520 + ' SIZE 520 EQUAL NIP')
case('a push of 521', '', '0x4d 0x0902 0x' + '00' * 521 + ' DROP 1')
def sized(n):
    """A script of n bytes (9,958 to 10,212) that leaves true."""
    body = (b'\x4d\x08\x02' + bytes(520) + b'\x75') * 19
    rest = n - len(body) - 4
    return body + b'\x4c' + bytes([rest]) + bytes(rest) + b'\x75\x51'


case('a script of 10,000 bytes', '', sized(10000))
case('a script of 10,001 bytes', '', sized(10001))
case('201 ops', '1', ' '.join(['NOP'] * 201))
case('202 ops', '1', ' '.join(['NOP'] * 202))
case('202 ops, unexecuted', '0', 'IF ' + ' '.join(['NOP'] * 200) + ' ENDIF 1')
case('pushes are not ops', '', ' '.join(['1'] * 250) + ' ' + ' '.join(['DROP'] * 199))
case('1,000 elements', '', ' '.join(['1'] * 500) + ' ' + ' '.join(['1'] * 500))
case('1,001 elements', '', ' '.join(['1'] * 1001))
case('1,001 with the alt stack', '', ' '.join(['1'] * 500) + ' TOALTSTACK ' + ' '.join(['1'] * 501))

# The stack opcodes: each result, and each one a stack too short.
for label, spk in [
    ('TOALTSTACK FROMALTSTACK', '1 2 TOALTSTACK 3 FROMALTSTACK 2 EQUALVERIFY 3 EQUALVERIFY 1 EQUAL'),
    ('2DROP', '1 2 3 2DROP 1 EQUAL'), ('2DUP', '1 2 2DUP 2 EQUALVERIFY 1 EQUALVERIFY 2 EQUALVERIFY 1 EQUAL'),
    ('3DUP', '1 2 3 3DUP 3 EQUALVERIFY 2 EQUALVERIFY 1 EQUALVERIFY 3 EQUALVERIFY 2 EQUALVERIFY 1 EQUAL'),
    ('2OVER', '1 2 3 4 2OVER 2 EQUALVERIFY 1 EQUALVERIFY 4 EQUALVERIFY 3 EQUALVERIFY 2 EQUALVERIFY 1 EQUAL'),
    ('2ROT', '1 2 3 4 5 6 2ROT 2 EQUALVERIFY 1 EQUALVERIFY 6 EQUALVERIFY 5 EQUALVERIFY 4 EQUALVERIFY 3 EQUAL'),
    ('2ROT deeper', '9 1 2 3 4 5 6 2ROT 2 EQUALVERIFY 1 EQUALVERIFY 6 EQUALVERIFY 5 EQUALVERIFY 4 EQUALVERIFY 3 EQUALVERIFY 9 EQUAL'),
    ('2SWAP', '1 2 3 4 2SWAP 2 EQUALVERIFY 1 EQUALVERIFY 4 EQUALVERIFY 3 EQUAL'),
    ('IFDUP true', '2 IFDUP 2 EQUALVERIFY 2 EQUAL'), ('IFDUP false', '1 0 IFDUP DEPTH 2 EQUALVERIFY DROP'),
    ('DEPTH', '7 7 7 DEPTH 3 EQUAL'), ('DEPTH empty', 'DEPTH 0 EQUAL'), ('DROP', '1 2 DROP'),
    ('DUP', '2 DUP 2 EQUALVERIFY 2 EQUAL'), ('NIP', '1 2 NIP 2 EQUALVERIFY DEPTH 0 EQUAL'),
    ('OVER', '1 2 OVER 1 EQUALVERIFY 2 EQUALVERIFY 1 EQUAL'),
    ('PICK', '1 2 3 2 PICK 1 EQUALVERIFY 3 EQUALVERIFY 2 EQUALVERIFY 1 EQUAL'),
    ('PICK 0', '1 2 0 PICK 2 EQUALVERIFY 2 EQUALVERIFY 1 EQUAL'),
    ('ROLL', '1 2 3 2 ROLL 1 EQUALVERIFY 3 EQUALVERIFY 2 EQUALVERIFY DEPTH 0 EQUAL'),
    ('ROLL 1', '1 2 3 1 ROLL 2 EQUALVERIFY 3 EQUALVERIFY 1 EQUAL'),
    ('ROT', '1 2 3 ROT 1 EQUALVERIFY 3 EQUALVERIFY 2 EQUAL'), ('SWAP', '1 2 SWAP 1 EQUALVERIFY 2 EQUAL'),
    ('TUCK', '1 2 TUCK 2 EQUALVERIFY 1 EQUALVERIFY 2 EQUAL'), ('SIZE', "'abc' SIZE 3 EQUALVERIFY 'abc' EQUAL"),
    ('SIZE empty', '0 SIZE 0 EQUALVERIFY 0 EQUAL'),
    ('EQUAL false', '1 2 EQUAL NOT'), ('EQUALVERIFY false', '1 2 EQUALVERIFY 1'),
    ('EQUAL lengths', '0x02 0x0100 1 EQUAL NOT'),
]:
    case(label, '', spk)
for op, need in [('TOALTSTACK', 1), ('2DROP', 2), ('2DUP', 2), ('3DUP', 3), ('2OVER', 4), ('2ROT', 6), ('2SWAP', 4),
                 ('IFDUP', 1), ('DROP', 1), ('DUP', 1), ('NIP', 2), ('OVER', 2), ('ROT', 3), ('SWAP', 2), ('TUCK', 2),
                 ('SIZE', 1), ('EQUAL', 2), ('EQUALVERIFY', 2), ('1ADD', 1), ('ADD', 2), ('WITHIN', 3), ('SHA256', 1),
                 ('CHECKSIG', 2), ('NUMEQUALVERIFY', 2)]:
    case(op + ' short', '', ' '.join(['1'] * (need - 1)) + ' ' + op)
case('FROMALTSTACK empty', '1', 'FROMALTSTACK')
case('PICK short', '', '1 PICK')
case('PICK past the end', '', '1 1 PICK')
case('PICK negative', '', '1 -1 PICK')
case('ROLL past the end', '', '1 2 2 ROLL')
case('PICK non-minimal', '', '1 0x01 0x00 PICK', 'MINIMALDATA')
case('PICK non-minimal, allowed', '', '1 0x01 0x00 PICK')
case('PICK 5 bytes', '', '1 0x05 0x0000000000 PICK')

# Numbers.
for expr in ['1 1ADD 2 EQUAL', '-1 1ADD 0 EQUAL', '1 1SUB 0 EQUAL', '0 1SUB -1 EQUAL', '-1 NEGATE 1 EQUAL', '0 NEGATE 0 EQUAL',
             '-5 ABS 5 EQUAL', '5 ABS 5 EQUAL', '0 NOT 1 EQUAL', '7 NOT 0 EQUAL', '0 0NOTEQUAL 0 EQUAL',
             '-3 0NOTEQUAL 1 EQUAL', '2 3 ADD 5 EQUAL', '2 3 SUB -1 EQUAL', '1 0 BOOLAND 0 EQUAL', '0 1 BOOLAND 0 EQUAL',
             '1 2 BOOLAND 1 EQUAL', '0 0 BOOLOR 0 EQUAL', '0 3 BOOLOR 1 EQUAL', '3 0 BOOLOR 1 EQUAL', '3 3 NUMEQUAL',
             '3 4 NUMEQUAL NOT', '3 4 NUMNOTEQUAL', '3 3 NUMNOTEQUAL NOT', '3 4 LESSTHAN', '4 4 LESSTHAN NOT',
             '4 3 GREATERTHAN', '4 4 GREATERTHAN NOT', '4 4 LESSTHANOREQUAL', '5 4 LESSTHANOREQUAL NOT',
             '4 4 GREATERTHANOREQUAL', '3 4 GREATERTHANOREQUAL NOT', '3 4 MIN 3 EQUAL', '4 3 MIN 3 EQUAL',
             '3 4 MAX 4 EQUAL', '4 3 MAX 4 EQUAL', '2 2 5 WITHIN', '4 2 5 WITHIN', '5 2 5 WITHIN NOT', '1 2 5 WITHIN NOT',
             '3 3 NUMEQUALVERIFY 1', '-1 -1 ADD -2 EQUAL', '0x01 0x80 NOT', '0x02 0x0080 0 NUMEQUAL',
             '0x02 0xff00 255 EQUAL', '0x02 0xff80 -255 EQUAL', '0x04 0xffffff7f 1ADD 0x05 0x0000008000 EQUAL',
             '0x04 0xffffffff 1SUB 0x05 0x0000008080 EQUAL', '0x04 0xffffff7f DUP ADD 0x05 0xfeffffff00 EQUAL',
             '128 0x02 0x8000 EQUAL', '-128 0x02 0x8080 EQUAL', '127 0x01 0x7f EQUAL', '256 0x02 0x0001 EQUAL',
             '0x05 0x0000008000 SIZE 5 EQUALVERIFY DROP 1']:
    case(expr, '', expr)
case('NUMEQUALVERIFY false', '', '3 4 NUMEQUALVERIFY 1')
case('a 5-byte operand', '', '0x05 0x0000008000 1ADD')
case('a 5-byte second operand', '', '1 0x05 0x0000008000 ADD')
case('a 5-byte third operand', '', '1 1 0x05 0x0000008000 WITHIN')
case('negative zero, minimal', '', '0x01 0x80 NOT', 'MINIMALDATA')
case('0x0100, minimal', '', '0x02 0x0100 1ADD', 'MINIMALDATA')
case('0xff00, minimal', '', '0x02 0xff00 1ADD 256 EQUAL', 'MINIMALDATA')

# Hashes of 'abc' and of nothing.
abc = b'abc'
for op, digest in [('RIPEMD160', R.ripemd160(abc)), ('SHA1', hashlib.sha1(abc).digest()), ('SHA256', hashlib.sha256(abc).digest()),
                   ('HASH160', R.ripemd160(hashlib.sha256(abc).digest())), ('HASH256', T.sha256d(abc))]:
    case(op, '', "'abc' %s 0x%02x 0x%s EQUAL" % (op, len(digest), digest.hex()))
case('SHA256 of nothing', '', '0 SHA256 0x20 0x' + hashlib.sha256(b'').hexdigest() + ' EQUAL')

# Pushes under MINIMALDATA.
for label, spk in [('a one-byte push of 5', '0x01 0x05'), ('PUSHDATA1 of one byte', '0x4c 0x01 0x07'),
                   ('PUSHDATA1 of nothing', '0x4c 0x00 1'), ('a direct push of nothing', '0 1'),
                   ('a push of 0x81', '0x01 0x81'), ('PUSHDATA1 of 75 bytes', '0x4c 0x4b 0x' + '01' * 75),
                   ('a direct push of 75 bytes', '0x4b 0x' + '01' * 75), ('PUSHDATA2 of 255 bytes', '0x4d 0xff00 0x' + '01' * 255),
                   ('PUSHDATA1 of 255 bytes', '0x4c 0xff 0x' + '01' * 255), ('PUSHDATA2 of 256 bytes', '0x4d 0x0001 0x' + '01' * 256),
                   ('PUSHDATA4 of 256 bytes', '0x4e 0x00010000 0x' + '01' * 256), ('a push of 0x00', '0x01 0x00 NOT'),
                   ('a push of 0x11', '0x01 0x11'), ('a push of two bytes', '0x02 0x0101')]:
    case(label, '', spk, 'MINIMALDATA')
case('non-minimal, unexecuted', '', '0 IF 0x01 0x05 ENDIF 1', 'MINIMALDATA')
case('non-minimal in the scriptSig', '0x01 0x05', '', 'MINIMALDATA')

# NOPs and their flag.
for op in ('NOP1', 'NOP4', 'NOP10', 'CHECKLOCKTIMEVERIFY', 'CHECKSEQUENCEVERIFY'):
    case(op, '1', op)
    case(op + ' discouraged', '1', op, 'DISCOURAGE_UPGRADABLE_NOPS')
case('NOP4 discouraged, unexecuted', '0', 'IF NOP4 ENDIF 1', 'DISCOURAGE_UPGRADABLE_NOPS')

# CHECKSIG and CHECKMULTISIG without a valid signature.
case('CHECKSIG, empty signature', '0', KEY05 + ' CHECKSIG NOT')
case('CHECKSIG, bad key', SIG11, KEY05 + ' CHECKSIG NOT')
case('CHECKSIG, NULLFAIL', SIG11, KEY05 + ' CHECKSIG NOT', 'NULLFAIL')
case('CHECKSIG, NULLFAIL, empty', '0', KEY05 + ' CHECKSIG NOT', 'NULLFAIL')
case('CHECKSIGVERIFY fails', '0', KEY05 + ' CHECKSIGVERIFY 1')
case('CHECKSIG, DERSIG', '0x01 0x01', KEY05 + ' CHECKSIG NOT', 'DERSIG')
case('CHECKSIG, DERSIG off', '0x01 0x01', KEY05 + ' CHECKSIG NOT')
case('CHECKSIG, LOW_S', HIGH_S, KEY05 + ' CHECKSIG NOT', 'LOW_S')
case('CHECKSIG, STRICTENC hash type', '0x09 0x300602010102010100', KEY05 + ' CHECKSIG NOT', 'STRICTENC')
case('CHECKSIG, STRICTENC key', '0', KEY05 + ' CHECKSIG NOT', 'STRICTENC')
case('CHECKSIG, STRICTENC 65-byte key', '0', UNCOMPRESSED + ' CHECKSIG NOT', 'STRICTENC')
case('CHECKSIG, STRICTENC 02 key of 65', '0', '0x41 0x02' + '22' * 64 + ' CHECKSIG NOT', 'STRICTENC')
case('CHECKSIG, STRICTENC 04 key of 33', '0', '0x21 0x04' + '22' * 32 + ' CHECKSIG NOT', 'STRICTENC')
case('CHECKSIG, an empty key', SIG11, '0 CHECKSIG NOT')
case('CHECKSIG, a 33-byte key not on the curve', SIG11, '0x21 0x02' + 'ff' * 32 + ' CHECKSIG NOT')
case('CHECKSIG, a hybrid key of the wrong parity', SIG11, '0x41 0x07' + '79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798'
     + '483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8' + ' CHECKSIG NOT')
case('CHECKSIG, a 65-byte key with prefix 05', SIG11, '0x41 0x05' + '22' * 64 + ' CHECKSIG NOT')
case('CHECKSIG, a 32-byte key', SIG11, '0x20 0x02' + '22' * 31 + ' CHECKSIG NOT')
case('CHECKSIG, a signature r = 0', '0x09 0x300602010002010101', KEY05 + ' CHECKSIG NOT')
case('FindAndDelete, CONST_SCRIPTCODE', '0', '0 DROP ' + KEY05 + ' CHECKSIG NOT', 'CONST_SCRIPTCODE')
case('FindAndDelete of a signature', SIG11, SIG11 + ' DROP ' + KEY05 + ' CHECKSIG NOT', 'CONST_SCRIPTCODE')
case('FindAndDelete, not at an op boundary', SIG11, '0x0a 0x' + SIG11[2:].replace(' 0x', '') + ' DROP ' + KEY05 + ' CHECKSIG NOT', 'CONST_SCRIPTCODE')
case('CODESEPARATOR, CONST_SCRIPTCODE', '1', 'CODESEPARATOR', 'CONST_SCRIPTCODE')
case('CODESEPARATOR unexecuted, CONST_SCRIPTCODE', '0', 'IF CODESEPARATOR ENDIF 1', 'CONST_SCRIPTCODE')
case('CODESEPARATOR', '1', 'CODESEPARATOR')
case('CHECKMULTISIG 0 of 0', '0', '0 0 CHECKMULTISIG')
case('CHECKMULTISIG 0 of 1', '0', '0 ' + KEY05 + ' 1 CHECKMULTISIG')
case('CHECKMULTISIG 1 of 1 fails', '0 0', KEY05 + ' 1 CHECKMULTISIG NOT')
case('CHECKMULTISIG 1 of 2 fails', '0 0', '1 ' + KEY05 + ' ' + KEY05 + ' 2 CHECKMULTISIG NOT')
case('CHECKMULTISIG 2 of 2 fails early', '0 0 0', '2 ' + KEY05 + ' ' + KEY05 + ' 2 CHECKMULTISIG NOT')
case('CHECKMULTISIGVERIFY fails', '0 0', '1 ' + KEY05 + ' 1 CHECKMULTISIGVERIFY 1')
case('CHECKMULTISIG, NULLDUMMY', '1 0', '0 0 CHECKMULTISIG NOT NOT', 'NULLDUMMY')
case('CHECKMULTISIG, dummy allowed', '1', '0 0 CHECKMULTISIG')
case('CHECKMULTISIG, no dummy', '', '0 0 CHECKMULTISIG')
case('CHECKMULTISIG, NULLFAIL', '0 ' + SIG11, '1 ' + KEY05 + ' 1 CHECKMULTISIG NOT', 'NULLFAIL')
case('CHECKMULTISIG, NULLFAIL, empty', '0 0', '1 ' + KEY05 + ' 1 CHECKMULTISIG NOT', 'NULLFAIL')
case('CHECKMULTISIG, NULLFAIL, second', '0 ' + SIG11 + ' 0', '2 ' + KEY05 + ' ' + KEY05 + ' 2 CHECKMULTISIG NOT', 'NULLFAIL')
case('CHECKMULTISIG, NULLFAIL, keys are not checked', '0 0', '1 ' + KEY05 + ' 1 CHECKMULTISIG NOT', 'NULLFAIL')
case('CHECKMULTISIG, 21 keys', '0 0', ' '.join(['0'] * 21) + ' 21 CHECKMULTISIG')
case('CHECKMULTISIG, 20 keys', '0 0', ' '.join(['0'] * 20) + ' 20 CHECKMULTISIG')
case('CHECKMULTISIG, -1 keys', '0', '-1 CHECKMULTISIG')
case('CHECKMULTISIG, 2 of 1', '0 0 0', '2 0 1 CHECKMULTISIG')
case('CHECKMULTISIG, -1 of 1', '0', '-1 0 1 CHECKMULTISIG')
case('CHECKMULTISIG, short of keys', '', '1 0 2 CHECKMULTISIG')
case('CHECKMULTISIG, short of signatures', '', '0 2 0 0 2 CHECKMULTISIG')
case('CHECKMULTISIG, short of everything', '', 'CHECKMULTISIG')
case('CHECKMULTISIG, a 5-byte key count', '', '0x05 0x0000000000 CHECKMULTISIG')
case('CHECKMULTISIG, a 5-byte signature count', '', '0x05 0x0000000000 0 CHECKMULTISIG')
case('CHECKMULTISIG, keys as ops', '0 0', ' '.join(['NOP'] * 180) + ' ' + ' '.join(['0'] * 20) + ' 20 CHECKMULTISIG')
case('CHECKMULTISIG, keys past 201', '0 0', ' '.join(['NOP'] * 181) + ' ' + ' '.join(['0'] * 20) + ' 20 CHECKMULTISIG')
case('CHECKMULTISIG, keys counted, then ops', '0 0', ' '.join(['NOP'] * 160) + ' ' + ' '.join(['0'] * 20) + ' 20 CHECKMULTISIG ' + ' '.join(['NOP'] * 20))
case('CHECKMULTISIG, keys counted, one op more', '0 0', ' '.join(['NOP'] * 160) + ' ' + ' '.join(['0'] * 20) + ' 20 CHECKMULTISIG ' + ' '.join(['NOP'] * 21))
case('CHECKMULTISIG, FindAndDelete', '0 ' + SIG11, SIG11 + ' DROP 1 ' + KEY05 + ' 1 CHECKMULTISIG NOT', 'CONST_SCRIPTCODE')
case('CHECKMULTISIGVERIFY, keys as ops', '0 0', ' '.join(['NOP'] * 181) + ' 0 0 CHECKMULTISIGVERIFY 1')

# VerifyScript: P2SH, SIGPUSHONLY, CLEANSTACK and the flag rules.
redeem = b'\x51'
case('P2SH', '0x01 0x51', sh(redeem), 'P2SH')
case('P2SH off', '0x01 0x51', sh(redeem))
case('P2SH, a redeem script that fails', '0x01 0x00', sh(b'\x00'), 'P2SH')
case('P2SH, a redeem script that leaves nothing', '0x01 0x75', sh(b'\x75'), 'P2SH')
case('P2SH, a redeem script that errs', '0x01 0x6a', sh(b'\x6a'), 'P2SH')
case('P2SH, not push only', 'NOP 0x01 0x51', sh(redeem), 'P2SH')
case('P2SH, not push only, off', 'NOP 0x01 0x51', sh(redeem))
case('P2SH, arguments', '1 2 0x03 0x52935387', sh(bytes.fromhex('52935387')), 'P2SH')
case('P2SH, a 22-byte script', '0x01 0x51', sh(redeem)[:22], 'P2SH')
case('P2SH, a wrong last opcode', '0x01 0x51', sh(redeem)[:22] + b'\x88' + b'\x51', 'P2SH')
case('P2SH, a wrong push', '0x01 0x51', b'\xa9\x15' + sh(redeem)[2:22] + b'\x00\x87', 'P2SH')
case('P2SH, a wrong first opcode', '0x01 0x51', b'\xa8' + sh(redeem)[1:], 'P2SH')
case('SIGPUSHONLY', 'NOP 1', '', 'SIGPUSHONLY')
case('SIGPUSHONLY, RESERVED is a push', 'RESERVED 1', '0x02 0x5051 EQUAL', 'SIGPUSHONLY')
case('SIGPUSHONLY, a push past the end', '0x4c', '1', 'SIGPUSHONLY')
case('SIGPUSHONLY, 16 is a push', '16', '16 EQUAL', 'SIGPUSHONLY')
case('SIGPUSHONLY, NOP is not', 'NOP', '1', 'SIGPUSHONLY')
case('CLEANSTACK', '1 1', '', 'P2SH,WITNESS,CLEANSTACK')
case('CLEANSTACK, clean', '1', '', 'P2SH,WITNESS,CLEANSTACK')
case('CLEANSTACK, P2SH', '1 0x01 0x51', sh(redeem), 'P2SH,WITNESS,CLEANSTACK')
case('WITNESS without P2SH', '1', '', 'WITNESS')
case('CLEANSTACK without WITNESS', '1', '', 'P2SH,CLEANSTACK')
case('the scriptSig errs', 'RETURN', '1')
case('the scriptPubKey errs', '1', 'RETURN')

# Witness programs.
W = 'P2SH,WITNESS'
ok = b'\x51'
case('P2WSH', '', wsh(ok), W, [ok])
case('P2WSH, witness off', '', wsh(ok), 'P2SH', [ok])
case('P2WSH, arguments', '', wsh(bytes.fromhex('935387')), W, [b'\x02', b'\x03', bytes.fromhex('935387')])
case('P2WSH, mismatch', '', wsh(ok), W, [b'\x52'])
case('P2WSH, no witness', '', wsh(ok), W, [])
case('P2WSH, two left', '', wsh(ok), W, [b'\x01', ok])
case('P2WSH, false', '', wsh(b'\x00'), W, [b'\x00'])
case('P2WSH, leaves nothing', '', wsh(b'\x75'), W, [b'\x01', b'\x75'])
case('P2WSH, errs', '', wsh(b'\x6a'), W, [b'\x6a'])
case('P2WSH, an item of 521', '', wsh(b'\x75\x51'), W, [bytes(521), b'\x75\x51'])
case('P2WSH, an item of 520', '', wsh(b'\x75\x51'), W, [bytes(520), b'\x75\x51'])
case('P2WSH, a script of 521 bytes', '', wsh(bytes(520) + b'\x51'), W, [bytes(520) + b'\x51'])
case('P2WSH, MINIMALIF', '', wsh(P('IF 1 ENDIF')), W + ',MINIMALIF', [b'\x02', P('IF 1 ENDIF')])
case('P2WSH, MINIMALIF two bytes', '', wsh(P('NOTIF 1 ENDIF')), W + ',MINIMALIF', [b'\x00\x00', P('NOTIF 1 ENDIF')])
case('P2WSH, MINIMALIF one', '', wsh(P('IF 1 ENDIF')), W + ',MINIMALIF', [b'\x01', P('IF 1 ENDIF')])
case('P2WSH, MINIMALIF off', '', wsh(P('IF 1 ENDIF')), W, [b'\x02', P('IF 1 ENDIF')])
case('MINIMALIF, legacy', '2', 'IF 1 ENDIF', W + ',MINIMALIF')
case('P2WSH, WITNESS_PUBKEYTYPE', '', wsh(P(UNCOMPRESSED + ' CHECKSIG NOT')), W + ',WITNESS_PUBKEYTYPE', [b'', P(UNCOMPRESSED + ' CHECKSIG NOT')])
case('WITNESS_PUBKEYTYPE, legacy', '0', UNCOMPRESSED + ' CHECKSIG NOT', W + ',WITNESS_PUBKEYTYPE')
case('P2WSH, no FindAndDelete', '', wsh(P('0 DROP ' + KEY05 + ' CHECKSIG NOT')), W + ',CONST_SCRIPTCODE', [b'', P('0 DROP ' + KEY05 + ' CHECKSIG NOT')])
case('P2WSH, CODESEPARATOR allowed', '', wsh(P('CODESEPARATOR 1')), W + ',CONST_SCRIPTCODE', [P('CODESEPARATOR 1')])
case('P2WSH, CHECKSIG, a bad key', '', wsh(P(KEY05 + ' CHECKSIG NOT')), W, [bytes.fromhex('300602010102010101'), P(KEY05 + ' CHECKSIG NOT')])
case('P2WPKH, one item', '', b'\x00\x14' + b'\x01' * 20, W, [b'\x01'])
case('P2WPKH, wrong key', '', b'\x00\x14' + b'\x01' * 20, W, [b'', bytes.fromhex('02' + '22' * 32)])
case('a v0 program of 21 bytes', '', b'\x00\x15' + b'\x01' * 21, W, [b'\x01'])
case('a witness program and a scriptSig', '1', wsh(ok), W, [ok])
case('a witness on a legacy output', '1', '', W, [b'\x01'])
case('a witness, witness off', '1', '', 'P2SH', [b'\x01'])
case('an empty witness item on a legacy output', '1', '', W, [b''])
case('v2', '', b'\x52\x02\x01\x02', W, [b'\x01'])
case('v2, discouraged', '', b'\x52\x02\x01\x02', W + ',DISCOURAGE_UPGRADABLE_WITNESS_PROGRAM', [b'\x01'])
case('v16 of 40 bytes, discouraged', '', b'\x60\x28' + b'\x01' * 40, W + ',DISCOURAGE_UPGRADABLE_WITNESS_PROGRAM', [])
case('v1 of 32 bytes, Taproot off', '', b'\x51\x20' + b'\x01' * 32, W, [b'\x01'])
case('v1 of 32 bytes, Taproot on', '', b'\x51\x20' + b'\x01' * 32, W + ',TAPROOT', [b'\x01'])
case('P2A', '', b'\x51\x02\x4e\x73', W + ',TAPROOT,DISCOURAGE_UPGRADABLE_WITNESS_PROGRAM', [])
case('v1, 2 bytes, not P2A', '', b'\x51\x02\x4e\x74', W + ',DISCOURAGE_UPGRADABLE_WITNESS_PROGRAM', [])
case('v1, 2 bytes, first byte not P2A', '', b'\x51\x02\x4f\x73', W + ',DISCOURAGE_UPGRADABLE_WITNESS_PROGRAM', [])
case('three bytes is no program', '', b'\x00\x01\x01', W, [b'\x01'])
case('43 bytes is no program', '', b'\x00\x29' + b'\x01' * 41, W, [b'\x01'])
case('a push of the wrong length is no program', '', b'\x00\x21' + b'\x01' * 32, W, [b'\x01'])
case('OP_1NEGATE is no version', '', b'\x4f\x02\x01\x02', W, [b'\x01'])
case('P2SH-P2WSH', push := R.push_data(wsh(ok)), sh(wsh(ok)), W, [ok])
case('P2SH-P2WSH, malleated', b'\x00' + push, sh(wsh(ok)), W, [ok])
case('P2SH-P2WSH, PUSHDATA1', b'\x4c' + push, sh(wsh(ok)), W, [ok])
case('P2SH-P2WSH, another byte', push[:-1] + bytes([push[-1] ^ 1]), sh(push[1:-1] + bytes([push[-1] ^ 1])), W, [ok])
case('P2SH-P2WSH, mismatch', push, sh(wsh(ok)), W, [b'\x52'])
case('P2SH-P2WSH, witness off', push, sh(wsh(ok)), 'P2SH', [ok])
case('P2SH-v1 of 32 bytes is not Taproot', R.push_data(b'\x51\x20' + b'\x01' * 32), sh(b'\x51\x20' + b'\x01' * 32), W + ',TAPROOT,DISCOURAGE_UPGRADABLE_WITNESS_PROGRAM', [b'\x01'])
case('P2SH-P2A is not an anchor', R.push_data(b'\x51\x02\x4e\x73'), sh(b'\x51\x02\x4e\x73'), W + ',DISCOURAGE_UPGRADABLE_WITNESS_PROGRAM', [])
case('P2SH-v2', R.push_data(b'\x52\x02\x01\x02'), sh(b'\x52\x02\x01\x02'), W, [b'\x01'])
case('P2SH-P2WSH, CLEANSTACK', push, sh(wsh(ok)), W + ',CLEANSTACK', [ok])
case('P2WSH, CLEANSTACK', '', wsh(ok), W + ',CLEANSTACK', [ok])
# Large witness stacks: more elements than the workspace stores.
for count, script in [(1200, b''), (1200, b'\x75'), (1200, P('1100 PICK')), (1200, P('1099 PICK')), (1200, P('1200 PICK')),
                      (1200, P('DEPTH')), (1200, P('1 ROLL')), (1001, b'\x75'), (1001, P('2DROP')), (1000, P('DEPTH 1000 EQUALVERIFY 2DROP'))]:
    case('a witness stack of %d, %s' % (count, script.hex() or 'empty'), '', wsh(script), W, [b''] * count + [script])

# Lock times: CLTV against nLockTime, CSV against nSequence.
L = 'CHECKLOCKTIMEVERIFY'; Q = 'CHECKSEQUENCEVERIFY'
case('CLTV', '', '100 ' + L, L, locktime=100, sequence=0)
case('CLTV, a later height', '', '101 ' + L, L, locktime=100, sequence=0)
case('CLTV, final', '', '100 ' + L, L, locktime=100)
case('CLTV, a time against a height', '', '500000000 ' + L, L, locktime=100, sequence=0)
case('CLTV, a height against a time', '', '100 ' + L, L, locktime=500000000, sequence=0)
case('CLTV, times', '', '500000000 ' + L, L, locktime=500000001, sequence=0)
case('CLTV, the last height', '', '499999999 ' + L, L, locktime=499999999, sequence=0)
case('CLTV, negative', '', '-1 ' + L, L, locktime=100, sequence=0)
case('CLTV, 5 bytes', '', '0x05 0xffffffff00 ' + L, L, locktime=0xffffffff, sequence=0)
case('CLTV, 6 bytes', '', '0x06 0xffffffff0000 ' + L, L, locktime=0xffffffff, sequence=0)
case('CLTV, empty', '', L, L, locktime=100, sequence=0)
case('CLTV, minimal', '', '0x02 0x6400 ' + L, L + ',MINIMALDATA', locktime=100, sequence=0)
case('CLTV, unexecuted', '', '0 IF 101 ' + L + ' ENDIF 1', L, locktime=100, sequence=0)
case('CSV', '', '10 ' + Q, Q, version=2, sequence=10)
case('CSV, more', '', '11 ' + Q, Q, version=2, sequence=10)
case('CSV, version 1', '', '10 ' + Q, Q, version=1, sequence=10)
case('CSV, version 0xffffffff', '', '10 ' + Q, Q, version=0xffffffff, sequence=10)
case('CSV, disabled in the input', '', '10 ' + Q, Q, version=2, sequence=(1 << 31) | 10)
case('CSV, disabled in the operand', '', '0x05 0x0a00008000 ' + Q, Q, version=1, sequence=10)
case('CSV, time against height', '', '0x03 0x0a0040 ' + Q, Q, version=2, sequence=10)
case('CSV, height against time', '', '10 ' + Q, Q, version=2, sequence=(1 << 22) | 10)
case('CSV, times', '', '0x03 0x0a0040 ' + Q, Q, version=2, sequence=(1 << 22) | 11)
case('CSV, masked bits', '', '0x04 0x0a00ff00 ' + Q, Q, version=2, sequence=(1 << 23) | 10)
case('CSV, masked bits of the input', '', '10 ' + Q, Q, version=2, sequence=(1 << 30) | (1 << 16) | 10)
case('CSV, negative', '', '-1 ' + Q, Q, version=2, sequence=10)
case('CSV, empty', '', Q, Q, version=2, sequence=10)
case('CSV, 6 bytes', '', '0x06 0x0a0000000000 ' + Q, Q, version=2, sequence=10)

# Edges the cases above leave open: each flag's constant with P2SH alone
# set, results compared with 1 rather than read as true, exact stack
# sizes, and the parts of VerifyScript a witness program skips.
G_X = '79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798'
SIG22 = '0x09 0x300602010202010101'          # DER r = 2, s = 1, SIGHASH_ALL
NONDER = '0x01 0x01'
UNDEFINED = '0x09 0x300602010102010100'      # hash type 0
for label, spk in [('a key of 33 with prefix 02', '0x21 0x02' + '22' * 32), ('a key of 33 with prefix 03', '0x21 0x03' + '22' * 32)]:
    case('CHECKSIG, STRICTENC, ' + label, '0', spk + ' CHECKSIG NOT', 'STRICTENC')
for label, key in [('02 of 33', '0x21 0x02' + '22' * 32), ('03 of 33', '0x21 0x03' + '22' * 32),
                   ('04 of 33', '0x21 0x04' + '22' * 32), ('02 of 65', '0x41 0x02' + '22' * 64)]:
    script = P(key + ' CHECKSIG NOT')
    case('P2WSH, WITNESS_PUBKEYTYPE, a key ' + label, '', wsh(script), W + ',WITNESS_PUBKEYTYPE', [b'', script])
case('CHECKSIG, a signature the lax parser refuses', '0x02 0x0101', '0x21 0x02' + G_X + ' CHECKSIG NOT')
for label, sig, flags in [('non-DER, P2SH only', NONDER, 'P2SH'), ('high S, P2SH only', HIGH_S, 'P2SH'),
                          ('high S, DERSIG', HIGH_S, 'DERSIG'), ('an undefined hash type, P2SH only', UNDEFINED, 'P2SH'),
                          ('non-DER, LOW_S', NONDER, 'LOW_S'), ('non-DER, STRICTENC', NONDER, 'STRICTENC'),
                          ('NULLFAIL, a 1-byte signature', NONDER, 'NULLFAIL')]:
    case('CHECKSIG, ' + label, sig, KEY05 + ' CHECKSIG NOT', flags)
case('FindAndDelete of a signature, P2SH only', SIG11, SIG11 + ' DROP ' + KEY05 + ' CHECKSIG NOT', 'P2SH')
case('CODESEPARATOR, P2SH only', '1', 'CODESEPARATOR', 'P2SH')
case('CHECKMULTISIG, FindAndDelete, P2SH only', '0 ' + SIG11, SIG11 + ' DROP 1 ' + KEY05 + ' 1 CHECKMULTISIG NOT', 'P2SH')
case('CHECKMULTISIG, CONST_SCRIPTCODE, no signature in the script', '0 ' + SIG11, '1 ' + KEY05 + ' 1 CHECKMULTISIG NOT', 'CONST_SCRIPTCODE')
case('CHECKMULTISIG, FindAndDelete of the second signature', '0 ' + SIG11 + ' ' + SIG22,
     SIG11 + ' DROP 2 ' + KEY05 + ' ' + KEY05 + ' 2 CHECKMULTISIG NOT', 'CONST_SCRIPTCODE')
case('CHECKMULTISIG, a non-minimal key count', '0 0', '0x02 0x0000 CHECKMULTISIG', 'MINIMALDATA')
case('CHECKMULTISIG, a non-minimal key count, P2SH only', '0 0', '0x02 0x0000 CHECKMULTISIG', 'P2SH')
case('CHECKMULTISIG, a non-minimal signature count', '0 0x02 0x0000', '0 CHECKMULTISIG', 'MINIMALDATA')
case('CHECKMULTISIG, STRICTENC key', '0 0', '1 ' + KEY05 + ' 1 CHECKMULTISIG NOT', 'STRICTENC')
case('CHECKMULTISIG, NULLFAIL, a 1-byte signature', '0 ' + NONDER, '1 ' + KEY05 + ' 1 CHECKMULTISIG NOT', 'NULLFAIL')
case('CHECKMULTISIG fails with a signature, P2SH only', '0 ' + SIG11, '1 ' + KEY05 + ' 1 CHECKMULTISIG NOT', 'P2SH')
case('CHECKMULTISIG, NULLDUMMY, a dummy of 1', '1', '0 0 CHECKMULTISIG', 'NULLDUMMY')
case('CHECKMULTISIG, a dummy of 1, P2SH only', '1', '0 0 CHECKMULTISIG', 'P2SH')
case('CHECKMULTISIG pops the dummy', '', '0 0 0 CHECKMULTISIG DEPTH 1 EQUALVERIFY')
case('TOALTSTACK of the only element', '', '1 TOALTSTACK DEPTH 0 EQUALVERIFY FROMALTSTACK')
case('2DROP of the only two', '', '1 2 2DROP DEPTH 0 EQUALVERIFY 1')
case('VERIFY pops', '', '1 VERIFY DEPTH 0 EQUAL')
for expr in ['3 3 NUMEQUAL', '3 4 NUMNOTEQUAL', '3 4 LESSTHAN', '4 3 GREATERTHAN', '4 4 LESSTHANOREQUAL', '4 4 GREATERTHANOREQUAL']:
    case(expr + ' is 1', '', expr + ' 1 EQUAL')
case('OP_16 is a push, not an op', '', '16 ' + ' '.join(['NOP'] * 201))
case('PICK non-minimal, P2SH only', '', '1 0x01 0x00 PICK', 'P2SH')
case('CLTV, P2SH only', '', '101 ' + L, 'P2SH', locktime=100, sequence=0)
case('CSV, P2SH only', '', '11 ' + Q, 'P2SH', version=2, sequence=10)
case('NOP1, P2SH only', '1', 'NOP1', 'P2SH')
case('CLTV of 0', '', '0 ' + L + ' DROP 1', L, locktime=0, sequence=0)
case('CSV, the type bit on both sides', '', '0x03 0x000040 ' + Q, Q, version=2, sequence=1 << 22)
case('the alt stack starts empty for the scriptPubKey', '1 TOALTSTACK', 'FROMALTSTACK')
case('MINIMALIF, not in the scriptSig', '2 IF 1 ENDIF', '', W + ',MINIMALIF')
case('P2SH, MINIMALIF, not in the redeem script', '2 0x03 0x635168', sh(bytes.fromhex('635168')), W + ',MINIMALIF')
case('v1 of 20 bytes, discouraged', '', b'\x51\x14' + b'\x01' * 20, W + ',DISCOURAGE_UPGRADABLE_WITNESS_PROGRAM', [b'\x01'])
case('P2WSH, mismatch, witness off', '', wsh(ok), 'P2SH', [b'\x52'])
case('P2SH-P2WSH, mismatch, witness off', push, sh(wsh(ok)), 'P2SH', [b'\x52'])
case('v2, CLEANSTACK', '', b'\x52\x02\x01\x02', W + ',CLEANSTACK', [b'\x01'])
case('P2SH-v2, CLEANSTACK', R.push_data(b'\x52\x02\x01\x02'), sh(b'\x52\x02\x01\x02'), W + ',CLEANSTACK', [b'\x01'])
# PICK and ROLL as a witness script's first opcode, the index the last
# witness item, around the elements the workspace stores.
for count, index, op in [(1200, 1098, 'PICK'), (1200, 1099, 'PICK'), (1200, 1198, 'PICK'), (1200, 1199, 'PICK'),
                         (1200, 1099, 'ROLL'), (1101, 1099, 'PICK'), (1101, 1100, 'PICK'), (1100, 1098, 'PICK')]:
    script = P(op)
    case('a witness stack of %d, %d %s' % (count, index, op), '', wsh(script), W,
         [b''] * (count - 1) + [index.to_bytes(2, 'little')] + [script])

# Signed spends, with keys from fixed labels (test keys, nothing else).
def key(label):
    return int.from_bytes(hashlib.sha256(label.encode()).digest(), 'big') % D.N


def sign(d, digest, high=False):
    z = int.from_bytes(digest, 'big')
    k = int.from_bytes(hashlib.sha256(d.to_bytes(32, 'big') + digest).digest(), 'big') % D.N
    r = D.mul(k, D.G)[0] % D.N
    s = pow(k, -1, D.N) * (z + r * d) % D.N
    s = max(s, D.N - s) if high else min(s, D.N - s)
    enc = lambda v: (lambda b: b'\x02' + bytes([len(b)]) + b)(v.to_bytes(33, 'big').lstrip(b'\0') if v.to_bytes(33, 'big').lstrip(b'\0')[0] < 0x80 else b'\0' + v.to_bytes(33, 'big').lstrip(b'\0'))
    body = enc(r) + enc(s)
    return b'\x30' + bytes([len(body)]) + body


d1, d2 = key('Bitcoin chapter 36 test key 1'), key('Bitcoin chapter 36 test key 2')
q1, q2 = D.mul(d1, D.G), D.mul(d2, D.G)
hybrid = bytes([6 + (q1[1] & 1)]) + q1[0].to_bytes(32, 'big') + q1[1].to_bytes(32, 'big')
compressed2 = bytes([2 + (q2[1] & 1)]) + q2[0].to_bytes(32, 'big')
signed = []
# A: legacy, 1 of 2: the first key tried (the last pushed) is no key, the
# second is the hybrid form of key 1. The scriptPubKey starts with
# CODESEPARATOR and carries the signature's own push, which FindAndDelete
# takes out of the scriptCode before hashing.
# Its outpoint is fixed rather than the crediting transaction's, whose
# txid would cover the scriptPubKey and so the signature.
tail = P('DROP 1 0x41 0x' + hybrid.hex() + ' ' + KEY05 + ' 2 CHECKMULTISIG')
s_a = spend(b'', [], b'')
s_a['ins'][0]['prevout'] = bytes([0x36]) * 32 + bytes(4)
digest = R.sighash_legacy(tail, s_a, 0, 1)   # the hash covers neither scriptSig nor scriptPubKey
sig1 = sign(d1, digest) + b'\x01'
spk_a = b'\xab' + R.push_data(sig1) + tail
s_a['ins'][0]['script'] = b'\x00' + R.push_data(sig1)
assert R.sighash_legacy(tail, s_a, 0, 1) == digest
# B: P2WPKH with key 2, an amount, and SIGHASH_ALL.
amount_b = 123456789
spk_b = b'\x00\x14' + R.ripemd160(hashlib.sha256(compressed2).digest())
s_b = spend(b'', [b'', compressed2], spk_b, amount_b)
code_b = b'\x76\xa9\x14' + spk_b[2:] + b'\x88\xac'
sig2 = sign(d2, R.sighash_v0(code_b, s_b, 0, 1, amount_b)) + b'\x01'
s_b['ins'][0]['witness'] = [sig2, compressed2]


# C: legacy, NOP CODESEPARATOR, then a CHECKSIGVERIFY with a key of prefix
# 02 and a 4-of-4 CHECKMULTISIGVERIFY with keys of prefix 03, 04, 06 and
# 07, every signature matching, one with the high s.
def key_with(prefix_parity, start):
    n = start
    while D.mul(key('Bitcoin chapter 36 test key %d' % n), D.G)[1] & 1 != prefix_parity: n += 1
    return key('Bitcoin chapter 36 test key %d' % n), n + 1


def encode(d, prefix):
    q = D.mul(d, D.G)
    x, y = q[0].to_bytes(32, 'big'), q[1].to_bytes(32, 'big')
    return bytes([prefix]) + x + (b'' if prefix in (2, 3) else y)


ds, nxt = [], 3
for prefix in (2, 3, 4, 6, 7):
    d, nxt = key_with({2: 0, 3: 1, 4: 0, 6: 0, 7: 1}[prefix], nxt)
    ds.append((d, encode(d, prefix)))
code_c = P('0x21 0x' + ds[0][1].hex() + ' CHECKSIGVERIFY 4 ' + ' '.join('0x%02x 0x%s' % (len(k), k.hex()) for _, k in ds[1:]) + ' 4 CHECKMULTISIGVERIFY 1')
spk_c = P('NOP CODESEPARATOR') + code_c
s_c = spend(b'', [], spk_c)
digest = R.sighash_legacy(code_c, s_c, 0, 1)
sigs_c = [sign(d, digest, high=(k == 2)) + b'\x01' for k, (d, _) in enumerate(ds)]
s_c['ins'][0]['script'] = b'\x00' + b''.join(R.push_data(s) for s in sigs_c[1:]) + R.push_data(sigs_c[0])
for label, s, spk, amount, flags in [('legacy 1 of 2, a hybrid key, CODESEPARATOR, FindAndDelete', s_a, spk_a, 0, ''),
                                     ('P2WPKH', s_b, spk_b, amount_b, 'P2SH,WITNESS,STRICTENC,DERSIG,LOW_S,NULLFAIL,WITNESS_PUBKEYTYPE'),
                                     ('legacy CHECKSIGVERIFY and 4 of 4, keys 02 to 07, a high s', s_c, spk_c, 0, 'P2SH,DERSIG,NULLDUMMY,NULLFAIL')]:
    flags_n = R.parse_flags(flags)
    i = s['ins'][0]
    error = R.verify_script(i['script'], spk, i['witness'], flags_n, R.Checker(s, 0, amount))
    assert error == 0, label
    signed.append(dict(label=label, tx=T.serialize(s, True), spk=spk, amount=amount, flags=flags_n, error=0))

# ---- Taproot and tapscript (chapter 44)
import taproot_reference as TR  # noqa: E402
WT = W + ',TAPROOT'


def tkey(n):
    return key('Bitcoin chapter 44 test key %d' % n)


def xonly(d):
    return TR.R.pubkey_gen(d.to_bytes(32, 'big'))


def leaf_hash(script, version=0xc0):
    return TR.R.tagged_hash('TapLeaf', bytes([version]) + TR.ser_script(script))


def branch(a, b):
    return TR.R.tagged_hash('TapBranch', a + b if a < b else b + a)


INTERNAL = tkey(0)
SIBLING = leaf_hash(P('1'))                      # a second leaf, for a path of one hash


class Sig:
    """A Schnorr signature made once the transaction is known: by key d
    (or the tweaked internal key, d None, for the key path), with a hash
    type, at a codesep position, and optionally spoiled."""
    def __init__(self, d=None, hashtype=0, codesep=0xffffffff, bad=False, annex='same', tail=None):
        self.d, self.hashtype, self.codesep, self.bad, self.annex, self.tail = d, hashtype, codesep, bad, annex, tail

    def make(self, s, spk, amount, script, annex, root):
        checker = R.Checker(s, 0, amount)
        checker.spk = spk
        a = annex if self.annex == 'same' else self.annex
        if self.d is None:
            msg = checker.sigmsg(self.hashtype, R.TAPROOT, dict(annex=a))
            d = TR.R.taproot_tweak_seckey(INTERNAL.to_bytes(32, 'big'), root)
        else:
            msg = checker.sigmsg(self.hashtype, R.TAPSCRIPT, dict(annex=a, script=script, codesep=self.codesep))
            d = self.d.to_bytes(32, 'big')
        sig = TR.R.schnorr_sign(TR.R.tagged_hash('TapSighash', msg), d, bytes(32))
        if self.bad: sig = sig[:63] + bytes([sig[63] ^ 1])
        if self.hashtype: sig += bytes([self.hashtype])
        if self.tail is not None: sig += self.tail
        return sig


def tap(label, script, items, flags=WT, version=0xc0, annex=None, path=True, control=None, amount=0, key_path=False):
    """A Taproot spend: by the key path (items is the signature alone) or by
    script, a leaf of version under the internal key, with a sibling leaf
    when path is set. control edits the control block."""
    if key_path:
        root = b''
    else:
        leaf = leaf_hash(script, version)
        root = branch(leaf, SIBLING) if path else leaf
    parity, q = TR.R.taproot_tweak_pubkey(xonly(INTERNAL), root)
    spk = b'\x51\x20' + q
    cb = bytes([version | parity]) + xonly(INTERNAL) + (SIBLING if path else b'')
    if control: cb = control(cb)
    tail = [] if key_path else [script, cb]
    if annex is not None: tail.append(annex)
    blank = spend(b'', [b'' if isinstance(i, Sig) else i for i in items] + tail, spk, amount)
    witness = [i.make(blank, spk, amount, script, annex, root) if isinstance(i, Sig) else i for i in items] + tail
    case(label, '', spk, flags, witness, amount)


K1, K2, K3 = tkey(1), tkey(2), tkey(3)
X1, X2, X3 = xonly(K1), xonly(K2), xonly(K3)
CS = lambda x: P('0x20 0x' + x.hex() + ' CHECKSIG')
ANNEX = b'\x50\x01\x02'
# The key path.
tap('key path, SIGHASH_DEFAULT', None, [Sig()], key_path=True)
tap('key path, SIGHASH_ALL', None, [Sig(hashtype=1)], key_path=True)
tap('key path, SINGLE|ANYONECANPAY', None, [Sig(hashtype=0x83)], key_path=True, amount=5000)
tap('key path, a spoiled signature', None, [Sig(bad=True)], key_path=True)
tap('key path, hash type 0 written out', None, [Sig(tail=b'\x00')], key_path=True)
tap('key path, hash type 4', None, [Sig(tail=b'\x04')], key_path=True)
tap('key path, hash type 0x80', None, [Sig(tail=b'\x80')], key_path=True)
tap('key path, 63 bytes', None, [bytes(63)], key_path=True)
tap('key path, an empty signature', None, [b''], key_path=True)
tap('key path, with an annex', None, [Sig()], key_path=True, annex=ANNEX)
tap('key path, signed without the annex it carries', None, [Sig(annex=None)], key_path=True, annex=ANNEX)
tap('key path, Taproot off', None, [b''], key_path=True, flags=W)
case('Taproot, an empty witness', '', b'\x51\x20' + X1, WT, [])
# The script path: the commitment.
tap('script path, a leaf that leaves 1', P('1'), [])
tap('script path, a lone leaf', P('1'), [], path=False)
tap('script path, the parity bit flipped', P('1'), [], control=lambda c: bytes([c[0] ^ 1]) + c[1:])
tap('script path, the path hash changed', P('1'), [], control=lambda c: c[:40] + bytes([c[40] ^ 1]) + c[41:])
tap('script path, a control block of 34 bytes', P('1'), [], control=lambda c: c[:34])
tap('script path, a control block of 32 bytes', P('1'), [], control=lambda c: c[:32])
tap('script path, an internal key off the curve', P('1'), [], control=lambda c: c[:1] + bytes(32) + c[33:])
tap('script path, leaf version 0xc2', P('RETURN'), [], version=0xc2)
tap('script path, leaf version 0xc2, discouraged', P('RETURN'), [], version=0xc2, flags=WT + ',DISCOURAGE_UPGRADABLE_TAPROOT_VERSION')
tap('script path, with an annex', P('1'), [], annex=ANNEX)
tap('script path, an annex of 0x50 alone', P('1'), [], annex=b'\x50')
# Tapscript: signatures.
tap('CHECKSIG, a valid signature', CS(X1), [Sig(K1)])
tap('CHECKSIG, SINGLE|ANYONECANPAY, an annex', CS(X1), [Sig(K1, 0x83)], annex=ANNEX)
tap('CHECKSIG, the wrong key signed', CS(X1), [Sig(K2)])
tap('CHECKSIG, a spoiled signature', CS(X1), [Sig(K1, bad=True)])
tap('CHECKSIG, an empty signature', CS(X1), [b''])
tap('CHECKSIG NOT, an empty signature', CS(X1) + P('NOT'), [b''])
tap('CHECKSIGVERIFY, an empty signature', P('0x20 0x' + X1.hex() + ' CHECKSIGVERIFY 1'), [b''])
tap('CHECKSIG, 65 bytes ending 0', CS(X1), [Sig(K1, tail=b'\x00')])
tap('CHECKSIG, a 1-byte signature', CS(X1), [b'\x01'])
tap('CHECKSIG, an empty key', P('0 CHECKSIG'), [b''])
tap('CHECKSIG, a 33-byte key, any signature', P('0x21 0x02' + '11' * 32 + ' CHECKSIG'), [b'\x01'])
tap('CHECKSIG, a 33-byte key, discouraged', P('0x21 0x02' + '11' * 32 + ' CHECKSIG'), [b'\x01'],
    flags=WT + ',DISCOURAGE_UPGRADABLE_PUBKEYTYPE')
tap('CHECKSIG, a 33-byte key, an empty signature', P('0x21 0x02' + '11' * 32 + ' CHECKSIG NOT'), [b''])
tap('CHECKSIG, a key off the curve', P('0x20 0x' + '00' * 32 + ' CHECKSIG'), [Sig(K1)])
tap('CODESEPARATOR, signed at its position', P('CODESEPARATOR') + CS(X1), [Sig(K1, codesep=0)])
tap('CODESEPARATOR, signed with none', P('CODESEPARATOR') + CS(X1), [Sig(K1)])
tap('CODESEPARATOR unexecuted, signed with none', P('0 IF CODESEPARATOR ENDIF') + CS(X1), [Sig(K1)])
tap('two CODESEPARATORs, the last at opcode 3', P('CODESEPARATOR 1 DROP CODESEPARATOR') + CS(X1), [Sig(K1, codesep=3)])
multi = P('0x20 0x' + X1.hex() + ' CHECKSIG 0x20 0x' + X2.hex() + ' 0xba 0x20 0x' + X3.hex() + ' 0xba 2 NUMEQUAL')
tap('CHECKSIGADD, 2 of 3, keys 1 and 3', multi, [Sig(K3), b'', Sig(K1)])
tap('CHECKSIGADD, 2 of 3, one signature', multi, [b'', b'', Sig(K1)])
tap('CHECKSIGADD, 2 of 3, a spoiled one', multi, [Sig(K3, bad=True), b'', Sig(K1)])
tap('CHECKSIGADD, a 5-byte number', P('0x05 0x0000000001 0x20 0x' + X1.hex() + ' 0xba'), [b''])
tap('CHECKSIGADD, a non-minimal number, MINIMALDATA', P('0x02 0x0100 0x20 0x' + X1.hex() + ' 0xba'), [b''],
    flags=WT + ',MINIMALDATA')
tap('CHECKSIGADD, a non-minimal number', P('0x02 0x0100 0x20 0x' + X1.hex() + ' 0xba 1 NUMEQUAL'), [b''])
tap('CHECKSIGADD, two arguments', P('0x20 0x' + X1.hex() + ' 0xba'), [])
case('CHECKSIGADD outside tapscript', '', wsh(P('0 0 0 0xba')), W, [P('0 0 0 0xba')])
tap('CHECKMULTISIG', P('0 0 0 CHECKMULTISIG'), [])
tap('CHECKMULTISIG unexecuted', P('0 IF CHECKMULTISIG ENDIF 1'), [])
tap('CHECKMULTISIGVERIFY', P('0 0 0 CHECKMULTISIGVERIFY 1'), [])
# The budget: each signature checked costs 50 of 50 + the witness size.
unknown = P('DUP 0x21 0x02' + '11' * 32 + ' CHECKSIGVERIFY')
for n, pad in [(6, 0), (7, 0), (7, 1), (7, 13), (7, 14), (8, 0), (8, 30)]:
    script = unknown * n + P('DROP 1')
    sig = b'\x01' * (1 + pad)
    tap('the budget: %d checks, a %d-byte signature' % (n, len(sig)), script, [sig], path=False)
# Tapscript: everything else.
for label, script, items in [('MINIMALIF, 0x02', P('IF 1 ENDIF 1'), [b'\x02']), ('MINIMALIF, 0x01', P('IF 1 ENDIF'), [b'\x01']),
                             ('MINIMALIF, empty', P('IF 0 ENDIF 1'), [b'']), ('MINIMALIF, 0x0100', P('NOTIF 0 ENDIF 1'), [b'\x01\x00']),
                             ('OP_SUCCESS 0x50 after RETURN', P('RETURN') + b'\x50', []),
                             ('OP_SUCCESS 0xbb', b'\xbb', []), ('OP_SUCCESS 0xfe unexecuted', P('0 IF') + b'\xfe' + P('ENDIF'), []),
                             ('OP_SUCCESS CAT', P('CAT'), []), ('OP_SUCCESS then a bad push', b'\x50\x4c\x05', []),
                             ('a bad push, no OP_SUCCESS', b'\x51\x4c\x05', []), ('0xff executed', P('1') + b'\xff', []),
                             ('VERIF', P('0 IF VERIF ENDIF 1'), []), ('EVAL_FALSE', P('0'), []), ('CLEANSTACK', P('1 1'), []),
                             ('300 NOPs', P('NOP') * 300 + P('1'), []), ('202 ops then CHECKSIG', P('NOP') * 202 + CS(X1), [Sig(K1)]),
                             ('a script of 10,001 bytes', P('0x4d 0x0802 0x' + '00' * 520 + ' DROP') * 19 + b'\x2a' + bytes(42) + P('DROP 1'), []),
                             ('1,000 stack elements', P('2DROP') * 500 + P('1'), [b''] * 1000),
                             ('1,001 stack elements', P('1'), [b''] * 1001),
                             ('an element of 521 bytes', P('DROP 1'), [bytes(521)]),
                             ('1,001 elements after an opcode', P('1'), [b''] * 1000)]:
    tap('tapscript, ' + label, script, items)
tap('tapscript, OP_SUCCESS 0x50, discouraged', b'\x50', [], flags=WT + ',DISCOURAGE_OP_SUCCESS')
# 2,000 SHA-256s: 64,000 bytes of results, twice the workspace's arena.
chain = b'\x07'
for _ in range(2000): chain = hashlib.sha256(chain).digest()
tap('tapscript, 2,000 SHA256s', P('SHA256') * 2000 + P('0x20 0x' + chain.hex() + ' EQUAL'), [b'\x07'])
tap('tapscript, 2,000 SHA256s, wrong', P('SHA256') * 2000 + P('0x20 0x' + bytes(32).hex() + ' EQUAL'), [b'\x07'])
hs = [b'\x07']
for _ in range(1100): hs.append(hashlib.sha256(hs[-1]).digest())
tap('tapscript, 900 SHA256s kept, then 200 more', P('DUP SHA256') * 900 + P('SHA256') * 200 + P('DEPTH 901 EQUALVERIFY 0x20 0x' + hs[1100].hex()
    + ' EQUALVERIFY 0x20 0x' + hs[899].hex() + ' EQUALVERIFY') + P('2DROP') * 449 + P('0x01 0x07 EQUAL'), [b'\x07'])

# Legacy sighash rows for the library: one per kind.
picks = []
for want in [lambda r, tx: r[3] & 0x1f == 1 and not r[3] & 0x80 and b'\xab' not in bytes.fromhex(r[1]),
             lambda r, tx: r[3] & 0x1f == 2, lambda r, tx: r[3] & 0x1f == 3 and r[2] < len(tx['outs']),
             lambda r, tx: r[3] & 0x80 and r[3] > 0,
             lambda r, tx: r[3] < 0, lambda r, tx: bytes.fromhex(r[1]).count(b'\xab') >= 2,
             lambda r, tx: r[3] & 0x1f not in (1, 2, 3),
             lambda r, tx: bytes.fromhex(r[1])[:1] == b'\xab',
             lambda r, tx: r[3] & 0x9f == 2 and r[2] > 0 and len(tx['ins']) >= 2,
             lambda r, tx: r[3] & 0x9f == 3 and 0 < r[2] < len(tx['outs']) and len(tx['ins']) >= 2]:
    for r in sighash:
        tx = T.unserialize(bytes.fromhex(r[0]), True)
        if want(r, tx) and len(r[0]) < 1200 and r not in picks:
            picks.append(r)
            break
assert len(picks) == 10

# BIP-143 hashes of sighash.json transactions with more inputs than
# outputs: each hash type at input 1, and SINGLE at the first input with
# no output.
picks_v0 = []
for r in sighash:
    tx = T.unserialize(bytes.fromhex(r[0]), True)
    nin, nout = len(tx['ins']), len(tx['outs'])
    if len(r[0]) < 1200 and nin <= 8 and nout <= 8 and nin > nout >= 2:
        code = bytes.fromhex(r[1])
        for index, hashtype in [(1, 1), (1, 2), (1, 3), (1, 0x81), (1, 0x82), (1, 0x83), (nout, 3), (nout, 0x83), (nout, 1)]:
            h = R.sighash_v0(code, tx, index, hashtype, 5000000000 + index)
            picks_v0.append((r[0], r[1], index, 5000000000 + index, hashtype, h.hex()))
        break
assert len(picks_v0) == 9


# ---- rendering
def render_rows(name, struct, items, fields):
    out = [f'{name} :: [{len(items)}] {struct}.[']
    for it in items:
        out.append(f'  {struct}.{{' + ', '.join(f(it) for f in fields) + '},')
    out.append('];')
    return out


def hexes(b):
    return '"' + b.hex() + '"'


main = ['// Generated by make_script_vectors.py from Bitcoin Core\'s script_tests.json,',
        '// tx_valid.json, tx_invalid.json and sighash.json, pinned in',
        '// coax/tests/vectors/upstream/bitcoin-core/ (see SOURCES.md there). Test',
        '// data only.',
        '//',
        '// SCRIPT_ROWS: each runnable row of script_tests.json, by line (its index',
        '// in the file\'s top-level array): the spending transaction script_tests.cpp',
        '// builds (version 1, one input spending output 0 of the crediting',
        '// transaction, the row\'s scriptSig and witness, one output of the amount',
        '// with an empty script), the scriptPubKey, the amount in satoshis, the',
        '// flags as run (CLEANSTACK adds P2SH and WITNESS, as DoTest does) and the',
        '// expected error, Core\'s number.',
        '// curve: how many signatures reached the curve arithmetic (Python decides).',
        'ScriptRow :: struct { line: int; tx: string; spk: string; amount: int; flags: int; error: int; curve: int; }']
main += render_rows('SCRIPT_ROWS', 'ScriptRow', run, [
    lambda r: f'line={r["line"]}', lambda r: 'tx=' + hexes(r['tx']), lambda r: 'spk=' + hexes(r['spk']),
    lambda r: f'amount={r["amount"]}', lambda r: f'flags={r["flags"]}', lambda r: f'error={R.E[r["expected"]]}', lambda r: f'curve={r["curve"]}'])
main += ['// The Taproot rows: their leaf script, control block and output key are',
         '// built as script_tests.cpp builds them (chapter 44).',
         'SCRIPT_TAPROOT_LINES :: [%d] int.[%s];' % (len(taproot_lines), ', '.join(map(str, taproot_lines)))]
main += ['// tx_valid.json (valid) and tx_invalid.json rows other than BADTX: the',
         '// transaction and, for input k, SCRIPT_PREVS[first + k], the scriptPubKey',
         '// and amount it spends (0 when the row gives none).',
         'ScriptTx :: struct { tx: string; valid: bool; first: int; count: int; }',
         'ScriptPrev :: struct { spk: string; amount: int; }']
main += render_rows('SCRIPT_TXS', 'ScriptTx', txs, [
    lambda t: 'tx=' + hexes(t['tx']), lambda t: 'valid=' + ('true' if t['valid'] else 'false'),
    lambda t: f'first={t["first"]}', lambda t: f'count={t["count"]}'])
main += render_rows('SCRIPT_PREVS', 'ScriptPrev', prevs, [lambda p: 'spk=' + hexes(p[0]), lambda p: f'amount={p[1]}'])
main += ['// Each flag set transaction_tests.cpp runs a row under, and the verdict:',
         '// the row\'s own (for tx_valid.json, every flag but the listed ones),',
         '// each flag removed from a valid row or added to an invalid one, and',
         '// each listed flag the other way, which must flip the verdict.',
         'ScriptTxCase :: struct { tx: int; flags: int; valid: bool; }']
main += render_rows('SCRIPT_TX_CASES', 'ScriptTxCase', cases, [
    lambda c: f'tx={c[0]}', lambda c: f'flags={c[1]}', lambda c: 'valid=' + ('true' if c[2] else 'false')])
main += ['// sighash.json: transaction, scriptCode, input, hash type, and the legacy',
         '// signature hash as Core displays it (reversed).',
         'ScriptSighash :: struct { tx: string; code: string; index: int; hashtype: int; hash: string; }']
main += render_rows('SCRIPT_SIGHASH', 'ScriptSighash', sighash, [
    lambda s: f'tx="{s[0]}"', lambda s: f'code="{s[1]}"', lambda s: f'index={s[2]}', lambda s: f'hashtype={s[3]}', lambda s: f'hash="{s[4]}"'])

small = ['// Generated by make_script_vectors.py for the checks of',
         '// coax/lib/bitcoin/script.coax. Test data only.',
         '//',
         '// SI_CASES: a script pair and flags, run in a transaction built as',
         '// script_tests.cpp builds its own (some with another version, locktime or',
         '// sequence), with the error script_reference.py decides. SI_SIGNED: three',
         '// spends signed by make_script_vectors.py with test keys, which pass.',
         '// SI_TAPROOT: the Taproot spends that reach the curve, a Schnorr',
         '// verification or the commitment check (chapter 44), signed with test',
         '// keys, each with its decided error.',
         '// SI_SIGHASH: ten rows of Core\'s sighash.json, one per kind of hash type',
         '// and position.',
         'SiCase :: struct { label: string; tx: string; spk: string; amount: int; flags: int; error: int; }']
for name, items in (('SI_CASES', lib), ('SI_SIGNED', signed), ('SI_TAPROOT', schnorr)):
    small += render_rows(name, 'SiCase', items, [
        lambda c: f'label="{text(c["label"])}"', lambda c: 'tx=' + hexes(c['tx']), lambda c: 'spk=' + hexes(c['spk']),
        lambda c: f'amount={c["amount"]}', lambda c: f'flags={c["flags"]}', lambda c: f'error={c["error"]}'])
small += ['SiSighash :: struct { tx: string; code: string; index: int; hashtype: int; hash: string; }']
small += render_rows('SI_SIGHASH', 'SiSighash', picks, [
    lambda s: f'tx="{s[0]}"', lambda s: f'code="{s[1]}"', lambda s: f'index={s[2]}', lambda s: f'hashtype={s[3]}', lambda s: f'hash="{s[4]}"'])
small += ['// SI_SIGHASH_V0: BIP-143 hashes script_reference.py computes for a',
          '// sighash.json transaction, its scriptCode and an amount.',
          'SiSighashV0 :: struct { tx: string; code: string; index: int; amount: int; hashtype: int; hash: string; }']
small += render_rows('SI_SIGHASH_V0', 'SiSighashV0', picks_v0, [
    lambda s: f'tx="{s[0]}"', lambda s: f'code="{s[1]}"', lambda s: f'index={s[2]}', lambda s: f'amount={s[3]}',
    lambda s: f'hashtype={s[4]}', lambda s: f'hash="{s[5]}"'])

(HERE/'script_vectors.coax').write_text('\n'.join(main) + '\n')
(HERE/'script_lib_vectors.coax').write_text('\n'.join(small) + '\n')
counts = {}
for c in lib: counts[c['error']] = counts.get(c['error'], 0) + 1
print(f'{len(run)} script rows, {len(taproot_lines)} of them Taproot, {len(txs)} transactions, {len(cases)} flag cases, '
      f'{len(sighash)} sighash rows; {len(lib)} library cases, {len(schnorr)} Taproot cases, errors {sorted(counts.items())}')
