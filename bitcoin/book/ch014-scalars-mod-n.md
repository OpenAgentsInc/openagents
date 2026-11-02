# 14. Scalars mod n

The points of secp256k1 form a group with a prime number of elements, the
*group order*:

```text
n = fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141
```

Adding the generator `G` to itself `n` times gives the point at infinity,
so a multiplier of a point only matters mod `n`. Private keys, nonces, the
challenge `e` and the `s` half of a BIP-340 signature are all *scalars*:
integers mod `n`. This chapter builds scalar arithmetic on eight 32-bit
limbs, with Montgomery multiplication, and the range checks that BIP-340
requires.

`n` is not the field prime `p` of chapter 13. Both are just below 2^256,
but `n < p`: both start with 15 bytes of `ff`, and the 16th byte is `fe`
in `n` and `ff` in `p`. Don't mix them up: a coordinate is reduced mod
`p`, and a scalar mod `n`.

## What this chapter teaches

- Reducing a 32-byte number mod `n` with at most one subtraction, as
  BIP-340 does for the challenge `e`.
- Refusing a private key of 0 or one at least `n`.
- Refusing a signature whose `s` is at least `n`.
- Addition, subtraction and negation with a masked correction.
- Montgomery multiplication: `R = 2^256`, the constants `n0inv` and
  `R^2 mod n`, and why REDC needs one subtraction at the end.
- Inversion as `a^(n-2)` over the public bits of `n - 2`.

## Where the code is

[coax/lib/crypto/secp_scalar.coax](../../coax/lib/crypto/secp_scalar.coax)
has the scalars. A `ScScalar` is eight little-endian `u32` limbs
(chapter 2).

| Procedure | Result |
| --- | --- |
| `sc_reduce_bytes` | 32 big-endian bytes reduced mod `n`, with an `overflow` flag that says the input was `>= n` |
| `sc_load` | 32 big-endian bytes that must be below `n`; otherwise `ok` is false and the value is 0 |
| `sc_store` | a canonical scalar to 32 big-endian bytes; refuses a value `>= n` |
| `sc_secret_valid` | true for a scalar in `1 .. n-1` |
| `sc_add`, `sc_sub`, `sc_negate` | `a + b`, `a - b`, `-a` |
| `sc_mul` | `a * b`, by Montgomery multiplication |
| `sc_inverse` | `1 / a`; `ok` is false for 0 |
| `sc_window` | 1 to 8 bits at a public position, for scalar multiplication (chapter 16) |
| `sc_is_zero`, `sc_equal` | comparisons that read every limb |

## Reduction

`n` is 2^256 minus a 129-bit number:

```text
2^256 - n = 14551231950b75fc4402da1732fc9bebf
```

That number is far smaller than `n`, so `2^256 < 2n`. Any 256-bit value is
below `2n`, and one subtraction of `n` reduces it. `sc_reduce9` computes
`a - n` with a borrow chain and keeps `a` if the subtraction borrowed, or
`a - n` if it didn't. `sc_select` makes the choice with a mask, not a
branch (chapter 3).

### Worked example: 2^256 − 1 mod n

1. `2^256 - 1` is at least `n`, so `overflow` is true.
2. Subtract `n` once: `(2^256 - 1) - n = (2^256 - n) - 1`.
3. `2^256 - n` ends in `...bebf`, so the result ends in `...bebe`:

```text
000000000000000000000000000000014551231950b75fc4402da1732fc9bebe
```

### Which inputs reduce and which refuse

BIP-340 treats a 32-byte value in three ways:

- **The challenge `e`** is a SHA-256 output reduced mod `n`. Use
  `sc_reduce_bytes`. A hash is at least `n` with probability about
  2^-128, and that case is not an error.
- **A secret key** must be in `1 .. n-1`. Use `sc_load`, then
  `sc_secret_valid`. 0 and any value `>= n` are refused.
- **The `s` of a signature** must be below `n`. Use `sc_load`. A value
  `>= n` makes the signature invalid; it is never reduced.

`sc_load` sets `ok` to false and the value to 0 when the input is `>= n`,
so a caller that ignores `ok` still never gets a reduced value.

## Addition, subtraction and negation

`sc_add` adds the limbs with a carry into a ninth limb and reduces the
9-limb sum with `sc_reduce9`. Both inputs are below `n`, so the sum is
below `2n` and one subtraction is enough.

`sc_sub` subtracts with a borrow chain. If the last borrow is 1, the
result went below zero, so it adds `n` back. The code adds `n` masked by
the borrow: all of `n` when it borrowed, and 0 when it didn't. Both cases
run the same additions.

`sc_negate(a)` is `sc_sub(0, a)`. So `-1` is `n - 1`.

## Montgomery multiplication

A 512-bit product needs a reduction mod `n`, and dividing by `n` is slow
and hard to make data-independent. Montgomery multiplication replaces the
division with shifts by whole words.

Take `R = 2^256`. For `a` and `b` below `n`, `sc_montgomery(a, b)`
computes `a · b · R^-1 mod n`:

1. Multiply `a` and `b` into a 16-word product `T`, one 32 × 32-bit
   multiply-accumulate at a time (chapter 2).
2. For each of the 8 low words, compute `factor = t[i] · n0inv mod 2^32`
   and add `factor · n · 2^(32i)`. `n0inv` is chosen so that
   `n[0] · n0inv ≡ -1 (mod 2^32)`, so each round makes word `i` zero.
3. After 8 rounds the low 256 bits are zero. Shift them off: that divides
   by `R`. Adding multiples of `n` doesn't change the value mod `n`.
4. The result is below `2n`, so one masked subtraction makes it canonical.

The `R^-1` factor is removed with the precomputed constant
`SC_R2 = R^2 mod n`:

```text
sc_mul(a, b) = mont(mont(a, R^2), b)
             = (a · R^2 · R^-1) · b · R^-1
             = a · b  (mod n)
```

## Inversion

`n` is prime, so `a^-1 = a^(n-2)`, as in chapter 13. `sc_inverse` runs 256
rounds of square-and-multiply over the bits of `n - 2`, from the top bit
down. Each round squares, computes the product with `a` too, and keeps the
product only when the exponent bit is 1. The exponent is a public
constant, so the choice depends on no secret; the code still computes both
candidates and selects with a mask.

0 has no inverse. `sc_inverse` runs every round on it and returns `ok`
false.

### Worked example: the inverse of 2

`n` is odd, so `(n + 1) / 2` is an integer, and
`2 · (n + 1) / 2 = n + 1 ≡ 1`. Halving `n + 1`, which ends in `...4142`,
gives a number that ends in `...20a1`:

```text
1/2 = 7fffffffffffffffffffffffffffffff5d576e7357a4501ddfe92f46681b20a1
```

## The example

The example is [bitcoin/examples/scalars.coax](../examples/scalars.coax). It
prints `n`'s limbs, reduces `2^256 - 1`, wraps addition and multiplication
at `n`, inverts 2, checks the Montgomery constants, admits four secret keys
and shows three refusals.

<!-- source: bitcoin/examples/scalars.coax example -->
```jai
#load "../../coax/lib/crypto/secp_scalar.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [80] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// Print a scalar as 32 big-endian bytes.
show_scalar :: (label: string, a: ScScalar) {
  b: [32] u8;
  if sc_store(a, b[..]) { show(label, b[..]); } else { print("%: not canonical\n", label); }
}
// Decode 64 hex digits into 32 bytes; false if the hex is bad.
bytes_of :: (text: string, out: [] u8) -> bool {
  return hex_decode_checked(text, out, false).error == 0;
}
// Name the outcome of admitting 32 bytes as a BIP-340 secret key.
SECRET :: [3] string.["ok", "zero", "not below n"];
secret_status :: (bytes: [] u8) -> int {
  loaded := sc_load(bytes);
  if !loaded.ok { return 2; }
  if !sc_secret_valid(loaded.value) { return 1; }
  return 0;
}

main :: () {
  // n in eight 32-bit limbs, lowest first.
  print("n limbs: %\n", SC_N.limbs);

  // 2^256 - 1 reduces to 2^256 - 1 - n.
  b: [32] u8;
  if !bytes_of("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff", b[..]) { print("bad hex\n"); }
  reduced := sc_reduce_bytes(b[..]);
  print("reduce 2^256 - 1: ok %, overflow %\n", reduced.ok, reduced.overflow);
  show_scalar("2^256 - 1 mod n", reduced.value);

  // Arithmetic wraps at n.
  minus_one := sc_negate(sc_small(1));
  show_scalar("-1", minus_one);
  show_scalar("(n - 1) + 2", sc_add(minus_one, sc_small(2)));
  show_scalar("(n - 1) * (n - 1)", sc_mul(minus_one, minus_one));
  inv := sc_inverse(sc_small(2));
  print("inverse of 2: ok %\n", inv.ok);
  show_scalar("1/2", inv.value);
  show_scalar("2 * (1/2)", sc_mul(sc_small(2), inv.value));

  // Montgomery constants: n0 * n0inv = -1 mod 2^32, and R^2 mod n.
  print("n0 * n0inv mod 2^32: %\n", (cast(u64) SC_N.limbs[0] * SC_N0_INV) & SC_WORD);
  show_scalar("R^2 mod n", SC_R2);

  // Secret keys must be in 1..n-1.
  keys := [4] string.[
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000001",
    "fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364140",
    "fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141"];
  labels := [4] string.["0", "1", "n - 1", "n"];
  for i: 0..<4 {
    if !bytes_of(keys[i], b[..]) { print("bad hex\n"); }
    print("secret key %: %\n", labels[i], SECRET[secret_status(b[..])]);
  }

  // A signature's s must be below n: sc_load refuses n and clears the value.
  s := sc_load(b[..]);
  print("s = n: ok %, overflow %, value is zero %\n", s.ok, s.overflow, sc_is_zero(s.value));
  print("31-byte s: ok %\n", sc_load(b[0..<31]).ok);
  print("inverse of 0: ok %\n", sc_inverse(sc_small(0)).ok);
}
```

Run it from the repository root:

```shell
coax/bin/coax run bitcoin/examples/scalars.coax
```

The output matches the values worked out by hand: `2^256 - 1` reduces to
`...bebe` with `overflow` set, `-1` is `n - 1`, and `1/2` is
`(n + 1) / 2`:

```text
n limbs: [3493216577, 3218235020, 2940772411, 3132021990, 4294967294, 4294967295, 4294967295, 4294967295]
reduce 2^256 - 1: ok true, overflow true
2^256 - 1 mod n: 000000000000000000000000000000014551231950b75fc4402da1732fc9bebe
-1: fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364140
(n - 1) + 2: 0000000000000000000000000000000000000000000000000000000000000001
(n - 1) * (n - 1): 0000000000000000000000000000000000000000000000000000000000000001
inverse of 2: ok true
1/2: 7fffffffffffffffffffffffffffffff5d576e7357a4501ddfe92f46681b20a1
2 * (1/2): 0000000000000000000000000000000000000000000000000000000000000001
n0 * n0inv mod 2^32: 4294967295
R^2 mod n: 9d671cd581c69bc5e697f5e45bcd07c6741496c20e7cf878896cf21467d7d140
secret key 0: zero
secret key 1: ok
secret key n - 1: ok
secret key n: not below n
s = n: ok false, overflow true, value is zero true
31-byte s: ok false
inverse of 0: ok false
```

`(n - 1) · (n - 1)` is 1, because `(-1)^2 = 1`. `n0 · n0inv mod 2^32` is
4,294,967,295, which is `2^32 - 1`, or `-1 mod 2^32`, as REDC needs.

The refusals:

- The secret keys 0 and `n` are refused, each with its own outcome:
  `sc_secret_valid` refuses 0, and `sc_load` refuses `n` as not below `n`.
  `n - 1` is the largest valid key.
- An `s` equal to `n` is refused with `overflow` set, and its value is
  cleared to 0.
- A 31-byte view is refused.
- The inverse of 0 is refused.

The book's checks run this program in debug and release builds and compare
the output with [bitcoin/fixtures/scalars.out](../fixtures/scalars.out).

## How the code is checked

### An independent big-integer oracle

[coax/tests/nostr/scalar.py](../../coax/tests/nostr/scalar.py) compiles
the scalars to C and to wasm32 and compares them with Python's
arbitrary-precision integers:

- every pair of nine edge values (0, 1, `2^32 - 1`, `2^32`, `2^64 - 1`,
  `2^255`, `n - 1`, `n` and `2^256 - 1`) under all six operations;
- one million seeded random operations, in debug and release builds;
- canonical bytes, windows and aliased outputs;
- a planted lost carry in the high product, which the oracle detects.

```shell
python3 coax/tests/nostr/scalar.py
```

### Checks in Coax

- `secp_scalar.coax` checks the multiply-accumulate bound, small scalars
  against ordinary arithmetic, that every limb can make equality and zero
  false, every one-bit window, and the admission of `n`, 0 and `2^256 - 1`
  with the output bytes unchanged on refusal.
- The example computes `R^2 mod n` with additions only, by doubling
  `R = 2^256 mod n` 256 times, and compares it with `SC_R2`. It also checks
  that multiplying by `k` equals `k` additions and that every edge value
  times its inverse is 1:

<!-- source: bitcoin/examples/scalars.coax check -->
```jai
// R^2 mod n by doubling only: start from R = 2^256 mod n and add it to
// itself 256 times, which multiplies by 2^256. The result is the
// Montgomery constant, computed without any multiplication.
#test "doubling R 256 times gives SC_R2" #steps 100000000 {
  b: [32] u8;
  for i: 0..<32 { b[i] = 255; }
  r := sc_add(sc_reduce_bytes(b[..]).value, sc_small(1));
  x := r;
  for i: 0..<256 { x = sc_add(x, x); }
  #assert sc_equal(x, SC_R2);
}
// Six values near 0, near n and in between.
EDGES :: [6] string.[
    "0000000000000000000000000000000000000000000000000000000000000001",
    "0000000000000000000000000000000000000000000000000000000100000000",
    "7fffffffffffffffffffffffffffffff5d576e7357a4501ddfe92f46681b20a1",
    "fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364140",
    "fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd036413f",
    "0123456789abcdeffedcba98765432100123456789abcdeffedcba9876543210"];
// Multiplication by k equals k additions, and subtraction undoes addition.
#forall "multiplying by k is adding k times" which: 0..<6, k: 0..<6 {
  b: [32] u8;
  #assert bytes_of(EDGES[which], b[..]);
  loaded := sc_load(b[..]);
  #assert loaded.ok;
  a := loaded.value;
  sum: ScScalar;
  for i: 0..<k { sum = sc_add(sum, a); }
  #assert sc_equal(sc_mul(a, sc_small(cast(u32) k)), sum);
  #assert sc_equal(sc_sub(sc_add(a, sum), sum), a);
}
// The inverse multiplies back to 1.
#forall "a times its inverse is 1" which: 0..<6 #steps 100000000 {
  b: [32] u8;
  #assert bytes_of(EDGES[which], b[..]);
  a := sc_load(b[..]).value;
  inv := sc_inverse(a);
  #assert inv.ok && sc_equal(sc_mul(a, inv.value), sc_small(1));
}
```

To run all the checks:

```shell
coax/bin/coax test bitcoin/examples/scalars.coax
coax/bin/coax mutate coax/lib/crypto/secp_scalar.coax
```

`coax mutate` leaves zero survivors: 385 mutants in `secp_scalar.coax` with
the files it loads, and 480 in the example with the files it loads.
`coax test` on the example runs 32 checks. `scalar.py` runs its million
random operations in about 30 seconds.

## Summary

- Scalars are integers mod the group order `n`, not the field prime `p`.
- `2^256 < 2n`, so one masked subtraction reduces any 32-byte value.
- The challenge `e` is reduced; a secret key must be in `1 .. n-1`; an `s`
  at least `n` is refused.
- Multiplication is Montgomery's: `mont(mont(a, R^2), b) = a · b`.
- The inverse is `a^(n-2)` over the public bits of `n - 2`.

## Dependencies

- [2. Big integers in fixed limbs](ch002-big-integers-in-fixed-limbs.md)
- [3. Constant-time rules and secrets](ch003-constant-time-rules.md)

## Sources

- [SEC 2: Recommended Elliptic Curve Domain Parameters](https://www.secg.org/sec2-v2.pdf),
  section 2.4.1, for `n`.
- [BIP-340](https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki),
  which defines the key and `s` ranges and reduces `e` mod `n`.
- Peter L. Montgomery, "Modular Multiplication Without Trial Division",
  Mathematics of Computation 44 (1985).
- Notes: [coax/tests/nostr/SCALAR.md](../../coax/tests/nostr/SCALAR.md).
