# 42. Script trees: tapleaves and tapbranches

Chapter 41 tweaked the internal key with a Merkle root and took the root
as given. This chapter builds it. A Taproot output's spending scripts are
the leaves of a binary tree. Each leaf hashes to a *TapLeaf* hash, each
inner node to a *TapBranch* hash of its two children, and the root is the
tree's commitment. To spend with one script, the spender reveals that
script and a *control block*: the internal key, Q's parity and the
sibling hashes on the path from the leaf to the root. The verifier
recomputes the root and Q from them.

Every leaf hash, root and control block in BIP-341's wallet test vectors
comes out the same, every control block commits to its output key, and
BIP-341's own Python, pinned, prints the example's output line for line.

## What this chapter teaches

- TapLeaf hashes: the leaf version (0xc0 for tapscript), the script's
  compact size, then the script.
- TapBranch hashes, with the two children sorted, so a path needs no
  left/right bits.
- A tree from its leaves listed with their depths, and each leaf's path.
- Control blocks: 33 + 32m bytes, the leaf version with the parity bit,
  the internal key, then m path hashes, with m at most 128.
- The commitment check a script-path spend must pass before its script
  runs, and what it refuses.

## Where Bitcoin uses script trees

| Where | What it does | Chapter |
| --- | --- | --- |
| A wallet creating a Taproot output with scripts | the root that chapter 41's tweak commits to | 41, 98, 99 |
| A wallet spending by a script | the leaf's control block | 45, 99 |
| The script interpreter | the commitment check of a script-path spend | 44, 45 |
| Descriptors and PSBTs | `tr(KEY, {…})` and the tap tree field, as leaves with depths | 30, 94 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [taproot.coax](../../coax/lib/bitcoin/taproot.coax) | `taproot_leaf_hash_vartime(version, script, out)` | the TapLeaf hash |
| | `taproot_branch_vartime(a, b, out)` | the TapBranch hash of two sorted children |
| | `taproot_tree_vartime(depths, leaves, tree)` | a tree's root and each leaf's path |
| | `taproot_control_block_vartime(tree, k, internal, version, out)` | the control block for leaf k |
| | `taproot_commitment_vartime(control, script, program)` | BIP-341's commitment check |
| [address.coax](../../coax/lib/bitcoin/address.coax) | `taproot_output_key_vartime` | Q from P and the root (chapter 41) |

Scripts, trees and control blocks are public, so all of it runs in
variable time.

## Leaves and branches

A leaf is a leaf version and a script. Its hash is

```text
TapLeaf = H_TapLeaf(version || compact_size(len(script)) || script)
```

The version is 0xc0 for BIP-342 tapscript, the only one with rules today.
Other versions are allowed: a script-path spend with an unknown version
succeeds without running its script, which leaves room for later soft
forks. A version is always even, because a control block's first byte
carries it in the high seven bits and Q's parity in the low bit.

An inner node's hash is `H_TapBranch(a || b)` for its children's hashes
a and b, the smaller one first, compared byte by byte. The sorting is
why a control block lists only hashes: the verifier joins its running
hash with each path hash in sorted order and never needs to know which
side it is on. A single-leaf tree's root is the leaf hash itself.

<!-- source: coax/lib/bitcoin/taproot.coax tree -->
```jai
TR_BAD_LEAF_VERSION :: -3;  // a leaf version with its low bit set, or above 0xfe
TR_BAD_TREE :: -4;          // depths that don't make a binary tree, or more than TR_TREE_LEAVES leaves
TR_BAD_CONTROL_SIZE :: -5;  // a control block that is not 33 + 32m bytes with m at most 128
TR_MISMATCH :: -6;          // a control block and script that don't commit to the output key
TR_CAPACITY :: -7;          // an output buffer of the wrong size, or a script too long to hash
TR_OK :: 0;
TR_LEAF_TAPSCRIPT :: 0xc0;  // BIP-342's leaf version
TR_TREE_LEAVES :: 16;       // the leaves one TrTree holds, so a path is at most 15 hashes
TR_CONTROL_MAX :: 33 + 32 * 128;   // consensus: a path of at most 128 hashes
// A built tree: its leaf hashes, root, and each leaf's path, the sibling
// hashes from the leaf up to the root's child, as a control block lists them.
TrTree :: struct { count: int; root: [32] u8; leaf: [TR_TREE_LEAVES][32] u8; path_n: [TR_TREE_LEAVES] int; path: [TR_TREE_LEAVES][TR_TREE_LEAVES - 1][32] u8; }

// TapLeaf: H_TapLeaf(version || compact size(script) || script). The low
// bit of a control block's first byte is the output key's parity, so a
// leaf version is always even.
taproot_leaf_hash_vartime :: (version: int, script: [] u8, out: [] u8) -> int {
  if version < 0 || version > 0xfe || (version & 1) != 0 { return TR_BAD_LEAF_VERSION; }
  if out.count != 32 || !sha256_input_ok(script.count) { return TR_CAPACITY; }
  state: [SHA_STATE_BYTES] u8; prefix: [10] u8;
  prefix[0] = cast(u8) version;
  n := bt_var_put(prefix[..], 1, script.count);
  #assert bitcoin_tagged_init(state[..], BH_TAPLEAF) == SHA_OK;
  #assert sha256_update_checked(state[..], prefix[0..<n], false) == SHA_OK && sha256_update_checked(state[..], script, false) == SHA_OK;
  #assert sha256_final_checked(state[..], out, false) == SHA_OK;
  return TR_OK;
}
// TapBranch: H_TapBranch of the two children, the smaller one first, so a
// node's hash doesn't depend on which child is on the left. out may be a
// or b.
taproot_branch_vartime :: (a: [] u8, b: [] u8, out: [] u8) {
  m: [64] u8;
  b_first := false;
  for i: 0..<32 { if a[i] != b[i] { b_first = b[i] < a[i]; break; } }
  for i: 0..<32 { m[i] = ifx b_first then b[i] else a[i]; m[32 + i] = ifx b_first then a[i] else b[i]; }
  bt_tagged(BH_TAPBRANCH, m[..], out);
}
// A tree from its leaf hashes, listed left to right with their depths (a
// lone leaf has depth 0, the root's children depth 1): the order and form
// descriptors and PSBTs use. A stack joins the top two nodes while they
// have the same depth. Each join adds each side's hash to the other side's
// leaves' paths. The depths make a tree when one node of depth 0 is left.
taproot_tree_vartime :: (depths: [] int, leaves: [] [32] u8, tree: [] TrTree) -> int {
  fresh: TrTree;
  if tree.count != 1 { return TR_CAPACITY; }
  tree[0] = fresh;
  n := depths.count;
  if n == 0 || n > TR_TREE_LEAVES || leaves.count != n { return TR_BAD_TREE; }
  hash: [TR_TREE_LEAVES][32] u8; depth: [TR_TREE_LEAVES] int; lo: [TR_TREE_LEAVES] int; hi: [TR_TREE_LEAVES] int;
  top := 0;
  for k: 0..<n {
    if depths[k] < 0 || depths[k] >= TR_TREE_LEAVES { tree[0] = fresh; return TR_BAD_TREE; }
    tree[0].leaf[k] = leaves[k];
    hash[top] = leaves[k]; depth[top] = depths[k]; lo[top] = k; hi[top] = k + 1; top += 1;
    while top >= 2 && depth[top - 1] == depth[top - 2] {
      a := top - 2; b := top - 1;
      if depth[a] == 0 { tree[0] = fresh; return TR_BAD_TREE; }
      for j: lo[a]..<hi[a] { tree[0].path[j][tree[0].path_n[j]] = hash[b]; tree[0].path_n[j] += 1; }
      for j: lo[b]..<hi[b] { tree[0].path[j][tree[0].path_n[j]] = hash[a]; tree[0].path_n[j] += 1; }
      taproot_branch_vartime(hash[a][..], hash[b][..], hash[a][..]);
      depth[a] -= 1; hi[a] = hi[b]; top -= 1;
    }
  }
  if top != 1 || depth[0] != 0 { tree[0] = fresh; return TR_BAD_TREE; }
  tree[0].count = n; tree[0].root = hash[0];
  return TR_OK;
}
```

### Describing a tree by its leaves' depths

BIP-341's Python walks a nested list. The Coax code takes the leaves left
to right, each with its depth: the form output descriptors and PSBTs use.
A stack builds the tree. Each leaf is pushed, and while the top two nodes
have the same depth they are joined into one node a level up. The depths
make a tree exactly when one node of depth 0 is left. `1 2 2` is a leaf
beside a branch of two; `1 2 1`, `1 2` and `0 0` make no tree.

Each join also records the paths. Joining nodes L and R adds R's hash to
the path of every leaf under L, and L's hash to every leaf under R, so
each path lists siblings from the leaf upward, as a control block needs
them. This is what BIP-341's `taproot_tree_helper` does recursively.

A `TrTree` holds 16 leaves, so a path is at most 15 hashes. That bound is
the builder's, not consensus: the commitment check accepts paths of up to
128 hashes.

### Worked example: vector 5

BIP-341's vector 5 has one leaf at depth 1 and two at depth 2:

1. Leaves 1 and 2 have the TapLeaf hashes `ba98…de1c` and `9e31…eaf6`.
   `9e31` sorts first, so their branch is
   `H_TapBranch(9e31… || ba98…)` = `ffe5…e553`.
2. Leaf 0's hash `2645…9817` sorts before `ffe5…`, so the root is
   `H_TapBranch(2645… || ffe5…)` = `ccbd…e0e2`, BIP-341's `merkleRoot`.
3. Leaf 1's path is leaf 2's hash, then leaf 0's: its sibling first, then
   its parent's sibling.

## Control blocks

A script-path witness ends with the script and its control block:

| Bytes | What |
| --- | --- |
| 1 | the leaf version, plus 1 if Q's y is odd |
| 32 | the internal key P |
| 32m | the path, sibling first, with m from 0 to 128 |

The parity is Q's: chapter 41 explains why the verifier checks it.

<!-- source: coax/lib/bitcoin/taproot.coax control -->
```jai
// The control block that spends leaf k of tree (whose leaf version is
// version) from the output of internal key P: one byte of leaf version
// plus Q's parity, P, then the leaf's path. Returns its length, 33 + 32m.
taproot_control_block_vartime :: (tree: [] TrTree, k: int, internal: [] u8, version: int, out: [] u8) -> int {
  if tree.count != 1 { return TR_CAPACITY; }
  if k < 0 || k >= tree[0].count || version < 0 || version > 0xfe || (version & 1) != 0 { return TR_BAD_TREE; }
  n := 33 + 32 * tree[0].path_n[k];
  if out.count < n { return TR_CAPACITY; }
  q: [32] u8;
  parity := taproot_output_key_vartime(internal, tree[0].root[..], q[..]);
  if parity < 0 { return TR_MISMATCH; }
  out[0] = cast(u8) (version + parity);
  for i: 0..<32 { out[1 + i] = internal[i]; }
  for j: 0..<tree[0].path_n[k] { for i: 0..<32 { out[33 + 32 * j + i] = tree[0].path[k][j][i]; } }
  return n;
}
// BIP-341's script-path commitment check: the leaf hash of script under
// the control block's leaf version, joined with each path hash in turn,
// is the root r, and P + H_TapTweak(P || r)G must have the x in the
// witness program and the parity in the control block. Bitcoin Core
// reports every failure after the size check as
// SCRIPT_ERR_WITNESS_PROGRAM_MISMATCH: an internal key off the curve too.
taproot_commitment_vartime :: (control: [] u8, script: [] u8, program: [] u8) -> int {
  if control.count < 33 || control.count > TR_CONTROL_MAX || (control.count - 33) % 32 != 0 { return TR_BAD_CONTROL_SIZE; }
  if program.count != 32 { return TR_MISMATCH; }
  k: [32] u8;
  if taproot_leaf_hash_vartime(cast(int) (control[0] & 0xfe), script, k[..]) != TR_OK { return TR_CAPACITY; }
  for j: 0..<(control.count - 33) / 32 { taproot_branch_vartime(k[..], control[33 + 32 * j..<65 + 32 * j], k[..]); }
  q: [32] u8;
  parity := taproot_output_key_vartime(control[1..<33], k[..], q[..]);
  if parity != cast(int) (control[0] & 1) { return TR_MISMATCH; }
  for i: 0..<32 { if q[i] != program[i] { return TR_MISMATCH; } }
  return TR_OK;
}
```

The check follows BIP-341's script validation rules in order:

1. The control block is 33 + 32m bytes, with m from 0 to 128. Anything
   else fails before any hashing (Core's
   `SCRIPT_ERR_TAPROOT_WRONG_CONTROL_SIZE`).
2. k₀ is the TapLeaf hash of the script with the control block's leaf
   version, its first byte with the low bit cleared.
3. Each path hash eⱼ is joined in sorted order: kⱼ₊₁ is
   `H_TapBranch` of kⱼ and eⱼ, the smaller first.
4. Q = P + H_TapTweak(P ‖ kₘ)G, chapter 41's output key.
5. Q's x must be the witness program and its parity the control block's
   low bit. Otherwise the spend fails (Core's
   `SCRIPT_ERR_WITNESS_PROGRAM_MISMATCH`, which it also reports for an
   internal key off the curve).

Only then does the script run. Chapter 44 builds tapscript, and chapter
45 puts the witness together: the annex, the script, the control block
and the stack the script starts with.

## The example

The example is
[bitcoin/examples/taproot-script-trees.coax](../examples/taproot-script-trees.coax).
It builds the six script trees of BIP-341's vectors, makes and checks
every control block, takes vector 5 apart step by step, changes one part
of a control block at a time, and tries depths that make no tree.

<!-- source: bitcoin/examples/taproot-script-trees.coax example -->
```jai
#load "../../coax/lib/bitcoin/taproot.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [8400] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// Print one byte as two hex digits.
hex_byte :: (v: int) {
  b: [1] u8; text: [2] u8; b[0] = cast(u8) v;
  r := hex_encode_checked(cast(string) b[..], text[..], false);
  print("%", cast(string) text[0..<r.written]);
}
NAMES :: [8] string.["ok", "bad hash type", "bad size", "bad leaf version", "bad tree",
                     "bad control block size", "mismatch", "capacity"];
name :: (code: int) -> string { return ifx code <= 0 && code >= -7 then NAMES[-code] else "?"; }
// a < b, byte by byte: the order TapBranch sorts children in.
less :: (a: [] u8, b: [] u8) -> bool {
  for i: 0..<32 { if a[i] != b[i] { return a[i] < b[i]; } }
  return false;
}
same :: (ok: bool) -> string { return ifx ok then "matches" else "differs"; }

// A vector's tree from its leaves' depths and scripts.
build :: (v: TrSpk, tree: [] TrTree) -> int {
  depths: [TR_TREE_LEAVES] int; hashes: [TR_TREE_LEAVES][32] u8; sc: [64] u8;
  for k: 0..<v.count {
    leaf := TR_LEAVES[v.first + k]; depths[k] = leaf.depth;
    #assert taproot_leaf_hash_vartime(leaf.version, bt_unhex(leaf.script, sc[..]), hashes[k][..]) == TR_OK;
  }
  return taproot_tree_vartime(depths[0..<v.count], hashes[0..<v.count], tree);
}

main :: () {
  p: [32] u8; spk: [34] u8; c: [TR_CONTROL_MAX + 32] u8; sc: [64] u8; tree: [1] TrTree;

  // Every script tree in BIP-341's vectors.
  print("BIP-341's script trees\n");
  for i: 1..<TR_SPK.count {
    v := TR_SPK[i];
    #assert build(v, tree[..]) == TR_OK;
    pk := bt_unhex(v.internal, p[..]); prog := bt_unhex(v.spk, spk[..])[2..<34];
    print("  vector %: root %\n", i, same(bt_hex_is(tree[0].root[..], v.root)));
    for k: 0..<v.count {
      leaf := TR_LEAVES[v.first + k];
      n := taproot_control_block_vartime(tree[..], k, pk, leaf.version, c[..]);
      print("    leaf % (version 0x", k);
      hex_byte(leaf.version);
      print(", depth %): hash %, control block of % bytes %, commits: %\n", leaf.depth,
            same(bt_hex_is(tree[0].leaf[k][..], leaf.hash)), n, same(bt_hex_is(c[0..<n], leaf.control)),
            name(taproot_commitment_vartime(c[0..<n], bt_unhex(leaf.script, sc[..]), prog)));
    }
  }

  // Vector 5 step by step: one leaf at depth 1, two at depth 2.
  print("vector 5, step by step\n");
  v := TR_SPK[5];
  #assert build(v, tree[..]) == TR_OK;
  for k: 0..<3 { print("  leaf % ", k); show("TapLeaf", tree[0].leaf[k][..]); }
  first := ifx less(tree[0].leaf[1][..], tree[0].leaf[2][..]) then 1 else 2;
  print("  leaves 1 and 2 join with leaf % first, as the smaller hash\n", first);
  b12: [32] u8; taproot_branch_vartime(tree[0].leaf[1][..], tree[0].leaf[2][..], b12[..]);
  show("  TapBranch(1, 2)", b12[..]);
  first = ifx less(tree[0].leaf[0][..], b12[..]) then 0 else 12;
  print("  the root joins leaf 0 and that branch with %\n", ifx first == 0 then "leaf 0 first" else "the branch first");
  show("  root", tree[0].root[..]);
  pk := bt_unhex(v.internal, p[..]);
  n := taproot_control_block_vartime(tree[..], 1, pk, TR_LEAF_TAPSCRIPT, c[..]);
  print("  leaf 1's control block, % bytes:\n", n);
  print("    first byte 0x"); hex_byte(cast(int) c[0]); print(": leaf version 0x"); hex_byte(cast(int) (c[0] & 0xfe));
  print(", Q's parity %\n", c[0] & 1);
  show("    internal key P", c[1..<33]);
  show("    path 0 (leaf 2)", c[33..<65]);
  show("    path 1 (leaf 0)", c[65..<97]);

  // What the commitment check refuses, on leaf 1 of vector 5.
  print("leaf 1 of vector 5, changed\n");
  prog := bt_unhex(v.spk, spk[..])[2..<34];
  script := bt_unhex(TR_LEAVES[v.first + 1].script, sc[..]);
  print("  as built: %\n", name(taproot_commitment_vartime(c[0..<n], script, prog)));
  c[0] ^= 1; print("  the parity bit flipped: %\n", name(taproot_commitment_vartime(c[0..<n], script, prog))); c[0] ^= 1;
  c[0] ^= 2; print("  leaf version 0xc2: %\n", name(taproot_commitment_vartime(c[0..<n], script, prog))); c[0] ^= 2;
  c[40] ^= 1; print("  one bit of a path hash: %\n", name(taproot_commitment_vartime(c[0..<n], script, prog))); c[40] ^= 1;
  sw: [64] u8; for i: 0..<64 { sw[i] = c[33 + i]; }
  for i: 0..<32 { c[33 + i] = sw[32 + i]; c[65 + i] = sw[i]; }
  print("  the path in the other order: %\n", name(taproot_commitment_vartime(c[0..<n], script, prog)));
  for i: 0..<64 { c[33 + i] = sw[i]; }
  print("  the last path hash left out: %\n", name(taproot_commitment_vartime(c[0..<n - 32], script, prog)));
  script[2] ^= 1; print("  one bit of the script: %\n", name(taproot_commitment_vartime(c[0..<n], script, prog))); script[2] ^= 1;
  print("  34 bytes: %\n", name(taproot_commitment_vartime(c[0..<34], script, prog)));
  print("  33 + 32 * 129 bytes: %\n", name(taproot_commitment_vartime(c[0..<TR_CONTROL_MAX + 32], script, prog)));

  // Depths that make no tree.
  print("depths that make no tree\n");
  h: [3][32] u8; d := [3] int.[1, 2, 2];
  print("  1 2 2: %\n", name(taproot_tree_vartime(d[..], h[..], tree[..])));
  d[2] = 1; print("  1 2 1: %\n", name(taproot_tree_vartime(d[..], h[..], tree[..])));
  print("  1 2: %\n", name(taproot_tree_vartime(d[0..<2], h[0..<2], tree[..])));
  d[0] = 0; d[1] = 0; print("  0 0: %\n", name(taproot_tree_vartime(d[0..<2], h[0..<2], tree[..])));
  print("  an odd leaf version, 0xc1: %\n", name(taproot_leaf_hash_vartime(0xc1, script, h[0][..])));
}
```

To run it:

```shell
coax/bin/coax run bitcoin/examples/taproot-script-trees.coax --run-steps 250000000
```

It prints:

```text
BIP-341's script trees
  vector 1: root matches
    leaf 0 (version 0xc0, depth 0): hash matches, control block of 33 bytes matches, commits: ok
  vector 2: root matches
    leaf 0 (version 0xc0, depth 0): hash matches, control block of 33 bytes matches, commits: ok
  vector 3: root matches
    leaf 0 (version 0xc0, depth 1): hash matches, control block of 65 bytes matches, commits: ok
    leaf 1 (version 0xfa, depth 1): hash matches, control block of 65 bytes matches, commits: ok
  vector 4: root matches
    leaf 0 (version 0xc0, depth 1): hash matches, control block of 65 bytes matches, commits: ok
    leaf 1 (version 0xc0, depth 1): hash matches, control block of 65 bytes matches, commits: ok
  vector 5: root matches
    leaf 0 (version 0xc0, depth 1): hash matches, control block of 65 bytes matches, commits: ok
    leaf 1 (version 0xc0, depth 2): hash matches, control block of 97 bytes matches, commits: ok
    leaf 2 (version 0xc0, depth 2): hash matches, control block of 97 bytes matches, commits: ok
  vector 6: root matches
    leaf 0 (version 0xc0, depth 1): hash matches, control block of 65 bytes matches, commits: ok
    leaf 1 (version 0xc0, depth 2): hash matches, control block of 97 bytes matches, commits: ok
    leaf 2 (version 0xc0, depth 2): hash matches, control block of 97 bytes matches, commits: ok
vector 5, step by step
  leaf 0 TapLeaf: 2645a02e0aac1fe69d69755733a9b7621b694bb5b5cde2bbfc94066ed62b9817
  leaf 1 TapLeaf: ba982a91d4fc552163cb1c0da03676102d5b7a014304c01f0c77b2b8e888de1c
  leaf 2 TapLeaf: 9e31407bffa15fefbf5090b149d53959ecdf3f62b1246780238c24501d5ceaf6
  leaves 1 and 2 join with leaf 2 first, as the smaller hash
  TapBranch(1, 2): ffe578e9ea769027e4f5a3de40732f75a88a6353a09d767ddeb66accef85e553
  the root joins leaf 0 and that branch with leaf 0 first
  root: ccbd66c6f7e8fdab47b3a486f59d28262be857f30d4773f2d5ea47f7761ce0e2
  leaf 1's control block, 97 bytes:
    first byte 0xc0: leaf version 0xc0, Q's parity 0
    internal key P: e0dfe2300b0dd746a3f8674dfd4525623639042569d829c7f0eed9602d263e6f
    path 0 (leaf 2): 9e31407bffa15fefbf5090b149d53959ecdf3f62b1246780238c24501d5ceaf6
    path 1 (leaf 0): 2645a02e0aac1fe69d69755733a9b7621b694bb5b5cde2bbfc94066ed62b9817
leaf 1 of vector 5, changed
  as built: ok
  the parity bit flipped: mismatch
  leaf version 0xc2: mismatch
  one bit of a path hash: mismatch
  the path in the other order: mismatch
  the last path hash left out: mismatch
  one bit of the script: mismatch
  34 bytes: bad control block size
  33 + 32 * 129 bytes: bad control block size
depths that make no tree
  1 2 2: ok
  1 2 1: bad tree
  1 2: bad tree
  0 0: bad tree
  an odd leaf version, 0xc1: bad leaf version
```

What the output shows:

- Every leaf hash, root and control block matches BIP-341's, and every
  control block commits to its output key. Vector 3's leaf with version
  0xfa, a version with no rules yet, commits like any other.
- A leaf at depth d has a control block of 33 + 32d bytes.
- Branches sort their children: leaves 1 and 2 of vector 5 join with
  leaf 2 first.
- Every change to the control block or the script fails the check: the
  parity bit, the leaf version, one bit of a path hash, the path's order,
  a missing path hash and one bit of the script. Only the sizes fail
  before hashing.

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/taproot-script-trees.out](../fixtures/taproot-script-trees.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/taproot-script-trees.coax](../examples/checks/taproot-script-trees.coax),
which loads the example.

<!-- source: bitcoin/examples/checks/taproot-script-trees.coax check -->
```jai
// BIP-341's figure: five leaves, AB = (A, B) and CDE = ((C, D), E), so the
// depths are 2 2 3 3 2. D's control block lists C, E and AB, in that
// order, and every leaf's control block commits to Q.
#test "script-trees: BIP-341's five-leaf figure" #steps 2000000000 {
  scripts := [5] u8.[0x51, 0x52, 0x53, 0x54, 0x55];   // OP_1 to OP_5, one leaf each
  depths := [5] int.[2, 2, 3, 3, 2];
  h: [5][32] u8; tree: [1] TrTree; p: [32] u8; q: [32] u8; c: [129] u8;
  for k: 0..<5 { #assert taproot_leaf_hash_vartime(TR_LEAF_TAPSCRIPT, scripts[k..<k + 1], h[k][..]) == TR_OK; }
  #assert taproot_tree_vartime(depths[..], h[..], tree[..]) == TR_OK;
  ab: [32] u8; cd: [32] u8; cde: [32] u8; root: [32] u8;
  taproot_branch_vartime(h[0][..], h[1][..], ab[..]); taproot_branch_vartime(h[2][..], h[3][..], cd[..]);
  taproot_branch_vartime(cd[..], h[4][..], cde[..]); taproot_branch_vartime(ab[..], cde[..], root[..]);
  #assert bt_same(tree[0].root[..], root[..]);
  #assert tree[0].path_n[3] == 3 && bt_same(tree[0].path[3][0][..], h[2][..]) && bt_same(tree[0].path[3][1][..], h[4][..]) && bt_same(tree[0].path[3][2][..], ab[..]);
  pk := bt_unhex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798", p[..]);
  #assert taproot_output_key_vartime(pk, root[..], q[..]) >= 0;
  for k: 0..<5 {
    n := taproot_control_block_vartime(tree[..], k, pk, TR_LEAF_TAPSCRIPT, c[..]);
    #assert n == 33 + 32 * depths[k] && taproot_commitment_vartime(c[0..<n], scripts[k..<k + 1], q[..]) == TR_OK;
    // Another leaf's script with this leaf's control block fails.
    #assert taproot_commitment_vartime(c[0..<n], scripts[(k + 1) % 5..<(k + 1) % 5 + 1], q[..]) == TR_MISMATCH;
  }
}
// The same leaves in a different shape give a different root: a tree
// commits to its shape as well as its leaves.
#test "script-trees: the shape is committed" {
  scripts := [3] u8.[0x51, 0x52, 0x53];
  h: [3][32] u8; tree: [1] TrTree; r: [32] u8;
  for k: 0..<3 { #assert taproot_leaf_hash_vartime(TR_LEAF_TAPSCRIPT, scripts[k..<k + 1], h[k][..]) == TR_OK; }
  a := [3] int.[1, 2, 2]; b := [3] int.[2, 2, 1];
  #assert taproot_tree_vartime(a[..], h[..], tree[..]) == TR_OK; r = tree[0].root;
  #assert taproot_tree_vartime(b[..], h[..], tree[..]) == TR_OK && !bt_same(r[..], tree[0].root[..]);
}
```

To run them:

```shell
coax/bin/coax test bitcoin/examples/checks/taproot-script-trees.coax --run-steps 250000000 --only script-trees:
```

On 2026-11-03 both checks pass.

### The library's own checks

`taproot.coax` builds the six script trees of BIP-341's vectors from
their published scripts and depths. Every leaf hash, root, path length
and control block matches, and every control block commits, then fails
with its parity bit or a path bit flipped. Other checks cover sorting
(including equal children and a difference in the last byte, and the
output aliasing a child), leaf hashes with a three-byte compact size,
leaf version refusals, twelve depth lists that make no tree or don't fit, the deepest
tree a `TrTree` holds (16 leaves, a path of 15), and control block sizes
from 32 bytes to 33 + 32 × 129.

```shell
coax/bin/coax test coax/lib/bitcoin/taproot.coax --run-steps 250000000
```

### BIP-341's own Python

[taproot_reference.py](../../coax/tests/bitcoin/taproot_reference.py)
runs BIP-341's `taproot_tree_helper`, `taproot_output_script` and
`taproot_sign_script` as the BIP gives them, and adds the script-path
commitment check transliterated from the BIP's validation rules. It
checks every published control block against both.
[taproot_examples.py](../../coax/tests/bitcoin/taproot_examples.py)
writes the example's output from that code. It decides which depth lists
make a tree by reading them recursively instead of with a stack.

```shell
python3 coax/tests/bitcoin/taproot_reference.py
python3 coax/tests/bitcoin/taproot_examples.py script-trees bitcoin/fixtures/taproot-script-trees.out
```

On 2026-11-03 all 48 lines agree. Mutation testing was not run for this
chapter (owner speed order).

## What this chapter leaves out

- Running the leaf's script: tapscript, chapter 44.
- The script-path witness as a whole (the annex, the script, the control
  block and the initial stack) and spending BIP-341's script paths:
  chapter 45.
- The script-path signature hash, which commits to the leaf hash:
  chapter 43.
- Choosing a tree's shape (a Huffman tree by spending probability) and
  the `tr()` descriptor: chapters 30 and 98.

## Summary

- A leaf hashes as `H_TapLeaf(version || compact size || script)`, with
  an even version, 0xc0 for tapscript.
- A branch hashes its two children sorted, so a path is just hashes.
- Leaves with depths build the tree with a stack; the paths come out of
  the same joins.
- A control block is 33 + 32m bytes, m ≤ 128: version and parity, P, the
  path.
- The commitment check rebuilds the root and Q, and the spend fails
  unless Q's x and parity match.

## Dependencies

- [41. The key tweak and the key path](ch041-the-key-tweak-and-the-key-path.md)

## Sources

- [BIP-341](https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki),
  pinned at
  [coax/tests/vectors/upstream/bip341/bip-0341.mediawiki](../../coax/tests/vectors/upstream/bip341/bip-0341.mediawiki),
  with its wallet test vectors at
  [wallet-test-vectors.json](../../coax/tests/vectors/upstream/bip341/wallet-test-vectors.json).
- Bitcoin Core at `ddea9196`: `VerifyTaprootCommitment` and
  `ComputeTaprootMerkleRoot` in `script/interpreter.cpp`, pinned at
  [coax/tests/vectors/upstream/bitcoin-core/interpreter.cpp](../../coax/tests/vectors/upstream/bitcoin-core/interpreter.cpp).
- The pinned files are listed in
  [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
