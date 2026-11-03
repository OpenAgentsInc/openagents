# 52. Transaction checks outside script

Chapter 36's interpreter decides whether an input's scripts allow a spend.
Before any script runs, a transaction must pass simpler rules: it has
inputs and outputs, its amounts are in range, it does not spend one coin
twice, and the coins it spends exist and are old enough. These checks are
cheap, and two of them closed bugs that once let anyone create bitcoin.

This chapter builds Bitcoin Core's three checks outside script.
`CheckTransaction` reads only the transaction. `CheckTxInputs` reads the
coins it spends. The lock-time rule in `ContextualCheckBlock` reads the
block it is in. Every case of Core's `tx_invalid.json` that Core marks
`BADTX` refuses with its reason, and every other case of both test files
passes.

## What this chapter teaches

- `CheckTransaction`: the rules that need nothing but the transaction.
- Why output values and their running total must stay within
  `MAX_MONEY` (CVE-2010-5139).
- Why a transaction may not spend the same outpoint twice
  (CVE-2018-17144).
- The coinbase's script length, and why a null outpoint is allowed only in
  a coinbase.
- `CheckTxInputs`: missing coins, coinbase maturity, input ranges and the
  fee.
- When a transaction is final in a block, and how BIP-113 changed the
  time it is measured against.

## Where Bitcoin uses these checks

| Check | Who calls it | Chapter |
| --- | --- | --- |
| `CheckTransaction` | every transaction of a block (`CheckBlock`), and every transaction the mempool considers | 57, 82 |
| `CheckTxInputs` | connecting a block (`ConnectBlock`), and the mempool | 58, 82 |
| Lock time in a block | `ContextualCheckBlock` | 57 |
| Sequence locks (BIP-68) | `ConnectBlock`, with chapter 39's `locktime_sequence_locks_vartime` | 39, 58 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [txcheck.coax](../../coax/lib/bitcoin/txcheck.coax) | `txcheck_vartime` | Core's `CheckTransaction` |
| | `txc_null_vartime`, `txc_same_vartime` | `COutPoint::IsNull` and equality |
| | `TxcCoin`, `txcheck_inputs_vartime` | Core's `Coin` and `CheckTxInputs` |
| | `txcheck_final_vartime` | the lock-time rule of `ContextualCheckBlock` |
| [tx.coax](../../coax/lib/bitcoin/tx.coax) | `tx_read_vartime`, `tx_coinbase_vartime` | chapter 34's reader and `IsCoinBase` |
| [fees.coax](../../coax/lib/bitcoin/fees.coax) | `FEE_MAX_MONEY`, `fee_vartime` | chapter 40's `MAX_MONEY` and fee |
| [locktime.coax](../../coax/lib/bitcoin/locktime.coax) | `locktime_final_vartime` | chapter 39's `IsFinalTx` |

## Result codes

Each code is one of Core's reject reasons:

<!-- source: coax/lib/bitcoin/txcheck.coax codes -->
```jai
// Result codes, with Core's reject reason.
TXC_OK :: 0;
TXC_VIN_EMPTY :: 1;          // "bad-txns-vin-empty"
TXC_VOUT_EMPTY :: 2;         // "bad-txns-vout-empty"
TXC_OVERSIZE :: 3;           // "bad-txns-oversize"
TXC_VOUT_NEGATIVE :: 4;      // "bad-txns-vout-negative"
TXC_VOUT_TOOLARGE :: 5;      // "bad-txns-vout-toolarge"
TXC_VOUT_TOTAL :: 6;         // "bad-txns-txouttotal-toolarge"
TXC_DUPLICATE :: 7;          // "bad-txns-inputs-duplicate"
TXC_CB_LENGTH :: 8;          // "bad-cb-length"
TXC_PREVOUT_NULL :: 9;       // "bad-txns-prevout-null"
TXC_MISSING :: 10;           // "bad-txns-inputs-missingorspent"
TXC_PREMATURE :: 11;         // "bad-txns-premature-spend-of-coinbase"
TXC_INPUT_RANGE :: 12;       // "bad-txns-inputvalues-outofrange"
TXC_IN_BELOW_OUT :: 13;      // "bad-txns-in-belowout"
TXC_FEE_RANGE :: 14;         // "bad-txns-fee-outofrange": unreachable, as in Core
TXC_NONFINAL :: 15;          // "bad-txns-nonfinal"
TXC_CONTEXT :: 16;           // the transaction did not read, or the coins are not one per input
TXC_REASONS :: [17] string.["ok", "bad-txns-vin-empty", "bad-txns-vout-empty", "bad-txns-oversize",
  "bad-txns-vout-negative", "bad-txns-vout-toolarge", "bad-txns-txouttotal-toolarge",
  "bad-txns-inputs-duplicate", "bad-cb-length", "bad-txns-prevout-null",
  "bad-txns-inputs-missingorspent", "bad-txns-premature-spend-of-coinbase",
  "bad-txns-inputvalues-outofrange", "bad-txns-in-belowout", "bad-txns-fee-outofrange",
  "bad-txns-nonfinal", "bad context"];
```

`TXC_FEE_RANGE` cannot happen. The input total is at most `MAX_MONEY`, and
the fee is checked only when the inputs cover the outputs, so the fee is
between 0 and `MAX_MONEY`. Core keeps the check, and so does this
chapter.

## Amounts

Amounts are signed 64-bit numbers of satoshis. `MAX_MONEY` is 21 million
bitcoin, 2,100,000,000,000,000 satoshis. It is not the supply: it is a
bound that no amount, and no sum of amounts, may pass. Chapter 40 defined
it; this chapter adds the range test Core calls `MoneyRange`:

<!-- source: coax/lib/bitcoin/txcheck.coax limits -->
```jai
TXC_COINBASE_MATURITY :: 100;   // consensus.h COINBASE_MATURITY
TXC_CB_MIN :: 2;                // a coinbase scriptSig's bounds in CheckTransaction
TXC_CB_MAX :: 100;
// MAX_MONEY is chapter 40's FEE_MAX_MONEY, MAX_BLOCK_WEIGHT its
// FEE_MAX_BLOCK_WEIGHT, and WITNESS_SCALE_FACTOR chapter 34's TX_SCALE.
txc_money_range :: (v: int) -> bool #export_c { return v >= 0 && v <= FEE_MAX_MONEY; }
```

## Outpoints

An input names the coin it spends by an outpoint: a transaction ID and an
output index, 36 bytes. A coinbase spends nothing, so its single input
has the null outpoint: 32 zero bytes and index `0xffffffff`.

<!-- source: coax/lib/bitcoin/txcheck.coax outpoints -->
```jai
// An outpoint is 36 bytes: the txid, then the output index. It is null
// when the txid is zero and the index is 0xffffffff.
txc_null_vartime :: (b: [] u8, at: int) -> bool #export_c {
  for i: 0..<32 { if b[at+i] != 0 { return false; } }
  return tx_le_vartime(b, at + 32, 4) == 0xffffffff;
}
txc_same_vartime :: (b: [] u8, x: int, y: int) -> bool #export_c {
  for i: 0..<36 { if b[x+i] != b[y+i] { return false; } }
  return true;
}
```

## CheckTransaction

`txcheck_vartime` applies Core's rules in Core's order:

<!-- source: coax/lib/bitcoin/txcheck.coax check -->
```jai
// Core's CheckTransaction, its rules in its order, on a transaction read
// by tx_read_vartime. The size is the stripped size: the witness is not
// counted, because it is not yet known to be the right one. The duplicate
// search compares every pair of inputs; Core sorts them into a set.
txcheck_vartime :: (b: [] u8, t: Tx, ins: [] TxIn, outs: [] TxOut) -> int #export_c {
  if t.code != TX_OK { return TXC_CONTEXT; }
  if t.nin == 0 { return TXC_VIN_EMPTY; }
  if t.nout == 0 { return TXC_VOUT_EMPTY; }
  if t.stripped * TX_SCALE > FEE_MAX_BLOCK_WEIGHT { return TXC_OVERSIZE; }
  total := 0;
  for k: 0..<t.nout {
    v := tx_le_vartime(b, outs[k].value, 8);
    if v < 0 { return TXC_VOUT_NEGATIVE; }
    if v > FEE_MAX_MONEY { return TXC_VOUT_TOOLARGE; }
    total += v;
    if !txc_money_range(total) { return TXC_VOUT_TOTAL; }
  }
  for i: 1..<t.nin {
    for j: 0..<i { if txc_same_vartime(b, ins[i].outpoint, ins[j].outpoint) { return TXC_DUPLICATE; } }
  }
  if tx_coinbase_vartime(b, t, ins) {
    if ins[0].script_n < TXC_CB_MIN || ins[0].script_n > TXC_CB_MAX { return TXC_CB_LENGTH; }
  } else {
    for k: 0..<t.nin { if txc_null_vartime(b, ins[k].outpoint) { return TXC_PREVOUT_NULL; } }
  }
  return TXC_OK;
}
```

1. **At least one input and one output.** A transaction with no inputs
   cannot be written in the usual form: its zero input count is the segwit
   marker. Read with witnesses, `01000000 00 00 00000000` has a flag of
   0, so the reader stops with no inputs and no outputs.
2. **A size that fits a block.** The size without witnesses, times 4, may
   not exceed 4,000,000 weight units. The witness is not counted, because
   at this point nothing has checked that it is the witness the block
   commits to.
3. **Output values in range.** Each value must be between 0 and
   `MAX_MONEY`, and so must the running total. In August 2010, block
   74,638 created over 184 billion bitcoin: its transaction had two outputs
   whose values overflowed when added, so the sum looked small
   (CVE-2010-5139). The running-total check closes that.
4. **No outpoint twice.** Bitcoin Core 0.14 to 0.16.2 skipped this check
   for transactions in blocks, as an optimization, on the assumption that
   spending the coins later would catch a duplicate. It did not catch one
   within a single transaction. A block spending one coin twice could crash
   a node and, from 0.15, create coins (CVE-2018-17144). The bug was found
   and fixed in September 2018, and Core now runs this check for every
   transaction.
5. **A coinbase script of 2 to 100 bytes**, and outside a coinbase, no
   null outpoint. A null outpoint in a transaction with two inputs is not
   a coinbase: `IsCoinBase` requires exactly one input.

## Core's vectors

Core's `transaction_tests.cpp` reads every case of `tx_invalid.json` and
checks that `CheckTransaction` fails exactly when the case is marked
`BADTX`; every case of `tx_valid.json` must pass. The example runs both
files:

```text
  BADTX No outputs (1 inputs, 0 outputs): bad-txns-vout-empty
  BADTX Negative output (1 inputs, 1 outputs): bad-txns-vout-negative
  BADTX MAX_MONEY + 1 output (1 inputs, 1 outputs): bad-txns-vout-toolarge
  BADTX MAX_MONEY output + 1 output (1 inputs, 2 outputs): bad-txns-txouttotal-toolarge
  BADTX Duplicate inputs (2 inputs, 1 outputs): bad-txns-inputs-duplicate
  BADTX Coinbase of size 1 (1 inputs, 1 outputs): bad-cb-length
  BADTX Coinbase of size 101 (1 inputs, 1 outputs): bad-cb-length
  BADTX Null txin, but without being a coinbase (because there are two inputs) (2 inputs, 1 outputs): bad-txns-prevout-null
  BADTX Null txin, but without being a coinbase (because there are two inputs) (2 inputs, 1 outputs): bad-txns-prevout-null
  tx_valid.json: 122 of 122 pass
  tx_invalid.json without BADTX: 84 of 84 pass (their scripts fail)
```

The 84 other invalid cases fail in their scripts, which chapter 36
checks. Chapter 34's legacy example, a coinbase with a one-byte script,
refuses as `bad-cb-length`: it reads, but it is not a valid transaction.

## CheckTxInputs

`CheckTxInputs` runs when a transaction is connected to the chain, with
the coins it spends. The UTXO set (chapter 56) holds each coin's value, the
height of the block that created it, and whether it came from a coinbase.
`TxcCoin` holds the same:

<!-- source: coax/lib/bitcoin/txcheck.coax inputs -->
```jai
// The coin an input spends, as the UTXO set holds it (chapter 56): found
// is false when it is missing or spent.
TxcCoin :: struct { found: bool; value: int; height: int; coinbase: bool; }
TxcInputs :: struct { code: int; fee: int; }
// Core's CheckTxInputs for a transaction that passed CheckTransaction,
// spent in a block at spend_height. coins holds one coin per input.
txcheck_inputs_vartime :: (b: [] u8, t: Tx, outs: [] TxOut, coins: [] TxcCoin, spend_height: int) -> TxcInputs #export_c {
  r: TxcInputs;
  if t.code != TX_OK || coins.count != t.nin { r.code = TXC_CONTEXT; return r; }
  for k: 0..<coins.count { if !coins[k].found { r.code = TXC_MISSING; return r; } }
  value_in := 0;
  for k: 0..<coins.count {
    c := coins[k];
    if c.coinbase && spend_height - c.height < TXC_COINBASE_MATURITY { r.code = TXC_PREMATURE; return r; }
    value_in += c.value;
    if !txc_money_range(c.value) || !txc_money_range(value_in) { r.code = TXC_INPUT_RANGE; return r; }
  }
  value_out := 0;
  for k: 0..<t.nout { value_out += tx_le_vartime(b, outs[k].value, 8); }
  if value_in < value_out { r.code = TXC_IN_BELOW_OUT; return r; }
  r.fee = value_in - value_out;
  if !txc_money_range(r.fee) { r.code = TXC_FEE_RANGE; return r; }
  return r;
}
```

- **Missing or spent.** Every input's coin must be in the UTXO set.
- **Coinbase maturity.** A coinbase's coins may be spent only 100 blocks
  after the coinbase. If the coinbase's block leaves the chain in a
  reorganization, its coins disappear; the wait makes that unlikely to
  undo transactions that spent them. A coin from height 500 is spendable
  in the block at height 600.
- **Input ranges.** Each coin's value and the running total must be in
  range. The coins in the UTXO set passed `CheckTransaction` when they
  were created, so this check guards against a damaged database.
- **The inputs cover the outputs.** The fee is the difference.

```text
  a coinbase coin from height 500, spent at 599: bad-txns-premature-spend-of-coinbase (fee 0)
  a coinbase coin from height 500, spent at 600: ok (fee 500000)
  a coin of 999999: bad-txns-in-belowout (fee 0)
  a coin of 1000000: ok (fee 0)
```

Chapter 40's `fee_vartime` computes the same fee with the same range
checks; the checks below compare the two.

## Final transactions in a block

A transaction with a lock time is not final until a height or time.
Chapter 39 built `IsFinalTx`. In a block, the time it is compared with
depends on CSV. Before CSV's height, a time lock is compared with the
block's own time, which the miner chooses. From CSV's height (BIP-113), it
is compared with the median time past of the 11 blocks before (chapter
51), which no single miner controls:

<!-- source: coax/lib/bitcoin/txcheck.coax final -->
```jai
// ContextualCheckBlock's lock-time rule: every transaction of a block at
// height must be final. Before CSV, IsFinalTx compares time lock times
// with the block's own time; from CSV's height (BIP-113) with the median
// time past of the 11 blocks before it.
txcheck_final_vartime :: (p: ChainParams, b: [] u8, t: Tx, ins: [] TxIn, height: int, prev_mtp: int, block_time: int) -> int #export_c {
  if t.code != TX_OK { return TXC_CONTEXT; }
  cutoff := ifx chain_deployment_active(p, DEP_CSV, height) then prev_mtp else block_time;
  if !locktime_final_vartime(b, t, ins, height, cutoff) { return TXC_NONFINAL; }
  return TXC_OK;
}
```

On mainnet, CSV is active from height 419,328:

```text
  height 419327, median 1499999999 and block time 1500000001: ok
  height 419327, median 1500000001 and block time 1500000000: bad-txns-nonfinal
  height 419328, median 1499999999 and block time 1500000001: bad-txns-nonfinal
  height 419328, median 1500000001 and block time 1500000000: ok
```

Relative lock times (BIP-68) are also checked when a block is connected,
with chapter 39's `locktime_sequence_locks_vartime` and the heights and
times of the coins spent. Chapter 58 connects them.

## The audit

Chapter 34's `tx.coax` had `tx_coinbase_vartime`. Chapter 40's
`fees.coax` had `MAX_MONEY` and a fee with Core's range checks, but no
reasons and no coinbase maturity. Chapter 39's `locktime.coax` had
`IsFinalTx` and sequence locks. Nothing checked for empty inputs or
outputs, the size, duplicate inputs, the coinbase's script length or null
outpoints before this chapter.

## The example

The example is
[bitcoin/examples/tx-checks.coax](../examples/tx-checks.coax). It runs
`CheckTransaction` over Core's vectors and a few more cases, then
`CheckTxInputs` over coins of several values and ages, and the lock-time
rule on both sides of CSV's height.

<!-- source: bitcoin/examples/tx-checks.coax example -->
```jai
#load "../../coax/lib/bitcoin/txcheck.coax";
#import "Basic";

reason :: (code: int) -> string { return TXC_REASONS[code]; }
put_le :: (raw: [] u8, at: int, v: int, width: int) {
  for j: 0..<width { raw[at+j] = cast,trunc(u8) (v >> (8 * j)); }
}

main :: () {
  raw: [2048] u8; ins: [16] TxIn; outs: [16] TxOut; items: [16] TxItem;

  // Bitcoin Core's tx_valid.json and tx_invalid.json through CheckTransaction.
  print("CheckTransaction on Core's vectors:\n");
  valid := 0; valid_ok := 0; other := 0; other_ok := 0;
  for i: 0..<TXCHECK_CASES.count {
    c := TXCHECK_CASES[i];
    r := txc_read(c.tx, raw[..], ins[..], outs[..], items[..]);
    code := txcheck_vartime(raw[0..<r.n], r.t, ins[..], outs[..]);
    if c.badtx { print("  BADTX % (% inputs, % outputs): %\n", c.note, r.t.nin, r.t.nout, reason(code)); }
    else if c.valid { valid += 1; if code == TXC_OK { valid_ok += 1; } }
    else { other += 1; if code == TXC_OK { other_ok += 1; } }
  }
  print("  tx_valid.json: % of % pass\n", valid_ok, valid);
  print("  tx_invalid.json without BADTX: % of % pass (their scripts fail)\n", other_ok, other);
  print("  no inputs: %\n", reason(txc_code("01000000000000000000")));
  print("  chapter 34's one-byte coinbase: %\n", reason(txc_code(TX_LEGACY_OK)));
  r := txc_read(TXCHECK_CASES[0].tx, raw[..], ins[..], outs[..], items[..]);
  t := r.t;
  print("  tx_valid's first case, % bytes stripped: %\n", t.stripped, reason(txcheck_vartime(raw[0..<r.n], t, ins[..], outs[..])));
  t.stripped = FEE_MAX_BLOCK_WEIGHT / TX_SCALE;
  print("  as if % bytes: %\n", t.stripped, reason(txcheck_vartime(raw[0..<r.n], t, ins[..], outs[..])));
  t.stripped += 1;
  print("  as if % bytes: %\n", t.stripped, reason(txcheck_vartime(raw[0..<r.n], t, ins[..], outs[..])));

  // CheckTxInputs: the first case again, paying 1,000,000 satoshis.
  print("CheckTxInputs, one output of 1000000:\n");
  put_le(raw[..], outs[0].value, 1000000, 8);
  b := raw[0..<r.n];
  coin: [1] TxcCoin;
  coin[0] = TxcCoin.{found=true, value=1500000, height=500, coinbase=true};
  for spend: 598..<601 {
    x := txcheck_inputs_vartime(b, r.t, outs[..], coin[..], spend);
    print("  a coinbase coin from height 500, spent at %: % (fee %)\n", spend, reason(x.code), x.fee);
  }
  coin[0].coinbase = false;
  values := [5] int.[999999, 1000000, FEE_MAX_MONEY, FEE_MAX_MONEY + 1, -1];
  for i: 0..<5 {
    coin[0].value = values[i];
    x := txcheck_inputs_vartime(b, r.t, outs[..], coin[..], 501);
    print("  a coin of %: % (fee %)\n", values[i], reason(x.code), x.fee);
  }
  coin[0].found = false;
  print("  a coin that is spent: %\n", reason(txcheck_inputs_vartime(b, r.t, outs[..], coin[..], 501).code));
  r2 := txc_read(TXCHECK_CASES[9].tx, raw[..], ins[..], outs[..], items[..]);
  two: [16] TxcCoin;
  for k: 0..<r2.t.nin { two[k] = TxcCoin.{found=true, value=FEE_MAX_MONEY / 2 + 1, height=1}; }
  print("  % coins of % each: %\n", r2.t.nin, FEE_MAX_MONEY / 2 + 1, reason(txcheck_inputs_vartime(raw[0..<r2.n], r2.t, outs[..], two[0..<r2.t.nin], 10).code));

  // Lock time in a block, before and after CSV's height on mainnet.
  print("final in a block (lock time 1500000000, sequence 0xfffffffe):\n");
  r = txc_read(TXCHECK_CASES[0].tx, raw[..], ins[..], outs[..], items[..]);
  lock := 1500000000;
  put_le(raw[..], r.t.locktime, lock, 4);
  put_le(raw[..], ins[0].sequence, 0xfffffffe, 4);
  b = raw[0..<r.n];
  m := CHAIN_MAINNET_PARAMS;
  print("  CSV active from %\n", chain_deployment_height(m, DEP_CSV));
  heights := [2] int.[419327, 419328];
  for i: 0..<2 {
    print("  height %, median % and block time %: %\n", heights[i], lock - 1, lock + 1, reason(txcheck_final_vartime(m, b, r.t, ins[..], heights[i], lock - 1, lock + 1)));
    print("  height %, median % and block time %: %\n", heights[i], lock + 1, lock, reason(txcheck_final_vartime(m, b, r.t, ins[..], heights[i], lock + 1, lock)));
  }
  put_le(raw[..], ins[0].sequence, 0xffffffff, 4);
  print("  with a final sequence, median and time 0: %\n", reason(txcheck_final_vartime(m, b, r.t, ins[..], 419328, 0, 0)));
}
```

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/tx-checks.out](../fixtures/tx-checks.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/tx-checks.coax](../examples/checks/tx-checks.coax),
which loads the example:

- `CheckTxInputs` and chapter 40's `fee_vartime` accept the same 12 coin
  values and give the same fee;
- a coinbase's coin is premature at every depth below 100 and spendable
  at every depth from 100, up to 200;
- the lock-time rule equals chapter 39's `IsFinalTx` with the block's time
  before CSV and the median from CSV's height, at the heights around CSV
  on every network;
- `CheckTransaction` gives the same answer for chapter 34's legacy and
  extended readings of every vector that both read.

<!-- source: bitcoin/examples/checks/tx-checks.coax check -->
```jai
// CheckTxInputs and chapter 40's fee agree: for a coin that is not a
// coinbase, both accept the same values and give the same fee.
#forall "the fee is chapter 40's" i: 0..<12 #steps 100000000 {
  raw: [2048] u8; ins: [16] TxIn; outs: [16] TxOut; items: [16] TxItem;
  r := txc_read(TXCHECK_CASES[0].tx, raw[..], ins[..], outs[..], items[..]);
  put_le(raw[..], outs[0].value, 1000000, 8);
  b := raw[0..<r.n];
  values := [12] int.[-2, -1, 0, 1, 999999, 1000000, 1000001, 5000000, FEE_MAX_MONEY - 1, FEE_MAX_MONEY, FEE_MAX_MONEY + 1, 1 << 62];
  coin: [1] TxcCoin;
  coin[0] = TxcCoin.{found=true, value=values[i], height=1};
  x := txcheck_inputs_vartime(b, r.t, outs[..], coin[..], 2);
  v := [1] int.[values[i]];
  f := fee_vartime(b, r.t, outs[..], v[..]);
  #assert (x.code == TXC_OK) == f.ok;
  if f.ok { #assert x.fee == f.fee; }
}
// A coinbase's coin is spendable exactly 100 blocks later.
#forall "coinbase maturity is 100 blocks" depth: 0..<200 #steps 100000000 {
  raw: [2048] u8; ins: [16] TxIn; outs: [16] TxOut; items: [16] TxItem;
  r := txc_read(TXCHECK_CASES[0].tx, raw[..], ins[..], outs[..], items[..]);
  coin: [1] TxcCoin;
  coin[0] = TxcCoin.{found=true, value=FEE_MAX_MONEY, height=1000, coinbase=true};
  code := txcheck_inputs_vartime(raw[0..<r.n], r.t, outs[..], coin[..], 1000 + depth).code;
  #assert (code == TXC_PREMATURE) == (depth < TXC_COINBASE_MATURITY) && (code == TXC_OK) == (depth >= TXC_COINBASE_MATURITY);
}
// The lock-time rule is chapter 39's IsFinalTx, given the block's time
// before CSV and the median time past from CSV's height, on every network.
#forall "the cutoff switches at CSV's height" chain: 1..<6 #steps 100000000 {
  raw: [2048] u8; ins: [16] TxIn; outs: [16] TxOut; items: [16] TxItem;
  r := txc_read(TXCHECK_CASES[0].tx, raw[..], ins[..], outs[..], items[..]);
  lock := 1500000000;
  put_le(raw[..], r.t.locktime, lock, 4);
  put_le(raw[..], ins[0].sequence, 0, 4);
  b := raw[0..<r.n];
  p := chain_params(chain);
  csv := chain_deployment_height(p, DEP_CSV);
  for d: -2..<3 {
    h := csv + d;
    if h < 0 { continue; }
    for k: 0..<3 {
      mtp := lock - 1 + k; time := lock + 1 - k;
      want := locktime_final_vartime(b, r.t, ins[..], h, ifx h >= csv then mtp else time);
      #assert (txcheck_final_vartime(p, b, r.t, ins[..], h, mtp, time) == TXC_OK) == want;
    }
  }
}
// CheckTransaction reads nothing but the transaction: chapter 34's two
// readings of the same bytes, when both read, get the same answer.
#forall "legacy and extended readings agree" i: 0..<TXCHECK_CASES.count #steps 100000000 {
  raw: [2048] u8; ins: [16] TxIn; outs: [16] TxOut; items: [16] TxItem;
  n := cp_unhex(TXCHECK_CASES[i].tx, raw[..]);
  b := raw[0..<n];
  a := tx_read_vartime(b, true, ins[..], outs[..], items[..]);
  ca := txcheck_vartime(b, a, ins[..], outs[..]);
  l := tx_read_vartime(b, false, ins[..], outs[..], items[..]);
  if a.code == TX_OK && l.code == TX_OK && !a.segwit { #assert txcheck_vartime(b, l, ins[..], outs[..]) == ca; }
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/tx-checks.coax
```

`coax test` runs 297 checks, including the loaded files' own checks, in
about 110 seconds on 2026-11-03.

### The library's own checks

`txcheck.coax` is an `#authority` file. Its checks run all 215 of Core's
vectors with the reason expected for each `BADTX` case. They also cover a
transaction with no inputs and the size at its bound and one byte past
it. `CheckTxInputs` is checked with a coinbase at 99 and 100 blocks,
coins of each boundary value, a spent coin, a wrong number of coins, and
two coins whose total overflows. The lock-time rule is checked on both
sides of CSV's height, and the outpoint helpers on null and equal
outpoints.

```shell
coax/bin/coax test coax/lib/bitcoin/txcheck.coax
```

`coax test` runs 293 checks, including those of the files it loads, in
about 110 seconds on 2026-11-03. Most of them are the loaded
chapters' own checks. Mutation testing: not run (owner speed order).

### Python

[txcheck.py](../../coax/tests/bitcoin/txcheck.py) reads the reject
reasons of `CheckTransaction` and `CheckTxInputs` from the pinned
`consensus/tx_check.cpp` and `consensus/tx_verify.cpp` in their order,
checks the rules' text, and reads `COINBASE_MATURITY`,
`MAX_BLOCK_WEIGHT`, `WITNESS_SCALE_FACTOR`, `MAX_MONEY` and mainnet's CSV
height. It checks that `txcheck.coax`'s codes, reasons and constants are
Core's, then runs transliterations of `CheckTransaction`, `CheckTxInputs`
and `IsFinalTx` with chapter 34's reader,
[tx_reference.py](../../coax/tests/bitcoin/tx_reference.py), and writes
the transcript the example must print.
[make_txcheck_vectors.py](../../coax/tests/bitcoin/make_txcheck_vectors.py)
writes the vectors file from Core's two JSON files.

```shell
python3 -I coax/tests/bitcoin/txcheck.py --coax coax/bin/coax
```

On 2026-11-03 all 35 lines agree with Python, and the example's
output in native debug and release builds equals them. Wasm was not run.

## What this chapter leaves out

- Script checks: chapters 36 to 44. Sigop limits and weight: chapter 53.
- The block rules around transactions (the coinbase first and only once,
  the Merkle root, the witness commitment, the coinbase height): chapters
  46, 49, 54 and 57.
- The subsidy and the coinbase's limit on what it may claim: chapter 54.
- BIP-30's duplicate transaction IDs: chapters 50 and 58.
- Policy rules that only the mempool applies, such as the minimum size,
  standard scripts and dust: chapters 81 to 83.

## Summary

- `CheckTransaction` needs only the transaction: inputs and outputs, a
  size that fits a block, amounts within `MAX_MONEY`, no outpoint twice, a
  coinbase script of 2 to 100 bytes, and no null outpoint outside a
  coinbase.
- The running-total check closes CVE-2010-5139, and the duplicate-input
  check CVE-2018-17144.
- `CheckTxInputs` needs the coins: they exist, a coinbase's coins wait 100
  blocks, values stay in range, and the inputs cover the outputs.
- In a block, a time lock is compared with the block's time before CSV and
  with the median time past after.
- Core marks the cases of `tx_invalid.json` that fail outside script as
  `BADTX`; this chapter refuses exactly those.

## Dependencies

- [34. Transactions and their serialization](ch034-transactions-and-their-serialization.md)
- [35. Txids, wtxids and malleability](ch035-txids-wtxids-and-malleability.md)
- [39. Timelocks](ch039-timelocks.md)
- [40. Weight, vsize and fees](ch040-weight-vsize-and-fees.md)

## Sources

- Bitcoin Core at revision `ddea9196`: `consensus/tx_check.cpp`
  (`CheckTransaction`), `consensus/tx_verify.cpp` (`CheckTxInputs`,
  `IsFinalTx`), `consensus/amount.h`, `consensus/consensus.h`,
  `validation.cpp` (`ContextualCheckBlock`), `test/transaction_tests.cpp`
  and `test/data/tx_valid.json` and `tx_invalid.json`, pinned in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/tx_check.cpp).
- [BIP-113](https://github.com/bitcoin/bips/blob/master/bip-0113.mediawiki)
  (median time past as the lock-time cutoff) and
  [BIP-68](https://github.com/bitcoin/bips/blob/master/bip-0068.mediawiki)
  (relative lock times).
- CVE-2010-5139 and CVE-2018-17144, as Core's comments in `tx_check.cpp`
  name them.
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
