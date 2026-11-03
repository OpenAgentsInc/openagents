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
    built = []
    for desc, address, message, signature in constructed():
        state, reason, lt, seq, fmt = R.verify(address, signature, message)
        built.append(dict(source='constructed', type=desc, address=address, message=message, keys='', script='',
                          version=0, locktime=0, sequence=0, signature=signature, ours='', state=STATE[state],
                          reason=reason, format=FORMAT[fmt], t=lt, s=seq))
    return hashes, cases, errors, built


def render(hashes, cases, errors, built):
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
    for name, rows in (('B322_CASES', cases), ('B322_BUILT', built)):
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
    hashes, cases, errors, built = build()
    text = render(hashes, cases, errors, built)
    if '--write' in sys.argv: OUT.write_text(text)
    assert OUT.read_text() == text, 'bip322_vectors.coax is not current: run with --write'
    states = [sum(1 for c in built if c['state'] == k) for k in range(3)]
    print(f'bip322_vectors.coax matches: {len(hashes)} hashes, {len(cases)} signatures, {len(errors)} errors, '
          f'{len(built)} constructed ({states[0]} valid, {states[1]} invalid, {states[2]} inconclusive)')


if __name__ == '__main__':
    main()
