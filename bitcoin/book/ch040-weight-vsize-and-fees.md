# 40. Weight, vsize and fees

A transaction pays a fee: what its inputs spend minus what its outputs
pay. Miners fill a block of limited weight, so they rank transactions by
fee per unit of size. This chapter computes the fee, the feerate in the
units wallets and nodes use, Bitcoin Core's sigop-adjusted virtual size,
and the weight of a spend before it is signed. Chapter 34 already gives
the weight and the vsize of a read transaction; this chapter builds on
them.

## What this chapter teaches

- Weight units, the witness discount and why segwit has it, and the
  virtual size, rounded up.
- The fee, inputs minus outputs, with the coins' values from outside the
  transaction, and Core's range checks on it.
- Feerates in sat/vB, sat/kvB (Core's unit) and sat/kWU, and the fee for
  a target rate.
- Core's sigop-adjusted virtual size, `GetVirtualTransactionSize` with
  `nBytesPerSigOp`.
- Estimating the weight of P2PKH, P2WPKH and key-path P2TR spends before
  signing, with the longest signature each can have.
- The block weight limit of 4,000,000 and Core's standard limit of
  400,000.

## Where Bitcoin uses weight and fees

| Where | What it uses | Chapter |
| --- | --- | --- |
| Block validity | the sum of weights, at most 4,000,000 | 53 |
| Mempool acceptance | the fee, the sigop-adjusted vsize and the minimum relay feerate | 82 |
| Replacement (RBF) and package relay | feerates and absolute fees | 84, 85 |
| Fee estimation and mining | feerates of confirmed and waiting transactions | 87, 88 |
| A wallet | the estimated weight of a transaction it hasn't signed yet | 95 |
| Lightning | feerates in sat/kWU for commitment and HTLC transactions | 108 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [fees.coax](../../coax/lib/bitcoin/fees.coax) | `fee_vartime(b, t, outs, values)` | inputs minus outputs, with Core's range checks |
| | `fee_vsize_sigops_vartime(weight, sigop_cost, bytes_per_sigop)` | Core's `GetVirtualTransactionSize` |
| | `fee_per_kvb_vartime`, `fee_per_kwu_vartime` | a feerate per 1,000 virtual bytes or weight units |
| | `fee_for_rate_vartime(rate, vsize)` | the fee for a rate, rounded up |
| | `fee_input_weight_vartime`, `fee_output_size_vartime`, `fee_estimate_weight_vartime` | the weight of a spend before signing |
| [tx.coax](../../coax/lib/bitcoin/tx.coax) | `tx_weight_vartime`, `tx_vsize_vartime` | BIP-141's weight and vsize (chapter 34) |

Fees and sizes are public, so every procedure runs in variable time and
its name ends in `_vartime` (chapter 3). The limits come from Bitcoin
Core's `consensus/amount.h`, `consensus/consensus.h` and
`policy/policy.h`:

<!-- source: coax/lib/bitcoin/fees.coax limits -->
```jai
FEE_MAX_MONEY :: 2100000000000000;   // amount.h MAX_MONEY: 21,000,000 bitcoin of 100,000,000 satoshis
FEE_MAX_BLOCK_WEIGHT :: 4000000;     // consensus.h MAX_BLOCK_WEIGHT
FEE_MAX_STANDARD_WEIGHT :: 400000;   // policy.h MAX_STANDARD_TX_WEIGHT: the largest transaction Core relays
FEE_BYTES_PER_SIGOP :: 20;           // policy.h DEFAULT_BYTES_PER_SIGOP
```

## Weight and the witness discount

BIP-141 counts a byte outside the witness as 4 weight units and a witness
byte as 1:

```text
weight = 4 × stripped size + (total size − stripped size)
vsize  = ceil(weight / 4) = (weight + 3) / 4
```

The rounding is up, so a transaction never pays for less than it is. The
P2WPKH spend `f5864806` has 83 stripped bytes and 110 witness bytes:
4 × 83 + 110 = 442 weight units, and 442 / 4 = 110.5, so its vsize is
111. If every byte weighed 4, it would weigh 772.

The discount has two reasons:

- **Compatibility.** Old nodes see only the stripped bytes. A limit of
  4,000,000 weight units holds at most 1,000,000 stripped bytes, so a
  segwit block never breaks the old 1 MB limit, and segwit could be a
  soft fork.
- **Cost to the network.** An output becomes an entry in the UTXO set,
  which every full node keeps until the output is spent. A witness is
  checked once and then needed only to serve old blocks. Counting
  witness bytes at a quarter makes spending an output cheaper than
  creating one, which favours consolidating coins over making more.

## The fee

A transaction doesn't state its fee. The fee is the sum of the values of
the coins it spends minus the sum of its outputs, and the spent values
live in the UTXO set (chapter 56), not in the transaction. The caller
passes them. Core's `CheckTxInputs` refuses a value or total outside
0 to 21 million bitcoin (`MoneyRange`) and outputs that pay more than
the inputs (`bad-txns-in-belowout`):

<!-- source: coax/lib/bitcoin/fees.coax fee -->
```jai
// The fee of t, read from b with its outputs in outs: the values of the
// coins its inputs spend, one per input in order, minus the values of its
// outputs. Like Core's CheckTxInputs it refuses (ok is false) when a value
// or a running total is outside 0..MAX_MONEY or the outputs pay more than
// the inputs; it also refuses a count of values that isn't t's input count.
FeeResult :: struct { ok: bool; fee: int; inputs: int; outputs: int; }
fee_vartime :: (b: [] u8, t: Tx, outs: [] TxOut, values: [] int) -> FeeResult #export_c {
  r: FeeResult; bad: FeeResult;
  if t.code != TX_OK || values.count != t.nin { return bad; }
  for i: 0..<values.count {
    if values[i] < 0 || values[i] > FEE_MAX_MONEY { return bad; }
    r.inputs += values[i];
    if r.inputs > FEE_MAX_MONEY { return bad; }
  }
  for k: 0..<t.nout {
    v := tx_le_vartime(b, outs[k].value, 8);     // signed, so a value of 2^63 or more is negative
    if v < 0 || v > FEE_MAX_MONEY { return bad; }
    r.outputs += v;
    if r.outputs > FEE_MAX_MONEY { return bad; }
  }
  if r.outputs > r.inputs { return bad; }
  r.fee = r.inputs - r.outputs; r.ok = true;
  return r;
}
```

## Feerates

Wallets show feerates in sat/vB. Bitcoin Core states them in sat/kvB:
`-minrelaytxfee` defaults to 100 sat/kvB in the pinned `policy.h`, a
tenth of a sat/vB. Lightning's BOLT 3 and LDK use sat/kWU, satoshis per
1,000 weight units. One sat/vB is 1,000 sat/kvB and 250 sat/kWU.

Core doesn't divide by the plain vsize. A transaction can have many
signature operations in few bytes, and a block allows at most 80,000
sigop cost (chapter 53). Core's mempool charges for them: the virtual
size is the larger of the weight and the sigop cost times
`-bytespersigop` (20 by default), divided by 4 and rounded up. A
transaction of weight 442 pays for its sigops only beyond 22: at 23,
23 × 20 = 460 is more than 442. Counting the sigop cost of a transaction
needs the script rules of chapter 53, so here the caller passes it; the
example uses mempool.space's published costs.

<!-- source: coax/lib/bitcoin/fees.coax rates -->
```jai
// Core's GetVirtualTransactionSize: the larger of the weight and the sigop
// cost times bytes_per_sigop, divided by 4 and rounded up. With a sigop
// cost of 0 it is chapter 34's vsize. Returns -1 for a negative input, a
// weight above a block's, a sigop cost above a block's 80,000, or
// bytes_per_sigop above 1,000.
fee_vsize_sigops_vartime :: (weight: int, sigop_cost: int, bytes_per_sigop: int) -> int #export_c {
  if weight < 0 || weight > FEE_MAX_BLOCK_WEIGHT || sigop_cost < 0 || sigop_cost > 80000 || bytes_per_sigop < 0 || bytes_per_sigop > 1000 { return -1; }
  w := weight;
  if sigop_cost * bytes_per_sigop > w { w = sigop_cost * bytes_per_sigop; }
  return (w + TX_SCALE - 1) / TX_SCALE;
}
// A feerate, rounded down: satoshis per 1,000 virtual bytes (sat/kvB,
// Core's unit), or per 1,000 weight units (sat/kWU, LDK's and Lightning's
// unit). 1 sat/vB is 1,000 sat/kvB and 250 sat/kWU. -1 for a fee outside
// 0..MAX_MONEY or a size of 0 or less.
fee_per_kvb_vartime :: (fee: int, vsize: int) -> int #export_c {
  if fee < 0 || fee > FEE_MAX_MONEY || vsize <= 0 { return -1; }
  return fee * 1000 / vsize;
}
fee_per_kwu_vartime :: (fee: int, weight: int) -> int #export_c {
  if fee < 0 || fee > FEE_MAX_MONEY || weight <= 0 { return -1; }
  return fee * 1000 / weight;
}
// The fee that pays at least rate sat/kvB for vsize virtual bytes:
// rate × vsize / 1000, rounded up. The rate is split into its thousands
// and the rest so that no product passes 2^63. -1 for a rate outside
// 0..MAX_MONEY or a size outside 0..MAX_BLOCK_WEIGHT.
fee_for_rate_vartime :: (rate: int, vsize: int) -> int #export_c {
  if rate < 0 || rate > FEE_MAX_MONEY || vsize < 0 || vsize > FEE_MAX_BLOCK_WEIGHT { return -1; }
  return (rate / 1000) * vsize + ((rate % 1000) * vsize + 999) / 1000;
}
```

### Worked example: the fee of f5864806

`f5864806` spends one coin of 511,884 satoshis and pays one output of
506,078. Its fee is 5,806 satoshis, which is what mempool.space
publishes. Its vsize is 111, so it paid

```text
5806 × 1000 / 111 = 52306 sat/kvB, which is 52.306 sat/vB
5806 × 1000 / 442 = 13135 sat/kWU
```

Its sigop cost is 1 (one `CHECKSIG` in the P2WPKH program), and
1 × 20 = 20 is far below 442, so the sigop-adjusted vsize is also 111.

## Estimating before signing

A wallet must choose the fee before it signs, because the signatures
sign the outputs and so the change. It doesn't know the signatures'
lengths yet, so it assumes the longest:

- A DER signature with a low `s` is at most 71 bytes: `r` can need a
  `0x00` before it (33 bytes), `s` can't (32 bytes), plus 6 bytes of DER
  structure. With its sighash byte that is 72.
- A BIP-340 signature is always 64 bytes. With `SIGHASH_DEFAULT` there is
  no sighash byte.

Every input has a 32-byte txid, a 4-byte index and a 4-byte sequence.
So, in weight units:

| Input | Outside the witness | Witness | Weight |
| --- | --- | --- | --- |
| P2PKH | 40 + 1 + 107 (scriptSig: 1 + 72 + 1 + 33) = 148 bytes × 4 | none | 592 |
| P2WPKH | 40 + 1 (empty scriptSig) = 41 bytes × 4 | 1 + 1 + 72 + 1 + 33 = 108 | 272 |
| P2TR key path | 41 bytes × 4 | 1 + 1 + 64 = 66 | 230 |

The rest of the transaction adds 4 × (version, input and output counts,
outputs and locktime), and, with any segwit input, 2 for the marker and
flag and 1 for each P2PKH input's empty witness stack:

<!-- source: coax/lib/bitcoin/fees.coax estimate -->
```jai
// Input kinds the estimator knows, and the most each signature can take.
FEE_P2PKH :: 0;              // a scriptSig of a signature and a compressed key
FEE_P2WPKH :: 1;             // a witness of a signature and a compressed key
FEE_P2TR :: 2;               // a key-path witness of one Schnorr signature
FEE_SIG_MAX :: 72;           // DER with low s is at most 71 bytes, then the sighash byte
FEE_KEY :: 33;               // a compressed public key
FEE_SCHNORR :: 64;           // BIP-340, with SIGHASH_DEFAULT and no sighash byte
FEE_INPUT_FIXED :: 32 + 4 + 4;   // every input's txid, output index and sequence
// The weight one input adds, signed with the longest signature its kind
// allows, or -1 for an unknown kind. A byte outside the witness weighs 4.
fee_input_weight_vartime :: (kind: int) -> int #export_c {
  if kind == FEE_P2PKH {
    script := 1 + FEE_SIG_MAX + 1 + FEE_KEY;                    // two pushes: 107 bytes
    return TX_SCALE * (FEE_INPUT_FIXED + tx_compact_size_vartime(script) + script);
  }
  empty := TX_SCALE * (FEE_INPUT_FIXED + 1);                    // a scriptSig of length 0
  if kind == FEE_P2WPKH { return empty + 1 + 1 + FEE_SIG_MAX + 1 + FEE_KEY; }   // count, then two items
  if kind == FEE_P2TR { return empty + 1 + 1 + FEE_SCHNORR; }   // count, then one item
  return -1;
}
// The serialized size of an output whose scriptPubKey has script_n bytes.
fee_output_size_vartime :: (script_n: int) -> int #export_c {
  if script_n < 0 { return -1; }
  return 8 + tx_compact_size_vartime(script_n) + script_n;
}
// The weight of a transaction that spends inputs of kinds to nout outputs
// of output_bytes in all, before anyone signs it: the version, the counts,
// the inputs, the outputs and the locktime; and with any segwit input the
// marker and flag, and an empty stack for each P2PKH input. -1 for an
// unknown kind or a negative count or size.
fee_estimate_weight_vartime :: (kinds: [] int, nout: int, output_bytes: int) -> int #export_c {
  if nout < 0 || output_bytes < 0 { return -1; }
  w := TX_SCALE * (4 + tx_compact_size_vartime(kinds.count) + tx_compact_size_vartime(nout) + output_bytes + 4);
  legacy := 0;
  for i: 0..<kinds.count {
    k := fee_input_weight_vartime(kinds[i]);
    if k < 0 { return -1; }
    w += k;
    if kinds[i] == FEE_P2PKH { legacy += 1; }
  }
  if legacy < kinds.count { w += 2 + legacy; }
  return w;
}
```

The estimates of `f5864806` and `33e794d0` equal their signed weights,
442 and 676, because their signatures are as long as the estimate
allows: 72 bytes and 64 bytes. A P2WPKH spend whose `r` has no padding
byte weighs one unit less than the estimate. That never changes what a
wallet pays by more than one virtual byte, and it never pays too little.

## The block limit

A block's transactions weigh at most 4,000,000 units (`MAX_BLOCK_WEIGHT`).
That fits 9,049 transactions the size of `f5864806`, against 5,181 if
every byte weighed 4. Core relays no transaction above 400,000 weight
units (`MAX_STANDARD_TX_WEIGHT`), a tenth of a block. Chapter 53 refuses
a block over the limit.

## The example

The example is [bitcoin/examples/fees.coax](../examples/fees.coax). It
prints the weight and vsize of chapter 34's six transactions (0–2 are
the legacy `23b397ed`, `c99c49da` and `f7fdd091`; 3–5 the segwit
`f5864806`, `33e794d0` and `37777def`), the witness discount on
`f5864806`, the fee, feerates and sigop-adjusted vsize of the three
segwit ones, the fee's refusals, the pre-signing estimates, and the
block limit.

<!-- source: bitcoin/examples/fees.coax example -->
```jai
#load "../../coax/lib/bitcoin/fees.coax";
#load "../../coax/lib/nostr/hex.coax";
#load "../../coax/tests/bitcoin/spent_vectors.coax";
#import "Basic";

decode :: (text: string, out: [] u8) -> [] u8 {
  r := hex_decode_checked(text, out, false);
  #assert r.error == 0;
  return out[0..<r.written];
}
// A number of thousandths, printed with three decimals: 52306 is 52.306.
milli :: (v: int) {
  f := v % 1000;
  print("%.", v / 1000);
  if f < 100 { print("0"); }
  if f < 10 { print("0"); }
  print("%", f);
}

main :: () {
  b: [1024] u8; ins: [4] TxIn; outs: [4] TxOut; items: [8] TxItem;

  // ---- weight and virtual size of chapter 34's six transactions
  print("weight = 4 x stripped + witness bytes; vsize = weight / 4, rounded up\n");
  for i: 0..<6 {
    m := TX_MAINNET[i];
    s := decode(m.tx, b[..]);
    t := tx_read_vartime(s, true, ins[..], outs[..], items[..]);
    w := tx_weight_vartime(t);
    print("%: % bytes, stripped %, witness %, weight % (published %), vsize %\n",
      i, t.total, t.stripped, t.total - t.stripped, w, m.weight, tx_vsize_vartime(t));
  }
  print("the witness discount on f5864806\n");
  s := decode(TX_MAINNET[3].tx, b[..]);
  t := tx_read_vartime(s, true, ins[..], outs[..], items[..]);
  print("  % stripped bytes x 4 = %, plus % witness bytes x 1 = %: weight %\n",
    t.stripped, 4 * t.stripped, t.total - t.stripped, t.total - t.stripped, tx_weight_vartime(t));
  print("  at 4 units for every byte it would weigh % (vsize %)\n", 4 * t.total, t.total);

  // ---- fees: the inputs' values minus the outputs'
  print("fees\n");
  for i: 0..<3 {
    m := TX_MAINNET[3+i]; spent := TX_SPENT[i];
    s = decode(m.tx, b[..]);
    t = tx_read_vartime(s, true, ins[..], outs[..], items[..]);
    f := fee_vartime(s, t, outs[..], spent.values[0..<spent.nin]);
    w := tx_weight_vartime(t); v := tx_vsize_vartime(t);
    print("%: in %, out %, fee % (published %)\n", 3 + i, f.inputs, f.outputs, f.fee, spent.fee);
    print("  vsize %: ", v); milli(fee_per_kvb_vartime(f.fee, v));
    print(" sat/vB, % sat/kvB, % sat/kWU\n", fee_per_kvb_vartime(f.fee, v), fee_per_kwu_vartime(f.fee, w));
    print("  sigop cost % (published): vsize with sigops %; it would grow at % sigops (% x 20 > %)\n",
      spent.sigops, fee_vsize_sigops_vartime(w, spent.sigops, FEE_BYTES_PER_SIGOP), w / 20 + 1, w / 20 + 1, w);
  }
  print("sigops beyond weight / 20 set the vsize\n");
  for c: [4] int.[1, 22, 23, 100] {
    print("  weight 442, sigop cost %: vsize %\n", c, fee_vsize_sigops_vartime(442, c, FEE_BYTES_PER_SIGOP));
  }
  print("refusals\n");
  s = decode(TX_MAINNET[3].tx, b[..]);
  t = tx_read_vartime(s, true, ins[..], outs[..], items[..]);
  short := [1] int.[506077]; two := [2] int.[511884, 0]; over := [1] int.[FEE_MAX_MONEY + 1];
  print("  spent value 506077, one less than the output: ok %\n", fee_vartime(s, t, outs[..], short[..]).ok);
  print("  two values for one input: ok %\n", fee_vartime(s, t, outs[..], two[..]).ok);
  print("  a value above MAX_MONEY: ok %\n", fee_vartime(s, t, outs[..], over[..]).ok);

  // ---- estimating before signing
  print("estimates before signing, with the longest signatures\n");
  print("  input weights: P2PKH %, P2WPKH %, P2TR key path %\n",
    fee_input_weight_vartime(FEE_P2PKH), fee_input_weight_vartime(FEE_P2WPKH), fee_input_weight_vartime(FEE_P2TR));
  p2wpkh := [1] int.[FEE_P2WPKH]; p2tr := [1] int.[FEE_P2TR];
  print("  f5864806, one P2WPKH input to one P2SH output: estimate %, signed %\n",
    fee_estimate_weight_vartime(p2wpkh[..], 1, fee_output_size_vartime(23)), TX_MAINNET[3].weight);
  print("  33e794d0, one P2TR input to OP_RETURN and P2TR outputs: estimate %, signed %\n",
    fee_estimate_weight_vartime(p2tr[..], 2, fee_output_size_vartime(49) + fee_output_size_vartime(34)), TX_MAINNET[4].weight);
  pair := [2] int.[FEE_P2WPKH, FEE_P2WPKH];
  w := fee_estimate_weight_vartime(pair[..], 2, 2 * fee_output_size_vartime(22));
  v := (w + 3) / 4;
  print("  two P2WPKH inputs to two P2WPKH outputs: weight %, vsize %\n", w, v);
  for rate: [3] int.[1000, 10000, 52306] {
    print("    at "); milli(rate); print(" sat/vB the fee is %\n", fee_for_rate_vartime(rate, v));
  }

  // ---- the block limit
  print("a block holds % weight units: % stripped bytes at most\n", FEE_MAX_BLOCK_WEIGHT, FEE_MAX_BLOCK_WEIGHT / 4);
  print("  % transactions the size of f5864806, or % at 4 units a byte\n", FEE_MAX_BLOCK_WEIGHT / 442, FEE_MAX_BLOCK_WEIGHT / (4 * 193));
  print("  Core relays transactions up to % weight units (% vB)\n", FEE_MAX_STANDARD_WEIGHT, FEE_MAX_STANDARD_WEIGHT / 4);
}
```

Run it from the repository root:

```shell
coax/bin/coax run bitcoin/examples/fees.coax
```

The output:

```text
weight = 4 x stripped + witness bytes; vsize = weight / 4, rounded up
0: 158 bytes, stripped 158, witness 0, weight 632 (published 632), vsize 158
1: 225 bytes, stripped 225, witness 0, weight 900 (published 900), vsize 225
2: 402 bytes, stripped 402, witness 0, weight 1608 (published 1608), vsize 402
3: 193 bytes, stripped 83, witness 110, weight 442 (published 442), vsize 111
4: 220 bytes, stripped 152, witness 68, weight 676 (published 676), vsize 169
5: 358 bytes, stripped 123, witness 235, weight 727 (published 727), vsize 182
the witness discount on f5864806
  83 stripped bytes x 4 = 332, plus 110 witness bytes x 1 = 110: weight 442
  at 4 units for every byte it would weigh 772 (vsize 193)
fees
3: in 511884, out 506078, fee 5806 (published 5806)
  vsize 111: 52.306 sat/vB, 52306 sat/kvB, 13135 sat/kWU
  sigop cost 1 (published): vsize with sigops 111; it would grow at 23 sigops (23 x 20 > 442)
4: in 88480, out 67230, fee 21250 (published 21250)
  vsize 169: 125.739 sat/vB, 125739 sat/kvB, 31434 sat/kWU
  sigop cost 0 (published): vsize with sigops 169; it would grow at 34 sigops (34 x 20 > 676)
5: in 983500, out 965300, fee 18200 (published 18200)
  vsize 182: 100.000 sat/vB, 100000 sat/kvB, 25034 sat/kWU
  sigop cost 0 (published): vsize with sigops 182; it would grow at 37 sigops (37 x 20 > 727)
sigops beyond weight / 20 set the vsize
  weight 442, sigop cost 1: vsize 111
  weight 442, sigop cost 22: vsize 111
  weight 442, sigop cost 23: vsize 115
  weight 442, sigop cost 100: vsize 500
refusals
  spent value 506077, one less than the output: ok false
  two values for one input: ok false
  a value above MAX_MONEY: ok false
estimates before signing, with the longest signatures
  input weights: P2PKH 592, P2WPKH 272, P2TR key path 230
  f5864806, one P2WPKH input to one P2SH output: estimate 442, signed 442
  33e794d0, one P2TR input to OP_RETURN and P2TR outputs: estimate 676, signed 676
  two P2WPKH inputs to two P2WPKH outputs: weight 834, vsize 209
    at 1.000 sat/vB the fee is 209
    at 10.000 sat/vB the fee is 2090
    at 52.306 sat/vB the fee is 10932
a block holds 4000000 weight units: 1000000 stripped bytes at most
  9049 transactions the size of f5864806, or 5181 at 4 units a byte
  Core relays transactions up to 400000 weight units (100000 vB)
```

What the output shows:

- Each weight equals its published value, and each legacy
  transaction's weight is four times its size.
- The witness discount takes `f5864806` from 772 to 442 weight units.
- Each fee equals mempool.space's. `37777def` paid exactly 100 sat/vB.
- None of the three pays for sigops: their costs (1, 0 and 0) are far
  below weight / 20.
- A spent value one satoshi short, a wrong count of values and a value
  above `MAX_MONEY` refuse.
- The estimates of the P2WPKH and P2TR spends equal their signed
  weights, and a two-in, two-out P2WPKH spend estimates at 209 vB, so
  2,090 satoshis at 10 sat/vB.

The book's checks run this program in debug and release builds and
compare the output with [bitcoin/fixtures/fees.out](../fixtures/fees.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/fees.coax](../examples/checks/fees.coax), which
loads the example:

- the segwit transactions' published fees and weights, the vsize with no
  sigops and with the published sigop cost, and a feerate that rounds
  down so that paying it never exceeds the fee;
- the legacy transactions' weight of four units a byte;
- the estimates against the signed weights.

<!-- source: bitcoin/examples/checks/fees.coax check -->
```jai
// The three segwit mainnet transactions: the fee is the published one, the
// published sigop cost leaves the vsize alone, and Core's
// GetVirtualTransactionSize with no sigops is chapter 34's vsize.
#forall "fees and vsizes of the segwit mainnet transactions" i: 0..<3 #steps 100000000 {
  b: [1024] u8; ins: [4] TxIn; outs: [4] TxOut; items: [8] TxItem;
  m := TX_MAINNET[3+i]; spent := TX_SPENT[i];
  s := decode(m.tx, b[..]);
  t := tx_read_vartime(s, true, ins[..], outs[..], items[..]);
  f := fee_vartime(s, t, outs[..], spent.values[0..<spent.nin]);
  #assert f.ok && f.fee == spent.fee && tx_weight_vartime(t) == m.weight;
  #assert fee_vsize_sigops_vartime(m.weight, 0, FEE_BYTES_PER_SIGOP) == tx_vsize_vartime(t);
  #assert fee_vsize_sigops_vartime(m.weight, spent.sigops, FEE_BYTES_PER_SIGOP) == tx_vsize_vartime(t);
  // The rate rounds down, so paying it for the vsize never exceeds the fee,
  // and one sat/kvB more would.
  rate := fee_per_kvb_vartime(f.fee, tx_vsize_vartime(t));
  #assert fee_for_rate_vartime(rate, tx_vsize_vartime(t)) <= f.fee && fee_for_rate_vartime(rate + 1, tx_vsize_vartime(t)) > f.fee;
}
// The legacy ones weigh 4 units a byte, and their vsize is their size.
#forall "legacy transactions: weight 4 x size" i: 0..<3 #steps 100000000 {
  b: [1024] u8; ins: [4] TxIn; outs: [4] TxOut; items: [8] TxItem;
  s := decode(TX_MAINNET[i].tx, b[..]);
  t := tx_read_vartime(s, true, ins[..], outs[..], items[..]);
  #assert tx_weight_vartime(t) == 4 * s.count && tx_vsize_vartime(t) == s.count;
}
// The estimates before signing equal the signed weights of the P2WPKH and
// key-path P2TR spends, whose signatures are the longest their kind allows.
#test "estimates match the signed weights" {
  p2wpkh := [1] int.[FEE_P2WPKH]; p2tr := [1] int.[FEE_P2TR];
  #assert fee_estimate_weight_vartime(p2wpkh[..], 1, fee_output_size_vartime(23)) == TX_MAINNET[3].weight;
  #assert fee_estimate_weight_vartime(p2tr[..], 2, fee_output_size_vartime(49) + fee_output_size_vartime(34)) == TX_MAINNET[4].weight;
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/fees.coax
```

`coax test` runs 76 checks, including the loaded files' own checks, in
about 2 seconds on 2026-11-03.

### The library's own checks

`fees.coax` is an `#authority` file. Its checks cover the fee of
`f5864806` and `37777def` and every refusal (a short spent value, the
wrong count, a negative value, a value or total above `MAX_MONEY`, an
output with its sign bit set or above `MAX_MONEY`, an unread
transaction); the sigop-adjusted vsize at the edges where sigops start to
count and at a block's 80,000; feerates and the fee for a rate at their
rounding edges and at the largest inputs; and the estimates, including a
legacy spend, a mixed one and 253 inputs, whose count takes a 3-byte
CompactSize.

```shell
coax/bin/coax test coax/lib/bitcoin/fees.coax
```

`coax test` runs 70 checks, including those of the files it loads, in
about 2 seconds on 2026-11-03. Mutation testing: not run (owner speed
order).

### Python

[fees.py](../../coax/tests/bitcoin/fees.py) computes every number the
example prints in Python that shares no code with Coax. It reads the
pinned transactions with
[tx_reference.py](../../coax/tests/bitcoin/tx_reference.py), the spent
values, fees and sigop costs from the pinned mempool.space JSON, the
limits from the pinned Core headers, and checks that the pinned
`policy.cpp` computes the sigop-adjusted weight as
`std::max(weight, sigop_cost * bytes_per_sigop)`. The transcript must
equal the fixture.

```shell
python3 -I coax/tests/bitcoin/fees.py --coax coax/bin/coax
```

On 2026-11-03 all 40 lines agree with Python, and the example's output
in native debug and release builds equals them. Wasm was not run.

## What this chapter leaves out

- Counting sigops: legacy, P2SH and witness costs, chapter 53.
- Bitcoin Core's `decoderawtransaction`: the weights here match
  mempool.space's and rust-bitcoin's published values instead.
- The mempool's minimum relay fee, replacement rules and package
  feerates: chapters 82, 84 and 85.
- Fee estimation from recent blocks: chapter 87.
- Choosing coins and change: chapter 95.

## Summary

- Weight is 4 × stripped size + witness size; vsize is weight / 4,
  rounded up.
- The fee is the spent coins' values minus the outputs', and the spent
  values come from outside the transaction.
- Core divides the fee by a vsize that charges 20 bytes for each unit of
  sigop cost when that is larger than the weight.
- Before signing, assume a 72-byte ECDSA signature with its sighash byte
  and a 64-byte Schnorr signature: a P2WPKH input weighs 272 and a
  key-path P2TR input 230.
- A block holds 4,000,000 weight units; Core relays transactions of up
  to 400,000.

## Dependencies

- [34. Transactions and their serialization](ch034-transactions-and-their-serialization.md)

## Sources

- [BIP-141](https://github.com/bitcoin/bips/blob/master/bip-0141.mediawiki)
  (weight, the block weight limit and the reasons for the discount).
- Bitcoin Core at revision `ddea9196`: `policy/policy.cpp`
  (`GetVirtualTransactionSize`), `policy/policy.h`,
  `consensus/consensus.h` and `consensus/amount.h`, pinned in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/policy.cpp).
- Three mainnet transactions, their spent coins, fees and sigop costs
  from mempool.space, pinned in
  [coax/tests/vectors/upstream/mainnet/](../../coax/tests/vectors/upstream/mainnet/33e794d097969002ee05d336686fc03c9e15a597c1b9827669460fac98799036.json)
  and copied into
  [spent_vectors.coax](../../coax/tests/bitcoin/spent_vectors.coax).
- [BOLT 3](https://github.com/lightning/bolts/blob/master/03-transactions.md)
  for feerates in sat/kWU.
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
