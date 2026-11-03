# 103. Adaptor signatures

An adaptor signature ties a signature to a secret. The signer makes a
*pre-signature* that is locked to a point T = t G. Anyone can check that
it is a correct pre-signature, but nobody can use it as a signature
without t. Whoever completes it with t and publishes the signature
reveals t to everyone who holds the pre-signature.

That one property lets two parties make an exchange atomic without a
script: publishing the signature that takes one payment hands over the
secret that unlocks the other. The final signature is an ordinary
BIP-340 signature, so on chain it looks like any other key-path spend.

## What this chapter teaches

- Pre-signing a BIP-340 signature for an adaptor point T, and why the
  pre-signature carries the parity of the final nonce.
- Pre-verifying: checking a pre-signature against the key, the message
  and T.
- Adapting a pre-signature into a signature with t, and extracting t
  from the pre-signature and the signature.
- Two ways to lose everything: reusing a nonce gives away the key, and
  reusing a point lets anyone complete every pre-signature locked to it.

## Where adaptor signatures are used

- Atomic swaps: one secret completes the signatures on both sides, so
  either both payments happen or neither does.
- Point time-locked contracts (PTLCs): Lightning payments locked to a
  point instead of a payment hash, so each hop can see a different lock.
- Discreet log contracts: an oracle's future signature is the adaptor
  secret, and the outcome it signs decides which pre-signature can be
  completed.

None of these is in this book yet. This chapter builds the primitive
they share.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [adaptor.coax](../../coax/lib/bitcoin/adaptor.coax) | `ads_pubkey(sk, px, w)` | the x-only key a pre-signature verifies under |
| | `ads_presign(sk, msg, adaptor, aux, presig, w)` | a pre-signature for the point T |
| | `ads_preverify_vartime(px, msg, adaptor, presig)` | checks a pre-signature |
| | `ads_adapt(presig, secret, sig)` | completes it with t |
| | `ads_extract_vartime(presig, sig, adaptor, secret)` | reads t back from the signature |
| | `ads_presign_with_nonce`, `ads_recover_key_vartime` | only to show what nonce reuse does |
| [musig.coax](../../coax/lib/bitcoin/musig.coax) | `ms_tag`, `ms_nonce_point`, `ms_cpoint_vartime` | tagged hashes and points (chapter 101) |
| [schnorr_scalar_workspace.coax](../../coax/lib/crypto/schnorr_scalar_workspace.coax) | `ss_key`, `ss_even`, `ss_write` | secret scalars (chapter 24) |
| [schnorr.coax](../../coax/lib/crypto/schnorr.coax) | `schnorr_verify_vartime` | BIP-340 verification (chapter 23) |

The secret key, the nonce and t are secret. They go through chapter
24's constant-time scalar code and fixed-base multiplication. Keys,
points, pre-signatures and signatures are public, and the procedures
that only read them end in `_vartime`.

The library's codes:

<!-- source: coax/lib/bitcoin/adaptor.coax codes -->
```jai
ADS_OK :: 0;
ADS_BAD_KEY :: 1;           // a secret key of 0 or at least n
ADS_BAD_POINT :: 2;         // an adaptor point, public key or R that is not a point
ADS_BAD_SECRET :: 3;        // an adaptor secret of 0 or at least n
ADS_INFINITY :: 4;          // k G + T is the point at infinity
ADS_INVALID :: 5;           // a pre-signature or signature that does not verify
ADS_MISMATCH :: 6;          // a signature of another R, or a secret whose point is not T
ADS_CAPACITY :: 7;          // a wrong length
ADS_REASONS :: [8] string.["OK", "BAD_KEY", "BAD_POINT", "BAD_SECRET", "INFINITY", "INVALID", "MISMATCH", "CAPACITY"];
ADS_PRESIG :: 65;           // R (33 bytes) || s' (32)
```

## The idea

A BIP-340 signature on message m under key P = d G is (R, s) with
s = k + e d, where R = k G and e is the challenge
hash_BIP0340/challenge(x(R) || x(P) || m). It verifies because
s G = R + e P.

For a pre-signature, the signer uses the nonce point R = k G + T instead
of k G, but computes s' = k + e d without t. Then:

- s' G = R - T + e P. Anyone can check that, knowing only P, m, T and the
  pre-signature. That is pre-verification.
- s = s' + t gives s G = R + e P: a valid signature (x(R), s) with nonce
  k + t. Only someone who knows t can compute it. That is adapting.
- Given s' and s, t = s - s'. That is extraction.

The challenge already commits to R, which includes T, so the signature
that comes out is locked to that T.

BIP-340 needs R to have an even y. The signer does not know t, so it
cannot choose k + t; it can only look at R = k G + T. When R has an odd
y, the final nonce must be -(k + t), so the signer uses -k in s', and
the person adapting must subtract t instead of adding it. The
pre-signature therefore carries R in compressed form, 33 bytes, whose
first byte is that parity. With s', it is 65 bytes.

## The challenge

<!-- source: coax/lib/bitcoin/adaptor.coax challenge -->
```jai
// BIP-340's challenge e = hash_BIP0340/challenge(x(R) || x(P) || m) mod n.
ads_challenge :: (rx: [] u8, px: [] u8, msg: [] u8) -> ScScalar {
  w: [1] ShaOwned;
  ms_tag(w[..], "BIP0340/challenge"); ms_add(w[..], rx); ms_add(w[..], px); ms_add(w[..], msg);
  return ms_reduce(w[..]);
}
// The x-only key's point, with an even y (BIP-340's lift_x).
ads_lift_vartime :: (px: [] u8) -> SgResult {
  if px.count != 32 { return SgResult.{}; }
  return sg_lift_x_public(px);
}
```

## Pre-signing

The nonce follows BIP-340: 32 random bytes are hashed and XORed with the
key, then hashed with the rest. The hash here also covers T, under its
own tag `SchnorrAdaptor/nonce`. Pre-signing one message for two
different points then never uses one nonce twice. A nonce of 0 has a
probability of about 2^-256 and stops the program.

<!-- source: coax/lib/bitcoin/adaptor.coax presign -->
```jai
// The secret workspace of a pre-signature. Each return clears it.
AdsWork :: struct { sha: [1] ShaOwned; aux: [32] u8; rand: [32] u8; h: [32] u8; d: [1] SignScalar; point: [1] SignPoint; pk: [33] u8; r: [33] u8; }
ads_clear :: (w: [] AdsWork, code: int) -> int { fresh: AdsWork; w[0] = fresh; return code; }
// The x-only public key of sk into px (32 bytes): the key a pre-signature
// and its signature verify under.
ads_pubkey :: (sk: [] u8, px: [] u8, w: [] AdsWork) -> int {
  if w.count != 1 || sk.count != 32 || px.count != 32 { return ADS_CAPACITY; }
  #assert ss_key(sk, w[0].d[..], false);
  if !ec_declassify(w[0].d[0].valid) { return ads_clear(w, ADS_BAD_KEY); }
  ms_nonce_point(w[0].d[0].safe, w[0].point[..], w[0].pk[..]);
  for i: 0..<32 { px[i] = w[0].pk[1 + i]; }
  return ads_clear(w, ADS_OK);
}
// The pre-signature of msg under sk for the adaptor point T (33 bytes),
// into presig (65 bytes). As in BIP-340, aux is 32 fresh random bytes that
// are masked with the key: rand = d XOR hash_BIP0340/aux(aux). The nonce
// is k = hash_SchnorrAdaptor/nonce(rand || T || x(P) || m) mod n. T is in
// the hash, so pre-signing one message for two points gives two nonces.
ads_presign :: (sk: [] u8, msg: [] u8, adaptor: [] u8, aux: [] u8, presig: [] u8, w: [] AdsWork) -> int {
  if w.count != 1 || sk.count != 32 || adaptor.count != 33 || aux.count != 32 || presig.count != ADS_PRESIG { return ADS_CAPACITY; }
  if !ms_cpoint_vartime(adaptor).ok { return ADS_BAD_POINT; }
  #assert ss_key(sk, w[0].d[..], false);
  if !ec_declassify(w[0].d[0].valid) { return ads_clear(w, ADS_BAD_KEY); }
  ms_nonce_point(w[0].d[0].safe, w[0].point[..], w[0].pk[..]);
  d := ss_even(w[0].d[0].safe, w[0].point[0].odd);
  s := w[0].sha[..];
  ms_tag(s, "BIP0340/aux"); ms_add(s, aux); ms_end(s, w[0].aux[..]);
  #assert ss_write(d, w[0].h[..]);
  for i: 0..<32 { w[0].rand[i] = w[0].h[i] ^ w[0].aux[i]; }
  ms_tag(s, "SchnorrAdaptor/nonce"); ms_add(s, w[0].rand[..]); ms_add(s, adaptor); ms_add(s, w[0].pk[1..<33]); ms_add(s, msg);
  ms_end(s, w[0].h[..]);
  k := sc_reduce_bytes(w[0].h[..]).value;
  // k = 0 happens with negligible probability.
  #assert ec_declassify(ec_nonzero(k));
  return ads_clear(w, ads_finish(d, k, msg, adaptor, presig, w));
}
// The rest of a pre-signature, once the key d (even-y) and the nonce k are
// known: R = k G + T, then s' = (k, negated when R is odd) + e d.
ads_finish :: (d: ScScalar, k: ScScalar, msg: [] u8, adaptor: [] u8, presig: [] u8, w: [] AdsWork) -> int {
  ms_nonce_point(k, w[0].point[..], w[0].r[..]);              // k G: R - T, so public
  r := sg_add(ms_cpoint_vartime(w[0].r[..]).value, ms_cpoint_vartime(adaptor).value).value;
  if !ms_finite_vartime(r) { return ADS_INFINITY; }
  ms_cbytes_vartime(r, presig[0..<33]);
  e := ads_challenge(presig[1..<33], w[0].pk[1..<33], msg);
  odd := cast(u64) (presig[0] & 1);                            // public
  #assert ss_write(sc_add(ss_even(k, odd), sc_mul(e, d)), presig[33..<65]);
  return ADS_OK;
}
```

## Pre-verifying

`ads_preverify_vartime` computes s' G - e P with chapter 16's
multiplication and compares it with R - T, or with T - R when R has an
odd y. A pre-signature that passes guarantees two things: adapting it
with the discrete logarithm of T gives a valid signature, and that
signature reveals the discrete logarithm.

<!-- source: coax/lib/bitcoin/adaptor.coax preverify -->
```jai
// Checks a pre-signature against the x-only key, the message and T:
// s' G - e P must be R - T, or T - R when R has an odd y.
ads_preverify_vartime :: (px: [] u8, msg: [] u8, adaptor: [] u8, presig: [] u8) -> int {
  if px.count != 32 || adaptor.count != 33 || presig.count != ADS_PRESIG { return ADS_CAPACITY; }
  p := ads_lift_vartime(px); t := ms_cpoint_vartime(adaptor); r := ms_cpoint_vartime(presig[0..<33]);
  if !p.ok || !t.ok || !r.ok { return ADS_BAD_POINT; }
  s := sc_load(presig[33..<65]);
  if !s.ok { return ADS_INVALID; }
  e := ads_challenge(presig[1..<33], px, msg);
  lhs := ecmult_vartime(s.value, p.value, sc_negate(e)).value;
  k := sg_add(r.value, sg_negate(t.value).value).value;
  if presig[0] == 3 { k = sg_negate(k).value; }
  return ifx sg_equal_mask(lhs, k) == CB_MAX64 then ADS_OK else ADS_INVALID;
}
```

## Adapting

Adapting adds t to s', or subtracts it when R is odd. It checks neither
that t belongs to T nor that the pre-signature is valid; the caller
pre-verifies first and verifies the signature afterwards.

<!-- source: coax/lib/bitcoin/adaptor.coax adapt -->
```jai
// Completes a pre-signature with the adaptor secret t: the BIP-340
// signature x(R) || s' + t (s' - t when R is odd) into sig (64 bytes).
// It does not check that t matches T or that the pre-signature is valid:
// pre-verify first, and verify the signature after.
ads_adapt :: (presig: [] u8, secret: [] u8, sig: [] u8) -> int {
  if presig.count != ADS_PRESIG || secret.count != 32 || sig.count != 64 { return ADS_CAPACITY; }
  if !ms_cpoint_vartime(presig[0..<33]).ok { return ADS_BAD_POINT; }
  s := sc_load(presig[33..<65]);
  if !s.ok { return ADS_INVALID; }
  t: [1] SignScalar;
  #assert ss_key(secret, t[..], false);
  if !ec_declassify(t[0].valid) { fresh: SignScalar; t[0] = fresh; return ADS_BAD_SECRET; }
  for i: 0..<32 { sig[i] = presig[1 + i]; }
  #assert ss_write(sc_add(s.value, ss_even(t[0].safe, cast(u64) (presig[0] & 1))), sig[32..<64]);
  fresh: SignScalar; t[0] = fresh;
  return ADS_OK;
}
```

## Extracting

Extraction subtracts the other way and checks the result against T, so a
signature of some other nonce, or one made from a different
pre-signature, is refused.

<!-- source: coax/lib/bitcoin/adaptor.coax extract -->
```jai
// Recovers t from a pre-signature and the signature made from it: t = s -
// s' (s' - s when R is odd), checked against T. Into secret (32 bytes).
ads_extract_vartime :: (presig: [] u8, sig: [] u8, adaptor: [] u8, secret: [] u8) -> int {
  if presig.count != ADS_PRESIG || sig.count != 64 || adaptor.count != 33 || secret.count != 32 { return ADS_CAPACITY; }
  big_t := ms_cpoint_vartime(adaptor);
  if !big_t.ok || !ms_cpoint_vartime(presig[0..<33]).ok { return ADS_BAD_POINT; }
  if !bt_same(sig[0..<32], presig[1..<33]) { return ADS_MISMATCH; }
  s := sc_load(sig[32..<64]); sp := sc_load(presig[33..<65]);
  if !s.ok || !sp.ok { return ADS_INVALID; }
  t := sc_sub(s.value, sp.value);
  if presig[0] == 3 { t = sc_negate(t); }
  q := ecmult_vartime(t, sg_infinity(), sc_small(0)).value;
  if sg_equal_mask(q, big_t.value) != CB_MAX64 { return ADS_MISMATCH; }
  #assert sc_store(t, secret);
  return ADS_OK;
}
```

## Nonce reuse and statement reuse

An adaptor signature fails the same way a signature does when a nonce
repeats. Two pre-signatures with the same k and the same T have the same
R. Their s' values differ only in e d: s1' - s2' = (e1 - e2) d, so
anyone who sees both learns d. `ads_presign` never repeats a nonce
because it hashes the key, T and the message. To show the danger, the
library also has a variant that takes the nonce from the caller, and the
equation that recovers the key.

<!-- source: coax/lib/bitcoin/adaptor.coax danger -->
```jai
// Only to show why a nonce must never be reused: a pre-signature with a
// nonce the caller chooses (32 bytes). ads_presign never reuses one,
// because its nonce hashes the key, T and the message.
ads_presign_with_nonce :: (sk: [] u8, msg: [] u8, adaptor: [] u8, nonce: [] u8, presig: [] u8, w: [] AdsWork) -> int {
  if w.count != 1 || sk.count != 32 || adaptor.count != 33 || nonce.count != 32 || presig.count != ADS_PRESIG { return ADS_CAPACITY; }
  if !ms_cpoint_vartime(adaptor).ok { return ADS_BAD_POINT; }
  k := sc_load(nonce);
  if !k.ok || sc_is_zero(k.value) { return ADS_BAD_SECRET; }
  #assert ss_key(sk, w[0].d[..], false);
  if !ec_declassify(w[0].d[0].valid) { return ads_clear(w, ADS_BAD_KEY); }
  ms_nonce_point(w[0].d[0].safe, w[0].point[..], w[0].pk[..]);
  d := ss_even(w[0].d[0].safe, w[0].point[0].odd);
  return ads_clear(w, ads_finish(d, k.value, msg, adaptor, presig, w));
}
// Two pre-signatures with the same R and different challenges give away
// the key: s1' - s2' = (e1 - e2) d. The even-y key into out (32 bytes).
ads_recover_key_vartime :: (px: [] u8, msg1: [] u8, presig1: [] u8, msg2: [] u8, presig2: [] u8, out: [] u8) -> int {
  if px.count != 32 || presig1.count != ADS_PRESIG || presig2.count != ADS_PRESIG || out.count != 32 { return ADS_CAPACITY; }
  if !bt_same(presig1[0..<33], presig2[0..<33]) { return ADS_MISMATCH; }
  s1 := sc_load(presig1[33..<65]); s2 := sc_load(presig2[33..<65]);
  if !s1.ok || !s2.ok { return ADS_INVALID; }
  de := sc_sub(ads_challenge(presig1[1..<33], px, msg1), ads_challenge(presig2[1..<33], px, msg2));
  inv := sc_inverse(de);
  if !inv.ok || sc_is_zero(de) { return ADS_MISMATCH; }
  d := sc_mul(sc_sub(s1.value, s2.value), inv.value);
  #assert sc_store(d, out);
  return ADS_OK;
}
```

Reusing a point is different: it gives away no key, but it ties
payments together. If two pre-signatures are locked to the same T,
completing one publishes t, and anyone who holds the other pre-signature
can complete it too. In a payment route, every hop that reuses one
point also makes the hops linkable, the same weakness that one payment
hash has in today's HTLCs. Each lock needs its own point. PTLCs give
each hop a different point by adding a random offset per hop.

## The example

The example,
[bitcoin/examples/adaptor-signatures.coax](../examples/adaptor-signatures.coax),
runs one locked payment from Alice to Bob, the six vectors, both
dangers and the refusals.

<!-- source: bitcoin/examples/adaptor-signatures.coax example -->
```jai
#load "../../coax/lib/bitcoin/adaptor.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

put_hex :: (b: [] u8) {
  text: [200] u8;
  r := hex_encode_checked(cast(string) b, text[..], false);
  print("%", cast(string) text[0..<r.written]);
}
line :: (label: string, b: [] u8) { print("  %: ", label); put_hex(b); print("\n"); }
yes :: (b: bool) -> string { return ifx b then "yes" else "no"; }

main :: () {
  w: [1] AdsWork; c := ads_case(0); msg := c.m[0..<c.n];
  presig: [65] u8; sig: [64] u8; t: [32] u8;
  print("Alice pre-signs a payment locked to Bob's point T = t G\n");
  line("message", msg);
  line("Alice's key P", c.px[..]);
  line("Bob's point T", c.adaptor[..]);
  #assert ads_presign(c.sk[..], msg, c.adaptor[..], c.aux[..], presig[..], w[..]) == ADS_OK;
  line("R = k G + T", presig[0..<33]);
  line("s'", presig[33..<65]);
  print("  it pre-verifies under P and T: %\n", ADS_REASONS[ads_preverify_vartime(c.px[..], msg, c.adaptor[..], presig[..])]);
  other := ads_case(1);
  print("  under another point: %\n", ADS_REASONS[ads_preverify_vartime(c.px[..], msg, other.adaptor[..], presig[..])]);
  print("  x(R) || s' is a BIP-340 signature: %\n", yes(schnorr_verify_vartime(msg, c.px[..], presig[1..<65])));

  print("Bob completes it with t and publishes it\n");
  #assert ads_adapt(presig[..], c.t[..], sig[..]) == ADS_OK;
  line("signature", sig[..]);
  print("  BIP-340 verifies it: %\n", yes(schnorr_verify_vartime(msg, c.px[..], sig[..])));

  print("Alice reads t from the signature\n");
  print("  extract: %\n", ADS_REASONS[ads_extract_vartime(presig[..], sig[..], c.adaptor[..], t[..])]);
  line("t", t[..]);
  print("  Bob's t: %\n", yes(bt_same(t[..], c.t[..])));
  print("  with another point: %\n", ADS_REASONS[ads_extract_vartime(presig[..], sig[..], other.adaptor[..], t[..])]);

  print("Completing with a wrong secret\n");
  wrong := c.t; wrong[31] ^= 1;
  #assert ads_adapt(presig[..], wrong[..], sig[..]) == ADS_OK;
  print("  BIP-340 verifies it: %\n", yes(schnorr_verify_vartime(msg, c.px[..], sig[..])));

  print("Every vector\n");
  for i: 0..<ADS_VECTORS.count {
    v := ads_case(i); p: [65] u8; s: [64] u8; back: [32] u8;
    #assert ads_presign(v.sk[..], v.m[0..<v.n], v.adaptor[..], v.aux[..], p[..], w[..]) == ADS_OK;
    ok := ads_preverify_vartime(v.px[..], v.m[0..<v.n], v.adaptor[..], p[..]) == ADS_OK;
    ok = ok && ads_adapt(p[..], v.t[..], s[..]) == ADS_OK && schnorr_verify_vartime(v.m[0..<v.n], v.px[..], s[..]);
    ok = ok && ads_extract_vartime(p[..], s[..], v.adaptor[..], back[..]) == ADS_OK && bt_same(back[..], v.t[..]);
    print("  %: % message bytes, R's y %, round trip %\n", i, v.n, ifx p[0] == 3 then "odd" else "even", yes(ok));
  }

  print("Nonce reuse: Alice pre-signs two messages with one nonce\n");
  nonce: [32] u8; for i: 0..<32 { nonce[i] = 0x5a; }
  second := ads_case(2); msg2 := second.m[0..<second.n];
  p1: [65] u8; p2: [65] u8; key: [32] u8; px: [32] u8;
  #assert ads_presign_with_nonce(c.sk[..], msg, c.adaptor[..], nonce[..], p1[..], w[..]) == ADS_OK;
  #assert ads_presign_with_nonce(c.sk[..], msg2, c.adaptor[..], nonce[..], p2[..], w[..]) == ADS_OK;
  print("  both pre-verify: %, the same R: %\n",
        yes(ads_preverify_vartime(c.px[..], msg, c.adaptor[..], p1[..]) == ADS_OK && ads_preverify_vartime(c.px[..], msg2, c.adaptor[..], p2[..]) == ADS_OK),
        yes(bt_same(p1[0..<33], p2[0..<33])));
  #assert ads_recover_key_vartime(c.px[..], msg, p1[..], msg2, p2[..], key[..]) == ADS_OK;
  line("(s1' - s2') / (e1 - e2)", key[..]);
  #assert ads_pubkey(key[..], px[..], w[..]) == ADS_OK;
  print("  its key is P: %\n", yes(bt_same(px[..], c.px[..])));

  print("Statement reuse: Alice locks two payments to the same T\n");
  #assert ads_presign(c.sk[..], msg2, c.adaptor[..], second.aux[..], p2[..], w[..]) == ADS_OK;
  print("  the second pre-verifies: %\n", ADS_REASONS[ads_preverify_vartime(c.px[..], msg2, c.adaptor[..], p2[..])]);
  #assert ads_adapt(presig[..], c.t[..], sig[..]) == ADS_OK;
  print("  Bob completes the first; Carol, who holds both pre-signatures, reads t: %\n",
        ADS_REASONS[ads_extract_vartime(presig[..], sig[..], c.adaptor[..], t[..])]);
  s2: [64] u8;
  #assert ads_adapt(p2[..], t[..], s2[..]) == ADS_OK;
  line("Carol's signature of the second", s2[..]);
  print("  BIP-340 verifies it: %\n", yes(schnorr_verify_vartime(msg2, c.px[..], s2[..])));

  print("Refused\n");
  zero: [32] u8; bad: [33] u8; bad[0] = 2;
  print("  a secret key of 0: %\n", ADS_REASONS[ads_presign(zero[..], msg, c.adaptor[..], c.aux[..], p1[..], w[..])]);
  print("  T not on the curve: %\n", ADS_REASONS[ads_presign(c.sk[..], msg, bad[..], c.aux[..], p1[..], w[..])]);
  print("  adapting with t = 0: %\n", ADS_REASONS[ads_adapt(presig[..], zero[..], sig[..])]);
  #assert ads_adapt(presig[..], c.t[..], sig[..]) == ADS_OK;
  print("  extracting from another R's signature: %\n", ADS_REASONS[ads_extract_vartime(p2[..], sig[..], c.adaptor[..], t[..])]);
}
```

The output:

```text
Alice pre-signs a payment locked to Bob's point T = t G
  message: 123684d93fa44e3d3aefc69c02950f4aabf8c56f6176d981d4266ccfee4f88bd
  Alice's key P: f5dd8b2cb05845d9ada7403fbf73402378f2daeb2175c7dd50786381658ab085
  Bob's point T: 03d6b2818f5d795afe2f79b65c1a34bf7267f6b28a3ceec197233fd5a6d2e60e93
  R = k G + T: 02dbeb5270b87ce647fdbc51eb659d3a92ce9bb0f138fcc6ba49eb4eefb5ae5183
  s': 67abdffdb20a2ef2b4205b0f72972298cba5465d35d4e3c3c58c3c9a8a9e5296
  it pre-verifies under P and T: OK
  under another point: INVALID
  x(R) || s' is a BIP-340 signature: no
Bob completes it with t and publishes it
  signature: dbeb5270b87ce647fdbc51eb659d3a92ce9bb0f138fcc6ba49eb4eefb5ae5183b8a8655d918f51688208843d4071b225a10d80cb18db1895ca5c67e99663bada
  BIP-340 verifies it: yes
Alice reads t from the signature
  extract: OK
  t: 50fc855fdf852275cde8292dcdda8f8cd5683a6de30634d204d02b4f0bc56844
  Bob's t: yes
  with another point: MISMATCH
Completing with a wrong secret
  BIP-340 verifies it: no
Every vector
  0: 32 message bytes, R's y even, round trip yes
  1: 0 message bytes, R's y even, round trip yes
  2: 1 message bytes, R's y odd, round trip yes
  3: 64 message bytes, R's y odd, round trip yes
  4: 7 message bytes, R's y odd, round trip yes
  5: 32 message bytes, R's y odd, round trip yes
Nonce reuse: Alice pre-signs two messages with one nonce
  both pre-verify: yes, the same R: yes
  (s1' - s2') / (e1 - e2): eb5ef26ad2aaf2b3812094299a35feb0493e8165c0ce9182fb3d97d264858fef
  its key is P: yes
Statement reuse: Alice locks two payments to the same T
  the second pre-verifies: OK
  Bob completes the first; Carol, who holds both pre-signatures, reads t: OK
  Carol's signature of the second: bd96d8bf4be24b80ea6e4271b36a755b8941a32fb3f2c0af9947aeb56d48974ee71a76f27ba9771ad008e566d1104c07d28ad4ba2591fadf986aa63ea90e6157
  BIP-340 verifies it: yes
Refused
  a secret key of 0: BAD_KEY
  T not on the curve: BAD_POINT
  adapting with t = 0: BAD_SECRET
  extracting from another R's signature: MISMATCH
```

What the output shows:

- Alice's pre-signature pre-verifies under her key and Bob's point, and
  not under another point. Its last 64 bytes are not a BIP-340
  signature.
- Bob completes it with t. The result verifies as a BIP-340 signature,
  and Alice reads Bob's t back from it.
- A wrong secret gives a signature that does not verify.
- All six vectors round-trip, with messages of 0, 1, 7, 32 and 64 bytes
  and R of either parity.
- Two pre-signatures with one nonce give away Alice's key: the scalar
  recovered from them has her public key.
- With one point for two payments, Carol completes the second payment's
  signature with nothing but the first signature and the two
  pre-signatures.

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/adaptor-signatures.out](../fixtures/adaptor-signatures.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/adaptor-signatures.coax](../examples/checks/adaptor-signatures.coax),
which loads the example:

- for every vector, the pre-signature does not pre-verify with another
  key, another point or another message
- for every vector, extraction returns the secret adapting used, and the
  pre-signature alone is never a valid signature

<!-- source: bitcoin/examples/checks/adaptor-signatures.coax check -->
```jai
// A pre-signature binds its key, its point and its message: with the next
// vector's key, point or message in place of its own, it does not
// pre-verify.
#forall "a pre-signature verifies only under its own key, point and message" k: 0..<6 #steps 3000000000 {
  a := ads_case(k); b := ads_case((k + 1) % 6); w: [1] AdsWork; p: [65] u8;
  msg := a.m[0..<a.n];
  #assert ads_presign(a.sk[..], msg, a.adaptor[..], a.aux[..], p[..], w[..]) == ADS_OK;
  #assert ads_preverify_vartime(b.px[..], msg, a.adaptor[..], p[..]) == ADS_INVALID;
  #assert ads_preverify_vartime(a.px[..], msg, b.adaptor[..], p[..]) == ADS_INVALID;
  #assert ads_preverify_vartime(a.px[..], b.m[0..<b.n], a.adaptor[..], p[..]) == ADS_INVALID;
}
// Adapting and extracting undo each other: the secret read back from the
// signature is the one that completed it, whichever parity R has, and a
// pre-signature alone is never a valid signature.
#forall "extract returns the secret adapt used" k: 0..<6 #steps 3000000000 {
  a := ads_case(k); w: [1] AdsWork; p: [65] u8; s: [64] u8; t: [32] u8;
  msg := a.m[0..<a.n];
  #assert ads_presign(a.sk[..], msg, a.adaptor[..], a.aux[..], p[..], w[..]) == ADS_OK;
  #assert !schnorr_verify_vartime(msg, a.px[..], p[1..<65]);
  #assert ads_adapt(p[..], a.t[..], s[..]) == ADS_OK && schnorr_verify_vartime(msg, a.px[..], s[..]);
  #assert ads_extract_vartime(p[..], s[..], a.adaptor[..], t[..]) == ADS_OK && bt_same(t[..], a.t[..]);
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/adaptor-signatures.coax
```

`coax test` runs 425 checks, including the loaded files' own checks, in
about four and a half minutes on 2026-11-03.

### The library's own checks

`adaptor.coax` is an `#authority` file. Its checks run every vector
through pre-signing, pre-verification, adapting, BIP-340 verification
and extraction and compare each value; recover a key from a reused
nonce; complete a second pre-signature from a reused point; and refuse
bad keys, points, secrets, lengths, pre-signatures and signatures.

```shell
coax/bin/coax test coax/lib/bitcoin/adaptor.coax
```

`coax test` runs 421 checks, including those of the files it loads, in
about four minutes on 2026-11-03. Mutation testing: not run (owner speed
order).

### Vectors from an independent implementation

No BIP publishes adaptor signature vectors. So
[adaptor.py](../../coax/tests/bitcoin/adaptor.py) makes them: it
implements pre-signing, pre-verification, adapting and extraction on
the point and hash functions of BIP-340's pinned `reference.py`, checks
every final signature with that file's `schnorr_verify`, and writes
[adaptor_vectors.coax](../../coax/tests/bitcoin/adaptor_vectors.coax).
It shares no code with Coax. The script also computes every line the
example prints.

```shell
python3 -I coax/tests/bitcoin/adaptor.py --coax coax/bin/coax
```

On 2026-11-03 all 6 vectors and all 40 lines agree, and the example's
output in native debug and release builds equals them. Wasm was not run.

## What this chapter leaves out

- Scriptless scripts: building contracts, such as atomic swaps, out of
  signatures alone. Adaptor signatures are their main tool.
- PTLCs, which replace the payment hash of HTLCs with points and
  adaptor signatures, so that each hop of a route sees a different lock.
  They need MuSig2 (chapter 101) or ECDSA adaptor signatures in the
  channel, and no BOLT specifies them yet.
- Adaptor signatures for ECDSA, which need a proof that the signer's
  nonce and T share a discrete logarithm.
- MuSig2 with an adaptor point, where the aggregate nonce includes T.

## Summary

- A pre-signature is R = k G + T and s' = k + e d, with k negated when R
  has an odd y; it is 65 bytes.
- Pre-verification checks s' G - e P against R - T.
- Adapting with t gives the BIP-340 signature (x(R), s' ± t), and that
  signature reveals t to whoever holds the pre-signature.
- One nonce for two pre-signatures gives away the key; one point for
  two locks lets anyone complete both once one is completed.

## Dependencies

- [23. Verify a Schnorr signature](ch023-verify-a-schnorr-signature.md)
- [24. Sign with Schnorr](ch024-sign-with-schnorr.md)
- [101. MuSig2](ch101-musig2.md)

## Sources

- [BIP-340](https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki),
  Schnorr signatures, and its `reference.py`, at bitcoin/bips revision
  `5aadddb5`, pinned in
  [coax/tests/vectors/upstream/bip340/](../../coax/tests/vectors/upstream/bip340/reference.py).
- Andrew Poelstra, "Scriptless Scripts", 2017, which introduced adaptor
  signatures.
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
