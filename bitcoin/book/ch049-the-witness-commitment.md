# 49. The witness commitment

A block header commits to its transactions through the Merkle root of
their txids (chapter 46). A txid leaves out the witness, so the header
says nothing about the signatures and scripts in a segwit transaction's
witness. Segwit was a soft fork: it could not change the header. BIP-141
puts the missing commitment inside the block instead. The coinbase gets
one more output, an `OP_RETURN` that holds a hash of the root of the
wtxids, and the coinbase's input gets a witness of its own, a 32-byte
value reserved for future commitments.

The commitment matters because a witness can change without changing
anything the header covers. Change one byte of a signature in a witness
and the txid, the Merkle root, the header and the block hash all stay the
same. Without the commitment, a peer could hand a node a block whose hash
is right and whose witnesses are wrong. With it, the node finds the
change, and knows the block it was given is mutated, not invalid.

This chapter finds the commitment, recomputes it and runs Bitcoin Core's
three rules for it, with Core's reject reasons. It then runs
`IsBlockMutated`, the check a node makes on a block it received or rebuilt
before it trusts the block's hash, which chapter 73 uses for compact
blocks.

## What this chapter teaches

- Where the commitment is: the last coinbase output that starts
  `6a24aa21a9ed` and has at least 38 bytes.
- What it commits to: SHA-256d of the witness root and the witness
  reserved value, the one item of the coinbase's witness.
- Core's three rules: `bad-witness-nonce-size`, `bad-witness-merkle-match`
  and `unexpected-witness`, and when each applies.
- Why a block with no witnesses may leave the commitment out, and why one
  with witnesses before segwit's activation is refused.
- How `IsBlockMutated` puts the Merkle root and the witness rules together
  to tell a mutated block from an invalid one.

## Where Bitcoin uses the commitment

| Where | What it uses | Chapter |
| --- | --- | --- |
| Signet | the commitment output carries the block's signature after the 32-byte hash | 50 |
| Block validation | `CheckWitnessMalleation` in `ContextualCheckBlock`, before the weight limit | 57 |
| Block download | `IsBlockMutated` on every `block` message from a peer | 72 |
| Compact blocks | `IsBlockMutated` on a rebuilt block, to catch a short-ID collision | 73 |
| Mining | `GenerateCoinbaseCommitment` adds the output to a template | 88 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [witness.coax](../../coax/lib/bitcoin/witness.coax) | `witness_commitment_index_vartime(b, t, outs)` | `GetWitnessCommitmentIndex`: which coinbase output commits |
| | `witness_commitment_hash_vartime(root, reserved, out)` | SHA-256d(witness root, reserved value) |
| | `witness_commitment_script_vartime(root, reserved, out)` | the 38-byte script `GenerateCoinbaseCommitment` writes |
| | `witness_check_vartime(b, segwit, ins, outs, items, wtxids, work)` | `CheckWitnessMalleation` |
| | `witness_mutated_vartime(b, segwit, ins, outs, items, txids, wtxids, work)` | `IsBlockMutated` |
| [merkle.coax](../../coax/lib/bitcoin/merkle.coax) | `merkle_witness_root_vartime`, `merkle_check_vartime` | the witness root and the header's root (chapter 46) |
| [tx.coax](../../coax/lib/bitcoin/tx.coax) | `tx_parse_vartime`, `tx_wtxid_vartime` | transactions and their wtxids (chapters 34 and 35) |

Blocks are public, so every procedure runs in variable time and ends in
`_vartime` (chapter 3). The procedures copy nothing: they read positions
in the caller's bytes and write hashes into the caller's views.

The result codes carry Core's reject reasons:

<!-- source: coax/lib/bitcoin/witness.coax codes -->
```jai
// Result codes. Core's reject reason, where there is one, is in quotes.
WITNESS_OK :: 0;
WITNESS_NONCE_SIZE :: 1;    // "bad-witness-nonce-size": the coinbase's witness is not one 32-byte item
WITNESS_MERKLE_MATCH :: 2;  // "bad-witness-merkle-match": the commitment is not to these wtxids
WITNESS_UNEXPECTED :: 3;    // "unexpected-witness": a witness in a block that commits to none
WITNESS_TXN_MERKLE :: 4;    // "bad-txnmrklroot": the txids' root is not the header's
WITNESS_DUPLICATE :: 5;     // "bad-txns-duplicate": the txid tree is mutated (chapter 46)
WITNESS_TX_64 :: 6;         // no coinbase first, and a transaction of 64 stripped bytes
WITNESS_BAD_BLOCK :: 7;     // the block's bytes don't read; the transaction reader's code says why
WITNESS_CAPACITY :: 8;      // more transactions, inputs, outputs or items than the caller's views hold

WITNESS_NONE :: -1;         // NO_WITNESS_COMMITMENT
WITNESS_MINIMUM :: 38;      // MINIMUM_WITNESS_COMMITMENT: a script at least this long
WITNESS_HEADER :: [6] u8.[0x6a, 0x24, 0xaa, 0x21, 0xa9, 0xed];   // OP_RETURN, a push of 36, aa21a9ed
```

## The commitment

BIP-141 gives the layout of the output's script, its `scriptPubKey`:

```text
1 byte    OP_RETURN (0x6a)
1 byte    push the next 36 bytes (0x24)
4 bytes   the commitment header, aa21a9ed
32 bytes  SHA-256d(witness root || witness reserved value)
38th byte onward: optional data with no consensus meaning
```

The witness root is chapter 46's tree over the wtxids, with the
coinbase's wtxid taken as 32 zero bytes. The coinbase can't commit to its
own wtxid: the commitment is part of the coinbase, so its wtxid depends on
the hash it would have to contain.

### Finding it

`OP_RETURN` makes an output unspendable, and many coinbases have more
than one. Core's `GetWitnessCommitmentIndex` in `consensus/validation.h`
looks at every output and keeps the last one whose script is at least 38
bytes and starts with those six bytes:

<!-- source: coax/lib/bitcoin/witness.coax index -->
```jai
// GetWitnessCommitmentIndex: the last output of the coinbase b (read as
// t) whose scriptPubKey has at least 38 bytes and starts with the six
// bytes of WITNESS_HEADER; WITNESS_NONE when none does. Bytes after the
// 38th have no meaning, and an earlier match is not the commitment.
witness_commitment_index_vartime :: (b: [] u8, t: Tx, outs: [] TxOut) -> int #export_c {
  found := WITNESS_NONE;
  for o: 0..<t.nout {
    s := b[outs[o].script..<outs[o].script+outs[o].script_n];
    if s.count < WITNESS_MINIMUM { continue; }
    same := true;
    for i: 0..<6 { if s[i] != WITNESS_HEADER[i] { same = false; } }
    if same { found = o; }
  }
  return found;
}
```

"Last" is BIP-141's own rule: "If there are more than one
`scriptPubKey` matching the pattern, the one with highest output index is
assumed to be the commitment." A miner that adds a second commitment
after the real one breaks the block; one added before it changes nothing.
The script may be longer than 38 bytes, and signet (chapter 50) uses the
bytes after the 38th to carry the block's signature. A script of 37 bytes
is not a commitment at all.

### Computing it

The hash covers 64 bytes: the witness root, then the reserved value.

<!-- source: coax/lib/bitcoin/witness.coax hash -->
```jai
// The commitment: SHA-256d of the witness root (32 bytes, hash order)
// and then the witness reserved value (32 bytes), into out.
witness_commitment_hash_vartime :: (root: [] u8, reserved: [] u8, out: [] u8) -> bool #export_c {
  if root.count != 32 || reserved.count != 32 || out.count != 32 { return false; }
  pair: [64] u8;
  for i: 0..<32 { pair[i] = root[i]; pair[32+i] = reserved[i]; }
  return sha256d_checked(pair[..], out, false) == SHA_OK;
}
// The 38-byte scriptPubKey that carries it, as GenerateCoinbaseCommitment
// and blocktools.py's get_witness_script write it: WITNESS_HEADER, then
// the commitment. Returns 38, or -1 when a view is the wrong size.
witness_commitment_script_vartime :: (root: [] u8, reserved: [] u8, out: [] u8) -> int #export_c {
  if out.count < WITNESS_MINIMUM { return -1; }
  if !witness_commitment_hash_vartime(root, reserved, out[6..<38]) { return -1; }
  for i: 0..<6 { out[i] = WITNESS_HEADER[i]; }
  return WITNESS_MINIMUM;
}
```

Core's miner, `GenerateCoinbaseCommitment` in `validation.cpp`, writes the
output with a reserved value of 32 zero bytes and then, in
`UpdateUncommittedBlockStructures`, sets the coinbase's witness to that
value. Bitcoin Core's functional tests do the same with
`add_witness_commitment` from `test_framework/blocktools.py`, which takes
the reserved value as a number.

### The reserved value

The coinbase's single input has a witness stack of exactly one item of 32
bytes. BIP-141 calls it the witness reserved value and gives it no meaning
yet. It exists so that a later soft fork can commit to something new
without another coinbase output: the coinbase witness would hold
`Hash(new commitment || reserved value)`, and the reserved value itself
would move elsewhere. Commitments that aren't consensus rules, such as
merge mining, must not use it.

### Worked example: block 514,999

Mainnet block 514,999 has 19 transactions, and five of them have
witnesses: the coinbase and transactions 10, 13, 14 and 15. Its coinbase
has two outputs, and the second commits:

```text
block 514,999: 19 transactions
  with witnesses: 0 10 13 14 15
  coinbase outputs: 2, commitment at output 1
  its script: 6a24aa21a9ed0bbd0d4ebdafaf572ceac348d1477a962301e18a2b53824fc46f0af56647cfbe
  coinbase witness: 1 item of 32 bytes
  reserved value: 0000000000000000000000000000000000000000000000000000000000000000
  witness root: 6947cde2e7987a0f5d059432c314d504304bb5f86e3138069bacb56e44b9e411
  SHA-256d(root || reserved): 0bbd0d4ebdafaf572ceac348d1477a962301e18a2b53824fc46f0af56647cfbe
  check: ok
  the script GenerateCoinbaseCommitment writes is the block's: true
```

The witness root is shown reversed, the way explorers show hashes. The
SHA-256d digest is in hash order, the order it has in the script, and its
32 bytes are the script's last 32. The reserved value is zero, as Core's
miner writes it, so the script Core would generate for this witness root
is the one in the block, byte for byte.

## The rules

`ContextualCheckBlock` calls `CheckWitnessMalleation` with
`expect_witness_commitment` set when segwit is active for the block, that
is, from height 481,824 on mainnet. The checker walks the block once,
writing each wtxid and noting the first transaction with a witness:

<!-- source: coax/lib/bitcoin/witness.coax walk -->
```jai
// What a check found. code is WITNESS_OK or a refusal; a block that
// doesn't read keeps the transaction reader's code in tx_code and the
// byte in at. count transactions were read; the coinbase starts at byte
// coinbase; first_witness is the first transaction with a witness, or -1;
// commitment is the coinbase output that commits, or WITNESS_NONE; and
// reserved is where the reserved value is, when it was found.
WitnessCheck :: struct {
  code: int; at: int; tx_code: int; count: int;
  coinbase: int; first_witness: int; commitment: int; reserved: int;
}
// Reads a serialized block as chapter 46's merkle_block_ids_vartime does,
// writing each txid (unless txids is empty) and each wtxid, and noting the
// first transaction with a witness. A transaction read with the segwit
// marker has a witness: the reader refuses one whose stacks are all empty.
witness_walk_vartime :: (b: [] u8, ins: [] TxIn, outs: [] TxOut, items: [] TxItem, txids: [] u8, wtxids: [] u8) -> WitnessCheck {
  k := WitnessCheck.{first_witness=-1, commitment=WITNESS_NONE};
  if b.count < MERKLE_HEADER { k.code = WITNESS_BAD_BLOCK; k.tx_code = TX_TRUNCATED; return k; }
  c := tx_compact_read_vartime(b, MERKLE_HEADER);
  if c.code != TX_OK { k.code = WITNESS_BAD_BLOCK; k.tx_code = c.code; k.at = c.next; return k; }
  if wtxids.count < 32 * c.value || (txids.count > 0 && txids.count < 32 * c.value) {
    k.code = WITNESS_CAPACITY; k.at = MERKLE_HEADER; return k;
  }
  at := c.next;
  k.coinbase = at;
  while k.count < c.value {
    t := tx_parse_vartime(b[at..<b.count], true, ins, outs, items);
    if t.code != TX_OK {
      k.code = ifx t.code == TX_CAPACITY then WITNESS_CAPACITY else WITNESS_BAD_BLOCK;
      k.tx_code = t.code; k.at = at + t.at;
      return k;
    }
    s := b[at..<at+t.total];
    n := k.count;
    if t.segwit && k.first_witness < 0 { k.first_witness = n; }
    if txids.count > 0 { #assert tx_txid_vartime(s, t, txids[32*n..<32*n+32]); }
    #assert tx_wtxid_vartime(s, t, wtxids[32*n..<32*n+32]);
    k.count += 1; at += t.total;
  }
  if at != b.count { k.code = WITNESS_BAD_BLOCK; k.tx_code = TX_TRAILING; k.at = at; }
  return k;
}
```

The rules then run in Core's order:

<!-- source: coax/lib/bitcoin/witness.coax check -->
```jai
// CheckWitnessMalleation, for a block b. With segwit (the deployment is
// active, so the block must commit) and a commitment in the coinbase, the
// coinbase's first input must have one witness item of 32 bytes, the
// reserved value, and the commitment must be SHA-256d(witness root ||
// reserved value). Otherwise no transaction may have a witness. work holds
// at least as many bytes as the wtxids.
witness_check_vartime :: (b: [] u8, segwit: bool, ins: [] TxIn, outs: [] TxOut, items: [] TxItem, wtxids: [] u8, work: [] u8) -> WitnessCheck #export_c {
  k := witness_walk_vartime(b, ins, outs, items, wtxids[0..<0], wtxids);
  if k.code != WITNESS_OK || k.count == 0 { return k; }
  if work.count < 32 * k.count { k.code = WITNESS_CAPACITY; return k; }
  cb := b[k.coinbase..<b.count];
  t := tx_parse_vartime(cb, true, ins, outs, items);
  k.commitment = witness_commitment_index_vartime(cb, t, outs);
  if segwit && k.commitment != WITNESS_NONE {
    if !t.segwit || ins[0].count != 1 || items[ins[0].first].n != 32 { k.code = WITNESS_NONCE_SIZE; return k; }
    k.reserved = k.coinbase + items[ins[0].first].at;
    root: [32] u8; want: [32] u8;
    #assert merkle_witness_root_vartime(wtxids[0..<32*k.count], work, root[..]) == MERKLE_OK;
    #assert witness_commitment_hash_vartime(root[..], b[k.reserved..<k.reserved+32], want[..]);
    got := cb[outs[k.commitment].script+6..<outs[k.commitment].script+38];
    for i: 0..<32 { if got[i] != want[i] { k.code = WITNESS_MERKLE_MATCH; return k; } }
    return k;
  }
  if k.first_witness >= 0 { k.code = WITNESS_UNEXPECTED; }
  return k;
}
```

When segwit is active and the coinbase commits:

1. The coinbase's first input must have exactly one witness item, of 32
   bytes. A missing witness, two items or an item of 31 bytes is
   `bad-witness-nonce-size`. Core checks this before it hashes anything.
2. SHA-256d(witness root || reserved value) must equal bytes 6 to 38 of
   the commitment's script, or the block is `bad-witness-merkle-match`.

Otherwise, whether segwit is inactive or the coinbase has no commitment,
no transaction may have a witness. One that does is `unexpected-witness`.
This one rule covers two cases:

- After activation, a block whose transactions have no witnesses may
  leave the commitment out. BIP-141 says so: "If all transactions in a
  block do not have witness data, the commitment is optional." A block
  with witnesses and no commitment would let anyone change them, so it is
  refused.
- Before activation, witnesses mean nothing to the rules, so a block that
  carries any is refused, even if it also commits. That keeps old blocks
  from carrying arbitrary data that no rule checked.

A transaction has a witness when one of its inputs has a non-empty stack.
Chapter 34's reader refuses a transaction written with the segwit marker
whose stacks are all empty, so here a transaction has a witness exactly
when it was read with the marker.

### Why the rules come before the weight limit

In `ContextualCheckBlock` the witness rules run before the weight limit,
`bad-blk-weight`. A comment there explains why. The coinbase's witness
doesn't change the block hash, so before the reserved value is checked,
anyone could pad it until the block is too heavy. Core marks a block that
fails a consensus check as permanently invalid, by its hash. If the weight
came first, a padded copy of a good block would get the good block's hash
marked invalid. Checking the witness first means a too-heavy block that
gets past it was too heavy as mined.

### Worked example: the cases

The small block is block 514,999 cut to its coinbase, its transaction 10
(segwit) and its transaction 2 (legacy), with the commitment written again
by Core's `add_witness_commitment`. Each other case changes one thing and
then writes the header's Merkle root again, so that only the witness rules
decide it:

```text
CheckWitnessMalleation
  block 514,999: commitment at output 1, ok
  block 100,000: no commitment, ok
  block 100,000 (segwit not active): no commitment, ok
  the small block: commitment at output 1, ok
  the small block (segwit not active): commitment at output 1, unexpected-witness
  reserved value 7: commitment at output 1, ok
  a witness byte changed: commitment at output 1, bad-witness-merkle-match
  the commitment changed: commitment at output 1, bad-witness-merkle-match
  a 31-byte reserved value: commitment at output 1, bad-witness-nonce-size
  two reserved values: commitment at output 1, bad-witness-nonce-size
  no coinbase witness: commitment at output 1, bad-witness-nonce-size
  a wrong commitment after it: commitment at output 2, bad-witness-merkle-match
  a wrong commitment before it: commitment at output 2, ok
  a 39-byte commitment: commitment at output 1, ok
  a 37-byte commitment: no commitment, unexpected-witness
  no commitment: no commitment, unexpected-witness
  no commitment, no witnesses: no commitment, ok
  no commitment, no witnesses (segwit not active): no commitment, ok
```

Block 100,000 predates segwit: no witnesses and no commitment, so it
passes either way. A wrong commitment after the real one becomes output
2, the one that counts, and fails; before it, the real one is still last.
A 37-byte script doesn't match, so the block has witnesses and no
commitment.

## Mutated, not invalid

A node that gets a block from a peer has to decide what a failure means.
If the block is invalid, the node can mark its hash invalid forever. If
the peer changed bytes that the hash doesn't cover, the block behind the
hash may still be good, and marking it would let a peer make the node
reject a valid block. Core calls a block of the second kind mutated, and
checks for it with `IsBlockMutated` before anything is stored by hash.

The example changes one byte inside the witness of the small block's
segwit transaction:

```text
a witness byte changed
  same txids: true, same block hash: true
  wtxid of transaction 1 before: d58b754c9b0f78fc7421e6cd3d277e57163a1f001dee336843cc5f3fa5327f61
  wtxid of transaction 1 after: 92df639d81d710b7ba73b66893ec1c355a684e74d7ca57b213b84cdd3a908866
```

Nothing the header covers moved. Only the wtxid, and with it the witness
root, changed, so only the commitment can tell the two blocks apart.

`IsBlockMutated` checks the Merkle root first, then the witness rules:

<!-- source: coax/lib/bitcoin/witness.coax mutated -->
```jai
// IsBlockMutated: CheckMerkleRoot first ("bad-txnmrklroot", then
// "bad-txns-duplicate"). A block whose first transaction is not a coinbase
// is mutated only if some transaction has 64 stripped bytes; it is invalid
// anyway, and the witness rules are not run. Otherwise the witness rules
// decide, with segwit as check_witness_root.
witness_mutated_vartime :: (b: [] u8, segwit: bool, ins: [] TxIn, outs: [] TxOut, items: [] TxItem, txids: [] u8, wtxids: [] u8, work: [] u8) -> WitnessCheck #export_c {
  if txids.count == 0 { return WitnessCheck.{code=WITNESS_CAPACITY, first_witness=-1, commitment=WITNESS_NONE}; }
  k := witness_walk_vartime(b, ins, outs, items, txids, wtxids);
  if k.code != WITNESS_OK { return k; }
  if work.count < 32 * k.count { k.code = WITNESS_CAPACITY; return k; }
  m := merkle_check_vartime(txids[0..<32*k.count], work, b[36..<68]);
  if m == MERKLE_MISMATCH { k.code = WITNESS_TXN_MERKLE; return k; }
  if m == MERKLE_MUTATED { k.code = WITNESS_DUPLICATE; return k; }
  coinbase := false;
  if k.count > 0 {
    t := tx_parse_vartime(b[k.coinbase..<b.count], true, ins, outs, items);
    coinbase = tx_coinbase_vartime(b[k.coinbase..<b.count], t, ins);
  }
  if !coinbase {
    at := k.coinbase;
    for n: 0..<k.count {
      t := tx_parse_vartime(b[at..<b.count], true, ins, outs, items);
      if t.stripped == 64 { k.code = WITNESS_TX_64; return k; }
      at += t.total;
    }
    return k;
  }
  return witness_check_vartime(b, segwit, ins, outs, items, wtxids, work);
}
```

A root that doesn't match, or a tree with a repeated transaction (chapter
46), is mutated. A block that doesn't start with a coinbase is invalid in
any case; Core calls it mutated only if one of its transactions has 64
stripped bytes, which could be an inner node of the tree (chapter 46), and
otherwise leaves it to full validation. Core skips that test when there is
a coinbase: a 64-byte coinbase that is also an inner node needs about 224
bits of work to find. Then the witness rules decide, with
`check_witness_root` set when segwit is active:

```text
IsBlockMutated
  the small block: not mutated
  a witness byte changed: mutated, bad-witness-merkle-match
  its last transaction repeated: mutated, bad-txns-duplicate
  its header root changed: mutated, bad-txnmrklroot
  the small block (segwit not active): mutated, unexpected-witness
```

Core calls `IsBlockMutated` on every `block` message whose parent it
knows, and penalizes the peer that sent a mutated one. Chapter 73 calls it
on a block rebuilt from a compact block, where a mutated result most
likely means a short-ID collision rather than a dishonest peer.

## The example

The example is
[bitcoin/examples/witness-commitment.coax](../examples/witness-commitment.coax).
It shows block 514,999's commitment and recomputes it, runs
`CheckWitnessMalleation` on every case, compares a block with one witness
byte changed against the original, and runs `IsBlockMutated`.

<!-- source: bitcoin/examples/witness-commitment.coax example -->
```jai
#load "../../coax/lib/bitcoin/witness.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [200] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// A hash as explorers show it: the digest reversed.
show_id :: (label: string, digest: [] u8) {
  shown: [32] u8;
  #assert bitcoin_reverse32(digest, shown[..]) == SHA_OK;
  show(label, shown[..]);
}
// Decode hex into out and return the filled part. Every literal here and
// in the vector tables is valid lowercase hex, so an error stops the program.
decode :: (text: string, out: [] u8) -> [] u8 {
  r := hex_decode_checked(text, out, false);
  #assert r.error == 0;
  return out[0..<r.written];
}
equal_bytes :: (a: [] u8, b: [] u8) -> bool {
  if a.count != b.count { return false; }
  for i: 0..<a.count { if a[i] != b[i] { return false; } }
  return true;
}
// A verdict: Core's reject reason, or "ok".
verdict :: (code: int) -> string { return ifx code == WITNESS_OK then "ok" else WITNESS_REASONS[code]; }
// A case's name, and whether segwit is active for it.
named :: (c: WtCase) {
  print("  %", c.name);
  if !c.segwit { print(" (segwit not active)"); }
}
// Where the commitment is.
where :: (commitment: int) {
  if commitment == WITNESS_NONE { print("no commitment"); } else { print("commitment at output %", commitment); }
}

main :: () {
  b: [6000] u8; wids: [32 * 20] u8; ids: [32 * 20] u8; work: [32 * 20] u8;
  root: [32] u8; d: [32] u8; script: [38] u8;
  ins: [4] TxIn; outs: [4] TxOut; items: [8] TxItem;

  // Block 514,999: where the commitment is and what it commits to.
  s := decode(MK_BLOCK_514999, b[..]);
  k := witness_check_vartime(s, true, ins[..], outs[..], items[..], wids[..], work[..]);
  print("block 514,999: % transactions\n", k.count);
  print("  with witnesses:");
  at := k.coinbase;
  for n: 0..<k.count {
    t := tx_parse_vartime(s[at..<s.count], true, ins[..], outs[..], items[..]);
    if t.segwit { print(" %", n); }
    at += t.total;
  }
  print("\n");
  cb := s[k.coinbase..<s.count];
  t := tx_parse_vartime(cb, true, ins[..], outs[..], items[..]);
  print("  coinbase outputs: %, ", t.nout);
  where(k.commitment);
  print("\n");
  o := outs[k.commitment];
  show("  its script", cb[o.script..<o.script+o.script_n]);
  print("  coinbase witness: % item of % bytes\n", ins[0].count, items[ins[0].first].n);
  reserved := s[k.reserved..<k.reserved+32];
  show("  reserved value", reserved);
  #assert merkle_witness_root_vartime(wids[0..<32*k.count], work[..], root[..]) == MERKLE_OK;
  show_id("  witness root", root[..]);
  #assert witness_commitment_hash_vartime(root[..], reserved, d[..]);
  show("  SHA-256d(root || reserved)", d[..]);
  print("  check: %\n", verdict(k.code));
  zero: [32] u8;
  #assert witness_commitment_script_vartime(root[..], zero[..], script[..]) == WITNESS_MINIMUM;
  print("  the script GenerateCoinbaseCommitment writes is the block's: %\n", equal_bytes(script[..], cb[o.script..<o.script+o.script_n]));

  // The cases: CheckWitnessMalleation on each.
  print("CheckWitnessMalleation\n");
  for i: 0..<WT_CASES.count {
    c := WT_CASES[i];
    x := decode(c.hex, b[..]);
    k = witness_check_vartime(x, c.segwit, ins[..], outs[..], items[..], wids[..], work[..]);
    named(c); print(": ");
    where(k.commitment);
    print(", %\n", verdict(k.code));
  }

  // A witness changed after the block was mined: the txids, the header
  // and the block hash stay; one wtxid and the witness root move.
  print("a witness byte changed\n");
  s = decode(WT_CASES[3].hex, b[..]);
  k = witness_mutated_vartime(s, true, ins[..], outs[..], items[..], ids[..], wids[..], work[..]);
  before: [32 * 3] u8; was: [32 * 3] u8;
  for i: 0..<96 { before[i] = ids[i]; was[i] = wids[i]; }
  hash: [32] u8;
  #assert sha256d_checked(s[0..<80], hash[..], false) == SHA_OK;
  x := decode(WT_CASES[6].hex, b[..]);
  k = witness_mutated_vartime(x, true, ins[..], outs[..], items[..], ids[..], wids[..], work[..]);
  #assert sha256d_checked(x[0..<80], d[..], false) == SHA_OK;
  print("  same txids: %, same block hash: %\n", equal_bytes(before[..], ids[0..<96]), equal_bytes(hash[..], d[..]));
  show_id("  wtxid of transaction 1 before", was[32..<64]);
  show_id("  wtxid of transaction 1 after", wids[32..<64]);

  // IsBlockMutated, as a node runs it on a block it rebuilt.
  print("IsBlockMutated\n");
  for i: 0..<WT_MUTATED.count {
    c := WT_MUTATED[i];
    y := decode(c.hex, b[..]);
    k = witness_mutated_vartime(y, c.segwit, ins[..], outs[..], items[..], ids[..], wids[..], work[..]);
    named(c);
    if k.code == WITNESS_OK { print(": not mutated\n"); } else { print(": mutated, %\n", verdict(k.code)); }
  }
}
```

Run it:

```shell
coax/bin/coax run bitcoin/examples/witness-commitment.coax
```

The output, which the example's fixture
[bitcoin/fixtures/witness-commitment.out](../fixtures/witness-commitment.out)
holds, is the worked examples above in order: 39 lines.

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/witness-commitment.coax](../examples/checks/witness-commitment.coax),
which loads the example:

- on every case, `IsBlockMutated` says what `CheckWitnessMalleation`
  says, since every case starts with a coinbase and has the right root
- a reserved value of 7, written by `blocktools.py` as a little-endian
  256-bit number, commits: the script recomputed from it is the coinbase's
- the coinbase's wtxid never enters the witness root: a changed reserved
  value moves the coinbase's wtxid but not the root, and fails the match

<!-- source: bitcoin/examples/checks/witness-commitment.coax check -->
```jai
// Every case: IsBlockMutated says what CheckWitnessMalleation says, since
// each case's header carries its own root and starts with a coinbase.
#forall "the cases are only witness cases" i: 0..<WT_CASES.count #steps 300000000 {
  c := WT_CASES[i];
  b: [6000] u8; ids: [32 * 20] u8; wids: [32 * 20] u8; work: [32 * 20] u8;
  ins: [4] TxIn; outs: [4] TxOut; items: [8] TxItem;
  s := decode(c.hex, b[..]);
  k := witness_check_vartime(s, c.segwit, ins[..], outs[..], items[..], wids[..], work[..]);
  m := witness_mutated_vartime(s, c.segwit, ins[..], outs[..], items[..], ids[..], wids[..], work[..]);
  #assert k.code == m.code;
}
// "reserved value 7": blocktools.py writes the value as a 256-bit number,
// little-endian, and the commitment Core's rule recomputes from it is the
// one in the coinbase, byte for byte.
#test "any reserved value commits" #steps 300000000 {
  b: [6000] u8; wids: [32 * 20] u8; work: [32 * 20] u8; root: [32] u8; script: [38] u8; seven: [32] u8;
  ins: [4] TxIn; outs: [4] TxOut; items: [8] TxItem;
  s := decode(WT_CASES[5].hex, b[..]);
  k := witness_check_vartime(s, true, ins[..], outs[..], items[..], wids[..], work[..]);
  #assert k.code == WITNESS_OK;
  seven[0] = 7;
  #assert equal_bytes(s[k.reserved..<k.reserved+32], seven[..]);
  #assert merkle_witness_root_vartime(wids[0..<32*k.count], work[..], root[..]) == MERKLE_OK;
  #assert witness_commitment_script_vartime(root[..], seven[..], script[..]) == WITNESS_MINIMUM;
  cb := s[k.coinbase..<s.count];
  t := tx_parse_vartime(cb, true, ins[..], outs[..], items[..]);
  o := outs[k.commitment];
  #assert equal_bytes(script[..], cb[o.script..<o.script+o.script_n]);
}
// The coinbase's own wtxid never enters the witness root: changing its
// reserved value moves its wtxid but not the root.
#test "the coinbase's wtxid is zero in the tree" #steps 300000000 {
  b: [6000] u8; wids: [32 * 20] u8; work: [32 * 20] u8; one: [32] u8; two: [32] u8; first: [32] u8;
  ins: [4] TxIn; outs: [4] TxOut; items: [8] TxItem;
  s := decode(WT_CASES[3].hex, b[..]);
  k := witness_check_vartime(s, true, ins[..], outs[..], items[..], wids[..], work[..]);
  #assert merkle_witness_root_vartime(wids[0..<32*k.count], work[..], one[..]) == MERKLE_OK;
  for i: 0..<32 { first[i] = wids[i]; }
  s[k.reserved] = 1;
  k = witness_check_vartime(s, true, ins[..], outs[..], items[..], wids[..], work[..]);
  #assert merkle_witness_root_vartime(wids[0..<32*k.count], work[..], two[..]) == MERKLE_OK;
  #assert equal_bytes(one[..], two[..]) && !equal_bytes(first[..], wids[0..<32]);
  #assert k.code == WITNESS_MERKLE_MATCH;
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/witness-commitment.coax
```

`coax test` runs 93 checks, including the loaded files' own checks, in
about 3 seconds on 2026-11-03.

### The library's own checks

`witness.coax` is an `#authority` file. Its checks run every case of
`CheckWitnessMalleation` and `IsBlockMutated` against Core's verdict and
commitment position, compare block 514,999's commitment script with the
one Core would write for its witness root, refuse views that are too
small, a truncated block and a block with trailing bytes, pass an empty
block, and find a block that starts with a 64-byte transaction instead of
a coinbase mutated, and one with a 65-byte transaction not mutated.

```shell
coax/bin/coax test coax/lib/bitcoin/witness.coax
```

`coax test` runs 87 checks, including those of the files it loads, in
about 2 seconds on 2026-11-03. Mutation testing: not run (owner speed
order).

### Bitcoin Core's rules and functional-test framework

[make_witness_vectors.py](../../coax/tests/bitcoin/make_witness_vectors.py)
builds every case with Bitcoin Core's `test_framework/messages.py`, which
reads and writes the blocks, and `add_witness_commitment` and
`get_witness_script`, read out of the pinned
`test_framework/blocktools.py` and run as written. It decides each case
with `GetWitnessCommitmentIndex`, `CheckWitnessMalleation` and
`IsBlockMutated` transliterated from the pinned `consensus/validation.h`
and `validation.cpp`, with chapter 46's `merkle_reference.py` for the
mutated flag, and writes the blocks and verdicts into
[witness_vectors.coax](../../coax/tests/bitcoin/witness_vectors.coax).

[witness.py](../../coax/tests/bitcoin/witness.py) computes every line the
example prints from those blocks, read back with `messages.py`, and
compares the transcript with the fixture:

```shell
python3 -I coax/tests/bitcoin/witness.py --coax coax/bin/coax
```

On 2026-11-03 all 39 lines agree, and the example's output in native
debug and release builds equals them. Wasm was not run.

## What this chapter leaves out

- The weight limit and the other block rules that run after these:
  chapters 53 and 57.
- Segwit's activation height and how a node decides it is active for a
  block: chapter 51.
- Signet's use of the bytes after the 38th: chapter 50.
- Building a block template with the commitment: chapter 88.
- Marking a block invalid, and when a failure is cached: chapters 59 and
  60.

## Summary

- The commitment is the last coinbase output whose script starts
  `6a24aa21a9ed` and has at least 38 bytes; bytes 6 to 38 are
  SHA-256d(witness root || reserved value).
- The witness root is the tree of wtxids with the coinbase's taken as zero,
  and the reserved value is the coinbase's one 32-byte witness item.
- Core refuses a commitment without exactly one 32-byte item as
  `bad-witness-nonce-size`, a wrong hash as `bad-witness-merkle-match`, and
  any witness in a block that doesn't commit, or before segwit, as
  `unexpected-witness`.
- A witness can change without changing the header, so these failures mean
  mutated, not invalid. `IsBlockMutated` checks the root and then these
  rules before a node trusts a block's hash.

## Dependencies

- [35. Txids, wtxids and malleability](ch035-txids-wtxids-and-malleability.md)
- [46. Merkle trees and the duplicate-leaf flaw](ch046-merkle-trees-and-the-duplicate-leaf-flaw.md)

## Sources

- [BIP-141](https://github.com/bitcoin/bips/blob/master/bip-0141.mediawiki),
  pinned in
  [coax/tests/vectors/upstream/bip141/](../../coax/tests/vectors/upstream/bip141/bip-0141.mediawiki)
  at revision `927b6de9`.
- Bitcoin Core at revision `ddea9196`: `consensus/validation.h`
  (`GetWitnessCommitmentIndex`, `MINIMUM_WITNESS_COMMITMENT`) and
  `test_framework/blocktools.py` (`add_witness_commitment`,
  `get_witness_script`), pinned in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/consensus_validation.h)
  by this chapter; `validation.cpp` (`CheckWitnessMalleation`,
  `IsBlockMutated`, `GenerateCoinbaseCommitment`,
  `UpdateUncommittedBlockStructures`, `ContextualCheckBlock`) and
  `test_framework/messages.py`, pinned there by chapters 46 and 65.
- Mainnet blocks 100,000 and 514,999, pinned in
  [coax/tests/vectors/upstream/mainnet/](../../coax/tests/vectors/upstream/mainnet/)
  by chapter 46.
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
