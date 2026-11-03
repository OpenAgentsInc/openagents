#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: copies Bitcoin Core's published coin-selection test cases and decides each with a Python transliteration outside Coax)
"""Writes coinselect_vectors.coax for Bitcoin chapter 95 from Bitcoin
Core's pinned wallet tests (coinselection_tests.cpp and
coinselector_tests.cpp at revision ddea9196):

  python3 -I coax/tests/bitcoin/make_coinselect_vectors.py          # check the file
  python3 -I coax/tests/bitcoin/make_coinselect_vectors.py --write  # rewrite it

Each case is copied from a named test: bnb_test (at all ten FEERATES),
bnb_exhaustion_with_solution_test, bnb_feerate_sensitivity_test,
coin_grinder_tests, srd_test (all ten feerates), knapsack_solver_test,
ApproximateBestSubset, waste_test and effective_value_test. Every case's
title or statement is checked to be in the pinned test file, and every
published result (input values, attempt counts, totals, failures) is
recomputed by coinselect_reference.py before the file is written. The
cases whose Core results hold for every random draw (Knapsack, SRD) run
with the fixed seed 0x95...95.
"""
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import addrman as A  # noqa: E402
import coinselect_reference as R  # noqa: E402

OUT = HERE / 'coinselect_vectors.coax'
COIN, CENT, FEE = R.COIN, R.CENT, R.get_fee
FEERATES = [0, 1, 99, 100, 315, 1_000, 2_345, 10_292, 59_764, 1_500_000]
TESTS = R.pinned('bitcoin-core/coinselection_tests.cpp')
OLD = R.pinned('bitcoin-core/coinselector_tests.cpp')
CASES = []


def in_tests(text, src):
    # One of Core's titles has a narrow no-break space (U+202F) before "sats"; the names here are ASCII.
    assert text.replace('21k sats', '21k\u202fsats') in src, text


def spec(coins):
    """[(value, vsize, count)] as "value/vsize*count,..."."""
    return ','.join(f'{v}/{s}*{c}' for v, s, c in coins)


def run(c):
    pool = []
    for v, s, n in c['coins']:
        pool += [R.coin(v, s, c['feerate'], c['long_term']) for _ in range(n)]
    rng = A.Rng(R.SEED)
    a = c['algo']
    if a == R.BNB: return R.bnb(pool, c['target'], c['extra'], c['max_weight'])
    if a == R.CG: return R.coin_grinder(pool, c['target'], c['extra'], c['max_weight'])
    if a == R.SRD: return R.srd(pool, c['target'], c['extra'], c['max_weight'], rng)
    return R.knapsack(pool, c['target'], c['extra'], c['max_weight'], rng)


def case(name, algo, feerate, long_term, coins, target, extra, max_weight=R.MAX_STANDARD_TX_WEIGHT,
         code=R.OK, want=None, tries=-1, count=-1, total=-1, src=TESTS):
    in_tests(name, src)
    c = dict(name=name, algo=algo, feerate=feerate, long_term=long_term, coins=coins, target=target, extra=extra,
             max_weight=max_weight, code=code, want=want, tries=tries, count=count, total=total)
    r = run(c)
    assert r.code == code, (name, feerate, r.code)
    if code == R.OK:
        vals = sorted(g.value for g in r.groups)
        if want is not None: assert vals == sorted(want), (name, feerate, vals, want)
        if tries >= 0: assert r.tries == tries, (name, feerate, r.tries, tries)
        if count >= 0: assert r.count() == count, (name, r.count(), count)
        if total >= 0: assert r.value() == total, (name, r.value(), total)
        if algo == R.SRD:
            assert r.effective() >= target + extra + R.CHANGE_LOWER and r.weight() <= max_weight, name
    CASES.append(c)


def bnb_cases():
    """coinselection_tests.cpp bnb_test, at every feerate."""
    for f in FEERATES:
        fee68 = FEE(f, 68)
        coc = FEE(3000, 68) + FEE(f, 31)          # init_cs_params: min_viable_change + m_change_fee
        eff = lambda xs, s=68: [(x + FEE(f, s), s, 1) for x in xs]
        val = lambda xs, s=68: [x + FEE(f, s) for x in xs]
        b = lambda name, coins, target, **k: case(name, R.BNB, f, 10_000, coins, target, coc, **k)
        b('Empty UTXO pool', [], 1 * CENT, code=R.INSUFFICIENT)
        pool = eff([1 * CENT, 3 * CENT, 5 * CENT])
        b('Select smallest UTXO', pool, 1 * CENT, want=val([1 * CENT]), tries=3)
        b('Select middle UTXO', pool, 3 * CENT, want=val([3 * CENT]), tries=3)
        b('Select biggest UTXO', pool, 5 * CENT, want=val([5 * CENT]), tries=2)
        b('Select two UTXOs', pool, 4 * CENT, want=val([1 * CENT, 3 * CENT]), tries=4)
        b('Select all UTXOs', pool, 9 * CENT, want=val([1 * CENT, 3 * CENT, 5 * CENT]), tries=5)
        b('Select upper bound', pool, 4 * CENT - coc, want=val([1 * CENT, 3 * CENT]), tries=4)
        b('Overshoot upper bound', pool, 4 * CENT - coc - 1, code=R.INSUFFICIENT)
        b('Select max weight', pool, 4 * CENT, max_weight=4 * 2 * 68, want=val([1 * CENT, 3 * CENT]), tries=4)
        b('Exceed max weight', pool, 4 * CENT, max_weight=4 * 2 * 68 - 1, code=R.MAX_WEIGHT)
        b('Smallest combination too big', pool, CENT // 2, code=R.INSUFFICIENT)
        b('No UTXO combination in target window', pool, 7 * CENT, code=R.INSUFFICIENT)
        b('Select more than available', pool, 10 * CENT, code=R.INSUFFICIENT)
        clones = eff([2 * CENT, 7 * CENT, 7 * CENT]) + [(5 * CENT + fee68, 68, 50_000)]
        b('Skip equivalent input sets', clones, 16 * CENT, want=val([2 * CENT, 7 * CENT, 7 * CENT]), tries=16)
        small = [1 * CENT + i for i in range(8)]
        dopp = small + [1 * CENT + coc + i for i in range(8, 17)]
        b('Combine smallest 8 of 17 unique UTXOs', eff(dopp), 8 * CENT, want=val(small), tries=51_765)
        dopp.append(1 * CENT + coc + 17)
        b('Combine smallest 8 of 18 unique UTXOs', eff(dopp), 8 * CENT, want=val(small), tries=87_957)
        dopp.append(1 * CENT + coc + 18)
        b('Exhaust looking for smallest 8 of 19 unique UTXOs', eff(dopp), 8 * CENT, code=R.INSUFFICIENT)
    # bnb_exhaustion_with_solution_test: a solution, eight inputs, but the search is cut off.
    in_tests('BOOST_CHECK_EQUAL(result->GetSelectionsEvaluated(), 100\'000U);', TESTS)
    coc = FEE(3000, 68) + FEE(5000, 31)
    case('Exhaust with early solution', R.BNB, 5000, 10_000, [(100_000 + i + FEE(5000, 68), 68, 1) for i in range(19)],
         800_000, coc, count=8, tries=100_000)
    # bnb_feerate_sensitivity_test
    low = lambda xs, s=68: [(x + FEE(5000, s), s, 1) for x in xs]
    high = lambda xs, s=68: [(x + FEE(25_000, s), s, 1) for x in xs]
    lv = lambda xs, s=68: [x + FEE(5000, s) for x in xs]
    hv = lambda xs, s=68: [x + FEE(25_000, s) for x in xs]
    base = [2 * CENT, 3 * CENT, 5 * CENT, 10 * CENT]
    coc_low, coc_high = FEE(3000, 68) + FEE(5000, 31), FEE(3000, 68) + FEE(25_000, 31)
    case('Select many inputs at low feerates', R.BNB, 5000, 10_000, low(base), 10 * CENT, coc_low, want=lv([2 * CENT, 3 * CENT, 5 * CENT]), tries=6)
    case('Select one input at high feerates', R.BNB, 25_000, 10_000, high(base), 10 * CENT, coc_high, want=hv([10 * CENT]), tries=5)
    case('Prefer two heavy inputs over two light inputs at low feerates', R.BNB, 5000, 10_000,
         low(base) + low([6 * CENT, 7 * CENT], 500), 13 * CENT, coc_low, want=lv([6 * CENT, 7 * CENT], 500), tries=18)
    case('Prefer two light inputs over two heavy inputs at high feerates', R.BNB, 25_000, 10_000,
         high(base) + high([6 * CENT, 7 * CENT], 500), 13 * CENT, coc_high, want=hv([3 * CENT, 10 * CENT]), tries=9)


def cg_cases():
    """coinselector_tests.cpp coin_grinder_tests 1) to 8)."""
    g = lambda name, coins, target, max_weight, **k: case(name, R.CG, 5000, 2000, coins, target, k.pop('extra', CENT), max_weight, src=OLD, **k)
    g('1) Insufficient funds, select all provided coins and fail', [(COIN, -1, 10), (2 * COIN, -1, 10)], int(49.5 * COIN), 10_000, code=R.INSUFFICIENT)
    g('2) Test max weight exceeded', [(COIN, 68, 10), (2 * COIN, 68, 10)], int(29.5 * COIN), 3000, code=R.MAX_WEIGHT)
    g('3) Test that the lowest-weight solution is found when some combinations would exceed the allowed weight',
      [(33_000_000, 68, 60), (2 * COIN, 68, 10)], 2_533_000_000, 10_000, want=[2 * COIN] * 10 + [33_000_000] * 17, tries=37)
    g('4) Test that two less valuable UTXOs with a combined lower weight are preferred over a more valuable heavier UTXO',
      [(2 * COIN, 148, 1), (COIN, 68, 2)], 190_000_000, 400_000, want=[COIN, COIN], tries=3)
    mixed = []
    for j in range(5):
        mixed += [((3 + 3 * j) * COIN, 350, 1), ((2 + 3 * j) * COIN, 250, 1), ((1 + 3 * j) * COIN, 150, 1)]
    g('5) Test finding a solution in a UTXO pool with mixed weights', mixed, 30 * COIN, 400_000,
      want=[14 * COIN, 13 * COIN, 4 * COIN], tries=92)
    clones = [(4 * COIN, 100, 1), (3 * COIN, 100, 1), (2 * COIN, 100, 1), (COIN, 100, 1),
              (8 * COIN, 1000, 100), (7 * COIN, 800, 100), (6 * COIN, 600, 100), (5 * COIN, 400, 100)]
    g('6) Test that the lightest solution among many clones is found', clones, 990_000_000, 400_000,
      want=[4 * COIN, 3 * COIN, 2 * COIN, COIN], tries=38)
    tiny = [(180_000_000, 2500, 1), (COIN, 1000, 2)] + [(1_000_000 + j, 110, 1) for j in range(100)]
    g('7) Test that lots of tiny UTXOs can be skipped if they are too heavy while there are enough funds in lookahead',
      tiny, 190_000_000, 40_000, want=[COIN, COIN], tries=7)
    in_tests('dummy_params.m_min_change_target = 0;', OLD)
    dopp = [(COIN + i, 96 + i, 1) for i in range(18)]
    case('8) Test input set that has a solution will not find a solution before reaching the attempt limit', R.CG, 0, 2000,
         dopp, 8 * COIN, 0, 3200, want=[COIN + i for i in range(8)], tries=87_525, src=OLD)
    case('Adding one more doppelganger causes the attempt limit to be reached before finding a solution.', R.CG, 0, 2000,
         dopp + [(COIN + 18, 96 + 18, 1)], 8 * COIN, 0, 3200, code=R.MAX_WEIGHT, src=OLD)


def srd_cases():
    """coinselection_tests.cpp srd_test, at every feerate."""
    for f in FEERATES:
        cf = FEE(f, 31)
        eff = lambda xs, s=68, n=1: [(x + FEE(f, s), s, n) for x in xs]
        s = lambda name, coins, target, **k: case(name, R.SRD, f, 10_000, coins, target, cf, **k)
        s('Empty UTXO pool', [], 1 * CENT, code=R.INSUFFICIENT)
        pool = eff([1 * CENT, 3 * CENT, 5 * CENT])
        for name, t in [('Select 21k sats', 21_000), ('Select 1 CENT', 1 * CENT), ('Select 3.125 CENT', 3_125_000),
                        ('Select 4 CENT', 4 * CENT), ('Select 7 CENT', 7 * CENT),
                        ('Create minimum change', 9 * CENT - cf - R.CHANGE_LOWER)]:
            s(name, pool, t)
        s('Undershoot minimum change by one sat', pool, 9 * CENT - cf - R.CHANGE_LOWER + 1, code=R.INSUFFICIENT)
        s('Spend more than available', pool, 9 * CENT + 1, code=R.INSUFFICIENT)
        s('Spend everything', pool, 9 * CENT, code=R.INSUFFICIENT)
        pool = pool + eff([5 * CENT], n=100) + eff([7 * CENT], n=3)
        s('Select most valuable UTXOs for acceptable weight', pool, 20 * CENT, max_weight=4 * 4 * 67)
        s('No acceptable weight possible', pool, 25 * CENT, max_weight=4 * 3 * 68, code=R.MAX_WEIGHT)
        # Core's test passes utxo_pool here, not the mixed-weight pool it builds.
        s('Tie-break same effective value with lower weight', pool, 9 * CENT, max_weight=4 * 3 * 67)


def knapsack_cases():
    """coinselector_tests.cpp knapsack_solver_test with filter_confirmed
    (every coin eligible), feerate 0 and change target CENT, and the
    ApproximateBestSubset test."""
    k = lambda name, coins, target, **kw: case(name, R.KNAPSACK, 0, 0, [(v, -1, n) for v, n in coins], target, CENT, src=OLD, **kw)
    k('with an empty wallet we can\'t even pay one cent', [], CENT, code=R.INSUFFICIENT)
    k('but we can find a new 1 cent', [(CENT, 1)], CENT, total=CENT, count=1)
    k('we can make 3 cents of new coins', [(CENT, 1), (2 * CENT, 1)], 3 * CENT, total=3 * CENT, count=2)
    five = [(CENT, 1), (2 * CENT, 1), (5 * CENT, 1), (10 * CENT, 1), (20 * CENT, 1)]
    k('and we can make 38 cents if we accept all new coins', five, 38 * CENT, total=38 * CENT, count=5)
    k('try making 34 cents from 1,2,5,10,20 - we can\'t do it exactly', five, 34 * CENT, total=35 * CENT, count=3)
    k('when we try making 7 cents, the smaller coins (1,2,5) are enough.  We should see just 2+5', five, 7 * CENT, total=7 * CENT, count=2)
    k('when we try making 8 cents, the smaller coins (1,2,5) are exactly enough.', five, 8 * CENT, total=8 * CENT, count=3)
    k('when we try making 9 cents, no subset of smaller coins is enough, and we get the next bigger coin (10)', five, 9 * CENT, total=10 * CENT, count=1)
    b = [(6 * CENT, 1), (7 * CENT, 1), (8 * CENT, 1), (20 * CENT, 1), (30 * CENT, 1)]
    k('check that we have 71 and not 72', b, 71 * CENT, total=71 * CENT, count=5)
    k('check that we have 71 and not 72', b, 72 * CENT, code=R.INSUFFICIENT)
    k('now try making 16 cents.  the best smaller coins can do is 6+7+8 = 21; not as good at the next biggest coin, 20', b, 16 * CENT, total=20 * CENT, count=1)
    b = b + [(5 * CENT, 1)]
    k('now if we try making 16 cents again, the smaller coins can make 5+6+7 = 18 cents, better than the next biggest coin, 20', b, 16 * CENT, total=18 * CENT, count=3)
    b = b + [(18 * CENT, 1)]
    k('and now if we try making 16 cents again, the smaller coins can make 5+6+7 = 18 cents, the same as the next biggest coin, 18', b, 16 * CENT, total=18 * CENT, count=1)
    k('now try making 11 cents.  we should get 5+6', b, 11 * CENT, total=11 * CENT, count=2)
    b = b + [(COIN, 1), (2 * COIN, 1), (3 * COIN, 1), (4 * COIN, 1)]
    k('check that the smallest bigger coin is used', b, 95 * CENT, total=COIN, count=1)
    k('check that the smallest bigger coin is used', b, 195 * CENT, total=2 * COIN, count=1)
    frac = [(CENT * i // 10, 1) for i in range(1, 6)]
    k('try making 1 * CENT from the 1.5 * CENT', frac, CENT, total=CENT)
    frac = frac + [(1111 * CENT, 1)]
    k('try making 1 from 0.1 + 0.2 + 0.3 + 0.4 + 0.5 + 1111 = 1112.5', frac, CENT, total=CENT)
    frac = frac + [(CENT * 6 // 10, 1), (CENT * 7 // 10, 1)]
    k('and try again to make 1.0 * CENT', frac, CENT, total=CENT)
    k('run the \'mtgox\' test', [(50_000 * COIN, 20)], 500_000 * COIN, total=500_000 * COIN, count=10)
    k('sometimes it will fail, and so we use the next biggest coin:', [(CENT * 5 // 10, 1), (CENT * 6 // 10, 1), (CENT * 7 // 10, 1), (1111 * CENT, 1)],
      CENT, total=1111 * CENT, count=1)
    k('but sometimes it\'s possible, and we use an exact subset (0.4 + 0.6 = 1.0)', [(CENT * 4 // 10, 1), (CENT * 6 // 10, 1), (CENT * 8 // 10, 1), (1111 * CENT, 1)],
      CENT, total=CENT, count=2)
    small = [(CENT * 5 // 100, 1), (CENT, 1), (CENT * 100, 1)]
    k('trying to make 100.01 from these three coins', small, CENT * 10001 // 100, total=CENT * 10105 // 100, count=3)
    k('but if we try to make 99.9, we should take the bigger of the two small coins to avoid small change', small, CENT * 9990 // 100, total=101 * CENT, count=2)
    amt = 1500
    while amt < COIN:
        if amt - 2000 < CENT:
            n = -(-(2000 + CENT) // amt)
            k('test with many inputs', [(amt, 676)], 2000, total=amt * n, count=n)
        else:
            k('test with many inputs', [(amt, 676)], 2000, total=amt, count=1)
        amt *= 10
    k('Test vValue sort order', [(1000 * COIN, 1000), (3 * COIN, 1)], 1003 * COIN, total=1003 * COIN, count=2)


WASTE = []


def waste_cases():
    """coinselector_tests.cpp waste_test: two coins of 1 and 2 bitcoin."""
    fee, mvc, cc, cf, diff, inp, target, excess = 100, 300, 125, 30, 40, 3 * COIN, 2 * COIN, 80
    exact = inp - fee * 2
    for name, t, f, ltf, change_cost, want in [
            ('Waste with change is the change cost and difference between fee and long term fee', target, fee, fee - diff, cc, diff * 2 + cc),
            ('With long term fee greater than fee, waste should be less than when long term fee is less than fee', target, fee, fee + diff, cc, -diff * 2 + cc),
            ('Waste without change is the excess and difference between fee and long term fee', exact - excess, fee, fee - diff, cc, diff * 2 + excess),
            ('Waste without change is the excess and difference between fee and long term fee', exact - excess, fee, fee + diff, cc, -diff * 2 + excess),
            ('Waste with change and fee == long term fee is just cost of change', target, fee, fee, cc, cc),
            ('Waste without change and fee == long term fee is just the excess', exact - excess, fee, fee, cc, excess),
            ('Waste is 0 when fee == long_term_fee, no change, and no excess', exact, fee, fee, cc, 0),
            ('Waste is 0 when (fee - long_term_fee) == (-cost_of_change), and no excess', target, fee, fee + diff, diff * 2, 0),
            ('Waste is 0 when (fee - long_term_fee) == (-excess), no change cost', exact - diff * 2, fee, fee + diff, cc, 0),
            ('Negative waste when the long term fee is greater than the current fee and the selected value == target', exact, fee, fee + diff, cc, -2 * diff),
            ('Negative waste when the long term fee is greater than the current fee and change_cost < - (inputs * (fee - long_term_fee))', target, fee, fee + 90, cc, -55)]:
        in_tests(name, OLD)
        r = R.Result(R.MANUAL, t)
        r.groups = [R.Group(COIN, f, ltf, 592), R.Group(2 * COIN, f, ltf, 592)]
        assert r.recalc_waste(mvc, change_cost, cf) == want, name
        WASTE.append(dict(name=name, target=t, inputs=inp, fees=2 * f, ltf=2 * ltf, mvc=mvc, cost=change_cost, cf=cf, waste=want))


def render():
    lines = ['// Generated by make_coinselect_vectors.py from Bitcoin Core\'s pinned',
             '// coinselection_tests.cpp and coinselector_tests.cpp (coax/tests/vectors/',
             '// upstream/, listed in SOURCES.md). Published results, copied; each',
             '// recomputed in Python.',
             '// One selection: the algorithm, the pool ("value/vsize*count,..." at',
             '// feerate and long_term sat/kvB), the target, extra (BnB\'s cost of change,',
             '// CoinGrinder\'s and Knapsack\'s change target, SRD\'s change fee), the',
             '// weight limit, the code, and what Core checks: the input values, the',
             '// attempts, the coin count and the total (-1 or empty: not checked).',
             'CsCase :: struct { name: string; algo: int; feerate: int; long_term: int; pool: string; target: int; extra: int;',
             '                   max_weight: int; code: int; want: string; tries: int; count: int; total: int; }',
             f'CS_CASES :: [{len(CASES)}] CsCase.[']
    for c in CASES:
        want = ','.join(str(v) for v in sorted(c['want'])) if c['want'] is not None else ''
        lines.append(f'  CsCase.{{name="{c["name"]}", algo={c["algo"]}, feerate={c["feerate"]}, long_term={c["long_term"]}, '
                     f'pool="{spec(c["coins"])}", target={c["target"]}, extra={c["extra"]}, max_weight={c["max_weight"]}, '
                     f'code={c["code"]}, want="{want}", tries={c["tries"]}, count={c["count"]}, total={c["total"]}}},')
    lines += ['];',
              '// waste_test: two coins, 1 and 2 bitcoin, with their fees and long-term',
              '// fees, against a target; the change parameters and the waste.',
              'CsWaste :: struct { name: string; target: int; inputs: int; fees: int; long_term_fees: int;',
              '                    min_viable_change: int; change_cost: int; change_fee: int; waste: int; }',
              f'CS_WASTE :: [{len(WASTE)}] CsWaste.[']
    for w in WASTE:
        lines.append(f'  CsWaste.{{name="{w["name"]}", target={w["target"]}, inputs={w["inputs"]}, fees={w["fees"]}, '
                     f'long_term_fees={w["ltf"]}, min_viable_change={w["mvc"]}, change_cost={w["cost"]}, change_fee={w["cf"]}, waste={w["waste"]}}},')
    lines.append('];')
    return '\n'.join(lines) + '\n'


def main():
    R.check_core()
    bnb_cases(); cg_cases(); srd_cases(); knapsack_cases(); waste_cases()
    text = render()
    if '--write' in sys.argv:
        OUT.write_text(text)
        print(f'wrote {OUT.name}: {len(CASES)} selection cases, {len(WASTE)} waste cases')
    else:
        assert OUT.read_text() == text, f'{OUT.name} differs; run with --write'
        print(f'{OUT.name} matches: {len(CASES)} selection cases, {len(WASTE)} waste cases, every published result recomputed')


if __name__ == '__main__':
    main()
