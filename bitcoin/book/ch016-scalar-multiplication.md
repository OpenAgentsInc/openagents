# 16. Scalar multiplication

Signing, verifying and ECDH each multiply a point by a scalar: `k·P` is `P`
added to itself `k` times. Verification works on public data and may run in
variable time. Signing and ECDH use secrets and must not. This chapter
builds three multiplications on the group law of chapter 15 and says which
later chapter uses each one.

## What this chapter teaches

- Double-and-add with signed windows (wNAF) for public verification,
  `s·G - e·P`.
- A precomputed comb table for `k·G` that reads every table entry.
- A fixed-work signed radix-16 ladder for `k·P` with a secret `k`.
- What each path costs, and why GLV and larger tables wait for measured
  need.

## Where the code is

| File | Procedure | Inputs | Used by |
| --- | --- | --- | --- |
| [secp_mult.coax](../../coax/lib/crypto/secp_mult.coax) | `ecmult_vartime(a, P, b)` | public `a`, `b`, `P` | verification (chapter 23) |
| [secp_mult_base.coax](../../coax/lib/crypto/secp_mult_base.coax) | `smb_comb` | secret `k` | key generation and signing (chapters 18 and 24) |
| [secp_mult_base_native.coax](../../coax/lib/crypto/secp_mult_base_native.coax) | `smb_multiply_native(k)` | 8 teeth, spacing 32, 256 entries | native builds |
| [secp_mult_base_web.coax](../../coax/lib/crypto/secp_mult_base_web.coax) | `smb_multiply_web(k)` | 7 teeth, spacing 37, 128 entries | wasm builds |
| [secp_mult_const.coax](../../coax/lib/crypto/secp_mult_const.coax) | `smc_multiply(k, P)` | secret `k`, public peer `P` | ECDH (Nostr chapter 14) |

Every procedure returns canonical coordinates and the canonical infinity
`(1, 1, 0)` of chapter 15. Each `_vartime` name marks code that branches and
indexes on its inputs.

## Double-and-add

Read `k` from its top bit down. For each bit, double the running sum, then
add `P` if the bit is 1. A 256-bit `k` costs 256 doublings and, on average,
128 additions. Two ideas cut the additions.

### Signed windows (wNAF)

A width-`w` NAF writes `k` as digits `d[i]` that are zero or odd with
`|d[i]| < 2^(w-1)`, and any nonzero digit is followed by at least `w - 1`
zeros. A negative digit costs the same as a positive one, because `-P` is
`(x, p - y)`. `sm_wnaf_vartime` computes the digits from the bottom:

1. If `k` is even, the digit is 0.
2. If `k` is odd, the digit is `k mod 2^w`, minus `2^w` when that is at
   least `2^(w-1)`. Subtract the digit from `k`.
3. Halve `k` and repeat, 257 times, so a carry out of bit 255 is kept.

### Worked example: 200 with w = 5 and w = 2

`200 = 0xc8`. With `w = 5`:

1. Bits 0, 1 and 2: 200, 100 and 50 are even. `k` becomes 25.
2. Bit 3: `25 mod 32 = 25`, which is at least 16, so the digit is
   `25 - 32 = -7`. `k` becomes `(25 + 7) / 2 = 16`.
3. Bits 4 to 7: 16, 8, 4 and 2 are even. `k` becomes 1.
4. Bit 8: the digit is 1, and `k` becomes 0.

So `200 = -7·2^3 + 1·2^8 = -56 + 256`: two additions instead of three.
With `w = 2`, the same steps give `200 = 2^3 - 2^6 + 2^8`.

### The verification sum

BIP-340 verification computes `s·G - e·P`. `ecmult_vartime(a, P, b)`
computes `a·G + b·P` in one loop, sharing the doublings. `G` uses `w = 2`
with the two-entry table `[G, -G]`, built at compile time; `P` uses `w = 5`
with the odd multiples `P, 3P, ..., 15P`, built with one doubling and seven
additions. The loop runs to the highest nonzero digit of either recoding:
at most 257 doublings, 129 additions of `±G` and 52 additions of `±P`.

To compute `s·G - e·P`, pass `b = n - e` (`sc_negate(e)`).

### Worked example: s·G - e·P by hand

Take the secret key `d = 3`, so `P = 3G`, the nonce `k = 5` and the
challenge `e = 2`. A signature has `s = k + e·d = 5 + 6 = 11`. Then
`s·G - e·P = 11G - 2·3G = 5G`, which is the nonce point `k·G`. The
example computes this sum with `ecmult_vartime` and compares it with `5G`.

## The fixed-base comb

When the point is always `G`, you can precompute. The comb splits the 256
bits of `k` into `t` *teeth* of `s` bits each (`t·s ≥ 256`). Entry `j` of
the table, for a `t`-bit `j`, is the sum of `2^(i·s)·G` over the bits `i`
set in `j`. Row `r` of the comb gathers bit `r + i·s` of `k` for each tooth
`i` into an index. Then

```text
k·G = sum over rows r of 2^r · table[index(r)]
```

which the code evaluates from row `s - 1` down: one doubling and one
addition per row.

| Profile | Teeth | Spacing | Entries | Table bytes | Doublings | Additions |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Native | 8 | 32 | 256 | 61,440 | 31 | 31 |
| Web | 7 | 37 | 128 | 30,720 | 36 | 36 |

The scalar is secret, so its index must not choose a memory address
(chapter 3). `smb_pick_generated` reads every entry and keeps the one whose
position equals the index with a mask. The native profile reads 256 entries
on each of its 32 rows.

The tables are built in Coax at compile time by `#run`. The native and web
tables together take about 167 million evaluation steps, more than the
default 100,000, so a program that loads them compiles with
`--run-steps 250000000`.

### Worked example: comb rows

In the native profile, row `r` reads bits `r, r + 32, ..., r + 224`.

- `200` has bits 3, 6 and 7 set, all in tooth 0. Rows 3, 6 and 7 have
  index 1 (the entry `G`); every other row has index 0 (infinity). The sum
  is `(2^3 + 2^6 + 2^7)·G = 200G`.
- `2^32 + 1` has bit 0 (tooth 0, row 0) and bit 32 (tooth 1, row 0) set.
  Row 0 has index `1 + 2 = 3`, the entry `G + 2^32·G`, and every other row
  has index 0.

## The constant-time ladder

ECDH multiplies the peer's point, so no table can be precomputed.
`smc_multiply(k, P)`:

1. reduces `k` mod `n` and recodes it into 65 signed radix-16 digits, the
   low 64 in `-8..7` and the top one 0 or 1, with the sign kept as a mask;
2. builds `0·P, 1·P, ..., 8·P` with seven additions;
3. starts from the top digit, then for each of the 64 lower digits doubles
   four times, reads all nine table entries to pick `|d|·P`, negates it with
   a mask when `d` is negative, and adds.

Every scalar runs 256 doublings and 64 additions after the seven that build
the table, and reads the same memory.
The peer must be on the curve, with every coordinate limb inside the field
code's bound; otherwise the result is `ok` false and infinity. Whether a
peer is acceptable for ECDH is Nostr chapter 14's decision.

### Worked example: 200 in signed radix 16

`smc_recode` adds the carry to each 4-bit digit and, when the sum is 8 or
more, stores `16 - value` as a negative digit and carries 1:

1. Digit 0: `8 + 0 = 8`, so the digit is `-8` and the carry is 1.
2. Digit 1: `0xc + 1 = 13`, so the digit is `-3` and the carry is 1.
3. Digit 2: `0 + 1 = 1`.

`-8 - 3·16 + 1·256 = 200`.

## The example

The example is [bitcoin/examples/scalar-mult.coax](../examples/scalar-mult.coax).
It prints the three recodings of 200 and its comb rows, computes `k·G` for
small and edge scalars by every route, computes `11G - 2(3G)`, multiplies
the BIP-340 vector 1 secret key, and shows four refusals.

<!-- source: bitcoin/examples/scalar-mult.coax example -->
```jai
#load "../../coax/lib/crypto/secp_mult.coax";
#load "../../coax/lib/crypto/secp_mult_base_native.coax";
#load "../../coax/lib/crypto/secp_mult_base_web.coax";
#load "../../coax/lib/crypto/secp_mult_const.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [80] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// Print a point's affine x, or "infinity".
show_x :: (label: string, p: SgPoint) {
  a := sg_to_affine(p);
  if !a.ok { print("%: not a valid point\n", label); return; }
  if a.value.infinity != 0 { print("%: infinity\n", label); return; }
  b: [32] u8;
  if sf_store(a.value.x, b[..]) { show(label, b[..]); }
}
// Decode 64 hex digits into a scalar; the value may be n or more.
scalar_of :: (text: string) -> ScScalar {
  b: [32] u8;
  #assert hex_decode_checked(text, b[..], false).error == 0;
  return sc_reduce_bytes(b[..]).value;
}
// The same 32 bytes without reduction, for the raw recoding.
raw_scalar_of :: (text: string) -> ScScalar {
  b: [32] u8; r: ScScalar;
  #assert hex_decode_checked(text, b[..], false).error == 0;
  for i: 0..<32 { r.limbs[i / 4] |= cast(u32) b[31 - i] << (8 * (i % 4)); }
  return r;
}
// True when two points are the same valid point.
same_point :: (a: SgPoint, b: SgPoint) -> bool { return sg_equal_mask(a, b) == CB_MAX64; }
// Print the nonzero digits of a wNAF as "position:digit".
show_wnaf :: (label: string, w: SmWnaf) {
  print("%: length %, digits", label, w.length);
  for i: 0..<w.length { if w.digits[i] != 0 { print(" %:%", i, w.digits[i]); } }
  print("\n");
}
// Print the nonzero signed radix-16 digits as "position:digit".
show_radix16 :: (label: string, d: SmcDigits) {
  print("%: digits", label);
  for i: 0..<65 {
    if d.magnitude[i] != 0 {
      sign := ifx d.negative[i] != 0 then "-" else "";
      print(" %:%", i, sign); print("%", d.magnitude[i]);
    }
  }
  print("\n");
}
// The comb's table index for one row of the native profile.
comb_index :: (k: ScScalar, row: int) -> u64 {
  wide: [9] u32;
  for i: 0..<8 { wide[i] = k.limbs[i]; }
  return smb_index(wide, smb_native_width(), smb_native_spacing(), row);
}
// k·G by all four routes; true when every route gives the same point.
all_agree :: (k: ScScalar) -> bool {
  g := sg_generator();
  native := smb_multiply_native(k);
  web := smb_multiply_web(k);
  ladder := smc_multiply(k, g);
  public := ecmult_vartime(k, sg_infinity(), sc_small(0));
  return ladder.ok && public.ok && same_point(native, web) &&
         same_point(native, ladder.value) && same_point(native, public.value);
}

N_MINUS_1 :: "fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364140";
N_HEX     :: "fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141";

main :: () {
  g := sg_generator();
  inf := sg_infinity();

  // The three recodings of k = 200 = 0xc8.
  k := sc_small(200);
  show_wnaf("wNAF w=5 of 200", sm_wnaf_vartime(k, 5));
  show_wnaf("wNAF w=2 of 200", sm_wnaf_vartime(k, 2));
  show_radix16("radix 16 of 200", smc_recode(k));
  print("comb rows of 200:");
  for row: 0..<32 { if comb_index(k, row) != 0 { print(" %:%", row, comb_index(k, row)); } }
  print("\n");
  teeth := sc_small(1); teeth.limbs[1] = 1;
  print("comb row 0 of 2^32 + 1: index %\n", comb_index(teeth, 0));

  // k·G for small and edge scalars; every route must agree.
  show_x("200G", smb_multiply_native(k));
  print("200G, all routes agree: %\n", all_agree(k));
  show_x("(2^32 + 1)G", smb_multiply_native(teeth));
  print("(2^32 + 1)G, all routes agree: %\n", all_agree(teeth));
  minus := scalar_of(N_MINUS_1);
  print("(n - 1)G = -G: %\n", same_point(smb_multiply_native(minus), sg_negate(g).value));
  print("(n - 1)G, all routes agree: %\n", all_agree(minus));
  print("0G is infinity: %\n", same_point(smb_multiply_native(sc_small(0)), inf));
  print("0G, all routes agree: %\n", all_agree(sc_small(0)));
  print("nG is infinity on every route: %\n",
        all_agree(raw_scalar_of(N_HEX)) && same_point(smb_multiply_native(raw_scalar_of(N_HEX)), inf));

  // A verification-shaped sum: s·G - e·P with P = 3G, e = 2, s = 11.
  p := smc_multiply(sc_small(3), g).value;
  e := sc_small(2);
  r := ecmult_vartime(sc_small(11), p, sc_negate(e));
  print("11G - 2(3G): ok %\n", r.ok);
  show_x("11G - 2(3G)", r.value);
  print("equal to 5G: %\n", same_point(r.value, smb_multiply_native(sc_small(5))));

  // A BIP-340 secret key and its public key x.
  key := scalar_of("b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef");
  show_x("BIP-340 vector 1 key times G", smb_multiply_native(key));

  // Refusals.
  off: SgPoint; off.x.limb[0] = 1; off.y.limb[0] = 1; off.z.limb[0] = 1;
  refused := ecmult_vartime(sc_small(1), off, sc_small(1));
  print("ecmult_vartime with (1, 1): ok %, value is infinity %\n", refused.ok, same_point(refused.value, inf));
  refused = smc_multiply(sc_small(1), off);
  print("smc_multiply with (1, 1): ok %, value is infinity %\n", refused.ok, same_point(refused.value, inf));
  wide := g; wide.x.limb[0] = CB_MAX64;
  refused = smc_multiply(sc_small(1), wide);
  print("smc_multiply with an oversized limb: ok %\n", refused.ok);
  print("wNAF with window 6: ok %\n", sm_wnaf_vartime(k, 6).ok);
}
```

Run it from the repository root:

```shell
coax/bin/coax run bitcoin/examples/scalar-mult.coax --run-steps 250000000
```

The output matches the values worked out by hand: the digits of 200, the
comb index 3 for `2^32 + 1`, and `11G - 2(3G) = 5G`. The x of the BIP-340
vector 1 key is the published public key `dff1d77f...502ba659`:

```text
wNAF w=5 of 200: length 9, digits 3:-7 8:1
wNAF w=2 of 200: length 9, digits 3:1 6:-1 8:1
radix 16 of 200: digits 0:-8 1:-3 2:1
comb rows of 200: 3:1 6:1 7:1
comb row 0 of 2^32 + 1: index 3
200G: cd5a3be41717d65683fe7a9de8ae5b4b8feced69f26a8b55eeefbcc2e74b75fb
200G, all routes agree: true
(2^32 + 1)G: 0ed7ffde4cb4fd27164ae5f435e4a3ae1905ba30e9f06eccab50fcbd9f341f81
(2^32 + 1)G, all routes agree: true
(n - 1)G = -G: true
(n - 1)G, all routes agree: true
0G is infinity: true
0G, all routes agree: true
nG is infinity on every route: true
11G - 2(3G): ok true
11G - 2(3G): 2f8bde4d1a07209355b4a7250a5c5128e88b84bddc619ab7cba8d569b240efe4
equal to 5G: true
BIP-340 vector 1 key times G: dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659
ecmult_vartime with (1, 1): ok false, value is infinity true
smc_multiply with (1, 1): ok false, value is infinity true
smc_multiply with an oversized limb: ok false
wNAF with window 6: ok false
```

`(n - 1)G` is `-G`, and both `0` and `n` give infinity on every route: the
group has order `n`.

The refusals:

- `(1, 1)` is not on the curve, so `ecmult_vartime` and `smc_multiply`
  return `ok` false and infinity.
- `G` with one limb set to `2^64 - 1` is outside the bound that the field
  code needs, so `smc_multiply` refuses it before any arithmetic.
- Window 6 is outside 2..5, so `sm_wnaf_vartime` returns `ok` false.

The book's checks run this program in debug and release builds and compare
the output with [bitcoin/fixtures/scalar-mult.out](../fixtures/scalar-mult.out).

## Costs

Measured on an Apple M5 Max (arm64, Apple clang `-O2`, release build),
2026-11-01, by the timing harnesses below, on a loaded machine:

| Path | Mean time per call |
| --- | ---: |
| Native comb, `smb_multiply_native` | 136 µs |
| Web comb, `smb_multiply_web` | 150 µs |
| Ladder, `smc_multiply` | 446 µs (run beside the comb measurement) |

The comb's table scan dominates its cost: 32 rows of 256 entries of 30
limbs. The two other levers are the ones libsecp256k1 uses:

- **GLV.** secp256k1 has a map `(x, y) -> (βx, y)` that equals
  multiplication by a fixed `λ`. It splits `k` into two 128-bit halves and
  halves the doublings.
- **Larger tables.** A window-15 table for `G` in verification, as
  libsecp256k1 builds, costs about a megabyte.

[Chapter 77](ch077-faster-verification-the-glv-endomorphism.md) builds
GLV for the verification path and measures the gain; it keeps the small
tables.

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/scalar-mult.coax](../examples/checks/scalar-mult.coax),
which loads the example, so running the example doesn't run them. They
compare every route with independent values:

- 10 points `k·G`, computed with Python's integers by affine
  double-and-add, for the native comb, the ladder and the window-5 path;
- the BIP-340 vectors 0 to 3, whose secret keys must give the published x,
  for the native and web combs and the window-2 path;
- `k·P` for `P = 7G` and `a·G + b·P` for `P = 5G` against repeated
  addition, which uses only chapter 15's group law;
- `2^bit·G` against `bit` doublings, at the first and last rows of the
  teeth of both combs.

<!-- source: bitcoin/examples/checks/scalar-mult.coax check -->
```jai
// k·G for 10 scalars as (k, x, y) hex, computed with Python's
// arbitrary-precision integers by affine double-and-add. They share no code
// with the three multiplications.
POINTS :: 10;
POINT_HEX :: [30] string.[
    "0000000000000000000000000000000000000000000000000000000000000001", "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798", "483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8",
    "0000000000000000000000000000000000000000000000000000000000000002", "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5", "1ae168fea63dc339a3c58419466ceaeef7f632653266d0e1236431a950cfe52a",
    "0000000000000000000000000000000000000000000000000000000000000007", "5cbdf0646e5db4eaa398f365f2ea7a0e3d419b7e0330e39ce92bddedcac4f9bc", "6aebca40ba255960a3178d6d861a54dba813d0b813fde7b5a5082628087264da",
    "0000000000000000000000000000000000000000000000000000000000000008", "2f01e5e15cca351daff3843fb70f3c2f0a1bdd05e5af888a67784ef3e10a2a01", "5c4da8a741539949293d082a132d13b4c2e213d6ba5b7617b5da2cb76cbde904",
    "0000000000000000000000000000000000000000000000000000000000000011", "defdea4cdb677750a420fee807eacf21eb9898ae79b9768766e4faa04a2d4a34", "4211ab0694635168e997b0ead2a93daeced1f4a04a95c0f6cfb199f69e56eb77",
    "00000000000000000000000000000000000000000000000000000000000000c8", "cd5a3be41717d65683fe7a9de8ae5b4b8feced69f26a8b55eeefbcc2e74b75fb", "fd6381eaf29657fdb1373fbba578001efa92c34b1f38c89f9c0b605ba95832a5",
    "0000000000000000000000000000000000000000000000000000000100000001", "0ed7ffde4cb4fd27164ae5f435e4a3ae1905ba30e9f06eccab50fcbd9f341f81", "731e74549045132626c4b20dd9971afffdcf3a400e12b2dd7f887edfb8b205a8",
    "8000000000000000000000000000000000000000000000000000000000000000", "b23790a42be63e1b251ad6c94fdef07271ec0aada31db6c3e8bd32043f8be384", "fc6b694919d55edbe8d50f88aa81f94517f004f4149ecb58d10a473deb19880e",
    "fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364140", "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798", "b7c52588d95c3b9aa25b0403f1eef75702e84bb7597aabe663b82f6f04ef2777",
    "766bad0734c2da8003cc0f2793fdcab87b89296c6dcbac5008577eb1924770d4", "77dead7787a849d25f81bf8924141e3833a7b963a5cb4feb2b14e9fdd088418b", "9da5f512cf7ec3a9d50aa239691dda54589022e7f32f62673af93e4c0c19b98f"];
oracle_point :: (i: int) -> SgPoint {
  b: [32] u8; a: SgAffine;
  #assert hex_decode_checked(POINT_HEX[3 * i + 1], b[..], false).error == 0; a.x = sf_load(b[..]).value;
  #assert hex_decode_checked(POINT_HEX[3 * i + 2], b[..], false).error == 0; a.y = sf_load(b[..]).value;
  return sg_from_affine(a).value;
}
// The fixed-base comb, the ladder and the public window-5 path.
#forall "every route gives the oracle's point" i: 0..<POINTS #steps 1000000000 {
  k := scalar_of(POINT_HEX[3 * i]);
  want := oracle_point(i);
  #assert same_point(smb_multiply_native(k), want);
  ladder := smc_multiply(k, sg_generator());
  #assert ladder.ok && same_point(ladder.value, want);
  public := ecmult_vartime(sc_small(0), sg_generator(), k);
  #assert public.ok && same_point(public.value, want);
}
// The BIP-340 test vectors 0 to 3: each secret key times G has the
// published public key as its x. These also run the web comb and the
// public window-2 path for G.
BIP340 :: [8] string.[
    "0000000000000000000000000000000000000000000000000000000000000003", "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9",
    "b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef", "dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659",
    "c90fdaa22168c234c4c6628b80dc1cd129024e088a67cc74020bbea63b14e5c9", "dd308afec5777e13121fa72b9cc1b7cc0139715309b086c960e18fd969774eb8",
    "0b432b2677937381aef05bb02a66ecd012773062cf3fa2549e44f58ed2401710", "25d1dff95105f5253c4022f628a996ad3a0d95fbf21d468a1b33f8c160d8f517"];
#forall "secret keys give the BIP-340 public keys" i: 0..<4 #steps 1000000000 {
  k := scalar_of(BIP340[2 * i]);
  b: [32] u8;
  #assert hex_decode_checked(BIP340[2 * i + 1], b[..], false).error == 0;
  lifted := sg_lift_x_public(b[..]);
  #assert lifted.ok;
  // The published key is x only: k·G is the lifted point or its negative.
  got := smb_multiply_native(k);
  #assert same_point(got, lifted.value) || same_point(got, sg_negate(lifted.value).value);
  #assert same_point(smb_multiply_web(k), got);
  public := ecmult_vartime(k, sg_infinity(), sc_small(0));
  #assert public.ok && same_point(public.value, got);
}
// Repeated addition uses only the group law of chapter 15.
times :: (k: int, p: SgPoint) -> SgPoint {
  r := sg_infinity();
  for i: 0..<k { r = sg_add(r, p).value; }
  return r;
}
// Small multiples of a peer P = 7G.
#forall "k·P matches repeated addition" k: 0..<6 #steps 1000000000 {
  p := times(7, sg_generator());
  want := times(k, p);
  ladder := smc_multiply(sc_small(cast(u32) k), p);
  #assert ladder.ok && same_point(ladder.value, want);
  public := ecmult_vartime(sc_small(0), p, sc_small(cast(u32) k));
  #assert public.ok && same_point(public.value, want);
}
// a·G + b·P with P = 5G equals (a + 5b)G.
#forall "the dual sum adds both terms" a: 0..<6, b: 0..<6 #steps 1000000000 {
  g := sg_generator();
  got := ecmult_vartime(sc_small(cast(u32) a), times(5, g), sc_small(cast(u32) b));
  #assert got.ok && same_point(got.value, times(a + 5 * b, g));
}
// One-hot scalars 2^bit against bit doublings of G. The bits sit at the
// first and last rows of the native comb's teeth (32 apart) and the web
// comb's teeth (37 apart).
BITS :: [10] int.[0, 31, 32, 36, 37, 63, 111, 222, 224, 255];
#forall "2^bit G matches bit doublings" i: 0..<10 #steps 1000000000 {
  bit := BITS[i];
  want := sg_generator();
  for j: 0..<bit { want = sg_double(want).value; }
  k: ScScalar; k.limbs[bit / 32] = cast(u32) 1 << (bit % 32);
  #assert same_point(smb_multiply_native(k), want);
  #assert same_point(smb_multiply_web(k), want);
}
```

To run them:

```shell
coax/bin/coax test bitcoin/examples/checks/scalar-mult.coax --run-steps 250000000
coax/bin/coax mutate bitcoin/examples/checks/scalar-mult.coax --root-only --run-steps 250000000
```

`coax test` runs 130 checks, including the loaded files' own checks, in
about 90 seconds.

### libsecp256k1

[secp_mult.py](../../coax/tests/nostr/secp_mult.py),
[secp_base.py](../../coax/tests/nostr/secp_base.py) and
[secp_mult_const.py](../../coax/tests/nostr/secp_mult_const.py) compile each
path to C and wasm and compare it with libsecp256k1 `v0.8.0` on 100,000
random scalars and the edge cases (0, 1, `n - 1`, `n`, powers of two),
and the full comb tables entry by entry. They need the oracle built as
chapter 13 describes. On 2026-11-01, with the oracle built from the pinned
archive, `secp_mult.py` passed its 6 tests in about 10 minutes and
`secp_base.py` its 5 tests in about 5 minutes, on a loaded machine. The
records are
[coax-secp-mult-verify-2026-10-28.md](../../docs/verification/coax-secp-mult-verify-2026-10-28.md),
[2026-10-29-secp-fixed-base.md](../../docs/verification/2026-10-29-secp-fixed-base.md)
and [2026-10-29-secp-variable-base.md](../../docs/verification/2026-10-29-secp-variable-base.md).

### Timing

[secp_base_timing.py](../../coax/tests/nostr/secp_base_timing.py) and
[secp_mult_const_timing.py](../../coax/tests/nostr/secp_mult_const_timing.py)
run the Welch t-test of chapter 3 on 1,000,000 calls each: a zero scalar
against random scalars, with a fixed-work control and a planted branch.
`|t| ≥ 10` is a detected difference. Results on 2026-11-01:

| Fixture | t | Result |
| --- | ---: | --- |
| Native comb | 0.06 | no difference detected |
| Web comb | 0.65 | no difference detected |
| Ladder | 0.76 | no difference detected |
| Fixed-work control (comb run) | −1.17 | no difference detected |
| Planted branch (comb run) | −560.45 | detected |
| Fixed-work control (ladder run) | 0.25 | no difference detected |
| Planted branch (ladder run) | −37.78 | detected |

Both planted leaks are detected. A result below the threshold means only
that this experiment found no difference; it is not a proof of constant
time.

## Summary

- `ecmult_vartime` computes `a·G + b·P` with signed windows for public
  verification.
- `smb_multiply_native` and `smb_multiply_web` compute `k·G` with a comb
  that reads every table entry.
- `smc_multiply` computes `k·P` with 65 signed radix-16 digits and the same
  work for every scalar.
- Each route gives the independent point for random and edge scalars, and
  the timing harness detects its planted leaks.

## Dependencies

- [14. Scalars mod n](ch014-scalars-mod-n.md)
- [15. The secp256k1 group law](ch015-the-secp256k1-group-law.md)
- [3. Constant-time rules and secrets](ch003-constant-time-rules.md)

## Sources

- [BIP-340](https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki)
  and its pinned
  [test-vectors.csv](../../coax/tests/vectors/upstream/bip340/test-vectors.csv).
- [libsecp256k1](https://github.com/bitcoin-core/secp256k1), release
  `v0.8.0`, pinned in
  [UPSTREAM.json](../../tools/coax-nostr/fuzz/UPSTREAM.json).
