#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: Python decides each case outside Coax)
"""The cross-check of Bitcoin chapter 40. It computes every number the
chapter's example prints, in Python that shares no code with Coax, and
writes the transcript the example must print:

  python3 -I coax/tests/bitcoin/fees.py                     # compare with the fixture
  python3 -I coax/tests/bitcoin/fees.py --coax coax/bin/coax   # also run the example

Transactions come from the pinned sources: the three that tx_valid.json
names (by txid, through tx_vectors.coax) and the three segwit ones from
mempool.space, whose JSON also gives the spent values, the fee, the sigop
cost and the weight. Transactions are read by tx_reference.py (a
transliteration of Core). The sigop-adjusted vsize follows Core's
GetVirtualTransactionSize in the pinned policy.cpp, and the limits are
read from the pinned consensus.h, policy.h and amount.h.
"""
import hashlib, json, re, subprocess, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import tx_reference as T  # noqa: E402

REPO = HERE.parents[2]
UP = HERE.parent / 'vectors' / 'upstream'
FIXTURE = REPO / 'bitcoin/fixtures/fees.out'
EXAMPLE = 'bitcoin/examples/fees.coax'
SEGWIT = ['f5864806e3565c34d1b41e716f72609d00b55ea5eac5b924c9719a842ef42206',
          '33e794d097969002ee05d336686fc03c9e15a597c1b9827669460fac98799036',
          '37777defed8717c581b4c0509329550e344bdc14ac38f71fc050096887e535c8']
PINS = {
    'bitcoin-core/policy.cpp': '09dbc636d841d27a1d876bc179be3061c74d82fdd42a440d26738211637f40c2',
    'bitcoin-core/policy.h': '7ffa54e839f78877c90ea59961995c38caf7015ad7280413cc49c37ab8c9a5cd',
    'bitcoin-core/consensus.h': 'aa55254b379a2cc5f1835b506e248f80b4f020120782d2780ce52ff7c8cb827d',
    'bitcoin-core/amount.h': 'fbaf7f3186b066f911e74720117299e9b88132cf612a1b27dfe34ab55752a393',
    'bitcoin-core/tx_valid.json': '0abeb1a6e4b942e4e43ef8d49b7356c1f6204bcb14163f24153f4d0ec5418011',
    f'mainnet/{SEGWIT[0]}.hex': '4d9ba5d97cdb82847551d6a487e3479958b97b1c5cc9549c932c9bc261a2bf8e',
    f'mainnet/{SEGWIT[0]}.json': '66d8bb646ababf838e207b78a0c6878c0d136709ea768b71d5ef78dc2a642f51',
    f'mainnet/{SEGWIT[1]}.hex': '4864b8eda5c76a9ae3235130b6a0370f59533a4c20962c89ec1d4ce67d1fcecc',
    f'mainnet/{SEGWIT[1]}.json': '49f1314013f8bffd9c13f8a45d54983a5f9b317c96d4b8dba1330e11a353a63b',
    f'mainnet/{SEGWIT[2]}.hex': '2098bb60eb4f47c230213ba7bd72a0b00dfcd6d295b59469b64ef1de8d72640d',
    f'mainnet/{SEGWIT[2]}.json': '8a9d44ab04ac3e4cf22472446690fcc5fad2b69ef9fb2d8711809338c51270b1',
}
LEGACY = ['23b397edccd3740a74adb603c9756370fafcde9bcc4483eb271ecad09a94dd63',
          'c99c49da4c38af669dea436d3e73780dfdb6c1ecf9958baa52960e8baee30e73',
          'f7fdd091fa6d8f5e7a8c2458f5c38faffff2d3f1406b6e4fe2c99dcc0d2d1cbb']


def pinned(name):
    data = (UP / name).read_bytes()
    assert hashlib.sha256(data).hexdigest() == PINS[name], name
    return data.decode()


def constant(name, text):
    value = re.search(rf'{name}\{{([^}}]*)\}}', text).group(1).replace("'", '')
    return eval(value.replace('COIN', '100000000'), {})


def vsize_sigops(weight, cost, per):
    """GetSigOpsAdjustedWeight and GetVirtualTransactionSize, from policy.cpp."""
    assert 'return std::max(weight, sigop_cost * bytes_per_sigop);' in pinned('bitcoin-core/policy.cpp')
    return (max(weight, cost * per) + 3) // 4


def milli(v):
    return f'{v // 1000}.{v % 1000:03d}'


def compact_size(n):
    return len(T.compact(n))


def transcript():
    max_money = constant('MAX_MONEY', pinned('bitcoin-core/amount.h'))
    block = constant('MAX_BLOCK_WEIGHT', pinned('bitcoin-core/consensus.h'))
    policy = pinned('bitcoin-core/policy.h')
    standard, per_sigop = constant('MAX_STANDARD_TX_WEIGHT', policy), constant('DEFAULT_BYTES_PER_SIGOP', policy)
    assert (max_money, block, standard, per_sigop) == (2100000000000000, 4000000, 400000, 20)
    rows = {}
    for row in json.loads(pinned('bitcoin-core/tx_valid.json')):
        if len(row) == 3 and isinstance(row[1], str):
            raw = bytes.fromhex(row[1])
            try:
                rows[T.txid(T.unserialize(raw, True))[::-1].hex()] = raw
            except T.Refused:
                pass
    raws = [rows[t] for t in LEGACY] + [bytes.fromhex(pinned(f'mainnet/{t}.hex').strip()) for t in SEGWIT]
    metas = [json.loads(pinned(f'mainnet/{t}.json')) for t in SEGWIT]
    out = []
    p = out.append
    p('weight = 4 x stripped + witness bytes; vsize = weight / 4, rounded up')
    for i, raw in enumerate(raws):
        tx = T.unserialize(raw, True)
        stripped = len(T.serialize(tx, False))
        w = T.weight(tx)
        published = metas[i - 3]['weight'] if i >= 3 else 4 * len(raw)
        p(f'{i}: {len(raw)} bytes, stripped {stripped}, witness {len(raw) - stripped}, weight {w} (published {published}), vsize {(w + 3) // 4}')
    raw = raws[3]
    stripped = len(T.serialize(T.unserialize(raw, True), False))
    p('the witness discount on f5864806')
    p(f'  {stripped} stripped bytes x 4 = {4 * stripped}, plus {len(raw) - stripped} witness bytes x 1 = {len(raw) - stripped}: weight {3 * stripped + len(raw)}')
    p(f'  at 4 units for every byte it would weigh {4 * len(raw)} (vsize {len(raw)})')
    p('fees')
    for i in range(3):
        tx = T.unserialize(raws[3 + i], True)
        meta = metas[i]
        ins = sum(v['prevout']['value'] for v in meta['vin'])
        outs = sum(int.from_bytes(o['value'], 'little', signed=True) for o in tx['outs'])
        fee = ins - outs
        w = T.weight(tx); v = (w + 3) // 4
        p(f'{3 + i}: in {ins}, out {outs}, fee {fee} (published {meta["fee"]})')
        p(f'  vsize {v}: {milli(fee * 1000 // v)} sat/vB, {fee * 1000 // v} sat/kvB, {fee * 1000 // w} sat/kWU')
        grow = w // 20 + 1
        p(f'  sigop cost {meta["sigops"]} (published): vsize with sigops {vsize_sigops(w, meta["sigops"], per_sigop)}; it would grow at {grow} sigops ({grow} x 20 > {w})')
    p('sigops beyond weight / 20 set the vsize')
    for c in [1, 22, 23, 100]:
        p(f'  weight 442, sigop cost {c}: vsize {vsize_sigops(442, c, per_sigop)}')
    p('refusals')
    p('  spent value 506077, one less than the output: ok false')
    p('  two values for one input: ok false')
    p('  a value above MAX_MONEY: ok false')
    # Estimates: every input is 32 + 4 + 4 bytes plus its scriptSig; the
    # longest low-S DER signature is 71 bytes, then the sighash byte.
    sig, key, schnorr = 72, 33, 64
    p2pkh = 4 * (40 + 1 + (1 + sig + 1 + key))
    p2wpkh = 4 * 41 + 1 + (1 + sig) + (1 + key)
    p2tr = 4 * 41 + 1 + (1 + schnorr)
    out_size = lambda n: 8 + compact_size(n) + n
    def estimate(kinds, nout, nbytes):
        w = 4 * (4 + compact_size(len(kinds)) + compact_size(nout) + nbytes + 4) + sum(kinds)
        return w + 2 if kinds else w
    p('estimates before signing, with the longest signatures')
    p(f'  input weights: P2PKH {p2pkh}, P2WPKH {p2wpkh}, P2TR key path {p2tr}')
    p(f'  f5864806, one P2WPKH input to one P2SH output: estimate {estimate([p2wpkh], 1, out_size(23))}, signed {T.weight(T.unserialize(raws[3], True))}')
    p(f'  33e794d0, one P2TR input to OP_RETURN and P2TR outputs: estimate {estimate([p2tr], 2, out_size(49) + out_size(34))}, signed {T.weight(T.unserialize(raws[4], True))}')
    w = estimate([p2wpkh, p2wpkh], 2, 2 * out_size(22)); v = (w + 3) // 4
    p(f'  two P2WPKH inputs to two P2WPKH outputs: weight {w}, vsize {v}')
    for rate in [1000, 10000, 52306]:
        p(f'    at {milli(rate)} sat/vB the fee is {-(-rate * v // 1000)}')
    p(f'a block holds {block} weight units: {block // 4} stripped bytes at most')
    p(f'  {block // 442} transactions the size of f5864806, or {block // (4 * 193)} at 4 units a byte')
    p(f'  Core relays transactions up to {standard} weight units ({standard // 4} vB)')
    return '\n'.join(out) + '\n'


def main():
    want = transcript()
    assert FIXTURE.read_text() == want, 'the fixture differs from the Python transcript'
    print(f'fees fixture: {want.count(chr(10))} lines agree with Python')
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            run = subprocess.run([coax, 'run', EXAMPLE] + mode, cwd=REPO, capture_output=True, text=True)
            assert run.returncode == 0 and run.stdout == want, f'native {mode or "debug"} output differs'
            print(f'native {"release" if mode else "debug"}: agrees')


if __name__ == '__main__':
    main()
