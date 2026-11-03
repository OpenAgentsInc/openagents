#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Python transliterations of Bitcoin Core's PSBT code decide each published case and role outside Coax)
"""The cross-check of Bitcoin chapter 94. It checks that
coax/lib/bitcoin/psbt.coax spells Core's refusal messages as
psbt_reference.py does, in the same order, then writes the transcript
the example must print from psbt_reference.py (Core's reader, Updater,
Signer, Combiner, Finalizer and Extractor in Python), a BIP-32
derivation of BIP-174's keys, and script_reference.py's interpreter and
BIP-340 verification for our own spend:

  python3 -I coax/tests/bitcoin/psbt.py                       # compare with the fixture
  python3 -I coax/tests/bitcoin/psbt.py --coax coax/bin/coax  # also run the example
"""
import copy, hashlib, hmac, re, subprocess, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import psbt_cases as PC  # noqa: E402
import psbt_reference as R  # noqa: E402
import tx_reference as T  # noqa: E402
import script_reference as S  # noqa: E402
import der_reference as D  # noqa: E402

REPO = HERE.parents[2]
FIXTURE = REPO / 'bitcoin/fixtures/psbt.out'
EXAMPLE = 'bitcoin/examples/psbt.coax'
LIB = REPO / 'coax/lib/bitcoin/psbt.coax'
B58 = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'
KINDS = {'invalid': 'invalid', 'valid': 'valid', 'signer': "valid, fails the signer's checks", 'locktime': 'lock-time case'}


def check_lib():
    src = LIB.read_text()
    reasons = re.search(r'PSBT_REASONS :: \[(\d+)\] string\.\[\n(.*?)\n\];', src, re.S)
    assert int(reasons.group(1)) == len(R.REASONS)
    got = [x.strip().rstrip(',') for x in reasons.group(2).split('\n')]
    assert got == ['"' + r + '"' for r in R.REASONS], 'PSBT_REASONS differs from psbt_reference.py'
    for name, k in R.C.items():
        assert re.search(rf'^PSBT_{name} :: {k};', src, re.M), name
    signs = re.search(r'PSBT_SIGN_REASONS :: \[10\] string\.\[(.*?)\];', src, re.S).group(1)
    assert re.findall(r'"([^"]*)"', signs) == R.SIGN_REASONS + ['capacity']


def unb58(s):
    n = 0
    for c in s: n = n * 58 + B58.index(c)
    b = n.to_bytes((len(s) * 733) // 1000 + 1, 'big').lstrip(b'\0')
    assert hashlib.sha256(hashlib.sha256(b[:-4]).digest()).digest()[:4] == b[-4:]
    return b[:-4]


def wif(s):
    b = unb58(s)
    assert b[0] == 0xef and b[33] == 1
    return int.from_bytes(b[1:33], 'big')


def derive(tprv, path):
    """BIP-32's hardened private derivation from an extended private key."""
    b = unb58(tprv)
    chain, k = b[13:45], int.from_bytes(b[46:78], 'big')
    for i in path:
        i |= 0x80000000
        h = hmac.new(chain, b'\0' + k.to_bytes(32, 'big') + i.to_bytes(4, 'big'), hashlib.sha512).digest()
        k, chain = (int.from_bytes(h[:32], 'big') + k) % D.N, h[32:]
    return k


def path_of(p):
    return [int(x[:-1]) for x in p.split('/')[1:]]


def yes(b):
    return 'yes' if b else 'no'


def plural(n, one, many):
    return one if n == 1 else many


def bip174(lines):
    w = PC.bip174_workflow()
    res = [bytes.fromhex(h) for h in w['results']]
    p = lines.append
    p(f"BIP-174's worked example, from its master key {w['tprv']}:")
    b = unb58(w['tprv'])
    fp = R.hash160(R.pubkey(int.from_bytes(b[46:78], 'big')))[:4]
    ok = all(R.pubkey(derive(w['tprv'], path_of(q))).hex() == k for k, q in w['paths'])
    p(f"  the master key's fingerprint: {fp.hex()}; its six public keys at their paths are the published ones: {yes(ok)}")
    signers = [(wif(k), q) for s in w['signers'] for k, q in s]
    p(f"  the signers' keys at their paths are their WIFs': {yes(all(derive(w['tprv'], path_of(q)) == k for k, q in signers))}")
    outs = [(bytes.fromhex(s), round(float(a) * 1e8)) for s, a in w['outputs']]
    tx = dict(version=(2).to_bytes(4, 'little'),
              ins=[dict(prevout=bytes.fromhex(t)[::-1] + i.to_bytes(4, 'little'), script=b'', sequence=b'\xff' * 4, witness=[]) for t, i in w['inputs']],
              outs=[dict(value=a.to_bytes(8, 'little'), script=s) for s, a in outs], locktime=bytes(4), segwit=False)
    m = R.create(T.serialize(tx, False))
    p(f'  Creator: {yes(R.write(m) == res[0])}')
    origin = lambda q: fp + b''.join((x | 0x80000000).to_bytes(4, 'little') for x in path_of(q))
    paths = {bytes.fromhex(k): origin(q) for k, q in w['paths']}
    prevs = [T.unserialize(bytes.fromhex(h), True) for h in w['prev']]
    redeems = [bytes.fromhex(h) for h in w['redeem']]
    wss = [bytes.fromhex(h) for h in w['witness']]
    for n, i in enumerate(tx['ins']):
        prev = next(x for x in prevs if T.txid(x) == i['prevout'][:32])
        o = prev['outs'][int.from_bytes(i['prevout'][32:], 'little')]
        pairs = m['ins'][n]
        spk = script = o['script']
        for r in redeems:
            if R.hash160(r) == spk[2:22]: script = r; R.put(pairs, b'\x04', r)
        if R.is_p2wsh(script) or R.is_p2wpkh(script): R.put(pairs, b'\x01', o['value'] + T.compact(len(spk)) + spk)
        else: R.put(pairs, b'\x00', T.serialize(prev, True))
        if R.is_p2wsh(script):
            for x in wss:
                if hashlib.sha256(x).digest() == script[2:]: script = x; R.put(pairs, b'\x05', x)
        for k, v in paths.items():
            if bytes([33]) + k in script: R.put(pairs, b'\x06' + k, v)
    for n, o in enumerate(tx['outs']):
        for k, v in paths.items():
            if R.is_p2wpkh(o['script']) and R.hash160(k) == o['script'][2:]: R.put(m['outs'][n], b'\x02' + k, v)
    p(f'  Updater: {yes(R.write(m) == res[1])}')
    for pairs in m['ins']: R.put(pairs, b'\x03', (1).to_bytes(4, 'little'))
    p(f'  SIGHASH_ALL on every input: {yes(R.write(m) == res[2])}')
    signed = []
    for s in range(2):
        mm = copy.deepcopy(m)
        for k in range(2):
            key, q = signers[2 * s + k]
            for n in range(len(mm['ins'])):
                p(f"  Signer {s + 1}, key m/0'/0'/{path_of(q)[2]}', input {n}: {R.SIGN_REASONS[R.sign(mm, n, key, grind=False)]}")
        p(f'  Signer {s + 1}: {yes(R.write(mm) == res[3 + s])}')
        signed.append(mm)
    both = R.combine(signed[0], signed[1])
    p(f'  Combiner, in either order: {yes(R.write(both) == res[5] and R.write(R.combine(signed[1], signed[0])) == res[5])}')
    final = R.finalize(both)
    p(f'  Input Finalizer: final {yes(final)}, {yes(R.write(both) == res[6])}')
    net = R.extract(both)
    p(f'  Transaction Extractor: {yes(net == res[7])}, {len(net)} bytes')
    p(f'  Combiner, keys only one side has: {yes(R.write(R.combine(R.maps(res[8]), R.maps(res[9]))) == res[10])}')
    p(f'  Combiner, a key both sides have: {yes(R.write(R.combine(R.maps(res[11]), R.maps(res[12]))) == res[13])}')
    return signers[0]


def own(lines):
    p = lines.append
    p('Our own spend of P2WPKH, P2WSH 2-of-2 and P2TR coins:')
    keys = [int('11' * 32, 16), int('22' * 32, 16), int('33' * 32, 16)]
    pubs = [R.pubkey(k) for k in keys]
    amounts = [50000, 60000, 70000]
    ws = b'\x52\x21' + pubs[0] + b'\x21' + pubs[1] + b'\x52\xae'
    _, q = R.taproot_tweak(keys[2], b'')
    spks = [b'\x00\x14' + R.hash160(pubs[0]), b'\x00\x20' + hashlib.sha256(ws).digest(), b'\x51\x20' + q]
    tx = dict(version=(2).to_bytes(4, 'little'),
              ins=[dict(prevout=bytes([i + 1]) * 32 + bytes(4), script=b'', sequence=b'\xff' * 4, witness=[]) for i in range(3)],
              outs=[dict(value=(179000).to_bytes(8, 'little'), script=b'\x00\x14' + R.hash160(pubs[1]))], locktime=bytes(4), segwit=False)
    unsigned = T.serialize(tx, False)
    m = R.create(unsigned)
    for i in range(3): R.put(m['ins'][i], b'\x01', amounts[i].to_bytes(8, 'little') + T.compact(len(spks[i])) + spks[i])
    R.put(m['ins'][1], b'\x05', ws)
    p(f'  Creator and Updater: {len(R.write(m))} bytes')
    for i, k in ((0, 0), (1, 0), (1, 1), (2, 2)):
        p(f'  Signer, input {i} with key {"ABC"[k]}: {R.SIGN_REASONS[R.sign(m, i, keys[k])]}')
    p(f'  key A on the P2TR input: {R.SIGN_REASONS[R.sign(m, 2, keys[0])]}')
    p(f'  signed: {len(R.write(m))} bytes')
    final = R.finalize(m)
    p(f'  Input Finalizer: final {yes(final)}, {len(R.write(m))} bytes')
    net = R.extract(m)
    t = T.unserialize(net, True)
    p(f'  Transaction Extractor: {len(net)} bytes, weight {T.weight(t)}, txid {T.txid(t)[::-1].hex()}')
    flags = sum(S.F[f] for f in ('P2SH', 'WITNESS', 'DERSIG', 'LOW_S', 'STRICTENC', 'NULLDUMMY', 'CLEANSTACK', 'NULLFAIL', 'WITNESS_PUBKEYTYPE'))
    for i in range(2):
        e = S.verify_script(t['ins'][i]['script'], spks[i], t['ins'][i]['witness'], flags, S.Checker(t, i, amounts[i]))
        p(f'  input {i}: {S.ERRORS[e]}')
    sig = t['ins'][2]['witness'][0]
    h = R.sighash_taproot(T.unserialize(unsigned, False), 2, list(zip(amounts, spks)), 0)
    p(f'  input 2: a {len(sig)} byte key-path signature, valid {yes(R.schnorr_verify(q, h, sig))}')


def transcript():
    lines = []
    p = lines.append
    p('Every published PSBT, read as Bitcoin Core reads it:')
    refused = 0
    cases = PC.all_cases()
    for i, (kind, src, _, text) in enumerate(cases):
        kind = kind.split('=')[0]
        e = R.refusal_text(text)
        line = f'  {i + 1} {src} ({KINDS[kind]}): '
        if e is not None:
            line += e.message(); refused += 1
        else:
            b = R.unbase64(text)
            d = R.decode(b)
            same = R.write(R.maps(b)) == b
            line += (f"v{d['version']}, {d['nin']} {plural(d['nin'], 'input', 'inputs')}, {d['nout']} "
                     f"{plural(d['nout'], 'output', 'outputs')}, {len(d['fields'])} {plural(len(d['fields']), 'pair', 'pairs')}, written back {'the same' if same else 'in Core' + chr(39) + 's order'}")
            if kind == 'locktime':
                lt = R.locktime(R.maps(b))
                line += '; no lock time satisfies every input' if lt is None else f'; lock time {lt}'
        p(line)
    p(f'  {len(cases)} PSBTs: {refused} refused, {len(cases) - refused} read; 0 differ from the published verdicts')
    key0, q0 = bip174(lines)
    p(f"BIP-174's signer checks, each input signed with m/0'/0'/{path_of(q0)[2]}':")
    for i, (kind, _, _, text) in enumerate(cases):
        if kind != 'signer': continue
        m = R.maps(R.unbase64(text))
        p(f'  case {i + 1}:' + ''.join(f' input {k}, {R.SIGN_REASONS[R.sign(m, k, key0)]};' for k in range(len(m['ins']))))
    _, d = PC.core_cases()
    p("Bitcoin Core's role cases (rpc_psbt.json):")
    for i, s in enumerate(d['signer']):
        m = R.maps(R.unbase64(s['psbt']))
        codes = [R.sign(m, n, wif(k)) for k in s['privkeys'] for n in range(len(m['ins']))]
        p(f'  walletprocesspsbt {i + 1}:' + ''.join(f' {c}' for c in codes) + f'; the published result: {yes(R.write(m) == R.unbase64(s["result"]))}')
    for i, c in enumerate(d['combiner']):
        a, b = (R.maps(R.unbase64(x)) for x in c['combine'])
        want = R.unbase64(c['result'])
        p(f'  combinepsbt {i + 1}, in either order: {yes(R.write(R.combine(a, b)) == want and R.write(R.combine(b, a)) == want)}')
    f = d['finalizer'][0]
    m = R.maps(R.unbase64(f['finalize']))
    final = R.finalize(m)
    p(f'  finalizepsbt: final {yes(final)}, {yes(R.write(m) == R.unbase64(f["result"]))}')
    x = d['extractor'][0]
    p(f'  finalizepsbt with extract: {yes(R.extract(R.maps(R.unbase64(x["extract"]))).hex() == x["result"])}')
    own(lines)
    return '\n'.join(lines) + '\n'


def main():
    check_lib()
    want = transcript()
    assert not re.search(r'(: |, |final )no(,|\n)', want), 'a published value differs'
    if '--write' in sys.argv:
        FIXTURE.write_text(want)
    assert FIXTURE.read_text() == want, 'the fixture differs from the Python transcript'
    print('psbt: the fixture is the Python transcript')
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            run = subprocess.run([coax, 'run', EXAMPLE, '--run-steps', '250000000'] + mode, cwd=REPO, capture_output=True, text=True)
            assert run.returncode == 0 and run.stdout == want, (mode, run.stderr[-2000:])
            print(f'native {"release" if mode else "debug"}: agrees')


if __name__ == '__main__':
    main()
