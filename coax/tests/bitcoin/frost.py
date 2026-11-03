#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: the FROST draft BIP's own reference implementation, pinned by hash, decides every case outside Coax)
"""The cross-check of Bitcoin chapter 102. It runs the reference
implementation that ships with the FROST draft BIP (BIP-445, pinned at
siv2r/bip-frost-signing ba299bad) over every pinned vector file (its own
tests, and six random sessions), checks that coax/lib/bitcoin/frost.coax
names the reference's errors and that frost_vectors.coax is what
make_frost_vectors.py writes, and then writes, with the reference's
functions and secp256k1lab's BIP-340 signer, the transcript the example
must print:

  python3 -I coax/tests/bitcoin/frost.py                       # compare with the fixture
  python3 -I coax/tests/bitcoin/frost.py --coax coax/bin/coax  # also run the example
"""
import hashlib, json, re, subprocess, sys
sys.dont_write_bytecode = True   # nothing written into the pinned upstream folder
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import make_frost_vectors as V  # noqa: E402

REPO = HERE.parents[2]
VEC = HERE.parent / 'vectors'
BIP = VEC / 'upstream' / 'bip445'
FIXTURE = REPO / 'bitcoin/fixtures/frost.out'
EXAMPLE = 'bitcoin/examples/frost.coax'
LIB = REPO / 'coax/lib/bitcoin/frost.coax'


def check_pins():
    # Every pinned file of the BIP, its reference code and secp256k1lab,
    # before any of it runs.
    m = json.loads((VEC / 'manifest.json').read_text())
    files = [e for e in m['files'] if e['path'].startswith('upstream/bip445/')]
    assert len(files) == 20, len(files)
    for e in files:
        assert hashlib.sha256((VEC / e['path']).read_bytes()).hexdigest() == e['sha256'], e['path']


check_pins()
sys.path.insert(0, str(BIP / 'python' / 'secp256k1lab' / 'src'))
sys.path.insert(0, str(BIP / 'python'))
import frost_ref as F  # noqa: E402
from frost_ref import signing as S  # noqa: E402
import trusted_dealer as D  # noqa: E402
import tests as T  # noqa: E402
from secp256k1lab.secp256k1 import G, GE, Scalar  # noqa: E402
from secp256k1lab.bip340 import schnorr_sign, schnorr_verify  # noqa: E402
from secp256k1lab.util import tagged_hash  # noqa: E402
T.sys = type('S', (), {'path': [str(BIP / 'python')]})


def run_reference():
    T.test_nonce_gen_vectors()
    T.test_nonce_agg_vectors()
    T.test_sign_verify_vectors()
    T.test_tweak_vectors()
    T.test_det_sign_vectors()
    T.test_sig_agg_vectors()
    T.test_threshold_info_vectors()
    T.test_sign_and_verify_random(6)


def check_lib():
    lib = LIB.read_text()
    reasons = re.findall(r'"([^"]*)"', re.search(r'FR_REASONS :: \[32\] string\.\[(.*?)\];', lib, re.S).group(1))
    codes = dict((int(v), n) for n, v in re.findall(r'(?m)^(FR_[A-Z_0-9]+) :: (\d+);', lib) if n not in ('FR_MAX', 'FR_TWEAKS'))
    assert len(reasons) == 32 and sorted(codes) == list(range(32)), codes
    source = (BIP / 'python' / 'frost_ref' / 'signing.py').read_text()
    for contrib, code in V.CONTRIB.items():
        assert reasons[code] == contrib and f'"{contrib}"' in source, (contrib, code)
    for message, code in V.VALUE.items():
        assert reasons[code] == message, (message, code)
        forms = [message.replace('%', x) for x in ('{i}', '{idx}')] + [message.replace('128', '{MAX_PARTICIPANTS}')]
        assert any(f in source for f in [message] + forms), message
    assert reasons[V.FR_INVALID] == 'invalid' and codes[V.FR_INVALID] == 'FR_INVALID'
    assert (HERE / 'frost_vectors.coax').read_text() == V.render(), 'frost_vectors.coax differs; run make_frost_vectors.py --write'


def outcome(fn):
    try:
        fn()
        return 'ok'
    except F.InvalidContributionError as e:
        return e.contrib
    except ValueError as e:
        return str(e)


# The DKG of frost.coax, written out with secp256k1lab: each participant
# deals its seed with the trusted dealer's polynomial, commits to the
# coefficients and signs hash_FROST/dkg/pop(context || i || commitments)
# with BIP-340 and zero auxiliary randomness.
def dkg_poly(seed, t):
    a0 = Scalar.from_bytes_nonzero_checked(seed)
    return [a0] + [Scalar.from_bytes_nonzero_checked(tagged_hash(D.COEFF_DERIVATION_TAG, seed + (t - k).to_bytes(4, 'big'))) for k in range(1, t)]


def poly_at(a, x):
    return sum((c * Scalar(x) ** k for k, c in enumerate(a)), Scalar(0))


def dkg_round1(seed, i, n, t, ctx):
    a = dkg_poly(seed, t)
    commit = b''.join((c * G).to_bytes_compressed() for c in a)
    msg = tagged_hash('FROST/dkg/pop', ctx + i.to_bytes(4, 'big') + commit)
    pop = schnorr_sign(msg, seed, bytes(32))
    shares = [poly_at(a, j + 1).to_bytes() for j in range(n)]
    return commit, pop, shares


def dkg_check(commit, pop, ctx, i, share, j):
    msg = tagged_hash('FROST/dkg/pop', ctx + i.to_bytes(4, 'big') + commit)
    if not schnorr_verify(msg, commit[1:33], pop):
        return 'proof of possession'
    want = GE()
    for k in range(len(commit) // 33):
        want += Scalar(j + 1) ** k * GE.from_bytes_compressed(commit[33 * k:33 * k + 33])
    return 'ok' if Scalar.from_bytes_checked(share) * G == want else 'share'


def session(secshares, n, t, ids, pubshares, thresh_pk, tweaks, xonly, msg, seed):
    q = F.get_xonly_pk(S.thresh_pubkey_and_tweak(thresh_pk, tweaks, xonly))
    secs, pubs = [], []
    for i, id_ in enumerate(ids):
        sec, pub = S.nonce_gen_internal(bytes([seed + id_]) * 32, secshares[id_], id_, pubshares[i], q, msg, b'')
        secs.append(sec); pubs.append(pub)
    agg = F.nonce_agg(pubs)
    ctx = F.SessionContext(n, t, ids, pubshares, thresh_pk, agg, tweaks, xonly, msg)
    psigs = [F.sign(secs[i], secshares[id_], id_, ctx) for i, id_ in enumerate(ids)]
    return pubs, agg, psigs, F.partial_sig_agg(psigs, ctx)


def transcript():
    out = []
    p = out.append
    p('A trusted dealer splits a secret 2-of-3:')
    secret = bytes(range(1, 33))
    tp, sec, pub = D.trusted_dealer_keygen(secret, 3, 2)
    p(f'  threshold key: {tp.hex()}')
    for i in range(3):
        p(f'  participant {i}: secret share {sec[i].hex()}, public share {pub[i].hex()}')
    p(f'  the shares lie on one line through the key: {outcome(lambda: F.validate_threshold_info(F.ThresholdInfo(2, tp, pub)))}')
    sw = [pub[1], pub[0], pub[2]]
    p(f'  with shares 0 and 1 swapped: {outcome(lambda: F.validate_threshold_info(F.ThresholdInfo(2, tp, sw)))}')

    p('Lagrange interpolation at x = 0:')
    for ids in ([0, 1], [1, 2], [0, 2]):
        l0 = S.derive_interpolating_value(ids, ids[0]).to_bytes().hex()
        l1 = S.derive_interpolating_value(ids, ids[1]).to_bytes().hex()
        back = D.secret_share_combine([Scalar.from_bytes_checked(sec[i]) for i in ids], ids)
        p(f'  participants {ids[0]} and {ids[1]}: lambda {l0}, {l1}; the secret: {"yes" if back.to_bytes() == secret else "no"}')

    p('Participants 0 and 2 sign one message:')
    msg = bytes(0xb0 + j for j in range(32))
    ids = [0, 2]
    ps = [pub[i] for i in ids]
    pubs, agg, psigs, sig = session(sec, 3, 2, ids, ps, tp, [], [], msg, 1)
    for i in range(2):
        p(f"  participant {ids[i]}'s nonce: {pubs[i].hex()}")
    p(f'  aggnonce: {agg.hex()}')
    ctx = F.SessionContext(3, 2, ids, ps, tp, agg, [], [], msg)
    Q, gacc, tacc, _, _, b, Rp, e = S.get_session_values(ctx)
    p(f'  b: {b.to_bytes().hex()}')
    p(f'  R = R1 + b R2: {Rp.to_bytes_compressed().hex()}')
    p(f'  e: {e.to_bytes().hex()}')
    p(f'  the threshold key has an {"even" if Q.has_even_y() else "odd"} y, so g is {"1" if Q.has_even_y() else "n - 1"}')
    for i in range(2):
        lam = S.derive_interpolating_value(ids, ids[i]).to_bytes().hex()
        p(f'  participant {ids[i]}: lambda {lam}, partial signature {psigs[i].hex()}')
        ok = F.partial_sig_verify(psigs[i], pubs, 3, 2, ids, ps, tp, [], [], msg, i)
        p(f'    verifies: {"ok" if ok else "invalid"}')
    bent = psigs[0][:31] + bytes([psigs[0][31] ^ 1])
    p(f"  participant 0's with its last bit changed: {'ok' if F.partial_sig_verify(bent, pubs, 3, 2, ids, ps, tp, [], [], msg, 0) else 'invalid'}")
    p(f'  signature: {sig.hex()}')
    p(f'  a valid BIP-340 signature under the threshold key: {"yes" if schnorr_verify(msg, tp[1:], sig) else "no"}')
    other = [0, 1]
    b2 = S.get_session_values(F.SessionContext(3, 2, other, [pub[i] for i in other], tp, agg, [], [], msg))[5]
    p(f'  b for the same nonces with participants 0 and 1: {b2.to_bytes().hex()}')
    p(f'  participant 0 alone: {outcome(lambda: S.get_session_values(F.SessionContext(3, 2, [0], [pub[0]], tp, agg, [], [], msg)))}')

    p('The threshold key as a Taproot internal key:')
    tweak = tagged_hash('TapTweak', tp[1:])
    p(f'  TapTweak(Q): {tweak.hex()}')
    tctx = S.thresh_pubkey_and_tweak(tp, [tweak], [True])
    output = F.get_xonly_pk(tctx)
    p(f'  output key: {output.hex()}')
    p(f'  gacc is {"1" if tctx.gacc == Scalar(1) else "n - 1"}')
    # BIP-341's taproot_tweak_pubkey, written out.
    P = GE.from_bytes_xonly(tp[1:]) + Scalar.from_bytes_checked(tweak) * G
    p(f"  chapter 41's output key of the same internal key: {'yes' if P.to_bytes_xonly() == output else 'no'}")
    p(f'  address: {load_segwit().encode("bc", 1, output)}')
    _, _, _, sig = session(sec, 3, 2, ids, ps, tp, [tweak], [True], msg, 11)
    p(f'  key-path signature: {sig.hex()}')
    p(f'  verifies under the output key: {"yes" if schnorr_verify(msg, output, sig) else "no"}')

    p('A 2-of-3 Pedersen DKG:')
    ctxb = b'chapter 102 DKG'
    seeds = bytes((3 * i + 5) & 255 for i in range(96))
    rounds = [dkg_round1(seeds[32 * i:32 * i + 32], i, 3, 2, ctxb) for i in range(3)]
    for i, (commit, pop, _) in enumerate(rounds):
        p(f'  participant {i} commits {commit[:33].hex()} and {commit[33:].hex()}')
        p(f'    proof of possession {pop.hex()}')
    checks = sum(dkg_check(rounds[i][0], rounds[i][1], ctxb, i, rounds[i][2][j], j) == 'ok' for i in range(3) for j in range(3))
    p(f'  checks that pass, of 9: {checks}')
    p(f"  participant 0's proof checked as participant 1's: {dkg_check(rounds[0][0], rounds[0][1], ctxb, 1, rounds[0][2][0], 0)}")
    dsec = [sum((Scalar.from_bytes_checked(rounds[i][2][j]) for i in range(3)), Scalar(0)).to_bytes() for j in range(3)]
    dtp = sum((GE.from_bytes_compressed(rounds[i][0][:33]) for i in range(3)), GE()).to_bytes_compressed()
    dpub = [(Scalar.from_bytes_checked(s) * G).to_bytes_compressed() for s in dsec]
    p(f'  threshold key: {dtp.hex()}')
    for i in range(3):
        p(f"  participant {i}'s public share: {dpub[i].hex()}")
    # The public shares come from the commitments alone; they match the
    # secret shares.
    from_commits = []
    for j in range(3):
        X = GE()
        for i in range(3):
            c = rounds[i][0]
            X += GE.from_bytes_compressed(c[:33]) + Scalar(j + 1) * GE.from_bytes_compressed(c[33:])
        from_commits.append(X.to_bytes_compressed())
    p(f'  every participant computed the same key and shares: {"yes" if from_commits == dpub else "no"}')
    p(f'  threshold info: {outcome(lambda: F.validate_threshold_info(F.ThresholdInfo(2, dtp, dpub)))}')
    dids = [1, 2]
    _, _, _, sig = session(dsec, 3, 2, dids, [dpub[i] for i in dids], dtp, [], [], msg, 21)
    p(f'  participants 1 and 2 sign: {sig.hex()}')
    p(f"  a valid BIP-340 signature under the DKG's key: {'yes' if schnorr_verify(msg, dtp[1:], sig) else 'no'}")

    ds = json.loads((BIP / 'python/vectors/det_sign_vectors.json').read_text())
    g = ds['test_groups'][0]
    c = g['valid_tests'][0]
    p('DeterministicSign (det_sign_vectors case 1):')
    pubn, psig = F.deterministic_sign(bytes.fromhex(g['secshares'][c['secshare_index']]), c['signer_id'], bytes.fromhex(c['aggothernonce']),
                                      g['n'], g['t'], c['ids'], [bytes.fromhex(g['pubshares'][i]) for i in c['pubshare_indices']],
                                      bytes.fromhex(g['thresh_pk']), [], [], bytes.fromhex(c['msg']), bytes.fromhex(c['aux_rand']))
    p(f'  pubnonce: {pubn.hex()}')
    p(f'  psig: {psig.hex()}')
    same = pubn.hex() == c['expected'][0].lower() and psig.hex() == c['expected'][1].lower()
    p(f'  ok, as published: {"yes" if same else "no"}')

    # Every case passed run_reference(); the counts are the cases the
    # generator wrote for each file.
    text = V.render()
    count = lambda name: int(re.search(rf'{name} :: \[(\d+)\]', text).group(1))
    p("BIP-445's vector files, cases giving their stated result:")
    for label, name in [('nonce_gen_vectors', 'FR_NONCE_GEN'), ('nonce_agg_vectors', 'FR_NONCE_AGG'),
                        ('sign_verify_vectors and tweak_vectors, Sign', 'FR_SIGN'),
                        ('sign_verify_vectors and tweak_vectors, PartialSigVerify', 'FR_VERIFY'),
                        ('det_sign_vectors', 'FR_DET_SIGN'), ('sig_agg_vectors', 'FR_SIG_AGG'), ('threshold_info_vectors', 'FR_INFO')]:
        p(f'  {label}: {count(name)} of {count(name)}')
    return '\n'.join(out) + '\n'


def load_segwit():
    import types
    raw = (VEC / 'upstream/sipa-bech32/segwit_addr.py.txt').read_bytes()
    assert hashlib.sha256(raw).hexdigest() == '2884ce04a36c8374c4249177cd90e91e545ac73b5c9260307bbeb74ae2e9de0f'
    module = types.ModuleType('segwit_addr')
    exec(compile(raw, 'segwit_addr.py', 'exec'), module.__dict__)
    return module


def main():
    run_reference()
    print("BIP-445's reference passes every pinned vector file and six random sessions")
    check_lib()
    want = transcript()
    if '--write' in sys.argv:
        FIXTURE.write_text(want)
    assert FIXTURE.read_text() == want, 'the fixture differs from the Python transcript'
    print(f'frost fixture: {want.count(chr(10))} lines agree with Python')
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            run = subprocess.run([coax, 'run', EXAMPLE, '--run-steps', '250000000'] + mode, cwd=REPO, capture_output=True, text=True)
            assert run.returncode == 0 and run.stdout == want, f'native {mode or "debug"} output differs:\n{run.stdout}{run.stderr}'
            print(f'native {"release" if mode else "debug"}: agrees')


if __name__ == '__main__':
    main()
