# 45. Control blocks and script-path spends

[Chapter 42](ch042-script-trees-tapleaves-and-tapbranches.md) built a
Taproot output's script tree and each leaf's control block.
[Chapter 44](ch044-tapscript.md) taught `VerifyScript` to check a
script-path spend and run its tapscript. This chapter is the spender's
side: it signs a leaf, puts the witness together and spends the output,
then shows that chapter 44's verifier accepts the spend and refuses it
when the control block's parity or path is wrong.

Every leaf of BIP-341's six script trees passes the commitment check
with the control block this code builds, which is BIP-341's byte for
byte. A tree of three leaves with keys the example holds is spent by each
leaf and by its key path. BIP-341's own Python, BIP-340's reference
signer, Bitcoin Core's signature message and a transliteration of Core's
interpreter print the example's 42 lines, the witness bytes among them.

## What this chapter teaches

- The script-path witness: the stack the leaf script starts with, bottom
  first, then the script, then the control block, then an optional
  annex.
- How a leaf's signature differs from a key-path signature: it signs the
  leaf hash too, and the key signs as itself, untweaked.
- Where the signatures go on the stack, so that each `OP_CHECKSIG` or
  `OP_CHECKSIGADD` finds its own.
- One output, spent by any leaf or by its key path.
- What a verifier refuses: a flipped parity bit, a changed or reordered
  path, another leaf's control block, and a signature made for another
  leaf, another output or without the annex.

## Where Bitcoin uses script-path spends

| Where | What it does | Chapter |
| --- | --- | --- |
| A wallet spending by a script | builds this witness for a leaf it can satisfy | 99 |
| PSBT signers | the leaf script and control block a PSBT carries, and the signature this code makes | 94 |
| Multisig on Taproot | a k-of-n leaf of `OP_CHECKSIGADD` | 100 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [taproot.coax](../../coax/lib/bitcoin/taproot.coax) | `taproot_script_sighash_vartime` | BIP-342's signature hash for a tapscript leaf |
| | `taproot_script_sign` | a BIP-340 signature by the leaf's key, with its hash type |
| | `taproot_script_witness_vartime` | the serialized witness: stack, script, control block, annex |
| | `taproot_control_block_vartime` | the leaf's control block (chapter 42) |
| [script.coax](../../coax/lib/bitcoin/script.coax) | `script_verify_vartime` | chapter 44's `VerifyScript`, which checks the spend |
| [sighash.coax](../../coax/lib/bitcoin/sighash.coax) | `bt_taproot_sign` | the key path's signature by the tweaked key (chapter 41) |

The signer's key is secret, and BIP-340 signing runs in constant time
(chapter 24). Everything else here is public and runs in variable time.

## The witness of a script path

A script-path witness has, in order:

| Item | What |
| --- | --- |
| 0 or more | the stack the leaf script starts with, bottom first |
| 1 | the leaf script |
| 1 | the control block: leaf version and Q's parity, P, the path |
| 0 or 1 | the annex, which starts with `0x50` |

`taproot_script_witness_vartime` writes these items as one input's part
of a transaction's witness: the item count, then each item with its
compact size. It refuses a control block of the wrong size and an annex
that doesn't start with `0x50`, the two things a verifier would read
differently from what the wallet meant.

<!-- source: coax/lib/bitcoin/taproot.coax spend -->
```jai
TR_BAD_ANNEX :: -8;         // an annex that doesn't start with 0x50
// BIP-342's signature hash for a tapscript leaf (version 0xc0) of input
// index of the unsigned transaction b: BIP-341's message with the annex,
// if any, and the extension of the leaf hash, key version 0 and the
// position of the last OP_CODESEPARATOR run before the signature check
// (BT_NO_CODESEP for none). amounts and spent are every input's spent
// output. False when the transaction doesn't read or the message refuses.
taproot_script_sighash_vartime :: (b: [] u8, index: int, amounts: [] int, spent: [] BtScript, hashtype: int,
                                   script: [] u8, annex: [] u8, codesep: int, out: [] u8) -> bool {
  tp := bt_tx_read(b);
  if !tp.ok { return false; }
  leaf: [32] u8;
  if taproot_leaf_hash_vartime(TR_LEAF_TAPSCRIPT, script, leaf[..]) != TR_OK { return false; }
  return bt_sighash_taproot_ext(b, tp, index, amounts, spent, hashtype, annex, leaf[..], codesep, out);
}
// A tapscript signature: BIP-340 by key itself, not tweaked, since the
// script names the key it checks, then the hash type as a 65th byte
// unless it is SIGHASH_DEFAULT. Returns the length, 64 or 65, or 0.
taproot_script_sign :: (key: [] u8, sighash: [] u8, aux: [] u8, hashtype: int, w: [] SignWork, out: [] u8) -> int {
  n := ifx hashtype == BT_DEFAULT then 64 else 65;
  if out.count < n || hashtype < 0 || hashtype > 255 { return 0; }
  status := SIGN_REFUSED;
  for k: 0..<6 { status = schnorr_sign_step(key, aux, sighash, w, out[0..<64], false); }
  if status != SIGN_DONE { return 0; }
  if n == 65 { out[64] = cast(u8) hashtype; }
  return n;
}
// The witness of a script-path spend, serialized as one input's part of a
// transaction's witness (chapter 34): the item count, then each item with
// its compact size. The items are the stack the script starts with, its
// bottom first, then the leaf script, the control block, and the annex
// when there is one. Returns the length, TR_BAD_CONTROL_SIZE,
// TR_BAD_ANNEX or TR_CAPACITY.
taproot_script_witness_vartime :: (stack: [] [] u8, script: [] u8, control: [] u8, annex: [] u8, out: [] u8) -> int {
  if control.count < 33 || control.count > TR_CONTROL_MAX || (control.count - 33) % 32 != 0 { return TR_BAD_CONTROL_SIZE; }
  if annex.count > 0 && annex[0] != BT_ANNEX_TAG { return TR_BAD_ANNEX; }
  count := stack.count + 2 + (ifx annex.count > 0 then 1 else 0);
  size := tx_compact_size_vartime(count) + tr_sized(script) + tr_sized(control) + (ifx annex.count > 0 then tr_sized(annex) else 0);
  for item: stack { size += tr_sized(item); }
  if out.count < size { return TR_CAPACITY; }
  at := tx_compact_write_vartime(out, 0, count);
  for item: stack { at = tr_put(out, at, item); }
  at = tr_put(out, at, script); at = tr_put(out, at, control);
  if annex.count > 0 { at = tr_put(out, at, annex); }
  return at;
}
tr_sized :: (item: [] u8) -> int { return tx_compact_size_vartime(item.count) + item.count; }
tr_put :: (out: [] u8, at: int, item: [] u8) -> int {
  p := tx_compact_write_vartime(out, at, item.count);
  for i: 0..<item.count { out[p + i] = item[i]; }
  return p + item.count;
}
```

The signature hash is chapter 43's, with BIP-342's extension: the leaf
hash under version `0xc0`, key version 0 and the position of the last
`OP_CODESEPARATOR`. Since the leaf hash is signed, a signature for one
leaf fails in another, even with the same key. The key signs as itself:
the script names the key, and only the key path uses the tweaked key.
A hash type other than `SIGHASH_DEFAULT` goes after the 64 bytes.

## The order of the stack

A script consumes its stack from the top, so the witness lists items in
the reverse of the order the script uses them. In the leaf

```text
<key 2> OP_CHECKSIG <key 3> OP_CHECKSIGADD 2 OP_NUMEQUAL
```

key 2's `OP_CHECKSIG` runs first and takes the top item, so the witness
is key 3's signature, then key 2's. In

```text
OP_SHA256 <hash> OP_EQUALVERIFY <key 2> OP_CHECKSIG
```

the preimage is on top and the signature below it. An empty item stands
for a key that doesn't sign: `OP_CHECKSIG` and `OP_CHECKSIGADD` count it
as false.

## The example

The example is
[bitcoin/examples/script-path-spend.coax](../examples/script-path-spend.coax).
It spends every leaf of BIP-341's script trees, builds a tree of three
leaves with keys it holds, spends it by each leaf with a hash type and
an annex, tries what a verifier refuses, and spends the same output by
its key path. Every spend completes one unsigned transaction: one input
of 100,000 satoshis from the output under test, one output of 90,000.

<!-- source: bitcoin/examples/script-path-spend.coax example -->
```jai
#load "../../coax/lib/bitcoin/script.coax";
#load "../../coax/lib/bitcoin/malleability.coax";
#load "../../coax/lib/crypto/bip32.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// The unsigned transaction every spend here completes: version 2, one
// input, one output of 90,000 satoshis to a P2WPKH script, locktime 0.
// Its input spends AMOUNT satoshis from the Taproot output being tested.
UNSIGNED :: "0200000001abababababababababababababababababababababababababababababababab0000000000ffffffff01905f010000000000160014111111111111111111111111111111111111111100000000";
AMOUNT :: 100000;
FLAGS :: SCRIPT_VERIFY_P2SH | SCRIPT_VERIFY_WITNESS | SCRIPT_VERIFY_TAPROOT;
PREIMAGE :: "Bitcoin chapter 45";
AUX :: "0000000000000000000000000000000000000000000000000000000000000000";

// Room for one spend: the unsigned and the signed transaction and their parts.
Spend :: struct {
  raw: [256] u8; tx: [4096] u8; spk: [34] u8;
  ins: [2] TxIn; outs: [2] TxOut; items: [16] TxItem;
}
// Puts witness, one input's serialized witness, on the unsigned
// transaction and runs VerifyScript for its input, which spends the
// Taproot output with key program.
verify :: (s: [] Spend, witness: [] u8, program: [] u8, w: [] ScriptWork) -> int {
  raw := bt_unhex(UNSIGNED, s[0].raw[..]);
  t := tx_read_vartime(raw, false, s[0].ins[..], s[0].outs[..], s[0].items[0..<0]);
  n := mal_add_witness_vartime(raw, t, witness, s[0].tx[..]);
  #assert t.code == TX_OK && n > 0;
  u := tx_read_vartime(s[0].tx[0..<n], true, s[0].ins[..], s[0].outs[..], s[0].items[..]);
  #assert u.code == TX_OK;
  s[0].spk[0] = 0x51; s[0].spk[1] = 32; for i: 0..<32 { s[0].spk[2 + i] = program[i]; }
  sp := ScriptSpend.{b=s[0].tx[0..<n], t=u, ins=s[0].ins[0..<u.nin], outs=s[0].outs[0..<u.nout],
    items=s[0].items[0..<u.nitems], index=0, amount=AMOUNT, spk=s[0].spk[..]};
  return script_verify_vartime(sp, FLAGS, w);
}
name :: (error: int) -> string { return ifx error == SCRIPT_BAD_FLAGS then "BAD_FLAGS" else SCRIPT_ERR_NAMES[error]; }
// Prints a label, bytes as lowercase hex, and a line end.
show :: (label: string, bytes: [] u8) {
  text: [2048] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("% %\n", label, cast(string) text[0..<r.written]);
}
// x-only public key of a secret key: its compressed key without the prefix.
xonly :: (secret: [] u8, out: [] u8) { p: [33] u8; #assert bip32_public(secret, p[..]); for i: 0..<32 { out[i] = p[1 + i]; } }
// A push of 32 bytes at out[at], then op; returns the next position.
push_key :: (out: [] u8, at: int, key: [] u8, op: int) -> int {
  out[at] = 32; for i: 0..<32 { out[at + 1 + i] = key[i]; }
  out[at + 33] = cast(u8) op;
  return at + 34;
}

// The leaves of the tree this example builds, and what spends them.
Tree :: struct {
  secret: [4][32] u8; key: [4][32] u8;   // keys 1 to 3 sign in the leaves; key 0 is the internal key
  script: [3][80] u8; size: [3] int; preimage: [18] u8;
  tree: [1] TrTree; output: [32] u8; parity: int;
  amounts: [1] int; spent: [1] BtScript;
}
// Leaf k's script.
script_of :: (r: [] Tree, k: int) -> [] u8 { return r[0].script[k][0..<r[0].size[k]]; }
// A tapscript signature of leaf k by key j, with a hash type and annex.
sign :: (r: [] Tree, k: int, j: int, hashtype: int, annex: [] u8, out: [] u8) -> int {
  raw: [256] u8; h: [32] u8; aux: [32] u8; sw: [1] SignWork;
  b := bt_unhex(UNSIGNED, raw[..]);
  #assert taproot_script_sighash_vartime(b, 0, r[0].amounts[..], r[0].spent[..], hashtype,
    r[0].script[k][0..<r[0].size[k]], annex, BT_NO_CODESEP, h[..]);
  return taproot_script_sign(r[0].secret[j][..], h[..], bt_unhex(AUX, aux[..]), hashtype, sw[..], out);
}

// Builds the tree: keys 1 to 3 sign in its leaves, key 0 is the internal
// key; leaf 0 at depth 1, leaves 1 and 2 at depth 2.
make_tree :: (r: [] Tree) {
  hashes: [3][32] u8; depths := DEPTHS;
  for j: 0..<4 { for i: 0..<32 { r[0].secret[j][i] = cast(u8) (0x11 * (ifx j == 0 then 4 else j)); } xonly(r[0].secret[j][..], r[0].key[j][..]); }
  r[0].size[0] = push_key(r[0].script[0][..], 0, r[0].key[1][..], OP_CHECKSIG);
  at := push_key(r[0].script[1][..], 0, r[0].key[2][..], OP_CHECKSIG);
  at = push_key(r[0].script[1][..], at, r[0].key[3][..], OP_CHECKSIGADD);
  r[0].script[1][at] = 0x52; r[0].script[1][at + 1] = OP_NUMEQUAL; r[0].size[1] = at + 2;
  for i: 0..<18 { r[0].preimage[i] = PREIMAGE[i]; }
  r[0].script[2][0] = OP_SHA256; r[0].script[2][1] = 32;
  #assert sha256_hash_checked(r[0].preimage[..], r[0].script[2][2..<34], false) == SHA_OK;
  r[0].script[2][34] = OP_EQUALVERIFY;
  r[0].size[2] = push_key(r[0].script[2][..], 35, r[0].key[2][..], OP_CHECKSIG);
  for k: 0..<3 { #assert taproot_leaf_hash_vartime(TR_LEAF_TAPSCRIPT, r[0].script[k][0..<r[0].size[k]], hashes[k][..]) == TR_OK; }
  #assert taproot_tree_vartime(depths[..], hashes[..], r[0].tree[..]) == TR_OK;
  r[0].parity = taproot_output_key_vartime(r[0].key[0][..], r[0].tree[0].root[..], r[0].output[..]);
  r[0].amounts[0] = AMOUNT; r[0].spent[0].n = 34; r[0].spent[0].b[0] = 0x51; r[0].spent[0].b[1] = 32;
  for i: 0..<32 { r[0].spent[0].b[2 + i] = r[0].output[i]; }
}
DEPTHS :: [3] int.[1, 2, 2];

main :: () {
  s: [1] Spend; w: [1] ScriptWork; c: [TR_CONTROL_MAX] u8; wit: [1024] u8; sc: [64] u8; p: [32] u8; q: [34] u8;
  nothing: [1] u8; none := nothing[0..<0];

  // Each leaf of BIP-341's script trees, spent through its control block.
  print("BIP-341's script trees, spent by each leaf\n");
  parity := 0; path := 0; leaves := 0; paths := 0;
  for i: 1..<7 {
    v := TR_SPK[i]; tree: [1] TrTree;
    #assert tr_vector_tree(v, tree[..]) == TR_OK;
    internal := bt_unhex(v.internal, p[..]); program := bt_unhex(v.spk, q[..])[2..<34];
    for k: 0..<v.count {
      leaf := TR_LEAVES[v.first + k]; script := bt_unhex(leaf.script, sc[..]);
      n := taproot_control_block_vartime(tree[..], k, internal, leaf.version, c[..]);
      // A leaf of one key and OP_CHECKSIG gets an empty signature: no one
      // here holds the key, and the script then ends false.
      keyed := script.count == 34 && script[0] == 32 && script[33] == OP_CHECKSIG;
      stack: [1] [] u8; stack[0] = none;
      m := taproot_script_witness_vartime(stack[0..<ifx keyed then 1 else 0], script, c[0..<n], none, wit[..]);
      print("  vector %, leaf % (0x%, % bytes): control block %, %: %\n", i, k, ifx leaf.version == 0xc0 then "c0" else "fa",
            script.count, ifx bt_hex_is(c[0..<n], leaf.control) then "as BIP-341's" else "differs",
            ifx keyed then "an empty signature" else "no stack", name(verify(s[..], wit[0..<m], program, w[..])));
      leaves += 1;
      c[0] ^= 1;
      m = taproot_script_witness_vartime(stack[0..<ifx keyed then 1 else 0], script, c[0..<n], none, wit[..]);
      if verify(s[..], wit[0..<m], program, w[..]) == SCRIPT_ERR_WITNESS_PROGRAM_MISMATCH { parity += 1; }
      c[0] ^= 1;
      if n > 33 {
        paths += 1; c[40] ^= 1;
        m = taproot_script_witness_vartime(stack[0..<ifx keyed then 1 else 0], script, c[0..<n], none, wit[..]);
        if verify(s[..], wit[0..<m], program, w[..]) == SCRIPT_ERR_WITNESS_PROGRAM_MISMATCH { path += 1; }
        c[40] ^= 1;
      }
    }
  }
  print("  the parity bit flipped: % of % WITNESS_PROGRAM_MISMATCH\n", parity, leaves);
  print("  a path hash changed: % of % WITNESS_PROGRAM_MISMATCH\n", path, paths);

  // A tree of three leaves, with keys this example holds.
  r: [1] Tree; make_tree(r[..]);
  print("a tree of our own\n");
  show("  internal key P:", r[0].key[0][..]);
  labels := [3] string.["<key 1> CHECKSIG", "<key 2> CHECKSIG <key 3> CHECKSIGADD 2 NUMEQUAL", "SHA256 <hash> EQUALVERIFY <key 2> CHECKSIG"];
  for k: 0..<3 { print("  leaf %, depth %, % bytes: %\n", k, DEPTHS[k], r[0].size[k], labels[k]); }
  show("  root:", r[0].tree[0].root[..]);
  show("  output key Q:", r[0].output[..]);
  print("  Q's y is %\n", ifx r[0].parity == 1 then "odd" else "even");

  // Spend it by each leaf: the script's stack, the script, the control block.
  print("spending it\n");
  sig: [3][65] u8; stack: [2] [] u8; ctl: [3][TR_CONTROL_MAX] u8; cn: [3] int;
  for k: 0..<3 { cn[k] = taproot_control_block_vartime(r[0].tree[..], k, r[0].key[0][..], TR_LEAF_TAPSCRIPT, ctl[k][..]); }
  n := sign(r[..], 0, 1, BT_DEFAULT, none, sig[0][..]);
  stack[0] = sig[0][0..<n];
  m := taproot_script_witness_vartime(stack[0..<1], script_of(r[..], 0), ctl[0][0..<cn[0]], none, wit[..]);
  print("  leaf 0, % bytes of witness: %\n", m, name(verify(s[..], wit[0..<m], r[0].output[..], w[..])));
  show("   ", wit[0..<m]);
  // Key 2's CHECKSIG runs first and takes the top item, so key 3's
  // signature goes below it.
  n2 := sign(r[..], 1, 2, BT_DEFAULT, none, sig[1][..]); n3 := sign(r[..], 1, 3, BT_DEFAULT, none, sig[2][..]);
  stack[0] = sig[2][0..<n3]; stack[1] = sig[1][0..<n2];
  m = taproot_script_witness_vartime(stack[..], script_of(r[..], 1), ctl[1][0..<cn[1]], none, wit[..]);
  print("  leaf 1, % bytes of witness: %\n", m, name(verify(s[..], wit[0..<m], r[0].output[..], w[..])));
  stack[0] = none;
  m = taproot_script_witness_vartime(stack[..], script_of(r[..], 1), ctl[1][0..<cn[1]], none, wit[..]);
  print("  leaf 1, key 2's signature alone: %\n", name(verify(s[..], wit[0..<m], r[0].output[..], w[..])));
  n2 = sign(r[..], 2, 2, BT_DEFAULT, none, sig[1][..]);
  stack[0] = sig[1][0..<n2]; stack[1] = r[0].preimage[..];
  m = taproot_script_witness_vartime(stack[..], script_of(r[..], 2), ctl[2][0..<cn[2]], none, wit[..]);
  print("  leaf 2, % bytes of witness: %\n", m, name(verify(s[..], wit[0..<m], r[0].output[..], w[..])));
  stack[1] = r[0].preimage[0..<17];
  m = taproot_script_witness_vartime(stack[..], script_of(r[..], 2), ctl[2][0..<cn[2]], none, wit[..]);
  print("  leaf 2, the preimage cut short: %\n", name(verify(s[..], wit[0..<m], r[0].output[..], w[..])));

  // A hash type and an annex, both signed.
  n = sign(r[..], 0, 1, 0x83, none, sig[0][..]); stack[0] = sig[0][0..<n];
  m = taproot_script_witness_vartime(stack[0..<1], script_of(r[..], 0), ctl[0][0..<cn[0]], none, wit[..]);
  print("  leaf 0, signed SINGLE|ANYONECANPAY, % bytes: %\n", n, name(verify(s[..], wit[0..<m], r[0].output[..], w[..])));
  annex := [3] u8.[0x50, 0x04, 0x05];
  n = sign(r[..], 0, 1, BT_DEFAULT, annex[..], sig[0][..]); stack[0] = sig[0][0..<n];
  m = taproot_script_witness_vartime(stack[0..<1], script_of(r[..], 0), ctl[0][0..<cn[0]], annex[..], wit[..]);
  print("  leaf 0, with an annex it signs: %\n", name(verify(s[..], wit[0..<m], r[0].output[..], w[..])));
  m = taproot_script_witness_vartime(stack[0..<1], script_of(r[..], 0), ctl[0][0..<cn[0]], none, wit[..]);
  print("  the same signature, the annex left off: %\n", name(verify(s[..], wit[0..<m], r[0].output[..], w[..])));

  // What a verifier refuses.
  print("what refuses\n");
  n = sign(r[..], 1, 1, BT_DEFAULT, none, sig[0][..]); stack[0] = sig[0][0..<n];
  m = taproot_script_witness_vartime(stack[0..<1], script_of(r[..], 0), ctl[0][0..<cn[0]], none, wit[..]);
  print("  leaf 0, key 1 signed leaf 1's hash: %\n", name(verify(s[..], wit[0..<m], r[0].output[..], w[..])));
  n = sign(r[..], 0, 1, BT_DEFAULT, none, sig[0][..]); stack[0] = sig[0][0..<n];
  m = taproot_script_witness_vartime(stack[0..<1], script_of(r[..], 0), ctl[1][0..<cn[1]], none, wit[..]);
  print("  leaf 0 with leaf 1's control block: %\n", name(verify(s[..], wit[0..<m], r[0].output[..], w[..])));
  ctl[0][0] ^= 1;
  m = taproot_script_witness_vartime(stack[0..<1], script_of(r[..], 0), ctl[0][0..<cn[0]], none, wit[..]);
  print("  leaf 0, the parity bit flipped: %\n", name(verify(s[..], wit[0..<m], r[0].output[..], w[..])));
  ctl[0][0] ^= 1;
  // Leaf 1's path is leaf 2's hash, then leaf 0's. In the other order it
  // builds another root.
  swapped: [TR_CONTROL_MAX] u8;
  for i: 0..<33 { swapped[i] = ctl[1][i]; }
  for i: 0..<32 { swapped[33 + i] = ctl[1][65 + i]; swapped[65 + i] = ctl[1][33 + i]; }
  stack[0] = sig[2][0..<n3]; n2 = sign(r[..], 1, 2, BT_DEFAULT, none, sig[1][..]); stack[1] = sig[1][0..<n2];
  m = taproot_script_witness_vartime(stack[..], script_of(r[..], 1), swapped[0..<cn[1]], none, wit[..]);
  print("  leaf 1, its two path hashes swapped: %\n", name(verify(s[..], wit[0..<m], r[0].output[..], w[..])));
  m = taproot_script_witness_vartime(stack[..], script_of(r[..], 1), ctl[1][0..<cn[1] - 32], none, wit[..]);
  print("  leaf 1, its last path hash dropped: %\n", name(verify(s[..], wit[0..<m], r[0].output[..], w[..])));
  print("  the witness builder, a 64-byte control block: %\n",
        ifx taproot_script_witness_vartime(stack[..], script_of(r[..], 1), ctl[1][0..<64], none, wit[..]) == TR_BAD_CONTROL_SIZE then "refused" else "built");
  annex[0] = 0x51;
  print("  the witness builder, an annex starting 0x51: %\n",
        ifx taproot_script_witness_vartime(stack[..], script_of(r[..], 1), ctl[1][0..<cn[1]], annex[..], wit[..]) == TR_BAD_ANNEX then "refused" else "built");

  // The same output by its key path: one signature by the tweaked key.
  raw: [256] u8; h: [32] u8; aux: [32] u8; tw: [1] BtTapWork;
  b := bt_unhex(UNSIGNED, raw[..]);
  #assert bt_sighash_taproot(b, bt_tx_read(b), 0, r[0].amounts[..], r[0].spent[..], BT_DEFAULT, h[..]);
  #assert bt_taproot_sign(r[0].secret[0][..], r[0].tree[0].root[..], h[..], bt_unhex(AUX, aux[..]), tw[..], sig[0][0..<64], false);
  wit[0] = 1; wit[1] = 64; for i: 0..<64 { wit[2 + i] = sig[0][i]; }
  print("the key path, signed by P tweaked with the root: %\n", name(verify(s[..], wit[0..<66], r[0].output[..], w[..])));
}
```

To run it:

```shell
coax/bin/coax run bitcoin/examples/script-path-spend.coax --run-steps 250000000
```

It prints:

```text
BIP-341's script trees, spent by each leaf
  vector 1, leaf 0 (0xc0, 34 bytes): control block as BIP-341's, an empty signature: EVAL_FALSE
  vector 2, leaf 0 (0xc0, 34 bytes): control block as BIP-341's, an empty signature: EVAL_FALSE
  vector 3, leaf 0 (0xc0, 34 bytes): control block as BIP-341's, an empty signature: EVAL_FALSE
  vector 3, leaf 1 (0xfa, 7 bytes): control block as BIP-341's, no stack: OK
  vector 4, leaf 0 (0xc0, 34 bytes): control block as BIP-341's, an empty signature: EVAL_FALSE
  vector 4, leaf 1 (0xc0, 8 bytes): control block as BIP-341's, no stack: OK
  vector 5, leaf 0 (0xc0, 34 bytes): control block as BIP-341's, an empty signature: EVAL_FALSE
  vector 5, leaf 1 (0xc0, 34 bytes): control block as BIP-341's, an empty signature: EVAL_FALSE
  vector 5, leaf 2 (0xc0, 34 bytes): control block as BIP-341's, an empty signature: EVAL_FALSE
  vector 6, leaf 0 (0xc0, 34 bytes): control block as BIP-341's, an empty signature: EVAL_FALSE
  vector 6, leaf 1 (0xc0, 34 bytes): control block as BIP-341's, an empty signature: EVAL_FALSE
  vector 6, leaf 2 (0xc0, 34 bytes): control block as BIP-341's, an empty signature: EVAL_FALSE
  the parity bit flipped: 12 of 12 WITNESS_PROGRAM_MISMATCH
  a path hash changed: 10 of 10 WITNESS_PROGRAM_MISMATCH
a tree of our own
  internal key P: 2c0b7cf95324a07d05398b240174dc0c2be444d96b159aa6c7f7b1e668680991
  leaf 0, depth 1, 34 bytes: <key 1> CHECKSIG
  leaf 1, depth 2, 70 bytes: <key 2> CHECKSIG <key 3> CHECKSIGADD 2 NUMEQUAL
  leaf 2, depth 2, 69 bytes: SHA256 <hash> EQUALVERIFY <key 2> CHECKSIG
  root: 27213e44acb52ae3698fee9dd7429f61c34f7ea37a47796d0a5d1eede212668e
  output key Q: 4f8fa2db924fd6bb33dbef42dd3bf806dbbe608896cdb4dbd82e4058c750c0a8
  Q's y is odd
spending it
  leaf 0, 167 bytes of witness: OK
    0340d11033a4d4c21a03f4fe6bfda22690f10231847f91908ecd057802e21d1f1b3c560c205af8e0969ec69f6fec69df6bd8bc218dc6c64b209cc4f629a186d9006c22204f355bdcb7cc0af728ef3cceb9615d90684bb5b2ca5f859ab0f0b704075871aaac41c12c0b7cf95324a07d05398b240174dc0c2be444d96b159aa6c7f7b1e6686809919d6a59c460a77716669730a8dffac91fc965511c9776e217b09e80f3ef3eede5
  leaf 1, 300 bytes of witness: OK
  leaf 1, key 2's signature alone: EVAL_FALSE
  leaf 2, 253 bytes of witness: OK
  leaf 2, the preimage cut short: EQUALVERIFY
  leaf 0, signed SINGLE|ANYONECANPAY, 65 bytes: OK
  leaf 0, with an annex it signs: OK
  the same signature, the annex left off: SCHNORR_SIG
what refuses
  leaf 0, key 1 signed leaf 1's hash: SCHNORR_SIG
  leaf 0 with leaf 1's control block: WITNESS_PROGRAM_MISMATCH
  leaf 0, the parity bit flipped: WITNESS_PROGRAM_MISMATCH
  leaf 1, its two path hashes swapped: WITNESS_PROGRAM_MISMATCH
  leaf 1, its last path hash dropped: WITNESS_PROGRAM_MISMATCH
  the witness builder, a 64-byte control block: refused
  the witness builder, an annex starting 0x51: refused
the key path, signed by P tweaked with the root: OK
```

What the output shows:

- BIP-341 publishes the trees and control blocks but not the keys of its
  leaves. Every control block this code builds is BIP-341's, and every
  leaf passes the commitment check. The two leaves without a key spend:
  vector 3's leaf of version `0xfa`, which no soft fork has defined yet,
  passes unrun, and vector 4's leaf, which pushes `Taproot`, leaves a
  true element. The 10 leaves of one key and `OP_CHECKSIG` get an empty
  signature and end false, `EVAL_FALSE`: the script ran, so the
  commitment held.
- Flipping the parity bit fails every leaf, and changing a byte of a
  path hash fails every leaf that has one, with
  `WITNESS_PROGRAM_MISMATCH`.
- The tree of our own puts leaf 0 at depth 1, so its control block holds
  one hash and its witness is 167 bytes: a 64-byte signature, the
  34-byte script and the 65-byte control block, each with its size, after
  the item count.
- The 2-of-2 leaf passes with both signatures and ends false with one.
  The hash-lock leaf passes with the preimage and fails
  `OP_EQUALVERIFY` with one byte short.
- A 65-byte signature carries `SIGHASH_SINGLE|ANYONECANPAY`. A signature
  made with an annex passes with it and fails without it, since the
  annex is signed.
- A signature for leaf 1's hash fails in leaf 0, `SCHNORR_SIG`. Leaf 0
  with leaf 1's control block, a flipped parity bit, leaf 1's path in
  the other order or one hash short all build another output key, so
  they fail before the script runs.
- The same output spends by its key path with one signature by P
  tweaked with the root, without revealing any leaf.

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/script-path-spend.out](../fixtures/script-path-spend.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/script-path-spend.coax](../examples/checks/script-path-spend.coax),
which loads the example.

<!-- source: bitcoin/examples/checks/script-path-spend.coax check -->
```jai
// Each of BIP-341's leaves, with the control block this code builds,
// passes the commitment check: a keyed leaf stops at its empty signature
// (EVAL_FALSE) and the two others pass. Its control block is BIP-341's.
#test "script path: BIP-341's leaves pass the commitment" #steps 3000000000 {
  s: [1] Spend; w: [1] ScriptWork; c: [TR_CONTROL_MAX] u8; wit: [512] u8; sc: [64] u8; p: [32] u8; q: [34] u8;
  nothing: [1] u8; stack: [1] [] u8; stack[0] = nothing[0..<0]; count := 0;
  for i: 1..<7 {
    v := TR_SPK[i]; tree: [1] TrTree;
    #assert tr_vector_tree(v, tree[..]) == TR_OK;
    internal := bt_unhex(v.internal, p[..]); program := bt_unhex(v.spk, q[..])[2..<34];
    for k: 0..<v.count {
      leaf := TR_LEAVES[v.first + k]; script := bt_unhex(leaf.script, sc[..]);
      n := taproot_control_block_vartime(tree[..], k, internal, leaf.version, c[..]);
      #assert bt_hex_is(c[0..<n], leaf.control);
      keyed := script.count == 34 && script[33] == OP_CHECKSIG;
      m := taproot_script_witness_vartime(stack[0..<ifx keyed then 1 else 0], script, c[0..<n], nothing[0..<0], wit[..]);
      #assert verify(s[..], wit[0..<m], program, w[..]) == (ifx keyed then SCRIPT_ERR_EVAL_FALSE else SCRIPT_ERR_OK);
      count += 1;
    }
  }
  #assert count == 12;
}
// A signature commits to its leaf: leaf 0's signature checks out in leaf
// 0 and fails in a leaf with the same key and script but another version
// of the tree, here leaf 0 moved under another internal key.
#test "script path: a signature is bound to its leaf and output" #steps 3000000000 {
  s: [1] Spend; w: [1] ScriptWork; r: [1] Tree; sig: [65] u8; c: [TR_CONTROL_MAX] u8; wit: [512] u8;
  nothing: [1] u8; stack: [1] [] u8;
  make_tree(r[..]);
  n := sign(r[..], 0, 1, BT_DEFAULT, nothing[0..<0], sig[..]); stack[0] = sig[0..<n];
  cn := taproot_control_block_vartime(r[0].tree[..], 0, r[0].key[0][..], TR_LEAF_TAPSCRIPT, c[..]);
  m := taproot_script_witness_vartime(stack[..], script_of(r[..], 0), c[0..<cn], nothing[0..<0], wit[..]);
  #assert verify(s[..], wit[0..<m], r[0].output[..], w[..]) == SCRIPT_ERR_OK;
  // The same leaf under key 1 as the internal key: another output, so the
  // signed message differs and the signature fails there.
  other: [32] u8;
  #assert taproot_output_key_vartime(r[0].key[1][..], r[0].tree[0].root[..], other[..]) >= 0;
  cn = taproot_control_block_vartime(r[0].tree[..], 0, r[0].key[1][..], TR_LEAF_TAPSCRIPT, c[..]);
  m = taproot_script_witness_vartime(stack[..], script_of(r[..], 0), c[0..<cn], nothing[0..<0], wit[..]);
  #assert verify(s[..], wit[0..<m], other[..], w[..]) == SCRIPT_ERR_SCHNORR_SIG;
}
```

To run them:

```shell
coax/bin/coax test bitcoin/examples/checks/script-path-spend.coax --run-steps 250000000 --only "script path:"
```

On 2026-11-03 both checks pass.

### The library's own checks

`taproot.coax` checks the witness builder's layout byte by byte: the
count, an empty item, the annex last, an empty script, a path of one
hash, and a 300-byte item with its three-byte size. It checks what the
builder refuses: control blocks of 0, 32, 34, 64 and 98 bytes, an annex
starting `0x51`, and a buffer one byte short. It checks that the leaf's
signature hash equals chapter 43's with the leaf hash, for three hash
types with an annex and a codesep position, that an undefined hash type
and a cut transaction refuse, and that a signature of 64 or 65 bytes by
an untweaked key verifies with BIP-340. A zero key, a hash type past 255
and a short buffer refuse. Chapter 42's checks of BIP-341's control
blocks still pass.

```shell
coax/bin/coax test coax/lib/bitcoin/taproot.coax --run-steps 250000000
```

On 2026-11-03 it runs 403 checks, with those of the files it loads, all
passing.

### BIP-341's and Core's code in Python

[script_path_examples.py](../../coax/tests/bitcoin/script_path_examples.py)
writes the example's output from code that shares nothing with Coax:
BIP-341's own `taproot_tree_helper` and `taproot_sign_script` build each
tree, control block and witness stack, BIP-340's reference signs,
Bitcoin Core's `TaprootSignatureMsg` gives each message, and chapter 36's
[script_reference.py](../../coax/tests/bitcoin/script_reference.py),
with chapter 44's Taproot rules, verifies each spend. It compares the
result with the fixture line by line.

```shell
python3 coax/tests/bitcoin/script_path_examples.py bitcoin/fixtures/script-path-spend.out
```

On 2026-11-03 all 42 lines agree, the 167-byte witness included, so the
signatures are the same bytes. Mutation testing was not run for this
chapter (owner speed order).

## What this chapter leaves out

- Choosing which leaf to spend and a tree's shape by spending
  probability, and the `tr()` descriptor: chapters 30 and 98.
- Signing in a PSBT, where the leaf script, control block and
  signatures travel between signers: chapter 94.
- Spending with `OP_CODESEPARATOR`: chapter 44's library cases sign at a
  position, and this chapter always signs with none.
- Spends of several inputs, whose signatures commit to every spent
  output: chapter 43's message covers them, and this chapter's
  transaction has one input.

## Summary

- A script-path witness is the leaf's stack, the leaf script, the
  control block and an optional annex.
- A leaf's signature signs the leaf hash, by the key the script names,
  untweaked.
- The stack is listed bottom first, so the first check takes the last
  signature.
- The verifier rebuilds the output key from the control block and the
  script. A wrong parity, path or leaf fails before the script runs.
- One output spends by any leaf, or by the key path with one signature.

## Dependencies

- [42. Script trees: tapleaves and tapbranches](ch042-script-trees-tapleaves-and-tapbranches.md)
- [44. Tapscript](ch044-tapscript.md)

## Sources

- [BIP-341](https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki),
  "Script validation rules", "Constructing and spending Taproot outputs"
  and its Python, pinned at
  [coax/tests/vectors/upstream/bip341/bip-0341.mediawiki](../../coax/tests/vectors/upstream/bip341/bip-0341.mediawiki),
  with its wallet test vectors at
  [wallet-test-vectors.json](../../coax/tests/vectors/upstream/bip341/wallet-test-vectors.json).
- [BIP-342](https://github.com/bitcoin/bips/blob/master/bip-0342.mediawiki),
  "Signature validation".
- [BIP-340](https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki)'s
  reference code, pinned at
  [coax/tests/vectors/upstream/bip340/reference.py](../../coax/tests/vectors/upstream/bip340/reference.py).
- Bitcoin Core at `ddea9196`: `TaprootSignatureMsg` in
  `test/functional/test_framework/script.py`, pinned at
  [coax/tests/vectors/upstream/bitcoin-core/script.py](../../coax/tests/vectors/upstream/bitcoin-core/script.py).
- The pinned files are listed in
  [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
