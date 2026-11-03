# 46. Merkle trees and the duplicate-leaf flaw

A block header is 80 bytes, and one of its fields, the Merkle root,
commits to every transaction in the block. The root is the top of a binary
tree of SHA-256d hashes whose leaves are the txids. A node checks a block
by rebuilding that root from the transactions it received. A light client
checks that one transaction is in a block with about `log2(n)` hashes, a
Merkle branch, instead of the whole block.

Bitcoin's tree has a quirk: a level with an odd number of hashes pairs its
last hash with itself. That rule lets two different lists of transactions
have the same root, which is CVE-2012-2459. A second ambiguity comes from
the hash itself: an inner node hashes 64 bytes, and a transaction of 64
bytes hashes the same way. This chapter builds Bitcoin Core's tree, its
detector for the first flaw, branches and proofs, the witness root and a
reader for a block's txids. It checks them on three mainnet blocks, Bitcoin
Core's own Merkle tests and a Python transliteration of Core, natively and
in Wasm.

## What this chapter teaches

- How Bitcoin Core's `ComputeMerkleRoot` builds the root, and why an odd
  level pairs its last hash with itself.
- CVE-2012-2459: why repeating trailing transactions keeps the root, how
  Core's `mutated` flag detects it, and why a node must refuse such a
  block without marking its header invalid.
- The 64-byte transaction: why it is also an inner node, what that lets
  an attacker do to a light client, and what BIP-54 proposes. What is
  consensus today and what is only proposed.
- Merkle branches: how to build one, how to verify one, and why the
  verifier must know how many transactions the block has.
- The witness root over wtxids, with the coinbase's wtxid taken as zero.
- Nine result codes, with Core's reject reason for each that has one.

## Where Bitcoin uses Merkle trees

| Where | What the tree holds | Chapter |
| --- | --- | --- |
| The block header's `hashMerkleRoot` | the txids, coinbase first | this chapter, 47 |
| The witness commitment in the coinbase | the wtxids, the coinbase's as zero | this chapter, 49 |
| Block validation: "bad-txnmrklroot" and "bad-txns-duplicate" | the txids | this chapter, 57 |
| Light-client proofs (SPV) | a branch from one txid to the root | this chapter |
| BIP-37 `merkleblock` messages | a partial tree of several branches | left out of this book |
| Compact block filters | the header chain that a filter header follows | 74 |

Taproot's script trees (chapter 42) are also Merkle trees, but a different
kind: tagged hashes, sorted pairs and no duplication. Nothing here applies
to them.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [merkle.coax](../../coax/lib/bitcoin/merkle.coax) | `merkle_parent_vartime(left, right, out)` | one inner node: SHA-256d of 64 bytes |
| | `merkle_root_vartime(leaves, work, out)` | Core's `ComputeMerkleRoot`: the root and the `mutated` flag |
| | `merkle_pairs_equal_vartime`, `merkle_level_vartime` | the two halves of one level: the mutation test and the hashing |
| | `merkle_check_vartime(leaves, work, header_root)` | Core's `CheckMerkleRoot`, its two refusals in its order |
| | `merkle_witness_root_vartime(wtxids, work, out)` | Core's `BlockWitnessMerkleRoot` |
| | `merkle_depth_vartime(n)`, `merkle_branch_vartime(leaves, index, work, branch)` | a tree's depth, and Core's `TransactionMerklePath` |
| | `merkle_branch_root_vartime(leaf, branch, index, out)` | the root a branch gives |
| | `merkle_verify_vartime(leaf, branch, index, count, root)` | an SPV proof against a block of `count` transactions |
| | `merkle_block_ids_vartime(b, bip54, ...)` | the txids and wtxids of a serialized block |
| [tx.coax](../../coax/lib/bitcoin/tx.coax) | `tx_parse_vartime`, `tx_txid_vartime`, `tx_wtxid_vartime` | each transaction in the block, and its IDs (chapter 34) |
| [hash.coax](../../coax/lib/bitcoin/hash.coax) | `sha256d_checked`, `bitcoin_reverse32` | SHA-256d and the display order (chapter 5) |

Every hash here is public, so every procedure runs in variable time and its
name ends in `_vartime` (chapter 3). Leaves are 32-byte hashes in hash
order, packed one after another in a byte view. A procedure that builds
levels takes a `work` view at least as long as the leaves and hashes in
place there, so nothing is allocated.

Every procedure returns one of these codes:

<!-- source: coax/lib/bitcoin/merkle.coax codes -->
```jai
// Result codes. Core's reject reason, where there is one, is in quotes.
MERKLE_OK :: 0;
MERKLE_EMPTY :: 1;      // no leaves: the root is zero; a block with none is "bad-blk-length"
MERKLE_MUTATED :: 2;    // two equal hashes paired at some level: "bad-txns-duplicate" (CVE-2012-2459)
MERKLE_MISMATCH :: 3;   // the root is not the one the header or the proof names: "bad-txnmrklroot"
MERKLE_CAPACITY :: 4;   // a view is not whole hashes, or is too short for the result
MERKLE_BAD_INDEX :: 5;  // a proof's position is not a leaf of a tree that size
MERKLE_BAD_DEPTH :: 6;  // a proof's branch is not as long as a tree that size is deep
MERKLE_TX_64 :: 7;      // a transaction of 64 stripped bytes, under BIP-54's proposed rule
MERKLE_BAD_BLOCK :: 8;  // the block's bytes don't read; the transaction reader's code says why

MERKLE_HASH :: 32;
```

## The tree

The leaves are the block's txids in block order, coinbase first, each in
hash order (the bytes SHA-256d produced, not the reversed form explorers
show). Each level pairs hashes 0 and 1, 2 and 3, and so on, and replaces
each pair with SHA-256d of the 64 bytes `left || right`. The level above has
half as many hashes. The last level has one hash: the root.

<!-- source: coax/lib/bitcoin/merkle.coax parent -->
```jai
// One inner node: SHA-256d of the left child's 32 bytes, then the right's,
// into out. out may be either child: both are copied first.
merkle_parent_vartime :: (left: [] u8, right: [] u8, out: [] u8) -> bool #export_c {
  if left.count != MERKLE_HASH || right.count != MERKLE_HASH || out.count != MERKLE_HASH { return false; }
  pair: [64] u8;
  for i: 0..<32 { pair[i] = left[i]; pair[32+i] = right[i]; }
  #assert sha256d_checked(pair[..], out, false) == SHA_OK;
  return true;
}
```

When a level has an odd number of hashes, the last one has no partner.
Bitcoin pairs it with itself: the parent is SHA-256d of the hash written
twice. Core does this by appending a copy of the last hash before it
hashes the level. A tree of one transaction has no levels above it, and
its root is the txid itself.

<!-- source: coax/lib/bitcoin/merkle.coax root -->
```jai
// A root and whether two equal hashes were paired on the way up.
MerkleRoot :: struct { code: int; mutated: bool; }
// Core's mutation test for one level of n hashes: is any pair (0, 1),
// (2, 3), ... two equal hashes? An odd level's last hash is not compared:
// pairing it with itself is the rule, not a mutation.
merkle_pairs_equal_vartime :: (work: [] u8, n: int) -> bool #export_c {
  found := false;
  for pos: 0..<n/2 {
    same := true;
    for i: 0..<32 { if work[64*pos+i] != work[64*pos+32+i] { same = false; } }
    if same { found = true; }
  }
  return found;
}
// One level of the tree, in place: work holds n hashes; afterwards its
// first (n + 1) / 2 hashes are their parents.
merkle_level_vartime :: (work: [] u8, n: int) {
  for k: 0..<(n+1)/2 {
    right := ifx 2*k+1 < n then 2*k+1 else 2*k;    // an odd level's last hash pairs with itself
    #assert merkle_parent_vartime(work[64*k..<64*k+32], work[32*right..<32*right+32], work[32*k..<32*k+32]);
  }
}
// ComputeMerkleRoot: the root of the leaves (32-byte hashes, one after
// another) into out, using work, which holds at least as many bytes as
// the leaves. No leaves is MERKLE_EMPTY with a root of zero, as in Core.
// mutated is Core's flag: some level paired two equal hashes.
merkle_root_vartime :: (leaves: [] u8, work: [] u8, out: [] u8) -> MerkleRoot #export_c {
  if leaves.count % MERKLE_HASH != 0 || work.count < leaves.count || out.count != MERKLE_HASH {
    return MerkleRoot.{code=MERKLE_CAPACITY};
  }
  n := leaves.count / MERKLE_HASH;
  if n == 0 {
    for i: 0..<32 { out[i] = 0; }
    return MerkleRoot.{code=MERKLE_EMPTY};
  }
  for i: 0..<leaves.count { work[i] = leaves[i]; }
  mutated := false;
  while n > 1 {
    if merkle_pairs_equal_vartime(work, n) { mutated = true; }
    merkle_level_vartime(work, n);
    n = (n + 1) / 2;
  }
  for i: 0..<32 { out[i] = work[i]; }
  return MerkleRoot.{code=MERKLE_OK, mutated=mutated};
}
```

The level is hashed in place: parent `k` goes to slot `k`, which is at or
before slot `2k`, the first child it reads, and `merkle_parent_vartime`
copies both children before it writes. Core's `SHA256D64` hashes the
whole level in one call with the same layout.

### Worked example: block 100,000

Block 100,000 has four transactions, so the tree has two levels above the
leaves. The example prints the first three bytes of each hash, as
explorers show them:

```text
  level 0: 8c14f0 fff252 6359f0 e9a668
  level 1: ccdafb 8e3089
  level 2: f3e947
root: f3e94742aca4b5ef85488dc37c06c3282295ffec960994b2c0d5ac2a25a95766
header's root: f3e94742aca4b5ef85488dc37c06c3282295ffec960994b2c0d5ac2a25a95766
```

`ccdafb…` is SHA-256d of the first two txids, `8e3089…` of the last two,
and the root of those two. The header's bytes 36 to 68 hold the root, and
it is the root mempool.space publishes for the block.

### Worked example: an odd count

Block 514,999 has 19 transactions. Its levels have 19, 10, 5, 3, 2 and 1
hashes, so three of them pair their last hash with itself:

```text
  level 0: f4bf80 b49af2 797d67 … e9a028 d041bb  (odd: the last pairs with itself)
  level 1: fba5b6 6d0718 aae49b … 5fc5de 1e581e
  level 2: 9d6741 5b7cfa 55bedf 64a0d8 be534d  (odd: the last pairs with itself)
  level 3: 8ac33a 0e75c1 ed5e8a  (odd: the last pairs with itself)
  level 4: 55f9aa 1f6201
  level 5: a9cd74
```

The depth of a tree of `n` leaves, the number of levels above them, is
`ceil(log2(n))`: 5 for 19 transactions, 11 for the 1,557 of block 413,567.
`merkle_depth_vartime` computes it by halving and rounding up.

## CVE-2012-2459: two lists, one root

Pairing the last hash with itself makes the tree ambiguous. Take six
leaves. Level 1 has three hashes, so the third is paired with itself. Now
take eight leaves: the same six, then leaves 5 and 6 again. Level 1 has
four hashes, and the fourth equals the third, so level 2 is the same as
before, and so is the root. The comment above Core's `ComputeMerkleRoot`
draws it:

```text
                A               A
              /  \            /   \
            B     C         B       C
           / \    |        / \     / \
          D   E   F       D   E   F   F
         / \ / \ / \     / \ / \ / \ / \
         1 2 3 4 5 6     1 2 3 4 5 6 5 6
```

The example runs this on the first six txids of block 514,999:

```text
  level 0: f4bf80 b49af2 797d67 f9aac6 1822a1 5f377f
  level 1: fba5b6 6d0718 aae49b  (odd: the last pairs with itself)
  level 2: 9d6741 c9f27c
  level 3: 23ecb9
  root of 1, 2, 3, 4, 5, 6: 23ecb979e55aa39a0fb4aa7ef6f90cb2594c0f26c554376f3fcdacc5dce76e7a
  mutated: false
  level 0: f4bf80 b49af2 797d67 f9aac6 1822a1 5f377f 1822a1 5f377f
  level 1: fba5b6 6d0718 aae49b aae49b
  two equal hashes paired here
  level 2: 9d6741 c9f27c
  level 3: 23ecb9
  root of 1, 2, 3, 4, 5, 6, 5, 6: 23ecb979e55aa39a0fb4aa7ef6f90cb2594c0f26c554376f3fcdacc5dce76e7a
  mutated: true, same root: true
```

In general, repeating the last `2^k` transactions of a block whose count
is a multiple of `2^k` but not of `2^(k+1)` keeps the root. Core's
`merkle_test` builds exactly that, up to three times in a row. A count
that is a power of two cannot be extended this way: repeating all of it
adds a level and changes the root.

### Why it matters

The block hash is the hash of the header, and the header holds only the
root. So the honest block and the one with repeated transactions have the
same block hash. The second one is invalid: a transaction twice in one
block spends the same coins twice. Before the fix, a node that received
the mutated copy first found it invalid and remembered the block hash as
invalid. When the honest block arrived, with the same hash, the node
refused it without looking. An attacker could split nodes off the chain
this way by sending them a mutated copy of each new block.

### The detector

Core can't tell the two lists apart by the root, so it looks at how the
tree is built. A mutation always puts two equal hashes in one pair at some
level: at the leaves when one transaction is repeated, higher up when a
run of them is. `ComputeMerkleRoot` compares every pair `(0, 1)`, `(2, 3)`,
… at every level, before it pairs an odd level's last hash with itself,
and sets `mutated` if any pair is equal. `merkle_pairs_equal_vartime` is
that comparison. The last hash of an odd level is not compared with
anything: pairing it with itself is the rule, not a mutation.

Core's flag is broader than the attack. It also catches equal hashes in a
pair that is not at the end, such as leaves 1, 1, 3, 4. No valid block
has two equal txids, so nothing valid is lost. An older form of the
computation, kept in `merkle_tests.cpp` for comparison, looked only at the
last pair of each level; the test file shows that the two agree on every
trailing repetition, and `merkle_test_mutated_return_value` checks the
non-trailing case.

`CheckMerkleRoot` in `validation.cpp` uses the root and the flag in this
order:

<!-- source: coax/lib/bitcoin/merkle.coax check -->
```jai
// CheckMerkleRoot: the leaves (a block's txids) against the root in the
// block's header, in Core's order: a different root is "bad-txnmrklroot",
// then a mutated tree is "bad-txns-duplicate". Core's CheckBlock then
// refuses a block with no transactions; that is MERKLE_EMPTY here.
merkle_check_vartime :: (leaves: [] u8, work: [] u8, header_root: [] u8) -> int #export_c {
  if header_root.count != MERKLE_HASH { return MERKLE_CAPACITY; }
  root: [32] u8;
  r := merkle_root_vartime(leaves, work, root[..]);
  if r.code == MERKLE_CAPACITY { return MERKLE_CAPACITY; }
  for i: 0..<32 { if root[i] != header_root[i] { return MERKLE_MISMATCH; } }
  if r.mutated { return MERKLE_MUTATED; }
  return r.code;
}
```

Both refusals have the result `BLOCK_MUTATED` in Core, not
`BLOCK_CONSENSUS`, and that result changes what the node does next:

- `InvalidBlockFound` doesn't set `BLOCK_FAILED_VALID` on the block's index
  for a mutated block, and `ActivateBestChainStep` doesn't mark the chain
  invalid. The header stays a candidate.
- When a peer sends a block, `net_processing.cpp` calls `IsBlockMutated`
  before anything else. A mutated block costs the peer
  (`Misbehaving(peer, "mutated block")`), the request is dropped, and the
  block can be fetched again from another peer.

That is the rule for our node too: `MERKLE_MUTATED` and `MERKLE_MISMATCH`
say that these bytes are not the block the header names. They say nothing
about the header. A node that gets either one drops the bytes, may punish
the peer that sent them and downloads the block again. It never records
the block hash as invalid.

### Worked example: a mutated mainnet block

The example takes block 514,999, appends a second copy of its last
transaction and raises the count from 19 to 20. The header is untouched:

```text
block 514,999 with its last transaction twice: ok, 20 transactions, 5418 bytes
  same header, same block hash: true
  check: mutated
```

The bytes read as a block, every txid is computed, the root equals the
header's, and the check says `mutated`. Core's answer is
"bad-txns-duplicate".

## The 64-byte transaction

An inner node is SHA-256d of 64 bytes. A txid is SHA-256d of a
transaction's stripped serialization. If a transaction's stripped
serialization is 64 bytes long, its txid is also the parent of two
32-byte "hashes": its first and last 32 bytes. Nothing in the tree says
which reading is meant.

The smallest transaction with one input and one output is 60 bytes plus
its two scripts, so a transaction whose scripts total 4 bytes is 64 bytes.
The example builds one: block 100,000's coinbase txid as the outpoint, an
empty scriptSig, and an output of 0 satoshis whose script is `OP_RETURN`
and a 2-byte push.

```text
a 64-byte transaction
  reads: true, stripped size 64
  its txid is the parent of its two halves: true
```

### What an attacker gets

Put that transaction `T` in a block as transaction 1, after a coinbase. Its
honest proof is leaf `T`'s txid at index 1 with the coinbase's txid as the
branch. A forged proof treats `T` as an inner node: the leaf is `T`'s last
32 bytes, at index 3, and the branch is `T`'s first 32 bytes and then the
coinbase's txid. It reaches the same root:

```text
  a block with it: ok, the 64-byte one is transaction 1
  proof of transaction 1: ok
  the right half as leaf 3, one level deeper, reaches the root: true
  proof with the true count of 2: bad index
  proof with a claimed count of 4: ok
```

The forged leaf is just 32 bytes of `T`. To fool a light client, an
attacker must also make those 32 bytes the txid of a transaction of its
own, one that pays the victim. That needs a mined 64-byte transaction
whose last 32 bytes the attacker chose by brute force over the fields it
controls; its cost is analysed in the 2019 paper "Weaknesses in Bitcoin's
Merkle Root Construction", which BIP-54 and Core cite. It is
CVE-2017-12842. The other direction works too: an inner node whose 64
bytes happen to parse as a transaction can be presented as one, which
makes a different transaction list with the same root.

### What is consensus today

At the pinned Bitcoin Core revision (`ddea9196`, a development build
numbered 32.99), a 64-byte transaction is valid by consensus. Core defends
against it in three places that are not consensus rules:

- Policy. `MIN_STANDARD_TX_NONWITNESS_SIZE` is 65 in `policy/policy.h`:
  the mempool refuses a transaction of fewer than 65 stripped bytes as
  "tx-size-small", "to mitigate CVE-2017-12842". BIP-54 states that Core
  0.16.1 and later neither relay nor mine 64-byte transactions.
- `IsBlockMutated` in `validation.cpp`: a block whose first transaction
  is not a coinbase and that holds a 64-byte transaction is treated as
  mutated. Its comment says this is not a consensus change, because such a
  block is invalid anyway.
- The mutated flag above, which catches the CVE-2012-2459 form only.

BIP-54, "Consensus Cleanup" (the Great Consensus Cleanup proposal), adds
the consensus rule: "Transactions whose witness-stripped serialized size is
exactly 64 bytes are invalid." It is a proposed soft fork. The BIP text,
pinned at bips commit `927b6de9` (2026-10-02), has status Complete and
specifies the rules, but no deployment parameters. Core's implementation,
pull request 35793 "Implement BIP 54 (Consensus Cleanup) without mainnet
activation", was open on 2026-11-02, and the pinned revision has no 64-byte
consensus rule. What the pinned Core already does is forward-compatible
with BIP-54: its block templates exclude 64-byte transactions, and its
miner sets the coinbase's `nLockTime` to the height minus one and its
`nSequence` below `0xffffffff`, as BIP-54's coinbase rule requires.

`merkle_block_ids_vartime` follows consensus by default: a 64-byte
transaction reads, and the result names the first one in `first64`. With
`bip54` set, it refuses with `MERKLE_TX_64` at the transaction's first
byte, which is what a node enforcing BIP-54 would do after activation. Our
node (chapter 57) leaves the option off until the rule is activated.

### What a light client does about it

Until BIP-54 is active, a verifier of Merkle proofs needs a workaround.
BIP-54 names three: ask for a proof of the coinbase as well, whose branch
length is the tree's depth; refuse a proof in which an inner node is also
a valid transaction serialization; or carry inner nodes as single SHA-256
of their preimages. `merkle_verify_vartime` takes the first approach's
result as an input. It refuses a branch whose length is not the depth of a
tree of `count` leaves, and an index not below `count`. The forged proof
above is one level too deep, so the true count of 2 refuses it. A claimed
count of 4 accepts it, which is the point: the count is not in the header,
and a peer can lie about it. The caller must take it from a source it
trusts, such as a verified coinbase branch.

## Merkle branches

A branch for leaf `i` is its sibling at each level, from the leaves up:
`depth` hashes. With the leaf, the branch and the index, anyone can
rebuild the root. Bit `d` of the index says whether the node at level `d`
is a right child (1) or a left one (0). At the end of an odd level, the
sibling is the node itself.

<!-- source: coax/lib/bitcoin/merkle.coax branch -->
```jai
// The number of levels above n leaves: how long a branch is.
merkle_depth_vartime :: (n: int) -> int #export_c {
  depth := 0; m := n;
  while m > 1 { m = (m + 1) / 2; depth += 1; }
  return depth;
}
MerkleBranch :: struct { code: int; depth: int; }
// TransactionMerklePath: the siblings of leaf index from the leaves up,
// into branch, depth hashes. At an odd level's end the sibling is the
// hash itself.
merkle_branch_vartime :: (leaves: [] u8, index: int, work: [] u8, branch: [] u8) -> MerkleBranch #export_c {
  if leaves.count % MERKLE_HASH != 0 || work.count < leaves.count { return MerkleBranch.{code=MERKLE_CAPACITY}; }
  n := leaves.count / MERKLE_HASH;
  if n == 0 { return MerkleBranch.{code=MERKLE_EMPTY}; }
  if index < 0 || index >= n { return MerkleBranch.{code=MERKLE_BAD_INDEX}; }
  depth := merkle_depth_vartime(n);
  if branch.count < depth * MERKLE_HASH { return MerkleBranch.{code=MERKLE_CAPACITY}; }
  for i: 0..<leaves.count { work[i] = leaves[i]; }
  d := 0; at := index;
  while n > 1 {
    sibling := ifx (at ^ 1) < n then at ^ 1 else n - 1;
    for i: 0..<32 { branch[32*d+i] = work[32*sibling+i]; }
    merkle_level_vartime(work, n);
    n = (n + 1) / 2; at >>= 1; d += 1;
  }
  return MerkleBranch.{code=MERKLE_OK, depth=depth};
}
```

`merkle_branch_vartime` builds the tree level by level and records the
sibling before hashing each level, which is the older
`BlockGetMerkleBranch` in `merkle_tests.cpp`. Core's `TransactionMerklePath`
uses `MerkleComputation`, which processes leaves one at a time in constant
space. The Python reference transliterates `MerkleComputation`, so the two
algorithms are checked against each other on every branch.

Verifying is the reverse walk, followed by the checks a light client needs:

<!-- source: coax/lib/bitcoin/merkle.coax proof -->
```jai
// ComputeMerkleRootFromBranch: the root a branch gives for a leaf at
// index. Each bit of index, from the lowest, says whether the node is a
// right child (1) or a left one (0); bits past the branch are ignored.
merkle_branch_root_vartime :: (leaf: [] u8, branch: [] u8, index: int, out: [] u8) -> int #export_c {
  if leaf.count != MERKLE_HASH || branch.count % MERKLE_HASH != 0 || out.count != MERKLE_HASH || index < 0 { return MERKLE_CAPACITY; }
  node: [32] u8; at := index;
  for i: 0..<32 { node[i] = leaf[i]; }
  for d: 0..<branch.count/MERKLE_HASH {
    sibling := branch[32*d..<32*d+32];
    if (at & 1) != 0 { #assert merkle_parent_vartime(sibling, node[..], node[..]); }
    else { #assert merkle_parent_vartime(node[..], sibling, node[..]); }
    at >>= 1;
  }
  for i: 0..<32 { out[i] = node[i]; }
  return MERKLE_OK;
}
// An SPV proof: leaf is transaction index of a block of count
// transactions whose header has root. The branch must be exactly as long
// as that tree is deep. Without the count, a 64-byte transaction's two
// halves pass as an inner node, and a branch one level longer or shorter
// can prove a transaction the block doesn't hold.
merkle_verify_vartime :: (leaf: [] u8, branch: [] u8, index: int, count: int, root: [] u8) -> int #export_c {
  if leaf.count != MERKLE_HASH || branch.count % MERKLE_HASH != 0 || root.count != MERKLE_HASH { return MERKLE_CAPACITY; }
  if count < 1 || index < 0 || index >= count { return MERKLE_BAD_INDEX; }
  if branch.count != merkle_depth_vartime(count) * MERKLE_HASH { return MERKLE_BAD_DEPTH; }
  got: [32] u8;
  #assert merkle_branch_root_vartime(leaf, branch, index, got[..]) == MERKLE_OK;
  for i: 0..<32 { if got[i] != root[i] { return MERKLE_MISMATCH; } }
  return MERKLE_OK;
}
```

`merkle_branch_root_vartime` is `merkle_tests.cpp`'s
`ComputeMerkleRootFromBranch`: it ignores index bits past the branch, as
Core does. `merkle_verify_vartime` adds the count. Even with the true
count, the duplication rule lets one transaction sit at several indexes:
in a tree said to have eight leaves, leaf 4 of five is also at 5, 6 and 7,
because it is paired with itself twice on the way up. That proves nothing
false, since every copy is the same transaction, but it is another reason
the count must come from a trusted source.

### Worked example: a branch in block 100,000

```text
branch for transaction 2 of block 100,000: ok, 2 hashes
  sibling: e9a66845e05d5abc0ad04ec80f774a7e585c6e8db975962d069a522137b80c1d
  sibling: ccdafb73d8dcd0173d5d5c3c9a0770d0b3953db889dab99ef05b1907518cb815
  proof: ok
  as transaction 3: mismatch
  with one hash left off: bad depth
branch for transaction 1000 of block 413,567: 11 hashes, proof ok
```

Transaction 2's sibling at the leaves is transaction 3, and at level 1 it
is the parent of transactions 0 and 1 (`ccdafb…`, the first hash of level
1 above).

### BIP-37 partial Merkle trees

A BIP-37 `merkleblock` message proves several transactions of one block at
once. It sends the transaction count, the hashes a traversal needs and one
flag bit per node visited, and the receiver rebuilds the root
(`CPartialMerkleTree` in Core's `merkleblock.cpp`). It refuses a node
whose two children are equal, the same defense as the mutated flag. The
message exists for bloom-filter clients, and this book leaves the BIP-37
service out (see the plan). Light clients in this book use compact block
filters instead, in chapter 74.

## The witness root

Segwit adds a second tree over the wtxids. The coinbase's wtxid can't be
in it: the coinbase holds the commitment to this tree, so its wtxid
depends on the root. BIP-141 takes the coinbase's wtxid as 32 zero bytes.

<!-- source: coax/lib/bitcoin/merkle.coax witness -->
```jai
// BlockWitnessMerkleRoot: the root of a block's wtxids, with the
// coinbase's (the first) taken as 32 zero bytes. Core ignores the mutated
// flag here: the txid tree has already refused a repeated transaction.
merkle_witness_root_vartime :: (wtxids: [] u8, work: [] u8, out: [] u8) -> int #export_c {
  if wtxids.count % MERKLE_HASH != 0 || work.count < wtxids.count || out.count != MERKLE_HASH { return MERKLE_CAPACITY; }
  for i: 0..<wtxids.count { work[i] = ifx i < MERKLE_HASH then 0 else wtxids[i]; }
  return merkle_root_vartime(work[0..<wtxids.count], work, out).code;
}
```

Core computes this root without the mutated flag. A repeated transaction
repeats its txid as well as its wtxid, so the txid tree has already
refused it. For block 514,999, five of whose transactions have witnesses,
the example computes the witness root and checks it against the
coinbase's commitment, `SHA-256d(witness root || reserved value)` in an
`OP_RETURN` output that starts `aa21a9ed`:

```text
witness root: 6947cde2e7987a0f5d059432c314d504304bb5f86e3138069bacb56e44b9e411
SHA-256d(witness root, reserved value) is the coinbase's commitment: true
```

Chapter 49 builds the commitment: where it is, the reserved value, and
Core's "bad-witness-merkle-match" and "unexpected-witness" refusals.

## The txids of a block

A serialized block is the 80-byte header, a CompactSize count, and the
transactions, each in chapter 34's form with its witness. Chapter 47
reads the header. This chapter needs only the IDs:

<!-- source: coax/lib/bitcoin/merkle.coax block -->
```jai
// The transactions of a block. code is MERKLE_OK or a refusal at byte at;
// a refusal from the transaction reader keeps its TX_ code in tx_code.
// count transactions were read; first64 is the first whose stripped size
// is 64 bytes, or -1; last is where the last one starts.
MerkleBlock :: struct { code: int; at: int; tx_code: int; count: int; first64: int; last: int; }
MERKLE_HEADER :: 80;
// Reads a serialized block: the 80-byte header, a CompactSize count, then
// each transaction as chapter 34 reads it, with witnesses allowed, and
// nothing after. Writes each txid into txids and each wtxid into wtxids
// (hash order, 32 bytes each). With bip54, a transaction of 64 stripped
// bytes refuses (MERKLE_TX_64); that rule is proposed, not consensus.
merkle_block_ids_vartime :: (b: [] u8, bip54: bool, ins: [] TxIn, outs: [] TxOut, items: [] TxItem, txids: [] u8, wtxids: [] u8) -> MerkleBlock #export_c {
  if b.count < MERKLE_HEADER { return MerkleBlock.{code=MERKLE_BAD_BLOCK, tx_code=TX_TRUNCATED, first64=-1}; }
  c := tx_compact_read_vartime(b, MERKLE_HEADER);
  if c.code != TX_OK { return MerkleBlock.{code=MERKLE_BAD_BLOCK, at=c.next, tx_code=c.code, first64=-1}; }
  if txids.count < c.value * MERKLE_HASH || wtxids.count < c.value * MERKLE_HASH {
    return MerkleBlock.{code=MERKLE_CAPACITY, at=MERKLE_HEADER, first64=-1};
  }
  k := MerkleBlock.{first64=-1};
  at := c.next;
  while k.count < c.value {
    t := tx_parse_vartime(b[at..<b.count], true, ins, outs, items);
    if t.code != TX_OK {
      k.code = ifx t.code == TX_CAPACITY then MERKLE_CAPACITY else MERKLE_BAD_BLOCK;
      k.tx_code = t.code; k.at = at + t.at;
      return k;
    }
    if t.stripped == 64 {
      if bip54 { k.code = MERKLE_TX_64; k.at = at; return k; }
      if k.first64 < 0 { k.first64 = k.count; }
    }
    s := b[at..<at+t.total];
    #assert tx_txid_vartime(s, t, txids[32*k.count..<32*k.count+32]) && tx_wtxid_vartime(s, t, wtxids[32*k.count..<32*k.count+32]);
    k.last = at; k.count += 1; at += t.total;
  }
  if at != b.count { k.code = MERKLE_BAD_BLOCK; k.tx_code = TX_TRAILING; k.at = at; }
  return k;
}
```

A transaction the reader refuses keeps chapter 34's code in `tx_code`, and
`at` is the byte in the block where reading stopped. Bytes after the last
transaction refuse with `TX_TRAILING`. A block with no transactions reads,
with a count of zero; `merkle_check_vartime` then returns `MERKLE_EMPTY`,
or `MERKLE_MISMATCH` if the header's root is not zero, the order Core's
`CheckBlock` uses ("bad-txnmrklroot" first, then "bad-blk-length").

## The result codes

| Code | Core's reject reason | Returned by |
| --- | --- | --- |
| `MERKLE_OK` | none | all |
| `MERKLE_EMPTY` | "bad-blk-length" (from `CheckBlock`) | the root, the check, the witness root, the branch |
| `MERKLE_MUTATED` | "bad-txns-duplicate" | the check |
| `MERKLE_MISMATCH` | "bad-txnmrklroot"; for a proof, none | the check, the proof |
| `MERKLE_CAPACITY` | none: the caller's views | all |
| `MERKLE_BAD_INDEX` | none | the branch, the proof |
| `MERKLE_BAD_DEPTH` | none | the proof |
| `MERKLE_TX_64` | none yet: BIP-54 is proposed | the block reader, with `bip54` |
| `MERKLE_BAD_BLOCK` | the transaction reader's error | the block reader |

A refusal writes nothing to the caller's output, except that an empty tree
writes a root of 32 zero bytes, as Core returns.

## The audit

Two places in the library already touched Merkle trees:

- [nip03.coax](../../coax/lib/nostr/nip03.coax) (Nostr's OpenTimestamps
  attestations) replays a proof's operations: SHA-256, append and
  prepend. A Merkle step in such a proof is a prepend or append of the
  sibling and two SHA-256 operations, so the file has no tree code to
  replace. It returns the root the replay reached and leaves the header
  comparison to the caller, as its comment says. Like any Merkle proof
  that doesn't know the tree's depth, an OpenTimestamps proof is open to
  the 64-byte ambiguity above; the file is left as it is and the note is
  recorded here for the Nostr book.
- [sighash.coax](../../coax/lib/bitcoin/sighash.coax) takes a Taproot
  script tree's Merkle root as an input to the key tweak. Taproot's trees
  are chapter 42's subject and work differently.

Nothing else built a Bitcoin block tree, so `merkle.coax` is new. It builds
on chapter 5's SHA-256d and chapter 34's transaction reader and IDs.

## The example

The example is
[bitcoin/examples/merkle-trees.coax](../examples/merkle-trees.coax). It
prints the levels of blocks 100,000 and 514,999 and checks their roots,
computes block 514,999's witness root and its commitment, rebuilds the
root of block 413,567's 1,557 txids, runs the CVE-2012-2459 construction
and a mutated mainnet block, builds and verifies branches, and shows the
64-byte transaction, a forged proof and BIP-54's rule.

<!-- source: bitcoin/examples/merkle-trees.coax example -->
```jai
#load "../../coax/lib/bitcoin/merkle.coax";
#load "../../coax/lib/nostr/hex.coax";
#load "../../coax/tests/bitcoin/merkle_block413567.coax";
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
// Is the digest, shown reversed, this hex?
id_is :: (digest: [] u8, hex: string) -> bool {
  want: [32] u8; shown: [32] u8;
  #assert bitcoin_reverse32(digest, shown[..]) == SHA_OK;
  return equal_bytes(shown[..], decode(hex, want[..]));
}
// The name of each result code.
CODES :: [9] string.["ok", "empty", "mutated", "mismatch", "capacity", "bad index", "bad depth",
  "64-byte transaction", "bad block"];
// Print each level of the tree over n leaves, one short hash per node.
levels :: (leaves: [] u8, n: int) {
  work: [32 * 32] u8;
  for i: 0..<32*n { work[i] = leaves[i]; }
  level := 0; m := n;
  while true {
    print("  level %:", level);
    for k: 0..<m {
      shown: [32] u8;
      #assert bitcoin_reverse32(work[32*k..<32*k+32], shown[..]) == SHA_OK;
      text: [8] u8;
      r := hex_encode_checked(cast(string) shown[0..<3], text[..], false);
      print(" %", cast(string) text[0..<r.written]);
    }
    if m % 2 == 1 && m > 1 { print("  (odd: the last pairs with itself)"); }
    print("\n");
    if m == 1 { break; }
    if merkle_pairs_equal_vartime(work[..], m) { print("  two equal hashes paired here\n"); }
    merkle_level_vartime(work[..], m);
    m = (m + 1) / 2; level += 1;
  }
}

main :: () {
  b: [6000] u8; x: [6000] u8; ids: [32 * 20] u8; wids: [32 * 20] u8; work: [32 * 1557] u8;
  root: [32] u8; d: [32] u8; branch: [32 * 12] u8;
  ins: [4] TxIn; outs: [4] TxOut; items: [4] TxItem;

  // Block 100,000: four transactions, two levels.
  s := decode(MK_BLOCK_100000, b[..]);
  k := merkle_block_ids_vartime(s, false, ins[..], outs[..], items[..], ids[..], wids[..]);
  print("block 100,000: %, % transactions\n", CODES[k.code], k.count);
  for i: 0..<k.count { show_id("  txid", ids[32*i..<32*i+32]); }
  levels(ids[..], k.count);
  r := merkle_root_vartime(ids[0..<32*k.count], work[..], root[..]);
  show_id("root", root[..]);
  show_id("header's root", s[36..<68]);
  print("check: %, published root matches: %\n", CODES[merkle_check_vartime(ids[0..<32*k.count], work[..], s[36..<68])], id_is(root[..], MK_ROOT_100000));

  // Block 514,999: nineteen transactions, so odd levels.
  s = decode(MK_BLOCK_514999, b[..]);
  k = merkle_block_ids_vartime(s, false, ins[..], outs[..], items[..], ids[..], wids[..]);
  print("block 514,999: %, % transactions, depth %\n", CODES[k.code], k.count, merkle_depth_vartime(k.count));
  levels(ids[..], k.count);
  r = merkle_root_vartime(ids[0..<32*k.count], work[..], root[..]);
  print("check: %, published root matches: %\n", CODES[merkle_check_vartime(ids[0..<32*k.count], work[..], s[36..<68])], id_is(root[..], MK_ROOT_514999));
  // The witness root, with the coinbase's wtxid as zero.
  #assert merkle_witness_root_vartime(wids[0..<32*k.count], work[..], root[..]) == MERKLE_OK;
  show_id("witness root", root[..]);
  pair: [64] u8; f: [32] u8;
  for i: 0..<32 { pair[i] = root[i]; pair[32+i] = 0; }
  #assert sha256d_checked(pair[..], d[..], false) == SHA_OK;
  print("SHA-256d(witness root, reserved value) is the coinbase's commitment: %\n", equal_bytes(d[..], decode(MK_COMMITMENT_514999, f[..])));

  // Block 413,567: 1,557 transactions from Bitcoin Core's test data.
  h413567: [80] u8;
  header := decode(MK_HEADER_413567, h413567[..]);
  leaves: [32 * 1557] u8;
  big := decode(MK_LEAVES_413567, leaves[..]);
  r = merkle_root_vartime(big, work[..], root[..]);
  print("block 413,567: % transactions, depth %, mutated %\n", big.count / 32, merkle_depth_vartime(MK_COUNT_413567), r.mutated);
  show_id("  root", root[..]);
  print("check: %, published root matches: %\n", CODES[merkle_check_vartime(big, work[..], header[36..<68])], id_is(root[..], MK_ROOT_413567));

  // CVE-2012-2459: Core's construction, on block 514,999's first six
  // txids, then the same six with the last two repeated.
  print("CVE-2012-2459\n");
  s = decode(MK_BLOCK_514999, b[..]);
  k = merkle_block_ids_vartime(s, false, ins[..], outs[..], items[..], ids[..], wids[..]);
  six: [32 * 8] u8;
  for i: 0..<192 { six[i] = ids[i]; }
  for i: 0..<64 { six[192+i] = ids[128+i]; }
  levels(six[0..<192], 6);
  r = merkle_root_vartime(six[0..<192], work[..], root[..]);
  show_id("  root of 1, 2, 3, 4, 5, 6", root[..]);
  print("  mutated: %\n", r.mutated);
  levels(six[..], 8);
  r = merkle_root_vartime(six[..], work[..], d[..]);
  show_id("  root of 1, 2, 3, 4, 5, 6, 5, 6", d[..]);
  print("  mutated: %, same root: %\n", r.mutated, equal_bytes(root[..], d[..]));

  // Block 514,999 with its last transaction repeated.
  for i: 0..<80 { x[i] = s[i]; }
  x[80] = 20;
  for i: 81..<s.count { x[i] = s[i]; }
  n := s.count;
  for i: k.last..<s.count { x[n] = s[i]; n += 1; }
  m := merkle_block_ids_vartime(x[0..<n], false, ins[..], outs[..], items[..], ids[..], wids[..]);
  print("block 514,999 with its last transaction twice: %, % transactions, % bytes\n", CODES[m.code], m.count, n);
  #assert sha256d_checked(x[0..<80], d[..], false) == SHA_OK;
  print("  same header, same block hash: %\n", id_is(d[..], MK_HASH_514999));
  print("  check: %\n", CODES[merkle_check_vartime(ids[0..<32*m.count], work[..], x[36..<68])]);

  // A Merkle branch: transaction 2 of block 100,000.
  s = decode(MK_BLOCK_100000, b[..]);
  k = merkle_block_ids_vartime(s, false, ins[..], outs[..], items[..], ids[..], wids[..]);
  t := merkle_branch_vartime(ids[0..<32*k.count], 2, work[..], branch[..]);
  print("branch for transaction 2 of block 100,000: %, % hashes\n", CODES[t.code], t.depth);
  for i: 0..<t.depth { show_id("  sibling", branch[32*i..<32*i+32]); }
  print("  proof: %\n", CODES[merkle_verify_vartime(ids[64..<96], branch[0..<64], 2, 4, s[36..<68])]);
  print("  as transaction 3: %\n", CODES[merkle_verify_vartime(ids[64..<96], branch[0..<64], 3, 4, s[36..<68])]);
  print("  with one hash left off: %\n", CODES[merkle_verify_vartime(ids[64..<96], branch[0..<32], 2, 4, s[36..<68])]);
  t = merkle_branch_vartime(big, 1000, work[..], branch[..]);
  print("branch for transaction 1000 of block 413,567: % hashes, proof %\n", t.depth,
    CODES[merkle_verify_vartime(big[32000..<32032], branch[0..<32*t.depth], 1000, MK_COUNT_413567, header[36..<68])]);

  // A 64-byte transaction is also an inner node.
  print("a 64-byte transaction\n");
  t64: [64] u8;
  tx := decode(MK_TX64, t64[..]);
  y := tx_read_vartime(tx, true, ins[..], outs[..], items[..]);
  print("  reads: %, stripped size %\n", y.code == TX_OK, y.stripped);
  #assert tx_txid_vartime(tx, y, d[..]);
  #assert merkle_parent_vartime(tx[0..<32], tx[32..<64], root[..]);
  print("  its txid is the parent of its two halves: %\n", equal_bytes(d[..], root[..]));
  // A block: block 100,000's header and coinbase, then the 64-byte one.
  s = decode(MK_BLOCK_100000, b[..]);
  cb := tx_parse_vartime(s[81..<s.count], true, ins[..], outs[..], items[..]);
  s[80] = 2;
  end := 81 + cb.total;
  for i: 0..<64 { s[end+i] = tx[i]; }
  blk := s[0..<end+64];
  k = merkle_block_ids_vartime(blk, false, ins[..], outs[..], items[..], ids[..], wids[..]);
  print("  a block with it: %, the 64-byte one is transaction %\n", CODES[k.code], k.first64);
  #assert merkle_root_vartime(ids[0..<64], work[..], root[..]).code == MERKLE_OK;
  // The honest proof, and a forged one that takes the right half as a leaf.
  print("  proof of transaction 1: %\n", CODES[merkle_verify_vartime(ids[32..<64], ids[0..<32], 1, 2, root[..])]);
  forged: [64] u8;
  for i: 0..<32 { forged[i] = tx[i]; forged[32+i] = ids[i]; }
  #assert merkle_branch_root_vartime(tx[32..<64], forged[..], 3, d[..]) == MERKLE_OK;
  print("  the right half as leaf 3, one level deeper, reaches the root: %\n", equal_bytes(d[..], root[..]));
  print("  proof with the true count of 2: %\n", CODES[merkle_verify_vartime(tx[32..<64], forged[..], 3, 2, root[..])]);
  print("  proof with a claimed count of 4: %\n", CODES[merkle_verify_vartime(tx[32..<64], forged[..], 3, 4, root[..])]);
  k = merkle_block_ids_vartime(blk, true, ins[..], outs[..], items[..], ids[..], wids[..]);
  print("  under BIP-54's proposed rule: % at byte %\n", CODES[k.code], k.at);
}
```

Its output:

```text
block 100,000: ok, 4 transactions
  txid: 8c14f0db3df150123e6f3dbbf30f8b955a8249b62ac1d1ff16284aefa3d06d87
  txid: fff2525b8931402dd09222c50775608f75787bd2b87e56995a7bdd30f79702c4
  txid: 6359f0868171b1d194cbee1af2f16ea598ae8fad666d9b012c8ed2b79a236ec4
  txid: e9a66845e05d5abc0ad04ec80f774a7e585c6e8db975962d069a522137b80c1d
  level 0: 8c14f0 fff252 6359f0 e9a668
  level 1: ccdafb 8e3089
  level 2: f3e947
root: f3e94742aca4b5ef85488dc37c06c3282295ffec960994b2c0d5ac2a25a95766
header's root: f3e94742aca4b5ef85488dc37c06c3282295ffec960994b2c0d5ac2a25a95766
check: ok, published root matches: true
block 514,999: ok, 19 transactions, depth 5
  level 0: f4bf80 b49af2 797d67 f9aac6 1822a1 5f377f 42a75b 7cce4a 35d129 41a7bd f7e5d7 466796 7e2fc2 6d82e8 bda87e 761172 3b544a e9a028 d041bb  (odd: the last pairs with itself)
  level 1: fba5b6 6d0718 aae49b 4f2dc3 70243d c8e1f9 a1b712 1bbc31 5fc5de 1e581e
  level 2: 9d6741 5b7cfa 55bedf 64a0d8 be534d  (odd: the last pairs with itself)
  level 3: 8ac33a 0e75c1 ed5e8a  (odd: the last pairs with itself)
  level 4: 55f9aa 1f6201
  level 5: a9cd74
check: ok, published root matches: true
witness root: 6947cde2e7987a0f5d059432c314d504304bb5f86e3138069bacb56e44b9e411
SHA-256d(witness root, reserved value) is the coinbase's commitment: true
block 413,567: 1557 transactions, depth 11, mutated false
  root: 64a50c649fc816baaa2effda230c39cacf1504e4e616a2863685b72aaa7dce05
check: ok, published root matches: true
CVE-2012-2459
  level 0: f4bf80 b49af2 797d67 f9aac6 1822a1 5f377f
  level 1: fba5b6 6d0718 aae49b  (odd: the last pairs with itself)
  level 2: 9d6741 c9f27c
  level 3: 23ecb9
  root of 1, 2, 3, 4, 5, 6: 23ecb979e55aa39a0fb4aa7ef6f90cb2594c0f26c554376f3fcdacc5dce76e7a
  mutated: false
  level 0: f4bf80 b49af2 797d67 f9aac6 1822a1 5f377f 1822a1 5f377f
  level 1: fba5b6 6d0718 aae49b aae49b
  two equal hashes paired here
  level 2: 9d6741 c9f27c
  level 3: 23ecb9
  root of 1, 2, 3, 4, 5, 6, 5, 6: 23ecb979e55aa39a0fb4aa7ef6f90cb2594c0f26c554376f3fcdacc5dce76e7a
  mutated: true, same root: true
block 514,999 with its last transaction twice: ok, 20 transactions, 5418 bytes
  same header, same block hash: true
  check: mutated
branch for transaction 2 of block 100,000: ok, 2 hashes
  sibling: e9a66845e05d5abc0ad04ec80f774a7e585c6e8db975962d069a522137b80c1d
  sibling: ccdafb73d8dcd0173d5d5c3c9a0770d0b3953db889dab99ef05b1907518cb815
  proof: ok
  as transaction 3: mismatch
  with one hash left off: bad depth
branch for transaction 1000 of block 413,567: 11 hashes, proof ok
a 64-byte transaction
  reads: true, stripped size 64
  its txid is the parent of its two halves: true
  a block with it: ok, the 64-byte one is transaction 1
  proof of transaction 1: ok
  the right half as leaf 3, one level deeper, reaches the root: true
  proof with the true count of 2: bad index
  proof with a claimed count of 4: ok
  under BIP-54's proposed rule: 64-byte transaction at byte 216
```

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/merkle-trees.out](../fixtures/merkle-trees.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/merkle-trees.coax](../examples/checks/merkle-trees.coax),
which loads the example:

- `merkle_tests.cpp`'s `merkle_test` for 0 to 16 transactions, with Core's
  transactions (version 2, no inputs or outputs, locktime `j`): up to three
  rounds of repeating trailing transactions keep the root and set the flag,
  the root is zero only for no transactions, and every branch verifies;
- `merkle_test_oneTx_block`, `merkle_test_OddTxWithRepeatedLastTx_block`
  and `merkle_test_LeftSubtreeRightSubtree`;
- blocks 100,000 and 514,999: every txid, the header's and the published
  root, and the witness commitment;
- block 413,567: the root of its 1,557 txids, the header's and the
  published one, and the header's hash;
- the example's verdicts: the branch, the 64-byte block and the forged
  proof.

<!-- source: bitcoin/examples/checks/merkle-trees.coax check -->
```jai
// The txids of merkle_tests.cpp's blocks: transaction j (below 256) has
// version 2, no inputs, no outputs and locktime j.
core_leaves :: (n: int, out: [] u8) {
  for j: 0..<n {
    tx := [10] u8.[2, 0, 0, 0, 0, 0, cast(u8) j, 0, 0, 0];
    #assert sha256d_checked(tx[..], out[32*j..<32*j+32], false) == SHA_OK;
  }
}
// The first two, as computed by merkle_reference.py.
#test "Core's test transactions" {
  leaves: [64] u8;
  core_leaves(2, leaves[..]);
  #assert id_is(leaves[0..<32], "4ebd325a4b394cff8c57e8317ccf5a8d0e2bdf1b8526f8aad6c8e43d8240621a");
  #assert id_is(leaves[32..<64], "d877b150e2f2cb183f38643fca5169da842be9c8fb841570d6fdf496bd56e829");
}
// How many trailing transactions merkle_test repeats for a count above
// zero: 2 to the number of trailing zero bits of the count.
repeat :: (n: int) -> int {
  r := 1;
  while n % (2 * r) == 0 { r *= 2; }
  return r;
}
// How many rounds of repetition merkle_test makes for 0 to 16
// transactions, as merkle.py's transliteration counts them.
ROUNDS :: [17] int.[0, 0, 0, 1, 0, 2, 1, 1, 0, 3, 2, 2, 1, 2, 1, 1, 0];
// merkle_test for 0 to 16 transactions: up to three rounds of repeating
// the last 2^ctz(count) transactions keep the root and set the mutated
// flag, and the root is zero only for no transactions. Without
// repetition, every transaction's branch rebuilds the root.
#forall "Core's merkle_test" ntx: 0..<17 #steps 250000000 {
  leaves: [32 * 64] u8; work: [32 * 64] u8; root: [32] u8; again: [32] u8; branch: [32 * 6] u8;
  core_leaves(ntx, leaves[..]);
  r := merkle_root_vartime(leaves[0..<32*ntx], work[..], root[..]);
  #assert (r.code == MERKLE_EMPTY) == (ntx == 0) && !r.mutated;
  zero := true;
  for i: 0..<32 { if root[i] != 0 { zero = false; } }
  #assert zero == (ntx == 0);
  n := ntx; rounds := 0;
  for round: 1..<4 {
    if n == 0 { break; }
    dup := repeat(n);
    if dup >= n { break; }
    rounds += 1;
    for j: 0..<dup { for i: 0..<32 { leaves[32*(n+j)+i] = leaves[32*(n+j-dup)+i]; } }
    n += dup;
    m := merkle_root_vartime(leaves[0..<32*n], work[..], again[..]);
    #assert m.code == MERKLE_OK && m.mutated;
    for i: 0..<32 { #assert again[i] == root[i]; }
    #assert merkle_check_vartime(leaves[0..<32*n], work[..], root[..]) == MERKLE_MUTATED;
  }
  #assert rounds == ROUNDS[ntx];
  for i: 0..<ntx {
    b := merkle_branch_vartime(leaves[0..<32*ntx], i, work[..], branch[..]);
    #assert b.code == MERKLE_OK && merkle_verify_vartime(leaves[32*i..<32*i+32], branch[0..<32*b.depth], i, ntx, root[..]) == MERKLE_OK;
  }
}
// merkle_test_oneTx_block, merkle_test_OddTxWithRepeatedLastTx_block and
// merkle_test_LeftSubtreeRightSubtree.
#test "Core's small blocks" #steps 100000000 {
  leaves: [32 * 4] u8; work: [32 * 4] u8; a: [32] u8; b: [32] u8; halves: [64] u8;
  core_leaves(4, leaves[..]);
  r := merkle_root_vartime(leaves[0..<32], work[..], a[..]);
  #assert r.code == MERKLE_OK && !r.mutated && equal_bytes(a[..], leaves[0..<32]);
  r = merkle_root_vartime(leaves[0..<96], work[..], a[..]);
  #assert !r.mutated;
  for i: 0..<32 { leaves[96+i] = leaves[64+i]; }
  r = merkle_root_vartime(leaves[..], work[..], b[..]);
  #assert r.mutated && equal_bytes(a[..], b[..]);
  core_leaves(4, leaves[..]);
  #assert merkle_root_vartime(leaves[0..<64], work[..], halves[0..<32]).code == MERKLE_OK;
  #assert merkle_root_vartime(leaves[64..<128], work[..], halves[32..<64]).code == MERKLE_OK;
  #assert merkle_root_vartime(halves[..], work[..], a[..]).code == MERKLE_OK;
  #assert merkle_root_vartime(leaves[..], work[..], b[..]).code == MERKLE_OK && equal_bytes(a[..], b[..]);
}
// Blocks 100,000 and 514,999: each root is the header's and the published
// one, and block 514,999's witness root reaches its commitment.
#test "mainnet roots" #steps 250000000 {
  b: [6000] u8; ids: [32 * 19] u8; wids: [32 * 19] u8; work: [32 * 19] u8; root: [32] u8;
  ins: [4] TxIn; outs: [4] TxOut; items: [4] TxItem;
  s := decode(MK_BLOCK_100000, b[..]);
  k := merkle_block_ids_vartime(s, false, ins[..], outs[..], items[..], ids[..], wids[..]);
  #assert k.code == MERKLE_OK && k.count == 4 && merkle_check_vartime(ids[0..<128], work[..], s[36..<68]) == MERKLE_OK;
  #assert merkle_root_vartime(ids[0..<128], work[..], root[..]).code == MERKLE_OK && id_is(root[..], MK_ROOT_100000);
  s = decode(MK_BLOCK_514999, b[..]);
  k = merkle_block_ids_vartime(s, false, ins[..], outs[..], items[..], ids[..], wids[..]);
  #assert k.code == MERKLE_OK && k.count == 19 && merkle_check_vartime(ids[..], work[..], s[36..<68]) == MERKLE_OK;
  #assert merkle_root_vartime(ids[..], work[..], root[..]).code == MERKLE_OK && id_is(root[..], MK_ROOT_514999);
  #assert merkle_witness_root_vartime(wids[..], work[..], root[..]) == MERKLE_OK && id_is(root[..], MK_WITNESS_ROOT_514999);
}
// Block 413,567 from Bitcoin Core's test data: 1,557 txids, an odd count.
#test "block 413,567's root" #steps 1000000000 {
  h: [80] u8; leaves: [32 * 1557] u8; work: [32 * 1557] u8; root: [32] u8;
  header := decode(MK_HEADER_413567, h[..]);
  big := decode(MK_LEAVES_413567, leaves[..]);
  #assert big.count == 32 * MK_COUNT_413567 && merkle_check_vartime(big, work[..], header[36..<68]) == MERKLE_OK;
  #assert merkle_root_vartime(big, work[..], root[..]).code == MERKLE_OK && id_is(root[..], MK_ROOT_413567);
  #assert sha256d_checked(header, root[..], false) == SHA_OK && id_is(root[..], MK_HASH_413567);
}
// The worked example's verdicts: the repeated block, the branch, the
// 64-byte transaction and the forged proof.
#test "the example's verdicts" #steps 200000000 {
  b: [6000] u8; ids: [32 * 20] u8; wids: [32 * 20] u8; work: [32 * 20] u8; root: [32] u8; d: [32] u8; branch: [64] u8;
  ins: [4] TxIn; outs: [4] TxOut; items: [4] TxItem;
  s := decode(MK_BLOCK_100000, b[..]);
  k := merkle_block_ids_vartime(s, false, ins[..], outs[..], items[..], ids[..], wids[..]);
  t := merkle_branch_vartime(ids[0..<128], 2, work[..], branch[..]);
  #assert t.code == MERKLE_OK && t.depth == 2;
  #assert merkle_verify_vartime(ids[64..<96], branch[..], 2, 4, s[36..<68]) == MERKLE_OK;
  #assert merkle_verify_vartime(ids[64..<96], branch[..], 3, 4, s[36..<68]) == MERKLE_MISMATCH;
  #assert merkle_verify_vartime(ids[64..<96], branch[0..<32], 2, 4, s[36..<68]) == MERKLE_BAD_DEPTH;
  t64: [64] u8;
  tx := decode(MK_TX64, t64[..]);
  cb := tx_parse_vartime(s[81..<s.count], true, ins[..], outs[..], items[..]);
  s[80] = 2;
  end := 81 + cb.total;
  for i: 0..<64 { s[end+i] = tx[i]; }
  k = merkle_block_ids_vartime(s[0..<end+64], false, ins[..], outs[..], items[..], ids[..], wids[..]);
  #assert k.code == MERKLE_OK && k.count == 2 && k.first64 == 1;
  #assert merkle_root_vartime(ids[0..<64], work[..], root[..]).code == MERKLE_OK;
  forged: [64] u8;
  for i: 0..<32 { forged[i] = tx[i]; forged[32+i] = ids[i]; }
  #assert merkle_branch_root_vartime(tx[32..<64], forged[..], 3, d[..]) == MERKLE_OK && equal_bytes(d[..], root[..]);
  #assert merkle_verify_vartime(tx[32..<64], forged[..], 3, 2, root[..]) == MERKLE_BAD_INDEX;
  #assert merkle_verify_vartime(tx[32..<64], forged[..], 3, 4, root[..]) == MERKLE_OK;
  k = merkle_block_ids_vartime(s[0..<end+64], true, ins[..], outs[..], items[..], ids[..], wids[..]);
  #assert k.code == MERKLE_TX_64 && k.at == end && CODES[k.code] == "64-byte transaction";
}
```

To run them:

```shell
coax/bin/coax test bitcoin/examples/checks/merkle-trees.coax
coax/bin/coax mutate bitcoin/examples/checks/merkle-trees.coax --root-only
```

`coax test` runs 91 checks, including those of the files it loads, in about 50 seconds on 2026-11-02. Root-only mutation on 2026-11-02 makes 27 mutants and kills all 27.

### The library's own checks

`merkle.coax` is an `#authority` file. Its own checks cover:

- one and two leaves, three leaves by hand, and every refusal of the root
  with nothing written;
- `merkle_test_mutated_return_value`: leaves 1 to 6 against 1 to 6, 5, 6,
  and the non-trailing pair 1, 1, 3, 4; and equal hashes that are not a
  pair;
- the check in Core's order, including an empty tree with and without a
  zero header root;
- `merkle_test_BlockWitness`, the witness root's refusals, and a witness
  tree with equal leaves;
- depths, and every branch of trees of 1 to 9 leaves against the root and
  the proof; the branch's and the proof's refusals, a lying count, and a
  branch with bits past its depth;
- blocks 100,000 and 514,999 with their txids, roots and witness
  commitment, block 514,999 with its last transaction repeated, and each
  refusal of the block reader;
- the 64-byte transaction, with and without BIP-54's rule, and a 65-byte
  one.

```shell
coax/bin/coax test coax/lib/bitcoin/merkle.coax
coax/bin/coax mutate coax/lib/bitcoin/merkle.coax --root-only
```

`coax test` runs 82 checks, including those of the files it loads, in about 9 seconds on 2026-11-02. Root-only mutation on 2026-11-02 makes 445 mutants: it kills 444, and one is marked equivalent in the source with its reason (a test helper reads a coinbase, which has an input, so the witness flag reads it the same way). None survives.

### Core in Python, natively and in Wasm

[merkle_reference.py](../../coax/tests/bitcoin/merkle_reference.py) is a
transliteration of the pinned Core sources: `ComputeMerkleRoot`,
`BlockWitnessMerkleRoot` and `MerkleComputation` from
`consensus/merkle.cpp`, `ComputeMerkleRootFromBranch` and the older
`BlockBuildMerkleTree` and `BlockGetMerkleBranch` from `merkle_tests.cpp`,
and `CheckMerkleRoot`'s order from `validation.cpp`. It reads blocks with
chapter 34's `tx_reference.py`. It shares no code with Coax.
[merkle.py](../../coax/tests/bitcoin/merkle.py) compiles `merkle.coax` to C
in checked and release builds, through a flat test entry point
([merkle_report.coax](../../coax/tests/bitcoin/merkle_report.coax)). Its
cases are:

- trees: `merkle_test`'s construction for 0 to 16 transactions and six
  seeded larger counts, each with up to three rounds of repetition; Core's
  CVE-2012-2459 leaves; 120 seeded trees with an equal pair planted at a
  random level or a repeated last leaf; and the txids of the three mainnet
  blocks. For each: the root, the flag, the check, the witness root, the
  depth and the branches, which must equal `MerkleComputation`'s;
- blocks: the three mainnet blocks with and without BIP-54's rule, the
  mutated block, the 64-byte and 65-byte blocks, short and oversized
  counts, and 120 seeded edits of blocks 100,000 and 514,999;
- proofs: 80 seeded branches, each verified as it is and with the index,
  the count, the leaf, the root or the branch's length changed.

The report must equal the one built from the Python reference, and the
output buffer past it must be untouched. Then the harness builds the same
C for wasm32 and runs every case in Node.

```shell
python3 coax/tests/bitcoin/merkle.py
```

On 2026-11-02, 498 cases agree with Python in native checked and release builds and in Wasm checked and release builds. The check returns ok, empty, mutated and mismatch among them, and the block reader returns ok, capacity, bad block and 64-byte transaction. The run takes about 20 seconds.

The vectors come from
[make_merkle_vectors.py](../../coax/tests/bitcoin/make_merkle_vectors.py),
which checks each pinned file's SHA-256, that each block's header hashes
to its block hash, that its txids give the root in its header and the
published one, and that block 514,999's witness root gives its coinbase
commitment. It writes
[merkle_vectors.coax](../../coax/tests/bitcoin/merkle_vectors.coax) and
[merkle_block413567.coax](../../coax/tests/bitcoin/merkle_block413567.coax).

## What this chapter leaves out

- The header's other fields, the block hash and proof of work: chapter 47.
- The witness commitment, its position and the reserved value: chapter 49.
- The rest of block validation, and the BIP-54 rules other than the
  64-byte one (the timewarp fix, the sigop limit and the coinbase
  `nLockTime`): chapters 51, 53, 54 and 57.
- BIP-37 partial Merkle trees and bloom filters: left out of the book.
- Merkle proofs as a network message, and the light-client workflow
  around them: compact block filters in chapters 73 and 74 take that role.

## Summary

- The root is a tree of SHA-256d hashes over the txids, coinbase first. An
  odd level pairs its last hash with itself.
- Repeating trailing transactions can keep the root (CVE-2012-2459). Core
  flags any level that pairs two equal hashes, and refuses the block as
  mutated: "bad-txnmrklroot" first, then "bad-txns-duplicate".
- A mutated block says nothing about its header. A node drops the bytes
  and fetches the block again; it never marks the block hash invalid.
- A 64-byte transaction is also an inner node. Today that is valid by
  consensus and refused by policy; BIP-54 proposes to make it invalid.
- A branch has one hash per level. A verifier must check its length
  against a transaction count it trusts.
- The witness root is the same tree over wtxids, with the coinbase's as
  zero.

## Dependencies

- [3. Constant-time rules and secrets](ch003-constant-time-rules.md)
- [5. SHA-256d and tagged hashes](ch005-sha-256d-and-tagged-hashes.md)
- [34. Transactions and their serialization](ch034-transactions-and-their-serialization.md)

## Sources

- Bitcoin Core at revision `ddea9196`: `consensus/merkle.cpp`,
  `consensus/merkle.h`, `test/merkle_tests.cpp`, `validation.cpp` and
  `policy/policy.h`, and `src/bench/data/block413567.raw`, pinned in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/merkle.cpp).
  `net_processing.cpp`, `merkleblock.cpp` and `node/miner.cpp` at the same
  revision are cited, not pinned.
- [BIP-54](https://github.com/bitcoin/bips/blob/927b6de9915c9262615a6399de51b200f81e5aa4/bip-0054.md)
  (Consensus Cleanup) at bips commit `927b6de9`, pinned in
  [coax/tests/vectors/upstream/bip54/](../../coax/tests/vectors/upstream/bip54/bip-0054.md).
- [BIP-141](https://github.com/bitcoin/bips/blob/master/bip-0141.mediawiki)
  (the witness root and commitment) and
  [BIP-37](https://github.com/bitcoin/bips/blob/master/bip-0037.mediawiki)
  (partial Merkle trees).
- Bitcoin Core pull request
  [35793](https://github.com/bitcoin/bitcoin/pull/35793), "Implement BIP 54
  (Consensus Cleanup) without mainnet activation", open on 2026-11-02.
- Mainnet blocks 100,000 and 514,999 from mempool.space, pinned in
  [coax/tests/vectors/upstream/mainnet/](../../coax/tests/vectors/upstream/mainnet/000000000003ba27aa200b1cecaad478d2b00432346c3f1f3986da1afd33e506.json).
- All are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
