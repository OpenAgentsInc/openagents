# 15. The secp256k1 group law

A secp256k1 public key is a point on the curve

```text
y^2 = x^3 + 7  over F_p,  p = 2^256 - 2^32 - 977
```

The points, together with one extra point *O*, the *point at infinity*,
form a group: you can add two points and get a third, `O` is the identity,
and every point `P` has a negative `-P` with `P + (-P) = O`. This chapter
builds that addition on the field of chapter 13. Scalar multiplication
(chapter 16) and every signature and shared secret after it are repeated
additions.

## What this chapter teaches

- The curve equation, the generator `G`, and the negative `(x, p - y)`.
- The chord-and-tangent rule for adding two points.
- Jacobian coordinates `(X, Y, Z)` for `(X/Z^2, Y/Z^3)`, which add points
  with no field inversion.
- The cases where the generic formulas fail: equal points, opposite points
  and infinity, and how the code handles all of them with masks.
- Lifting an x-only public key to the point with even `y` (BIP-340
  `lift_x`), and refusing an x that is not on the curve.

## Where the code is

[coax/lib/crypto/secp_group.coax](../../coax/lib/crypto/secp_group.coax)
has the group. Its field elements are the `SecpField` values of chapter 13.

| Procedure | Result |
| --- | --- |
| `sg_generator`, `sg_infinity` | `G`, and infinity as the Jacobian triple `(1, 1, 0)` |
| `sg_valid_mask` | all ones when a point satisfies `Y^2 = X^3 + 7 Z^6`, or is the canonical infinity |
| `sg_equal_mask` | all ones when two valid points are the same point, whatever their `Z` |
| `sg_add`, `sg_double`, `sg_negate` | `a + b`, `2a` and `-a`, with `ok` false for an invalid input |
| `sg_mixed_add` | a Jacobian point plus an affine point |
| `sg_to_affine`, `sg_from_affine` | conversion between `(X, Y, Z)` and `(x, y)` |
| `sg_lift_x_public` | 32 bytes of x to the point with even `y`; refuses an x off the curve |
| `sg_decode_public`, `sg_encode_public` | the 33-byte compressed form, prefix `02` or `03` |
| `sg_pick` | one entry of a table of points, reading every entry (chapter 16) |

[coax/lib/crypto/secp_group32.coax](../../coax/lib/crypto/secp_group32.coax)
has the same addition and doubling on eight 32-bit limbs. It is an
experimental kernel that the scalar multiplications of chapter 16 and the
fast ECDH path load; its checks compare it with `secp_group.coax`.

## The curve and its points

The generator is a fixed point that SEC 2 publishes:

```text
G.x = 79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798
G.y = 483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8
```

If `(x, y)` is on the curve, so is `(x, -y)`, because only `y^2` appears in
the equation. That point is `-P`. In the field, `-y` is `p - y`.

### Worked example: −G

`p` ends in `...fffffc2f` and `G.y` ends in `...ffb10d4b8`. Subtract the
last four hex digits: `fc2f - d4b8 = 2777`, with no borrow. Every earlier
digit of `p` is `f` except one `e`, so each digit of `p - G.y` is `f` minus
the digit of `G.y`, or `e` minus it at that place:

```text
-G.y = b7c52588d95c3b9aa25b0403f1eef75702e84bb7597aabe663b82f6f04ef2777
```

## Adding two points

Draw the line through `P` and `Q`. It meets the curve in exactly one more
point, `R'`. The sum `P + Q` is the reflection of `R'`, `(x, -y)`. For
`P = Q`, the line is the tangent at `P`. In formulas, with `λ` the slope of
the line:

```text
λ = (y2 - y1) / (x2 - x1)        if P != Q
λ = 3 x1^2 / (2 y1)              if P == Q
x3 = λ^2 - x1 - x2
y3 = λ (x1 - x3) - y1
```

The rule breaks down in three cases:

- `x1 = x2` and `y1 = -y2`: the line is vertical, `P = -Q`, and the sum is
  `O`.
- `P = Q` with `y1 = 0`: the tangent is vertical, and `2P = O`. No point
  on secp256k1 has `y = 0`: such a point would have order 2, and the group
  has an odd prime number of elements, `n` (chapter 14).
- Either input is `O`: then the sum is the other input.

### Worked example: a small curve

The same equation over `F_17` has 18 points, including `O`, so you can
follow the formulas by hand. Take `P = (1, 5)`: `5^2 = 25 = 8` and
`1^3 + 7 = 8`.

Double `P`:

1. `λ = 3 · 1^2 / (2 · 5) = 3 / 10`. The inverse of 10 mod 17 is 12,
   because `10 · 12 = 120 = 7 · 17 + 1`. So `λ = 3 · 12 = 36 = 2`.
2. `x3 = 2^2 - 1 - 1 = 2`.
3. `y3 = 2 · (1 - 2) - 5 = -7 = 10`.

`2P = (2, 10)`. Check: `10^2 = 100 = 15` and `2^3 + 7 = 15`.

Add `P` and `2P`:

1. `λ = (10 - 5) / (2 - 1) = 5`.
2. `x3 = 5^2 - 1 - 2 = 22 = 5`.
3. `y3 = 5 · (1 - 5) - 5 = -25 = 9`.

`3P = (5, 9)`. Check: `9^2 = 81 = 13` and `5^3 + 7 = 132 = 13`.

On secp256k1 the numbers are 256 bits long, and each step is a call into
the field code.

## Jacobian coordinates

Each affine addition divides once, and a field inverse costs about 270
multiplications (chapter 13). Jacobian coordinates remove the division. A
point is a triple `(X, Y, Z)` that stands for the affine point
`(X / Z^2, Y / Z^3)`. Many triples stand for the same point: `(X, Y, Z)`
and `(X c^2, Y c^3, Z c)` are equal for every nonzero `c`. The curve
equation becomes

```text
Y^2 = X^3 + 7 Z^6
```

`Z = 0` stands for `O`. The code uses one triple for it, `(1, 1, 0)`, and
treats any other triple with `Z = 0` as invalid.

`sg_double_valid` uses the `dbl-2009-l` formulas and `sg_add_valid` the
`add-2007-bl` formulas from the Explicit-Formulas Database, for curves
with `a = 0`. Both use only additions, subtractions, multiplications and
squarings. You convert to affine once, at the end, with one inverse of
`Z`.

Two Jacobian points are equal when their cross-products are:
`X1 Z2^2 = X2 Z1^2` and `Y1 Z2^3 = Y2 Z1^3`. `sg_equal_mask` checks both,
so it compares points, not triples.

### Worked example: G with Z = 2

Take `c = 2`. The triple `(4 G.x, 8 G.y, 2)` is `G`. To find `4 G.x mod p`,
multiply by 4 and fold the bit above 2^256 back in, as chapter 13 does:
`2^256 ≡ 2^32 + 977 = 1000003d1` in hex. `4 G.x` is
`1e6f999f...6c5be05e60`, which is `2^256` plus `e6f999f...6c5be05e60`.
Add `1000003d1` to the low part: `...c5be05e60 + 1000003d1 = ...d5be06231`.

```text
4 G.x mod p = e6f999fbe772eeb156818a573a1c2c1c0a6ff36cb738a36567ca056d5be06231
```

## Every addition case, selected with masks

The generic addition formula computes `H = U2 - U1`, where `U1 = X1 Z2^2`
and `U2 = X2 Z1^2`. `H` is zero exactly when the two points have the same
x. Then the formula's result has `Z = 0`:

- If also `S1 = S2` (same y), the points are equal and the right answer is
  the doubling.
- Otherwise the points are opposite, and `Z = 0` is the right answer, `O`.

`sg_add_valid` always computes the generic sum and the doubling of the
first input. Then it selects with masks, in this order:

1. the doubling, if `H = 0` and `S1 = S2`;
2. the second input, if the first is infinity;
3. the first input, if the second is infinity.

A last step replaces any result with `Z = 0` by the canonical `(1, 1, 0)`.
Every input runs the same field operations, and the choice is never a
branch (chapter 3). `sg_mixed_add` converts its affine input and uses the
same kernel.

`sg_add`, `sg_double` and `sg_negate` check their inputs with
`sg_valid_mask` first. An input off the curve gives `ok` false and the
value infinity; the arithmetic still runs on it.

## Lifting an x-only key

A BIP-340 public key, as Taproot outputs and Nostr use it, is 32 bytes: the
x of a point. BIP-340 defines
`lift_x(x)`: the point `(x, y)` with `y^2 = x^3 + 7` and `y` even. It
fails when `x >= p` or when `x^3 + 7` is not a square.

`sg_lift_x_public`:

1. refuses a view that isn't 32 bytes;
2. loads x, stores it back, and compares the bytes, so an x of `p` or more
   is refused rather than reduced;
3. computes `x^3 + 7` and its even square root with `sf_sqrt` (chapter 13);
4. accepts only if the root squares back to `x^3 + 7`.

`sg_decode_public` reads the 33-byte compressed form: `02` for even `y`,
`03` for odd, and then x. The one byte `00` decodes as infinity, for
general group tools; a public key must be finite.

### Worked example: which x lift

About half of all x values have a point. Quadratic reciprocity decides
small cases by hand.

- **x = 0.** `0 + 7 = 7`. `p` and 7 are both `3 mod 4`, so
  `(7 / p) = -(p / 7)`. Since `2^3 = 8 ≡ 1 (mod 7)`, `2^256 ≡ 2`,
  `2^32 ≡ 4` and `977 ≡ 4`, so `p ≡ 2 - 4 - 4 ≡ 1 (mod 7)` and
  `(p / 7) = 1`. So `(7 / p) = -1`: 7 is not a square, and `lift_x(0)`
  is refused.
- **x = 1.** `1 + 7 = 8 = 2 · 2^2`. 2 is a square mod `p` when
  `p ≡ ±1 (mod 8)`; here `p ≡ 0 - 0 - 1 ≡ 7 (mod 8)`. So 8 is a square,
  and `lift_x(1)` succeeds.
- **x = 7.** `343 + 7 = 350 = 2 · 5^2 · 7`. 2 and `5^2` are squares and 7
  is not, so 350 is not, and `lift_x(7)` is refused.
- **x = p + 1.** It reduces to 1, which lifts, but it is not below `p`,
  so it is refused.

## The example

The example is [bitcoin/examples/group-law.coax](../examples/group-law.coax).
It negates, doubles and adds `G`, shows `G` with `Z = 2`, encodes `G`,
lifts `x = 1` and shows five refusals.

<!-- source: bitcoin/examples/group-law.coax example -->
```jai
#load "../../coax/lib/crypto/secp_group.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [80] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// Print a field element as 32 big-endian bytes.
show_field :: (label: string, a: SecpField) {
  b: [32] u8;
  if sf_store(a, b[..]) { show(label, b[..]); }
}
// Print a point's affine x and y, or "infinity".
show_point :: (label: string, p: SgPoint) {
  a := sg_to_affine(p);
  if !a.ok { print("%: not a valid point\n", label); return; }
  if a.value.infinity != 0 { print("%: infinity\n", label); return; }
  print("%:\n", label);
  show_field("  x", a.value.x);
  show_field("  y", a.value.y);
}
// Decode 64 hex digits into 32 bytes; false if the hex is bad.
bytes_of :: (text: string, out: [] u8) -> bool {
  return hex_decode_checked(text, out, false).error == 0;
}
// True when two points are the same valid point.
same_point :: (a: SgPoint, b: SgPoint) -> bool { return sg_equal_mask(a, b) == CB_MAX64; }
// lift_x of 64 hex digits.
lift_hex :: (text: string) -> SgResult {
  b: [32] u8;
  if !bytes_of(text, b[..]) { print("bad hex\n"); }
  return sg_lift_x_public(b[..]);
}

main :: () {
  g := sg_generator();
  inf := sg_infinity();
  show_point("G", g);
  print("G is on the curve: %\n", sg_valid_mask(g) == CB_MAX64);

  // The negative of (x, y) is (x, p - y).
  minus_g := sg_negate(g).value;
  show_point("-G", minus_g);
  print("G + (-G) is infinity: %\n", same_point(sg_add(g, minus_g).value, inf));
  print("G + infinity is G: %\n", same_point(sg_add(g, inf).value, g));

  // Doubling and addition.
  two := sg_double(g).value;
  print("2G = G + G: %\n", same_point(two, sg_add(g, g).value));
  show_point("2G", two);
  three := sg_add(two, g).value;
  show_point("3G", three);

  // The same point in other Jacobian coordinates: (X Z^2, Y Z^3, Z) for Z = 2.
  z: SecpField; z.limb[0] = 2;
  scaled := g;
  scaled.x = sf_multiply(g.x, sf_square(z));
  scaled.y = sf_multiply(g.y, sf_multiply(sf_square(z), z));
  scaled.z = z;
  show_field("G with Z = 2, X", scaled.x);
  print("equal to G: %\n", same_point(scaled, g));
  print("scaled G + G = 2G: %\n", same_point(sg_add(scaled, g).value, two));

  // Compressed encoding: a parity byte and x.
  encoded := sg_encode_public(g);
  show("compressed G", encoded.bytes[0..<encoded.length]);

  // lift_x: x = 1 gives 1 + 7 = 8, a square; the even root is returned.
  one := lift_hex("0000000000000000000000000000000000000000000000000000000000000001");
  print("lift_x(1): ok %\n", one.ok);
  show_point("lift_x(1)", one.value);

  // Refusals.
  print("lift_x(0): ok %\n", lift_hex("0000000000000000000000000000000000000000000000000000000000000000").ok);
  print("lift_x(7): ok %\n", lift_hex("0000000000000000000000000000000000000000000000000000000000000007").ok);
  p1 := lift_hex("fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc30");
  print("lift_x(p + 1): ok %, value is infinity %\n", p1.ok, same_point(p1.value, inf));
  prefix: [33] u8;
  for i: 0..<33 { prefix[i] = encoded.bytes[i]; }
  prefix[0] = 4;
  print("decode with prefix 04: ok %\n", sg_decode_public(prefix[..]).ok);
  off: SgPoint; off.x.limb[0] = 1; off.y.limb[0] = 1; off.z.limb[0] = 1;
  refused := sg_add(off, g);
  print("(1, 1) + G: ok %, value is infinity %\n", refused.ok, same_point(refused.value, inf));
}
```

Run it from the repository root:

```shell
coax/bin/coax run bitcoin/examples/group-law.coax
```

The output matches the values worked out by hand: `-G.y` ends in `...2777`,
the scaled `X` is `4 G.x mod p`, `lift_x(1)` succeeds, and `lift_x(0)`,
`lift_x(7)` and `lift_x(p + 1)` are refused:

```text
G:
  x: 79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798
  y: 483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8
G is on the curve: true
-G:
  x: 79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798
  y: b7c52588d95c3b9aa25b0403f1eef75702e84bb7597aabe663b82f6f04ef2777
G + (-G) is infinity: true
G + infinity is G: true
2G = G + G: true
2G:
  x: c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5
  y: 1ae168fea63dc339a3c58419466ceaeef7f632653266d0e1236431a950cfe52a
3G:
  x: f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9
  y: 388f7b0f632de8140fe337e62a37f3566500a99934c2231b6cb9fd7584b8e672
G with Z = 2, X: e6f999fbe772eeb156818a573a1c2c1c0a6ff36cb738a36567ca056d5be06231
equal to G: true
scaled G + G = 2G: true
compressed G: 0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798
lift_x(1): ok true
lift_x(1):
  x: 0000000000000000000000000000000000000000000000000000000000000001
  y: 4218f20ae6c646b363db68605822fb14264ca8d2587fdd6fbc750d587e76a7ee
lift_x(0): ok false
lift_x(7): ok false
lift_x(p + 1): ok false, value is infinity true
decode with prefix 04: ok false
(1, 1) + G: ok false, value is infinity true
```

The `y` of `lift_x(1)` ends in `e`, so it is even, as BIP-340 requires.
The compressed form of `G` starts with `02` because `G.y` ends in `8`.

The refusals:

- `lift_x(0)` and `lift_x(7)` are refused because `x^3 + 7` is not a
  square.
- `lift_x(p + 1)` is refused because x is not below `p`. The value is
  infinity, not the point for `x = 1`.
- The prefix `04` is refused: the uncompressed 65-byte form isn't
  accepted, and a 33-byte view with that prefix is neither form.
- `(1, 1)` is not on the curve (`1 ≠ 1 + 7`), so `sg_add` refuses it and
  returns infinity with `ok` false.

The book's checks run this program in debug and release builds and compare
the output with [bitcoin/fixtures/group-law.out](../fixtures/group-law.out).

## How the code is checked

### libsecp256k1 and an affine oracle

[coax/tests/nostr/secp_group.py](../../coax/tests/nostr/secp_group.py)
compiles the group to C in debug and release builds and compares it with
two independent implementations: Python's integers running the affine
formulas, and libsecp256k1, Bitcoin Core's secp256k1 library.

- 1,030 pairs of points, most of them random multiples of `G` made by
  libsecp256k1, plus infinity, equal, opposite and equal-y pairs. For each
  pair, libsecp256k1's `secp256k1_ec_pubkey_combine` gives the same sum as
  the affine formulas, and `sg_add`, `sg_mixed_add`, `sg_double` and
  `sg_negate` give that sum with random `Z` on both inputs.
- 128 random triples, where `(P + Q) + R` and `P + (Q + R)` both equal
  libsecp256k1's sum, a quarter of them with `Q + R = O`.
- Compressed encoding, decoding, lifting and affine conversion of 68
  points, compared with libsecp256k1's serialized keys.
- Invalid formats, flags and table indices, and the generated C and wasm
  code.

The libsecp256k1 tests need the native oracle in
`tools/coax-nostr/fuzz/.cache/`. Build it from the pinned `v0.8.0` archive
as chapter 13 describes, then run:

```shell
cd coax/tests/nostr
python3 secp_group.py
```

All 7 tests pass in about 8 seconds.

### Checks in Coax

- `secp_group.coax` checks `2G` and `3G` against fixed values, every
  exceptional case of addition and doubling, scaled coordinates, field
  aliases of `p`, the refusal of invalid points and flags, and the
  compressed and x-only round trips.
- The example's checks compare the Jacobian code with affine formulas
  written in the example, on a table of six points that Python computed
  (`G`, `2G`, `3G`, `4G`, `-G` and `-3G`). They check associativity,
  commutativity, inverses, the identity and doubling on all pairs, and
  that `lift_x` accepts an x from 0 to 11 exactly when Euler's criterion
  `(x^3 + 7)^((p-1)/2) = 1` holds. Euler's criterion is computed by
  square-and-multiply, not by the square-root chain:

<!-- source: bitcoin/examples/group-law.coax check -->
```jai
// Affine chord-and-tangent formulas, written from the curve equation with
// one field inverse each. They share no code with the Jacobian kernels.
affine_add :: (a: SgAffine, b: SgAffine) -> SgAffine {
  slope: SecpField;
  if sf_equal_mask(a.x, b.x) == CB_MAX64 {
    three: SecpField; three.limb[0] = 3;
    slope = sf_multiply(sf_multiply(three, sf_square(a.x)), sf_inverse(sf_add(a.y, a.y)).value);
  } else {
    slope = sf_multiply(sf_subtract(b.y, a.y), sf_inverse(sf_subtract(b.x, a.x)).value);
  }
  r: SgAffine;
  r.x = sf_subtract(sf_subtract(sf_square(slope), a.x), b.x);
  r.y = sf_subtract(sf_multiply(slope, sf_subtract(a.x, r.x)), a.y);
  return r;
}
// Six points as affine (x, y) hex: G, 2G, 3G, 4G, -G and -3G, computed with
// Python's arbitrary-precision integers and the affine formulas.
POINTS :: 6;
POINT_HEX :: [12] string.[
    "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798", "483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8",
    "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5", "1ae168fea63dc339a3c58419466ceaeef7f632653266d0e1236431a950cfe52a",
    "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9", "388f7b0f632de8140fe337e62a37f3566500a99934c2231b6cb9fd7584b8e672",
    "e493dbf1c10d80f3581e4904930b1404cc6c13900ee0758474fa94abe8c4cd13", "51ed993ea0d455b75642e2098ea51448d967ae33bfbdfe40cfe97bdc47739922",
    "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798", "b7c52588d95c3b9aa25b0403f1eef75702e84bb7597aabe663b82f6f04ef2777",
    "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9", "c77084f09cd217ebf01cc819d5c80ca99aff5666cb3ddce4934602897b4715bd"];
affine_at :: (i: int) -> SgAffine {
  b: [32] u8; r: SgAffine;
  #assert bytes_of(POINT_HEX[2 * i], b[..]); r.x = sf_load(b[..]).value;
  #assert bytes_of(POINT_HEX[2 * i + 1], b[..]); r.y = sf_load(b[..]).value;
  return r;
}
jacobian :: (i: int) -> SgPoint { return sg_from_affine(affine_at(i)).value; }
// The table agrees with the group code: 2G, 3G and 4G by doubling and
// addition, and the negatives by negation.
#test "the oracle's points match the group code" #steps 100000000 {
  for i: 0..<POINTS { #assert sg_from_affine(affine_at(i)).ok; }
  g := jacobian(0);
  #assert same_point(sg_double(g).value, jacobian(1));
  #assert same_point(sg_add(jacobian(1), g).value, jacobian(2));
  #assert same_point(sg_double(jacobian(1)).value, jacobian(3));
  #assert same_point(sg_negate(g).value, jacobian(4));
  #assert same_point(sg_negate(jacobian(2)).value, jacobian(5));
}
// Jacobian addition and doubling give the affine formulas' points.
#forall "Jacobian sums match the affine formulas" a: 0..<POINTS, b: 0..<POINTS #steps 400000000 {
  got := sg_add(jacobian(a), jacobian(b));
  #assert got.ok;
  pa := affine_at(a); pb := affine_at(b);
  if sf_equal_mask(pa.x, pb.x) == CB_MAX64 && sf_equal_mask(pa.y, pb.y) != CB_MAX64 {
    #assert same_point(got.value, sg_infinity());
  } else {
    want := sg_from_affine(affine_add(pa, pb));
    #assert want.ok && same_point(got.value, want.value);
  }
}
// Associativity, commutativity, inverses, identity and doubling.
#forall "the group laws hold" a: 0..<POINTS, b: 0..<POINTS #steps 400000000 {
  inf := sg_infinity();
  p := jacobian(a); q := jacobian(b); r := jacobian((a + 2 * b + 1) % POINTS);
  #assert same_point(sg_add(sg_add(p, q).value, r).value, sg_add(p, sg_add(q, r).value).value);
  #assert same_point(sg_add(p, q).value, sg_add(q, p).value);
  #assert same_point(sg_add(p, sg_negate(p).value).value, inf);
  #assert same_point(sg_add(p, inf).value, p);
  #assert same_point(sg_double(p).value, sg_add(p, p).value);
}
// lift_x accepts exactly the x whose x^3 + 7 passes Euler's criterion,
// (x^3 + 7)^((p-1)/2) = 1, computed by square-and-multiply over the bits
// of (p - 1) / 2 instead of the square-root chain.
HALF :: "7fffffffffffffffffffffffffffffffffffffffffffffffffffffff7ffffe17";
euler :: (a: SecpField) -> SecpField {
  e: [32] u8;
  #assert bytes_of(HALF, e[..]);
  // Right to left: the last byte first, its low bit first.
  r: SecpField; r.limb[0] = 1;
  power := a;
  for j: 0..<32 {
    for bit: 0..<8 {
      if ((e[31 - j] >> bit) & 1) != 0 { r = sf_multiply(r, power); }
      power = sf_square(power);
    }
  }
  return r;
}
#forall "lift_x accepts exactly the squares" x: 0..<12 #steps 400000000 {
  b: [32] u8; b[31] = cast(u8) x;
  v: SecpField; v.limb[0] = cast(u64) (x * x * x + 7);
  one: SecpField; one.limb[0] = 1;
  lifted := sg_lift_x_public(b[..]);
  #assert lifted.ok == (sf_equal_mask(euler(v), one) == CB_MAX64);
  if lifted.ok {
    #assert (lifted.value.y.limb[0] & 1) == 0;
    #assert sf_equal_mask(sf_square(lifted.value.y), v) == CB_MAX64;
  }
}
```

To run all the checks:

```shell
coax/bin/coax test bitcoin/examples/group-law.coax
coax/bin/coax mutate bitcoin/examples/group-law.coax
```

`coax mutate` on the example leaves zero survivors. The full run made 987
mutants in the example and the files it loads, `secp_group.coax` among
them, and killed 986. The survivor was in the first version of `euler`,
which read the exponent from the top bit, a 0, so skipping that bit changed
nothing. `euler` now reads the exponent from the low bit up, and a
`--root-only` run kills all 38 mutants in the example.
`coax test` on the example runs 57 checks in about 6 seconds.

## Summary

- The points of `y^2 = x^3 + 7` over `F_p`, with `O`, form a group; `-P`
  is `(x, p - y)`.
- Addition follows the chord-and-tangent rule; equal points, opposite
  points and `O` are special cases.
- Jacobian coordinates `(X/Z^2, Y/Z^3)` add with no inversion.
- `sg_add` computes both the sum and the doubling and selects with masks;
  infinity is always `(1, 1, 0)`.
- `lift_x` refuses an x of `p` or more and an x whose `x^3 + 7` isn't a
  square, and returns the even `y`.

## Dependencies

- [13. Field arithmetic mod p](ch013-field-arithmetic-mod-p.md)

## Sources

- [SEC 2: Recommended Elliptic Curve Domain Parameters](https://www.secg.org/sec2-v2.pdf),
  section 2.4.1, for the curve and `G`.
- [BIP-340](https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki),
  which defines `lift_x` and x-only public keys.
- The [Explicit-Formulas Database](https://www.hyperelliptic.org/EFD/g1p/auto-shortw-jacobian-0.html),
  for `add-2007-bl` and `dbl-2009-l`.
- [libsecp256k1](https://github.com/bitcoin-core/secp256k1), release
  `v0.8.0`, pinned in
  [UPSTREAM.json](../../tools/coax-nostr/fuzz/UPSTREAM.json).
- Record:
  [coax-secp-group-2026-10-28.md](../../docs/verification/coax-secp-group-2026-10-28.md).
