#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: a Python transliteration of Bitcoin Core's pinned coinselection.cpp decides each selection outside Coax)
"""Bitcoin Core's coin selection (wallet/coinselection.cpp, spend.cpp and
policy/feerate.cpp at revision ddea9196) transliterated into Python for
Bitcoin chapter 95: effective values, the cost of change, Branch and
Bound, CoinGrinder, Knapsack (with ApproximateBestSubset), Single Random
Draw, GenerateChangeTarget, the waste metric and ChooseSelectionResult.

Randomness is FastRandomContext from chapter 69's addrman.py (the pinned
ChaCha20 keystream). Core shuffles with std::shuffle, whose draws depend
on the C++ library; this module, like coinselect.coax, shuffles with
Fisher-Yates from the back over randrange. pinned() checks every file
against manifest.json, and check_core() checks that the lines this
module follows still read that way in the pinned sources.
"""
import hashlib, json, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import addrman as A  # noqa: E402  (FastRandomContext)

UP = HERE.parent / 'vectors' / 'upstream'
COIN, CENT = 100_000_000, 1_000_000
CHANGE_LOWER, CHANGE_UPPER, TOTAL_TRIES = 50_000, 1_000_000, 100_000
MAX_MONEY, MAX_STANDARD_TX_WEIGHT = 21_000_000 * COIN, 400_000
OK, INSUFFICIENT, MAX_WEIGHT = 0, -1, -2
BNB, KNAPSACK, SRD, CG, MANUAL = 0, 1, 2, 3, 4
ALGOS = ['bnb', 'knapsack', 'srd', 'cg', 'manual']
SEED = bytes([0x95]) * 32


def pinned(name):
    b = (UP / name).read_bytes()
    m = json.loads((UP.parent / 'manifest.json').read_text())
    pin = next(e['sha256'] for e in m['files'] if e['path'] == 'upstream/' + name)
    assert hashlib.sha256(b).hexdigest() == pin, name
    return b.decode()


def check_core():
    cs, h = pinned('bitcoin-core/coinselection.cpp'), pinned('bitcoin-core/coinselection.h')
    spend, rate, policy = pinned('bitcoin-core/spend.cpp'), pinned('bitcoin-core/feerate.cpp'), pinned('bitcoin-core/policy.cpp')
    for line in ['static const size_t TOTAL_TRIES = 100000;',
                 'target_value += CHANGE_LOWER + change_fee;',
                 'if (nPass == 0 ? insecure_rand.randbool() : !vfIncluded[i])',
                 'return change_fee + rng.randrange(upper_bound - CHANGE_LOWER) + CHANGE_LOWER;',
                 'waste += selected_effective_value - m_target;',
                 'return *m_waste < *other.m_waste || (*m_waste == *other.m_waste && m_selected_inputs.size() > other.m_selected_inputs.size());',
                 '} else if (is_feerate_high && curr_selection_waste > best_waste) {',
                 'if (curr_weight < best_selection_weight || (curr_weight == best_selection_weight && curr_amount < best_selection_amount)) {']:
        assert line in cs, line
    assert 'inline constexpr CAmount CHANGE_LOWER{50\'000};' in h and 'inline constexpr CAmount CHANGE_UPPER{1\'000\'000};' in h
    assert 'static constexpr size_t OUTPUT_GROUP_MAX_ENTRIES{100};' in spend
    assert 'if (coin_selection_params.m_effective_feerate > CFeeRate{3 * coin_selection_params.m_long_term_feerate}) {' in spend
    assert 'coin_selection_params.min_viable_change = std::max(change_spend_fee + 1, dust);' in spend
    assert 'CAmount nFee = CAmount(m_feerate.EvaluateFeeUp(virtual_bytes));' in rate
    assert 'nSize += (32 + 4 + 1 + (107 / WITNESS_SCALE_FACTOR) + 4);' in policy


def get_fee(rate, vbytes):
    """CFeeRate(rate sat/kvB).GetFee(vbytes): rounded up."""
    return -(-rate * vbytes // 1000)


class Group:
    """An OutputGroup: value, fee, long-term fee, weight and coin count."""
    def __init__(self, value, fee=0, ltf=0, weight=0, count=1, ident=0):
        self.value, self.fee, self.ltf, self.weight, self.count, self.ident = value, fee, ltf, weight, count, ident

    def amount(self):
        return self.value - self.fee


def coin(value, input_bytes, feerate, long_term, ident=0):
    if input_bytes < 0:
        return Group(value, ident=ident)
    return Group(value, get_fee(feerate, input_bytes), get_fee(long_term, input_bytes), 4 * input_bytes, 1, ident)


class Result:
    def __init__(self, algo, target):
        self.algo, self.target, self.groups, self.tries, self.completed, self.code, self.waste = algo, target, [], 0, True, OK, None

    def add(self, g):
        self.groups.append(g)

    def value(self): return sum(g.value for g in self.groups)
    def effective(self): return sum(g.amount() for g in self.groups)
    def weight(self): return sum(g.weight for g in self.groups)
    def count(self): return sum(g.count for g in self.groups)
    def fee(self): return sum(g.fee for g in self.groups)
    def ltf(self): return sum(g.ltf for g in self.groups)

    def change(self, min_viable, change_fee):
        c = self.effective() - self.target - change_fee
        return 0 if c < min_viable else c

    def recalc_waste(self, min_viable, change_cost, change_fee):
        w = self.fee() - self.ltf()
        w += change_cost if self.change(min_viable, change_fee) else self.effective() - self.target
        self.waste = w
        return w


def fail(code):
    r = Result(None, 0)
    r.code = code
    return r


def key_descending(g):            # descending: effective value, then less waste
    return (-g.amount(), g.fee - g.ltf)


def key_effval_weight(g):         # descending_effval_weight: effective value, then lighter
    return (-g.amount(), g.weight)


def bnb(pool, target, cost_of_change, max_weight):
    utxo = sorted([g for g in pool if g.amount() > 0], key=key_descending)
    look, total = [0] * len(utxo), 0
    for i in range(len(utxo) - 1, -1, -1):
        look[i] = total
        total += utxo[i].amount()
    if not utxo or total < target:
        return fail(INSUFFICIENT)
    curr, best = [], []
    amount = waste = weight = 0
    best_waste, exceeded, nxt, tries = MAX_MONEY, False, 0, 0
    res = Result(BNB, target)
    high = utxo[0].fee > utxo[0].ltf

    def deselect():
        nonlocal amount, weight, waste
        u = utxo[curr.pop()]
        amount -= u.amount(); weight -= u.weight; waste -= u.fee - u.ltf
    done = False
    while not done:
        shift = cut = False
        u = utxo[nxt]
        amount += u.amount(); weight += u.weight; waste += u.fee - u.ltf
        curr.append(nxt); nxt += 1; tries += 1
        if amount + look[curr[-1]] < target:
            cut = True
        elif weight > max_weight:
            exceeded = shift = True
        elif amount > target + cost_of_change:
            shift = True
        elif high and waste > best_waste:
            shift = True
        elif amount >= target:
            shift = True
            w = waste + amount - target
            if w <= best_waste:
                best, best_waste = list(curr), w
        if tries >= TOTAL_TRIES:
            res.completed = False
            break
        if nxt == len(utxo):
            cut = True
        if cut:
            deselect()
            shift = True
        while shift:
            if not curr:
                done, res.completed = True, True
                break
            nxt = curr[-1] + 1
            deselect()
            shift = False
            while utxo[nxt - 1].amount() == utxo[nxt].amount():
                if nxt >= len(utxo) - 1:
                    shift = True
                    break
                nxt += 1
    res.tries = tries
    if not best:
        r = fail(MAX_WEIGHT if exceeded else INSUFFICIENT); r.tries = tries
        return r
    for i in best:
        res.add(utxo[i])
    return res


def coin_grinder(pool, target, change_target, max_weight):
    utxo = sorted([g for g in pool if g.amount() > 0], key=key_effval_weight)
    n = len(utxo)
    look, tail_min = [0] * n, [0] * n
    total, lightest = 0, 2**31 - 1
    for i in range(n - 1, -1, -1):
        look[i], tail_min[i] = total, lightest
        total += utxo[i].amount()
        lightest = min(lightest, utxo[i].weight)
    goal = target + change_target
    if total < goal:
        return fail(INSUFFICIENT)
    curr, best = [], []
    amount = weight = 0
    best_amount, best_weight, exceeded, nxt, tries = MAX_MONEY, max_weight, False, 0, 0
    res = Result(CG, target)

    def deselect():
        nonlocal amount, weight
        u = utxo[curr.pop()]
        amount -= u.amount(); weight -= u.weight
    done = False
    while not done:
        shift = cut = False
        u = utxo[nxt]
        amount += u.amount(); weight += u.weight
        curr.append(nxt); nxt += 1; tries += 1
        t = curr[-1]
        if amount + look[t] < goal:
            cut = True
        elif weight > best_weight:
            if weight > max_weight:
                exceeded = True
            if utxo[t].weight <= tail_min[t]:
                cut = True
            else:
                shift = True
        elif amount >= goal:
            shift = True
            if weight < best_weight or (weight == best_weight and amount < best_amount):
                best, best_weight, best_amount = list(curr), weight, amount
        elif best and weight + tail_min[t] * ((goal - amount + utxo[t].amount() - 1) // utxo[t].amount()) > best_weight:
            if utxo[t].weight <= tail_min[t]:
                cut = True
            else:
                shift = True
        if tries >= TOTAL_TRIES:
            res.completed = False
            break
        if nxt == n:
            cut = True
        if cut:
            deselect()
            shift = True
        while shift:
            if not curr:
                done, res.completed = True, True
                break
            nxt = curr[-1] + 1
            deselect()
            shift = False
            while utxo[nxt - 1].amount() == utxo[nxt].amount():
                if nxt >= n - 1:
                    shift = True
                    break
                nxt += 1
    res.tries = tries
    if not best:
        r = fail(MAX_WEIGHT if exceeded else INSUFFICIENT); r.tries = tries
        return r
    for i in best:
        res.add(utxo[i])
    return res


def shuffle(rng, a):
    for i in range(len(a) - 1, 0, -1):
        j = rng.randrange(i + 1)
        a[i], a[j] = a[j], a[i]


def srd(pool, target, change_fee, max_weight, rng):
    order = [g for g in pool if g.amount() > 0]
    shuffle(rng, order)
    goal = target + CHANGE_LOWER + change_fee
    held, amount, weight, exceeded = [], 0, 0, False
    for g in order:
        held.append(g); amount += g.amount(); weight += g.weight
        if weight > max_weight:
            exceeded = True
            while held and weight > max_weight:
                # The priority queue's top: least effective value, then heaviest.
                top = min(held, key=lambda x: (x.amount(), -x.weight))
                held.remove(top)
                amount -= top.amount(); weight -= top.weight
        if amount >= goal:
            res = Result(SRD, target)
            for h in held:
                res.add(h)
            return res
    return fail(MAX_WEIGHT if exceeded else INSUFFICIENT)


def approximate_best_subset(rng, groups, total_lower, target, max_weight, iterations=1000):
    best, nbest = [True] * len(groups), total_lower
    rep = 0
    while rep < iterations and nbest != target:
        inc = [False] * len(groups)
        total = weight = 0
        reached = False
        for npass in range(2):
            if reached:
                break
            for i, g in enumerate(groups):
                if (rng.randbool() if npass == 0 else not inc[i]):
                    total += g.amount(); weight += g.weight; inc[i] = True
                    if total >= target and weight <= max_weight:
                        reached = True
                        if total < nbest:
                            nbest, best = total, list(inc)
                        total -= g.amount(); weight -= g.weight; inc[i] = False
        rep += 1
    return best, nbest


def knapsack(pool, target, change_target, max_weight, rng):
    res = Result(KNAPSACK, target)
    groups = list(pool)
    shuffle(rng, groups)
    exceeded, larger, applicable, total_lower = False, None, [], 0
    for g in groups:
        if g.weight > max_weight:
            exceeded = True
            continue
        if g.amount() == target:
            res.add(g)
            return res
        elif g.amount() < target + change_target:
            applicable.append(g); total_lower += g.amount()
        elif larger is None or g.amount() < larger.amount():
            larger = g
    if total_lower == target:
        for g in applicable:
            res.add(g)
        if res.weight() <= max_weight:
            return res
        exceeded = True
        res.groups = []
    if total_lower < target:
        if larger is None:
            return fail(MAX_WEIGHT if exceeded else INSUFFICIENT)
        res.add(larger)
        return res
    applicable.sort(key=key_descending)
    best, nbest = approximate_best_subset(rng, applicable, total_lower, target, max_weight)
    if nbest != target and total_lower >= target + change_target:
        best, nbest = approximate_best_subset(rng, applicable, total_lower, target + change_target, max_weight)
    if larger is not None and ((nbest != target and nbest < target + change_target) or larger.amount() <= nbest):
        res.add(larger)
    else:
        for g, b in zip(applicable, best):
            if b:
                res.add(g)
        if res.weight() > max_weight:
            if larger is None:
                return fail(MAX_WEIGHT)
            res.groups = [larger]
    return res


def change_target(payment, change_fee, rng):
    if payment <= CHANGE_LOWER // 2:
        return change_fee + CHANGE_LOWER
    upper = min(payment * 2, CHANGE_UPPER)
    return change_fee + rng.randrange(upper - CHANGE_LOWER) + CHANGE_LOWER


def output_size(script_n):
    return 8 + (1 if script_n < 253 else 3) + script_n


def dust_threshold(spk, rate):
    if spk[:1] == b'\x6a':
        return 0
    size = output_size(len(spk))
    program = 4 <= len(spk) <= 42 and (spk[0] == 0 or 0x51 <= spk[0] <= 0x60) and spk[1] + 2 == len(spk)
    size += (32 + 4 + 1 + 107 // 4 + 4) if program else (32 + 4 + 1 + 107 + 4)
    return get_fee(rate, size)


class Params:
    def __init__(self, feerate, long_term, discard, change_spk, change_spend_size, tx_noinputs_size, payments, recipients, rng):
        self.feerate, self.long_term, self.discard = feerate, long_term, discard
        self.change_output_size = output_size(len(change_spk))
        self.change_spend_size, self.tx_noinputs_size = change_spend_size, tx_noinputs_size
        self.change_fee = get_fee(feerate, self.change_output_size)
        spend = get_fee(discard, change_spend_size)
        self.cost_of_change = spend + self.change_fee
        self.min_change_target = change_target(payments // recipients, self.change_fee, rng)
        self.min_viable_change = max(spend + 1, dust_threshold(change_spk, discard))


def choose(pool, target, p, max_tx_weight, rng):
    """ChooseSelectionResult: every algorithm's result, then the least waste."""
    max_weight = max_tx_weight - 4 * p.tx_noinputs_size
    results, first_error, ran = [], INSUFFICIENT, {}
    b = bnb(pool, target, p.cost_of_change, max_weight)
    ran[BNB] = b
    if b.code == OK: results.append(b)
    elif b.code == MAX_WEIGHT: first_error = MAX_WEIGHT
    max_weight -= 4 * p.change_output_size
    k = knapsack(pool, target, p.min_change_target, max_weight, rng)
    ran[KNAPSACK] = k
    if k.code == OK: results.append(k)
    elif k.code == MAX_WEIGHT: first_error = MAX_WEIGHT
    if p.feerate > 3 * p.long_term:
        c = coin_grinder(pool, target, p.min_change_target, max_weight)
        ran[CG] = c
        if c.code == OK: results.append(c)
        elif c.code == MAX_WEIGHT: first_error = MAX_WEIGHT
    s = srd(pool, target, p.change_fee, max_weight, rng)
    ran[SRD] = s
    if s.code == OK: results.append(s)
    elif s.code == MAX_WEIGHT: first_error = MAX_WEIGHT
    for r in results:
        r.target = target
        r.recalc_waste(p.min_viable_change, p.cost_of_change, p.change_fee)
    if not results:
        return fail(first_error), ran
    win = results[0]
    for r in results[1:]:
        if r.waste < win.waste or (r.waste == win.waste and r.count() > win.count()):
            win = r
    return win, ran
