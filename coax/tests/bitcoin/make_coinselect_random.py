#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: coinselect_reference.py, a Python transliteration of Bitcoin Core's coinselection.cpp, decides every selection outside Coax)
"""Writes coinselect_random.coax for Bitcoin chapter 95: wallets drawn
with a fixed seed, each run through coinselect_reference.py's Branch and
Bound, CoinGrinder, Single Random Draw and Knapsack, and through
ChooseSelectionResult, with every result written down in full: the
code, the attempts, the coins and their sums. coinselect_checks.coax
runs the same wallets through coinselect.coax and compares.

  python3 -I coax/tests/bitcoin/make_coinselect_random.py          # check the file
  python3 -I coax/tests/bitcoin/make_coinselect_random.py --write  # rewrite it

The wallets lean on the places a selector can go wrong: coins whose
effective value is zero or negative, coins of equal value (BnB's and
CoinGrinder's skip), coins of equal value and different weight (the sort
tie-breaks), coins of unknown size, targets a subset meets exactly or at
the top of BnB's window or one satoshi past it, weight limits at a
subset's weight and one below, and feerates on both sides of three times
the long-term feerate.
"""
import inspect, random, sys, zlib
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import addrman as A  # noqa: E402
import coinselect_reference as R  # noqa: E402

OUT = HERE / 'coinselect_random.coax'
FEE = R.get_fee
SIZES = [68, 58, 148, 91, 68, 68, 58, -1, 272]
FEERATES = [0, 1, 999, 1000, 3000, 5000, 10000, 29999, 30000, 30001, 45000, 100000]


def draw_pool(g, feerate, long_term):
    n = g.randrange(1, 13)
    coins = []
    for _ in range(n):
        kind = g.randrange(10)
        size = g.choice(SIZES)
        if kind == 0 and coins:
            v, s = g.choice(coins)                                 # the same coin again
        elif kind == 1 and coins:
            v, s = g.choice(coins)[0], g.choice(SIZES)             # the same value, another weight
        elif kind == 2 and size > 0:
            v, s = FEE(feerate, size) + g.randrange(-2, 3), size   # effective value near zero
            v = max(v, 1)
        elif kind == 3 and coins:
            v0, s0 = g.choice(coins)                               # the same effective value, another weight
            s = g.choice([x for x in SIZES if x > 0])
            v = v0 - (FEE(feerate, s0) if s0 > 0 else 0) + FEE(feerate, s)
            if v < 1: v, s = v0, s0
        else:
            v, s = g.choice([g.randrange(1, 3_000_000), g.randrange(1, 60_000), 1_000_000 * g.randrange(1, 9)]), size
        coins.append((v, s))
    return coins


def spec(coins):
    return ','.join(f'{v}/{s}*1' for v, s in coins)


def groups(coins, feerate, long_term):
    return [R.coin(v, s, feerate, long_term, ident=i) for i, (v, s) in enumerate(coins)]


def draw_target(g, pool, slack):
    pos = [x for x in pool if x.amount() > 0]
    sub = [x for x in pos if g.randrange(2)] or pos[:1]
    exact = sum(x.amount() for x in sub) if sub else 1
    k = g.randrange(8)
    if k == 0: return max(1, exact)
    if k == 1: return max(1, exact - slack)                         # the top of BnB's window
    if k == 2: return max(1, exact - slack - 1)                     # one past it
    if k == 3: return exact + 1
    if k == 4: return max(1, exact - g.randrange(1, 400))
    total = sum(x.amount() for x in pos)
    if k == 5: return max(1, total)
    return max(1, g.randrange(1, max(2, total + 50_000)))


def draw_weight(g, pool):
    k = g.randrange(6)
    if k < 3: return R.MAX_STANDARD_TX_WEIGHT
    sub = [x for x in pool if g.randrange(2)] or pool[:1]
    w = sum(x.weight for x in sub)
    return max(0, w - (k - 3))                                      # at a subset's weight, one and two below


def result_row(algo, seed, f, lt, coins, target, extra, max_weight, r):
    ok = r.code == R.OK
    want = ','.join(str(v) for v in sorted(x.value for x in r.groups)) if ok else ''
    completed = (1 if r.completed else 0) if ok else -1
    tries = r.tries if algo in (R.BNB, R.CG) else 0
    return dict(algo=algo, seed=seed, feerate=f, long_term=lt, pool=spec(coins), target=target, extra=extra,
                max_weight=max_weight, code=r.code, tries=tries, completed=completed,
                count=r.count() if ok else 0, value=r.value() if ok else 0, effective=r.effective() if ok else 0,
                weight=r.weight() if ok else 0, fee=r.fee() if ok else 0, ltf=r.ltf() if ok else 0, want=want)


def selections():
    g = random.Random(95_0001)
    rows = []
    for i in range(240):
        f = g.choice(FEERATES); lt = g.choice([0, 1000, 10000])
        coins = draw_pool(g, f, lt)
        pool = groups(coins, f, lt)
        algo = i % 4
        seed = i % 256
        if algo == R.BNB:
            extra = g.choice([0, 1, FEE(3000, 68) + FEE(f, 31), g.randrange(0, 5000)])
        elif algo == R.SRD:
            extra = FEE(f, 31)
        else:
            extra = g.choice([0, 1, R.CHANGE_LOWER + FEE(f, 31), g.randrange(0, 200_000)])
        target = draw_target(g, pool, extra if algo == R.BNB else 0)
        max_weight = draw_weight(g, pool)
        rng = A.Rng(bytes([seed]) + bytes(31))
        if algo == R.BNB: r = R.bnb(pool, target, extra, max_weight)
        elif algo == R.CG: r = R.coin_grinder(pool, target, extra, max_weight)
        elif algo == R.SRD: r = R.srd(pool, target, extra, max_weight, rng)
        else: r = R.knapsack(pool, target, extra, max_weight, rng)
        rows.append(result_row(algo, seed, f, lt, coins, target, extra, max_weight, r))
    return rows


# One-character changes to Core's code, as coax mutate made them to
# coinselect.coax and no other wallet here noticed: for each, wallets
# drawn until the changed code gives another result than Core's. The
# wallets join CS_RUNS with Core's result; the change itself is only the
# search's guide.
CHANGES = [
    ('CoinGrinder rounds the coins it still needs up', R.coin_grinder, 'utxo[t].amount() - 1)', 'utxo[t].amount() + 1)'),
    ('CoinGrinder rounds the coins it still needs up, not down', R.coin_grinder, 'utxo[t].amount() - 1)', 'utxo[t].amount() - 2)'),
    ('SRD drops coins only while over the limit', R.srd, 'while held and weight > max_weight', 'while held and weight >= max_weight'),
    ('ApproximateBestSubset starts each pass at zero', R.approximate_best_subset, 'total = weight = 0', 'total = 1; weight = 0'),
    ('ApproximateBestSubset allows exactly the weight limit', R.approximate_best_subset, 'weight <= max_weight', 'weight < max_weight'),
    ('ApproximateBestSubset backs out the weight too', R.approximate_best_subset, 'total -= g.amount(); weight -= g.weight;', 'total -= g.amount();'),
    ('Knapsack: a coin of exactly target + change target is a larger coin', R.knapsack, 'g.amount() < target + change_target', 'g.amount() <= target + change_target'),
    ('Knapsack: an approximation just short of target + change target takes the larger coin', R.knapsack, 'nbest < target + change_target', 'nbest <= target + change_target'),
    ('Knapsack: a subset of exactly the weight limit is kept', R.knapsack, 'if res.weight() > max_weight:', 'if res.weight() >= max_weight:'),
]
ALGO = {R.coin_grinder: R.CG, R.srd: R.SRD, R.knapsack: R.KNAPSACK, R.approximate_best_subset: R.KNAPSACK}


def changed(fn, old, new):
    """The selector with one change, built from the reference's own source."""
    ns = dict(vars(R))
    src = inspect.getsource(fn)
    assert src.count(old) == 1, old
    exec(src.replace(old, new), ns)
    if fn is R.approximate_best_subset:
        exec(inspect.getsource(R.knapsack), ns)
        return ns['knapsack']
    return ns[fn.__name__]


def run_algo(func, algo, coins, f, lt, target, extra, max_weight, seed):
    pool = groups(coins, f, lt)
    if algo == R.CG: return func(pool, target, extra, max_weight)
    return func(pool, target, extra, max_weight, A.Rng(bytes([seed]) + bytes(31)))


def same(a, b):
    return (a.code, a.tries, sorted((x.value, x.weight) for x in a.groups)) == (b.code, b.tries, sorted((x.value, x.weight) for x in b.groups))


def hunted():
    rows = []
    for name, fn, old, new in CHANGES:
        algo, alt = ALGO[fn], changed(fn, old, new)
        orig = {R.CG: R.coin_grinder, R.SRD: R.srd, R.KNAPSACK: R.knapsack}[algo]
        g = random.Random(zlib.crc32(name.encode()))
        found = 0
        for _ in range(20000):
            f = g.choice(FEERATES); lt = g.choice([0, 1000, 10000])
            coins = draw_pool(g, f, lt)
            pool = groups(coins, f, lt)
            extra = FEE(f, 31) if algo == R.SRD else g.choice([0, 1, R.CHANGE_LOWER + FEE(f, 31), g.randrange(0, 200_000)])
            target = draw_target(g, pool, 0); max_weight = draw_weight(g, pool); seed = g.randrange(256)
            a = run_algo(orig, algo, coins, f, lt, target, extra, max_weight, seed)
            if not same(a, run_algo(alt, algo, coins, f, lt, target, extra, max_weight, seed)):
                rows.append(result_row(algo, seed, f, lt, coins, target, extra, max_weight, a))
                found += 1
                if found == 2: break
        assert found == 2, name
    return rows


# Wallets made by hand for one edge each, decided by the reference too.
HAND = [
    # BnB orders equal effective values by waste: below the long-term feerate the heavier coin wastes less.
    (R.BNB, 0, 1000, 10000, [(100068, 68), (100148, 148)], 100000, 0, R.MAX_STANDARD_TX_WEIGHT),
    # BnB with coins, but not enough: it fails before searching.
    (R.BNB, 0, 1000, 10000, [(1000, 68), (2000, 68)], 5000, 0, R.MAX_STANDARD_TX_WEIGHT),
    # BnB never takes a selection whose waste passes MAX_MONEY, its starting best.
    (R.BNB, 0, 0, 0, [(R.MAX_MONEY + 2, -1)], 1, R.MAX_MONEY + 1, R.MAX_STANDARD_TX_WEIGHT),
    (R.BNB, 0, 0, 0, [(R.MAX_MONEY + 1, -1)], 1, R.MAX_MONEY + 1, R.MAX_STANDARD_TX_WEIGHT),
    # CoinGrinder's first best is the weight limit with an amount of MAX_MONEY: a tie at both is not better.
    (R.CG, 0, 0, 0, [(R.MAX_MONEY, -1)], R.MAX_MONEY, 0, 0),
    (R.CG, 0, 0, 0, [(R.MAX_MONEY - 1, -1)], R.MAX_MONEY - 1, 0, 0),
    # Knapsack: the smaller coins total exactly target + change target, and the first pass finds 6, not 5.
    (R.KNAPSACK, 0, 0, 0, [(3, -1), (3, -1), (4, -1)], 5, 5, R.MAX_STANDARD_TX_WEIGHT),
    # Knapsack: the smaller coins total the target at exactly the weight limit.
    (R.KNAPSACK, 0, 0, 0, [(500, 68), (500, 68), (9000, 68)], 1000, 1000, 544),
    (R.KNAPSACK, 0, 0, 0, [(500, 68), (500, 68), (9000, 68)], 1000, 1000, 543),
    # Knapsack: the larger coin is the pool's first.
    (R.KNAPSACK, 0, 0, 0, [(9000, 25), (500, 75), (500, 75)], 1000, 1000, 500),
    # Knapsack: a coin of exactly target + change target.
    (R.KNAPSACK, 0, 0, 0, [(1500, -1), (400, -1), (700, -1)], 1000, 500, R.MAX_STANDARD_TX_WEIGHT),
    # Knapsack: with a negative change target the larger coin can be below the target; it replaces the smaller coins.
    (R.KNAPSACK, 0, 0, 0, [(500, 68), (500, 68), (950, -1)], 1000, -100, 500),
    (R.KNAPSACK, 0, 0, 0, [(500, 68), (500, 68), (950, -1)], 1000, -100, 544),
    # SRD below a weight limit of -1 (ChooseSelectionResult can pass one after BnB succeeds) drops every coin.
    (R.SRD, 0, 1000, 10000, [(100000, 68), (200000, -1)], 1000, 31, -1),
]


def hand():
    rows = []
    for algo, seed, f, lt, coins, target, extra, max_weight in HAND:
        func = {R.BNB: None, R.CG: R.coin_grinder, R.SRD: R.srd, R.KNAPSACK: R.knapsack}[algo]
        if algo == R.BNB:
            r = R.bnb(groups(coins, f, lt), target, extra, max_weight)
        else:
            r = run_algo(func, algo, coins, f, lt, target, extra, max_weight, seed)
        rows.append(result_row(algo, seed, f, lt, coins, target, extra, max_weight, r))
    return rows


def pick_row(seed, f, lt, coins, payment, noinputs, max_tx, spk):
    rng = A.Rng(bytes([seed]) + bytes(31))
    pool = groups(coins, f, lt)
    p = R.Params(f, lt, 3000, spk, 68, noinputs, payment, 1, rng)
    target = payment + FEE(f, noinputs)
    win, ran = R.choose(pool, target, p, max_tx, rng)
    row = dict(seed=seed, feerate=f, long_term=lt, pool=spec(coins), payment=payment, noinputs=noinputs, max_tx=max_tx,
               code=win.code, algo=win.algo if win.code == R.OK else R.MANUAL, waste=win.waste if win.code == R.OK else 0,
               want=','.join(str(v) for v in sorted(x.value for x in win.groups)) if win.code == R.OK else '',
               target=p.min_change_target)
    for a in range(4):
        x = ran.get(a)
        row[f'code{a}'] = 9 if x is None else x.code
        row[f'want{a}'] = ','.join(str(v) for v in sorted(y.value for y in x.groups)) if x is not None and x.code == R.OK else ''
    return row


def picks():
    g = random.Random(95_0002)
    rows = []
    spk = b'\x00\x14' + bytes(20)
    for i in range(80):
        f = g.choice(FEERATES); lt = 10000
        coins = draw_pool(g, f, lt)
        seed = (i * 7 + 3) % 256
        payment = g.choice([g.randrange(1, 25_000), 25_000, 25_001, g.randrange(25_002, 600_000), g.randrange(1, 9_000_000)])
        noinputs = 10 + 31 + g.choice([0, 12])
        max_tx = g.choice([R.MAX_STANDARD_TX_WEIGHT] * 4 + [4 * noinputs, 4 * noinputs + 1, 4 * noinputs + 4 * 31 - 1,
                                                             4 * noinputs + 4 * 31 + 272, 4 * noinputs + 4 * 31 + 600])
        rows.append(pick_row(seed, f, lt, coins, payment, noinputs, max_tx, spk))
    return rows


# Choices made by hand: a weight limit that leaves exactly nothing for
# inputs, or for the change output, with coins of unknown size that weigh
# nothing; and a heavy coin worth less than its fee, which only Knapsack
# (it searches every group) is too heavy for.
HAND_PICKS = [
    (1000, [(60000, -1)] * 3, 50000, 42, 4 * 42),
    (1000, [(60000, -1)] * 3, 50000, 42, 4 * 42 + 1),
    (1000, [(60000, -1)] * 3, 50000, 42, 4 * 42 + 4 * 31),
    (1000, [(60000, -1)] * 3, 50000, 42, 4 * 42 + 4 * 31 - 1),
    (100000, [(100, 1000), (200000, 68)], 500000, 42, 4 * 42 + 4 * 31 + 1000),
    # A coin of unknown size inside BnB's window: BnB succeeds with no weight at all.
    (1000, [(50100, -1), (60000, -1), (60000, -1)], 50000, 42, 4 * 42),
    (1000, [(50100, -1), (60000, -1), (60000, -1)], 50000, 42, 4 * 42 + 1),
    (1000, [(50100, -1), (60000, -1), (60000, -1)], 50000, 42, 4 * 42 + 4 * 31),
]


def hand_picks():
    rows = []
    spk = b'\x00\x14' + bytes(20)
    for i, (f, coins, payment, noinputs, max_tx) in enumerate(HAND_PICKS):
        rows.append(pick_row(i, f, 10000, coins, payment, noinputs, max_tx, spk))
    return rows


def render(sel, pk):
    lines = ['// Generated by make_coinselect_random.py: wallets drawn with a fixed seed and',
             '// decided by coinselect_reference.py, a transliteration of Bitcoin Core\'s',
             '// coinselection.cpp. Test data only.',
             '// One run of one algorithm: its seed byte (the rest of the key is zero),',
             '// the pool ("value/vsize*count,..." at feerate and long_term sat/kvB), the',
             '// target, extra (as CsCase), the weight limit, and the whole result: the',
             '// code, the attempts (BnB and CoinGrinder), whether the search finished',
             '// (-1: not a finished BnB or CoinGrinder search), the sums and the values.',
             'CsRun :: struct { algo: int; seed: int; feerate: int; long_term: int; pool: string; target: int; extra: int; max_weight: int;',
             '                  code: int; tries: int; completed: int; count: int; value: int; effective: int; weight: int; fee: int;',
             '                  long_term_fee: int; want: string; }',
             f'CS_RUNS :: [{len(sel)}] CsRun.[']
    for r in sel:
        lines.append(f'  CsRun.{{algo={r["algo"]}, seed={r["seed"]}, feerate={r["feerate"]}, long_term={r["long_term"]}, pool="{r["pool"]}", '
                     f'target={r["target"]}, extra={r["extra"]}, max_weight={r["max_weight"]}, code={r["code"]}, tries={r["tries"]}, '
                     f'completed={r["completed"]}, count={r["count"]}, value={r["value"]}, effective={r["effective"]}, weight={r["weight"]}, '
                     f'fee={r["fee"]}, long_term_fee={r["ltf"]}, want="{r["want"]}"}},')
    lines += ['];',
              '// One ChooseSelectionResult: a P2WPKH change output (spent with 68 vB at',
              '// 3,000 sat/kvB), one recipient paid payment, tx_noinputs_size and the',
              '// weight limit; the winner\'s code, algorithm, waste and values, the',
              '// change target the parameters drew, and each algorithm\'s own code (9: not',
              '// run) and values, in the order bnb, knapsack, srd, cg.',
              'CsPick :: struct { seed: int; feerate: int; long_term: int; pool: string; payment: int; noinputs: int; max_tx: int;',
              '                   code: int; algo: int; waste: int; want: string; change_target: int; codes: [4] int; wants: [4] string; }',
              f'CS_PICKS :: [{len(pk)}] CsPick.[']
    for r in pk:
        codes = ', '.join(str(r[f'code{a}']) for a in range(4))
        wants = ', '.join(f'"{r[f"want{a}"]}"' for a in range(4))
        lines.append(f'  CsPick.{{seed={r["seed"]}, feerate={r["feerate"]}, long_term={r["long_term"]}, pool="{r["pool"]}", payment={r["payment"]}, '
                     f'noinputs={r["noinputs"]}, max_tx={r["max_tx"]}, code={r["code"]}, algo={r["algo"]}, waste={r["waste"]}, want="{r["want"]}", '
                     f'change_target={r["target"]}, codes=[4] int.[{codes}], wants=[4] string.[{wants}]}},')
    lines.append('];')
    return '\n'.join(lines) + '\n'


def main():
    R.check_core()
    sel, pk = selections() + hunted() + hand(), picks() + hand_picks()
    text = render(sel, pk)
    codes = [r['code'] for r in sel]
    summary = (f'{len(sel)} runs ({codes.count(R.OK)} selections, {codes.count(R.INSUFFICIENT)} insufficient, '
               f'{codes.count(R.MAX_WEIGHT)} over the weight limit), {len(pk)} choices')
    if '--write' in sys.argv:
        OUT.write_text(text)
        print(f'wrote {OUT.name}: {summary}')
    else:
        assert OUT.read_text() == text, f'{OUT.name} differs; run with --write'
        print(f'{OUT.name} matches: {summary}')


if __name__ == '__main__':
    main()
