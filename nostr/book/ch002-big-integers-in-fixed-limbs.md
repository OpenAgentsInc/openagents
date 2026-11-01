# 2. Big integers in fixed limbs

secp256k1 works on 256-bit numbers, and Coax has no 256-bit integer type.
This chapter shows how a 256-bit number becomes a fixed array of smaller
words called *limbs*, how addition and multiplication keep every
intermediate value inside `u64`, and how the code proves those bounds before
it runs.

Two representations are in use:

- Field elements mod p, in ten limbs of 26 bits or less, in
  [coax/lib/crypto/secp_field.coax](../../coax/lib/crypto/secp_field.coax).
  Chapter 7 builds the field arithmetic on them.
- Scalars mod the group order n, in eight 32-bit limbs, in
  [coax/lib/crypto/secp_scalar.coax](../../coax/lib/crypto/secp_scalar.coax).
  Chapter 8 builds the scalar arithmetic on them.

## What this chapter teaches

- How a 256-bit number splits into ten little-endian limbs: nine of 26 bits
  and one of 22 bits.
- Why a limb has fewer bits than its `u64`, and what *magnitude* means.
- How a carry pass moves the excess bits of each limb into the next one.
- How a product of two numbers fits in twenty `u64` columns, and the bound
  certificate that proves it.
- Why scalars use eight 32-bit limbs instead, with one widening
  multiply-accumulate per step.
- How independent values check the code: hand-derived values in this
  chapter, and a Python big-integer oracle in the test harness.

## Ten limbs of 26 bits

A `SecpField` is a struct of ten `u64` limbs, lowest first:

```jai
SecpField :: struct { limb: [10] u64; }
```

The value of a field element is:

```text
limb[0] + limb[1]·2^26 + limb[2]·2^52 + … + limb[9]·2^234
```

Limbs 0 to 8 hold 26 bits each, and limb 9 holds 22 bits. That gives
9 × 26 + 22 = 256 bits. Bit `b` of the number is in limb `b / 26`, at
position `b % 26` within that limb.

A limb uses 26 bits of a 64-bit word so the code can add and multiply limbs
without checking for overflow:

- The product of two 26-bit limbs needs 52 bits.
- A multiplication adds up to ten such products in one column, which needs
  at most 56 bits.
- The remaining 8 bits hold the carries from lower columns.

### Magnitude

After an addition, a limb can be larger than its mask. Adding two canonical
values gives limbs of up to 2 × (2^26 − 1). The *magnitude* of a value is the
largest multiple of the mask that any limb reaches. Every function in
`secp_field.coax` accepts magnitude up to `SF_MAGNITUDE`, which is 8, and
returns a canonical value of magnitude 1. A canonical value is less than p and
has every limb within its mask.

### Carry passes

A carry pass moves each limb's bits above the mask into the next limb. This
is `sf_carry`, from `secp_field.coax`:

```jai
sf_carry :: (digits: [] u64) -> u64 {
  #assert digits.count == 10;
  for i: 0..<9 {
    carry := digits[i] >> 26;
    digits[i] &= SF_MASK26;
    digits[i + 1] += carry;
  }
  carry := digits[9] >> 22;
  digits[9] &= SF_MASK22;
  return carry;
}
```

The carry out of limb 9 is the part of the number at bit 256 and above. The
field reduction in chapter 7 folds it back in, using
2^256 ≡ 2^32 + 977 (mod p).

### Worked example: 2^256 − 1

Set every limb to its mask. The value is 2^256 − 1, the largest 256-bit
number. It's larger than p = 2^256 − 2^32 − 977, so it isn't canonical.

1. Subtract p: (2^256 − 1) − (2^256 − 2^32 − 977) = 2^32 + 976.
2. 2^32 + 976 is less than p, so 2^256 − 1 mod p = 2^32 + 976.
3. Split it into limbs. 2^32 = 64 × 2^26, so limb 1 is 64. The remainder,
   976, is less than 2^26, so limb 0 is 976. Every other limb is 0.

With every limb at 8 times its mask, the value is 8 × (2^256 − 1), which
reduces to 8 × (2^32 + 976). Limb 1 is 8 × 64 = 512 and limb 0 is
8 × 976 = 7,808.

The largest canonical value is p − 1 = (2^256 − 1) − (2^32 + 977). Start
from every limb at its mask and subtract 977 from limb 0 and 64 from limb 1:

- Limb 0 is 67,108,863 − 977 = 67,107,886.
- Limb 1 is 67,108,863 − 64 = 67,108,799.
- Limbs 2 to 9 stay at their masks.

As bytes, p − 1 is p's big-endian hex with the last digit one lower:

```text
p     = fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2f
p - 1 = fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2e
```

## Products in twenty columns

`sf_multiply` first normalizes both inputs to canonical limbs. It then
multiplies with the schoolbook method: limb `i` of one input times limb `j`
of the other adds to column `i + j`. Ten limbs times ten limbs gives columns 0
to 18, plus one column for the final carry, so the product has twenty `u64`
columns.

The middle column, column 9, receives ten products. Each product is at most
(2^26 − 1)^2, so the column is at most:

```text
10 × (2^26 − 1)^2 = 10 × 4,503,599,493,152,769 = 45,035,994,931,527,690
```

That's less than 2^56 = 72,057,594,037,927,936, and far below the `u64`
maximum of 18,446,744,073,709,551,615.

### The bound certificate

The code doesn't rely on this arithmetic in a comment. `sf_mul_bounds`
computes every intermediate bound of the multiplication schedule at compile
time, with additions and products that refuse to overflow `u64`. It records
27 facts, and `sf_mul_budget` compares each one with a declared limit in
`SF_MUL_LIMITS`. The file refuses to compile unless the certificate for
magnitude 8 holds:

```jai
SF_MUL_BOUNDS :: #run sf_mul_bounds(8);
#assert SF_MUL_BOUNDS.ok;
```

`sf_mul_bounds` refuses any magnitude outside 1 to 8. A request for
magnitude 9 returns `ok = false` with every fact 0, so no caller can build a
schedule the code hasn't proved.

## Scalars in eight 32-bit limbs

Scalars use a different representation:

```jai
ScScalar :: struct {limbs:[8]u32;}
```

Scalar arithmetic reduces mod n with Montgomery multiplication (chapter 8).
Montgomery reduction works one machine word at a time, so it needs full words
with no spare bits. Eight 32-bit limbs give exactly 256 bits.

Each step of the multiplication is one multiply-accumulate in `u64`: a
32-bit by 32-bit product, plus one existing limb, plus one carry. The largest
possible result is exactly the `u64` maximum:

```text
(2^32 − 1)^2 + (2^32 − 1) + (2^32 − 1)
  = 2^64 − 2^33 + 1 + 2^33 − 2
  = 2^64 − 1
```

So one multiply-accumulate never overflows. There is no room for a second
product, so the code splits the accumulator into a 32-bit limb and a carry
after every step. `SC_MAC_MAX` records this bound, and
a check in `secp_scalar.coax` confirms it equals 18,446,744,073,709,551,615.

## The example

The example is
[nostr/examples/big-integers-in-limbs.coax](../examples/big-integers-in-limbs.coax).
It prints the limbs of the values worked out above, the bound certificate,
the scalar bound, and four refusals.

<!-- source: nostr/examples/big-integers-in-limbs.coax example -->
```jai
#load "../../coax/lib/crypto/secp_field.coax";
#load "../../coax/lib/crypto/secp_scalar.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Print the ten limbs of a field element, lowest first.
show_limbs :: (label: string, a: SecpField) {
  print("%:", label);
  for i: 0..<10 { print(" %", a.limb[i]); }
  print("\n");
}

// Print 32 bytes as lowercase hex.
show_bytes :: (label: string, bytes: [] u8) {
  text: [64] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}

// Every limb at its mask: 2^256 - 1, before any reduction.
all_ones :: (magnitude: u64) -> SecpField {
  a: SecpField;
  for i: 0..<10 { a.limb[i] = magnitude * (ifx i == 9 then SF_MASK22 else SF_MASK26); }
  return a;
}

main :: () {
  one: SecpField; one.limb[0] = 1;
  bytes: [32] u8;

  // A bit's limb is bit / 26 and its place in the limb is bit % 26.
  bit26: SecpField; bit26.limb[1] = 1;
  #assert sf_store(bit26, bytes[..]);
  show_bytes("2^26 as bytes", bytes[..]);

  // 2^256 - 1 is 2^32 + 976 more than p, so it reduces to limbs 976 and 64.
  show_limbs("2^256 - 1 raw", all_ones(1));
  show_limbs("2^256 - 1 mod p", sf_normalize(all_ones(1)));

  // p - 1 is the largest canonical value.
  minus_one := sf_negate(one);
  show_limbs("p - 1", minus_one);
  #assert sf_store(minus_one, bytes[..]);
  show_bytes("p - 1 as bytes", bytes[..]);

  // (p - 1) * (p - 1) = (-1) * (-1) = 1.
  show_limbs("(p - 1)^2", sf_square(minus_one));

  // Magnitude 8: every limb holds 8 times its mask. Normalize still works.
  show_limbs("magnitude 8 mod p", sf_normalize(all_ones(8)));

  // The bound certificate for magnitude 8.
  b := sf_mul_bounds(8);
  print("largest product column: %\n", b.facts[5]);
  print("u64 maximum:            %\n", CB_MAX64);
  print("certificate for magnitude 8 holds: %\n", b.ok);

  // Scalars: eight 32-bit limbs, one widening multiply-accumulate each.
  print("one multiply-accumulate at most: %\n", SC_MAC_MAX);
  n_minus_one := sc_sub(ScScalar.{}, sc_small(1));
  print("n - 1 limbs:");
  for i: 0..<8 { print(" %", n_minus_one.limbs[i]); }
  print("\n");
  print("(n - 1) + 1 is zero: %\n", sc_is_zero(sc_add(n_minus_one, sc_small(1))));

  // Refusals.
  p_bytes := [32] u8.[255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
    255, 254, 255, 255, 252, 47];
  print("strict load of p: ok %\n", sf_load_strict(p_bytes[..]).ok);
  print("load of 31 bytes: ok %\n", sf_load(p_bytes[0..<31]).ok);
  n_bytes := [32] u8.[255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
    255, 255, 255, 255, 254, 186, 174, 220, 230, 175, 72, 160, 59, 191, 210,
    94, 140, 208, 54, 65, 65];
  n := sc_load(n_bytes[..]);
  print("scalar load of n: ok %, overflow %\n", n.ok, n.overflow);
  print("certificate for magnitude 9: ok %\n", sf_mul_bounds(9).ok);
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/big-integers-in-limbs.coax
```

```text
2^26 as bytes: 0000000000000000000000000000000000000000000000000000000004000000
2^256 - 1 raw: 67108863 67108863 67108863 67108863 67108863 67108863 67108863 67108863 67108863 4194303
2^256 - 1 mod p: 976 64 0 0 0 0 0 0 0 0
p - 1: 67107886 67108799 67108863 67108863 67108863 67108863 67108863 67108863 67108863 4194303
p - 1 as bytes: fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2e
(p - 1)^2: 1 0 0 0 0 0 0 0 0 0
magnitude 8 mod p: 7808 512 0 0 0 0 0 0 0 0
largest product column: 45035994931527690
u64 maximum:            18446744073709551615
certificate for magnitude 8 holds: true
one multiply-accumulate at most: 18446744073709551615
n - 1 limbs: 3493216576 3218235020 2940772411 3132021990 4294967294 4294967295 4294967295 4294967295
(n - 1) + 1 is zero: true
strict load of p: ok false
load of 31 bytes: ok false
scalar load of n: ok false, overflow true
certificate for magnitude 9: ok false
```

The output matches the values worked out by hand:

- 2^26 sets bit 26, which is bit 2 of byte 28 from the left: `04` followed
  by three zero bytes.
- 2^256 − 1 reduces to limb 0 = 976 and limb 1 = 64.
- p − 1 starts with limbs 67,107,886 and 67,108,799, and its bytes end in
  `fc2e`.
- (p − 1)^2 is 1, because p − 1 is −1 mod p.
- Magnitude 8 reduces to limb 0 = 7,808 and limb 1 = 512.
- The largest product column is 10 × (2^26 − 1)^2.
- The scalar multiply-accumulate bound is exactly 2^64 − 1.

### The refusals

The last four lines are refusals:

- `sf_load_strict` refuses the bytes of p, because a strict load accepts only
  canonical values less than p. `sf_load` would reduce them to 0. Chapter 11
  uses the strict load for public keys and signature values.
- `sf_load` refuses a 31-byte input. It writes nothing and returns
  `ok = false`.
- `sc_load` refuses the bytes of n and reports `overflow = true`. A secret
  key or signature scalar must be less than n (chapter 12).
- `sf_mul_bounds` refuses magnitude 9.

## How the code is checked

The example adds two checks of its own:

<!-- source: nostr/examples/big-integers-in-limbs.coax check -->
```jai
// The hand-derived values from the chapter, checked against the code.
#test "hand-derived limbs match the code" {
  r := sf_normalize(all_ones(1));
  #assert r.limb[0] == 976 && r.limb[1] == 64;
  for i: 2..<10 { #assert r.limb[i] == 0; }
  // 10 * (2^26 - 1)^2, the largest column of a product of canonical limbs.
  #assert sf_mul_bounds(8).facts[5] == 10 * 67108863 * 67108863;
  // (2^32 - 1)^2 + 2 * (2^32 - 1) = 2^64 - 1 exactly.
  #assert SC_MAC_MAX == CB_MAX64;
}

// Every byte position, with every bit set, round-trips through strict load
// and store, and lands in the limb that bit / 26 names.
#forall "each byte maps to the limbs its bits name" at: 0..<32 {
  bytes: [32] u8; back: [32] u8;
  bytes[31 - at] = 255;
  r := sf_load_strict(bytes[..]);
  #assert r.ok && sf_store(r.value, back[..]);
  for i: 0..<32 { #assert back[i] == bytes[i]; }
  for bit: at * 8..<at * 8 + 8 {
    #assert ((r.value.limb[bit / 26] >> cast(u64) (bit % 26)) & 1) == 1;
  }
}
```

The library files carry the main checks:

- **The bound certificate.** A `#forall` over every magnitude from 1 to 8
  checks that `sf_mul_bounds` covers the actual schedule, and pins each of
  the 27 facts to an independently computed value. Another `#forall` raises
  each fact one past its limit and checks that `sf_mul_budget` refuses it.
- **Edge values.** Checks load 0, 1, p − 1 and 2^256 − 1, and normalize every
  limb at every magnitude from 1 to 8 to the independently computed residue
  m × (2^32 + 976).
- **Every bit.** A `#forall` over all 256 bit positions round-trips each
  single-bit value through store and strict load, and checks that it cancels
  under addition.
- **Scalar edges.** `secp_scalar.coax` checks that n and 0 are refused as
  keys, that every limb can make equality false, and that small scalars
  match ordinary arithmetic.

The Python harnesses compare the code with Python's arbitrary-precision
integers, an implementation that shares nothing with the limbs. The
reference is
[tools/coax-nostr/fuzz/oracle.py](../../tools/coax-nostr/fuzz/oracle.py):

- [coax/tests/nostr/secp_field.py](../../coax/tests/nostr/secp_field.py)
  builds `secp_field.coax` as a native library and as wasm. It checks
  normalize, load, store, strict load, add, subtract, negate and equality on
  0, 1, 2, p − 2, p − 1, p, p + 1, 2^256 − 1, every 2^i − 1, 2^i and 2^i + 1,
  1,024 random values, and raw limbs at every magnitude bound. Native and
  wasm results must be the same bytes.
- [coax/tests/nostr/scalar.py](../../coax/tests/nostr/scalar.py) does the
  same for the scalar functions mod n.

To run the checks:

```shell
coax/bin/coax test nostr/examples/big-integers-in-limbs.coax
coax/bin/coax mutate coax/lib/crypto/secp_field.coax
coax/bin/coax mutate coax/lib/crypto/secp_scalar.coax
python3 coax/tests/nostr/secp_field.py
python3 coax/tests/nostr/scalar.py
```

`coax test` on the example runs 57 checks, including the checks in the loaded
files. `coax mutate` kills every mutant: 541 in `secp_field.coax`, 240 in
`secp_scalar.coax` and 6 in the example, with zero survivors. Both Python
harnesses pass, and the native and wasm builds agree.

## Summary

- A field element is ten little-endian `u64` limbs: nine of 26 bits and one
  of 22 bits. Bit `b` is in limb `b / 26`.
- The spare bits in each limb absorb unreduced additions up to magnitude 8
  and the columns of a product.
- A bound certificate, computed at compile time with checked arithmetic,
  proves that no intermediate value of a multiplication overflows `u64`.
- A scalar is eight 32-bit limbs, because Montgomery reduction needs full
  words. One multiply-accumulate is at most exactly 2^64 − 1.
- Hand-derived values and a Python big-integer oracle check the code. The
  code is never checked against its own output.

## Dependencies

- [1. Bytes, hex and base64](ch001-bytes-hex-and-base64.md)
- [Coax chapter 13, Data-oriented layout](../../coax/book/ch13-data-layout.md)

## Sources

- [BIP-340](https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki),
  which defines p and n for secp256k1.
- [SEC 2](https://www.secg.org/sec2-v2.pdf), section 2.4.1, the secp256k1
  domain parameters.
- [NIP-01](../nips/official/01.md), which uses BIP-340 Schnorr signatures on
  secp256k1.
- The verification records:
  [coax-secp-field-2026-10-28.md](../../docs/verification/coax-secp-field-2026-10-28.md)
  and
  [coax-secp-field-mul-2026-10-28.md](../../docs/verification/coax-secp-field-mul-2026-10-28.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
