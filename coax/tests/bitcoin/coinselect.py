#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: coinselect_reference.py, a Python transliteration of Bitcoin Core's coinselection.cpp, decides every line outside Coax)
"""The cross-check of Bitcoin chapter 95. It writes the transcript the
example must print from Python: the wallet's effective values, the dust
thresholds, four payments run through coinselect_reference.py's Branch
and Bound, Knapsack, Single Random Draw and CoinGrinder with Core's
waste metric, and every case of make_coinselect_vectors.py:

  python3 -I coax/tests/bitcoin/coinselect.py                       # compare with the fixture
  python3 -I coax/tests/bitcoin/coinselect.py --coax coax/bin/coax  # also run the example
"""
import subprocess, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import addrman as A  # noqa: E402
import coinselect_reference as R  # noqa: E402
import make_coinselect_vectors as V  # noqa: E402

REPO = HERE.parents[2]
FIXTURE = REPO / 'bitcoin/fixtures/coinselect.out'
EXAMPLE = 'bitcoin/examples/coinselect.coax'
NAMES = 'ABCDEFGHIJ'
VALUES = [1000000, 250000, 3500, 600000, 50000, 2000000, 120000, 75000, 400000, 9000]
SIZES = [68, 58, 148, 68, 58, 68, 148, 58, 68, 68]
KINDS = ['P2WPKH', 'P2TR', 'P2PKH', 'P2WPKH', 'P2TR', 'P2WPKH', 'P2PKH', 'P2TR', 'P2WPKH', 'P2WPKH']
LONG_TERM, DISCARD, P2TR_OUTPUT = 10000, 3000, 43


def pay(p_, title, payment, feerate, seed_byte):
    p_(f'{title}: {payment} sat to a P2TR address at {feerate} sat/kvB')
    pool = [R.coin(VALUES[i], SIZES[i], feerate, LONG_TERM, ident=i) for i in range(10)]
    rng = A.Rng(bytes([seed_byte]) + bytes(31))
    p = R.Params(feerate, LONG_TERM, DISCARD, b'\x00\x14' + bytes(20), 68, 10 + 1 + P2TR_OUTPUT, payment, 1, rng)
    target = payment + R.get_fee(feerate, p.tx_noinputs_size)
    p_(f'  target {target} (the payment and {target - payment} sat for the transaction without inputs)')
    p_(f'  change fee {p.change_fee}, cost of change {p.cost_of_change}, smallest change {p.min_viable_change}, change target {p.min_change_target}')
    win, ran = R.choose(pool, target, p, R.MAX_STANDARD_TX_WEIGHT, rng)
    for a in range(4):
        if a == R.CG and p.feerate <= 3 * p.long_term:
            p_('  cg: not run below three times the long-term feerate')
            continue
        r = ran[a]
        if r.code != R.OK:
            p_(f'  {R.ALGOS[a]}: no selection')
            continue
        r.target = target
        names = ' '.join(NAMES[i] for i in sorted(g.ident for g in r.groups))
        p_(f'  {R.ALGOS[a]}: {names}, effective {r.effective()}, weight {r.weight()}, '
           f'change {r.change(p.min_viable_change, p.change_fee)}, waste {r.recalc_waste(p.min_viable_change, p.cost_of_change, p.change_fee)}')
    if win.code != R.OK:
        p_(f'  no selection: code {win.code}')
        return
    change = win.change(p.min_viable_change, p.change_fee)
    vsize = p.tx_noinputs_size + win.weight() // 4 + (p.change_output_size if change > 0 else 0)
    fee = win.value() - payment - change
    tail = f' and {change} change' if change > 0 else ', no change'
    p_(f'  chosen: {R.ALGOS[win.algo]}, waste {win.waste}; the transaction: {win.value()} in, {payment} out{tail}, '
       f'fee {fee}, {vsize} vB, {fee * 1000 // vsize} sat/kvB')


def transcript():
    lines = []
    p_ = lines.append
    p_("The wallet's coins, at 1, 10 and 100 sat/vB:")
    for i in range(10):
        eff = ' '.join(str(R.coin(VALUES[i], SIZES[i], r, LONG_TERM).amount()) for r in (1000, 10000, 100000))
        p_(f'  {NAMES[i]}: {VALUES[i]} sat {KINDS[i]}, effective {eff}')
    p2pkh, p2wpkh, p2tr = b'\x76\xa9\x14' + bytes(22), b'\x00\x14' + bytes(20), b'\x51\x20' + bytes(32)
    p_(f'Dust at the discard feerate, 3 sat/vB: P2PKH {R.dust_threshold(p2pkh, DISCARD)}, '
       f'P2WPKH {R.dust_threshold(p2wpkh, DISCARD)}, P2TR {R.dust_threshold(p2tr, DISCARD)}')
    pay(p_, 'Pay 830,000 at 35 sat/vB', 830000, 35000, 1)
    pay(p_, 'Pay 995,730 at 35 sat/vB', 995730, 35000, 2)
    pay(p_, 'Pay 830,000 at 2 sat/vB', 830000, 2000, 3)
    pay(p_, 'Pay 4,600,000 at 35 sat/vB', 4600000, 35000, 4)
    p_("Bitcoin Core's coin-selection cases:")
    V.bnb_cases(); V.cg_cases(); V.srd_cases(); V.knapsack_cases(); V.waste_cases()   # each asserts Core's result
    for a in range(4):
        n = sum(1 for c in V.CASES if c['algo'] == a)
        p_(f"  {R.ALGOS[a]}: {n} of {n} give Core's result")
    p_(f"  waste: {len(V.WASTE)} of {len(V.WASTE)} give Core's result")
    return '\n'.join(lines) + '\n'


def main():
    R.check_core()
    text = transcript()
    if '--write' in sys.argv:
        FIXTURE.write_text(text)
    assert FIXTURE.read_text() == text, 'the fixture differs from Python'
    print(f'coinselect.py: {text.count(chr(10))} lines agree with Python')
    if '--coax' in sys.argv:
        coax = sys.argv[sys.argv.index('--coax') + 1]
        for mode in ([], ['--release']):
            out = subprocess.run([coax, 'run', EXAMPLE] + mode, cwd=REPO, capture_output=True, text=True, check=True).stdout
            assert out == text, f'the example differs from Python ({mode or "debug"})'
            print(f'coinselect.coax ({"release" if mode else "debug"}) prints the same transcript')


if __name__ == '__main__':
    main()
