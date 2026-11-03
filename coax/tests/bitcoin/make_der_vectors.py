#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: copies published vectors outside Coax)
"""Writes coax/tests/bitcoin/der_vectors.coax for Bitcoin chapter 21 from two
pinned files in coax/tests/vectors/upstream/:

- Bitcoin Core's src/test/data/script_tests.json (bitcoin-core/), and
- bitcoinjs/bip66's test/fixtures.json (bip66/).

  python3 -I coax/tests/bitcoin/make_der_vectors.py

Each file's SHA-256 must equal its pin below, so a changed file refuses
instead of silently changing the checks.

The script_tests.json rows it takes are the ones whose scriptSig is only
pushes and whose scriptPubKey is pushes followed by CHECKSIG or
CHECKMULTISIG, optionally followed by NOT, with no witness. Rows are grouped
by their scripts; a group is taken when one of its rows sets DERSIG or
LOW_S or expects SIG_DER, SIG_HIGH_S or SIG_HASHTYPE. That is the set of
rows BIP-66 and its policy companions decide, together with the same
scripts under other flags.

The table copies each row's stack items, flags and expected result. One
value is computed here, not copied: the legacy signature hash each
signature signs, from the transactions Bitcoin Core's tests build
(BuildCreditingTransaction and BuildSpendingTransaction). Chapter 37 builds
that hash in Coax; until then this script supplies it, and der.py
recomputes it."""
import hashlib, json, re
from pathlib import Path

UPSTREAM = Path(__file__).resolve().parents[1] / 'vectors/upstream'
SCRIPT_TESTS = UPSTREAM / 'bitcoin-core/script_tests.json'
SCRIPT_TESTS_PIN = 'ccfadc13f9b06888faf57a4a2f86dd8feff35c80aa710cbf3eb421f326b8d8b7'
BIP66 = UPSTREAM / 'bip66/fixtures.json'
BIP66_PIN = '69b1504212b2596fdc62669e6193aebbb42c6e82ceec9f99ced205d557b7e83d'

OPS = {'CHECKSIG': 0xac, 'CHECKMULTISIG': 0xae, 'NOT': 0x91}
FLAGS = {'DERSIG': 1, 'LOW_S': 2, 'STRICTENC': 4, 'NULLFAIL': 8, 'NULLDUMMY': 16, 'P2SH': 32}
ENCODING = ('SIG_DER', 'SIG_HIGH_S', 'SIG_HASHTYPE')


def pinned(path, pin):
    data = path.read_bytes()
    assert hashlib.sha256(data).hexdigest() == pin, path
    return json.loads(data)


def sha256d(b):
    return hashlib.sha256(hashlib.sha256(b).digest()).digest()


def number(n):
    """CScriptNum serialization: little-endian, sign in the top bit."""
    out, v = bytearray(), abs(n)
    while v:
        out.append(v & 0xff); v >>= 8
    if out[-1] & 0x80: out.append(0x80 if n < 0 else 0)
    elif n < 0: out[-1] |= 0x80
    return bytes(out)


def push(data):
    """The script push of data, as CScript() << data writes it."""
    n = len(data)
    if n < 0x4c: return bytes([n]) + data
    if n <= 0xff: return bytes([0x4c, n]) + data
    return bytes([0x4d]) + n.to_bytes(2, 'little') + data


def assemble(text):
    """Bitcoin Core's ParseScript for the tokens these rows use."""
    out = b''
    for w in text.split():
        if re.fullmatch(r'-?\d+', w):
            n = int(w)
            out += bytes([0x50 + n]) if n == -1 or 1 <= n <= 16 else b'\0' if n == 0 else push(number(n))
        elif w.startswith('0x'):
            out += bytes.fromhex(w[2:])
        elif w in OPS:
            out += bytes([OPS[w]])
        else:
            return None   # an opcode no taken row uses
    return out


def items(script):
    """The stack items a run of pushes leaves, and the opcodes after them."""
    stack, ops, i = [], [], 0
    while i < len(script):
        op = script[i]; i += 1
        if ops or op > 0x60 or op == 0x50:
            ops.append(op); continue
        if op <= 0x4b: n = op
        elif op == 0x4c: n = script[i]; i += 1
        elif op == 0x4d: n = int.from_bytes(script[i:i+2], 'little'); i += 2
        elif op == 0x4e: n = int.from_bytes(script[i:i+4], 'little'); i += 4
        elif op == 0x4f: stack.append(b'\x81'); continue
        else: stack.append(bytes([op - 0x50])); continue
        assert i + n <= len(script), 'push past the end'
        stack.append(script[i:i+n]); i += n
    return stack, ops


def tx(inputs, outputs):
    """Version 1, legacy serialization, locktime 0."""
    out = (1).to_bytes(4, 'little') + bytes([len(inputs)])
    for prev, script, sequence in inputs:
        out += prev + bytes([len(script)]) + script + sequence.to_bytes(4, 'little')
    out += bytes([len(outputs)])
    for value, script in outputs:
        out += value.to_bytes(8, 'little') + bytes([len(script)]) + script
    return out + bytes(4)


def legacy_sighash(script_sig, script_pubkey, signature):
    """SignatureHash for input 0 of Core's spending transaction (SigVersion BASE)."""
    credit = tx([(bytes(32) + b'\xff\xff\xff\xff', b'\0\0', 0xffffffff)], [(0, script_pubkey)])
    prev = sha256d(credit) + bytes(4)
    code = script_pubkey
    # FindAndDelete of the signature and OP_CODESEPARATOR: neither occurs in these rows.
    assert push(signature) not in code and 0xab not in items(code)[1]
    hashtype = signature[-1]
    base, anyone = hashtype & 0x1f, hashtype & 0x80
    assert len(script_sig) < 0xfd and len(code) < 0xfd
    outputs = [] if base == 2 else [(0, b'')]   # NONE has none; SINGLE keeps output 0
    message = tx([(prev, code, 0xffffffff)], outputs) + hashtype.to_bytes(4, 'little')
    del anyone   # one input either way
    return sha256d(message)


def rows():
    doc = pinned(SCRIPT_TESTS, SCRIPT_TESTS_PIN)
    shaped = []
    for index, row in enumerate(doc):
        if not isinstance(row, list) or len(row) < 4 or isinstance(row[0], list):
            continue
        sig_asm, pk_asm, flags, expected = row[:4]
        comment = row[4] if len(row) > 4 else ''
        if assemble(sig_asm) is None or assemble(pk_asm) is None:
            continue
        sig_items, sig_ops = items(assemble(sig_asm))
        pk_items, pk_ops = items(assemble(pk_asm))
        if sig_ops or pk_ops not in ([0xac], [0xae], [0xac, 0x91], [0xae, 0x91]):
            continue
        shaped.append((index, sig_asm, pk_asm, flags, expected, comment, sig_items, pk_items, pk_ops))
    chosen = set()
    for r in shaped:
        if r[4] in ENCODING or {'DERSIG', 'LOW_S'} & set(r[3].split(',')):
            chosen.add((r[1], r[2]))
    out = []
    for index, sig_asm, pk_asm, flags, expected, comment, sig_items, pk_items, pk_ops in shaped:
        if (sig_asm, pk_asm) not in chosen:
            continue
        mask = 0
        for f in filter(None, flags.split(',')):
            mask |= FLAGS[f]
        script_pubkey = assemble(pk_asm)
        assert not (len(script_pubkey) == 23 and script_pubkey[0] == 0xa9), 'P2SH output'
        stack = sig_items + pk_items
        # Only scriptSig items are signatures; the scriptPubKey pushes keys and counts.
        hashes = [legacy_sighash(assemble(sig_asm), script_pubkey, item).hex() if item else '' for item in sig_items]
        hashes += [''] * len(pk_items)
        assert '"' not in comment and '\\' not in comment
        out.append(dict(line=index, flags=mask, expected=expected, multisig=pk_ops[0] == 0xae,
                        negate=len(pk_ops) == 2, stack=[i.hex() for i in stack], hashes=hashes,
                        comment=comment))
    return out


def bip66():
    doc = pinned(BIP66, BIP66_PIN)
    valid = [(v['DER'], v['r'], v['s']) for v in doc['valid']]
    invalid = [(v['DER'], v['exception'].strip('^$').replace('\\(', '(').replace('\\)', ')'))
               for v in doc['invalid']['decode']]
    assert len(valid) == 9 and len(invalid) == 15
    return valid, invalid


def render(table, valid, invalid):
    out = ['// Generated by make_der_vectors.py from Bitcoin Core\'s script_tests.json',
           '// (coax/tests/vectors/upstream/bitcoin-core/) and bitcoinjs/bip66\'s',
           '// test/fixtures.json (coax/tests/vectors/upstream/bip66/); see SOURCES.md.',
           '// Test data only. Every value is copied except each hash: the legacy',
           '// signature hash of Core\'s test transaction for that stack item.',
           '// stack and hashes list the items bottom first, separated by commas;',
           '// an empty item is empty. flags: 1 DERSIG, 2 LOW_S, 4 STRICTENC,',
           '// 8 NULLFAIL, 16 NULLDUMMY, 32 P2SH. row indexes the file\'s top-level array.',
           'DvRow :: struct {row:int;flags:int;expected:string;multisig:bool;negate:bool;stack:string;hashes:string;comment:string;}',
           'DV_ROWS :: [%d] DvRow.[\n%s];' % (len(table), ',\n'.join(
               '  DvRow.{row=%d,flags=%d,expected="%s",multisig=%s,negate=%s,stack="%s",hashes="%s",comment="%s"}'
               % (r['line'], r['flags'], r['expected'], 'true' if r['multisig'] else 'false',
                  'true' if r['negate'] else 'false', ','.join(r['stack']), ','.join(r['hashes']), r['comment'])
               for r in table)),
           '// bip66 fixtures: valid DER (no sighash byte) with its r and s, and the',
           '// decode cases its reader refuses, with the reason it gives.',
           'DvValid :: struct {der:string;r:string;s:string;}',
           'DV_VALID :: [%d] DvValid.[\n%s];' % (len(valid), ',\n'.join(
               '  DvValid.{der="%s",r="%s",s="%s"}' % v for v in valid)),
           'DvInvalid :: struct {der:string;reason:string;}',
           'DV_INVALID :: [%d] DvInvalid.[\n%s];' % (len(invalid), ',\n'.join(
               '  DvInvalid.{der="%s",reason="%s"}' % v for v in invalid))]
    return '\n'.join(out) + '\n'


if __name__ == '__main__':
    Path(__file__).with_name('der_vectors.coax').write_text(render(rows(), *bip66()))
