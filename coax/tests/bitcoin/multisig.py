#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Python transliterations of Bitcoin Core's solver, signer and interpreter decide each case outside Coax)
"""The cross-check of Bitcoin chapter 100. It writes the transcript the
example must print from Python: BIP-67's vectors through Python's sort,
script and Base58Check, Core's MatchMultisig and MatchMultiA as
make_multisig_vectors.py transliterates them, the four kinds of 2-of-3
spent through psbt_reference.py's roles and checked by
script_reference.py's interpreter, and the multi_a leaf signed and
checked with Python's BIP-340 and BIP-342 signature hash:

  python3 -I coax/tests/bitcoin/multisig.py                       # compare with the fixture
  python3 -I coax/tests/bitcoin/multisig.py --coax coax/bin/coax  # also run the example
"""
import copy, hashlib, re, subprocess, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import make_multisig_vectors as V  # noqa: E402
import psbt_reference as R  # noqa: E402
import tx_reference as T  # noqa: E402
import script_reference as S  # noqa: E402
import der_reference as D  # noqa: E402

REPO = HERE.parents[2]
FIXTURE = REPO / 'bitcoin/fixtures/multisig.out'
EXAMPLE = 'bitcoin/examples/multisig.coax'
LIB = REPO / 'coax/lib/bitcoin/multisig.coax'
KINDS = ['bare', 'P2SH', 'P2WSH', 'P2SH-P2WSH']
H = bytes.fromhex('50929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0')
CHARSET = 'qpzry9x8gf2tvdw0s3jn54khce6mua7l'


def bech32(hrp, version, program):
    """BIP-173 (version 0) or BIP-350 (version 1 and up) address."""
    def polymod(values):
        g = [0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3]
        c = 1
        for v in values:
            b = c >> 25
            c = (c & 0x1ffffff) << 5 ^ v
            for i in range(5): c ^= g[i] if (b >> i) & 1 else 0
        return c
    acc = bits = 0
    data = [version]
    for b in program:
        acc = (acc << 8) | b; bits += 8
        while bits >= 5: bits -= 5; data.append((acc >> bits) & 31)
    if bits: data.append((acc << (5 - bits)) & 31)
    const = 1 if version == 0 else 0x2bc830a3
    e = [ord(x) >> 5 for x in hrp] + [0] + [ord(x) & 31 for x in hrp]
    mod = polymod(e + data + [0] * 6) ^ const
    return hrp + '1' + ''.join(CHARSET[d] for d in data + [(mod >> 5 * (5 - i)) & 31 for i in range(6)])


def address(spk):
    if len(spk) == 23 and spk[:2] == b'\xa9\x14': return V.b58encode(b'\x05' + spk[2:22])
    if len(spk) in (22, 34) and spk[0] in (0, 0x51): return bech32('bc', 0 if spk[0] == 0 else 1, spk[2:])
    return 'no address'


def hash160(b):
    return hashlib.new('ripemd160', hashlib.sha256(b).digest()).digest()


def outputs(script):
    p2wsh = b'\x00\x20' + hashlib.sha256(script).digest()
    return [script, b'\xa9\x14' + hash160(script) + b'\x87', p2wsh, b'\xa9\x14' + hash160(p2wsh) + b'\x87'], p2wsh


def check_lib():
    src = LIB.read_text()
    assert re.search(r'^MS_MAX_KEYS :: 20;', src, re.M) and re.search(r'^MS_A_MAX_KEYS :: 999;', src, re.M)
    core = V.pinned('bitcoin-core/solver.cpp')
    assert 'GetScriptNumber(opcode, data, 1, MAX_PUBKEYS_PER_MULTISIG)' in core
    assert re.search(r'MS_KINDS :: \[4\] string\.\["bare", "P2SH", "P2WSH", "P2SH-P2WSH"\];', src)


def fund(fill, spks, amount):
    return dict(version=(2).to_bytes(4, 'little'),
                ins=[dict(prevout=bytes([fill]) * 32 + bytes(4), script=b'', sequence=b'\xff' * 4, witness=[])],
                outs=[dict(value=amount.to_bytes(8, 'little'), script=s) for s in spks], locktime=bytes(4), segwit=False)


def spend(prev, count, amount, to):
    return dict(version=(2).to_bytes(4, 'little'),
                ins=[dict(prevout=T.txid(prev) + i.to_bytes(4, 'little'), script=b'', sequence=b'\xff' * 4, witness=[]) for i in range(count)],
                outs=[dict(value=amount.to_bytes(8, 'little'), script=to)], locktime=bytes(4), segwit=False)


def sighash_leaf(tx, spent, leaf):
    """BIP-342's script-path signature hash, SIGHASH_DEFAULT, one input."""
    sha = lambda b: hashlib.sha256(b).digest()
    ser = lambda o: o['value'] + T.compact(len(o['script'])) + o['script']
    m = bytes([0, 0]) + tx['version'] + tx['locktime']
    m += sha(b''.join(i['prevout'] for i in tx['ins']))
    m += sha(b''.join(a.to_bytes(8, 'little') for a, _ in spent))
    m += sha(b''.join(T.compact(len(s)) + s for _, s in spent))
    m += sha(b''.join(i['sequence'] for i in tx['ins']))
    m += sha(b''.join(ser(o) for o in tx['outs']))
    m += bytes([2]) + (0).to_bytes(4, 'little')
    m += V.leaf_hash(leaf) + b'\x00' + b'\xff' * 4
    return V.tagged('TapSighash', m)


def check_multi_a(leaf, items, h):
    """The leaf as BIP-342 runs it: key i's opcode takes item n-1-i."""
    x = V.match_multi_a(leaf)
    if not x or len(items) != x[1]: return False
    count = 0
    for i in range(x[1]):
        sig = items[x[1] - 1 - i]
        if not sig: continue
        if len(sig) != 64 or not R.schnorr_verify(leaf[34 * i + 1:34 * i + 33], h, sig): return False
        count += 1
    return count == x[0]


def transcript():
    lines = []
    p = lines.append
    p("BIP-67's vectors, 2 of n with the keys sorted:")
    for i, (keys, script, addr) in enumerate(V.bip67()):
        keys = bytes.fromhex(keys)
        ks = [keys[k:k + 33] for k in range(0, len(keys), 33)]
        s = V.multisig(2, ks, True)
        p(f"  vector {i + 1}: 2 of {len(ks)}, script {'yes' if s.hex() == script else 'no'}, {address(outputs(s)[0][1])}")
    p("Core's MatchMultisig and MatchMultiA:")
    for name, s, _, m, n in V.match_cases():
        p(f'  {name}: {m} of {n}' if m else f'  {name}: not multisig')
    secrets = [int(('%02x' % (0x11 * (i + 1))) * 32, 16) for i in range(3)]
    pubs = [R.pubkey(k) for k in secrets]
    script = V.multisig(2, pubs, True)
    spks, p2wsh = outputs(script)
    p(f'One 2-of-3 (sortedmulti), {len(script)} bytes, locked four ways:')
    for kind in range(4): p(f'  {KINDS[kind]}: {spks[kind].hex()}, {address(spks[kind])}')
    prev = fund(1, spks, 100000)
    to = b'\x00\x14' + hash160(pubs[0])
    m = R.create(T.serialize(spend(prev, 4, 399000, to), False))
    for i in range(4):
        if i < 2: R.put(m['ins'][i], b'\x00', T.serialize(prev, False))
        else: R.put(m['ins'][i], b'\x01', (100000).to_bytes(8, 'little') + T.compact(len(spks[i])) + spks[i])
    R.put(m['ins'][1], b'\x04', script)
    R.put(m['ins'][2], b'\x05', script)
    R.put(m['ins'][3], b'\x04', p2wsh); R.put(m['ins'][3], b'\x05', script)
    p('Spending all four through one PSBT:')
    for i in range(4): p(f'  input {i} ({KINDS[i]}), key A: {R.SIGN_REASONS[R.sign(m, i, secrets[0])]}')
    one = copy.deepcopy(m)
    p(f"  with A's signatures only, input 0 is final: {'yes' if R.finalize_input(one, 0) else 'no'}")
    for i in range(4): p(f'  input {i} ({KINDS[i]}), key C: {R.SIGN_REASONS[R.sign(m, i, secrets[2])]}')
    p(f"  Input Finalizer: final {'yes' if R.finalize(m) else 'no'}")
    net = R.extract(m)
    t = T.unserialize(net, True)
    p(f'  Transaction Extractor: {len(net)} bytes, weight {T.weight(t)}, txid {T.txid(t)[::-1].hex()}')
    flags = sum(S.F[f] for f in ('P2SH', 'WITNESS', 'DERSIG', 'LOW_S', 'STRICTENC', 'NULLDUMMY', 'CLEANSTACK', 'NULLFAIL', 'WITNESS_PUBKEYTYPE'))
    for i in range(4):
        e = S.verify_script(t['ins'][i]['script'], spks[i], t['ins'][i]['witness'], flags, S.Checker(t, i, 100000))
        p(f"  input {i} ({KINDS[i]}): scriptSig {len(t['ins'][i]['script'])} bytes, {len(t['ins'][i]['witness'])} witness items, {S.ERRORS[e]}")
    # multi_a
    p("The same 2-of-3 as a multi_a leaf under BIP-341's key H:")
    xonly = [k[1:] for k in pubs]
    leaf = V.multi_a(2, xonly, True)
    order = sorted(range(3), key=lambda i: xonly[i])
    lh = V.leaf_hash(leaf)
    Q = D.add(R.lift_x(int.from_bytes(H, 'big')), D.mul(int.from_bytes(V.tagged('TapTweak', H + lh), 'big'), D.G))
    q = Q[0].to_bytes(32, 'big')
    spk = b'\x51\x20' + q
    control = bytes([0xc0 | (Q[1] & 1)]) + H
    p(f'  leaf, {len(leaf)} bytes: {leaf.hex()}')
    p(f'  output: {address(spk)}')
    prev = fund(2, [spk], 100000)
    tx = spend(prev, 1, 99000, to)
    h = sighash_leaf(tx, [(100000, spk)], leaf)
    items = [b''] * 3
    for i in range(3):
        who = order[i]
        if who != 1: items[2 - i] = R.schnorr(secrets[who], h, bytes(32))
        p(f"  key {'ABC'[who]} at leaf position {i}: {'no signature' if who == 1 else 'signs'}")
    # BIP-341's commitment, recomputed from the control block.
    P = R.lift_x(int.from_bytes(control[1:33], 'big'))
    Q2 = D.add(P, D.mul(int.from_bytes(V.tagged('TapTweak', control[1:33] + V.leaf_hash(leaf)), 'big'), D.G))
    p(f"  the control block commits to the output: {'yes' if Q2[0].to_bytes(32, 'big') == q and Q2[1] & 1 == control[0] & 1 else 'no'}")
    p(f"  A's and C's signatures satisfy the leaf: {'yes' if check_multi_a(leaf, items, h) else 'no'}")
    tx['ins'][0]['witness'] = items + [leaf, control]
    raw = T.serialize(tx, True)
    t = T.unserialize(raw, True)
    p(f'  the spend: {len(raw)} bytes, weight {T.weight(t)}, txid {T.txid(t)[::-1].hex()}')
    one = list(items)
    for i in range(3):
        if order[i] == 0: one[2 - i] = b''
    p(f"  one signature satisfies it: {'yes' if check_multi_a(leaf, one, h) else 'no'}")
    swapped = [items[2], items[1], items[0]]
    p(f"  the signatures swapped satisfy it: {'yes' if check_multi_a(leaf, swapped, h) else 'no'}")
    return '\n'.join(lines) + '\n'


def main():
    check_lib()
    want = transcript()
    if '--write' in sys.argv:
        FIXTURE.write_text(want)
    assert FIXTURE.read_text() == want, 'the fixture differs from the Python transcript'
    print('multisig: the fixture is the Python transcript')
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            run = subprocess.run([coax, 'run', EXAMPLE, '--run-steps', '250000000'] + mode, cwd=REPO, capture_output=True, text=True)
            assert run.returncode == 0 and run.stdout == want, (mode, run.stderr[-2000:])
            print(f'native {"release" if mode else "debug"}: agrees')


if __name__ == '__main__':
    main()
