# 7. Field arithmetic mod p

Every point on secp256k1 has two coordinates, and each coordinate is an
integer mod the prime:

```text
p = 2^256 - 2^32 - 977
  = fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2f
```

Adding, subtracting, multiplying and dividing points (chapter 9) all come
down to arithmetic on these coordinates. This chapter builds that
arithmetic on the ten 26-bit limbs of chapter 2: addition, negation,
multiplication with a reduction that uses the special form of `p`,
inversion by Fermat's little theorem and the square root that turns an
x-only public key into a point.

## What this chapter teaches

- Reduction using `2^256 ≡ 2^32 + 977 (mod p)`.
- Normalization to the one canonical value below `p`.
- Negation as `2p - x`, which needs no borrow.
- Inversion as `a^(p-2)` with a fixed addition chain, so the same
  squarings and multiplications run for every input.
- Square root as `a^((p+1)/4)`, valid because `p ≡ 3 (mod 4)`, and the
  check that the result squares back.
- Edge cases: 0, `p - 1`, `p` itself, and inputs with the largest admitted
  magnitude.

## Where the code is

[coax/lib/crypto/secp_field.coax](../../coax/lib/crypto/secp_field.coax)
has the field. A `SecpField` is ten `u64` limbs, and a `SecpFieldResult`
pairs a value with an `ok` flag for the operations that can fail.

| Procedure | Result |
| --- | --- |
| `sf_load`, `sf_load_strict` | 32 big-endian bytes to a field element; the strict form refuses values `>= p` |
| `sf_store` | a field element to 32 canonical big-endian bytes |
| `sf_normalize` | the canonical value below `p`, every limb within its mask |
| `sf_add`, `sf_negate`, `sf_subtract` | `a + b`, `-a`, `a - b` |
| `sf_multiply`, `sf_square` | `a * b`, `a * a` |
| `sf_inverse` | `1 / a`; refuses 0 |
| `sf_sqrt` | the even square root of `a`; refuses a non-square |
| `sf_equal_mask` | all ones when `a = b`, else 0 |

Every procedure accepts limbs up to 8 times their mask (magnitude 8,
chapter 2) and returns a canonical value of magnitude 1.
[secp_field32.coax](../../coax/lib/crypto/secp_field32.coax) and
[secp_field32_total.coax](../../coax/lib/crypto/secp_field32_total.coax)
hold a 32-bit-limb form of the same field for targets without a fast
64-bit multiply.

## Reduction

`p` is 2^256 minus a small number, `c = 2^32 + 977`. So:

```text
2^256 = p + c ≡ c (mod p)
```

Any bits at 2^256 or above can be folded back: a number `H·2^256 + L` is
congruent to `H·c + L`. Folding is a multiply by a 33-bit constant and an
addition, not a division.

In limbs, `c` is easy to add. `2^32 = 64 · 2^26`, so `c` is 977 in limb 0
and 64 in limb 1. Every fold in the code adds `q * 977` to limb 0 and
`q * 64` to limb 1.

### Worked example: 2^128 times 2^128

1. `2^128` is bit 128, in limb `128 / 26 = 4` at position
   `128 % 26 = 24`.
2. The product is `2^256`. Its low 256 bits are all zero, and the part at
   2^256 and above is `q = 1`.
3. Fold: limb 0 gets `1 * 977 = 977`, and limb 1 gets `1 * 64 = 64`.
4. The value is `977 + 64 · 2^26 = 977 + 2^32 = 0x1000003d1`.

That is `c`, as `2^256 ≡ c` says it must be.

### Multiplication

`sf_multiply` multiplies in 20 columns (chapter 2), carries them, then
splits the 512-bit product at bit 256. It folds the high ten limbs by `c`,
which can leave a few bits above 2^256 again. A second, smaller fold of
those bits and a carry pass follow, and `sf_normalize` makes the result
canonical.
`sf_square` is `sf_multiply(a, a)`.

## Normalization

Many limb patterns stand for the same field element: limbs over their mask
and values from `p` to `2^256 - 1` are all allowed as inputs. A canonical
value is the one below `p` with every limb within its mask. `sf_normalize`
makes it in two steps:

1. **Carry and fold.** Three carry passes move bits up the limbs. Each
   carry out of limb 9 is folded back as `carry * c`. After the third pass
   the value is below 2^256. The code asserts that the last carry is 0.
2. **Subtract `p` at most once, without a branch.** A value `r` below 2^256
   is at least `p` exactly when `r + c` reaches 2^256. So the code adds `c`
   to a copy and carries it. The carry out of the top limb is 1 when
   `r >= p`, and then the low 256 bits of the copy are `r - p`.
   `cb_ct_select64` picks the copy or the original with that carry as a
   mask, the same select for every value (chapter 3).

For `r = p`: `p + c = 2^256`, the carry is 1, and the low 256 bits are 0.

## Addition, negation and subtraction

`sf_add` normalizes both inputs, adds the limbs and normalizes the sum.

`sf_negate` computes `2p - x` limb by limb. Every limb of `2p` is at least
the matching limb of any canonical `x`, so no limb goes below zero and no
borrow is needed. Normalizing `2p - x` gives `p - x` for `x ≠ 0`, and 0 for
`x = 0`.

`sf_subtract(a, b)` is `sf_add(a, sf_negate(b))`. So `0 - 1` is `p - 1`.

## Inversion

Fermat's little theorem says that `a^(p-1) = 1` for any `a ≠ 0` mod a
prime `p`. Multiply both sides by `a^-1`:

```text
a^-1 = a^(p-2)
```

`p - 2` is a fixed public number, so you can compute the power with a fixed
sequence of squarings and multiplications, an *addition chain*. The code
never looks at the bits of `a` to decide what to do next, so the work is
the same for every input.

### The chain

`SF_FIELD_CHAIN` is twelve steps. Step `i` makes
`value[i + 1] = value[left]^(2^squares) * value[right]`, starting from
`value[0] = a`. The first steps build `a^3`, `a^(2^3 - 1)` and so on, with
runs of 1 bits doubling in length (11, 22, 44, 88), because `p - 2` is
mostly 1 bits. The last value is shared by the inverse and the square root.
Then:

- `sf_inverse` adds 13 more squarings and 3 more multiplications;
- `sf_sqrt` adds 8 more squarings and 1 more multiplication.

The code computes the exponent each chain produces at compile time, with
`sf_exponent_step` doing the same steps on 256-bit integers instead of
field elements. `#assert sf_exponent_matches(...)` stops the build unless
the exponents are exactly `p - 2` and `(p + 1) / 4`.

### Worked example: the inverse of 2

1. `p` is odd, so `p + 1` is even and `(p + 1) / 2` is an integer.
2. `2 · (p + 1) / 2 = p + 1`, and `p + 1 ≡ 1 (mod p)`. So `1/2` is
   `(p + 1) / 2`.
3. In hex, `p + 1` ends in `...fffffffefffffc30`. Shifting the whole
   number right by one bit turns the top byte `ff` into `7f`, and every
   later word takes a 1 bit from the word above it. The last 64 bits become
   `0x8000000000000000 | (0xfffffffefffffc30 >> 1)`, which is
   `0xffffffff7ffffe18`.

So `1/2 = 7fffffffffffffffffffffffffffffffffffffffffffffffffffffff7ffffe18`.

### Zero

0 has no inverse. `sf_inverse` runs the full chain on it anyway, which
gives `0^(p-2) = 0`, and sets `ok` to false because the result is 0. The
refusal costs the same as a success. `p` itself normalizes to 0 and is
refused the same way.

## Square roots

A field element `a` is a *square* when some `y` has `y^2 = a`. Half of the
nonzero elements are squares. Because `p ≡ 3 (mod 4)`, `(p + 1) / 4` is an
integer, and for a square `a`:

```text
(a^((p+1)/4))^2 = a^((p+1)/2) = a · a^((p-1)/2) = a · 1 = a
```

The last step is Euler's criterion: `a^((p-1)/2)` is 1 for a square. For a
non-square it is `-1`, and the candidate squares to `-a` instead.

`sf_sqrt` computes the candidate, squares it and compares with `a`. If they
differ, `ok` is false and the value is 0. If they match, it returns the
even one of the two roots `y` and `p - y`. `p` is odd, so exactly one of
them is even. BIP-340 uses the even root for every x-only key (chapter 11).

### Worked example: sqrt(9)

1. The roots of 9 are 3 and `p - 3`.
2. 3 is odd. `p - 3` ends in hex `...c2c`, and `0xc` is even.
3. So `sf_sqrt(9)` returns `p - 3`, not 3.

## Loading bytes

`sf_load` takes exactly 32 big-endian bytes and reduces them, so
`ff...ff` loads as `2^256 - 1 - p = 2^32 + 976`. A protocol that requires
a coordinate below `p`, as BIP-340 does for public keys and the `r` value
of a signature, uses `sf_load_strict`. It stores the loaded value back to
bytes and compares with the input; any value `>= p` changes, so it is
refused and the value is cleared to 0. A view that isn't 32 bytes is
refused by both.

## The example

The example is
[nostr/examples/field-arithmetic.coax](../examples/field-arithmetic.coax).
It prints `p`'s limbs, reduces `2^256 - 1` and `2^128 · 2^128`, wraps
addition at `p`, inverts 2, prints the exponents the chains compute, takes
square roots, lifts the generator's x-coordinate to its y-coordinate and
shows four refusals.

<!-- source: nostr/examples/field-arithmetic.coax example -->
```jai
#load "../../coax/lib/crypto/secp_field.coax";
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
// Print a 256-bit exponent, stored as eight little-endian u32 words.
show_exponent :: (label: string, e: SfExponent) {
  b: [32] u8;
  for i: 0..<8 { if !cb_store_be32(b[..], (7 - i) * 4, e.word[i]) { print("bad store\n"); } }
  show(label, b[..]);
}
// A field element from 64 hex digits; not ok if the hex is bad.
parse_field :: (text: string) -> SecpFieldResult {
  b: [32] u8;
  r: SecpFieldResult;
  if hex_decode_checked(text, b[..], false).error == 0 { r = sf_load(b[..]); }
  return r;
}
// A field element from 64 hex digits that the program itself supplies.
field :: (text: string) -> SecpField {
  r := parse_field(text);
  if !r.ok { print("bad hex\n"); }
  return r.value;
}
// A small field element.
small :: (n: u64) -> SecpField { r: SecpField; r.limb[0] = n; return r; }
equal :: (a: SecpField, b: SecpField) -> bool { return sf_equal_mask(a, b) == CB_MAX64; }

main :: () {
  // p in ten limbs: nine of 26 bits and one of 22 bits, lowest first.
  print("p limbs: %\n", SF_P);

  // Loading 2^256 - 1 reduces it: 2^256 - 1 - p = 2^32 + 976.
  all_ones := field("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff");
  show_field("2^256 - 1 mod p", all_ones);

  // 2^128 * 2^128 = 2^256, and 2^256 = 2^32 + 977 mod p.
  half := field("0000000000000000000000000000000100000000000000000000000000000000");
  show_field("2^128 * 2^128", sf_multiply(half, half));

  // Addition wraps at p: (p - 1) + 2 = 1.
  minus_one := sf_negate(small(1));
  show_field("-1", minus_one);
  show_field("(p - 1) + 2", sf_add(minus_one, small(2)));
  show_field("0 - 1", sf_subtract(small(0), small(1)));

  // Inverse: a^(p-2). The inverse of 2 is (p + 1) / 2.
  inv := sf_inverse(small(2));
  print("inverse of 2: ok %\n", inv.ok);
  show_field("1/2", inv.value);
  show_field("2 * (1/2)", sf_multiply(small(2), inv.value));

  // The fixed chains compute exactly p - 2 and (p + 1) / 4, checked when
  // the file compiles.
  show_exponent("inverse exponent", SF_INVERSE_EXPONENT);
  show_exponent("square-root exponent", SF_SQRT_EXPONENT);
  squares := 0;
  for i: 0..<12 { squares += SF_FIELD_CHAIN[i].squares; }
  print("shared chain: 12 steps, % squarings\n", squares);
  print("inverse: % squarings, 15 multiplications; root: % squarings, 13 multiplications\n",
    squares + 1 + 2 + 10, squares + 6 + 2);

  // Square roots: a^((p+1)/4), checked by squaring, with the even root
  // returned.
  r4 := sf_sqrt(small(4)); r9 := sf_sqrt(small(9));
  show_field("sqrt(4)", r4.value);
  show_field("sqrt(9)", r9.value);
  print("sqrt(9) is p - 3: %\n", equal(r9.value, sf_negate(small(3))));

  // Lift the generator's x to a point: y^2 = x^3 + 7.
  gx := field("79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798");
  y2 := sf_add(sf_multiply(sf_square(gx), gx), small(7));
  gy := sf_sqrt(y2);
  print("x^3 + 7 is a square: %\n", gy.ok);
  show_field("y", gy.value);
  print("matches G.y: %\n", equal(gy.value, field("483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8")));

  // Refusals.
  r3 := sf_sqrt(small(3));
  print("sqrt(3): ok %, value is zero %\n", r3.ok, equal(r3.value, small(0)));
  r0 := sf_inverse(small(0));
  print("inverse of 0: ok %\n", r0.ok);
  pb: [32] u8;
  if hex_decode_checked("fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2f", pb[..], false).error != 0 { print("bad hex\n"); }
  loaded := sf_load_strict(pb[..]);
  print("strict load of p: ok %, value is zero %\n", loaded.ok, equal(loaded.value, small(0)));
  print("31-byte load: ok %\n", sf_load(pb[0..<31]).ok);
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/field-arithmetic.coax
```

The output matches the values worked out by hand: `2^256 - 1` reduces to
`0x1000003d0`, `2^128 · 2^128` to `c = 0x1000003d1`, `1/2` to
`(p + 1) / 2` and `sqrt(9)` to the even root `p - 3`:

```text
p limbs: [67107887, 67108799, 67108863, 67108863, 67108863, 67108863, 67108863, 67108863, 67108863, 4194303]
2^256 - 1 mod p: 00000000000000000000000000000000000000000000000000000001000003d0
2^128 * 2^128: 00000000000000000000000000000000000000000000000000000001000003d1
-1: fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2e
(p - 1) + 2: 0000000000000000000000000000000000000000000000000000000000000001
0 - 1: fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2e
inverse of 2: ok true
1/2: 7fffffffffffffffffffffffffffffffffffffffffffffffffffffff7ffffe18
2 * (1/2): 0000000000000000000000000000000000000000000000000000000000000001
inverse exponent: fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2d
square-root exponent: 3fffffffffffffffffffffffffffffffffffffffffffffffffffffffbfffff0c
shared chain: 12 steps, 245 squarings
inverse: 258 squarings, 15 multiplications; root: 253 squarings, 13 multiplications
sqrt(4): 0000000000000000000000000000000000000000000000000000000000000002
sqrt(9): fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2c
sqrt(9) is p - 3: true
x^3 + 7 is a square: true
y: 483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8
matches G.y: true
sqrt(3): ok false, value is zero true
inverse of 0: ok false
strict load of p: ok false, value is zero true
31-byte load: ok false
```

The chains' exponents are `p - 2` and `(p + 1) / 4`. The shared chain
does 245 squarings; the inverse does 258 in all and the root 253, plus one
squaring to check the root.

The generator's y-coordinate is the value published in SEC 2 and BIP-340,
`483ada77...fb10d4b8`. It ends in `8`, so it is the even root, as `sf_sqrt`
chooses.

The four refusals:

- `sqrt(3)` is refused, because 3 is not a square mod `p`, and its value is
  0.
- The inverse of 0 is refused after the full chain.
- `sf_load_strict` refuses the bytes of `p`, which `sf_load` would accept
  as 0, and clears the value.
- A 31-byte view is refused.

The book's checks run this program in debug and release builds and compare
the output with
[nostr/fixtures/field-arithmetic.out](../fixtures/field-arithmetic.out).

## How the code is checked

### Independent big-integer oracles

Three Python harnesses compile the field to C and to wasm32 and compare
every operation with Python's arbitrary-precision integers:

- [coax/tests/nostr/secp_field.py](../../coax/tests/nostr/secp_field.py)
  checks loading, storing, normalization, addition and negation on edge
  values and raw limbs up to magnitude 8, and that a planted wrong
  reduction is detected.
- [coax/tests/nostr/secp_field_mul.py](../../coax/tests/nostr/secp_field_mul.py)
  checks a seeded stream of products and squares, and carry edges.
- [coax/tests/nostr/secp_field_inv.py](../../coax/tests/nostr/secp_field_inv.py)
  checks a seeded stream of inverses and square roots, zero, non-squares,
  aliases of `p` and magnitude edges.

```shell
cd coax/tests/nostr
python3 secp_field.py
python3 secp_field_mul.py
python3 secp_field_inv.py
```

`secp_field_inv.py` also compares roots with public points from a pinned
libsecp256k1 build. That one test needs the native oracle in
`tools/coax-nostr/fuzz/.cache/`, built separately; the other 17 tests run
without it.

### Checks in Coax

- `secp_field.coax` checks the multiplication bound certificate at every
  magnitude from 1 to 8, that both chains compute their exact exponents and
  that changing any byte of an exponent breaks its certificate, the zero
  and `p` refusals of the inverse, and the parity of square roots.
- The example checks the field laws on twelve values near 0, near `p`,
  across limb boundaries and at the generator:

<!-- source: nostr/examples/field-arithmetic.coax check -->
```jai
// Field laws on values near 0, near p and across limb boundaries: the
// inverse multiplies back to 1, a square's root squares back and is even,
// negation adds to 0, and subtraction undoes addition.
#forall "field laws hold at the edges" which: 0..<12 #steps 400000000 {
  hexes := [12] string.[
    "0000000000000000000000000000000000000000000000000000000000000001",
    "0000000000000000000000000000000000000000000000000000000000000002",
    "0000000000000000000000000000000000000000000000000000000003ffffff",
    "0000000000000000000000000000000000000000000000000000000004000000",
    "00000000000000000000000000000000000000000000000000000001000003d1",
    "8000000000000000000000000000000000000000000000000000000000000000",
    "7fffffffffffffffffffffffffffffffffffffffffffffffffffffff7ffffe18",
    "fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2e",
    "fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2d",
    "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
    "483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8",
    "0123456789abcdeffedcba98765432100123456789abcdeffedcba9876543210"];
  pa := parse_field(hexes[which]);
  pb := parse_field(hexes[(which + 5) % 12]);
  #assert pa.ok && pb.ok;
  a := pa.value; b := pb.value;
  inv := sf_inverse(a);
  #assert inv.ok && equal(sf_multiply(a, inv.value), small(1));
  root := sf_sqrt(sf_square(a));
  #assert root.ok && equal(sf_square(root.value), sf_square(a));
  #assert (root.value.limb[0] & 1) == 0;
  #assert equal(root.value, a) || equal(root.value, sf_negate(a));
  #assert equal(sf_add(a, sf_negate(a)), small(0));
  #assert equal(sf_subtract(sf_add(a, b), b), a);
  #assert equal(sf_multiply(a, b), sf_multiply(b, a));
}
```

The check sets its own step budget with `#steps`, because each case runs
two full exponentiation chains in the compile-time interpreter.

To run all the checks:

```shell
coax/bin/coax test nostr/examples/field-arithmetic.coax
coax/bin/coax mutate coax/lib/crypto/secp_field.coax
```

`coax mutate` leaves zero survivors: 686 mutants in `secp_field.coax` with the
files it loads, and 786 in the example with the files it loads. `coax test`
on the example runs 43 checks.

## Summary

- `2^256 ≡ 2^32 + 977 (mod p)`, so high bits fold back with a multiply by
  977 into limb 0 and by 64 into limb 1.
- Normalization subtracts `p` at most once, chosen by a mask, not a branch.
- Negation is `2p - x`, which never borrows.
- The inverse is `a^(p-2)` and the square root is `a^((p+1)/4)`, both by a
  fixed chain whose exponent is checked when the file compiles.
- A square root is checked by squaring it back, and the even root is
  returned.

## Dependencies

- [2. Big integers in fixed limbs](ch002-big-integers-in-fixed-limbs.md)
- [3. Constant-time rules and secrets](ch003-constant-time-rules.md)

## Sources

- [SEC 2: Recommended Elliptic Curve Domain Parameters](https://www.secg.org/sec2-v2.pdf),
  section 2.4.1, for `p` and the generator.
- [BIP-340](https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki),
  which lifts x-only keys to points with the even square root.
- Records:
  [coax-secp-field-inv-2026-10-28.md](../../docs/verification/coax-secp-field-inv-2026-10-28.md)
  and
  [2026-10-29-secp-field-guard-proof.md](../../docs/verification/2026-10-29-secp-field-guard-proof.md).
