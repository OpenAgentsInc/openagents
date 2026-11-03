#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: BIP-327's own reference.py, pinned by hash, decides every case outside Coax)
"""The cross-check of Bitcoin chapter 101. It runs BIP-327's pinned
reference.py over every pinned vector file (its own tests, and six random
sessions), checks that coax/lib/bitcoin/musig.coax names the reference's
errors and that musig_vectors.coax is what make_musig_vectors.py writes,
and then writes, with the reference's functions, the transcript the
example must print:

  python3 -I coax/tests/bitcoin/musig.py                       # compare with the fixture
  python3 -I coax/tests/bitcoin/musig.py --coax coax/bin/coax  # also run the example
"""
import hashlib, json, re, subprocess, sys, types
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import make_musig_vectors as V  # noqa: E402

REPO = HERE.parents[2]
VEC = HERE.parent / 'vectors'
BIP = VEC / 'upstream' / 'bip327'
FIXTURE = REPO / 'bitcoin/fixtures/musig2.out'
EXAMPLE = 'bitcoin/examples/musig2.coax'
LIB = REPO / 'coax/lib/bitcoin/musig.coax'


def pinned(name):
    b = (BIP / name).read_bytes()
    m = json.loads((VEC / 'manifest.json').read_text())
    pin = next(e['sha256'] for e in m['files'] if e['path'] == 'upstream/bip327/' + name)
    assert hashlib.sha256(b).hexdigest() == pin, name
    return b


def load_reference():
    # Run the bytes whose hash was checked; its tests find the vectors
    # through sys.path[0], which the module gets as the pinned folder.
    for name in ['key_sort', 'key_agg', 'nonce_gen', 'nonce_agg', 'sign_verify', 'tweak', 'det_sign', 'sig_agg']:
        pinned(f'vectors/{name}_vectors.json')
    raw = pinned('reference.py')
    module = types.ModuleType('bip327_reference')
    exec(compile(raw, str(BIP / 'reference.py'), 'exec'), module.__dict__)
    module.sys = types.SimpleNamespace(path=[str(BIP)])
    return module


R = load_reference()
SEGWIT = None


def load_segwit():
    raw = (VEC / 'upstream/sipa-bech32/segwit_addr.py.txt').read_bytes()
    assert hashlib.sha256(raw).hexdigest() == '2884ce04a36c8374c4249177cd90e91e545ac73b5c9260307bbeb74ae2e9de0f'
    module = types.ModuleType('segwit_addr')
    exec(compile(raw, 'segwit_addr.py', 'exec'), module.__dict__)
    return module


def run_reference():
    R.test_key_sort_vectors()
    R.test_key_agg_vectors()
    R.test_nonce_gen_vectors()
    R.test_nonce_agg_vectors()
    R.test_sign_verify_vectors()
    R.test_tweak_vectors()
    R.test_det_sign_vectors()
    R.test_sig_agg_vectors()
    R.test_sign_and_verify_random(6)


def check_lib():
    lib = LIB.read_text()
    reasons = re.findall(r'"([^"]*)"', re.search(r'MS_REASONS :: \[15\] string\.\[(.*?)\];', lib, re.S).group(1))
    codes = dict((int(v), n) for n, v in re.findall(r'(?m)^(MS_[A-Z_0-9]+) :: (\d+);', lib) if n not in ('MS_KEYS', 'MS_TWEAKS'))
    assert len(reasons) == 15 and sorted(codes) == list(range(15)), codes
    for contrib, code in V.CONTRIB.items():
        assert reasons[code] == contrib, (contrib, code)
    for message, code in V.VALUE.items():
        assert reasons[code] == message, (message, code)
        assert message.replace("'", "\\'").encode() in pinned('reference.py') or message.encode() in pinned('reference.py'), message
    assert reasons[V.MS_INVALID] == 'invalid' and codes[V.MS_INVALID] == 'MS_INVALID'
    assert (HERE / 'musig_vectors.coax').read_text() == V.render(), 'musig_vectors.coax differs; run make_musig_vectors.py --write'


REASONS = ['ok', 'pubkey', 'pubnonce', 'aggnonce', 'aggothernonce', 'psig'] + list(V.VALUE) + ['invalid', 'capacity']


def outcome(fn):
    """The reason and signer the chapter prints for a call of the reference."""
    try:
        fn()
        return 'ok', -1
    except R.InvalidContributionError as e:
        return e.contrib, -1 if e.signer is None else e.signer
    except ValueError as e:
        return str(e), -1


def test_key(i):
    sk = bytes((17 * (i + 1) + j) & 255 for j in range(32))
    return sk, R.individual_pk(sk)


def xb(ctx):
    return R.get_xonly_pk(ctx)


def transcript():
    out = []
    p = out.append
    k = json.loads(pinned('vectors/key_agg_vectors.json'))
    X = [bytes.fromhex(x) for x in k['pubkeys']]
    keys = [X[0], X[1], X[2]]
    L = R.hash_keys(keys)
    p("KeyAgg of BIP-327's keys X1, X2, X3:")
    p(f'  L = hash_KeyAgg list(X1 || X2 || X3): {L.hex()}')
    second = next(j for j in range(1, 3) if keys[j] != keys[0])
    assert R.get_second_key(keys) == keys[second]
    p(f'  the second key: X{second + 1}, so its coefficient is 1')
    for i in range(3):
        p(f'  a_{i + 1}: {R.key_agg_coeff(keys, keys[i]).to_bytes(32, "big").hex()}')
    labels = ['X1, X2, X3', 'X3, X2, X1 (the order changes Q)', 'X1 three times', 'X1, X1, X2, X2']
    for c, label in zip(k['valid_test_cases'], labels):
        q = xb(R.key_agg([X[i] for i in c['key_indices']]))
        p(f'  Q of {label}: {q.hex()}, as published: {"yes" if q.hex() == c["expected"].lower() else "no"}')
    T = [bytes.fromhex(t) for t in k['tweaks']]
    for c in k['error_test_cases']:
        reason, signer = outcome(lambda: R.key_agg_and_tweak([X[i] for i in c['key_indices']], [T[i] for i in c['tweak_indices']], c['is_xonly']))
        p(f'  refused: {reason} (signer {signer})')

    s = json.loads(pinned('vectors/key_sort_vectors.json'))
    p('KeySort:')
    for key in R.key_sort([bytes.fromhex(x) for x in s['pubkeys']]):
        p(f'  {key.hex()}')

    tw = json.loads(pinned('vectors/tweak_vectors.json'))
    c = tw['valid_test_cases'][3]
    assert c['is_xonly'] == [False, False, True, True]
    p('Tweaking the key of tweak_vectors (plain, plain, x-only, x-only):')
    ctx = R.key_agg([bytes.fromhex(tw['pubkeys'][i]) for i in c['key_indices']])
    p(f'  Q: {R.cbytes(ctx.Q).hex()}')
    for i, (ti, xo) in enumerate(zip(c['tweak_indices'], c['is_xonly'])):
        ctx = R.apply_tweak(ctx, bytes.fromhex(tw['tweaks'][ti]), xo)
        p(f'  after {"x-only" if xo else "plain"} tweak {i + 1}: {R.cbytes(ctx.Q).hex()}, gacc is {"1" if ctx.gacc == 1 else "n - 1"}')
    p(f'  tacc: {ctx.tacc.to_bytes(32, "big").hex()}')

    p('Three signers sign one message:')
    signers = [test_key(i) for i in range(3)]
    sks = [x[0] for x in signers]
    pks = [x[1] for x in signers]
    msg = bytes(0xa0 + j for j in range(32))
    aggpk = xb(R.key_agg(pks))
    p(f'  aggregate key: {aggpk.hex()}')

    def session(tweaks, xonly, seed):
        tpk = xb(R.key_agg_and_tweak(pks, tweaks, xonly))
        secs, pubs = [], []
        for i in range(3):
            sec, pub = R.nonce_gen_internal(bytes([seed + i]) * 32, sks[i], pks[i], tpk, msg, b'')
            secs.append(sec); pubs.append(pub)
        agg = R.nonce_agg(pubs)
        ctx = R.SessionContext(agg, pks, tweaks, xonly, msg)
        psigs = [R.sign(secs[i], sks[i], ctx) for i in range(3)]
        return pubs, agg, psigs, R.partial_sig_agg(psigs, ctx)

    pubs, agg, psigs, sig = session([], [], 1)
    for i in range(3):
        p(f"  signer {i + 1}'s nonce: {pubs[i].hex()}")
    p(f'  aggnonce: {agg.hex()}')
    Q, gacc, tacc, b, Rp, e = R.get_session_values(R.SessionContext(agg, pks, [], [], msg))
    p(f'  b: {b.to_bytes(32, "big").hex()}')
    p(f'  R = R1 + b R2: {R.cbytes(Rp).hex()}')
    p(f'  e: {e.to_bytes(32, "big").hex()}')
    for i in range(3):
        p(f"  signer {i + 1}'s partial signature: {psigs[i].hex()}")
        p(f'    verifies: {"ok" if R.partial_sig_verify(psigs[i], pubs, pks, [], [], msg, i) else "invalid"}')
    bent = psigs[0][:31] + bytes([psigs[0][31] ^ 1])
    p(f"  signer 1's with its last bit changed: {'ok' if R.partial_sig_verify(bent, pubs, pks, [], [], msg, 0) else 'invalid'}")
    p(f'  signature: {sig.hex()}')
    p(f'  a valid BIP-340 signature under the aggregate key: {"yes" if R.schnorr_verify(msg, aggpk, sig) else "no"}')

    p('The aggregate key as a Taproot internal key:')
    tweak = R.tagged_hash('TapTweak', aggpk)
    p(f'  TapTweak(Q): {tweak.hex()}')
    q = xb(R.key_agg_and_tweak(pks, [tweak], [True]))
    p(f'  output key: {q.hex()}')
    # BIP-341's taproot_tweak_pubkey, written out.
    t = int.from_bytes(tweak, 'big')
    Pt = R.point_add(R.lift_x(aggpk), R.point_mul(R.G, t))
    p(f"  chapter 41's output key of the same internal key: {'yes' if R.xbytes(Pt) == q else 'no'}")
    p(f'  address: {load_segwit().encode("bc", 1, q)}')
    _, _, _, sig = session([tweak], [True], 11)
    p(f'  key-path signature: {sig.hex()}')
    p(f'  verifies under the output key: {"yes" if R.schnorr_verify(msg, q, sig) else "no"}')

    p('Nonce reuse:')
    sec, pub0 = R.nonce_gen_internal(b'\x55' * 32, sks[0], pks[0], None, None, None)
    copy = bytes(sec)

    def sign_one(sec, m, seed):
        pubs = [pub0] + [R.nonce_gen_internal(bytes([seed + i]) * 32, sks[i], pks[i], None, m, b'')[1] for i in (1, 2)]
        agg = R.nonce_agg(pubs)
        return pubs, agg, R.sign(sec, sks[0], R.SessionContext(agg, pks, [], [], m))

    p(f'  first Sign: {outcome(lambda: sign_one(sec, msg, 0))[0]}')
    p(f'  second Sign with the same secnonce: {outcome(lambda: sign_one(sec, msg, 0))[0]}')
    n = R.n
    rows = []
    for j in range(3):
        m = bytes([j + 1]) * 32
        pubs, agg, psig = sign_one(bytearray(copy), m, 20 + j)
        p(f'  message {j + 1}: s = {psig.hex()}')
        Qj, _, _, bj, Rj, ej = R.get_session_values(R.SessionContext(agg, pks, [], [], m))
        sigma = 1 if R.has_even_y(Rj) else n - 1
        a = R.key_agg_coeff(pks, pks[0])
        rows.append([1, bj, sigma * ej * a % n, sigma * int.from_bytes(psig, 'big') % n])

    def det3(M):
        return (M[0][0] * (M[1][1] * M[2][2] - M[1][2] * M[2][1]) - M[0][1] * (M[1][0] * M[2][2] - M[1][2] * M[2][0])
                + M[0][2] * (M[1][0] * M[2][1] - M[1][1] * M[2][0])) % n
    M = [r[:3] for r in rows]
    Md = [[r[0], r[1], r[3]] for r in rows]
    d = det3(Md) * pow(det3(M), n - 2, n) % n
    g = 1 if R.has_even_y(R.key_agg(pks).Q) else n - 1
    d = d * g % n
    p(f'  solved for the secret key: {d.to_bytes(32, "big").hex()}')
    p(f"  it is signer 1's secret key: {'yes' if d == int.from_bytes(sks[0], 'big') else 'no'}")

    ds = json.loads(pinned('vectors/det_sign_vectors.json'))
    c = ds['valid_test_cases'][0]
    p('DeterministicSign (det_sign_vectors case 1):')
    pub, psig = R.deterministic_sign(bytes.fromhex(ds['sk']), bytes.fromhex(c['aggothernonce']),
                                     [bytes.fromhex(ds['pubkeys'][i]) for i in c['key_indices']], [], [],
                                     bytes.fromhex(ds['msgs'][c['msg_index']]), bytes.fromhex(c['rand']))
    p(f'  pubnonce: {pub.hex()}')
    p(f'  psig: {psig.hex()}')
    same = pub.hex() == c['expected'][0].lower() and psig.hex() == c['expected'][1].lower()
    p(f'  ok, as published: {"yes" if same else "no"}')

    # Every case passed run_reference(); the counts are the cases the
    # generator wrote for each file.
    text = V.render()
    count = lambda name: int(re.search(rf'{name} :: \[(\d+)\]', text).group(1))
    p("BIP-327's vector files, cases giving their stated result:")
    p('  key_sort_vectors: 1 of 1')
    for label, name in [('key_agg_vectors', 'MS_KEY_AGG'), ('nonce_gen_vectors', 'MS_NONCE_GEN'), ('nonce_agg_vectors', 'MS_NONCE_AGG'),
                        ('sign_verify_vectors and tweak_vectors, Sign', 'MS_SIGN'),
                        ('sign_verify_vectors and tweak_vectors, PartialSigVerify', 'MS_VERIFY'),
                        ('det_sign_vectors', 'MS_DET_SIGN'), ('sig_agg_vectors', 'MS_SIG_AGG')]:
        p(f'  {label}: {count(name)} of {count(name)}')
    return '\n'.join(out) + '\n'


def main():
    run_reference()
    print("BIP-327's reference.py passes every pinned vector file and six random sessions")
    check_lib()
    want = transcript()
    if '--write' in sys.argv:
        FIXTURE.write_text(want)
    assert FIXTURE.read_text() == want, 'the fixture differs from the Python transcript'
    print(f'musig2 fixture: {want.count(chr(10))} lines agree with Python')
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            run = subprocess.run([coax, 'run', EXAMPLE, '--run-steps', '250000000'] + mode, cwd=REPO, capture_output=True, text=True)
            assert run.returncode == 0 and run.stdout == want, f'native {mode or "debug"} output differs:\n{run.stdout}{run.stderr}'
            print(f'native {"release" if mode else "debug"}: agrees')


if __name__ == '__main__':
    main()
