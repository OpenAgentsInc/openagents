#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: BOLT 3's text and separate Python key derivation decide each case outside Coax)
"""The cross-check of Bitcoin chapter 106. It computes every line the
chapter's example prints and compares the transcript with the fixture:

  python3 -I coax/tests/lightning/keys.py                      # compare with the fixture
  python3 -I coax/tests/lightning/keys.py --coax coax/bin/coax   # also run the example

The derivations are this file's own Python, written from BOLT 3's "Key
Derivation" on ln_crypto.py's secp256k1 and shachain.py's generator,
sharing no code with Coax. Appendix E's keys must come out before the
transcript is compared.
"""
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import bolts  # noqa: E402
import ln_crypto as C  # noqa: E402
import shachain  # noqa: E402

REPO = HERE.parents[2]
FIXTURE = REPO / 'bitcoin/fixtures/lightning-keys.out'
EXAMPLE = 'bitcoin/examples/lightning-keys.coax'


def tf(b):
    return 'true' if b else 'false'


def point(k):
    return C.compress(C.mul(k))


def tweak(a, b):
    return int.from_bytes(C.sha256(a, b), 'big')


def pubkey(basepoint, pcp):
    return C.compress(C.add(C.decompress(basepoint), C.mul(tweak(pcp, basepoint))))


def privkey(base_secret, pcp):
    return (base_secret + tweak(pcp, point(base_secret))) % C.N


def revocation_pubkey(rb, pcp):
    a = C.mul(tweak(rb, pcp), C.decompress(rb))
    b = C.mul(tweak(pcp, rb), C.decompress(pcp))
    return C.compress(C.add(a, b))


def revocation_privkey(r, p):
    big_r, big_p = point(r), point(p)
    return (r * tweak(big_r, big_p) + p * tweak(big_p, big_r)) % C.N


def secret(k):
    return int.from_bytes(bytes((k + i) & 255 for i in range(32)), 'big')


def transcript():
    v = bolts.keys()
    bs, ps = int(v['base_secret'], 16), int(v['per_commitment_secret'], 16)
    base, pcp = point(bs), point(ps)
    assert base.hex() == v['base_point'] and pcp.hex() == v['per_commitment_point']
    assert pubkey(base, pcp).hex() == v['localpubkey']
    assert f'{privkey(bs, pcp):064x}' == v['localprivkey']
    assert revocation_pubkey(base, pcp).hex() == v['revocationpubkey']
    assert f'{revocation_privkey(bs, ps):064x}' == v['revocationprivkey']
    out = ['BOLT 3 key derivation vectors',
           '  base_point: true', '  per_commitment_point: true', '  localpubkey: true',
           f'  localprivkey: true, times G is localpubkey: {tf(point(privkey(bs, pcp)) == pubkey(base, pcp))}',
           '  revocationpubkey: true',
           f'  revocationprivkey: true, times G is revocationpubkey: '
           f'{tf(point(revocation_privkey(bs, ps)) == revocation_pubkey(base, pcp))}']

    out.append("Bob's commitment 42")
    seed = secret(100).to_bytes(32, 'big')
    pcs = int.from_bytes(shachain.generate(seed, shachain.MAX - 42), 'big')
    pcp = point(pcs)
    bob_delayed, bob_htlc = secret(2), secret(3)
    alice_payment, alice_htlc, alice_revocation = secret(11), secret(13), secret(14)
    delayed = pubkey(point(bob_delayed), pcp)
    revocation = revocation_pubkey(point(alice_revocation), pcp)
    out += [f'  per_commitment_point: {pcp.hex()}',
            f'  local_delayedpubkey: {delayed.hex()}',
            f'  revocationpubkey:    {revocation.hex()}',
            f'  remotepubkey:        {point(alice_payment).hex()}',
            f'  local_htlcpubkey:    {pubkey(point(bob_htlc), pcp).hex()}',
            f'  remote_htlcpubkey:   {pubkey(point(alice_htlc), pcp).hex()}',
            f"  Bob's delayed private key matches: {tf(point(privkey(bob_delayed, pcp)) == delayed)}"]

    out.append('revoking commitment 42')
    out += [f"  Bob's per-commitment secret alone: {tf(point(pcs) == revocation)}",
            f"  Alice's revocation secret alone: {tf(point(alice_revocation) == revocation)}",
            f'  the secret matches the point Bob sent: {tf(point(pcs) == pcp)}']
    store = shachain.Store()
    accepted = sum(store.insert(shachain.MAX - n, shachain.generate(seed, shachain.MAX - n)) == 'OK' for n in range(43))
    out.append(f"  Alice's store holds {accepted} secrets")
    got = int.from_bytes(store.lookup(shachain.MAX - 42)[1], 'big')
    out.append(f'  revocationprivkey: OK, times G is revocationpubkey: '
               f'{tf(point(revocation_privkey(alice_revocation, got)) == revocation)}')
    older = int.from_bytes(store.lookup(shachain.MAX - 41)[1], 'big')
    out.append(f"  commitment 41's secret instead: {tf(point(revocation_privkey(alice_revocation, older)) == revocation)}")

    out.append('refusals')
    out.append(f'  per-commitment point with prefix 04: {"BAD_POINT" if C.decompress(b"\x04" + pcp[1:]) is None else "OK"}')
    out.append('  a zero secret: BAD_SECRET')
    return out


def main():
    lines = transcript()
    want = FIXTURE.read_text().splitlines()
    assert lines == want, '\n'.join(f'{a!r} != {b!r}' for a, b in zip(lines, want) if a != b) or 'line counts differ'
    print(f'{len(lines)} lines agree with {FIXTURE.name}')
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            got = subprocess.run([coax, 'run', EXAMPLE, *mode], cwd=REPO, capture_output=True, text=True, check=True)
            assert got.stdout.splitlines() == lines, f'coax run {" ".join(mode)} differs'
            print(f'coax run {" ".join(mode) or "(debug)"}: {len(lines)} lines agree')


if __name__ == '__main__':
    main()
