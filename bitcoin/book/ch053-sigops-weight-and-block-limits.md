# 53. Sigops, weight and block limits

A block is a bounded amount of work. Its bytes are limited by weight, and
the signature checks its scripts may ask for are limited by a count of
signature operations, or sigops. Both limits protect every node that
validates the block: a block that asked for millions of signature checks
would take a node minutes to verify, even if each check passed.

This chapter counts sigops as Bitcoin Core counts them, in three ways that
grew up with the protocol, and builds the checks Core runs on a whole
block before it looks at any coin: `CheckBlock`, in Core's order, and the
weight rule of `ContextualCheckBlock`. Mainnet block 514,999 passes, its
sigop cost of every transaction matches the cost mempool.space publishes,
and a crafted block for each rule refuses with Core's reason.

## What this chapter teaches

- What a signature operation is, and why `OP_CHECKMULTISIG` counts 20
  in the legacy count.
- The three counts: legacy, P2SH (BIP-16) and witness (BIP-141), and how
  they combine into a sigop cost of at most 80,000 per block.
- Block weight, at most 4,000,000 weight units, and the older 1,000,000
  bytes without witnesses.
- `CheckBlock`'s order: the Merkle root, the size, one coinbase first,
  every transaction's own checks, the legacy sigop count.
- Why a block's checks run in that order.

## Where Bitcoin uses these limits

| Limit | Who checks it | Chapter |
| --- | --- | --- |
| Merkle root and the duplicate-leaf flaw | `CheckBlock` (`CheckMerkleRoot`) | 46 |
| Size without witnesses, transaction count | `CheckBlock` | 53 |
| One coinbase, first | `CheckBlock` | 53, 54 |
| Each transaction's own rules | `CheckBlock` (`CheckTransaction`) | 52 |
| Legacy sigops times 4, at most 80,000 | `CheckBlock` | 53 |
| Weight, at most 4,000,000 | `ContextualCheckBlock`, after the witness commitment | 49, 53 |
| Full sigop cost, at most 80,000 | `ConnectBlock`, with the spent outputs | 53, 58 |
| Tapscript's signature budget | the interpreter, per input | 44 |
| A miner's template within both limits | `getblocktemplate` | 88 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [sigops.coax](../../coax/lib/bitcoin/sigops.coax) | `sigop_count_vartime` | Core's `CScript::GetSigOpCount(fAccurate)` |
| | `sigop_p2sh_vartime` | `CScript::GetSigOpCount(scriptSig)`: the redeem script's count |
| | `sigop_witness_vartime` | `CountWitnessSigOps` |
| | `sigop_legacy_vartime`, `sigop_cost_vartime` | `GetLegacySigOpCount`, `GetTransactionSigOpCost` |
| | `block_check_vartime` | `CheckBlock` after the header checks |
| | `block_weight_check_vartime` | `ContextualCheckBlock`'s weight rule |
| | `block_sigops_vartime` | `ConnectBlock`'s sigop limit |
| [script.coax](../../coax/lib/bitcoin/script.coax) | `script_op_vartime`, `script_p2sh_vartime`, `script_witness_version_vartime` | chapter 36's opcode reader and output shapes |
| [merkle.coax](../../coax/lib/bitcoin/merkle.coax) | `merkle_block_ids_vartime`, `merkle_check_vartime` | chapter 46's block reader and `CheckMerkleRoot` |
| [txcheck.coax](../../coax/lib/bitcoin/txcheck.coax) | `txcheck_vartime` | chapter 52's `CheckTransaction` |

## Result codes

Each code is one of Core's reject reasons. A transaction that fails its
own checks inside a block refuses the block with chapter 52's reason:

<!-- source: coax/lib/bitcoin/sigops.coax codes -->
```jai
// Result codes of the block checks, with Core's reject reason. A
// transaction that fails CheckTransaction is BLOCK_TX, with chapter 52's
// code in BlockCheck.tx_code.
BLOCK_OK :: 0;
BLOCK_BAD :: 1;            // the bytes do not read as a block
BLOCK_MERKLE :: 2;         // "bad-txnmrklroot"
BLOCK_DUPLICATE :: 3;      // "bad-txns-duplicate": a mutated Merkle tree (CVE-2012-2459)
BLOCK_LENGTH :: 4;         // "bad-blk-length"
BLOCK_CB_MISSING :: 5;     // "bad-cb-missing"
BLOCK_CB_MULTIPLE :: 6;    // "bad-cb-multiple"
BLOCK_TX :: 7;             // CheckTransaction's reason
BLOCK_SIGOPS :: 8;         // "bad-blk-sigops"
BLOCK_WEIGHT :: 9;         // "bad-blk-weight"
BLOCK_CAPACITY :: 10;      // a caller's array is too small, or the spent outputs do not match
BLOCK_REASONS :: [11] string.["ok", "bad block", "bad-txnmrklroot", "bad-txns-duplicate", "bad-blk-length",
  "bad-cb-missing", "bad-cb-multiple", "bad transaction", "bad-blk-sigops", "bad-blk-weight", "capacity"];
```

The limit is Core's `MAX_BLOCK_SIGOPS_COST`; the others are constants that
earlier chapters defined:

<!-- source: coax/lib/bitcoin/sigops.coax limits -->
```jai
BLOCK_MAX_SIGOPS_COST :: 80000;   // consensus.h MAX_BLOCK_SIGOPS_COST
// MAX_BLOCK_WEIGHT is chapter 40's FEE_MAX_BLOCK_WEIGHT, 4,000,000;
// WITNESS_SCALE_FACTOR chapter 34's TX_SCALE, 4; MAX_PUBKEYS_PER_MULTISIG
// chapter 36's SCRIPT_MAX_KEYS, 20.
```

## Counting signature operations

A signature operation is an opcode that checks a signature:
`OP_CHECKSIG` and `OP_CHECKSIGVERIFY` check one, and `OP_CHECKMULTISIG`
and `OP_CHECKMULTISIGVERIFY` check up to the number of keys they are
given. The count is not the number of checks a script makes when it runs.
It is read from the script's bytes without running them, so that a node
can bound a block's work before it does that work.

<!-- source: coax/lib/bitcoin/sigops.coax count -->
```jai
// CScript::GetSigOpCount(fAccurate): OP_CHECKSIG and OP_CHECKSIGVERIFY
// count 1. OP_CHECKMULTISIG and OP_CHECKMULTISIGVERIFY count 20, or, when
// accurate and the opcode before is OP_1 to OP_16, that number. Counting
// stops at a push that runs past the end.
sigop_count_vartime :: (s: [] u8, accurate: bool) -> int #export_c {
  n := 0; last := 0xff; pc := 0;
  while pc < s.count {
    op := script_op_vartime(s, pc);
    if !op.ok { break; }
    if op.op == OP_CHECKSIG || op.op == OP_CHECKSIGVERIFY { n += 1; }
    else if op.op == OP_CHECKMULTISIG || op.op == OP_CHECKMULTISIGVERIFY {
      n += ifx accurate && last >= OP_1 && last <= OP_16 then last - 0x50 else SCRIPT_MAX_KEYS;
    }
    last = op.op; pc = op.next;
  }
  return n;
}
```

The count has two forms. The legacy form, which Satoshi's client used from
2010, counts every multisig as 20, the most keys it may take
(`MAX_PUBKEYS_PER_MULTISIG`). The accurate form reads the opcode before
it: `OP_2 OP_CHECKMULTISIG` counts 2. Both forms only see opcodes. Pushed
bytes are data, so a script that pushes `ac` (the byte of `OP_CHECKSIG`)
counts 0, and counting stops at a push that runs past the end of the
script.

```text
Counting signature operations (GetSigOpCount):
  OP_CHECKSIG: 1
  1 <key> <key> 2 OP_CHECKMULTISIGVERIFY: legacy 20, accurate 2
  1 <20 bytes> <20 bytes> 2 OP_CHECKMULTISIG OP_IF OP_CHECKSIG OP_ENDIF: legacy 21, accurate 3
  that script under P2SH, spent with OP_0 <script>: 3
```

Those are the counts of Core's `sigopcount_tests`. The key in
`1 <key> <key> 2 OP_CHECKMULTISIGVERIFY` is fixed here, where Core's test
makes a random one: counting never reads a key.

### P2SH

A P2SH output (BIP-16) commits to a redeem script by its hash, so the
script's sigops are not in the output. The spend reveals it as the last
push of the scriptSig. Core counts that script accurately, because by
2012 the inaccurate count had no reason to stay:

<!-- source: coax/lib/bitcoin/sigops.coax p2sh -->
```jai
// CScript::GetSigOpCount(scriptSig) on a scriptPubKey: for a P2SH output,
// the accurate count of the redeem script, the last push of a push-only
// scriptSig (0 when the scriptSig is not push-only or ends in OP_0 to
// OP_16, which push no bytes here); for any other output its own accurate
// count.
sigop_p2sh_vartime :: (spk: [] u8, script_sig: [] u8) -> int #export_c {
  if !script_p2sh_vartime(spk) { return sigop_count_vartime(spk, true); }
  pc := 0; at := 0; n := 0;
  while pc < script_sig.count {
    op := script_op_vartime(script_sig, pc);
    if !op.ok || op.op > OP_16 { return 0; }
    at = op.at; n = op.n; pc = op.next;
  }
  return sigop_count_vartime(script_sig[at..<at+n], true);
}
```

### Witness programs

A segwit v0 output (BIP-141) also hides its script. A P2WPKH output always
counts 1. A P2WSH output counts its witness script, the last item of the
witness, accurately. Either can be nested in P2SH, where the redeem script
is the witness program. Every other witness version counts 0: Taproot's
script path (chapter 44) gives each input a signature budget of 50 plus
its witness size instead, so tapscript never adds to this count.

<!-- source: coax/lib/bitcoin/sigops.coax witness -->
```jai
// WitnessSigOps: a v0 program of 20 bytes (P2WPKH) is 1; of 32 bytes
// (P2WSH) the accurate count of the witness script, the last witness item.
// Every other version counts 0: Taproot's are budgeted per input.
sigop_program_vartime :: (version: int, program_n: int, script: [] u8, has_script: bool) -> int #export_c {
  if version != 0 { return 0; }
  if program_n == 20 { return 1; }
  if program_n == 32 && has_script { return sigop_count_vartime(script, true); }
  return 0;
}
// CountWitnessSigOps for input k of t, read from b: nothing without the
// WITNESS flag; a witness program, or a P2SH output whose push-only
// scriptSig's last push is one, counts by its program.
sigop_witness_vartime :: (b: [] u8, ins: [] TxIn, items: [] TxItem, k: int, spk: [] u8, flags: int) -> int #export_c {
  if (flags & SCRIPT_VERIFY_WITNESS) == 0 { return 0; }
  in := ins[k];
  script := b[0..<0];
  if in.count > 0 { last := items[in.first + in.count - 1]; script = b[last.at..<last.at+last.n]; }
  v := script_witness_version_vartime(spk);
  if v >= 0 { return sigop_program_vartime(v, spk.count - 2, script, in.count > 0); }
  sig := b[in.script..<in.script+in.script_n];
  if !script_p2sh_vartime(spk) || !script_push_only_vartime(sig) { return 0; }
  pc := 0; at := 0; n := 0;
  while pc < sig.count { op := script_op_vartime(sig, pc); at = op.at; n = op.n; pc = op.next; }
  redeem := sig[at..<at+n];
  v = script_witness_version_vartime(redeem);
  if v >= 0 { return sigop_program_vartime(v, redeem.count - 2, script, in.count > 0); }
  return 0;
}
```

### A transaction's sigop cost

Segwit made the limit a cost so that it scales with weight. Legacy and
P2SH sigops cost 4 each, as a byte outside the witness weighs 4; witness
sigops cost 1, as a witness byte weighs 1. The 20,000 sigops a block could
hold before segwit became 80,000 units of cost:

<!-- source: coax/lib/bitcoin/sigops.coax cost -->
```jai
// GetLegacySigOpCount: the inaccurate count of every scriptSig and every
// scriptPubKey of t.
sigop_legacy_vartime :: (b: [] u8, t: Tx, ins: [] TxIn, outs: [] TxOut) -> int #export_c {
  n := 0;
  for k: 0..<t.nin { n += sigop_count_vartime(b[ins[k].script..<ins[k].script+ins[k].script_n], false); }
  for k: 0..<t.nout { n += sigop_count_vartime(b[outs[k].script..<outs[k].script+outs[k].script_n], false); }
  return n;
}
// The output an input spends: its value and its scriptPubKey at
// prev[script..<script+script_n].
SigopSpent :: struct { value: int; script: int; script_n: int; }
// GetTransactionSigOpCost: the legacy count times 4; for a transaction
// that is not a coinbase, plus the P2SH count times 4 under the P2SH flag
// and the witness count under the WITNESS flag. spent holds one output per
// input; -1 when it does not.
sigop_cost_vartime :: (b: [] u8, t: Tx, ins: [] TxIn, outs: [] TxOut, items: [] TxItem, prev: [] u8, spent: [] SigopSpent, flags: int) -> int #export_c {
  cost := sigop_legacy_vartime(b, t, ins, outs) * TX_SCALE;
  if tx_coinbase_vartime(b, t, ins) { return cost; }
  if spent.count != t.nin { return -1; }
  for k: 0..<t.nin {
    spk := prev[spent[k].script..<spent[k].script+spent[k].script_n];
    if (flags & SCRIPT_VERIFY_P2SH) != 0 && script_p2sh_vartime(spk) {
      cost += sigop_p2sh_vartime(spk, b[ins[k].script..<ins[k].script+ins[k].script_n]) * TX_SCALE;
    }
    cost += sigop_witness_vartime(b, ins, items, k, spk, flags);
  }
  return cost;
}
```

A coinbase spends nothing, so only its legacy count is charged. The P2SH
count needs the P2SH flag and the witness count the WITNESS flag; chapter
55 gives the flags of each block. Core's `GetTxSigOpCost` cases, under
both flags:

```text
Core's GetTxSigOpCost cases, under P2SH and WITNESS:
  spending a bare 1-of-2 multisig: 0
  the multisig in P2SH: 8 (without the P2SH flag: 0)
  P2WPKH: 1 (without WITNESS: 0; as version 1: 0; as a coinbase: 0)
  P2WPKH in P2SH: 1
  P2WSH of the multisig: 2 (without WITNESS: 0)
  P2WSH in P2SH: 2
```

The first line is a spend of a bare multisig output. The spending
transaction contains no signature operation, so it costs 0; the
transaction that created the output paid 20 × 4 = 80 for it.

## CheckBlock

`block_check_vartime` runs Core's `CheckBlock` after the header checks
(chapters 47 and 50):

<!-- source: coax/lib/bitcoin/sigops.coax check -->
```jai
// What CheckBlock found: the code, the transaction it stopped at (-1 for
// the block), chapter 52's code for BLOCK_TX, and the block's size
// without witnesses, its weight and its legacy sigop count.
BlockCheck :: struct { code: int; tx: int; tx_code: int; count: int; stripped: int; weight: int; legacy: int; }
// CheckBlock after the header's own checks (chapters 47 and 50), in
// Core's order. txids needs 32 bytes per transaction and work as many.
block_check_vartime :: (blk: [] u8, ins: [] TxIn, outs: [] TxOut, items: [] TxItem, txids: [] u8, wtxids: [] u8, work: [] u8) -> BlockCheck #export_c {
  r := BlockCheck.{tx=-1};
  ids := merkle_block_ids_vartime(blk, false, ins, outs, items, txids, wtxids);
  if ids.code != MERKLE_OK { r.code = ifx ids.code == MERKLE_CAPACITY then BLOCK_CAPACITY else BLOCK_BAD; return r; }
  r.count = ids.count;
  m := merkle_check_vartime(txids[0..<32*r.count], work, blk[36..<68]);
  if m == MERKLE_CAPACITY { r.code = BLOCK_CAPACITY; return r; }
  if m == MERKLE_MISMATCH { r.code = BLOCK_MERKLE; return r; }
  if m == MERKLE_MUTATED { r.code = BLOCK_DUPLICATE; return r; }
  // The size without witnesses, and the weight: the full size plus three
  // times the stripped size.
  r.stripped = MERKLE_HEADER + tx_compact_size_vartime(r.count);
  at := tx_compact_read_vartime(blk, MERKLE_HEADER).next;
  for i: 0..<r.count {
    t := tx_parse_vartime(blk[at..<blk.count], true, ins, outs, items);
    r.stripped += t.stripped; at += t.total;
  }
  r.weight = r.stripped * (TX_SCALE - 1) + blk.count;
  if r.count == 0 || r.count * TX_SCALE > FEE_MAX_BLOCK_WEIGHT || r.stripped * TX_SCALE > FEE_MAX_BLOCK_WEIGHT { r.code = BLOCK_LENGTH; return r; }
  at = tx_compact_read_vartime(blk, MERKLE_HEADER).next;
  for i: 0..<r.count {
    s := blk[at..<blk.count];
    t := tx_parse_vartime(s, true, ins, outs, items);
    if tx_coinbase_vartime(s, t, ins) != (i == 0) { r.code = ifx i == 0 then BLOCK_CB_MISSING else BLOCK_CB_MULTIPLE; r.tx = i; return r; }
    at += t.total;
  }
  at = tx_compact_read_vartime(blk, MERKLE_HEADER).next;
  for i: 0..<r.count {
    s := blk[at..<blk.count];
    t := tx_parse_vartime(s, true, ins, outs, items);
    c := txcheck_vartime(s[0..<t.total], t, ins, outs);
    if c != TXC_OK { r.code = BLOCK_TX; r.tx = i; r.tx_code = c; return r; }
    r.legacy += sigop_legacy_vartime(s, t, ins, outs);
    at += t.total;
  }
  if r.legacy * TX_SCALE > BLOCK_MAX_SIGOPS_COST { r.code = BLOCK_SIGOPS; return r; }
  return r;
}
// ContextualCheckBlock's last rule, once the witness commitment is
// checked (chapter 49): the weight is at most 4,000,000.
block_weight_check_vartime :: (c: BlockCheck) -> int #export_c {
  if c.code != BLOCK_OK { return c.code; }
  return ifx c.weight > FEE_MAX_BLOCK_WEIGHT then BLOCK_WEIGHT else BLOCK_OK;
}
```

1. **The Merkle root.** The header commits to the transactions, so the
   root comes first: until it matches, the transactions may not be the
   ones the header names, and a refusal must not mark the header invalid.
   A tree that paired two equal hashes refuses as `bad-txns-duplicate`
   even when the root matches (chapter 46, CVE-2012-2459). This is the
   block-level duplicate-transaction check; a transaction repeated where
   it does not pair with itself changes the root, and its second copy
   fails later, when it spends coins already spent.
2. **The size.** At least one transaction, and at most 1,000,000 bytes
   without witnesses. That was the block size limit before segwit, and it
   is still a rule: weight counts a byte outside the witness as 4, so 1 MB
   of such bytes is already 4,000,000 weight units.
3. **One coinbase, first.** The first transaction must be a coinbase and
   no other may be. Chapter 54 checks what it may claim.
4. **Every transaction's own rules**: chapter 52's `CheckTransaction`,
   including duplicate inputs (CVE-2018-17144).
5. **The legacy sigop count, times 4, at most 80,000.** This count
   undercounts: without the spent outputs, P2SH and witness sigops are not
   known yet. It is cheap, and it refuses a block whose legacy count alone
   is too high before any coin is looked up.

The weight rule is not in `CheckBlock`. A block's weight includes its
witnesses, and the witnesses are not committed in the header; chapter
49's witness commitment ties them to the coinbase. Core checks the weight
in `ContextualCheckBlock` after the commitment, so that a peer who pads a
valid block's witness cannot get the block marked invalid for good.
`block_weight_check_vartime` is that last step.

## ConnectBlock's sigop cost

When a block is connected, the outputs it spends are known, and the full
cost is counted transaction by transaction:

<!-- source: coax/lib/bitcoin/sigops.coax connect -->
```jai
// ConnectBlock's sigop limit: the sigop cost of each transaction in turn,
// refused as soon as the total passes 80,000. spent holds the outputs the
// inputs spend, for every input but the coinbase's, in block order.
BlockSigops :: struct { code: int; cost: int; tx: int; }
block_sigops_vartime :: (blk: [] u8, ins: [] TxIn, outs: [] TxOut, items: [] TxItem, prev: [] u8, spent: [] SigopSpent, flags: int) -> BlockSigops #export_c {
  r := BlockSigops.{tx=-1};
  if blk.count <= MERKLE_HEADER { r.code = BLOCK_BAD; return r; }
  c := tx_compact_read_vartime(blk, MERKLE_HEADER);
  if c.code != TX_OK { r.code = BLOCK_BAD; return r; }
  at := c.next; used := 0;
  for i: 0..<c.value {
    s := blk[at..<blk.count];
    t := tx_parse_vartime(s, true, ins, outs, items);
    if t.code != TX_OK { r.code = BLOCK_BAD; r.tx = i; return r; }
    mine := spent[0..<0];
    if !tx_coinbase_vartime(s, t, ins) {
      if used + t.nin > spent.count { r.code = BLOCK_CAPACITY; r.tx = i; return r; }
      mine = spent[used..<used+t.nin]; used += t.nin;
    }
    r.cost += sigop_cost_vartime(s[0..<t.total], t, ins, outs, items, prev, mine, flags);
    if r.cost > BLOCK_MAX_SIGOPS_COST { r.code = BLOCK_SIGOPS; r.tx = i; return r; }
    at += t.total;
  }
  if used != spent.count { r.code = BLOCK_CAPACITY; }
  return r;
}
```

`spent` lists the output each input spends, from the UTXO set (chapter
56). Chapter 58 connects blocks and passes it.

## Mainnet block 514,999

Chapter 46 pinned block 514,999, 19 transactions mined in March 2018. Its
spent outputs are pinned beside it, as mempool.space published them; the
vectors script checks each one against the block, by the hash of a P2SH
redeem script, a P2WPKH key or a P2PKH key, and checks that the fees add
up to the coinbase's claim above the 12.5 BTC subsidy.

```text
Mainnet block 514,999, 19 transactions:
  4571 bytes without witnesses, 5044 in all, weight 18757
  legacy sigops 25, cost 100 of CheckBlock's 80000
  CheckBlock: ok; the weight limit: ok
  cost of each transaction: 4 12 8 4 8 8 4 8 8 4 5 4 4 1 1 1 16 8 8
  19 of 19 agree with mempool.space
  ConnectBlock's sigop cost: 116 of 80000; without P2SH and WITNESS: 100
```

Most of its transactions pay to P2PKH outputs, each 1 legacy sigop and a
cost of 4. Transaction 10 spends a P2WPKH output: 1 witness sigop plus 4
for its one P2PKH output, a cost of 5. Transaction 16 spends a P2SH
multisig of three keys: 3 accurate sigops, 12, plus 4 for its P2PKH
output. mempool.space
computes the same 19 costs with its own code.

## Blocks that break a rule

Each crafted block carries its own Merkle root, as a miner would write it,
so that it reaches the rule it breaks. The two sigop clusters are Core's
`miner_tests` `CreateBigSigOpsCluster`: a parent whose scriptSig holds one
`OP_CHECKSIG`, and 50 children with 20 outputs each of
`OP_0 OP_0 OP_0 OP_NOP OP_CHECKMULTISIG OP_1`. The `OP_NOP` before each
multisig makes even the accurate count say 20, so the block holds 20,001
legacy sigops, cost 80,004. Without the parent's `OP_CHECKSIG` it holds
20,000, exactly the limit, which passes.

```text
Blocks that break a rule:
  514,999 with a changed Merkle root: bad-txnmrklroot
  with its last transaction twice: bad-txns-duplicate
  without its coinbase: bad-cb-missing
  with a second coinbase: bad-cb-multiple
  with transaction 1 spending one outpoint twice: bad-txns-inputs-duplicate
  Core's sigop cluster, 52 transactions and 20001 legacy sigops: bad-blk-sigops
  Core's sigop cluster, 52 transactions and 20000 legacy sigops: ok
  a block of 1000000 bytes without witnesses: ok
  a block of 1000001 bytes without witnesses: bad-blk-length
  a block of weight 4000000, 168 bytes without witnesses: CheckBlock ok, then ok
  a block of weight 4000001, 168 bytes without witnesses: CheckBlock ok, then bad-blk-weight
```

The last four blocks are one coinbase each. A coinbase output script of
999,853 bytes makes a block of exactly 1,000,000 bytes without witnesses;
one byte more refuses. A coinbase witness of 3,999,320 bytes makes a
block of weight exactly 4,000,000; one byte more passes `CheckBlock` and
then refuses on weight.

## The audit

Chapter 40's `fees.coax` took a sigop cost as an input to
`GetVirtualTransactionSize`, but nothing computed one. Chapter 46's
`merkle.coax` read blocks and checked their roots, and chapter 52's
`txcheck.coax` checked one transaction. Nothing counted sigops, checked a
block's size, weight or coinbase position, or composed the checks in
Core's order before this chapter.

## The example

The example is
[bitcoin/examples/sigop-limits.coax](../examples/sigop-limits.coax). It
counts Core's scripts, runs Core's transaction cases, checks block 514,999
and its sigop cost, and refuses each crafted block.

<!-- source: bitcoin/examples/sigop-limits.coax example -->
```jai
#load "../../coax/lib/bitcoin/sigops.coax";
#import "Basic";

reason :: (c: BlockCheck) -> string {
  if c.code == BLOCK_TX { return TXC_REASONS[c.tx_code]; }
  return BLOCK_REASONS[c.code];
}
script :: (text: string, out: [] u8) -> [] u8 { return sg_unhex(text, out); }

main :: () {
  s: [128] u8; c: [128] u8;
  f := SCRIPT_VERIFY_P2SH | SCRIPT_VERIFY_WITNESS;

  // Counting one script: Core's sigopcount_tests.
  print("Counting signature operations (GetSigOpCount):\n");
  print("  OP_CHECKSIG: %\n", sigop_count_vartime(script("ac", s[..]), false));
  multi := script(SG_MULTI, s[..]);
  print("  1 <key> <key> 2 OP_CHECKMULTISIGVERIFY: legacy %, accurate %\n", sigop_count_vartime(multi, false), sigop_count_vartime(multi, true));
  core := script("5114000000000000000000000000000000000000000014000000000000000000000000000000000000000052ae63ac68", s[..]);
  print("  1 <20 bytes> <20 bytes> 2 OP_CHECKMULTISIG OP_IF OP_CHECKSIG OP_ENDIF: legacy %, accurate %\n", sigop_count_vartime(core, false), sigop_count_vartime(core, true));
  p2sh := script("a914000000000000000000000000000000000000000087", c[..]);
  sig: [128] u8;
  print("  that script under P2SH, spent with OP_0 <script>: %\n", sigop_p2sh_vartime(p2sh, script("00305114000000000000000000000000000000000000000014000000000000000000000000000000000000000052ae63ac68", sig[..])));

  // A transaction's cost: Core's GetTxSigOpCost cases.
  print("Core's GetTxSigOpCost cases, under P2SH and WITNESS:\n");
  nothing: [1] string; none := nothing[0..<0];
  two := [2] string.["", ""];
  three := [3] string.["", "", SG_MULTI];
  print("  spending a bare 1-of-2 multisig: %\n", sg_cost("0000", none, SG_MULTI, f, false));
  in_p2sh := "00004751210211111111111111111111111111111111111111111111111111111111111111112102111111111111111111111111111111111111111111111111111111111111111152af";
  p2sh_hex := "a914000000000000000000000000000000000000000087";
  print("  the multisig in P2SH: % (without the P2SH flag: %)\n", sg_cost(in_p2sh, none, p2sh_hex, f, false), sg_cost(in_p2sh, none, p2sh_hex, 0, false));
  p2wpkh := "00140000000000000000000000000000000000000000";
  print("  P2WPKH: % (without WITNESS: %; as version 1: %; as a coinbase: %)\n", sg_cost("", two[..], p2wpkh, f, false),
    sg_cost("", two[..], p2wpkh, SCRIPT_VERIFY_P2SH, false), sg_cost("", two[..], "51140000000000000000000000000000000000000000", f, false), sg_cost("", two[..], p2wpkh, f, true));
  print("  P2WPKH in P2SH: %\n", sg_cost("1600140000000000000000000000000000000000000000", two[..], p2sh_hex, f, false));
  p2wsh := "00200000000000000000000000000000000000000000000000000000000000000000";
  print("  P2WSH of the multisig: % (without WITNESS: %)\n", sg_cost("", three[..], p2wsh, f, false), sg_cost("", three[..], p2wsh, SCRIPT_VERIFY_P2SH, false));
  print("  P2WSH in P2SH: %\n", sg_cost("2200200000000000000000000000000000000000000000000000000000000000000000", three[..], p2sh_hex, f, false));

  // A real block: mainnet 514,999 and the outputs it spends.
  blk: [6000] u8; ins: [64] TxIn; outs: [64] TxOut; items: [64] TxItem;
  txids: [1700] u8; wtxids: [1700] u8; work: [1700] u8;
  n := sg_block(blk[..]);
  b := blk[0..<n];
  k := block_check_vartime(b, ins[..], outs[..], items[..], txids[..], wtxids[..], work[..]);
  print("Mainnet block 514,999, % transactions:\n", k.count);
  print("  % bytes without witnesses, % in all, weight %\n", k.stripped, n, k.weight);
  print("  legacy sigops %, cost % of CheckBlock's %\n", k.legacy, k.legacy * TX_SCALE, BLOCK_MAX_SIGOPS_COST);
  print("  CheckBlock: %; the weight limit: %\n", reason(k), BLOCK_REASONS[block_weight_check_vartime(k)]);
  prev: [1024] u8; spent: [32] SigopSpent;
  m := sg_spent(prev[..], spent[..]);
  costs: [19] int;
  count := sg_costs(b, prev[..], spent[0..<m], f, costs[..]);
  print("  cost of each transaction:");
  same := 0;
  for i: 0..<count { print(" %", costs[i]); if costs[i] == SIGOPS_MEMPOOL_514999[i] { same += 1; } }
  print("\n  % of % agree with mempool.space\n", same, count);
  x := block_sigops_vartime(b, ins[..], outs[..], items[..], prev[..], spent[0..<m], f);
  print("  ConnectBlock's sigop cost: % of %; without P2SH and WITNESS: %\n", x.cost, BLOCK_MAX_SIGOPS_COST,
    block_sigops_vartime(b, ins[..], outs[..], items[..], prev[..], spent[0..<m], 0).cost);

  // Changed and crafted blocks, each under its own Merkle root unless
  // the root is what is changed.
  print("Blocks that break a rule:\n");
  big: [4000200] u8;
  for i: 0..<n { big[i] = b[i]; }
  big[40] ^= 1;
  print("  514,999 with a changed Merkle root: %\n", reason(block_check_vartime(big[0..<n], ins[..], outs[..], items[..], txids[..], wtxids[..], work[..])));
  big[40] ^= 1;
  last := sg_last(b);
  big[80] = 20;
  e := cp_put(big[..], n, b[last..<n]);
  print("  with its last transaction twice: %\n", reason(block_check_vartime(big[0..<e], ins[..], outs[..], items[..], txids[..], wtxids[..], work[..])));
  first := sg_first_end(b);
  e = cp_put(big[..], 81, b[first..<n]);
  big[80] = 18;
  sg_fix_root(big[0..<e], ins[..], outs[..], items[..], txids[..], wtxids[..], work[..]);
  print("  without its coinbase: %\n", reason(block_check_vartime(big[0..<e], ins[..], outs[..], items[..], txids[..], wtxids[..], work[..])));
  e = cp_put(big[..], 0, b);
  e = cp_put(big[..], e, b[81..<first]);
  big[80] = 20;
  big[n + 4 + 2 + 1 + 36 + 1 + 3] ^= 1;
  sg_fix_root(big[0..<e], ins[..], outs[..], items[..], txids[..], wtxids[..], work[..]);
  print("  with a second coinbase: %\n", reason(block_check_vartime(big[0..<e], ins[..], outs[..], items[..], txids[..], wtxids[..], work[..])));
  e = cp_put(big[..], 0, b);
  t := tx_parse_vartime(b[first..<n], true, ins[..], outs[..], items[..]);
  for i: 0..<36 { big[first + ins[1].outpoint + i] = b[first + ins[0].outpoint + i]; }
  sg_fix_root(big[0..<e], ins[..], outs[..], items[..], txids[..], wtxids[..], work[..]);
  print("  with transaction 1 spending one outpoint twice: %\n", reason(block_check_vartime(big[0..<e], ins[..], outs[..], items[..], txids[..], wtxids[..], work[..])));
  for checksig: 0..<2 {
    e = sg_cluster(big[..], checksig == 0);
    sg_fix_root(big[0..<e], ins[..], outs[..], items[..], txids[..], wtxids[..], work[..]);
    q := block_check_vartime(big[0..<e], ins[..], outs[..], items[..], txids[..], wtxids[..], work[..]);
    print("  Core's sigop cluster, % transactions and % legacy sigops: %\n", q.count, q.legacy, reason(q));
  }
  // One coinbase, its output script sized to make the block 1,000,000 bytes
  // without witnesses, then one byte more.
  e = sg_coinbase_block(big[..], 999000, -1);
  spk_n := 999000 + 1000000 - e;
  for d: 0..<2 {
    e = sg_coinbase_block(big[..], spk_n + d, -1);
    q := block_check_vartime(big[0..<e], ins[..], outs[..], items[..], txids[..], wtxids[..], work[..]);
    print("  a block of % bytes without witnesses: %\n", q.stripped, reason(q));
  }
  // A small coinbase with a witness sized to make the weight 4,000,000,
  // then one more.
  e = sg_coinbase_block(big[..], 25, 3000000);
  wit := 3000000 + 4000000 - (3 * (e - 8 - 3000000) + e);
  for d: 0..<2 {
    e = sg_coinbase_block(big[..], 25, wit + d);
    q := block_check_vartime(big[0..<e], ins[..], outs[..], items[..], txids[..], wtxids[..], work[..]);
    print("  a block of weight %, % bytes without witnesses: CheckBlock %, then %\n", q.weight, q.stripped, reason(q), BLOCK_REASONS[block_weight_check_vartime(q)]);
  }
}
// Where block b's last transaction starts, and where its first ends.
sg_last :: (b: [] u8) -> int {
  ins: [64] TxIn; outs: [64] TxOut; items: [64] TxItem; ids: [1700] u8; w: [1700] u8;
  return merkle_block_ids_vartime(b, false, ins[..], outs[..], items[..], ids[..], w[..]).last;
}
sg_first_end :: (b: [] u8) -> int {
  ins: [64] TxIn; outs: [64] TxOut; items: [64] TxItem;
  return 81 + tx_parse_vartime(b[81..<b.count], true, ins[..], outs[..], items[..]).total;
}
```

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/sigop-limits.out](../fixtures/sigop-limits.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/sigop-limits.coax](../examples/checks/sigop-limits.coax),
which loads the example:

- every opcode before `OP_CHECKMULTISIG` gives Core's count, accurate and
  not;
- the accurate count is never above the legacy count, for every pair of
  opcodes before an `OP_CHECKSIG`;
- without the P2SH and WITNESS flags, `ConnectBlock`'s cost is
  `CheckBlock`'s legacy count times 4, for block 514,999 and both
  clusters.

<!-- source: bitcoin/examples/checks/sigop-limits.coax check -->
```jai
// OP_n OP_CHECKMULTISIG counts n accurately and 20 otherwise; any other
// opcode before it counts 20 both ways.
#forall "a multisig counts its key count only when accurate" op: 0..<256 #steps 100000000 {
  s: [2] u8;
  s[0] = cast(u8) op; s[1] = OP_CHECKMULTISIG;
  // A push opcode needs its bytes: 0x01 to 0x4e run past the end.
  if op >= 1 && op <= OP_PUSHDATA4 {
    #assert sigop_count_vartime(s[..], true) == 0 && sigop_count_vartime(s[..], false) == 0;
  } else {
    // A signature opcode before the multisig counts too.
    first := ifx op == OP_CHECKSIG || op == OP_CHECKSIGVERIFY then 1 else ifx op == OP_CHECKMULTISIG || op == OP_CHECKMULTISIGVERIFY then SCRIPT_MAX_KEYS else 0;
    want := ifx op >= OP_1 && op <= OP_16 then op - 0x50 else SCRIPT_MAX_KEYS;
    #assert sigop_count_vartime(s[..], true) == first + want && sigop_count_vartime(s[..], false) == first + SCRIPT_MAX_KEYS;
  }
}
// The accurate count is never above the legacy one, over every pair of
// opcodes before a CHECKSIG.
#forall "accurate is at most legacy" x: 0..<256 #steps 1000000000 {
  s: [3] u8;
  for y: 0..<256 {
    s[0] = cast(u8) x; s[1] = cast(u8) y; s[2] = OP_CHECKSIG;
    #assert sigop_count_vartime(s[..], true) <= sigop_count_vartime(s[..], false);
  }
}
// Without P2SH and WITNESS, ConnectBlock's cost is CheckBlock's legacy
// count times 4, for block 514,999 and both sigop clusters.
#forall "the cost without flags is the legacy count times 4" which: 0..<3 #steps 1000000000 {
  blk: [24000] u8; ins: [64] TxIn; outs: [64] TxOut; items: [64] TxItem;
  txids: [1700] u8; wtxids: [1700] u8; work: [1700] u8; prev: [1024] u8; spent: [64] SigopSpent;
  n := 0; k := 0;
  if which == 0 { n = sg_block(blk[..]); k = sg_spent(prev[..], spent[..]); }
  else {
    n = sg_cluster(blk[..], which == 1);
    prev[0] = OP_1; k = 51;
    for i: 0..<k { spent[i] = SigopSpent.{script=0, script_n=1}; }
  }
  b := blk[0..<n];
  sg_fix_root(b, ins[..], outs[..], items[..], txids[..], wtxids[..], work[..]);
  c := block_check_vartime(b, ins[..], outs[..], items[..], txids[..], wtxids[..], work[..]);
  x := block_sigops_vartime(b, ins[..], outs[..], items[..], prev[..], spent[0..<k], 0);
  #assert c.legacy * TX_SCALE == x.cost || (x.code == BLOCK_SIGOPS && c.legacy * TX_SCALE > BLOCK_MAX_SIGOPS_COST);
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/sigop-limits.coax
```

`coax test` runs 305 checks, including the loaded files' own checks, in about 125 seconds on 2026-11-03.

### The library's own checks

`sigops.coax` is an `#authority` file. Its checks run Core's
`sigopcount_tests` (`GetSigOpCount` and every `GetTxSigOpCost` case), the
opcode before a multisig and a push that runs past the end, the P2SH
count with scriptSigs that are not push-only, block 514,999 with each
transaction's cost against mempool.space's, each changed block with its
refusal, Core's sigop cluster on both sides of the limit, the weight
bound, and blocks that do not read or do not fit the caller's arrays.

```shell
coax/bin/coax test coax/lib/bitcoin/sigops.coax
```

`coax test` runs 302 checks, including those of the files it loads, in about 120 seconds on 2026-11-03. Mutation testing: not run (owner speed order).

### Python

[sigops.py](../../coax/tests/bitcoin/sigops.py) reads the counting rules
from the pinned `script.cpp`, `interpreter.cpp` and `tx_verify.cpp`, the
order and reasons of `CheckBlock`, `CheckMerkleRoot`,
`ContextualCheckBlock`'s weight rule and `ConnectBlock`'s sigop limit from
`validation.cpp`, the limits from `consensus.h` and `script.h`, and the
expected counts from `sigopcount_tests.cpp` and `miner_tests.cpp`. It
checks that `sigops.coax`'s reasons and constants are Core's, then runs
transliterations of the counts and the block checks, with chapter 34's
reader and chapter 46's Merkle root, builds the same crafted blocks, and
writes the transcript the example must print.
[make_sigops_vectors.py](../../coax/tests/bitcoin/make_sigops_vectors.py)
writes the spent outputs of block 514,999 from the pinned JSON and checks
them against the block.

```shell
python3 -I coax/tests/bitcoin/sigops.py --coax coax/bin/coax
```

On 2026-11-03 all 31 lines agree with Python, and the example's output in native debug and release builds equals them. Wasm was not run.

## What this chapter leaves out

- The header checks: chapters 47, 48, 50 and 51.
- The witness commitment, which comes before the weight rule: chapter 49.
- The coinbase's height and what it may claim: chapter 54.
- The script flags of each height, which decide whether P2SH and witness
  sigops count: chapter 55.
- Tapscript's per-input signature budget: chapter 44.
- Policy limits on a single transaction, such as Core's 16,000 sigop cost
  for a standard transaction and the sigop-adjusted virtual size: chapters
  81 and 82; chapter 40 computes the size.

## Summary

- A sigop is read from a script's bytes, not counted while it runs. The
  legacy count charges any multisig 20; the accurate count reads its key
  count.
- A transaction's sigop cost is its legacy and P2SH counts times 4 plus
  its witness count. A block's is at most 80,000.
- `CheckBlock` checks the Merkle root, the size, one coinbase first, each
  transaction and the legacy count, in that order.
- A block's weight is at most 4,000,000, checked after the witness
  commitment.
- Block 514,999's costs match mempool.space's, and each crafted block
  refuses with Core's reason.

## Dependencies

- [36. The script interpreter](ch036-the-script-interpreter.md)
- [40. Weight, vsize and fees](ch040-weight-vsize-and-fees.md)

## Sources

- Bitcoin Core at revision `ddea9196`: `script/script.cpp`
  (`CScript::GetSigOpCount`), `script/interpreter.cpp`
  (`CountWitnessSigOps`), `consensus/tx_verify.cpp`
  (`GetLegacySigOpCount`, `GetP2SHSigOpCount`, `GetTransactionSigOpCost`),
  `validation.cpp` (`CheckBlock`, `CheckMerkleRoot`,
  `ContextualCheckBlock`, `ConnectBlock`), `consensus/consensus.h`,
  `script/script.h`, `test/sigopcount_tests.cpp` and
  `test/miner_tests.cpp`, pinned in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/sigopcount_tests.cpp).
- [BIP-16](https://github.com/bitcoin/bips/blob/master/bip-0016.mediawiki)
  (P2SH and its sigop count) and
  [BIP-141](https://github.com/bitcoin/bips/blob/master/bip-0141.mediawiki)
  (weight and the sigop cost).
- Mainnet block 514,999 and the outputs it spends, from mempool.space,
  pinned in
  [coax/tests/vectors/upstream/mainnet/](../../coax/tests/vectors/upstream/mainnet/00000000000000000008fda0e5bb3105dab72605332085684c42dceb0ea61097.spent.json).
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
