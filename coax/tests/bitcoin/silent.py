#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: BIP-352's pinned reference.py and silent_reference.py decide every line outside Coax)
"""The cross-check of Bitcoin chapter 97. It runs BIP-352's own
reference.py, pinned by hash with the secp256k1lab it imports, over the
pinned send_and_receive_test_vectors.json; checks that
silent_vectors.coax is what make_silent_vectors.py writes; and writes,
with silent_reference.py, the transcript the example must print:

  python3 -I coax/tests/bitcoin/silent.py                       # compare with the fixture
  python3 -I coax/tests/bitcoin/silent.py --coax coax/bin/coax  # also run the example
"""
import hashlib, hmac, json, subprocess, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import make_silent_vectors as V  # noqa: E402
import silent_reference as R  # noqa: E402

REPO = HERE.parents[2]
VEC = HERE.parent / 'vectors'
BIP = VEC / 'upstream' / 'bip352'
FIXTURE = REPO / 'bitcoin/fixtures/silent.out'
EXAMPLE = 'bitcoin/examples/silent.coax'
H = 2 ** 31


def run_reference():
    """reference.py over the vectors, from the pinned files only. -B keeps
    Python from writing bytecode into the pinned folder."""
    manifest = json.loads((VEC / 'manifest.json').read_text())
    pins = {e['path']: e['sha256'] for e in manifest['files'] if e['path'].startswith('upstream/bip352/')}
    assert len(pins) == 11
    for path, sha in pins.items():
        assert hashlib.sha256((VEC / path).read_bytes()).hexdigest() == sha, path
    out = subprocess.run([sys.executable, '-B', '-E', '-s', 'reference.py', 'send_and_receive_test_vectors.json'],
                         cwd=BIP, capture_output=True, text=True, check=True).stdout
    assert out.endswith('All tests passed\n'), out[-500:]
    return out.count('\n') - 1


def bip32(seed, path):
    """BIP-32 private derivation along path."""
    i = hmac.new(b'Bitcoin seed', seed, hashlib.sha512).digest()
    k, c = int.from_bytes(i[:32], 'big'), i[32:]
    for index in path:
        data = (b'\0' + k.to_bytes(32, 'big') if index >= H else R.ser_p(R.base(k))) + index.to_bytes(4, 'big')
        i = hmac.new(c, data, hashlib.sha512).digest()
        k, c = (int.from_bytes(i[:32], 'big') + k) % R.N, i[32:]
    return k


class Wallet:
    def __init__(self, seed):
        self.scan = bip32(seed, [H + 352, H, H, H + 1, 0])
        self.spend = bip32(seed, [H + 352, H, H, H, 0])
        self.b_scan, self.b_spend = R.base(self.scan), R.base(self.spend)

    def address(self, m=None, testnet=False):
        b_m = self.b_spend if m is None else R.labeled_spend(self.scan, self.b_spend, m)
        return R.encode(R.ser_p(self.b_scan), R.ser_p(b_m), testnet)


def transcript():
    inputs, cases, recipients, receives = V.build()
    addresses = V.constructed()
    L = []
    receiver, sender, other = Wallet(bytes(range(16))), Wallet(b'\x97' * 16), Wallet(b'\x52' * 16)
    a = receiver.address()
    L.append("The receiver (seed 000102...0f, m/352'/0'/0'):")
    L.append(f'  address  {a} ({len(a)} characters)')
    L.append(f'  signet   {receiver.address(testnet=True)}')
    L.append(f'  label 1  {receiver.address(1)}')
    reason, _, version, scan, _ = R.decode(a)
    L.append(f'  decoded: version {version}, B_scan {scan.hex()}')
    L.append("The sender's change address (seed 9797...97, label 0):")
    L.append(f'  {sender.address(0)}')
    c = cases[9]
    L.append(f'The inputs ("{c["comment"]}"):')
    keys, pubs, outpoints = [], [], []
    for k, v in enumerate(inputs[c['first']:c['first'] + c['inputs']]):
        spk = bytes.fromhex(v['prevout'])
        pub = R.input_key(bytes.fromhex(v['script_sig']), bytes.fromhex(v['witness']), spk)
        outpoints.append(R.outpoint(v['txid'], v['vout']))
        kind = ('P2TR' if R.is_taproot(spk) else 'P2PKH' if len(spk) == 25 else 'P2WPKH' if len(spk) == 22 else 'P2SH-P2WPKH')
        if pub is None:
            L.append(f'  {k}: none '); continue
        d = int(v['key'], 16)
        odd = R.is_taproot(spk) and R.base(d)[1] % 2 == 1
        L.append(f'  {k}: {kind} {R.ser_p(pub).hex()}' + (' (odd y: negated)' if odd else ''))
        keys.append((d, R.is_taproot(spk))); pubs.append(pub)
    to = [receiver.address(), receiver.address(1), sender.address(0)]
    outputs, _, sum_a = R.send(keys, outpoints, to)
    a_point = R.base(sum_a)
    ih = R.input_hash(outpoints, a_point)
    L.append(f'  A          {R.ser_p(a_point).hex()}')
    L.append(f'  input_hash {ih:064x}')
    names = ['to the receiver, k = 0', 'to its label 1, k = 1  ', 'change, k = 0          ']
    for name, o in zip(names, outputs): L.append(f'  {name} {o.hex()}')
    tweak = R.mul(ih, a_point)
    L.append(f'Scanning with the tweak input_hash A = {R.ser_p(tweak).hex()}:')
    spends = 0
    for name, w in (('the receiver', receiver), ('the sender', sender), ('another wallet', other)):
        _, ecdh, found = R.scan(w.scan, w.b_spend, a_point, ih, outputs, [0, 1])
        parts = []
        for k, (key, t) in enumerate(found):
            tk = R.shared_tweak(ecdh, k)
            m = next((m for m in (0, 1) if (tk + R.label(w.scan, m)) % R.N == t), None) if t != tk else None
            parts.append(f'output {outputs.index(key)}' + (f' (label {m})' if m is not None else ''))
            if R.xonly(R.base((w.spend + t) % R.N)) == key: spends += 1
        L.append(f'  {name} finds {len(found)}' + (': ' + ', '.join(parts) if parts else ''))
    L.append(f'  b_spend + tweak is the secret key of each output found: {spends} of {len(outputs)}')
    L.append('The BIP-352 vectors:')
    L.append(f'  inputs: {len(inputs)} read, each giving the published key or none')
    L.append(f'  sending: {len(cases)} of {len(cases)} cases make a published output set, or fail as published')
    L.append(f'  receiving: {len(receives)} of {len(receives)} find the published outputs, tweaks and signatures;')
    kmax = next(r for r in receives if r['n'] == R.K_MAX)
    L.append(f'    one stops at K_max, finding {R.K_MAX} of its {len(kmax["outputs"]) // 64} outputs')
    L.append(f'  addresses: {len(addresses)} of {len(addresses)} constructed ones decode as decided')
    return '\n'.join(L) + '\n'


def main():
    print(f'reference.py: BIP-352\'s reference passes all {run_reference()} published cases')
    V.main()
    text = transcript()
    if '--write' in sys.argv:
        FIXTURE.write_text(text)
    assert FIXTURE.read_text() == text, 'the fixture differs from Python'
    print(f'silent.py: {text.count(chr(10))} lines agree with Python')
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            out = subprocess.run([coax, 'run', EXAMPLE, '--run-steps', '100000000'] + mode, cwd=REPO,
                                 capture_output=True, text=True, check=True).stdout
            assert out == text, f'the example differs from Python ({mode or "debug"})'
            print(f'silent.coax ({"release" if mode else "debug"}) prints the same transcript')


if __name__ == '__main__':
    main()
