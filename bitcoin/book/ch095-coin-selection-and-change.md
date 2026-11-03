# 95. Coin selection and change

A wallet doesn't hold a balance. It holds coins: the unspent outputs of
earlier transactions, each of a fixed amount. To pay someone, the wallet
picks coins that add up to at least the payment and the fee, spends
them whole, and sends what is left back to itself as a change output.

Which coins it picks matters. Each input makes the transaction larger,
so it adds fee. A coin worth less than the fee to spend it costs money
to use. A change output adds fee now and again later, when it is spent.
If the coins add up to the payment almost exactly, the wallet can skip
the change and give the small remainder to the miners.

This chapter follows Bitcoin Core's wallet. Core values each coin at the
current feerate, runs four selection algorithms, scores each result by
its waste, and uses the least wasteful one. Every case in Core's own
coin-selection tests that doesn't depend on a particular random draw
gives Core's result here.

## What this chapter teaches

- A coin's effective value: its amount minus the fee to spend it at the
  current feerate.
- The cost of change: the change output's fee now, plus the fee to spend
  it later, and when change is too small to be worth making.
- Branch and Bound, which looks for a changeless input set within the
  cost of change of the target.
- CoinGrinder, which looks for the lightest input set that leaves change
  when fees are high.
- Knapsack and Single Random Draw, Core's randomized fallbacks.
- The waste metric, which compares the results, and why it prefers more
  inputs at low feerates and fewer at high ones.
- Output groups, and avoid-reuse, which spends the coins of one address
  together.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [coinselect.coax](../../coax/lib/bitcoin/coinselect.coax) | `cs_coin`, `cs_amount` | A coin's fees, weight and effective value |
| | `cs_group_by_owner` | Avoid-reuse groups of at most 100 coins per address |
| | `cs_rng`, `cs_randrange`, `cs_shuffle` | Core's `FastRandomContext`, seeded |
| | `cs_dust_threshold_vartime`, `cs_change_target`, `cs_params` | Dust, the random change target and the cost of change |
| | `cs_change`, `cs_waste` | Core's `GetChange` and `RecalculateWaste` |
| | `cs_bnb` | Core's `SelectCoinsBnB` |
| | `cs_coin_grinder` | Core's `CoinGrinder` |
| | `cs_srd` | Core's `SelectCoinsSRD` |
| | `cs_knapsack`, `cs_approximate` | Core's `KnapsackSolver` and `ApproximateBestSubset` |
| | `cs_choose` | Core's `ChooseSelectionResult` |
| [fees.coax](../../coax/lib/bitcoin/fees.coax) | `fee_for_rate_vartime`, `fee_output_size_vartime` | Chapter 40's fee for a feerate, rounded up, and an output's size |
| [chacha20.coax](../../coax/lib/crypto/chacha20.coax) | `chacha20_block_checked` | Chapter 11's ChaCha20 block, the random stream |

## The parameters

Core's limits and result codes. A result is `CS_OK` or one of two
failures: insufficient funds, or every input set that would do is too
heavy. Each algorithm has its own bit in the marks array, so one array
can hold all four results.

<!-- source: coax/lib/bitcoin/coinselect.coax params -->
```jai
CS_COIN :: 100000000;           // amount.h COIN
CS_CENT :: 1000000;             // a hundredth of a bitcoin, the tests' unit
CS_CHANGE_LOWER :: 50000;       // coinselection.h: the smallest random change target
CS_CHANGE_UPPER :: 1000000;     // coinselection.h: the largest random change target
CS_TOTAL_TRIES :: 100000;       // coinselection.cpp: BnB's and CoinGrinder's search limit
CS_GROUP_MAX :: 100;            // spend.cpp OUTPUT_GROUP_MAX_ENTRIES
// Results.
CS_OK :: 0;
CS_INSUFFICIENT :: -1;          // util::Error() with no message: insufficient funds
CS_MAX_WEIGHT :: -2;            // "The inputs size exceeds the maximum weight. ..."
CS_CAPACITY :: -3;              // a work or mark array of the wrong size
// The algorithms, as GetAlgorithmName names them.
CS_BNB :: 0; CS_KNAPSACK :: 1; CS_SRD :: 2; CS_CG :: 3; CS_MANUAL :: 4;
CS_ALGOS :: [5] string.["bnb", "knapsack", "srd", "cg", "manual"];
```

## Effective value

A feerate is in satoshis per 1,000 virtual bytes (sat/kvB), Core's unit;
10 sat/vB is 10,000 sat/kvB. Spending a P2WPKH coin takes 68 virtual
bytes, a P2TR key-path coin 58 and a P2PKH coin 148. At 10 sat/vB a
P2WPKH input costs 680 satoshis, so a coin of 1,000,000 satoshis is
worth 999,320 to the transaction. That is its effective value.

The selectors work on effective values. The target is the payment plus
the fee for the parts of the transaction that are not inputs: the
version, the counts, the outputs and the lock time. Then any set of
coins whose effective values add up to the target pays the payment and
every fee exactly.

Each coin also gets a fee at the long-term feerate, the rate the wallet
expects to pay on average (Core's `-consolidatefeerate`, 10 sat/vB by
default). The difference says whether spending the coin now is cheap or
dear.

Core selects output groups, not coins. Normally each coin is its own
group. With avoid-reuse (`-avoidpartialspends`), every coin sent to one
address goes in one group of up to 100 coins, so spending one of them
spends all of them. A coin spent alone would link the address to the
transaction, and the next spend of its siblings would link them again.

<!-- source: coax/lib/bitcoin/coinselect.coax group -->
```jai
// An output group: the summed amount, fee at the effective feerate, fee
// at the long-term feerate and weight of its coins. A single coin is a
// group of one.
CsGroup :: struct { value: int; fee: int; long_term_fee: int; weight: int; count: int; }
// One coin of value satoshis that takes input_bytes virtual bytes to
// spend, at feerate and long_term sat/kvB, as COutput and
// OutputGroup::Insert value it. An unknown size (-1) has no fee and no
// weight.
cs_coin :: (value: int, input_bytes: int, feerate: int, long_term: int) -> CsGroup {
  if input_bytes < 0 { return CsGroup.{value=value, count=1}; }
  return CsGroup.{value=value, fee=fee_for_rate_vartime(feerate, input_bytes),
                  long_term_fee=fee_for_rate_vartime(long_term, input_bytes), weight=TX_SCALE * input_bytes, count=1};
}
// GetSelectionAmount: the effective value, amount minus fee.
cs_amount :: (g: CsGroup) -> int { return g.value - g.fee; }
// A coin added to a group, as OutputGroup::Insert adds it.
cs_group_add :: (g: CsGroup, c: CsGroup) -> CsGroup {
  return CsGroup.{value=g.value+c.value, fee=g.fee+c.fee, long_term_fee=g.long_term_fee+c.long_term_fee,
                  weight=g.weight+c.weight, count=g.count+c.count};
}
// Avoid-reuse grouping: coins sent to the same address (owner[i] names
// it) go into one group, so they are spent together or not at all. A
// group holds at most CS_GROUP_MAX coins; the next coin of that address
// starts a new group. Writes the groups into out and returns their count.
cs_group_by_owner :: (coins: [] CsGroup, owner: [] int, out: [] CsGroup, out_owner: [] int) -> int {
  n := 0;
  for i: 0..<coins.count {
    at := -1;
    for k: 0..<n { if out_owner[k] == owner[i] && out[k].count < CS_GROUP_MAX { at = k; } }
    if at < 0 { if n == out.count { return CS_CAPACITY; } at = n; out[n] = CsGroup.{}; out_owner[n] = owner[i]; n += 1; }
    out[at] = cs_group_add(out[at], coins[i]);
  }
  return n;
}
```

## What change costs

A change output costs fee twice. Now, the output's own bytes: 31 for
P2WPKH. Later, the input that spends it, which the wallet prices at the
discard feerate (`-discardfee`, 3 sat/vB). Their sum is the cost of
change.

Change below that later spending fee is worth nothing, and change
below the dust threshold won't relay. `GetDustThreshold` prices the
output and the input that would spend it at the discard rate: 294
satoshis for P2WPKH, 330 for P2TR and 546 for P2PKH. Core makes change
only when it is at least the larger of the two.

Knapsack and CoinGrinder also aim for change of a useful size, the
change target. For a payment of more than 25,000 satoshis it is random,
from 50,000 satoshis up to twice the payment or 1,000,000, whichever is
smaller. A fixed change amount would tell an observer which output is
the change.

<!-- source: coax/lib/bitcoin/coinselect.coax change -->
```jai
// What a transaction's change costs, from CoinSelectionParams: the fee
// for the change output now, the cost of change (that fee plus spending
// it later at the discard feerate), the smallest change worth making,
// and the change target that Knapsack and CoinGrinder aim to leave.
CsParams :: struct {
  feerate: int; long_term: int; discard: int;      // sat/kvB
  change_output_size: int; change_spend_size: int; tx_noinputs_size: int;
  change_fee: int; cost_of_change: int; min_viable_change: int; min_change_target: int;
}
// GetDustThreshold (policy.cpp): an output is dust when spending it would
// cost more than it is worth at rate. Its size plus the input that spends
// it: 148 bytes, or 67 for a witness program, whose 107-byte signature
// and key count a quarter. An OP_RETURN output is never dust.
cs_dust_threshold_vartime :: (spk: [] u8, rate: int) -> int {
  if spk.count > 0 && spk[0] == 0x6a { return 0; }
  size := fee_output_size_vartime(spk.count);
  program := spk.count >= 4 && spk.count <= 42 && (spk[0] == 0 || (spk[0] >= 0x51 && spk[0] <= 0x60)) && cast(int) spk[1] + 2 == spk.count;
  size += ifx program then 32 + 4 + 1 + 107 / TX_SCALE + 4 else 32 + 4 + 1 + 107 + 4;
  return fee_for_rate_vartime(rate, size);
}
// GenerateChangeTarget: 50,000 satoshis plus the change fee for a small
// payment; otherwise a random amount from 50,000 up to twice the payment
// or 1,000,000, whichever is less, so change doesn't give the payment away.
cs_change_target :: (payment: int, change_fee: int, r: [] CsRng) -> int {
  if payment <= CS_CHANGE_LOWER / 2 { return change_fee + CS_CHANGE_LOWER; }
  upper := ifx payment * 2 < CS_CHANGE_UPPER then payment * 2 else CS_CHANGE_UPPER;
  return change_fee + cs_randrange(r, upper - CS_CHANGE_LOWER) + CS_CHANGE_LOWER;
}
// The parameters as CreateTransactionInternal sets them for a change
// output change_spk, paying recipients outputs worth payments in all.
cs_params :: (feerate: int, long_term: int, discard: int, change_spk: [] u8, change_spend_size: int,
              tx_noinputs_size: int, payments: int, recipients: int, r: [] CsRng) -> CsParams {
  p: CsParams;
  p.feerate = feerate; p.long_term = long_term; p.discard = discard;
  p.change_output_size = fee_output_size_vartime(change_spk.count);
  p.change_spend_size = change_spend_size; p.tx_noinputs_size = tx_noinputs_size;
  p.change_fee = fee_for_rate_vartime(feerate, p.change_output_size);
  spend := fee_for_rate_vartime(discard, change_spend_size);
  p.cost_of_change = spend + p.change_fee;
  p.min_change_target = cs_change_target(payments / recipients, p.change_fee, r);
  dust := cs_dust_threshold_vartime(change_spk, discard);
  p.min_viable_change = ifx spend + 1 > dust then spend + 1 else dust;
  return p;
}
```

## Waste

Each algorithm returns a different input set, so Core needs one score
to compare them. The waste of a selection is the sum of two parts.

The first part is what the inputs cost now beyond what they would cost
at the long-term feerate: the fee minus the long-term fee, input by
input. Above the long-term feerate this part is positive, so fewer
inputs are better. Below it the part is negative, so spending more
inputs now, while fees are low, scores better. That is consolidation.

The second part depends on the change. With change, it is the cost of
change. Without change, it is the excess: the effective value above the
target, which goes to the miners.

<!-- source: coax/lib/bitcoin/coinselect.coax result -->
```jai
// A selection: the algorithm, its groups (marked in the caller's marks,
// bit 1 << algo per group), their sums, the search steps it evaluated,
// whether the search finished, and its waste.
CsResult :: struct {
  code: int; algo: int; count: int; value: int; effective: int; weight: int;
  fee: int; long_term_fee: int; tries: int; completed: bool; waste: int; target: int;
}
cs_fail :: (algo: int, code: int) -> CsResult { return CsResult.{code=code, algo=algo}; }
// AddInput: a group joins the result and is marked.
cs_add :: (r: CsResult, pool: [] CsGroup, i: int, marks: [] int) -> CsResult {
  x := r; g := pool[i];
  x.count += g.count; x.value += g.value; x.effective += cs_amount(g); x.weight += g.weight;
  x.fee += g.fee; x.long_term_fee += g.long_term_fee;
  marks[i] |= 1 << x.algo;
  return x;
}
// Clears algo's bit from every group.
cs_unmark :: (marks: [] int, algo: int) {
  for i: 0..<marks.count { if (marks[i] >> algo) & 1 == 1 { marks[i] -= 1 << algo; } }
}
cs_clear :: (r: CsResult, marks: [] int) -> CsResult {
  cs_unmark(marks, r.algo);
  return CsResult.{algo=r.algo, target=r.target, tries=r.tries, completed=r.completed};
}
// GetChange: what is left for a change output after the target and the
// change output's own fee, or 0 when that is below min_viable_change.
cs_change :: (r: CsResult, min_viable_change: int, change_fee: int) -> int {
  change := r.effective - r.target - change_fee;
  return ifx change < min_viable_change then 0 else change;
}
// RecalculateWaste: spending the inputs now rather than at the long-term
// feerate (fee - long_term_fee), plus the cost of change when there is
// change, or else the excess the transaction gives to the miners.
cs_waste :: (r: CsResult, min_viable_change: int, change_cost: int, change_fee: int) -> int {
  waste := r.fee - r.long_term_fee;
  if cs_change(r, min_viable_change, change_fee) != 0 { waste += change_cost; }
  else { waste += r.effective - r.target; }
  return waste;
}
```

## The search order

Branch and Bound and CoinGrinder search the groups largest first. Core
breaks ties two ways: Branch and Bound puts the group with less waste
first, and CoinGrinder puts the lighter one first. Either way, a second
group with the same effective value is never better than the first, which
lets the searches skip it.

<!-- source: coax/lib/bitcoin/coinselect.coax sort -->
```jai
// The groups to search, in pool order: all of them, or (positive) only
// those with a positive effective value. Returns how many.
cs_eligible :: (pool: [] CsGroup, order: [] int, positive: bool) -> int {
  n := 0;
  for i: 0..<pool.count { if !positive || cs_amount(pool[i]) > 0 { order[n] = i; n += 1; } }
  return n;
}
// Core's two orders: by effective value, largest first, then (by_weight)
// lighter first, as descending_effval_weight, or less waste (fee -
// long_term_fee) first, as descending. A stable insertion sort.
cs_before :: (a: CsGroup, b: CsGroup, by_weight: bool) -> bool {
  if cs_amount(a) != cs_amount(b) { return cs_amount(a) > cs_amount(b); }
  if by_weight { return a.weight < b.weight; }
  return a.fee - a.long_term_fee < b.fee - b.long_term_fee;
}
cs_sort :: (pool: [] CsGroup, order: [] int, n: int, by_weight: bool) {
  for i: 1..<n {
    j := i;
    while j > 0 && cs_before(pool[order[j]], pool[order[j-1]], by_weight) { t := order[j]; order[j] = order[j-1]; order[j-1] = t; j -= 1; }
  }
}
```

## Branch and Bound

Branch and Bound looks for a changeless transaction. It walks a binary
tree: at each depth it decides one group, trying to include it before
omitting it. A selection is a solution when its effective value is from
the target up to the target plus the cost of change. Overshooting by
less than the cost of change is cheaper than making change.

Four rules prune the tree:

- If the selection plus every group after it can't reach the target,
  the whole subtree is barren: cut it.
- If the selection overshoots the window, or is too heavy, adding more
  only makes it worse: shift to the omission branch.
- Above the long-term feerate, each input adds waste. A selection that
  already has more waste than the best solution can't beat it.
- After omitting a group, skip the next one if it has the same
  effective value. Every set with it was already tried with its twin.

The search stops after 100,000 steps. Core's test of 18 nearly equal
coins takes 87,957 steps to find the one window that fits; with 19 it
gives up.

<!-- source: coax/lib/bitcoin/coinselect.coax bnb -->
```jai
// SelectCoinsBnB: a depth-first search over the groups, largest first,
// that tries including each before omitting it, for an input set whose
// effective value lands from target to target + cost_of_change. Such a
// set needs no change output. Of those it finds, it keeps the least
// waste. work holds 5 ints per group.
cs_bnb :: (pool: [] CsGroup, target: int, cost_of_change: int, max_weight: int, marks: [] int, work: [] int) -> CsResult {
  r := CsResult.{algo=CS_BNB, target=target};
  if marks.count != pool.count || work.count < 5 * pool.count { return cs_fail(CS_BNB, CS_CAPACITY); }
  order := work[0..<pool.count];
  n := cs_eligible(pool, order, true);
  look := work[pool.count..<2*pool.count]; curr := work[2*pool.count..<3*pool.count]; best := work[3*pool.count..<4*pool.count];
  cs_unmark(marks, CS_BNB);
  cs_sort(pool, order, n, false);
  // lookahead[i]: the effective value of every group after position i.
  total := 0;
  for k: 0..<n { i := n - 1 - k; look[i] = total; total += cs_amount(pool[order[i]]); }
  if n == 0 || total < target { return cs_fail(CS_BNB, CS_INSUFFICIENT); }
  ncurr := 0; nbest := 0; amount := 0; waste := 0; weight := 0; next := 0; tries := 0;
  best_waste := FEE_MAX_MONEY; exceeded := false; done := false;
  // Above the long-term feerate each input adds waste.
  high := pool[order[0]].fee > pool[order[0]].long_term_fee;
  while !done {
    shift := false; cut := false;
    g := pool[order[next]];
    amount += cs_amount(g); weight += g.weight; waste += g.fee - g.long_term_fee;
    curr[ncurr] = next; ncurr += 1; next += 1; tries += 1;
    if amount + look[curr[ncurr-1]] < target { cut = true; }               // even every later group falls short
    else if weight > max_weight { exceeded = true; shift = true; }
    else if amount > target + cost_of_change { shift = true; }            // overshot the window
    else if high && waste > best_waste { shift = true; }                   // more inputs only add waste
    else if amount >= target {
      shift = true;                                                       // a solution: more inputs only cost more
      w := waste + amount - target;
      if w <= best_waste { for k: 0..<ncurr { best[k] = curr[k]; } nbest = ncurr; best_waste = w; }
    }
    if tries >= CS_TOTAL_TRIES { r.completed = false; break; }
    if next == n { cut = true; }
    if cut {                                                              // omit the last, then shift past the one before
      h := pool[order[curr[ncurr-1]]];
      amount -= cs_amount(h); weight -= h.weight; waste -= h.fee - h.long_term_fee; ncurr -= 1;
      shift = true;
    }
    while shift {
      if ncurr == 0 { done = true; r.completed = true; break; }
      next = curr[ncurr-1] + 1;
      h := pool[order[curr[ncurr-1]]];
      amount -= cs_amount(h); weight -= h.weight; waste -= h.fee - h.long_term_fee; ncurr -= 1;
      shift = false;
      // Skip a group equal in effective value to the one just omitted:
      // it can only repeat, or do worse than, what was already tried.
      while cs_amount(pool[order[next-1]]) == cs_amount(pool[order[next]]) {
        if next >= n - 1 { shift = true; break; }
        next += 1;
      }
    }
  }
  r.tries = tries;
  if nbest == 0 { return CsResult.{code=ifx exceeded then CS_MAX_WEIGHT else CS_INSUFFICIENT, algo=CS_BNB, tries=tries}; }
  for k: 0..<nbest { r = cs_add(r, pool, order[best[k]], marks); }
  return r;
}
```

## CoinGrinder

When fees are high, the lightest transaction is the cheapest. CoinGrinder
searches the same tree for the input set of least weight whose
effective value covers the target plus the change target. It always
makes change.

Its pruning is about weight. A selection heavier than the best solution
can't improve. And while a selection is short of the goal, the amount
still missing needs at least so many more groups, each at least as
light as the lightest group left, so CoinGrinder can tell early that a
branch can't be lighter than the best.

Core runs CoinGrinder only when the feerate is above three times the
long-term feerate: 30 sat/vB by default.

<!-- source: coax/lib/bitcoin/coinselect.coax cg -->
```jai
// CoinGrinder: the same search, ordered lightest first among equal
// values, for the input set of least weight whose effective value covers
// target + change_target, so the change output is worth making. It
// prunes on weight: a selection already heavier than the best can't
// improve, and min_tail[i] (the lightest group after i) bounds how much
// weight the missing amount still needs.
cs_coin_grinder :: (pool: [] CsGroup, target: int, change_target: int, max_weight: int, marks: [] int, work: [] int) -> CsResult {
  r := CsResult.{algo=CS_CG, target=target};
  if marks.count != pool.count || work.count < 5 * pool.count { return cs_fail(CS_CG, CS_CAPACITY); }
  m := pool.count;
  order := work[0..<m];
  n := cs_eligible(pool, order, true);
  look := work[m..<2*m]; min_tail := work[2*m..<3*m]; curr := work[3*m..<4*m]; best := work[4*m..<5*m];
  cs_unmark(marks, CS_CG);
  cs_sort(pool, order, n, true);
  total := 0; lightest := 0x7fffffff;
  for k: 0..<n {
    i := n - 1 - k;
    look[i] = total; min_tail[i] = lightest;
    total += cs_amount(pool[order[i]]);
    if pool[order[i]].weight < lightest { lightest = pool[order[i]].weight; }
  }
  goal := target + change_target;
  if total < goal { return cs_fail(CS_CG, CS_INSUFFICIENT); }
  ncurr := 0; nbest := 0; amount := 0; weight := 0; next := 0; tries := 0;
  best_amount := FEE_MAX_MONEY; best_weight := max_weight; exceeded := false; done := false;
  while !done {
    shift := false; cut := false;
    g := pool[order[next]];
    amount += cs_amount(g); weight += g.weight;
    curr[ncurr] = next; ncurr += 1; next += 1; tries += 1;
    tail := curr[ncurr-1];
    if amount + look[tail] < goal { cut = true; }
    else if weight > best_weight {
      if weight > max_weight { exceeded = true; }
      // Heavier than the best: cut if the last group was the lightest left.
      if pool[order[tail]].weight <= min_tail[tail] { cut = true; } else { shift = true; }
    } else if amount >= goal {
      shift = true;
      if weight < best_weight || (weight == best_weight && amount < best_amount) {
        for k: 0..<ncurr { best[k] = curr[k]; } nbest = ncurr; best_weight = weight; best_amount = amount;
      }
    } else if nbest > 0 {
      // Even groups as large as the last, each as light as the lightest
      // left, would need too much weight to beat the best.
      a := cs_amount(pool[order[tail]]);
      if weight + min_tail[tail] * ((goal - amount + a - 1) / a) > best_weight {
        if pool[order[tail]].weight <= min_tail[tail] { cut = true; } else { shift = true; }
      }
    }
    if tries >= CS_TOTAL_TRIES { r.completed = false; break; }
    if next == n { cut = true; }
    if cut {
      h := pool[order[curr[ncurr-1]]]; amount -= cs_amount(h); weight -= h.weight; ncurr -= 1;
      shift = true;
    }
    while shift {
      if ncurr == 0 { done = true; r.completed = true; break; }
      next = curr[ncurr-1] + 1;
      h := pool[order[curr[ncurr-1]]]; amount -= cs_amount(h); weight -= h.weight; ncurr -= 1;
      shift = false;
      while cs_amount(pool[order[next-1]]) == cs_amount(pool[order[next]]) {
        if next >= n - 1 { shift = true; break; }
        next += 1;
      }
    }
  }
  r.tries = tries;
  if nbest == 0 { return CsResult.{code=ifx exceeded then CS_MAX_WEIGHT else CS_INSUFFICIENT, algo=CS_CG, tries=tries}; }
  for k: 0..<nbest { r = cs_add(r, pool, order[best[k]], marks); }
  return r;
}
```

## Random numbers and Single Random Draw

Knapsack and Single Random Draw make random choices. Core draws them
from `FastRandomContext`, a ChaCha20 keystream. Here the stream is
seeded by the caller, so a seed fixes every result and Python can
follow the same draws. [Chapter 69](ch069-addresses-and-the-address-manager.md)'s
address manager reads the same stream.

Core shuffles with `std::shuffle`, whose use of the random stream
depends on the C++ standard library. No shuffle matches every Core
build, and Core's tests check only results that hold for every
shuffle. This code uses a Fisher-Yates shuffle over `randrange`.

<!-- source: coax/lib/bitcoin/coinselect.coax rng -->
```jai
// FastRandomContext (random.h): a ChaCha20 keystream with the seed as
// key and a zero nonce, read 8 bytes at a time, with RandomMixin's
// randbits, randrange and randbool. Chapter 69's address manager reads
// the same stream.
CsRng :: struct { key: [32] u8; block: u32; buf: [64] u8; left: int; bits: u64; nbits: int; }
cs_rng :: (seed: [] u8) -> CsRng {
  r: CsRng;
  for i: 0..<32 { r.key[i] = seed[i]; }
  return r;
}
cs_rand64 :: (r: [] CsRng) -> u64 {
  if r[0].left == 0 {
    nonce: [12] u8;
    c := chacha20_block_checked(r[0].key[..], nonce[..], r[0].block, r[0].buf[..], false);
    r[0].block += 1; r[0].left = 64;
  }
  v := cb_load_le64(r[0].buf[..], 64 - r[0].left).value;
  r[0].left -= 8;
  return v;
}
cs_randbits :: (r: [] CsRng, bits: int) -> u64 {
  if bits == 64 { return cs_rand64(r); }
  ret: u64;
  if bits <= r[0].nbits {
    ret = r[0].bits; r[0].bits >>= cast(u64) bits; r[0].nbits -= bits;
  } else {
    gen := cs_rand64(r);
    ret = (gen << cast(u64) r[0].nbits) | r[0].bits;
    r[0].bits = gen >> cast(u64) (bits - r[0].nbits);
    r[0].nbits = 64 + r[0].nbits - bits;
  }
  return ret & ((cast(u64) 1 << cast(u64) bits) - 1);
}
cs_randrange :: (r: [] CsRng, range: int) -> int {
  maxval := cast(u64) (range - 1);
  bits := 0;
  while (maxval >> cast(u64) bits) != 0 { bits += 1; }
  while true {
    v := cs_randbits(r, bits);
    if v <= maxval { return cast(int) v; }
  }
  return 0;
}
cs_randbool :: (r: [] CsRng) -> bool { return cs_randbits(r, 1) == 1; }
// A Fisher-Yates shuffle of a[0..<n], from the back. Core calls
// std::shuffle, whose draws depend on the C++ library, so no shuffle
// matches every Core build; Core's tests check only what holds for all.
cs_shuffle :: (r: [] CsRng, a: [] int, n: int) {
  i := n - 1;
  while i > 0 {
    j := cs_randrange(r, i + 1);
    t := a[i]; a[i] = a[j]; a[j] = t;
    i -= 1;
  }
}
```

Single Random Draw takes groups in random order until they cover the
target, the change fee and 50,000 satoshis, so its change is never
tiny. If the drawn groups get too heavy, it drops the least valuable
ones first.

<!-- source: coax/lib/bitcoin/coinselect.coax srd -->
```jai
// SelectCoinsSRD: draw groups in a random order until their effective
// value covers target + change_fee + CHANGE_LOWER, so the change is at
// least 50,000 satoshis. Over max_weight, drop the least valuable (on a
// tie the heaviest) groups drawn so far, as Core's priority queue does.
cs_srd :: (pool: [] CsGroup, target: int, change_fee: int, max_weight: int, r: [] CsRng, marks: [] int, work: [] int) -> CsResult {
  if marks.count != pool.count || work.count < 5 * pool.count { return cs_fail(CS_SRD, CS_CAPACITY); }
  m := pool.count;
  order := work[0..<m]; held := work[m..<2*m];
  n := cs_eligible(pool, order, true);
  cs_unmark(marks, CS_SRD);
  cs_shuffle(r, order, n);
  goal := target + CS_CHANGE_LOWER + change_fee;
  nheld := 0; amount := 0; weight := 0; exceeded := false;
  for k: 0..<n {
    i := order[k];
    held[nheld] = i; nheld += 1; amount += cs_amount(pool[i]); weight += pool[i].weight;
    if weight > max_weight {
      exceeded = true;
      while nheld > 0 && weight > max_weight {
        least := 0;                                       // the queue's top
        for h: 1..<nheld { if cs_before(pool[held[least]], pool[held[h]], true) { least = h; } }
        amount -= cs_amount(pool[held[least]]); weight -= pool[held[least]].weight;
        held[least] = held[nheld-1]; nheld -= 1;
      }
    }
    if amount >= goal {
      res := CsResult.{algo=CS_SRD, target=target, completed=true};
      for h: 0..<nheld { res = cs_add(res, pool, held[h], marks); }
      return res;
    }
  }
  return cs_fail(CS_SRD, ifx exceeded then CS_MAX_WEIGHT else CS_INSUFFICIENT);
}
```

## Knapsack

Knapsack is Core's oldest algorithm. A single group equal to the target
wins at once. Otherwise it collects the groups smaller than the target
plus the change target. If they add up to exactly the target, it uses
them all. If they fall short, it uses the smallest larger group.

Otherwise `ApproximateBestSubset` makes up to 1,000 random passes over
the smaller groups, keeping the smallest total at or above the target.
If that total isn't exact, Knapsack tries again for the target plus the
change target, so the change is worth having. The smallest larger group
still wins when it is as close.

Unlike the others, Knapsack also considers groups whose effective value
is zero or negative.

<!-- source: coax/lib/bitcoin/coinselect.coax knapsack -->
```jai
// ApproximateBestSubset: up to 1,000 random passes. Each includes each
// group with probability 1/2, then (second pass) the rest, and on
// reaching the target backs the last group out again to look for a
// closer total. best gets the smallest total at or above target.
cs_approximate :: (pool: [] CsGroup, groups: [] int, n: int, total_lower: int, target: int, max_weight: int,
                   r: [] CsRng, best: [] int, included: [] int) -> int {
  for i: 0..<n { best[i] = 1; }
  nbest := total_lower;
  rep := 0;
  while rep < 1000 && nbest != target {
    for i: 0..<n { included[i] = 0; }
    total := 0; weight := 0; reached := false;
    pass := 0;
    while pass < 2 && !reached {
      for i: 0..<n {
        take := included[i] == 0;
        if pass == 0 { take = cs_randbool(r); }      // a coin flip per group, every time
        if take {
          g := pool[groups[i]];
          total += cs_amount(g); weight += g.weight; included[i] = 1;
          if total >= target && weight <= max_weight {
            reached = true;
            if total < nbest { nbest = total; for k: 0..<n { best[k] = included[k]; } }
            total -= cs_amount(g); weight -= g.weight; included[i] = 0;
          }
        }
      }
      pass += 1;
    }
    rep += 1;
  }
  return nbest;
}
// KnapsackSolver: an exact match wins; else, from the groups smaller than
// target + change_target, the closest random subset at or above the
// target (or above target + change_target, to leave real change), unless
// the smallest larger group is as close.
cs_knapsack :: (pool: [] CsGroup, target: int, change_target: int, max_weight: int, r: [] CsRng, marks: [] int, work: [] int) -> CsResult {
  res := CsResult.{algo=CS_KNAPSACK, target=target, completed=true};
  if marks.count != pool.count || work.count < 5 * pool.count { return cs_fail(CS_KNAPSACK, CS_CAPACITY); }
  cs_unmark(marks, CS_KNAPSACK);
  m := pool.count;
  order := work[0..<m]; applicable := work[m..<2*m]; best := work[2*m..<3*m]; included := work[3*m..<4*m];
  n := cs_eligible(pool, order, false);                 // the mixed groups: Knapsack may spend dust
  cs_shuffle(r, order, n);
  exceeded := false; larger := -1; napp := 0; total_lower := 0;
  for k: 0..<n {
    i := order[k]; a := cs_amount(pool[i]);
    if pool[i].weight > max_weight { exceeded = true; }
    else if a == target { return cs_add(res, pool, i, marks); }
    else if a < target + change_target { applicable[napp] = i; napp += 1; total_lower += a; }
    else if larger < 0 || a < cs_amount(pool[larger]) { larger = i; }
  }
  if total_lower == target {
    for k: 0..<napp { res = cs_add(res, pool, applicable[k], marks); }
    if res.weight <= max_weight { return res; }
    exceeded = true;
    res = cs_clear(res, marks);
  }
  if total_lower < target {
    if larger < 0 { return cs_fail(CS_KNAPSACK, ifx exceeded then CS_MAX_WEIGHT else CS_INSUFFICIENT); }
    return cs_add(res, pool, larger, marks);
  }
  cs_sort(pool, applicable, napp, false);
  nbest := cs_approximate(pool, applicable, napp, total_lower, target, max_weight, r, best, included);
  if nbest != target && total_lower >= target + change_target {
    nbest = cs_approximate(pool, applicable, napp, total_lower, target + change_target, max_weight, r, best, included);
  }
  if larger >= 0 && ((nbest != target && nbest < target + change_target) || cs_amount(pool[larger]) <= nbest) {
    return cs_add(res, pool, larger, marks);
  }
  for k: 0..<napp { if best[k] == 1 { res = cs_add(res, pool, applicable[k], marks); } }
  if res.weight > max_weight {
    if larger < 0 { return cs_fail(CS_KNAPSACK, CS_MAX_WEIGHT); }
    res = cs_clear(res, marks);
    res = cs_add(res, pool, larger, marks);
  }
  return res;
}
```

## Choosing a result

`ChooseSelectionResult` runs Branch and Bound, Knapsack, CoinGrinder (at
high feerates) and Single Random Draw, then keeps the least waste. On a
tie it keeps the result with more inputs. Branch and Bound may use all
the weight the outputs leave; the others must also leave room for the
change output.

<!-- source: coax/lib/bitcoin/coinselect.coax choose -->
```jai
// ChooseSelectionResult: run every algorithm on the groups and keep the
// result with the least waste, the one with more inputs on a tie. BnB
// may use the whole weight left after the outputs; the others must also
// leave room for a change output. CoinGrinder runs only when the feerate
// is above three times the long-term feerate, where fewer inputs matter
// most. marks gets each algorithm's bit; the winner's is result.algo.
cs_choose :: (pool: [] CsGroup, target: int, p: CsParams, max_tx_weight: int, r: [] CsRng, marks: [] int, work: [] int) -> CsResult {
  max_weight := max_tx_weight - TX_SCALE * p.tx_noinputs_size;
  if max_weight <= 0 { return cs_fail(CS_MANUAL, CS_MAX_WEIGHT); }
  results: [4] CsResult; nres := 0; first_error := CS_INSUFFICIENT;
  b := cs_bnb(pool, target, p.cost_of_change, max_weight, marks, work);
  if b.code == CS_OK { results[nres] = b; nres += 1; } else if b.code == CS_MAX_WEIGHT { first_error = CS_MAX_WEIGHT; }
  max_weight -= TX_SCALE * p.change_output_size;
  if max_weight < 0 && nres == 0 { return cs_fail(CS_MANUAL, CS_MAX_WEIGHT); }
  k := cs_knapsack(pool, target, p.min_change_target, max_weight, r, marks, work);
  if k.code == CS_OK { results[nres] = k; nres += 1; } else if k.code == CS_MAX_WEIGHT { first_error = CS_MAX_WEIGHT; }
  if p.feerate > 3 * p.long_term {
    c := cs_coin_grinder(pool, target, p.min_change_target, max_weight, marks, work);
    if c.code == CS_OK { results[nres] = c; nres += 1; } else if c.code == CS_MAX_WEIGHT { first_error = CS_MAX_WEIGHT; }
  }
  s := cs_srd(pool, target, p.change_fee, max_weight, r, marks, work);
  if s.code == CS_OK { results[nres] = s; nres += 1; } else if s.code == CS_MAX_WEIGHT { first_error = CS_MAX_WEIGHT; }
  if nres == 0 { return cs_fail(CS_MANUAL, first_error); }
  win := 0;
  for i: 0..<nres {
    results[i].waste = cs_waste(results[i], p.min_viable_change, p.cost_of_change, p.change_fee);
    w := results[i]; best := results[win];
    if w.waste < best.waste || (w.waste == best.waste && w.count > best.count) { win = i; }
  }
  return results[win];
}
```

## The example

[bitcoin/examples/coinselect.coax](../examples/coinselect.coax) gives a
wallet ten coins, prints their effective values at three feerates and
the dust thresholds, and makes four payments to a P2TR address. Each
payment prints its parameters, what every algorithm chose, the winner
and the transaction it pays for. Last, it runs all of Core's cases.

<!-- source: bitcoin/examples/coinselect.coax example -->
```jai
#load "../../coax/lib/bitcoin/coinselect.coax";
#import "Basic";

// The wallet: ten coins, each with what spending it costs in virtual
// bytes: 148 for P2PKH, 68 for P2WPKH, 58 for a P2TR key path.
NAMES :: "ABCDEFGHIJ";
VALUES :: [10] int.[1000000, 250000, 3500, 600000, 50000, 2000000, 120000, 75000, 400000, 9000];
SIZES :: [10] int.[68, 58, 148, 68, 58, 68, 148, 58, 68, 68];
KINDS :: [10] string.["P2WPKH", "P2TR", "P2PKH", "P2WPKH", "P2TR", "P2WPKH", "P2PKH", "P2TR", "P2WPKH", "P2WPKH"];
LONG_TERM :: 10000;          // Core's -consolidatefeerate default, 10 sat/vB
DISCARD :: 3000;             // Core's -discardfee default, 3 sat/vB
P2TR_OUTPUT :: 43;           // a P2TR recipient's output: 8 + 1 + 34 bytes

wallet :: (feerate: int, pool: [] CsGroup) {
  for i: 0..<10 { pool[i] = cs_coin(VALUES[i], SIZES[i], feerate, LONG_TERM); }
}
put_inputs :: (marks: [] int, algo: int) {
  first := true;
  for i: 0..<10 {
    if (marks[i] >> algo) & 1 == 1 {
      if !first { print(" "); }
      print("%", NAMES[i..<i+1]); first = false;
    }
  }
}
// The result algo left in marks, with its change and waste.
put_result :: (pool: [] CsGroup, marks: [] int, algo: int, target: int, p: CsParams) {
  r := CsResult.{algo=algo, target=target};
  scratch: [10] int;
  for i: 0..<10 { if (marks[i] >> algo) & 1 == 1 { r = cs_add(r, pool, i, scratch[..]); } }
  if r.count == 0 { print("  %: no selection\n", CS_ALGOS[algo]); return; }
  print("  %: ", CS_ALGOS[algo]); put_inputs(marks, algo);
  change := cs_change(r, p.min_viable_change, p.change_fee);
  print(", effective %, weight %, change %, waste %\n", r.effective, r.weight, change, cs_waste(r, p.min_viable_change, p.cost_of_change, p.change_fee));
}
// One payment to a P2TR address at feerate: the parameters, every
// algorithm's result, the winner and the transaction it makes.
pay :: (title: string, payment: int, feerate: int, seed_byte: u8) {
  print("%: % sat to a P2TR address at % sat/kvB\n", title, payment, feerate);
  pool: [10] CsGroup; marks: [10] int; work: [50] int;
  wallet(feerate, pool[..]);
  seed: [32] u8; seed[0] = seed_byte; rng: [1] CsRng; rng[0] = cs_rng(seed[..]);
  change_spk: [22] u8; change_spk[1] = 20;           // a P2WPKH change output
  p := cs_params(feerate, LONG_TERM, DISCARD, change_spk[..], 68, 10 + 1 + P2TR_OUTPUT, payment, 1, rng[..]);
  target := payment + fee_for_rate_vartime(feerate, p.tx_noinputs_size);
  print("  target % (the payment and % sat for the transaction without inputs)\n", target, target - payment);
  print("  change fee %, cost of change %, smallest change %, change target %\n", p.change_fee, p.cost_of_change, p.min_viable_change, p.min_change_target);
  r := cs_choose(pool[..], target, p, FEE_MAX_STANDARD_WEIGHT, rng[..], marks[..], work[..]);
  for a: 0..<4 {
    if a == CS_CG && p.feerate <= 3 * p.long_term { print("  cg: not run below three times the long-term feerate\n"); }
    else { put_result(pool[..], marks[..], a, target, p); }
  }
  if r.code != CS_OK { print("  no selection: code %\n", r.code); return; }
  change := cs_change(r, p.min_viable_change, p.change_fee);
  vsize := p.tx_noinputs_size + r.weight / TX_SCALE + (ifx change > 0 then p.change_output_size else 0);
  fee := r.value - payment - change;
  print("  chosen: %, waste %; the transaction: % in, % out", CS_ALGOS[r.algo], r.waste, r.value, payment);
  if change > 0 { print(" and % change", change); } else { print(", no change"); }
  print(", fee %, % vB, % sat/kvB\n", fee, vsize, fee_per_kvb_vartime(fee, vsize));
}

main :: () {
  // What each coin is worth after the fee to spend it.
  print("The wallet's coins, at 1, 10 and 100 sat/vB:\n");
  for i: 0..<10 {
    print("  %: % sat %, effective", NAMES[i..<i+1], VALUES[i], KINDS[i]);
    for rate: [3] int.[1000, 10000, 100000] { print(" %", cs_amount(cs_coin(VALUES[i], SIZES[i], rate, LONG_TERM))); }
    print("\n");
  }
  // When change is worth making.
  p2wpkh: [22] u8; p2wpkh[1] = 20; p2tr: [34] u8; p2tr[0] = 0x51; p2tr[1] = 32; p2pkh: [25] u8; p2pkh[0] = 0x76; p2pkh[1] = 0xa9; p2pkh[2] = 20;
  print("Dust at the discard feerate, 3 sat/vB: P2PKH %, P2WPKH %, P2TR %\n",
        cs_dust_threshold_vartime(p2pkh[..], DISCARD), cs_dust_threshold_vartime(p2wpkh[..], DISCARD), cs_dust_threshold_vartime(p2tr[..], DISCARD));

  pay("Pay 830,000 at 35 sat/vB", 830000, 35000, 1);
  pay("Pay 995,730 at 35 sat/vB", 995730, 35000, 2);
  pay("Pay 830,000 at 2 sat/vB", 830000, 2000, 3);
  pay("Pay 4,600,000 at 35 sat/vB", 4600000, 35000, 4);

  // Bitcoin Core's own cases.
  print("Bitcoin Core's coin-selection cases:\n");
  pool: [50100] CsGroup; marks: [50100] int; work: [250500] int;
  for algo: 0..<4 {
    total := 0; pass := 0;
    for i: 0..<CS_CASES.count {
      if CS_CASES[i].algo == algo { total += 1; if cs_case_passes(CS_CASES[i], pool[..], marks[..], work[..]) { pass += 1; } }
    }
    print("  %: % of % give Core's result\n", CS_ALGOS[algo], pass, total);
  }
  pass := 0;
  for i: 0..<CS_WASTE.count {
    c := CS_WASTE[i];
    r := CsResult.{target=c.target, effective=c.inputs - c.fees, fee=c.fees, long_term_fee=c.long_term_fees, count=2};
    if cs_waste(r, c.min_viable_change, c.change_cost, c.change_fee) == c.waste { pass += 1; }
  }
  print("  waste: % of % give Core's result\n", pass, CS_WASTE.count);
}
```

```shell
coax/bin/coax run bitcoin/examples/coinselect.coax
```

The output:

```text
The wallet's coins, at 1, 10 and 100 sat/vB:
  A: 1000000 sat P2WPKH, effective 999932 999320 993200
  B: 250000 sat P2TR, effective 249942 249420 244200
  C: 3500 sat P2PKH, effective 3352 2020 -11300
  D: 600000 sat P2WPKH, effective 599932 599320 593200
  E: 50000 sat P2TR, effective 49942 49420 44200
  F: 2000000 sat P2WPKH, effective 1999932 1999320 1993200
  G: 120000 sat P2PKH, effective 119852 118520 105200
  H: 75000 sat P2TR, effective 74942 74420 69200
  I: 400000 sat P2WPKH, effective 399932 399320 393200
  J: 9000 sat P2WPKH, effective 8932 8320 2200
Dust at the discard feerate, 3 sat/vB: P2PKH 546, P2WPKH 294, P2TR 330
Pay 830,000 at 35 sat/vB: 830000 sat to a P2TR address at 35000 sat/kvB
  target 831890 (the payment and 1890 sat for the transaction without inputs)
  change fee 1085, cost of change 1289, smallest change 294, change target 760658
  bnb: no selection
  knapsack: A C D, effective 1593560, weight 1136, change 760585, waste 8389
  srd: E F J, effective 2052210, weight 776, change 1219235, waste 6139
  cg: F, effective 1997620, weight 272, change 1164645, waste 2989
  chosen: cg, waste 2989; the transaction: 2000000 in, 830000 out and 1164645 change, fee 5355, 153 vB, 35000 sat/kvB
Pay 995,730 at 35 sat/vB: 995730 sat to a P2TR address at 35000 sat/kvB
  target 997620 (the payment and 1890 sat for the transaction without inputs)
  change fee 1085, cost of change 1289, smallest change 294, change target 124663
  bnb: A, effective 997620, weight 272, change 0, waste 1700
  knapsack: A, effective 997620, weight 272, change 0, waste 1700
  srd: E F G H J, effective 2240000, weight 1600, change 1241295, waste 11289
  cg: F, effective 1997620, weight 272, change 998915, waste 2989
  chosen: bnb, waste 1700; the transaction: 1000000 in, 995730 out, no change, fee 4270, 122 vB, 35000 sat/kvB
Pay 830,000 at 2 sat/vB: 830000 sat to a P2TR address at 2000 sat/kvB
  target 830108 (the payment and 108 sat for the transaction without inputs)
  change fee 62, cost of change 266, smallest change 294, change target 857358
  bnb: no selection
  knapsack: A B E I, effective 1699496, weight 1008, change 869326, waste -1750
  srd: A C D G J, effective 1731500, weight 2000, change 901330, waste -3734
  cg: not run below three times the long-term feerate
  chosen: srd, waste -3734; the transaction: 1732500 in, 830000 out and 901330 change, fee 1170, 585 vB, 2000 sat/kvB
Pay 4,600,000 at 35 sat/vB: 4600000 sat to a P2TR address at 35000 sat/kvB
  target 4601890 (the payment and 1890 sat for the transaction without inputs)
  change fee 1085, cost of change 1289, smallest change 294, change target 961309
  bnb: no selection
  knapsack: no selection
  srd: no selection
  cg: no selection
  no selection: code -1
Bitcoin Core's coin-selection cases:
  bnb: 175 of 175 give Core's result
  knapsack: 30 of 30 give Core's result
  srd: 130 of 130 give Core's result
  cg: 9 of 9 give Core's result
  waste: 11 of 11 give Core's result
```

Coin C, 3,500 satoshis in P2PKH, is worth 2,020 at 10 sat/vB and
less than nothing at 100 sat/vB: spending it would lose money.

The first payment is at 35 sat/vB, above three times the long-term
feerate, so all four algorithms run. No subset lands within 1,289
satoshis of the target, so Branch and Bound finds nothing. CoinGrinder
picks coin F alone, one input of 272 weight units, and wins with the
least waste.

The second payment is chosen so that coin A's effective value is the
target exactly. Branch and Bound finds it, and the transaction has no
change. Its waste is only A's extra fee over the long-term rate, 1,700
satoshis. Knapsack finds the same coin, but Branch and Bound comes first.

The third payment is the first one at 2 sat/vB. Inputs now have negative
waste, so the winner is Single Random Draw's five inputs, which
consolidates the small coins C, G and J while fees are low.

The fourth asks for more than the wallet holds, and every algorithm
fails.

Every transaction pays at least its feerate. With change, the fee is
exactly the sum of the rounded-up fees of its parts. Without change, the
excess is below what a change output would return.

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/coinselect.coax](../examples/checks/coinselect.coax),
which loads the example. For 300 random pools of ten coins at a low and
a high feerate, they try all 1,023 nonempty subsets:

- Branch and Bound finds a changeless selection exactly when one exists,
  and none has less waste than its result;
- CoinGrinder finds a selection exactly when one covers the target and
  the change target, and none is lighter than its result.

<!-- source: bitcoin/examples/checks/coinselect.coax check -->
```jai
// Ten random coins and a random target, at a feerate above or below the
// long-term feerate.
random_pool :: (s: int, pool: [] CsGroup) -> [2] int {
  seed: [32] u8; seed[0] = cast(u8) (s & 255); seed[1] = cast(u8) (s >> 8);
  rng: [1] CsRng; rng[0] = cs_rng(seed[..]);
  feerate := ifx s % 2 == 0 then 3000 else 40000;
  for i: 0..<10 { pool[i] = cs_coin(1000 + cs_randrange(rng[..], 200000), ifx i % 3 == 0 then 148 else 68, feerate, 10000); }
  return [2] int.[feerate, 20000 + cs_randrange(rng[..], 600000)];
}
// Branch and Bound against every one of the 1,023 nonempty subsets: it
// finds a changeless selection exactly when one exists, and none has
// less waste than the one it returns.
#forall "BnB finds the least-waste changeless subset" s: 0..<300 #steps 600000000 {
  pool: [10] CsGroup; marks: [10] int; work: [50] int;
  x := random_pool(s, pool[..]);
  cost := 1289;
  r := cs_bnb(pool[..], x[1], cost, FEE_MAX_STANDARD_WEIGHT, marks[..], work[..]);
  best := FEE_MAX_MONEY;
  for mask: 1..<1024 {
    amount := 0; waste := 0; usable := true;
    for i: 0..<10 {
      if (mask >> i) & 1 == 1 {
        if cs_amount(pool[i]) <= 0 { usable = false; }
        amount += cs_amount(pool[i]); waste += pool[i].fee - pool[i].long_term_fee;
      }
    }
    if usable && amount >= x[1] && amount <= x[1] + cost && waste + amount - x[1] < best { best = waste + amount - x[1]; }
  }
  #assert (r.code == CS_OK) == (best < FEE_MAX_MONEY);
  if r.code == CS_OK {
    #assert r.effective >= x[1] && r.effective <= x[1] + cost && r.completed;
    #assert (r.fee - r.long_term_fee) + r.effective - x[1] == best;
  }
}
// CoinGrinder against every subset: no subset that covers the target
// and the change target is lighter than the one it returns.
#forall "CoinGrinder finds the lightest subset that leaves change" s: 0..<300 #steps 600000000 {
  pool: [10] CsGroup; marks: [10] int; work: [50] int;
  x := random_pool(s, pool[..]);
  change_target := 50000;
  r := cs_coin_grinder(pool[..], x[1], change_target, FEE_MAX_STANDARD_WEIGHT, marks[..], work[..]);
  lightest := FEE_MAX_MONEY;
  for mask: 1..<1024 {
    amount := 0; weight := 0; usable := true;
    for i: 0..<10 {
      if (mask >> i) & 1 == 1 {
        if cs_amount(pool[i]) <= 0 { usable = false; }
        amount += cs_amount(pool[i]); weight += pool[i].weight;
      }
    }
    if usable && amount >= x[1] + change_target && weight < lightest { lightest = weight; }
  }
  #assert (r.code == CS_OK) == (lightest < FEE_MAX_MONEY);
  if r.code == CS_OK { #assert r.weight == lightest && r.effective >= x[1] + change_target; }
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/coinselect.coax
```

`coax test` runs 86 checks, including the loaded files' own checks, on 2026-11-03.

### The library's own checks

`coinselect.coax` is an `#authority` file. Its checks run all 344 of
Core's selection cases: `bnb_test` and `srd_test` at all ten of their
feerates, `bnb_exhaustion_with_solution_test`,
`bnb_feerate_sensitivity_test`, `coin_grinder_tests`, the
`knapsack_solver_test` cases that hold for every draw, and the
`ApproximateBestSubset` test. Each gives Core's input values, attempt
counts, totals or failure. The checks also run all eleven `waste_test`
cases, `effective_value_test`, the dust thresholds from `policy.cpp`'s
comments, the change target's range, the first words of the ChaCha20
stream, avoid-reuse groups and the refusals. Sixty random wallets check
that a chosen selection pays its feerate, overpays by less than the
change it skips, and has no more waste than any other algorithm's
result.

```shell
coax/bin/coax test coax/lib/bitcoin/coinselect.coax
```

`coax test` runs 84 checks, including those of the files it loads, on 2026-11-03. Mutation testing: not run (owner speed order).

### Python

[coinselect_reference.py](../../coax/tests/bitcoin/coinselect_reference.py)
transliterates Core's `coinselection.cpp` into Python, and checks that
the lines it follows still read that way in the pinned sources.
[make_coinselect_vectors.py](../../coax/tests/bitcoin/make_coinselect_vectors.py)
copies each case from the pinned tests, checks that its title is there,
recomputes Core's published result with the Python transliteration, and
writes `coinselect_vectors.coax`.
[coinselect.py](../../coax/tests/bitcoin/coinselect.py) writes the
transcript the example must print, with the same seeds and the random
stream of chapter 69's `addrman.py`.

```shell
python3 -I coax/tests/bitcoin/make_coinselect_vectors.py
python3 -I coax/tests/bitcoin/coinselect.py --coax coax/bin/coax
```

On 2026-11-03 every case recomputes, all 50 lines of the transcript agree with Python, and the example's output in native debug and release builds equals them. Wasm was not run.

## What this chapter leaves out

- Subtracting the fee from the outputs (`m_subtract_fee_outputs`), under
  which Core skips Branch and Bound and selects on amounts.
- Unconfirmed coins: the bump fee to raise their ancestors to the
  feerate, and the eligibility filters on confirmations, ancestors and
  cluster size.
- Preset inputs, coins of mixed output types, and Core's retry without
  avoid-reuse when grouping costs too much.
- Building, signing and broadcasting the transaction: chapter 94's PSBT
  roles do that, and chapter 99's descriptor wallet puts them together.

## Summary

- A coin's effective value is its amount minus the fee to spend it now.
  Selecting effective values against a target that includes the
  non-input fee pays every fee exactly.
- Change costs its output's fee now and its input's fee later. Change
  below that, or below dust, goes to the miners.
- Branch and Bound finds changeless sets within the cost of change;
  CoinGrinder finds the lightest set with change at high feerates;
  Knapsack and Single Random Draw are randomized fallbacks.
- Waste, the inputs' extra fee over the long-term rate plus the cost of
  change or the excess, picks the winner. Low feerates favor more
  inputs, and high feerates favor fewer.

## Dependencies

- [11. ChaCha20](ch011-chacha20.md)
- [40. Weight, vsize and fees](ch040-weight-vsize-and-fees.md)
- [94. PSBT](ch094-psbt.md)

## Sources

- Bitcoin Core's `wallet/coinselection.cpp` and `coinselection.h`
  (`SelectCoinsBnB`, `CoinGrinder`, `SelectCoinsSRD`, `KnapsackSolver`,
  `GenerateChangeTarget`, `RecalculateWaste`), `wallet/spend.cpp`
  (`ChooseSelectionResult`, `CreateTransactionInternal`'s change
  parameters) and `policy/feerate.cpp`, with the tests
  `wallet/test/coinselection_tests.cpp` and `coinselector_tests.cpp`, pinned in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/coinselection.cpp).
- `policy/policy.cpp` (`GetDustThreshold`) and `random.h`
  (`FastRandomContext`), pinned for chapters 40 and 69.
- Mark Erhardt, [An Evaluation of Coin Selection Strategies](https://murch.one/wp-content/uploads/2016/11/erhardt2016coinselection.pdf)
  (2016), which introduced Branch and Bound and the waste metric.
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
