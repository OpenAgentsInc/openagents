# 77. Faster verification: the GLV endomorphism

A node verifies a signature for almost every input of every transaction it
downloads. Chapter 16's `ecmult_vartime` computes the sum `a·G + b·P` that
both ECDSA (chapter 19) and BIP-340 (chapter 23) need, with one chain of
256 doublings. secp256k1 has a shortcut. Multiplying a point by one
particular scalar, `λ`, costs one field multiplication: `λ·(x, y) =
(β·x, y)`. Gallant, Lambert and Vanstone (2001) showed how to use such a
map to split each 256-bit scalar into two halves of about 128 bits, so the
chain needs only about 128 doublings.

This chapter builds the split exactly as libsecp256k1 does, runs the four
half-length terms through one chain, and moves chapter 19's and chapter
23's verifiers onto it. Every result stays the same: the same Wycheproof
and BIP-340 suites pass, and both paths give the same point on thousands of
seeded inputs. Verification gets about 1.6 times faster, and the chapter
says how far it still is from libsecp256k1.

## What this chapter teaches

- Why secp256k1 has the endomorphism: `p ≡ 1` and `n ≡ 1 (mod 3)`, so
  both fields have cube roots of 1.
- How a scalar `k` splits into `r1 + λ·r2` with both halves within `2^128`
  of zero, using libsecp256k1's precomputed constants and its rounding.
- How four wNAF terms over `G`, `λG`, `P` and `λP` share one chain of
  doublings (Strauss's method, also called Shamir's trick).
- What the split saves, counted and timed, and what it does not.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [secp_glv.coax](../../coax/lib/crypto/secp_glv.coax) | `GLV_BETA`, `GLV_LAMBDA` | the cube roots of 1 |
| | `glv_mul_shift_vartime` | `round(k·g / 2^384)` |
| | `glv_split_vartime`, `glv_half_vartime` | `k = r1 + λ·r2`, and each half as a sign and a magnitude |
| | `glv_lambda_point`, `GLV_G_TABLE`, `GLV_G_LAMBDA_TABLE` | `λ·P`, and the odd multiples of `G` and `λG` |
| | `glv_ecmult_counted_vartime`, `ecmult_glv_vartime` | `a·G + b·P` on the four halves |
| | `glv_strauss_count_vartime` | the work chapter 16's path does, for comparison |
| [ecdsa_verify.coax](../../coax/lib/crypto/ecdsa_verify.coax) | `ev_prepared_matches_vartime` | chapter 19's equation, now on this path |
| [schnorr.coax](../../coax/lib/crypto/schnorr.coax) | `sv_prepared_matches_vartime` | chapter 23's equation, now on this path |

Like chapter 16's `ecmult_vartime`, every procedure here branches and
indexes on its inputs, so its name ends in `_vartime`. It is for public
data only. Signing, key generation and ECDH keep their fixed-work paths
(chapters 16, 20 and 24); this chapter does not change them.

## Why the endomorphism exists

The curve is `y² = x³ + 7`. If `β³ = 1` in the field, then `(β·x)³ = x³`,
so `(β·x, y)` is on the curve whenever `(x, y)` is. The field's nonzero
elements form a cyclic group of order `p - 1`, and `p ≡ 1 (mod 3)`, so it
has elements of order 3: `β` is one of them.

The map `(x, y) → (β·x, y)` respects the group law, and the group has prime
order `n`, so the map is multiplication by some scalar `λ`. Applying it
three times gives `β³·x = x`, so `λ³ = 1 (mod n)`, which needs `n ≡ 1
(mod 3)`; secp256k1's `n` is. Both `β` and `λ` are roots of `X² + X + 1`,
the factor of `X³ - 1` without the root 1. There are two choices of each;
libsecp256k1 pairs the two below.

<!-- source: coax/lib/crypto/secp_glv.coax constants -->
```jai
// β is a cube root of 1 modulo p, and λ the matching cube root of 1
// modulo n: λ·(x, y) = (β·x, y) for every point. Both exist because
// p ≡ 1 and n ≡ 1 (mod 3). Field limbs are 26 bits, little-endian
// (chapter 13); scalar limbs are 32 bits, little-endian (chapter 14).
GLV_BETA :: SecpField.{limb=[10] u64.[26542574,39520796,10050579,30690262,27062345,51187002,38042090,1851833,36398460,2013786]};
GLV_LAMBDA :: ScScalar.{limbs=[8] u32.[455327090,3741488764,545351288,305013482,2282906714,2770738178,3227267296,1399041356]};
// libsecp256k1's secp256k1_scalar_split_lambda: -b1 and -b2 from the
// reduced lattice basis {a1 + b1·λ, a2 + b2·λ} of the multiples of n, and
// g1 = round(2^384·b2/n), g2 = round(2^384·(-b1)/n).
GLV_MINUS_B1 :: ScScalar.{limbs=[8] u32.[180348099,1867808681,17729576,3829628630,0,0,0,0]};
GLV_MINUS_B2 :: ScScalar.{limbs=[8] u32.[1035032108,3613773224,125056109,2317880005,4294967294,4294967295,4294967295,4294967295]};
GLV_G1 :: ScScalar.{limbs=[8] u32.[1172025393,3901956250,1911081599,1034586644,2458184469,3899429092,2815716301,814141985]};
GLV_G2 :: ScScalar.{limbs=[8] u32.[2328133489,359773358,2650080966,571607212,180348100,1867808681,17729576,3829628630]};
```

The checks in `secp_glv.coax` confirm `β³ = 1`, `λ³ = 1`, `λ² + λ + 1 = 0`,
and that 256 doublings' worth of chapter 16's `λ·G` equals `(β·x(G), y(G))`.

## Splitting a scalar

Any `k` can be written as `r1 + λ·r2 (mod n)`: pick any `r2` and solve for
`r1`. The work is finding a pair where both are small. The multiples of `n`
written as `a + b·λ` form a lattice, and a short basis of it,
`{a1 + b1·λ, a2 + b2·λ}`, has entries of about 128 bits. Rounding `k`'s
coordinates in that basis to integers leaves a remainder that is short in
both coordinates (Hankerson, Menezes and Vanstone, algorithm 3.74).

The rounding needs `k·b2/n` and `k·(-b1)/n`. Division is slow, so
libsecp256k1 multiplies by `g1 = round(2^384·b2/n)` and `g2 =
round(2^384·(-b1)/n)` instead, keeps the top 128 bits of the 512-bit
product, and adds bit 383 to round:

<!-- source: coax/lib/crypto/secp_glv.coax shift -->
```jai
// round(k·g / 2^384) for k < n and a 256-bit g: the top 128 bits of the
// 512-bit product, plus bit 383 to round. The result fits in 129 bits.
glv_mul_shift_vartime :: (k: ScScalar, g: ScScalar) -> ScScalar {
  wide: [16] u32;
  for i: 0..<8 {
    carry := cast(u64) 0;
    for j: 0..<8 {
      t := mul_wrap(cast(u64) k.limbs[i], cast(u64) g.limbs[j]) + cast(u64) wide[i+j] + carry;
      wide[i+j] = cast,trunc(u32) (t); carry = t >> 32;
    }
    wide[i+8] = cast,trunc(u32) (carry);
  }
  result: ScScalar; carry := cast(u64) (wide[11] >> 31);
  for i: 0..<8 {
    word := ifx i < 4 then cast(u64) wide[12+i] else cast(u64) 0;
    t := word + carry; result.limbs[i] = cast,trunc(u32) (t); carry = t >> 32;
  }
  return result;
}
```

From the two estimates `c1` and `c2`, `r2 = -(c1·b1 + c2·b2)` and `r1 = k -
λ·r2`. Both are computed mod `n`, so a small negative half appears as a
value just below `n`. `glv_half_vartime` takes the shorter of `r` and
`n - r` and records which one it took; the multiplication then negates the
point instead of the scalar:

<!-- source: coax/lib/crypto/secp_glv.coax split -->
```jai
// k = r1 + λ·r2 (mod n), with r1 and r2 each within 2^128 of 0 or of n
// (libsecp256k1's secp256k1_scalar_split_lambda). c1 and c2 estimate
// k·b2/n and k·(-b1)/n, so r2 = -(c1·b1 + c2·b2) and r1 = k - λ·r2.
glv_split_vartime :: (k: ScScalar) -> GlvSplit {
  key := sc_normalize(k);
  c1 := sc_mul(glv_mul_shift_vartime(key, GLV_G1), GLV_MINUS_B1);
  c2 := sc_mul(glv_mul_shift_vartime(key, GLV_G2), GLV_MINUS_B2);
  r2 := sc_add(c1, c2);
  r1 := sc_add(key, sc_negate(sc_mul(r2, GLV_LAMBDA)));
  return GlvSplit.{r1=r1,r2=r2};
}
// The shorter of r and n - r, and which one it is. After a split, the value
// is below 2^128; a negative half negates its point instead.
glv_half_vartime :: (r: ScScalar) -> GlvHalf {
  wide: [9] u32; carry: u32;
  for i: 0..<8 { wide[i] = (r.limbs[i] << 1) | carry; carry = r.limbs[i] >> 31; }
  wide[8] = carry;
  if sc_subtract_n(wide).borrow == 0 { return GlvHalf.{value=sc_negate(r),negative=true}; }   // 2r >= n
  return GlvHalf.{value=r,negative=false};
}
```

libsecp256k1 proves in `scalar_impl.h` that each half is then below `2^128`
in absolute value. Its tests list twenty scalars whose halves come closest
to that bound; the checks split all twenty and confirm the bound for each.

## λ·P and the tables

For a point in Jacobian coordinates `(X, Y, Z)`, `x = X/Z²`, so multiplying
`X` by `β` multiplies `x` by `β`. The table for `λ·P` costs eight field
multiplications once `P`'s odd multiples are built:

<!-- source: coax/lib/crypto/secp_glv.coax lambda -->
```jai
// λ·P = (β·x, y). In Jacobian coordinates x = X/Z², so βX does the same.
// Infinity stays the canonical infinity.
glv_lambda_point :: (point: SgPoint) -> SgPoint {
  r := sg_normalize(point); r.x = sf_normalize(sf_multiply(r.x, GLV_BETA));
  return sg_canonical_infinity(r);
}
glv_lambda_table :: (table: [8] SgPoint) -> [8] SgPoint {
  r: [8] SgPoint;
  for i: 0..<8 { r[i] = glv_lambda_point(table[i]); }
  return r;
}
// P, 3P, ..., 15P, normalized so that negating y needs no reduction.
glv_odd_table_vartime :: (point: SgPoint) -> [8] SgPoint {
  table := sm_odd_table_vartime(point);
  for i: 0..<8 { table[i] = sg_normalize(table[i]); }
  return table;
}
```

The tables for `G` and `λG` are constants in affine form, computed with
Python integers. Coax's compile-time interpreter stops at 100,000 steps,
too few to build them with `#run`; a check recomputes both with
`sm_odd_table_vartime` and compares every entry.

## Four terms, one chain

`a·G + b·P` becomes `a1·G + a2·(λG) + b1·P + b2·(λP)`. Each half gets a
width-5 wNAF of at most 129 digits, and one loop walks the digits from the
top: one doubling per digit, and one addition per nonzero digit of each
term:

<!-- source: coax/lib/crypto/secp_glv.coax strauss -->
```jai
// Add the term for one signed digit d: |d|·T from the odd table, negated
// when d and the half's sign differ.
glv_add_digit :: (result: SgPoint, table: [8] SgPoint, digit: int, negative: bool) -> SgPoint {
  d := ifx negative then -digit else digit;
  selected := table[((ifx d < 0 then -d else d) - 1) / 2];
  if d < 0 { selected.y = sf_negate(selected.y); }
  return sg_add_valid(result, selected);
}
// a·G + b·P with a = a1 + λ·a2 and b = b1 + λ·b2: four width-5 wNAFs of
// at most 129 digits over G, λG, P and λP, sharing one chain of doublings
// (Strauss, also called Shamir's trick). P must be valid.
glv_ecmult_counted_vartime :: (a: ScScalar, point: SgPoint, b: ScScalar) -> GlvCount {
  p_table := glv_odd_table_vartime(point); l_table := glv_lambda_table(p_table);
  sa := glv_split_vartime(a); sb := glv_split_vartime(b);
  a1 := glv_half_vartime(sa.r1); a2 := glv_half_vartime(sa.r2);
  b1 := glv_half_vartime(sb.r1); b2 := glv_half_vartime(sb.r2);
  w1 := sm_wnaf_vartime(a1.value, SM_P_WINDOW); w2 := sm_wnaf_vartime(a2.value, SM_P_WINDOW);
  w3 := sm_wnaf_vartime(b1.value, SM_P_WINDOW); w4 := sm_wnaf_vartime(b2.value, SM_P_WINDOW);
  length := sm_joint_length_vartime(w1, w2);
  other := sm_joint_length_vartime(w3, w4);
  if other > length { length = other; }
  r := GlvCount.{value=sg_infinity()};
  for round: 0..<length {
    bit := length-1-round;
    if round > 0 { r.value = sg_double_valid(r.value); r.doubles += 1; }
    if w1.digits[bit] != 0 { r.value = glv_add_digit(r.value, GLV_G_TABLE, w1.digits[bit], a1.negative); r.additions += 1; }
    if w2.digits[bit] != 0 { r.value = glv_add_digit(r.value, GLV_G_LAMBDA_TABLE, w2.digits[bit], a2.negative); r.additions += 1; }
    if w3.digits[bit] != 0 { r.value = glv_add_digit(r.value, p_table, w3.digits[bit], b1.negative); r.additions += 1; }
    if w4.digits[bit] != 0 { r.value = glv_add_digit(r.value, l_table, w4.digits[bit], b2.negative); r.additions += 1; }
  }
  return r;
}
// The same result as chapter 16's ecmult_vartime, with an invalid point
// refused the same way.
ecmult_glv_vartime :: (a: ScScalar, point: SgPoint, b: ScScalar) -> SgResult #export_c {
  if sg_valid_mask(point) != CB_MAX64 { return SgResult.{value=sg_infinity()}; }
  return SgResult.{value=glv_ecmult_counted_vartime(a, point, b).value,ok=true};
}
```

Chapter 16's path uses a width-2 wNAF for `a`, with only `±G` in its
table, and a width-5 wNAF for `b`. A width-`w` wNAF has a nonzero digit
about once every `w + 1` positions, so that path spends 256 doublings and
about `256/3 + 256/6 ≈ 128` additions. The four half-length terms spend
about 128 doublings and `4 · 129/6 ≈ 86` additions.

Chapter 19's `ev_prepared_matches_vartime` and chapter 23's
`sv_prepared_matches_vartime` now call `ecmult_glv_vartime` in place of
`ecmult_vartime`. Nothing else in either verifier changes, and chapter 16's
`ecmult_vartime` stays for its other callers.

## The example

[bitcoin/examples/glv.coax](../examples/glv.coax) checks the constants,
splits chapter 19's `u1` and `u2` and three edge scalars, counts the work
of both paths for chapter 19's signature and for BIP-340 vector 1, and
refuses the signature after one bit of its hash changes:

<!-- source: bitcoin/examples/glv.coax example -->
```jai
#load "../../coax/lib/crypto/ecdsa_verify.coax";
#load "../../coax/lib/crypto/schnorr.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [64] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
show_field :: (label: string, f: SecpField) {
  b: [32] u8;
  if sf_store(sf_normalize(f), b[..]) { show(label, b[..]); }
}
show_scalar :: (label: string, s: ScScalar) {
  b: [32] u8;
  if sc_store(s, b[..]) { show(label, b[..]); }
}
// Decode hex into out and return the filled part. Every literal here is
// valid hex, so a decoding error stops the program.
decode :: (text: string, out: [] u8) -> [] u8 {
  r := hex_decode_checked(text, out, false);
  #assert r.error == 0;
  return out[0..<r.written];
}
scalar :: (text: string) -> ScScalar { b: [32] u8; return sc_reduce_bytes(decode(text, b[..])).value; }
// The number of bits in a scalar's value.
bits :: (s: ScScalar) -> int {
  for i: 0..<256 { at := 255 - i; if ((s.limbs[at/32] >> cast(u32) (at%32)) & 1) != 0 { return at + 1; } }
  return 0;
}
// One half of a split, as a sign and a 128-bit magnitude.
show_half :: (label: string, r: ScScalar) {
  h := glv_half_vartime(r); b: [32] u8;
  #assert sc_store(h.value, b[..]);
  text: [32] u8;
  e := hex_encode_checked(cast(string) b[16..<32], text[..], false);
  print("%: % %  (bit length %)\n", label, ifx h.negative then "-" else "+", cast(string) text[0..<e.written], bits(h.value));
}
split :: (label: string, k: ScScalar) {
  show_scalar(label, k);
  s := glv_split_vartime(k);
  show_half("  r1", s.r1);
  show_half("  r2", s.r2);
  print("  r1 + lambda*r2 = k: %\n", sc_equal(sc_add(s.r1, sc_mul(s.r2, GLV_LAMBDA)), k));
}
same :: (a: SgPoint, b: SgPoint) -> bool { return sg_equal_mask(a, b) == CB_MAX64; }

// Chapter 19's signature: libsecp256k1's, for the key 1 and
// SHA-256("Satoshi Nakamoto"), and chapter 23's BIP-340 vector 1.
SIG_1 :: "934b1ea10a4b3c1757e2b0c017d0b6143ce3c9a7e6a4a49860d7a6ab210ee3d82442ce9d2b916064108014783e923ec36b49743e2ffa1c4496f01a512aafd9e5";
HASH_1 :: "a0dc65ffca799873cbea0ac274015b9526505daaaed385155425f7337704883e";
KEY_1 :: "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
BIP340_KEY :: "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659";
BIP340_MSG :: "243f6a8885a308d313198a2e03707344a4093822299f31d0082efa98ec4e6c89";
BIP340_SIG :: "6896bd60eeae296db48a229ff71dfe071bde413e6d43f917dc8dcf8c78de33418906d11ac976abccb20b091292bff4ea897efcb639ea871cfa95f6de339e4b0a";

main :: () {
  // The endomorphism: beta and lambda are cube roots of one.
  one: SecpField; one.limb[0] = 1;
  show_field("beta", GLV_BETA);
  show_scalar("lambda", GLV_LAMBDA);
  print("beta^3 = 1 mod p: %\n", sf_equal_mask(sf_multiply(sf_square(GLV_BETA), GLV_BETA), one) == CB_MAX64);
  lambda2 := sc_mul(GLV_LAMBDA, GLV_LAMBDA);
  print("lambda^3 = 1 mod n: %, lambda^2 + lambda + 1 = 0: %\n",
        sc_equal(sc_mul(lambda2, GLV_LAMBDA), sc_small(1)), sc_is_zero(sc_add(sc_add(lambda2, GLV_LAMBDA), sc_small(1))));
  slow := sg_to_affine(ecmult_vartime(GLV_LAMBDA, sg_infinity(), sc_small(0)).value).value;
  fast := sg_to_affine(glv_lambda_point(sg_generator())).value;
  show_field("x(lambda*G) by 256 doublings", slow.x);
  show_field("beta * x(G)                 ", fast.x);
  print("same point: %\n", same(sg_from_affine_valid(slow), sg_from_affine_valid(fast)));

  // Splits: chapter 19's u1 and u2, n - 1, lambda, and the scalar near
  // libsecp256k1's bound with the longest half.
  key: [33] u8; sig: [64] u8; hash: [32] u8;
  public_key := decode(KEY_1, key[..]); signature := decode(SIG_1, sig[..]); z := decode(HASH_1, hash[..]);
  prepared := ev_prepare_vartime(z, sg_generator(), signature, true);
  split("u1", prepared.u1);
  split("u2", prepared.u2);
  split("n - 1", sc_negate(sc_small(1)));
  split("lambda", GLV_LAMBDA);
  split("near the bound", scalar("7fffffffffffffffffffffffffffffffd576e73557a4501ddfe92f46681b20a0"));

  // Chapter 19's R = u1*G + u2*Q, both ways, with the work each takes.
  old := glv_strauss_count_vartime(prepared.u1, prepared.u2);
  new := glv_ecmult_counted_vartime(prepared.u1, prepared.point, prepared.u2);
  print("chapter 16, wNAF widths 2 and 5:         % doublings, % additions\n", old.doubles, old.additions);
  print("chapter 77, four width-5 wNAFs on halves: % doublings, % additions\n", new.doubles, new.additions);
  r_old := ecmult_vartime(prepared.u1, prepared.point, prepared.u2).value;
  print("same R: %\n", same(r_old, new.value));
  x: [32] u8; #assert sf_store(sg_to_affine(new.value).value.x, x[..]);
  show("x(R)", x[..]);
  show("r   ", signature[0..<32]);
  print("ecdsa_verify_vartime: %\n", ecdsa_verify_vartime(z, public_key, signature, true));

  // A broken case: one bit of the hash changed. Both paths still agree on
  // R, and its x is no longer r, so the signature is refused.
  hash[31] ^= 1;
  bad := ev_prepare_vartime(z, sg_generator(), signature, true);
  r_bad := glv_ecmult_counted_vartime(bad.u1, bad.point, bad.u2).value;
  print("hash bit changed: same R: %\n", same(ecmult_vartime(bad.u1, bad.point, bad.u2).value, r_bad));
  #assert sf_store(sg_to_affine(r_bad).value.x, x[..]);
  show("x(R)", x[..]);
  print("ecdsa_verify_vartime: %\n", ecdsa_verify_vartime(z, public_key, signature, true));
  hash[31] ^= 1;

  // BIP-340 vector 1: R = s*G - e*P.
  k: [32] u8; m: [32] u8; s: [64] u8;
  bk := decode(BIP340_KEY, k[..]); bm := decode(BIP340_MSG, m[..]); bs := decode(BIP340_SIG, s[..]);
  sp := sv_prepare_vartime(bm, bk, bs);
  old = glv_strauss_count_vartime(sp.s, sp.negative_e);
  new = glv_ecmult_counted_vartime(sp.s, sp.point, sp.negative_e);
  print("BIP-340 vector 1: chapter 16 % doublings, % additions; chapter 77 % doublings, % additions\n",
        old.doubles, old.additions, new.doubles, new.additions);
  print("same R: %\n", same(ecmult_vartime(sp.s, sp.point, sp.negative_e).value, new.value));
  print("schnorr_verify_vartime: %\n", schnorr_verify_vartime(bm, bk, bs));
}
```

```shell
coax/bin/coax run bitcoin/examples/glv.coax
```

```text
beta: 7ae96a2b657c07106e64479eac3434e99cf0497512f58995c1396c28719501ee
lambda: 5363ad4cc05c30e0a5261c028812645a122e22ea20816678df02967c1b23bd72
beta^3 = 1 mod p: true
lambda^3 = 1 mod n: true, lambda^2 + lambda + 1 = 0: true
x(lambda*G) by 256 doublings: bcace2e99da01887ab0102b696902325872844067f15e98da7bba04400b88fcb
beta * x(G)                 : bcace2e99da01887ab0102b696902325872844067f15e98da7bba04400b88fcb
same point: true
u1: 5f5aa92cdd8788480fc88a00e9bdccece7b4c4a3f7a3e748d597da03af5520ce
  r1: + 400e16af8a8ede8880b6a47dbc2a8397  (bit length 127)
  r2: - 15db6ddc8fed8963539b67ab5e8203e2  (bit length 125)
  r1 + lambda*r2 = k: true
u2: 111b2f67088463219984f59cf80ca643ae04231d752d9260fbd17e4dabefc35e
  r1: + 34fd233dcbc25ee0af02157ca0ac0cd4  (bit length 126)
  r2: - 3837565729a9a982a57c96a266f4a7e7  (bit length 126)
  r1 + lambda*r2 = k: true
n - 1: fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364140
  r1: - 00000000000000000000000000000001  (bit length 1)
  r2: + 00000000000000000000000000000000  (bit length 0)
  r1 + lambda*r2 = k: true
lambda: 5363ad4cc05c30e0a5261c028812645a122e22ea20816678df02967c1b23bd72
  r1: + 00000000000000000000000000000000  (bit length 0)
  r2: + 00000000000000000000000000000001  (bit length 1)
  r1 + lambda*r2 = k: true
near the bound: 7fffffffffffffffffffffffffffffffd576e73557a4501ddfe92f46681b20a0
  r1: - 2a8918caa85bafe22016d0b917e4dd77  (bit length 126)
  r2: + 59de565a2c9d0e2d4373f7623c1d7cd7  (bit length 127)
  r1 + lambda*r2 = k: true
chapter 16, wNAF widths 2 and 5:         256 doublings, 128 additions
chapter 77, four width-5 wNAFs on halves: 126 doublings, 88 additions
same R: true
x(R): 934b1ea10a4b3c1757e2b0c017d0b6143ce3c9a7e6a4a49860d7a6ab210ee3d8
r   : 934b1ea10a4b3c1757e2b0c017d0b6143ce3c9a7e6a4a49860d7a6ab210ee3d8
ecdsa_verify_vartime: true
hash bit changed: same R: true
x(R): 620af8a37927e9d868270b8eff4dc626b3e2397bf59e712212fff24e71e04a8e
ecdsa_verify_vartime: false
BIP-340 vector 1: chapter 16 256 doublings, 132 additions; chapter 77 128 doublings, 86 additions
same R: true
schnorr_verify_vartime: true
```

`n - 1` splits into `-1 + λ·0`, and `λ` into `0 + λ·1`. The scalar from
libsecp256k1's list has a 127-bit half: the bound is tight. Chapter 16
counts 256 doublings because its loop doubles infinity once before the
first addition; this path skips that doubling.

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/glv.coax](../examples/checks/glv.coax), which
loads the example. For seeded `a`, `b` and `P`, both paths must give the
same point, every half must be below `2^128` and the endomorphism path must
spend at most 129 doublings and fewer additions. Chapter 19's signature
must verify, and each one-bit change to its hash, `r` or `s` must refuse:

<!-- source: bitcoin/examples/checks/glv.coax check -->
```jai
// For seeded scalars a and b and a seeded point P, both paths give the same
// a·G + b·P, every half of the two splits is below 2^128, and the
// endomorphism path spends at most 129 doublings and fewer additions.
#forall "both paths agree on seeded inputs, with half the doublings" seed: 0..<6 #steps 2000000000 {
  a := sc_mul(sc_small(cast(u32) (seed * 2654435761 % 4294967291 + 1)), GLV_G1); a = sc_mul(a, a);
  b := sc_add(sc_mul(sc_small(cast(u32) (seed * 40503 + 7)), GLV_G2), sc_small(cast(u32) seed));
  p := ecmult_vartime(sc_small(cast(u32) (seed + 2)), sg_infinity(), sc_small(0)).value;
  for k: 0..<2 {
    s := glv_split_vartime(ifx k == 0 then a else b);
    #assert glv_half_vartime(s.r1).value.limbs[4] == 0 && glv_half_vartime(s.r2).value.limbs[4] == 0;
  }
  old := glv_strauss_count_vartime(a, b); new := glv_ecmult_counted_vartime(a, p, b);
  #assert new.doubles <= 129 && new.additions < old.additions;
  #assert same(ecmult_vartime(a, p, b).value, new.value);
}
// Chapter 19's signature verifies, and each one-bit change to the hash, r
// or s is refused, on the endomorphism path that ecdsa_verify_vartime now
// takes.
#forall "chapter 19's signature verifies and its one-bit changes refuse" bit: 0..<3 #steps 2000000000 {
  key: [33] u8; sig: [64] u8; hash: [32] u8;
  k := decode(KEY_1, key[..]); s := decode(SIG_1, sig[..]); z := decode(HASH_1, hash[..]);
  #assert ecdsa_verify_vartime(z, k, s, true);
  if bit == 0 { hash[0] ^= 1; } else if bit == 1 { sig[31] ^= 1; } else { sig[63] ^= 1; }
  #assert !ecdsa_verify_vartime(z, k, s, true);
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/glv.coax
```

`coax test` runs 151 checks, including the loaded files' own checks, in
about 30 seconds on 2026-11-03.

### The library's own checks

`secp_glv.coax` is an `#authority` file. Its checks confirm the cube roots
and `λ·G = (β·x(G), y(G))`, the stored tables against computed ones, the
rounding at bit 383, the split and the `2^128` bound on libsecp256k1's
twenty near-bound scalars, on 0, 1, `n - 1` and `λ`, and on 64 seeded
scalars, the sign of a half, six seeded multiplications against chapter
16's path, and the edge inputs: zero scalars, `P` at infinity, sums that
cancel to infinity, `λ·G`, and an invalid point, which is refused as
chapter 16 refuses it.

```shell
coax/bin/coax test coax/lib/crypto/secp_glv.coax
```

`coax test` runs 83 checks, including those of the files it loads, in
about 8 seconds on 2026-11-03. Mutation testing: not run (owner speed
order).

### Python and libsecp256k1

[glv.py](../../coax/tests/bitcoin/glv.py) computes every line of the
example with Python integers: the split transliterates
`secp256k1_scalar_split_lambda`, the counts follow chapter 16's recoding,
and the points come from affine arithmetic. With `--native`, it compiles
[glv_bench.coax](../../coax/tests/bitcoin/glv_bench.coax) to C in checked
and release builds and compares 2,000 splits (1,990 seeded, plus 0, 1, 2,
`n - 1`, `n - 2`, `±λ`, `(n ± 1)/2` and `2^128`) and 300 multiplications
`a·G + b·P` on both paths with Python.

```shell
python3 -I coax/tests/bitcoin/glv.py --coax coax/bin/coax --native
```

On 2026-11-03 all 39 lines agreed with Python, the example's output in
native debug and release builds equaled them, and every split and
multiplication agreed in both builds. The suites of chapters 19 and 23
were rerun on the new path with no change:
[ecdsa_verify.py](../../coax/tests/bitcoin/ecdsa_verify.py) (all 464
Wycheproof cases with both key forms, the low-S variants and 60 seeded
signatures, native and Wasm, against Python and libsecp256k1),
[schnorr.py](../../coax/tests/nostr/schnorr.py) and
[schnorr_fast.py](../../coax/tests/nostr/schnorr_fast.py) (all 19 BIP-340
vectors and 790 cases per build, against libsecp256k1), and the
`coax test` checks of both chapters.

### Measured speed

`glv.py --native` also times three verifiers on the same 200 signatures,
half of them with one bit of `s` changed: chapter 19's or 23's verifier on
chapter 16's path, the same verifier on this path, and libsecp256k1
`v0.8.0`. It reports the best of three passes. The signatures are fresh
ones made in Python, not a pinned block's. On 2026-11-03, on an Apple
arm64 laptop with Apple clang 21 and the release build, every verifier
decided every signature as expected, and the times per verification were:

| Verifier | ECDSA | BIP-340 |
| --- | ---: | ---: |
| Chapter 16's path | 1,056 µs | 1,016 µs |
| The endomorphism path | 675 µs | 633 µs |
| libsecp256k1 `v0.8.0` | 14.9 µs | 15.1 µs |

The endomorphism path is 1.56 times faster for ECDSA and 1.61 times faster
for BIP-340. The doublings halve, but the gain is smaller than 2 because
every addition in chapter 15's complete formula also computes a doubling,
and the additions fall only by a third. libsecp256k1 is still about 45
times faster. Most of that gap is outside this chapter: its field
arithmetic uses five 52-bit limbs and 64-bit products where Coax uses ten
26-bit limbs, its additions take the faster incomplete formulas with
affine table entries, and it multiplies `G` with window-15 tables of 8,192
precomputed points for `G` and `2^128·G`. These timings are from one
machine and one run; they are a comparison, not a benchmark claim.

## What this chapter leaves out

- Larger tables for `G`. libsecp256k1 splits the `G` scalar into two
  128-bit halves at bit 128, not by `λ`, and uses window 15. This path
  keeps chapter 16's window 5 and splits both scalars by `λ`.
- Batch verification of many BIP-340 signatures at once, and the
  multi-scalar algorithms that serve it.
- Signing, key generation and ECDH, which must not depend on secret data.
  The endomorphism can speed up fixed-work multiplication too, as
  libsecp256k1's `ecmult_const_impl.h` does, but this chapter does not touch the
  constant-time paths.
- Other callers of `ecmult_vartime`, such as public-key recovery (chapter
  22) and key derivation (chapter 32). They keep chapter 16's path.

## Summary

- `β` and `λ` are cube roots of 1 modulo `p` and `n`; they exist because
  `p ≡ n ≡ 1 (mod 3)`, and `λ·(x, y) = (β·x, y)`.
- A scalar splits into `r1 + λ·r2` with both halves below `2^128` in
  absolute value, using libsecp256k1's lattice constants and a rounded
  384-bit shift.
- `a·G + b·P` becomes four half-length wNAF terms over `G`, `λG`, `P` and
  `λP` that share one chain of about 128 doublings.
- Chapter 19's and chapter 23's verifiers use it and give the same results
  on every suite; verification is about 1.6 times faster, and libsecp256k1
  remains about 45 times faster.

## Dependencies

- [16. Scalar multiplication](ch016-scalar-multiplication.md)
- [19. Verify ECDSA](ch019-verify-ecdsa.md)
- [23. Verify a Schnorr signature](ch023-verify-a-schnorr-signature.md)

## Sources

- R. Gallant, R. Lambert and S. Vanstone, "Faster Point Multiplication on
  Elliptic Curves with Efficient Endomorphisms", CRYPTO 2001.
- D. Hankerson, A. Menezes and S. Vanstone, *Guide to Elliptic Curve
  Cryptography*, Springer 2004, algorithm 3.74 and section 3.5.
- [libsecp256k1](https://github.com/bitcoin-core/secp256k1), release
  `v0.8.0`, pinned in
  [UPSTREAM.json](../../tools/coax-nostr/fuzz/UPSTREAM.json):
  `src/scalar_impl.h` (`secp256k1_scalar_split_lambda`, its constants and
  its proof), `src/scalar_8x32_impl.h` (`secp256k1_scalar_mul_shift_var`),
  `src/field.h` (`secp256k1_const_beta`), `src/ecmult_impl.h`
  (`secp256k1_ecmult_strauss_wnaf`) and `src/tests.c`
  (`scalars_near_split_bounds`).
