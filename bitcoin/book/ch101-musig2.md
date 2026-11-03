# 101. MuSig2

Chapter 100's multisig puts every key and every signature on the chain:
a 2-of-3 spend shows three keys and two signatures, and anyone can see
that the coins needed more than one signer. MuSig2 lets n signers make
one ordinary BIP-340 signature under one ordinary x-only key. The
signers aggregate their keys into one key Q, and each contributes a
partial signature. The partial signatures add up to a signature that
chapter 23's verifier accepts under Q. On the chain, a MuSig2 spend looks
like any key-path spend of chapter 41: 64 bytes of signature and a
32-byte key. It is n-of-n: every signer must sign.

This chapter builds BIP-327 in full: key aggregation, tweaking, nonce
generation, nonce aggregation, signing, partial signature verification,
aggregation and deterministic signing. Every case of BIP-327's eight
vector files, refusals included, gives its stated result, and BIP-327's
own Python reference, pinned, prints the example's output line for line.
A secret nonce signs once; the example then shows what a copied one
costs: three signatures give the secret key away.

## What this chapter teaches

- Key aggregation: Q = a_1 P_1 + … + a_n P_n, why each key gets a hashed
  coefficient, and why the second distinct key's coefficient is 1.
- Tweaks: the plain and the x-only tweak, and how the signers sign for a
  tweaked key with the two accumulators gacc and tacc.
- Nonces: why each signer makes two, what NonceGen hashes, and the
  constant-time rules for the secret ones.
- Why a reused nonce gives the secret key away, and how Sign makes reuse
  refuse.
- Signing, checking each signer's share, and adding the shares into one
  BIP-340 signature.
- DeterministicSign, for a last signer with no randomness.
- An aggregate key as a Taproot internal key, spent by the key path.

## Where MuSig2 is used

| Where | What it does | Chapter |
| --- | --- | --- |
| A Taproot output owned by several people | the aggregate key is the internal key, tweaked by chapter 41's TapTweak | 41, 101 |
| A Lightning channel with simple Taproot channels | the funding output's key is the two peers' aggregate key | 107 |
| A PSBT signed by several MuSig2 signers | BIP-373 fields carry the public nonces and partial signatures | 94 |
| A threshold group key | FROST replaces n-of-n with t-of-n over the same signature | 102 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [musig.coax](../../coax/lib/bitcoin/musig.coax) | `ms_key_agg_vartime(keys)` | KeyAgg: Q and the context a tweak changes |
| | `ms_key_sort_vartime(keys)` | KeySort: the keys in byte order |
| | `ms_tweak_vartime(ctx, tweak, xonly)` | ApplyTweak, plain or x-only |
| | `ms_taproot_tweak_vartime(keys, merkle, out)` | the TapTweak of the aggregate key (chapter 41) |
| | `ms_nonce_gen(rand, sk, pk, aggpk, msg, has_msg, extra, secnonce, pubnonce, w)` | NonceGen, secret inputs in an erased workspace |
| | `ms_nonce_agg_vartime(pubnonces, out)` | NonceAgg |
| | `ms_session_vartime(aggnonce, keys, tweaks, xonly, msg)` | GetSessionValues: b, R and e |
| | `ms_sign(secnonce, sk, aggnonce, keys, tweaks, xonly, msg, psig, w)` | Sign; zeroes the secnonce |
| | `ms_partial_verify_vartime(psig, pubnonces, keys, tweaks, xonly, msg, i)` | PartialSigVerify |
| | `ms_partial_agg_vartime(psigs, session, out)` | PartialSigAgg: the BIP-340 signature |
| | `ms_det_sign(sk, aggother, keys, tweaks, xonly, msg, rand, pubnonce, psig, w)` | DeterministicSign |

Keys, public nonces, partial signatures and messages are public, so
their procedures end in `_vartime` and use chapter 16's `ecmult_vartime`.
The secret key and the secret nonces follow chapter 3's rules: NonceGen,
Sign and DeterministicSign hash them with the erasable SHA-256 state of
chapter 24 (`ShaOwned`), multiply them by G with the fixed-base
`sp_base`, mask their range checks, and keep them in a workspace that
every return clears. Copies of scalars passed by value are not covered
by that erasure, as in chapter 24.

## Result codes

BIP-327's reference raises one of two errors. `InvalidContributionError`
names a signer (or none, for the aggregate nonce) and what it sent:
`pubkey`, `pubnonce`, `aggnonce`, `aggothernonce` or `psig`. A
`ValueError` carries a message. Each has a code here, and `MsResult`
carries the code and the signer it blames, -1 for none.

<!-- source: coax/lib/bitcoin/musig.coax codes -->
```jai
// Result codes. BIP-327's reference raises InvalidContributionError
// naming a signer and what they sent, or ValueError with a message.
MS_OK :: 0;
MS_BAD_PUBKEY :: 1;          // InvalidContributionError(i, "pubkey")
MS_BAD_PUBNONCE :: 2;        // InvalidContributionError(i, "pubnonce")
MS_BAD_AGGNONCE :: 3;        // InvalidContributionError(None, "aggnonce")
MS_BAD_AGGOTHERNONCE :: 4;   // InvalidContributionError(None, "aggothernonce")
MS_BAD_PSIG :: 5;            // InvalidContributionError(i, "psig")
MS_TWEAK_RANGE :: 6;         // "The tweak must be less than n."
MS_TWEAK_INFINITY :: 7;      // "The result of tweaking cannot be infinity."
MS_SECNONCE_1 :: 8;          // "first secnonce value is out of range."
MS_SECNONCE_2 :: 9;          // "second secnonce value is out of range."
MS_SECKEY :: 10;             // "secret key value is out of range."
MS_PK_MISMATCH :: 11;        // "Public key does not match nonce_gen argument"
MS_NOT_SIGNER :: 12;         // "The signer's pubkey must be included in the list of pubkeys."
MS_INVALID :: 13;            // a partial signature that does not verify
MS_CAPACITY :: 14;           // a wrong length, or more signers or tweaks than these arrays hold
MS_REASONS :: [15] string.["ok", "pubkey", "pubnonce", "aggnonce", "aggothernonce", "psig",
  "The tweak must be less than n.", "The result of tweaking cannot be infinity.",
  "first secnonce value is out of range.", "second secnonce value is out of range.",
  "secret key value is out of range.", "Public key does not match nonce_gen argument",
  "The signer's pubkey must be included in the list of pubkeys.", "invalid", "capacity"];
MS_KEYS :: 16;               // signers in one session
MS_TWEAKS :: 8;              // tweaks of one aggregate key
// A code and the signer it blames, -1 for none.
MsResult :: struct { code: int; signer: int; }
```

## Tagged hashes for any tag

MuSig2 uses BIP-340's tagged hash with seven tags of its own:
`KeyAgg list`, `KeyAgg coefficient`, `MuSig/aux`, `MuSig/nonce`,
`MuSig/noncecoef`, `MuSig/deterministic/nonce` and BIP-340's
`BIP0340/challenge`. Chapter 5's helper takes a fixed set of tag IDs, so
this file writes the construction for any tag over chapter 24's
erasable state. `ms_reduce` ends a hash as a scalar mod n.

<!-- source: coax/lib/bitcoin/musig.coax hash -->
```jai
// BIP-340's tagged hash, SHA-256(SHA-256(tag) || SHA-256(tag) || data),
// for any tag: ms_tag starts it, ms_add adds data, and ms_reduce ends it
// as a scalar mod n. The state is chapter 24's erasable ShaOwned.
ms_tag :: (w: [] ShaOwned, tag: string) {
  t: [32] u8; th: [32] u8;
  for i: 0..<tag.count { t[i] = tag[i]; }
  #assert so_init(w) && so_update(w, t[0..<tag.count], false) && so_finish(w, th[..], false);
  #assert so_init(w) && so_update(w, th[..], false) && so_update(w, th[..], false);
}
ms_add :: (w: [] ShaOwned, b: [] u8) { #assert so_update(w, b, false); }
ms_end :: (w: [] ShaOwned, out: [] u8) { #assert so_finish(w, out, false); }
ms_reduce :: (w: [] ShaOwned) -> ScScalar { h: [32] u8; ms_end(w, h[..]); return sc_reduce_bytes(h[..]).value; }
// A big-endian length of width bytes, added to the hash.
ms_add_length :: (w: [] ShaOwned, value: int, width: int) {
  b: [8] u8;
  for i: 0..<width { b[i] = cast(u8) ((value >> (8 * (width - 1 - i))) & 255); }
  ms_add(w, b[0..<width]);
}
```

## Points on the wire

Keys and nonces travel as 33-byte compressed points (chapter 17). An
aggregate nonce may hold the point at infinity, which BIP-327 writes as
33 zero bytes (`cbytes_ext`). The aggregate key is published as its
x-coordinate alone, and its parity decides a sign later.

<!-- source: coax/lib/bitcoin/musig.coax points -->
```jai
// cpoint: a 33-byte compressed point, 02 or 03 then x below p on the
// curve. cpoint_ext also reads 33 zero bytes as the point at infinity.
ms_cpoint_vartime :: (b: [] u8) -> SgResult {
  if b.count != 33 { return SgResult.{value=sg_infinity()}; }
  return sg_decode_public(b);
}
ms_cpoint_ext_vartime :: (b: [] u8) -> SgResult {
  zero := b.count == 33;
  for i: 0..<b.count { if b[i] != 0 { zero = false; } }
  if zero { return SgResult.{value=sg_infinity(), ok=true}; }
  return ms_cpoint_vartime(b);
}
ms_finite_vartime :: (p: SgPoint) -> bool { return sg_encode_public(p).length == 33; }
ms_odd_vartime :: (p: SgPoint) -> bool { return sg_encode_public(p).bytes[0] == 3; }
// cbytes_ext into out (33 bytes): compressed, or zeros for infinity.
ms_cbytes_vartime :: (p: SgPoint, out: [] u8) {
  e := sg_encode_public(p);
  for i: 0..<33 { out[i] = ifx e.length == 33 then e.bytes[i] else 0; }
}
// xbytes: the 32-byte x coordinate.
ms_xbytes_vartime :: (p: SgPoint, out: [] u8) {
  e := sg_encode_public(p);
  for i: 0..<32 { out[i] = e.bytes[1 + i]; }
}
// g: 1 when p has an even y, n - 1 when odd.
ms_parity_vartime :: (p: SgPoint) -> ScScalar { return ifx ms_odd_vartime(p) then sc_negate(sc_small(1)) else sc_small(1); }
```

## Key aggregation

The obvious aggregate key is the sum P_1 + … + P_n. It is unsafe. A
signer who goes last can announce P_2 = X − P_1 for a key X it alone
controls. The sum is then X, and that signer can sign alone. This is the
*rogue-key attack*.

KeyAgg stops it with a coefficient per key. L is the hash of every key
in order, and a_i = hash_KeyAgg coefficient(L ‖ P_i). The aggregate is
Q = a_1 P_1 + … + a_n P_n. A rogue key now changes L, which changes every
coefficient, so the attacker can't choose a key whose weighted sum
cancels the others.

The *second-key optimization* sets the coefficient of the second
distinct key to 1. It saves one multiplication in every aggregation and
keeps the scheme secure. When every key is the same, no key is second,
and all coefficients are hashed.

The order of the keys is part of L, so the same keys in another order
give another Q. KeySort sorts them by their 33 bytes first, so that
signers who agree on a set agree on a key.

<!-- source: coax/lib/bitcoin/musig.coax keyagg -->
```jai
// KeySort: the keys in lexicographic byte order, in place.
ms_key_sort_vartime :: (keys: [] u8) {
  u := keys.count / 33; t: [33] u8;
  for i: 1..<u {
    j := i;
    while j > 0 && ms_less_vartime(keys[33*j..<33*j+33], keys[33*(j-1)..<33*j]) {
      for k: 0..<33 { t[k] = keys[33*j+k]; keys[33*j+k] = keys[33*(j-1)+k]; keys[33*(j-1)+k] = t[k]; }
      j -= 1;
    }
  }
}
ms_less_vartime :: (a: [] u8, b: [] u8) -> bool {
  for i: 0..<a.count { if a[i] != b[i] { return a[i] < b[i]; } }
  return false;
}
// HashKeys: L, the tagged hash of every key in order.
ms_hash_keys_vartime :: (keys: [] u8, out: [] u8) {
  w: [1] ShaOwned;
  ms_tag(w[..], "KeyAgg list"); ms_add(w[..], keys); ms_end(w[..], out);
}
// GetSecondKey: the position of the first key unlike the first one, or
// -1 when every key is the first.
ms_second_key_vartime :: (keys: [] u8) -> int {
  for j: 1..<keys.count/33 { if !bt_same(keys[33*j..<33*j+33], keys[0..<33]) { return j; } }
  return -1;
}
// KeyAggCoeffInternal: 1 for the second key, otherwise
// hash_KeyAgg coefficient(L || pk) mod n.
ms_coeff_vartime :: (keys: [] u8, l: [] u8, second: int, pk: [] u8) -> ScScalar {
  if second >= 0 && bt_same(pk, keys[33*second..<33*second+33]) { return sc_small(1); }
  w: [1] ShaOwned;
  ms_tag(w[..], "KeyAgg coefficient"); ms_add(w[..], l); ms_add(w[..], pk);
  return ms_reduce(w[..]);
}
// The key aggregation context: Q, and the accumulated sign gacc and
// tweak tacc that later tweaks change.
MsKeyAgg :: struct { code: int; signer: int; q: SgPoint; gacc: ScScalar; tacc: ScScalar; }
// KeyAgg: Q = a_1 P_1 + ... + a_u P_u. A key that does not decode
// refuses, naming its signer.
ms_key_agg_vartime :: (keys: [] u8) -> MsKeyAgg {
  u := keys.count / 33;
  if keys.count % 33 != 0 || u < 1 || u > MS_KEYS { return MsKeyAgg.{code=MS_CAPACITY, signer=-1}; }
  l: [32] u8;
  ms_hash_keys_vartime(keys, l[..]);
  second := ms_second_key_vartime(keys);
  q := sg_infinity();
  for i: 0..<u {
    p := ms_cpoint_vartime(keys[33*i..<33*i+33]);
    if !p.ok { return MsKeyAgg.{code=MS_BAD_PUBKEY, signer=i}; }
    a := ms_coeff_vartime(keys, l[..], second, keys[33*i..<33*i+33]);
    q = sg_add(q, ecmult_vartime(sc_small(0), p.value, a).value).value;
  }
  // Q is infinity only with negligible probability (BIP-327 asserts it).
  #assert ms_finite_vartime(q);
  return MsKeyAgg.{code=MS_OK, signer=-1, q=q, gacc=sc_small(1)};
}
```

## Tweaks

A Taproot output key is an internal key plus tG (chapter 41). For a
MuSig2 key, the signers add the tweak after aggregation, and then must
sign for the tweaked key. BIP-327 allows a list of tweaks, each *plain*
(Q' = Q + tG, as BIP-32's non-hardened derivation does) or *x-only*
(Q' = gQ + tG, where g is −1 when Q has an odd y, as BIP-341 does with
the x-only key).

Each tweak changes the signing equation. The context keeps two
accumulators: gacc, the product of the signs applied to Q, and tacc,
the tweaks added so far, each times the signs that came after it. A
signer multiplies its secret key by gacc, and the aggregator adds
e·g·tacc to the signature for the tweak part no signer owns. A tweak
of n or more refuses, and so does a tweak that makes the key infinity.

<!-- source: coax/lib/bitcoin/musig.coax tweak -->
```jai
// ApplyTweak: Q' = g Q + t G, where g is n - 1 for an x-only tweak of a
// Q with an odd y and 1 otherwise. gacc and tacc record what was done,
// so a signer can tweak its own share to match: gacc' = g gacc and
// tacc' = t + g tacc.
ms_tweak_vartime :: (k: MsKeyAgg, tweak: [] u8, xonly: bool) -> MsKeyAgg {
  if tweak.count != 32 { return MsKeyAgg.{code=MS_CAPACITY, signer=-1}; }
  g := ifx xonly then ms_parity_vartime(k.q) else sc_small(1);
  t := sc_load(tweak);
  if !t.ok { return MsKeyAgg.{code=MS_TWEAK_RANGE, signer=-1}; }
  q := ecmult_vartime(t.value, k.q, g).value;
  if !ms_finite_vartime(q) { return MsKeyAgg.{code=MS_TWEAK_INFINITY, signer=-1}; }
  return MsKeyAgg.{code=MS_OK, signer=-1, q=q, gacc=sc_mul(g, k.gacc), tacc=sc_add(t.value, sc_mul(g, k.tacc))};
}
// KeyAgg then each tweak in turn: tweaks holds 32 bytes per flag in xonly.
ms_key_agg_tweak_vartime :: (keys: [] u8, tweaks: [] u8, xonly: [] bool) -> MsKeyAgg {
  if tweaks.count != 32 * xonly.count || xonly.count > MS_TWEAKS { return MsKeyAgg.{code=MS_CAPACITY, signer=-1}; }
  k := ms_key_agg_vartime(keys);
  for i: 0..<xonly.count {
    if k.code != MS_OK { return k; }
    k = ms_tweak_vartime(k, tweaks[32*i..<32*i+32], xonly[i]);
  }
  return k;
}
// The x-only tweak that makes the aggregate key Q a Taproot output key
// (chapter 41): TapTweak(xbytes(Q) || merkle), merkle empty for a key
// with no script tree or 32 bytes. Into out (32 bytes).
ms_taproot_tweak_vartime :: (keys: [] u8, merkle: [] u8, out: [] u8) -> int {
  if (merkle.count != 0 && merkle.count != 32) || out.count != 32 { return MS_CAPACITY; }
  k := ms_key_agg_vartime(keys);
  if k.code != MS_OK { return k.code; }
  m: [64] u8;
  ms_xbytes_vartime(k.q, m[0..<32]);
  for i: 0..<merkle.count { m[32 + i] = merkle[i]; }
  bt_tagged(BH_TAPTWEAK, m[0..<32+merkle.count], out);
  return MS_OK;
}
```

## Nonces

### Why two nonces

A Schnorr signature needs a nonce k with R = kG. With several signers,
each adds a share: R = R_1 + … + R_n. If each signer sends one nonce and
then signs, a signer that sees the others' nonces first can choose its
own, and an attacker who opens many sessions at once can combine them
into a forgery (the Wagner and ROS attacks, which broke the
two-round MuSig with one nonce each).

MuSig2 gives each signer two nonces, R_i1 and R_i2, and fixes the
final nonce only after everyone's are known: R = R_1 + b·R_2, where R_j
sums the signers' j-th nonces and b hashes the aggregate nonce, the key
and the message. No signer can predict b, so none can steer R. Nonces
can even be exchanged before the message is known, which makes MuSig2 a
two-round scheme with the first round done in advance.

### NonceGen and the constant-time rules

NonceGen hashes 32 fresh random bytes with whatever else the signer
knows: its secret key, its public key, the aggregate key, the message
and any extra input. Each optional input makes a repeated random value
less harmful. The secret key is mixed in as sk XOR hash_MuSig/aux(rand),
so a broken random source still gives nonces nobody without the key can
predict.

The secret nonce follows chapter 3's rules:

- the random bytes, the secret key and the nonces only go through
  SHA-256, scalar arithmetic and the fixed-base multiplication, which
  have no branch and no memory index on secret data;
- the check that each k_i is not zero is a mask, and only its verdict
  is declassified;
- the workspace that held them is overwritten before NonceGen returns.

The lengths of the optional inputs and whether they are present are
public, so the hash may branch on them.

<!-- source: coax/lib/bitcoin/musig.coax noncegen -->
```jai
// The workspace of NonceGen and DeterministicSign. Each return clears it.
MsNonceWork :: struct { sha: [1] ShaOwned; aux: [32] u8; rand: [32] u8; h: [32] u8; k: [2] ScScalar; valid: u64; point: [1] SignPoint; }
ms_nonce_clear :: (w: [] MsNonceWork) { fresh: MsNonceWork; w[0] = fresh; }
// k_i = hash_MuSig/nonce(rand || len(pk) || pk || len(aggpk) || aggpk ||
// msg_prefixed || len(extra_in) || extra_in || i) mod n. msg_prefixed is
// 0x00 without a message, otherwise 0x01, its 8-byte length and the
// message. The lengths are public; rand is secret.
ms_nonce_hash :: (w: [] MsNonceWork, pk: [] u8, aggpk: [] u8, msg: [] u8, has_msg: bool, extra: [] u8, i: int) -> ScScalar {
  s := w[0].sha[..];
  ms_tag(s, "MuSig/nonce"); ms_add(s, w[0].rand[..]);
  ms_add_length(s, pk.count, 1); ms_add(s, pk);
  ms_add_length(s, aggpk.count, 1); ms_add(s, aggpk);
  if has_msg { ms_add_length(s, 1, 1); ms_add_length(s, msg.count, 8); ms_add(s, msg); }
  else { ms_add_length(s, 0, 1); }
  ms_add_length(s, extra.count, 4); ms_add(s, extra);
  ms_add_length(s, i, 1);
  ms_end(s, w[0].h[..]);
  return sc_reduce_bytes(w[0].h[..]).value;
}
// The compressed point k G of a secret nonce, by the fixed-base table.
ms_nonce_point :: (k: ScScalar, w: [] SignPoint, out: [] u8) {
  #assert sp_base(k, w);
  out[0] = cast(u8) (2 | w[0].odd);
  #assert sp_x(w, out[1..<33]);
}
// NonceGen. rand_ is 32 fresh random bytes, never used again; sk (0 or
// 32 bytes), aggpk (0 or 32), msg (when has_msg) and extra are optional
// inputs that make a repeated rand_ less harmful. Writes secnonce (k_1,
// k_2 and pk, 97 bytes), which stays secret and signs once, and pubnonce
// (k_1 G and k_2 G, 66 bytes), which goes to the other signers.
ms_nonce_gen :: (rand_: [] u8, sk: [] u8, pk: [] u8, aggpk: [] u8, msg: [] u8, has_msg: bool, extra: [] u8, secnonce: [] u8, pubnonce: [] u8, w: [] MsNonceWork) -> int {
  if w.count != 1 || rand_.count != 32 || (sk.count != 0 && sk.count != 32) || pk.count != 33 || (aggpk.count != 0 && aggpk.count != 32) { return MS_CAPACITY; }
  if secnonce.count != 97 || pubnonce.count != 66 || extra.count > 0xffffffff { return MS_CAPACITY; }
  // With a secret key, rand = sk XOR hash_MuSig/aux(rand_): a weak random
  // source then still gives nonces nobody without sk can predict.
  for j: 0..<32 { w[0].rand[j] = rand_[j]; }
  if sk.count == 32 {
    ms_tag(w[0].sha[..], "MuSig/aux"); ms_add(w[0].sha[..], rand_); ms_end(w[0].sha[..], w[0].aux[..]);
    for j: 0..<32 { w[0].rand[j] = sk[j] ^ w[0].aux[j]; }
  }
  w[0].valid = 1;
  for i: 0..<2 {
    k := ms_nonce_hash(w, pk, aggpk, msg, has_msg, extra, i);
    nonzero := ec_nonzero(k);
    w[0].valid &= nonzero;
    w[0].k[i] = sc_select(nonzero, k, sc_small(1));   // masked, never a branch on k
  }
  // k_i = 0 happens with negligible probability (BIP-327 asserts it).
  #assert ec_declassify(w[0].valid);
  for i: 0..<2 {
    ms_nonce_point(w[0].k[i], w[0].point[..], pubnonce[33*i..<33*i+33]);
    #assert ss_write(w[0].k[i], secnonce[32*i..<32*i+32]);
  }
  for j: 0..<33 { secnonce[64 + j] = pk[j]; }
  ms_nonce_clear(w);
  return MS_OK;
}
```

### Why a reused nonce is fatal

A partial signature is s = k_1 + b·k_2 + e·a·d (mod n), with each k
negated when R has an odd y and d adjusted for Q's sign and the tweaks.
Everything in it except k_1, k_2 and d is public: anyone who sees the
session computes b, e, a and R's parity.

Each signature is one linear equation in the three secrets. A signer who
uses one secret nonce for one session gives one equation in three
unknowns, and reveals nothing. A signer who uses the same secret nonce in
three sessions with different messages or different co-signers gives
three equations with different b and e. They have one solution, and it
includes d. In a single-signer Schnorr signature, two equations are
enough. MuSig2 needs three because each signer has two nonces, but the
conclusion is the same: the key is gone.

Reuse is easy to cause by accident. A signer that saves its state to
disk and restores it, or a virtual machine that is snapshotted, can sign
twice with one secnonce. Two defenses follow:

- Sign zeroes the 64 secret nonce bytes as soon as it has read them,
  before any check can fail. A second Sign with the same secnonce reads
  zero and refuses: "first secnonce value is out of range."
- The secnonce is never copied or written to storage. A copy defeats the
  zeroing; the example below makes one on purpose and recovers the key.

### NonceAgg

The nonce aggregator, any party, adds the signers' j-th nonces. A sum
may be infinity, which is written as 33 zero bytes. A nonce that does not
decode refuses and names its signer.

<!-- source: coax/lib/bitcoin/musig.coax nonceagg -->
```jai
// NonceAgg: R_j is the sum of every signer's j-th nonce point, written
// with cbytes_ext, so the sum may be infinity. Into out (66 bytes). A
// nonce that does not decode refuses, naming its signer.
ms_nonce_agg_vartime :: (pubnonces: [] u8, out: [] u8) -> MsResult {
  u := pubnonces.count / 66;
  if pubnonces.count % 66 != 0 || u < 1 || u > MS_KEYS || out.count != 66 { return ms_fail(MS_CAPACITY, -1); }
  for j: 0..<2 {
    r := sg_infinity();
    for i: 0..<u {
      p := ms_cpoint_vartime(pubnonces[66*i+33*j..<66*i+33*j+33]);
      if !p.ok { return ms_fail(MS_BAD_PUBNONCE, i); }
      r = sg_add(r, p.value).value;
    }
    ms_cbytes_vartime(r, out[33*j..<33*j+33]);
  }
  return ms_ok();
}
```

## The session

Every signer and the aggregator compute the same session values from
the aggregate nonce, the keys, the tweaks and the message: Q with its
accumulators, b, the final nonce R (G when R_1 + b·R_2 is infinity, an
event that no honest session reaches) and BIP-340's challenge e. The
coefficient of a signer's key is looked up in the session's keys; a key
that is not there refuses.

<!-- source: coax/lib/bitcoin/musig.coax session -->
```jai
// GetSessionValues: the tweaked key, b = hash_MuSig/noncecoef(aggnonce ||
// xbytes(Q) || msg), the final nonce R = R_1 + b R_2 (G if that is
// infinity) and BIP-340's challenge e = hash_BIP0340/challenge(xbytes(R)
// || xbytes(Q) || msg).
MsSession :: struct { code: int; signer: int; q: SgPoint; gacc: ScScalar; tacc: ScScalar; b: ScScalar; r: SgPoint; e: ScScalar; }
ms_session_vartime :: (aggnonce: [] u8, keys: [] u8, tweaks: [] u8, xonly: [] bool, msg: [] u8) -> MsSession {
  x: MsSession; x.signer = -1;
  if aggnonce.count != 66 { x.code = MS_CAPACITY; return x; }
  k := ms_key_agg_tweak_vartime(keys, tweaks, xonly);
  x.code = k.code; x.signer = k.signer;
  if k.code != MS_OK { return x; }
  x.q = k.q; x.gacc = k.gacc; x.tacc = k.tacc;
  qx: [32] u8; rx: [32] u8; w: [1] ShaOwned;
  ms_xbytes_vartime(k.q, qx[..]);
  ms_tag(w[..], "MuSig/noncecoef"); ms_add(w[..], aggnonce); ms_add(w[..], qx[..]); ms_add(w[..], msg);
  x.b = ms_reduce(w[..]);
  r1 := ms_cpoint_ext_vartime(aggnonce[0..<33]);
  r2 := ms_cpoint_ext_vartime(aggnonce[33..<66]);
  if !r1.ok || !r2.ok { x.code = MS_BAD_AGGNONCE; return x; }
  r := sg_add(r1.value, ecmult_vartime(sc_small(0), r2.value, x.b).value).value;
  x.r = ifx ms_finite_vartime(r) then r else sg_generator();
  ms_xbytes_vartime(x.r, rx[..]);
  ms_tag(w[..], "BIP0340/challenge"); ms_add(w[..], rx[..]); ms_add(w[..], qx[..]); ms_add(w[..], msg);
  x.e = ms_reduce(w[..]);
  return x;
}
// GetSessionKeyAggCoeff: the coefficient of the signer's key pk, which
// must be one of the session's keys.
MsCoeff :: struct { value: ScScalar; ok: bool; }
ms_session_coeff_vartime :: (keys: [] u8, pk: [] u8) -> MsCoeff {
  found := false;
  for i: 0..<keys.count/33 { if bt_same(keys[33*i..<33*i+33], pk) { found = true; } }
  if !found { return MsCoeff.{}; }
  l: [32] u8;
  ms_hash_keys_vartime(keys, l[..]);
  return MsCoeff.{value=ms_coeff_vartime(keys, l[..], ms_second_key_vartime(keys), pk), ok=true};
}
```

## Signing

Sign computes s = k_1 + b·k_2 + e·a·d, where k_1 and k_2 are negated
when R has an odd y and d is the secret key times g (−1 when Q has an
odd y) and gacc. The order matters: the session values come first, so a
refused session leaves the secnonce untouched; then the nonces are read
and zeroed; then the checks run. The last 33 bytes of a secnonce are the
public key it was made for, and a different key refuses.

<!-- source: coax/lib/bitcoin/musig.coax sign -->
```jai
// The workspace of Sign. Each return clears it.
MsSignWork :: struct { k: [2] SignScalar; d: [1] SignScalar; point: [1] SignPoint; pk: [33] u8; s: ScScalar; }
ms_sign_clear :: (w: [] MsSignWork, code: int) -> MsResult { fresh: MsSignWork; w[0] = fresh; return ms_fail(code, -1); }
// Sign: s = k_1 + b k_2 + e a d mod n, where the k_i are negated when R
// has an odd y and d is the secret key times g (n - 1 when Q has an odd
// y) and gacc. Into psig (32 bytes). It zeroes the first 64 bytes of
// secnonce before it can fail, so a secnonce signs at most once: a second
// call refuses as MS_SECNONCE_1.
ms_sign :: (secnonce: [] u8, sk: [] u8, aggnonce: [] u8, keys: [] u8, tweaks: [] u8, xonly: [] bool, msg: [] u8, psig: [] u8, w: [] MsSignWork) -> MsResult {
  if w.count != 1 || secnonce.count != 97 || sk.count != 32 || psig.count != 32 { return ms_fail(MS_CAPACITY, -1); }
  x := ms_session_vartime(aggnonce, keys, tweaks, xonly, msg);
  if x.code != MS_OK { return ms_fail(x.code, x.signer); }
  #assert ss_key(secnonce[0..<32], w[0].k[0..<1], false) && ss_key(secnonce[32..<64], w[0].k[1..<2], false);
  for j: 0..<64 { secnonce[j] = 0; }
  // The range checks are masked; only their verdicts are declassified.
  if !ec_declassify(w[0].k[0].valid) { return ms_sign_clear(w, MS_SECNONCE_1); }
  if !ec_declassify(w[0].k[1].valid) { return ms_sign_clear(w, MS_SECNONCE_2); }
  #assert ss_key(sk, w[0].d[..], false);
  if !ec_declassify(w[0].d[0].valid) { return ms_sign_clear(w, MS_SECKEY); }
  // The secnonce was made for this key: its last 33 bytes are d G.
  ms_nonce_point(w[0].d[0].safe, w[0].point[..], w[0].pk[..]);
  if !bt_same(w[0].pk[..], secnonce[64..<97]) { return ms_sign_clear(w, MS_PK_MISMATCH); }
  a := ms_session_coeff_vartime(keys, w[0].pk[..]);
  if !a.ok { return ms_sign_clear(w, MS_NOT_SIGNER); }
  r_odd := cast(u64) (ifx ms_odd_vartime(x.r) then 1 else 0);   // public
  k1 := ss_even(w[0].k[0].safe, r_odd);
  k2 := ss_even(w[0].k[1].safe, r_odd);
  d := sc_mul(sc_mul(ms_parity_vartime(x.q), x.gacc), w[0].d[0].safe);
  w[0].s = sc_add(sc_add(k1, sc_mul(x.b, k2)), sc_mul(sc_mul(x.e, a.value), d));
  #assert ss_write(w[0].s, psig);
  return ms_sign_clear(w, MS_OK);
}
```

## Checking a partial signature

A bad partial signature makes the final signature invalid without saying
whose it was. PartialSigVerify checks one signer's share against its
public nonce and key: s·G must equal Re + e·a·g·gacc·P, where Re is that
signer's R_1 + b·R_2, negated when R has an odd y. A signer that checks
everyone's shares before aggregating can name the one who misbehaved.

<!-- source: coax/lib/bitcoin/musig.coax verify -->
```jai
// PartialSigVerifyInternal: s G = Re + e a g gacc P, where Re = R_1 + b R_2
// from this signer's nonce, negated when R has an odd y. g is Q's sign.
ms_partial_verify_session_vartime :: (psig: [] u8, pubnonce: [] u8, pk: [] u8, x: MsSession, keys: [] u8) -> MsResult {
  s := sc_load(psig);
  if psig.count != 32 || !s.ok { return ms_fail(MS_INVALID, -1); }
  r1 := ms_cpoint_vartime(pubnonce[0..<33]);
  r2 := ms_cpoint_vartime(pubnonce[33..<66]);
  if !r1.ok || !r2.ok { return ms_fail(MS_BAD_PUBNONCE, -1); }
  re := sg_add(r1.value, ecmult_vartime(sc_small(0), r2.value, x.b).value).value;
  if ms_odd_vartime(x.r) { re = sg_negate(re).value; }
  p := ms_cpoint_vartime(pk);
  if !p.ok { return ms_fail(MS_BAD_PUBKEY, -1); }
  a := ms_session_coeff_vartime(keys, pk);
  if !a.ok { return ms_fail(MS_NOT_SIGNER, -1); }
  c := sc_mul(sc_mul(x.e, a.value), sc_mul(ms_parity_vartime(x.q), x.gacc));
  // s G - c P, compared with Re.
  lhs := ecmult_vartime(s.value, p.value, sc_negate(c)).value;
  return ifx sg_equal_mask(lhs, re) == CB_MAX64 then ms_ok() else ms_fail(MS_INVALID, -1);
}
// PartialSigVerify: signer i's partial signature, from everyone's public
// nonces and keys. MS_OK, MS_INVALID, or the contribution that refused.
ms_partial_verify_vartime :: (psig: [] u8, pubnonces: [] u8, keys: [] u8, tweaks: [] u8, xonly: [] bool, msg: [] u8, i: int) -> MsResult {
  u := keys.count / 33;
  if pubnonces.count != 66 * u || keys.count % 33 != 0 || i < 0 || i >= u { return ms_fail(MS_CAPACITY, -1); }
  agg: [66] u8;
  n := ms_nonce_agg_vartime(pubnonces, agg[..]);
  if n.code != MS_OK { return n; }
  x := ms_session_vartime(agg[..], keys, tweaks, xonly, msg);
  if x.code != MS_OK { return ms_fail(x.code, x.signer); }
  return ms_partial_verify_session_vartime(psig, pubnonces[66*i..<66*i+66], keys[33*i..<33*i+33], x, keys);
}
```

## Aggregation

The signature is R's x-coordinate and s = s_1 + … + s_n + e·g·tacc. The
last term signs for the tweaks, which no signer's share included. Any
s_i of n or more refuses and names its signer.

<!-- source: coax/lib/bitcoin/musig.coax aggregate -->
```jai
// PartialSigAgg: s = s_1 + ... + s_u + e g tacc mod n, and the BIP-340
// signature xbytes(R) || s into out (64 bytes). The e g tacc term signs
// for the tweaks, which no signer's share included.
ms_partial_agg_vartime :: (psigs: [] u8, x: MsSession, out: [] u8) -> MsResult {
  u := psigs.count / 32;
  if x.code != MS_OK || psigs.count % 32 != 0 || u < 1 || u > MS_KEYS || out.count != 64 { return ms_fail(MS_CAPACITY, -1); }
  s: ScScalar;
  for i: 0..<u {
    si := sc_load(psigs[32*i..<32*i+32]);
    if !si.ok { return ms_fail(MS_BAD_PSIG, i); }
    s = sc_add(s, si.value);
  }
  s = sc_add(s, sc_mul(sc_mul(x.e, ms_parity_vartime(x.q)), x.tacc));
  ms_xbytes_vartime(x.r, out[0..<32]);
  #assert sc_store(s, out[32..<64]);
  return ms_ok();
}
```

## DeterministicSign

Some signers have no good randomness, or can't keep state between the
two rounds. DeterministicSign lets one such signer sign without a stored
nonce, under one condition: it must go last, after it has seen every
other signer's nonce. Its nonces are a hash of its secret key, the other
signers' aggregate nonce, the aggregate key and the message. If any of
those change, the nonces change, so a malicious co-signer can't make it
sign two different sessions with one nonce. If it went first, a
co-signer could replay its nonce in a second session with a different
nonce of its own, which is exactly the reuse above.

<!-- source: coax/lib/bitcoin/musig.coax detsign -->
```jai
// The workspace of DeterministicSign. Each return clears it.
MsDetWork :: struct { nonce: [1] MsNonceWork; sk: [32] u8; secnonce: [97] u8; sign: [1] MsSignWork; }
ms_det_clear :: (w: [] MsDetWork, r: MsResult) -> MsResult { fresh: MsDetWork; w[0] = fresh; return r; }
// DeterministicSign, for the last signer, who has seen every other
// nonce (aggothernonce, their NonceAgg) and has no randomness: k_i =
// hash_MuSig/deterministic/nonce(sk' || aggothernonce || aggpk ||
// len(msg) || msg || i) mod n, with sk' = sk XOR hash_MuSig/aux(rand)
// when rand (0 or 32 bytes) is given. Because every input of the hash is
// fixed, any change to the other nonces, the keys or the message changes
// the nonce. Writes pubnonce (66 bytes) and psig (32).
ms_det_sign :: (sk: [] u8, aggother: [] u8, keys: [] u8, tweaks: [] u8, xonly: [] bool, msg: [] u8, rand: [] u8, pubnonce: [] u8, psig: [] u8, w: [] MsDetWork) -> MsResult {
  if w.count != 1 || sk.count != 32 || aggother.count != 66 || (rand.count != 0 && rand.count != 32) || pubnonce.count != 66 || psig.count != 32 { return ms_fail(MS_CAPACITY, -1); }
  k := ms_key_agg_tweak_vartime(keys, tweaks, xonly);
  if k.code != MS_OK { return ms_det_clear(w, ms_fail(k.code, k.signer)); }
  aggpk: [32] u8;
  ms_xbytes_vartime(k.q, aggpk[..]);
  n := w[0].nonce[..]; s := n[0].sha[..];
  for j: 0..<32 { w[0].sk[j] = sk[j]; }
  if rand.count == 32 {
    ms_tag(s, "MuSig/aux"); ms_add(s, rand); ms_end(s, n[0].aux[..]);
    for j: 0..<32 { w[0].sk[j] = sk[j] ^ n[0].aux[j]; }
  }
  n[0].valid = 1;
  for i: 0..<2 {
    ms_tag(s, "MuSig/deterministic/nonce"); ms_add(s, w[0].sk[..]); ms_add(s, aggother); ms_add(s, aggpk[..]);
    ms_add_length(s, msg.count, 8); ms_add(s, msg); ms_add_length(s, i, 1);
    ms_end(s, n[0].h[..]);
    ki := sc_reduce_bytes(n[0].h[..]).value;
    nonzero := ec_nonzero(ki);
    n[0].valid &= nonzero;
    n[0].k[i] = sc_select(nonzero, ki, sc_small(1));
  }
  #assert ec_declassify(n[0].valid);
  for i: 0..<2 {
    ms_nonce_point(n[0].k[i], n[0].point[..], pubnonce[33*i..<33*i+33]);
    #assert ss_write(n[0].k[i], w[0].secnonce[32*i..<32*i+32]);
  }
  // The secnonce's public key: IndividualPubkey(sk).
  d := w[0].sign[0].d[..];
  #assert ss_key(sk, d, false);
  if !ec_declassify(d[0].valid) { return ms_det_clear(w, ms_fail(MS_SECKEY, -1)); }
  ms_nonce_point(d[0].safe, n[0].point[..], w[0].secnonce[64..<97]);
  both: [132] u8; agg: [66] u8;
  for j: 0..<66 { both[j] = pubnonce[j]; both[66 + j] = aggother[j]; }
  if ms_nonce_agg_vartime(both[..], agg[..]).code != MS_OK { return ms_det_clear(w, ms_fail(MS_BAD_AGGOTHERNONCE, -1)); }
  return ms_det_clear(w, ms_sign(w[0].secnonce[..], sk, agg[..], keys, tweaks, xonly, msg, psig, w[0].sign[..]));
}
```

## The vectors

BIP-327 publishes eight vector files. Each case states a result or the
error it raises and whom the error blames. `make_musig_vectors.py`
copies every case into `musig_vectors.coax`, with the JSON's indices
resolved into the bytes they name. Each `msv_` procedure counts the cases
of one file that give their stated result. A signing case that signs
also checks that the same secnonce then refuses, and every deterministic
partial signature is verified, and every aggregate signature passes
chapter 23's verifier.

<!-- source: coax/lib/bitcoin/musig.coax vectors -->
```jai
// Each BIP-327 vector file: the number of its cases that give their
// stated result, a value or the refusal with the signer it blames.
msv_key_sort :: () -> int {
  k: [256] u8; e: [256] u8;
  keys := ms_hex(MS_SORT_KEYS, k[..]);
  ms_key_sort_vartime(keys);
  return ifx bt_same(keys, ms_hex(MS_SORT_SORTED, e[..])) then 1 else 0;
}
msv_key_agg :: () -> int {
  passed := 0;
  for i: 0..<MS_KEY_AGG.count {
    c := MS_KEY_AGG[i];
    k: [512] u8; t: [256] u8; f: [8] bool; q: [32] u8;
    r := ms_key_agg_tweak_vartime(ms_hex(c.keys, k[..]), ms_hex(c.tweaks, t[..]), ms_flags(c.xonly, f[..]));
    ms_xbytes_vartime(r.q, q[..]);
    if r.code == c.code && r.signer == c.signer && (r.code != MS_OK || bt_hex_is(q[..], c.expected)) { passed += 1; }
  }
  return passed;
}
msv_nonce_gen :: () -> int {
  passed := 0; w: [1] MsNonceWork;
  for i: 0..<MS_NONCE_GEN.count {
    c := MS_NONCE_GEN[i];
    r: [32] u8; sk: [32] u8; pk: [33] u8; aggpk: [32] u8; msg: [64] u8; extra: [32] u8; sec: [97] u8; pub: [66] u8;
    code := ms_nonce_gen(ms_hex(c.rand, r[..]), ms_hex(c.sk, sk[..]), ms_hex(c.pk, pk[..]), ms_hex(c.aggpk, aggpk[..]), ms_hex(c.msg, msg[..]), c.has_msg,
      ms_hex(c.extra, extra[..]), sec[..], pub[..], w[..]);
    if code == MS_OK && bt_hex_is(sec[..], c.secnonce) && bt_hex_is(pub[..], c.pubnonce) { passed += 1; }
  }
  return passed;
}
msv_nonce_agg :: () -> int {
  passed := 0;
  for i: 0..<MS_NONCE_AGG.count {
    c := MS_NONCE_AGG[i];
    n: [256] u8; out: [66] u8;
    r := ms_nonce_agg_vartime(ms_hex(c.nonces, n[..]), out[..]);
    if r.code == c.code && r.signer == c.signer && (r.code != MS_OK || bt_hex_is(out[..], c.expected)) { passed += 1; }
  }
  return passed;
}
// Sign, from sign_verify_vectors and tweak_vectors, cases lo to hi - 1.
// A case that signs also refuses a second time with the same secnonce.
msv_sign :: (lo: int, hi: int) -> int {
  passed := 0; w: [1] MsSignWork;
  for i: lo..<hi {
    c := MS_SIGN[i];
    sec: [97] u8; sk: [32] u8; agg: [66] u8; k: [256] u8; t: [256] u8; f: [8] bool; m: [64] u8; psig: [32] u8;
    keys := ms_hex(c.keys, k[..]); tweaks := ms_hex(c.tweaks, t[..]); xonly := ms_flags(c.xonly, f[..]); msg := ms_hex(c.msg, m[..]);
    r := ms_sign(ms_hex(c.secnonce, sec[..]), ms_hex(c.sk, sk[..]), ms_hex(c.aggnonce, agg[..]), keys, tweaks, xonly, msg, psig[..], w[..]);
    ok := r.code == c.code && r.signer == c.signer;
    if r.code == MS_OK {
      ok = ok && bt_hex_is(psig[..], c.expected);
      ok = ok && ms_sign(sec[..], sk[..], agg[..], keys, tweaks, xonly, msg, psig[..], w[..]).code == MS_SECNONCE_1;
    }
    if ok { passed += 1; }
  }
  return passed;
}
// PartialSigVerify, from the same two files, cases lo to hi - 1.
msv_verify :: (lo: int, hi: int) -> int {
  passed := 0;
  for i: lo..<hi {
    c := MS_VERIFY[i];
    s: [32] u8; k: [256] u8; n: [256] u8; t: [256] u8; f: [8] bool; m: [64] u8;
    r := ms_partial_verify_vartime(ms_hex(c.sig, s[..]), ms_hex(c.nonces, n[..]), ms_hex(c.keys, k[..]), ms_hex(c.tweaks, t[..]),
      ms_flags(c.xonly, f[..]), ms_hex(c.msg, m[..]), c.index);
    if r.code == c.code && r.signer == c.signer { passed += 1; }
  }
  return passed;
}
// DeterministicSign, cases lo to hi - 1. A partial signature it makes
// also verifies.
msv_det_sign :: (lo: int, hi: int) -> int {
  passed := 0; w: [1] MsDetWork;
  for i: lo..<hi {
    c := MS_DET_SIGN[i];
    sk: [32] u8; o: [66] u8; k: [256] u8; t: [64] u8; f: [4] bool; m: [64] u8; rd: [32] u8; pub: [66] u8; psig: [32] u8;
    keys := ms_hex(c.keys, k[..]); tweaks := ms_hex(c.tweaks, t[..]); xonly := ms_flags(c.xonly, f[..]); msg := ms_hex(c.msg, m[..]);
    other := ms_hex(c.aggother, o[..]);
    r := ms_det_sign(ms_hex(c.sk, sk[..]), other, keys, tweaks, xonly, msg, ms_hex(c.rand, rd[..]), pub[..], psig[..], w[..]);
    ok := r.code == c.code && r.signer == c.signer;
    if r.code == MS_OK {
      both: [132] u8; agg: [66] u8;
      for j: 0..<66 { both[j] = other[j]; both[66 + j] = pub[j]; }
      ok = ok && bt_hex_is(pub[..], c.pubnonce) && bt_hex_is(psig[..], c.psig) && ms_nonce_agg_vartime(both[..], agg[..]).code == MS_OK;
      x := ms_session_vartime(agg[..], keys, tweaks, xonly, msg);
      ok = ok && ms_partial_verify_session_vartime(psig[..], pub[..], keys[33*c.index..<33*c.index+33], x, keys).code == MS_OK;
    }
    if ok { passed += 1; }
  }
  return passed;
}
// PartialSigAgg. Each signature it makes is a valid BIP-340 signature
// under the aggregate key.
msv_sig_agg :: () -> int {
  passed := 0;
  for i: 0..<MS_SIG_AGG.count {
    c := MS_SIG_AGG[i];
    agg: [66] u8; n: [256] u8; k: [256] u8; t: [256] u8; f: [8] bool; p: [256] u8; m: [64] u8; sig: [64] u8; check: [66] u8; q: [32] u8;
    keys := ms_hex(c.keys, k[..]); tweaks := ms_hex(c.tweaks, t[..]); xonly := ms_flags(c.xonly, f[..]); msg := ms_hex(c.msg, m[..]);
    ok := ms_nonce_agg_vartime(ms_hex(c.nonces, n[..]), check[..]).code == MS_OK && bt_same(check[..], ms_hex(c.aggnonce, agg[..]));
    x := ms_session_vartime(agg[..], keys, tweaks, xonly, msg);
    r := ms_partial_agg_vartime(ms_hex(c.psigs, p[..]), x, sig[..]);
    ok = ok && r.code == c.code && r.signer == c.signer;
    if r.code == MS_OK {
      ms_xbytes_vartime(x.q, q[..]);
      ok = ok && bt_hex_is(sig[..], c.expected) && schnorr_verify_vartime(msg, q[..], sig[..]);
    }
    if ok { passed += 1; }
  }
  return passed;
}
```

## A Taproot key-path key

An aggregate key can be a Taproot internal key. The signers aggregate,
compute TapTweak(xbytes(Q) ‖ merkle) as chapter 41 does, and apply it
as an x-only tweak. The tweaked aggregate key is chapter 41's output key
of the internal key Q, so the address is an ordinary P2TR address. To
spend, the signers sign the input's Taproot signature hash (chapter 43)
with that tweak in the session, and the result is a key-path witness of
one 64-byte signature.

## The example

The example is [bitcoin/examples/musig2.coax](../examples/musig2.coax).
It aggregates BIP-327's keys and shows each coefficient, sorts keys,
applies four tweaks, runs a three-signer session step by step, makes a
Taproot output and spends its key path, refuses a reused secnonce and
then recovers a secret key from a copied one, signs deterministically,
and counts every vector file.

<!-- source: bitcoin/examples/musig2.coax example -->
```jai
#load "../../coax/lib/bitcoin/musig.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

put_hex :: (b: [] u8) {
  text: [300] u8;
  r := hex_encode_checked(cast(string) b, text[..], false);
  print("%", cast(string) text[0..<r.written]);
}
line :: (label: string, b: [] u8) { print("  %: ", label); put_hex(b); print("\n"); }
scalar_line :: (label: string, s: ScScalar) { b: [32] u8; #assert sc_store(s, b[..]); line(label, b[..]); }
point_line :: (label: string, p: SgPoint) { b: [33] u8; ms_cbytes_vartime(p, b[..]); line(label, b[..]); }
yes :: (b: bool) -> string { return ifx b then "yes" else "no"; }
refusal :: (r: MsResult) -> string { return MS_REASONS[r.code]; }

main :: () {
  // KeyAgg: the coefficients, the second key's 1, and Q.
  print("KeyAgg of BIP-327's keys X1, X2, X3:\n");
  k: [512] u8; l: [32] u8; q: [32] u8;
  keys := ms_hex(MS_KEY_AGG[0].keys, k[..]);
  ms_hash_keys_vartime(keys, l[..]);
  line("L = hash_KeyAgg list(X1 || X2 || X3)", l[..]);
  second := ms_second_key_vartime(keys);
  print("  the second key: X%, so its coefficient is 1\n", second + 1);
  for i: 0..<3 {
    a: [32] u8; #assert sc_store(ms_coeff_vartime(keys, l[..], second, keys[33*i..<33*i+33]), a[..]);
    print("  a_%: ", i + 1); put_hex(a[..]); print("\n");
  }
  labels := [4] string.["X1, X2, X3", "X3, X2, X1 (the order changes Q)", "X1 three times", "X1, X1, X2, X2"];
  for i: 0..<4 {
    x := ms_key_agg_vartime(ms_hex(MS_KEY_AGG[i].keys, k[..]));
    ms_xbytes_vartime(x.q, q[..]);
    print("  Q of %: ", labels[i]); put_hex(q[..]); print(", as published: %\n", yes(bt_hex_is(q[..], MS_KEY_AGG[i].expected)));
  }
  for i: 4..<MS_KEY_AGG.count {
    c := MS_KEY_AGG[i]; t: [64] u8; f: [2] bool;
    r := ms_key_agg_tweak_vartime(ms_hex(c.keys, k[..]), ms_hex(c.tweaks, t[..]), ms_flags(c.xonly, f[..]));
    print("  refused: % (signer %)\n", MS_REASONS[r.code], r.signer);
  }

  // KeySort.
  print("KeySort:\n");
  sorted := ms_hex(MS_SORT_KEYS, k[..]);
  ms_key_sort_vartime(sorted);
  for i: 0..<sorted.count/33 { print("  "); put_hex(sorted[33*i..<33*i+33]); print("\n"); }

  // Tweaks: plain, plain, x-only, x-only, from tweak_vectors.
  print("Tweaking the key of tweak_vectors (plain, plain, x-only, x-only):\n");
  c := MS_SIGN[15]; t: [128] u8; f: [4] bool;
  tweaks := ms_hex(c.tweaks, t[..]); xonly := ms_flags(c.xonly, f[..]);
  x := ms_key_agg_vartime(ms_hex(c.keys, k[..]));
  point_line("Q", x.q);
  for i: 0..<4 {
    x = ms_tweak_vartime(x, tweaks[32*i..<32*i+32], xonly[i]);
    print("  after % tweak %: ", ifx xonly[i] then "x-only" else "plain", i + 1);
    b: [33] u8; ms_cbytes_vartime(x.q, b[..]); put_hex(b[..]);
    print(", gacc is %\n", ifx sc_equal(x.gacc, sc_small(1)) then "1" else "n - 1");
  }
  scalar_line("tacc", x.tacc);

  // A three-signer session.
  print("Three signers sign one message:\n");
  sk: [96] u8; pks: [99] u8; msg: [32] u8;
  for i: 0..<3 { ms_test_key(i, sk[32*i..<32*i+32], pks[33*i..<33*i+33]); }
  for j: 0..<32 { msg[j] = cast(u8) (0xa0 + j); }
  agg: [66] u8; pub: [198] u8; psigs: [96] u8; sig: [64] u8; aggpk: [32] u8;
  nf: [1] bool;
  ms_xbytes_vartime(ms_key_agg_vartime(pks[..]).q, aggpk[..]);
  line("aggregate key", aggpk[..]);
  session(sk[..], pks[..], k[0..<0], nf[0..<0], msg[..], 1, pub[..], agg[..], psigs[..], sig[..]);
  for i: 0..<3 { print("  signer %'s nonce: ", i + 1); put_hex(pub[66*i..<66*i+66]); print("\n"); }
  line("aggnonce", agg[..]);
  s := ms_session_vartime(agg[..], pks[..], k[0..<0], nf[0..<0], msg[..]);
  scalar_line("b", s.b);
  point_line("R = R1 + b R2", s.r);
  scalar_line("e", s.e);
  for i: 0..<3 {
    print("  signer %'s partial signature: ", i + 1); put_hex(psigs[32*i..<32*i+32]); print("\n");
    print("    verifies: %\n", refusal(ms_partial_verify_vartime(psigs[32*i..<32*i+32], pub[..], pks[..], k[0..<0], nf[0..<0], msg[..], i)));
  }
  psigs[31] ^= 1;
  print("  signer 1's with its last bit changed: %\n", refusal(ms_partial_verify_vartime(psigs[0..<32], pub[..], pks[..], k[0..<0], nf[0..<0], msg[..], 0)));
  psigs[31] ^= 1;
  line("signature", sig[..]);
  print("  a valid BIP-340 signature under the aggregate key: %\n", yes(schnorr_verify_vartime(msg[..], aggpk[..], sig[..])));

  // A Taproot key-path output.
  print("The aggregate key as a Taproot internal key:\n");
  tweak: [32] u8; output: [32] u8; addr: [100] u8; xf := [1] bool.[true];
  #assert ms_taproot_tweak_vartime(pks[..], k[0..<0], tweak[..]) == MS_OK;
  line("TapTweak(Q)", tweak[..]);
  ms_xbytes_vartime(ms_key_agg_tweak_vartime(pks[..], tweak[..], xf[..]).q, q[..]);
  line("output key", q[..]);
  #assert taproot_output_key_vartime(aggpk[..], k[0..<0], output[..]) >= 0;
  print("  chapter 41's output key of the same internal key: %\n", yes(bt_same(q[..], output[..])));
  n := segwit_encode_vartime(SW_MAINNET, 1, q[..], addr[..]);
  print("  address: %\n", cast(string) addr[0..<n]);
  session(sk[..], pks[..], tweak[..], xf[..], msg[..], 11, pub[..], agg[..], psigs[..], sig[..]);
  line("key-path signature", sig[..]);
  print("  verifies under the output key: %\n", yes(taproot_key_path_verify_vartime(q[..], sig[..], msg[..])));

  // A secnonce signs once; a copied one gives the key away.
  print("Nonce reuse:\n");
  nw: [1] MsNonceWork; sw: [1] MsSignWork; sec: [97] u8; copy: [97] u8; psig: [32] u8; rand: [32] u8;
  for j: 0..<32 { rand[j] = 0x55; }
  #assert ms_nonce_gen(rand[..], sk[0..<32], pks[0..<33], k[0..<0], k[0..<0], false, k[0..<0], sec[..], pub[0..<66], nw[..]) == MS_OK;
  for j: 0..<97 { copy[j] = sec[j]; }
  print("  first Sign: %\n", refusal(sign_one(sec[..], sk[..], pks[..], pub[..], msg[..], 0, psig[..], sw[..])));
  print("  second Sign with the same secnonce: %\n", refusal(sign_one(sec[..], sk[..], pks[..], pub[..], msg[..], 0, psig[..], sw[..])));
  // The bug: a copy of the secnonce signs three other messages.
  rows: [3][4] ScScalar;
  for j: 0..<3 {
    for i: 0..<97 { sec[i] = copy[i]; }
    m: [32] u8; for i: 0..<32 { m[i] = cast(u8) (j + 1); }
    #assert sign_one(sec[..], sk[..], pks[..], pub[..], m[..], 20 + j, psig[..], sw[..]).code == MS_OK;
    print("  message %: s = ", j + 1); put_hex(psig[..]); print("\n");
    rows[j] = reuse_row(psig[..], pub[..], pks[..], m[..]);
  }
  d := solve(rows);
  g := ms_parity_vartime(ms_key_agg_vartime(pks[..]).q);
  d = sc_mul(d, g);
  scalar_line("solved for the secret key", d);
  print("  it is signer 1's secret key: %\n", yes(sc_equal(d, sc_load(sk[0..<32]).value)));

  // DeterministicSign.
  print("DeterministicSign (det_sign_vectors case 1):\n");
  dw: [1] MsDetWork; dc := MS_DET_SIGN[0];
  o: [66] u8; m: [64] u8; rd: [32] u8; key: [32] u8; tf: [1] bool;
  r := ms_det_sign(ms_hex(dc.sk, key[..]), ms_hex(dc.aggother, o[..]), ms_hex(dc.keys, k[..]), k[0..<0], tf[0..<0], ms_hex(dc.msg, m[..]),
    ms_hex(dc.rand, rd[..]), pub[0..<66], psig[..], dw[..]);
  line("pubnonce", pub[0..<66]); line("psig", psig[..]);
  print("  %, as published: %\n", refusal(r), yes(bt_hex_is(pub[0..<66], dc.pubnonce) && bt_hex_is(psig[..], dc.psig)));

  // Every published vector.
  print("BIP-327's vector files, cases giving their stated result:\n");
  print("  key_sort_vectors: % of 1\n", msv_key_sort());
  print("  key_agg_vectors: % of %\n", msv_key_agg(), MS_KEY_AGG.count);
  print("  nonce_gen_vectors: % of %\n", msv_nonce_gen(), MS_NONCE_GEN.count);
  print("  nonce_agg_vectors: % of %\n", msv_nonce_agg(), MS_NONCE_AGG.count);
  print("  sign_verify_vectors and tweak_vectors, Sign: % of %\n", msv_sign(0, MS_SIGN.count), MS_SIGN.count);
  print("  sign_verify_vectors and tweak_vectors, PartialSigVerify: % of %\n", msv_verify(0, MS_VERIFY.count), MS_VERIFY.count);
  print("  det_sign_vectors: % of %\n", msv_det_sign(0, MS_DET_SIGN.count), MS_DET_SIGN.count);
  print("  sig_agg_vectors: % of %\n", msv_sig_agg(), MS_SIG_AGG.count);
}
// One session of the three signers: each makes a nonce from rand bytes
// seed + i, signs, and the partial signatures are added into sig.
session :: (sk: [] u8, pks: [] u8, tweaks: [] u8, xonly: [] bool, msg: [] u8, seed: int, pub: [] u8, agg: [] u8, psigs: [] u8, sig: [] u8) {
  nw: [1] MsNonceWork; sw: [1] MsSignWork; sec: [291] u8; rand: [32] u8; aggpk: [32] u8;
  ms_xbytes_vartime(ms_key_agg_tweak_vartime(pks, tweaks, xonly).q, aggpk[..]);
  for i: 0..<3 {
    for j: 0..<32 { rand[j] = cast(u8) (seed + i); }
    #assert ms_nonce_gen(rand[..], sk[32*i..<32*i+32], pks[33*i..<33*i+33], aggpk[..], msg, true, rand[0..<0], sec[97*i..<97*i+97], pub[66*i..<66*i+66], nw[..]) == MS_OK;
  }
  #assert ms_nonce_agg_vartime(pub, agg).code == MS_OK;
  for i: 0..<3 {
    #assert ms_sign(sec[97*i..<97*i+97], sk[32*i..<32*i+32], agg, pks, tweaks, xonly, msg, psigs[32*i..<32*i+32], sw[..]).code == MS_OK;
  }
  #assert ms_partial_agg_vartime(psigs, ms_session_vartime(agg, pks, tweaks, xonly, msg), sig).code == MS_OK;
}
// Signer 1 signs msg with secnonce while signers 2 and 3 bring fresh
// nonces from rand bytes seed; signer 1's public nonce is pub[0..<66].
sign_one :: (sec: [] u8, sk: [] u8, pks: [] u8, pub: [] u8, msg: [] u8, seed: int, psig: [] u8, sw: [] MsSignWork) -> MsResult {
  nw: [1] MsNonceWork; other: [97] u8; rand: [32] u8; agg: [66] u8; aggpk: [32] u8;
  for i: 1..<3 {
    for j: 0..<32 { rand[j] = cast(u8) (seed + i); }
    #assert ms_nonce_gen(rand[..], sk[32*i..<32*i+32], pks[33*i..<33*i+33], aggpk[0..<0], msg, true, rand[0..<0], other[..], pub[66*i..<66*i+66], nw[..]) == MS_OK;
  }
  #assert ms_nonce_agg_vartime(pub[0..<198], agg[..]).code == MS_OK;
  nf: [1] bool;
  return ms_sign(sec, sk[0..<32], agg[..], pks, pks[0..<0], nf[0..<0], msg, psig, sw);
}
// What anyone can compute from a public partial signature s: with σ the
// sign of R's y, σ s = k_1 + b k_2 + σ e a d', where d' = g d. One row
// (1, b, σ e a | σ s) of three equations in k_1, k_2 and d'.
reuse_row :: (psig: [] u8, pub: [] u8, pks: [] u8, msg: [] u8) -> [4] ScScalar {
  agg: [66] u8;
  #assert ms_nonce_agg_vartime(pub[0..<198], agg[..]).code == MS_OK;
  nf: [1] bool;
  x := ms_session_vartime(agg[..], pks, pks[0..<0], nf[0..<0], msg);
  a := ms_session_coeff_vartime(pks, pks[0..<33]).value;
  sigma := ms_parity_vartime(x.r);
  row: [4] ScScalar;
  row[0] = sc_small(1); row[1] = x.b; row[2] = sc_mul(sigma, sc_mul(x.e, a)); row[3] = sc_mul(sigma, sc_load(psig).value);
  return row;
}
det3 :: (m: [3][3] ScScalar) -> ScScalar {
  a := sc_mul(m[0][0], sc_sub(sc_mul(m[1][1], m[2][2]), sc_mul(m[1][2], m[2][1])));
  b := sc_mul(m[0][1], sc_sub(sc_mul(m[1][0], m[2][2]), sc_mul(m[1][2], m[2][0])));
  c := sc_mul(m[0][2], sc_sub(sc_mul(m[1][0], m[2][1]), sc_mul(m[1][1], m[2][0])));
  return sc_add(sc_sub(a, b), c);
}
// Cramer's rule for the third unknown, d'.
solve :: (rows: [3][4] ScScalar) -> ScScalar {
  m: [3][3] ScScalar; md: [3][3] ScScalar;
  for i: 0..<3 { for j: 0..<3 { m[i][j] = rows[i][j]; md[i][j] = ifx j == 2 then rows[i][3] else rows[i][j]; } }
  inverse := sc_inverse(det3(m));
  #assert inverse.ok;
  return sc_mul(det3(md), inverse.value);
}
```

```shell
coax/bin/coax run bitcoin/examples/musig2.coax --run-steps 250000000
```

It prints:

```text
KeyAgg of BIP-327's keys X1, X2, X3:
  L = hash_KeyAgg list(X1 || X2 || X3): 9d3ba89d01849c97649989f2a441f701be800aaa15d957e687d4f56479bc49b9
  the second key: X2, so its coefficient is 1
  a_1: ad0537c883813849e3b95ce5db1d45eb25cc5fae197c4e8759719065932aa183
  a_2: 0000000000000000000000000000000000000000000000000000000000000001
  a_3: f45bb025038e68f050ce4dbf8c63e613678892e2a3e930780afe6293376005c6
  Q of X1, X2, X3: 90539eede565f5d054f32cc0c220126889ed1e5d193baf15aef344fe59d4610c, as published: yes
  Q of X3, X2, X1 (the order changes Q): 6204de8b083426dc6eaf9502d27024d53fc826bf7d2012148a0575435df54b2b, as published: yes
  Q of X1 three times: b436e3bad62b8cd409969a224731c193d051162d8c5ae8b109306127da3aa935, as published: yes
  Q of X1, X1, X2, X2: 69bc22bfa5d106306e48a20679de1d7389386124d07571d0d872686028c26a3e, as published: yes
  refused: pubkey (signer 1)
  refused: pubkey (signer 1)
  refused: pubkey (signer 0)
  refused: The tweak must be less than n. (signer -1)
  refused: The result of tweaking cannot be infinity. (signer -1)
KeySort:
  023590a94e768f8e1815c2f24b4d80a8e3149316c3518ce7b7ad338368d038ca66
  02dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8
  02dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8
  02dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eff
  02f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9
  03dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659
Tweaking the key of tweak_vectors (plain, plain, x-only, x-only):
  Q: 03e2e14a303b7adeeaae81e72e9f26f75fb43102011b3803198351b48c82956c1f
  after plain tweak 1: 03c7a4356ba33438b49ef0141e9f00eb8146d21ca1e4fcd7f7fecefac2ba4943de, gacc is 1
  after plain tweak 2: 02fb7b7f46ee1ee138e039ecc8ef999ef231e485c666756a99f2569863219c257a, gacc is 1
  after x-only tweak 3: 03f26d7e8d67d4c46dc5a960a23cb67e828a224fd040faae29aff625c77bafc376, gacc is 1
  after x-only tweak 4: 0309faf3edbb16169fd17cbb8688142ab9099705548cd30761dc9cedc111ca4177, gacc is n - 1
  tacc: 8d14a817082e1d084b60ec1f5947725bcf682484bb616bd325699f405e89a370
Three signers sign one message:
  aggregate key: fd18ec6fb3c2285e3876265dbf6bf574bb9389f2b7da5cc2e797b78b93fd83f0
  signer 1's nonce: 022ba7efd6217eb04ce5c86f6ebe974aecb9076f8070e1adf558e5b120ad668f06022af17a6606dcc6021b92d4475141bc2146a6ca051b8727a82ab4edf245f5f5f7
  signer 2's nonce: 02164a523285a188111ded81e0b49ede0e2c3510750441be7253bdb92d42cbed59033146b7c79e3e09f48eb1c8ca3d6dfaa0c717aabf8dcf03c8e501238a09c36faf
  signer 3's nonce: 02d4dbdaa7fe131b67a7bf703a546a64da80379055f6ef656ed541c29dbe493b7503ef7f1acde574548c025f20826972377a658eb95e919318a33b7e1ce28d3e892f
  aggnonce: 0310798d984f55c30a6636f46fe885ff428e518012bc4d25dc99ff9d558ab49a9103892b798d0c4fd2132a5bdf4d58517f27034849ba0944c75370d82a542d88d472
  b: 973befa212fff41258f9bdcfa99894944a2325082a6e980bb43e97439013e205
  R = R1 + b R2: 0363a2e49ffef909c1a38bcead40eb1df8a4ffdd30a6cd903f3d813f5e0c26deb7
  e: 65f99b2bfeb4437f2e41ebb95653c1c027bd43aac732349b95b1f9fba75bcf37
  signer 1's partial signature: a9d11ac00d28ae95b372e97fe2b2bc5c44ffbaa6d1497fdd3824b8c9e8df4b00
    verifies: ok
  signer 2's partial signature: b18a43768ea1b55df44ea1c4da0043b08f1e778d8196005b30f1eaf224b787f2
    verifies: ok
  signer 3's partial signature: 43f811100b7f332b5a7f5c2c50f0015e08f947c013c7967920067f40eb3a0572
    verifies: ok
  signer 1's with its last bit changed: invalid
  signature: 63a2e49ffef909c1a38bcead40eb1df8a4ffdd30a6cd903f3d813f5e0c26deb79f536f46a749971f0240e7710da3016c22689d0db75e7675c94ac470289a9723
  a valid BIP-340 signature under the aggregate key: yes
The aggregate key as a Taproot internal key:
  TapTweak(Q): d1ea9d044134464b742f112e28a8454f488d743c68e5b4eb29328812ec9bb600
  output key: aaa05411e1c20921bb6d003a2ab19339f76d2254bc044952d3188bdf094bcf2f
  chapter 41's output key of the same internal key: yes
  address: bc1p42s9gy0pcgyjrwmdqqaz4vvn88mk6gj5hszyj5knrz9a7z2teuhs99qud8
  key-path signature: c3add59e2f43e8711e7af265820eb8fc8908c193ad609565e7e7f53430dcab60d08eb865dec6fe0f8fb4c02d706ec15b8a387c5b6f46c653f631ba6aaab67801
  verifies under the output key: yes
Nonce reuse:
  first Sign: ok
  second Sign with the same secnonce: first secnonce value is out of range.
  message 1: s = 86a5fcbd9978c1d2a3f5234ed48a2ddf93b46344ec5ad2a64c65d3fd6e84be07
  message 2: s = 01b634405e7cd3b5dd743da441ff48e752a525e5d3986d19689a247120805988
  message 3: s = 0a55c53b5861ee04e2e1d2b3ecd82824ceabafaaaa2d48c4f75ce38f6b3bb6cf
  solved for the secret key: 1112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f30
  it is signer 1's secret key: yes
DeterministicSign (det_sign_vectors case 1):
  pubnonce: 03d96275257c2fccbb6eeb77bddf51d3c88c26ee1626c6cda8999b9d34f4ba13a60309be2bf883c6abe907fa822d9ca166d51a3dcc28910c57528f6983fc378b7843
  psig: 41ea65093f71d084785b20dc26a887cd941c9597860a21660cbdb9cc2113cad3
  ok, as published: yes
BIP-327's vector files, cases giving their stated result:
  key_sort_vectors: 1 of 1
  key_agg_vectors: 9 of 9
  nonce_gen_vectors: 4 of 4
  nonce_agg_vectors: 5 of 5
  sign_verify_vectors and tweak_vectors, Sign: 18 of 18
  sign_verify_vectors and tweak_vectors, PartialSigVerify: 16 of 16
  det_sign_vectors: 9 of 9
  sig_agg_vectors: 5 of 5
```

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/musig2.out](../fixtures/musig2.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/musig2.coax](../examples/checks/musig2.coax),
which loads the example:

- every rotation of BIP-327's three keys gives a different Q, and after
  KeySort they give one;
- a published partial signature with any one of eight bits changed fails
  PartialSigVerify;
- each published signing case signs once, and the same secnonce then
  refuses for another message.

<!-- source: bitcoin/examples/checks/musig2.coax check -->
```jai
// The order of the keys is part of the key: every rotation of BIP-327's
// three keys aggregates to a different Q, and sorting first gives one Q.
#forall "key order changes Q, and KeySort removes it" r: 0..<3 #steps 4000000000 {
  k: [99] u8; s: [99] u8; base: [256] u8; a: [32] u8; b: [32] u8;
  keys := ms_hex(MS_KEY_AGG[0].keys, base[..]);
  for i: 0..<99 { k[i] = keys[(i + 33 * r) % 99]; s[i] = k[i]; }
  ms_xbytes_vartime(ms_key_agg_vartime(k[..]).q, a[..]);
  ms_xbytes_vartime(ms_key_agg_vartime(keys).q, b[..]);
  #assert bt_same(a[..], b[..]) == (r == 0);
  ms_key_sort_vartime(s[..]); ms_key_sort_vartime(keys);
  ms_xbytes_vartime(ms_key_agg_vartime(s[..]).q, a[..]);
  ms_xbytes_vartime(ms_key_agg_vartime(keys).q, b[..]);
  #assert bt_same(a[..], b[..]);
}
// Any one changed bit of a published partial signature fails
// PartialSigVerify for its signer.
#forall "a changed partial signature fails" bit: 0..<8 #steps 4000000000 {
  c := MS_VERIFY[0];
  s: [32] u8; k: [256] u8; n: [256] u8; f: [1] bool; m: [64] u8;
  sig := ms_hex(c.sig, s[..]);
  sig[(bit * 13) % 32] ^= cast(u8) (1 << bit);
  r := ms_partial_verify_vartime(sig, ms_hex(c.nonces, n[..]), ms_hex(c.keys, k[..]), k[0..<0], f[0..<0], ms_hex(c.msg, m[..]), c.index);
  #assert r.code == MS_INVALID;
}
// A secnonce signs once: after each published signing case, the same
// secnonce refuses, whatever the message.
#forall "a secnonce signs once" i: 0..<6 #steps 4000000000 {
  c := MS_SIGN[i]; w: [1] MsSignWork;
  sec: [97] u8; sk: [32] u8; agg: [66] u8; k: [256] u8; f: [1] bool; m: [64] u8; psig: [32] u8;
  nonce := ms_hex(c.secnonce, sec[..]); key := ms_hex(c.sk, sk[..]); aggnonce := ms_hex(c.aggnonce, agg[..]); keys := ms_hex(c.keys, k[..]);
  #assert ms_sign(nonce, key, aggnonce, keys, k[0..<0], f[0..<0], ms_hex(c.msg, m[..]), psig[..], w[..]).code == MS_OK;
  m[0] ^= 1;
  #assert ms_sign(nonce, key, aggnonce, keys, k[0..<0], f[0..<0], m[0..<32], psig[..], w[..]).code == MS_SECNONCE_1;
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/musig2.coax --run-steps 250000000
```

`coax test` runs 420 checks, including the loaded files' own checks, in about 480 CPU seconds on 2026-11-03.

### The library's own checks

`musig.coax` is an `#authority` file. Its checks run every case of the
eight vector files, the capacity refusals of every procedure, a secret
nonce of zero, a secret key of zero, a secnonce made for another key, the
contributions the internal verifier names, the clearing of NonceGen's
workspace, and a 2-of-2 session whose signature verifies under chapter
41's output key.

```shell
coax/bin/coax test coax/lib/bitcoin/musig.coax --run-steps 250000000
```

`coax test` runs 415 checks, including those of the files it loads, in about 400 CPU seconds on 2026-11-03. Each vector check runs a few cases, so that it stays inside the compile-time work limit. Mutation testing: not run (owner speed order).

### BIP-327's own Python

[musig.py](../../coax/tests/bitcoin/musig.py) loads BIP-327's
`reference.py` from the pinned bytes and runs its own tests over the
pinned vector files, and six random sessions. It checks that
`musig.coax`'s reasons are the reference's error messages and
contribution names, and that `musig_vectors.coax` is what
[make_musig_vectors.py](../../coax/tests/bitcoin/make_musig_vectors.py)
writes. Then it writes, with the reference's functions, the transcript
the example must print, including the address with BIP-350's pinned
reference encoder.

```shell
python3 -I coax/tests/bitcoin/musig.py --coax coax/bin/coax
```

On 2026-11-03 the reference passes every pinned vector file and six random sessions, all 75 lines agree with Python, and the example's output in native debug and release builds equals them. Wasm was not run.

## What this chapter leaves out

- Threshold signatures, t of n: [chapter 102](ch102-frost-threshold-signatures.md).
- Adaptor signatures, which MuSig2 sessions in Lightning use: chapter
  103.
- The PSBT fields for MuSig2 participants, nonces and partial signatures
  (BIP-373): chapter 94 reads PSBTs; these fields are left for a later
  chapter.
- Script-path spends with MuSig2 keys in leaves: chapters 42 and 45.
- Nested MuSig2, where one participant is itself an aggregate.

## Summary

- MuSig2 makes one BIP-340 signature under one x-only key from n
  signers; on the chain it looks like any key-path spend.
- Hashed key coefficients stop the rogue-key attack; the second distinct
  key's coefficient is 1.
- Each signer has two nonces, combined as R = R_1 + b·R_2 with b a hash
  of everything, so no signer can steer R.
- A secret nonce signs once. Sign zeroes it on first use; a copy that
  signs three times gives the secret key away.
- Plain and x-only tweaks, through gacc and tacc, make an aggregate key a
  Taproot output key.
- DeterministicSign is safe only for the signer who goes last.

## Dependencies

- [24. Sign with Schnorr](ch024-sign-with-schnorr.md)
- [41. The key tweak and the key path](ch041-the-key-tweak-and-the-key-path.md)

## Sources

- [BIP-327](https://github.com/bitcoin/bips/blob/master/bip-0327.mediawiki)
  (MuSig2 for BIP-340-compatible multi-signatures), with its
  `reference.py` and its vector files, pinned at revision `ada34196` in
  [coax/tests/vectors/upstream/bip327/](../../coax/tests/vectors/upstream/bip327/bip-0327.mediawiki).
- [BIP-340](https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki)
  (Schnorr signatures) and
  [BIP-341](https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki)
  (Taproot).
- Jonas Nick, Tim Ruffing and Yannick Seurin, "MuSig2: Simple Two-Round
  Schnorr Multi-Signatures", CRYPTO 2021,
  [eprint 2020/1261](https://eprint.iacr.org/2020/1261).
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
