# 41. The key tweak and the key path

A Taproot output looks like a single-key output: `OP_1` and a 32-byte
x-only public key Q. Q is not the owner's key, though. It is the owner's
*internal key* P plus a multiple of G chosen by a hash of P and, when the
output has spending scripts, of their Merkle root. Anyone who knows P's
secret key can still spend with one signature (the *key path*), and the
scripts stay hidden until someone uses one (the *script path*, chapters 42
and 45).

This chapter builds the tweak on both sides: the public tweak that makes Q
from P, and the secret tweak that makes Q's secret key from P's. Then it
spends with the key path: a BIP-340 signature by the tweaked key, checked
against the Q in the scriptPubKey. Every key, tweak, scriptPubKey, address
and signature in BIP-341's wallet test vectors comes out the same, and
BIP-341's own Python code, pinned, prints the example's output line for
line.

## What this chapter teaches

- The tweak t = H_TapTweak(P ‖ root) and the output key Q = P + tG.
- Q's parity bit: the scriptPubKey keeps only x, so the parity travels in
  the control block of a script-path spend.
- The tweaked secret key: negate d when dG has an odd y, then add t.
- Key-path spending: one signature by the tweaked key, 64 bytes for
  `SIGHASH_DEFAULT` or 65 bytes with a hash type, checked against Q.
- Why the tweak commits to the scripts, and why a key with no scripts is
  still tweaked, with the empty root (BIP-86).
- The two refusals: t ≥ n and Q at infinity.

## Where Bitcoin uses the key tweak

| Where | What it does | Chapter |
| --- | --- | --- |
| A wallet creating a P2TR address | Q from the internal key and the tree's root | 29, 99 |
| A wallet signing a key-path input | the tweaked secret key signs | 94, 99 |
| The script interpreter | checks a key-path witness against Q | 44, 57 |
| A script-path spend | recomputes Q from P, the leaf and the path | 42, 45 |
| MuSig2 and FROST outputs | the group key is the internal key; the tweak is applied after aggregation | 101, 102 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [address.coax](../../coax/lib/bitcoin/address.coax) | `taproot_output_key_vartime(internal, merkle, out)` | t = H_TapTweak(P ‖ root), then Q's x and parity |
| | `taproot_add_tweak_vartime(internal, t, out)` | Q = P + tG for a given t, with BIP-341's refusals |
| | `taproot_tweak_vartime(internal, out)` | the case with no script tree (chapter 29) |
| [sighash.coax](../../coax/lib/bitcoin/sighash.coax) | `bt_internal`, `bt_tweak` | P = dG and the tweaked secret key d + t, in constant time |
| | `bt_taproot_sign` | a key-path signature through chapter 24's signer |
| [taproot.coax](../../coax/lib/bitcoin/taproot.coax) | `taproot_sig_hashtype_vartime(sig)` | the hash type a key-path signature carries |
| | `taproot_key_path_verify_vartime(q, sig, sighash)` | the key-path check against Q (chapter 23's verifier) |

The public tweak runs in variable time: P, the root, t and Q are public.
The secret tweak doesn't branch or index on d or on d + t (chapter 3).
`bt_taproot_sign` and its tweak came first, for Nostr chapter 104's agent
treasuries. This chapter teaches them as they are, and adds the general
public tweak and the verifier's side.

## The output key

BIP-341 builds Q in three steps:

1. P is `lift_x(x)`: the point with x coordinate x and an even y
   (chapter 17). An internal key is always x-only, 32 bytes.
2. t is `H_TapTweak(x ‖ root)`, the BIP-341 tagged hash (chapter 5) of
   P's 32 bytes followed by the script tree's Merkle root. With no script
   tree the root is empty, so the hash input is 32 bytes, not 64.
3. Q = P + tG. The output key is Q's x, and the scriptPubKey is `OP_1`,
   a push of 32 bytes, and that x: `5120` and 64 hex digits.

<!-- source: coax/lib/bitcoin/address.coax commit -->
```jai
// BIP-341's output key (chapter 41): P = lift_x(internal),
// t = H_TapTweak(internal || merkle) and Q = P + tG. merkle is empty for a
// key with no script tree, or a script tree's 32-byte Merkle root (chapter
// 42). Writes Q's x (32 bytes) and returns Q's y parity, 0 or 1, which a
// script-path spend reveals in its control block.
taproot_output_key_vartime :: (internal: [] u8, merkle: [] u8, output: [] u8) -> int {
  if output.count < SE_XONLY { return AD_CAPACITY; }
  if internal.count != SE_XONLY { return AD_BAD_KEY; }
  if merkle.count != 0 && merkle.count != 32 { return AD_BAD_ROOT; }
  m: [64] u8; t: [32] u8;
  for i: 0..<32 { m[i] = internal[i]; }
  for i: 0..<merkle.count { m[32 + i] = merkle[i]; }
  #assert bitcoin_tagged_checked(BH_TAPTWEAK, m[0..<32 + merkle.count], t[..], false) == SHA_OK;
  return taproot_add_tweak_vartime(internal, t[..], output);
}
// Q = lift_x(internal) + tG for the 32-byte big-endian number t. BIP-341
// refuses t >= n and Q at infinity. A TapTweak hash reaches neither for
// any known key, but a t given directly reaches both.
taproot_add_tweak_vartime :: (internal: [] u8, t: [] u8, output: [] u8) -> int {
  if output.count < SE_XONLY { return AD_CAPACITY; }
  if internal.count != SE_XONLY || t.count != 32 { return AD_BAD_KEY; }
  p := se_decode_xonly(internal);
  if !p.ok { return AD_BAD_KEY; }
  tweak := sc_load(t);
  if !tweak.ok { return AD_BAD_TWEAK; }
  q := ecmult_vartime(tweak.value, p.value, sc_small(1));   // tG + 1P
  c: [33] u8;
  if se_encode(q.value, SE_COMPRESSED, c[..]) != SE_COMPRESSED { return AD_BAD_TWEAK; }
  for i: 0..<32 { output[i] = c[1 + i]; }
  return cast(int) (c[0] & 1);
}
AD_BAD_ROOT :: -9;          // a Merkle root that is neither empty nor 32 bytes
```

`taproot_add_tweak_vartime` computes tG + 1P in one two-scalar
multiplication, `ecmult_vartime`, the one Schnorr verification uses. It
returns the parity of Q's y as 0 or 1, and refusals as negative codes,
so one `int` carries both.

Chapter 29's `taproot_tweak_vartime` is now this procedure with an empty
root. Its listing in chapter 29 shows the call.

### The parity bit

The scriptPubKey keeps Q's x and drops its y. A key-path spend doesn't
need the y: BIP-340 verification lifts x to the even-y point, and the
signer makes its key fit that (below). A script-path spend does need it.
The spender reveals P and the path, the verifier recomputes Q = P + tG,
and that Q has a definite y. The verifier compares both x and the parity,
which the spender puts in the low bit of the control block's first byte
(chapter 42). Without the parity check, one output key would have two
valid control blocks for each leaf.

### Worked example: BIP-86's first address

BIP-86's `m/86'/0'/0'/0/0` has the internal key
`cc8a4bc64d897bddc5fbc2f670f7a8ba0b386779106cf1223c6fc5d7cd6fc115` and no
script tree:

1. t = H_TapTweak(P) is
   `2ca01ed85cf6b6526f73d39a1111cd80333bfdc00ce98992859848a90a6f0258`.
   The hash input is P's 32 bytes alone.
2. Q = P + tG has the x
   `a60869f0dbcf1dc659c9cecbaf8050135ea9e8cdc487053f1dc6880949dc684c` and
   an odd y. The parity bit is 1.
3. The scriptPubKey is `5120a608…684c`, and the address is
   `bc1p5cyxnuxmeuwuvkwfem96lqzszd02n6xdcjrs20cac6yqjjwudpxqkedrcr`.

## Why the tweak commits to the scripts

Q depends on the root through t, and t is a hash. To spend by a script,
the spender shows a leaf and a path that hash to some root r with
P + H_TapTweak(P ‖ r)G = Q. Finding a second root r′ that gives the same
Q means finding a collision in the tweak, which is as hard as breaking
SHA-256. So an output commits to exactly one script tree when it is
created, though nobody can see the tree.

The example flips one bit of a vector's root. The tweak changes
completely, and so does Q:

```text
  t = H_TapTweak(P || root): 9e0517edc8259bb3359255400b23ca9507f2a91cd1e4250ba068b4eafceba4a9
  Q = P + tG, x: 712447206d7a5238acc7ff53fbe94a3b64539ad291c7cdbc490b7577e4b17df5
  t = H_TapTweak(P || root): 1270e8aae02b2b67d7baef387fe07ceb5b6026beb6009a33098df7c850e19c60
  Q = P + tG, x: c695f0de0cd4b9b9575334e12582e380276eec8110419c2df012f457e5d42a93
```

### Why a key with no scripts is still tweaked

A single-key wallet could put P itself in the output. BIP-341 and BIP-86
tweak it anyway, with the empty root. The reason is P's origin. If P is an
aggregate of several parties' keys (MuSig2, chapter 101), one party could
choose its share so that P = P₀ + H_TapTweak(P₀ ‖ r)G for a script tree r
that only it knows. The others would sign for P without knowing that a
hidden script also spends it. Tweaking with the empty root makes P's
owners commit to "no scripts": any hidden tree would need a second tweak,
which is again a hash collision.

The empty root and a root of 32 zero bytes are different commitments: the
first hashes 32 bytes, the second 64. The checks confirm that they give
different keys.

## The tweaked secret key

To sign for Q, the signer needs Q's secret key. Its internal secret key d
has the public key dG, but BIP-340 x-only keys always mean the even-y
point, so P = lift_x(x(dG)) is dG only when dG has an even y. Otherwise
P = −dG, whose secret key is n − d. So:

1. Compute dG. If its y is odd, replace d with n − d.
2. t = H_TapTweak(x(P) ‖ root), the same t as the public side.
3. d′ = d + t mod n. Then d′G = P + tG = Q.

BIP-340 signing negates d′ again if Q has an odd y; chapter 24's signer
does that step, so the tweak doesn't have to.

<!-- source: coax/lib/bitcoin/sighash.coax tweak -->
```jai
// The internal key P = dG (public, into w.internal), then the tweak hash
// TapTweak(P || merkle) into th.
bt_internal :: (key:[]u8,merkle:[]u8,w:[]BtTapWork,th:[]u8) {
  #assert ss_key(key,w[0].key[..],false) && sp_base(w[0].key[0].safe,w[0].point[..]) && sp_x(w[0].point[..],w[0].internal[..]);
  m:[64]u8;n:=bt_put(m[..],0,w[0].internal[..]);
  bt_tagged(BH_TAPTWEAK,m[0..<bt_put(m[..],n,merkle)],th);
}
// The tweaked key d' + t into w.tweaked, where d' is d or n - d so that P
// has an even y and t is the tweak hash th; false when t >= n. valid is
// the masked result: d in [1, n-1] and d' + t not zero.
bt_tweak :: (th:[]u8,w:[]BtTapWork)->bool {
  t:=sc_load(th);                                       // public: from the public key
  tweaked:=sc_add(ss_even(w[0].key[0].safe,w[0].point[0].odd&1),t.value);
  #assert ss_write(tweaked,w[0].tweaked[..]);
  w[0].valid=w[0].key[0].valid & w[0].point[0].finite & ec_nonzero(tweaked);
  return t.ok;
}
```

`ss_even` selects d or n − d by the parity mask without a branch, and
`valid` is a mask too: it is 1 only when d is in [1, n − 1], P is finite,
and d + t is not zero. d + t = 0 is the secret side of Q at infinity.
`bt_tweak` returns whether t is below n, which depends only on public
values.

Three of BIP-341's seven key-path inputs have a dG with an odd y:

```text
  input 0: dG has an even y, so keep d; d' = d + t matches BIP-341
  input 3: dG has an odd y, so negate d; d' = d + t matches BIP-341
```

## Spending with the key path

A key-path witness is one element, the signature:

| Length | Hash type | Signed message |
| --- | --- | --- |
| 64 bytes | `SIGHASH_DEFAULT` (0x00) | the BIP-341 signature hash with type 0 |
| 65 bytes | the last byte, which must not be 0x00 | the BIP-341 signature hash with that type |
| anything else | — | invalid |

A 65-byte signature ending in 0x00 is invalid. `SIGHASH_DEFAULT` has
exactly one encoding, 64 bytes, so a third party can't add a byte to a
signature and change the transaction's witness. Chapter 43 builds the
signature hash and refuses the hash types BIP-341 doesn't define.

The signer computes the signature hash, then signs it with d′:

<!-- source: coax/lib/bitcoin/sighash.coax taproot -->
```jai
// The BIP-340 signature of a key-path sighash by the key tweaked with
// merkle (empty for a key with no script tree, else 32 bytes), with aux
// as BIP-340's auxiliary randomness, into out (64 bytes; a hash type other
// than SIGHASH_DEFAULT is appended by the caller). Views are stable and
// disjoint; the workspace is cleared on every return. schnorr_sign_step
// takes exactly six calls to publish; a refusal clears its workspace, and
// the calls left can't reach a signature.
bt_taproot_sign :: (key:[]u8,merkle:[]u8,sighash:[]u8,aux:[]u8,w:[]BtTapWork,out:[]u8,overlap:bool)->bool {
  if w.count!=1 || key.count!=32 || (merkle.count!=0 && merkle.count!=32) || sighash.count!=32 || aux.count!=32 || out.count!=64 || overlap {return false;}
  th:[32]u8;bt_internal(key,merkle,w,th[..]);
  if !bt_tweak(th[..],w) || !ec_declassify(w[0].valid) {return bt_tap_clear(w);}
  status:=schnorr_sign_step(w[0].tweaked[..],aux,sighash,w[0].sign[..],out,false);
  for k:1..<6 {status=schnorr_sign_step(w[0].tweaked[..],aux,sighash,w[0].sign[..],out,false);}
  return bt_tap_clear(w) || status==SIGN_DONE;
}
```

The verifier reads the hash type, computes the signature hash with it,
and checks the BIP-340 signature against Q, the 32 bytes after `5120` in
the spent scriptPubKey:

<!-- source: coax/lib/bitcoin/taproot.coax keypath -->
```jai
TR_BAD_SIZE :: -1;          // a key-path signature that is neither 64 nor 65 bytes
TR_BAD_HASHTYPE :: -2;      // 65 bytes ending in 0x00
// The hash type of a key-path signature: 64 bytes sign with
// SIGHASH_DEFAULT (0), and 65 bytes carry the type in the last byte. That
// byte must not be 0x00, because SIGHASH_DEFAULT is written by leaving it
// out: one hash type, one encoding.
taproot_sig_hashtype_vartime :: (sig: [] u8) -> int {
  if sig.count == 64 { return BT_DEFAULT; }
  if sig.count != 65 { return TR_BAD_SIZE; }
  if sig[64] == 0 { return TR_BAD_HASHTYPE; }
  return cast(int) sig[64];
}
// A key-path spend: the witness is one signature, and its first 64 bytes
// must be a BIP-340 signature of sighash by the output key Q (the 32 bytes
// after OP_1 0x20 in the spent scriptPubKey). The caller computes sighash
// with the hash type above (chapter 43), which refuses a type BIP-341
// doesn't define.
taproot_key_path_verify_vartime :: (output_key: [] u8, sig: [] u8, sighash: [] u8) -> bool {
  if output_key.count != 32 || sighash.count != 32 || taproot_sig_hashtype_vartime(sig) < 0 { return false; }
  return schnorr_verify_vartime(sighash, output_key, sig[0..<64]);
}
```

Nothing in the witness names P. A key-path spend looks the same whether
the output has a script tree or not, and whether P is one key or an
aggregate of many.

## Refusals

| Case | Where | Code |
| --- | --- | --- |
| t ≥ n | public tweak | `AD_BAD_TWEAK` |
| Q at infinity (tG = −P) | public tweak | `AD_BAD_TWEAK` |
| x not on the curve, or not 32 bytes | public tweak | `AD_BAD_KEY` |
| a root that is neither empty nor 32 bytes | public tweak | `AD_BAD_ROOT` |
| t ≥ n | secret tweak | `bt_tweak` returns false |
| d + t = 0 | secret tweak | `valid` is 0, and `bt_taproot_sign` refuses |
| a signature of 65 bytes ending in 0x00 | verifier | `TR_BAD_HASHTYPE` |
| a signature that is neither 64 nor 65 bytes | verifier | `TR_BAD_SIZE` |

A tagged hash is at least n with probability below 2⁻¹²⁷, and Q at
infinity needs H_TapTweak(P ‖ root)G = −P. No known key reaches either.
`taproot_add_tweak_vartime` takes t directly, so the example and the
checks reach both: t = n refuses, and P = G with t = n − 1 gives
G + (n − 1)G, which is infinity.

## The example

The example is
[bitcoin/examples/taproot-key-path.coax](../examples/taproot-key-path.coax).
It tweaks BIP-86's first key, builds the scriptPubKey and address of each
of BIP-341's seven vectors from their roots, flips one bit of a root,
tweaks the seven key-path inputs' secret keys, signs each input of
BIP-341's transaction and checks each signature against Q and P, and
shows the refusals.

<!-- source: bitcoin/examples/taproot-key-path.coax example -->
```jai
#load "../../coax/lib/bitcoin/taproot.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [512] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// Print one byte as two hex digits.
hex_byte :: (v: int) {
  b: [1] u8; text: [2] u8; b[0] = cast(u8) v;
  r := hex_encode_checked(cast(string) b[..], text[..], false);
  print("%", cast(string) text[0..<r.written]);
}
REFUSALS :: [9] string.["bad network", "wrong network", "no address", "bad key", "uncompressed key",
                        "script too long", "bad tweak", "capacity", "bad root"];
// A parity or one of address.coax's refusals.
outcome :: (code: int) -> string {
  if code == 0 { return "y even"; }
  if code == 1 { return "y odd"; }
  if code < 0 && code >= -9 { return REFUSALS[-code - 1]; }
  return "?";
}
// P, t and Q for an internal key and a Merkle root (empty: no script tree).
commit :: (internal: [] u8, merkle: [] u8) {
  m: [64] u8; t: [32] u8; q: [32] u8;
  for i: 0..<32 { m[i] = internal[i]; }
  for i: 0..<merkle.count { m[32 + i] = merkle[i]; }
  #assert bitcoin_tagged_checked(BH_TAPTWEAK, m[0..<32 + merkle.count], t[..], false) == SHA_OK;
  show("  t = H_TapTweak(P || root)", t[..]);
  parity := taproot_output_key_vartime(internal, merkle, q[..]);
  show("  Q = P + tG, x", q[..]);
  print("  Q's %\n", outcome(parity));
}

main :: () {
  b1: [32] u8; b2: [32] u8; q: [32] u8; s: [34] u8; a: [90] u8;

  // The key path with no script tree: BIP-86's first receiving key.
  print("no script tree: BIP-86 m/86'/0'/0'/0/0\n");
  internal := bt_unhex("cc8a4bc64d897bddc5fbc2f670f7a8ba0b386779106cf1223c6fc5d7cd6fc115", b1[..]);
  show("  internal key P", internal);
  commit(internal, internal[0..<0]);
  #assert taproot_output_key_vartime(internal, internal[0..<0], q[..]) >= 0 && ad_witness_script(1, q[..], s[..]) == 34;
  show("  scriptPubKey", s[..]);
  n := address_encode_vartime(SW_MAINNET, s[..], a[..]);
  print("  address: %\n", cast(string) a[0..<n]);

  // BIP-341's seven scriptPubKey vectors, from the given Merkle roots.
  print("BIP-341 scriptPubKey vectors\n");
  for i: 0..<TR_SPK.count {
    v := TR_SPK[i];
    p := bt_unhex(v.internal, b1[..]); root := bt_unhex(v.root, b2[..]);
    parity := taproot_output_key_vartime(p, root, q[..]);
    #assert ad_witness_script(1, q[..], s[..]) == 34;
    n = address_encode_vartime(SW_MAINNET, s[..], a[..]);
    print("  vector %: ", i);
    if v.count == 0 { print("no script tree"); } else if v.count == 1 { print("1 leaf"); } else { print("% leaves", v.count); }
    print(", Q's %, scriptPubKey %, address %\n", outcome(parity),
          ifx bt_hex_is(s[..], v.spk) then "matches" else "differs",
          ifx cast(string) a[0..<n] == v.address then "matches" else "differs");
  }

  // Q commits to the root: one bit of it changes Q.
  print("the commitment: vector 3 with its root, then one bit of the root changed\n");
  p := bt_unhex(TR_SPK[3].internal, b1[..]); root := bt_unhex(TR_SPK[3].root, b2[..]);
  commit(p, root);
  root[31] ^= 1;
  commit(p, root);

  // The tweaked secret key, for BIP-341's seven key-path inputs.
  print("the tweaked secret key\n");
  for i: 0..<BT_V341.count {
    v := BT_V341[i]; k: [32] u8; m: [32] u8; th: [32] u8; w: [1] BtTapWork;
    bt_internal(bt_unhex(v.key, k[..]), bt_unhex(v.merkle, m[..]), w[..], th[..]);
    ok := bt_tweak(th[..], w[..]);
    print("  input %: dG has %, so % d; d' = d + t ", v.index,
          ifx (w[0].point[0].odd & 1) == 1 then "an odd y" else "an even y",
          ifx (w[0].point[0].odd & 1) == 1 then "negate" else "keep");
    print("%\n", ifx ok && ec_declassify(w[0].valid) && bt_hex_is(w[0].tweaked[..], v.tweaked) then "matches BIP-341" else "differs");
    #assert !bt_tap_clear(w[..]);
  }

  // Sign each input with the tweaked key, and verify against the output
  // key in the scriptPubKey it spends.
  print("key-path spends of BIP-341's transaction\n");
  tx: [BT_TX] u8; spk: [9] BtScript;
  raw := bt_unhex(BT_V341_TX, tx[..]); t := bt_tx_read(raw);
  amounts := BT_V341_AMOUNT;
  for k: 0..<9 { spk[k] = bt_script(BT_V341_SPK[k]); }
  for i: 0..<BT_V341.count {
    v := BT_V341[i]; k: [32] u8; m: [32] u8; h: [32] u8; aux: [32] u8; w: [1] BtTapWork; sig: [65] u8; e: [65] u8;
    #assert bt_sighash_taproot(raw, t, v.index, amounts[..], spk[..], v.hashtype, h[..]);
    #assert bt_taproot_sign(bt_unhex(v.key, k[..]), bt_unhex(v.merkle, m[..]), h[..], aux[..], w[..], sig[0..<64], false);
    sig[64] = cast(u8) v.hashtype;
    signed := sig[0..<ifx v.hashtype == 0 then 64 else 65];
    q2 := spk[v.index].b[2..<34];
    print("  input % (hash type 0x", v.index);
    hex_byte(v.hashtype);
    print("), % bytes: witness %, verifies with Q %, with P %\n", signed.count,
          ifx bt_same(signed, bt_unhex(v.witness, e[..])) then "matches" else "differs",
          taproot_key_path_verify_vartime(q2, signed, h[..]),
          taproot_key_path_verify_vartime(bt_unhex(v.internal, b1[..]), signed, h[..]));
  }

  // What BIP-341 refuses.
  print("refusals\n");
  g := bt_unhex("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798", b1[..]);
  tw := bt_unhex("fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141", b2[..]);
  print("  t = n: %\n", outcome(taproot_add_tweak_vartime(g, tw, q[..])));
  tw[31] = 0x40;
  print("  P = G, t = n - 1, so Q is infinity: %\n", outcome(taproot_add_tweak_vartime(g, tw, q[..])));
  print("  a 31-byte root: %\n", outcome(taproot_output_key_vartime(g, tw[0..<31], q[..])));
  five: [32] u8; five[31] = 5;
  print("  x = 5, not on the curve: %\n", outcome(taproot_output_key_vartime(five[..], five[0..<0], q[..])));
  k: [32] u8; w: [1] BtTapWork; th: [32] u8; k[31] = 1;
  bt_internal(k[..], k[0..<0], w[..], th[..]);
  for i: 0..<32 { th[i] = 0xff; }
  print("  the secret tweak with t = 2^256 - 1: %\n", ifx bt_tweak(th[..], w[..]) then "accepted" else "refused");
  #assert !bt_tap_clear(w[..]);
  sig: [65] u8;
  print("  a 65-byte signature ending in 0x00: hash type %\n", taproot_sig_hashtype_vartime(sig[..]));
  print("  a 63-byte signature: hash type %\n", taproot_sig_hashtype_vartime(sig[0..<63]));
}
```

To run it:

```shell
coax/bin/coax run bitcoin/examples/taproot-key-path.coax --run-steps 250000000
```

It prints:

```text
no script tree: BIP-86 m/86'/0'/0'/0/0
  internal key P: cc8a4bc64d897bddc5fbc2f670f7a8ba0b386779106cf1223c6fc5d7cd6fc115
  t = H_TapTweak(P || root): 2ca01ed85cf6b6526f73d39a1111cd80333bfdc00ce98992859848a90a6f0258
  Q = P + tG, x: a60869f0dbcf1dc659c9cecbaf8050135ea9e8cdc487053f1dc6880949dc684c
  Q's y odd
  scriptPubKey: 5120a60869f0dbcf1dc659c9cecbaf8050135ea9e8cdc487053f1dc6880949dc684c
  address: bc1p5cyxnuxmeuwuvkwfem96lqzszd02n6xdcjrs20cac6yqjjwudpxqkedrcr
BIP-341 scriptPubKey vectors
  vector 0: no script tree, Q's y odd, scriptPubKey matches, address matches
  vector 1: 1 leaf, Q's y odd, scriptPubKey matches, address matches
  vector 2: 1 leaf, Q's y even, scriptPubKey matches, address matches
  vector 3: 2 leaves, Q's y even, scriptPubKey matches, address matches
  vector 4: 2 leaves, Q's y odd, scriptPubKey matches, address matches
  vector 5: 3 leaves, Q's y even, scriptPubKey matches, address matches
  vector 6: 3 leaves, Q's y odd, scriptPubKey matches, address matches
the commitment: vector 3 with its root, then one bit of the root changed
  t = H_TapTweak(P || root): 9e0517edc8259bb3359255400b23ca9507f2a91cd1e4250ba068b4eafceba4a9
  Q = P + tG, x: 712447206d7a5238acc7ff53fbe94a3b64539ad291c7cdbc490b7577e4b17df5
  Q's y even
  t = H_TapTweak(P || root): 1270e8aae02b2b67d7baef387fe07ceb5b6026beb6009a33098df7c850e19c60
  Q = P + tG, x: c695f0de0cd4b9b9575334e12582e380276eec8110419c2df012f457e5d42a93
  Q's y odd
the tweaked secret key
  input 0: dG has an even y, so keep d; d' = d + t matches BIP-341
  input 1: dG has an even y, so keep d; d' = d + t matches BIP-341
  input 3: dG has an odd y, so negate d; d' = d + t matches BIP-341
  input 4: dG has an even y, so keep d; d' = d + t matches BIP-341
  input 6: dG has an odd y, so negate d; d' = d + t matches BIP-341
  input 7: dG has an even y, so keep d; d' = d + t matches BIP-341
  input 8: dG has an odd y, so negate d; d' = d + t matches BIP-341
key-path spends of BIP-341's transaction
  input 0 (hash type 0x03), 65 bytes: witness matches, verifies with Q true, with P false
  input 1 (hash type 0x83), 65 bytes: witness matches, verifies with Q true, with P false
  input 3 (hash type 0x01), 65 bytes: witness matches, verifies with Q true, with P false
  input 4 (hash type 0x00), 64 bytes: witness matches, verifies with Q true, with P false
  input 6 (hash type 0x02), 65 bytes: witness matches, verifies with Q true, with P false
  input 7 (hash type 0x82), 65 bytes: witness matches, verifies with Q true, with P false
  input 8 (hash type 0x81), 65 bytes: witness matches, verifies with Q true, with P false
refusals
  t = n: bad tweak
  P = G, t = n - 1, so Q is infinity: bad tweak
  a 31-byte root: bad root
  x = 5, not on the curve: bad key
  the secret tweak with t = 2^256 - 1: refused
  a 65-byte signature ending in 0x00: hash type -2
  a 63-byte signature: hash type -1
```

What the output shows:

- BIP-86's address comes from the empty root.
- Each vector's scriptPubKey and address match BIP-341's. Q's y is odd
  for four of the seven: the parity bit is not always 0.
- One bit of the root gives a different t and Q.
- Each tweaked secret key matches BIP-341's `tweakedPrivkey`, with d
  negated for the three inputs whose dG has an odd y.
- Each signature matches the published witness byte for byte (BIP-341's
  vectors sign with 32 zero bytes of auxiliary randomness), verifies
  against Q, and doesn't verify against P.

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/taproot-key-path.out](../fixtures/taproot-key-path.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/taproot-key-path.coax](../examples/checks/taproot-key-path.coax),
which loads the example.

<!-- source: bitcoin/examples/checks/taproot-key-path.coax check -->
```jai
// The secret and public tweaks agree: d' = d + t (with d negated for an odd
// y) has d'G = Q, the output key the public tweak gives, for every key-path
// input of BIP-341's vectors.
#forall "key-path: the tweaked secret key's public key is the output key" i: 0..<7 #steps 2000000000 {
  v := BT_V341[i]; k: [32] u8; m: [32] u8; th: [32] u8; w: [1] BtTapWork; p: [32] u8; q: [32] u8; e: [32] u8;
  bt_internal(bt_unhex(v.key, k[..]), bt_unhex(v.merkle, m[..]), w[..], th[..]);
  #assert bt_hex_is(th[..], v.tweak) && bt_tweak(th[..], w[..]) && w[0].valid == 1;
  #assert bt_same(w[0].internal[..], bt_unhex(v.internal, p[..]));
  #assert taproot_output_key_vartime(p[..], bt_unhex(v.merkle, m[..]), q[..]) >= 0;
  d: [1] SignScalar; dq: [1] SignPoint; x: [32] u8;
  #assert ss_key(w[0].tweaked[..], d[..], false) && sp_base(d[0].safe, dq[..]) && sp_x(dq[..], x[..]);
  #assert bt_same(x[..], q[..]) && bt_hex_is(w[0].tweaked[..], v.tweaked);
  #assert !bt_tap_clear(w[..]) && w[0].tweaked[0] == 0;
}
// Q commits to the root: with one bit of vector 3's root flipped (bit k
// of byte 4k, for k from 0 to 7), Q changes.
#forall "key-path: one bit of the Merkle root changes Q" bit: 0..<8 #steps 2500000000 {
  v := TR_SPK[3]; p: [32] u8; m: [32] u8; q: [32] u8; q2: [32] u8;
  pk := bt_unhex(v.internal, p[..]); root := bt_unhex(v.root, m[..]);
  #assert taproot_output_key_vartime(pk, root, q[..]) >= 0 && bt_hex_is(q[..], v.tweaked);
  root[bit * 4] ^= cast(u8) (1 << bit);
  #assert taproot_output_key_vartime(pk, root, q2[..]) >= 0 && !bt_same(q[..], q2[..]);
}
// The no-script-tree tweak is the general one with an empty root, and
// differs from committing to a root of 32 zero bytes.
#test "key-path: an empty root is not a zero root" #steps 600000000 {
  p: [32] u8; z: [32] u8; a: [32] u8; b: [32] u8; c: [32] u8;
  pk := bt_unhex("cc8a4bc64d897bddc5fbc2f670f7a8ba0b386779106cf1223c6fc5d7cd6fc115", p[..]);
  #assert taproot_tweak_vartime(pk, a[..]) == 1 && taproot_output_key_vartime(pk, z[0..<0], b[..]) == 1 && bt_same(a[..], b[..]);
  #assert taproot_output_key_vartime(pk, z[..], c[..]) >= 0 && !bt_same(a[..], c[..]);
}
```

To run them:

```shell
coax/bin/coax test bitcoin/examples/checks/taproot-key-path.coax --run-steps 250000000 --only key-path:
```

On 2026-11-03 the three checks pass: 7 tweaked keys whose public key is
Q, 8 flipped root bits that each change Q, and the empty root against the
zero root.

### The library's own checks

`taproot.coax` checks all seven scriptPubKey vectors (Q, its parity
against chapter 29's tweak when there is no tree, the scriptPubKey and
the address), the refusals with t given directly, the hash-type rule, and
all seven key-path spends: each published witness verifies against the Q
in the spent scriptPubKey with the sighash for its own hash type, and
refuses against P, with a byte missing, with a 0x00 hash type and with a
changed sighash. `sighash.coax` checks the tweak, the tweaked key and
each signature against BIP-341's intermediate values, and the signer's
refusals.

```shell
coax/bin/coax test coax/lib/bitcoin/taproot.coax --run-steps 250000000
```

### BIP-341's own Python

[taproot_reference.py](../../coax/tests/bitcoin/taproot_reference.py)
loads BIP-340's `reference.py` and the Python functions in BIP-341's text
(`taproot_tweak_pubkey`, `taproot_tweak_seckey`, `taproot_tree_helper`,
`taproot_output_script`, `taproot_sign_key` and `taproot_sign_script`),
both pinned, and runs them as written. Run alone, it checks itself against
all 14 wallet test vectors.
[taproot_examples.py](../../coax/tests/bitcoin/taproot_examples.py)
writes the example's whole output from that code and compares it with the
fixture:

```shell
python3 coax/tests/bitcoin/taproot_reference.py
python3 coax/tests/bitcoin/taproot_examples.py key-path bitcoin/fixtures/taproot-key-path.out
```

On 2026-11-03 all 46 lines agree. Mutation testing was not run for this
chapter (owner speed order).

## What this chapter leaves out

- Building a script tree and its root, leaf hashes and control blocks:
  chapter 42.
- The BIP-341 signature hash itself, every field and hash type: chapter
  43.
- Running a script-path spend's script: chapters 44 and 45.
- Aggregate internal keys and tweaking them: MuSig2 and FROST, chapters
  101 and 102.

## Summary

- Q = P + H_TapTweak(P ‖ root)G, with an empty root when there are no
  scripts. The scriptPubKey is `OP_1 <Q's x>`.
- Q's parity is not in the output; a script-path spend reveals it.
- The secret key for Q is (d or n − d, whichever gives P) + t.
- A key-path witness is one signature by that key: 64 bytes for
  `SIGHASH_DEFAULT`, or 65 bytes with a nonzero hash type.
- The tweak commits to one script tree, and the empty-root tweak proves
  there is none.
- BIP-341 refuses t ≥ n and Q at infinity; only a t given directly reaches
  them.

## Dependencies

- [5. SHA-256d and tagged hashes](ch005-sha-256d-and-tagged-hashes.md)
- [24. Sign with Schnorr](ch024-sign-with-schnorr.md)

## Sources

- [BIP-341](https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki),
  pinned at
  [coax/tests/vectors/upstream/bip341/bip-0341.mediawiki](../../coax/tests/vectors/upstream/bip341/bip-0341.mediawiki),
  with its wallet test vectors at
  [wallet-test-vectors.json](../../coax/tests/vectors/upstream/bip341/wallet-test-vectors.json).
  Bitcoin Core's `src/test/data/bip341_wallet_vectors.json` at `ddea9196`
  is the same file, byte for byte.
- BIP-340's reference code, pinned at
  [coax/tests/vectors/upstream/bip340/reference.py](../../coax/tests/vectors/upstream/bip340/reference.py).
- [BIP-86](https://github.com/bitcoin/bips/blob/master/bip-0086.mediawiki),
  pinned at
  [coax/tests/vectors/upstream/bip86/bip-0086.mediawiki](../../coax/tests/vectors/upstream/bip86/bip-0086.mediawiki).
- The pinned files are listed in
  [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
