# 66. ElligatorSwift

BIP-324's v2 transport (chapter 67) starts with each peer sending a public
key in the clear. If that key were a 33-byte compressed key, anyone
watching could tell: about half of all 32-byte strings are not the x
coordinate of any point, so after a handful of connections a string that
was always a valid x gives the protocol away. ElligatorSwift encodes a
point as 64 bytes instead. Every 64-byte string decodes to a point, and
an encoder that draws at random produces strings that look uniformly
random.

This chapter builds BIP-324's ElligatorSwift: the decoder `XSwiftEC`, its
inverse `XSwiftECInv` with eight cases, an encoder, and the x-only ECDH
whose hash is the v2 transport's shared secret.

## What this chapter teaches

- Why a plain x coordinate is distinguishable from random bytes, and how
  64 bytes fix it.
- `XSwiftEC`: how any pair of field elements `(u, t)` maps to a valid x,
  through three candidates of which one or all three are on the curve.
- `XSwiftECInv`: how to find the `t` for a given `x` and `u`, and what the
  eight cases select.
- Why BIP-324 fixes one particular square root, and why its sign matters.
- The encoder's loop, and what it reveals.
- BIP-324's shared secret: x-only ECDH hashed with both encodings.

## Where Bitcoin uses ElligatorSwift

| Where | What it uses | Chapter |
| --- | --- | --- |
| The v2 transport | `ellswift_xdh` for the shared secret, the encodings as the first 64 bytes each side sends | 67 |
| The handshake | `ellswift_create` for each connection's ephemeral key | 68 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [ellswift.coax](../../coax/lib/bitcoin/ellswift.coax) | `ellswift_xswiftec(u, t)` | `XSwiftEC`: a pair of field elements to a valid x |
| | `ellswift_decode(ell, out)` | 64 bytes to a 32-byte x |
| | `ellswift_xswiftec_inv(x, u, case)` | `XSwiftECInv`: a `t`, or none |
| | `ellswift_encode(x, rnd, out)` | an x to 64 bytes, drawn from 32 random bytes |
| | `ellswift_create(secret, rnd, out, work)` | a private key's encoding |
| | `ellswift_xdh(ours, theirs, secret, initiating, out, work)` | BIP-324's shared secret |
| | `es_sqrt`, `es_valid_x` | BIP-324's square root, and the curve test |
| [secp_field.coax](../../coax/lib/crypto/secp_field.coax) | `sf_multiply`, `sf_inverse`, `sf_chain_prefix` | field arithmetic mod p (chapter 13) |
| [ecdh.coax](../../coax/lib/crypto/ecdh.coax) | `ecdh_stage` | x-only ECDH with a secret scalar (Nostr chapter 14, on chapter 16's ladder) |
| [sha256.coax](../../coax/lib/crypto/sha256.coax) | `sha256_tagged_checked` | the tagged hash (chapter 5) |

The encoding is sent in the clear, so BIP-324 treats encoding and
decoding as public work. The secrets are the private key and the shared
x. Both go through `ecdh_stage`, whose scalar multiplication is chapter
16's constant-time ladder. The field steps in this file are branch-free
anyway: fixed exponent chains and masked selects, never a branch or an
index on a value.

## Why 64 bytes

A secp256k1 x coordinate is valid when `x^3 + 7` is a square mod p, and
about half of all field elements are squares. A peer that sends its key's
x as 32 bytes therefore always sends a valid x, while random bytes are
valid only half the time. Twenty connections whose first 32 bytes were all
valid x would happen by chance once in a million.

ElligatorSwift sends two field elements, `u` and `t`, and decodes them to
a valid x however they were chosen. The encoder picks `u` at random and
solves for `t`. Each x has many encodings, and drawing one uniformly
makes the 64 bytes uniform too. The construction comes from the SwiftEC
paper (IACR ePrint 2022/759); BIP-324 instantiates it for secp256k1 with
one change, described below.

## The constants

<!-- source: coax/lib/bitcoin/ellswift.coax constants -->
```jai
// c, the square root of -3 that BIP-324 picks: the one that is itself a
// square, (-3)^((p+1)/4) mod p. Its sign matters: it decides which of two
// candidates xswiftec tries second.
ELLSWIFT_C :: [32] u8.[0x0a,0x2d,0x2b,0xa9,0x35,0x07,0xf1,0xdf,0x23,0x37,0x70,0xc2,
  0xa7,0x97,0x96,0x2c,0xc6,0x1f,0x6d,0x15,0xda,0x14,0xec,0xd4,0x7d,0x8d,0x27,0xae,
  0x1c,0xd5,0xf8,0x52];
// (1 - c)/2 and (1 + c)/2: the two nontrivial cube roots of 1, plus 1 and
// negated. xswiftec_inv multiplies u by them.
ELLSWIFT_A0 :: [32] u8.[0x7a,0xe9,0x6a,0x2b,0x65,0x7c,0x07,0x10,0x6e,0x64,0x47,0x9e,
  0xac,0x34,0x34,0xe9,0x9c,0xf0,0x49,0x75,0x12,0xf5,0x89,0x95,0xc1,0x39,0x6c,0x28,
  0x71,0x95,0x01,0xef];
ELLSWIFT_A1 :: [32] u8.[0x85,0x16,0x95,0xd4,0x9a,0x83,0xf8,0xef,0x91,0x9b,0xb8,0x61,
  0x53,0xcb,0xcb,0x16,0x63,0x0f,0xb6,0x8a,0xed,0x0a,0x76,0x6a,0x3e,0xc6,0x93,0xd6,
  0x8e,0x6a,0xfa,0x41];
// 1/2 mod p, which is (p + 1)/2.
ELLSWIFT_HALF :: [32] u8.[0x7f,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,
  0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,0xff,
  0x7f,0xff,0xfe,0x18];
ELLSWIFT_TAG :: "bip324_ellswift_xonly_ecdh";
```

There are two square roots of -3 mod p, `c` and `-c`. BIP-324 picks the
one that is itself a square. The choice is not cosmetic: the decoder's
second and third candidates swap when `c` changes sign, so a decoder with
the other root returns a different x for some inputs. `(1 - c)/2` and
`(1 + c)/2` are the two cube roots of 1 other than 1, negated; the library's
first test checks that.

## Square roots

BIP-324 defines "the" square root of `a` as `a^((p+1)/4)`. When `a` is a
square, this is one of its two roots, and it is itself a square, because
`(p+1)/4` is even. Chapter 13's `sf_sqrt` computes the same power but
returns the even root, which is the other one half the time. The inverse
below multiplies by roots, so it needs BIP-324's:

<!-- source: coax/lib/bitcoin/ellswift.coax sqrt -->
```jai
// A square root and whether it exists. BIP-324 defines "the" square root
// of a as a^((p+1)/4), which is itself a square. Chapter 13's sf_sqrt
// returns the even root instead, and xswiftec_inv needs BIP-324's sign.
EsRoot :: struct { value: SecpField; ok: u64; }
es_sqrt :: (a: SecpField) -> EsRoot {
  cache := sf_chain_prefix(a);
  root := sf_square_n(sf_multiply(sf_square_n(cache.value[12], 6), cache.value[1]), 2);
  return EsRoot.{value = root, ok = sf_equal_mask(sf_square(root), a)};
}
// x is a valid x coordinate when x^3 + 7 is a square.
es_valid_x :: (x: SecpField) -> u64 {
  return es_sqrt(sf_add(sf_multiply(sf_square(x), x), es_small(7))).ok;
}
```

`es_sqrt` reuses chapter 13's exponent chain, `sf_chain_prefix`, and adds
the last steps itself. It returns a mask, not a `bool`, so callers can
combine results without branching.

## Decoding

`XSwiftEC(u, t)` computes

```text
X = (u^3 + 7 - t^2) / (2t)
Y = (X + t) / (c u)
x1 = u + 4 Y^2
x2 = (-X/Y - u) / 2
x3 = ( X/Y - u) / 2
```

and returns the first of `x1`, `x2`, `x3` that is a valid x. BIP-324
states that at least one always is. In fact the count is one or three,
never two: 3,000 random pairs run through BIP-324's `reference.py` give
2,230 with one valid candidate, 770 with three and none with two. The
inverse below relies on it.

The paper's map sends about 3 in 2^256 inputs to the point at infinity.
BIP-324 changes those inputs first, so that no peer can craft one: `u = 0`
becomes 1, `t = 0` becomes 1, and when `u^3 + t^2 + 7 = 0`, `t` doubles.

<!-- source: coax/lib/bitcoin/ellswift.coax decode -->
```jai
// xswiftec: any (u, t) gives a valid x. One of three candidates is always
// on the curve; the first valid one wins.
ellswift_xswiftec :: (u_in: SecpField, t_in: SecpField) -> SecpField {
  one := es_small(1); seven := es_small(7);
  u := es_choose(es_is_zero(u_in), one, u_in);
  t := es_choose(es_is_zero(t_in), one, t_in);
  g := sf_add(sf_multiply(sf_square(u), u), seven);       // u^3 + 7
  t = es_choose(es_is_zero(sf_add(g, sf_square(t))), sf_add(t, t), t);
  c := es_const(ELLSWIFT_C); half := es_const(ELLSWIFT_HALF);
  two_t := sf_add(t, t); cu := sf_multiply(c, u);
  // One inversion for both denominators: 1/(2t) = cu/(2t cu), and so on.
  both := sf_inverse(sf_multiply(two_t, cu)).value;
  X := sf_multiply(sf_subtract(g, sf_square(t)), sf_multiply(both, cu));
  Y := sf_multiply(sf_add(X, t), sf_multiply(both, two_t));
  x1 := sf_add(u, sf_multiply(es_small(4), sf_square(Y)));
  q := sf_multiply(X, sf_inverse(Y).value);                // X/Y; 0 when Y is 0
  x2 := sf_multiply(sf_subtract(sf_negate(q), u), half);
  x3 := sf_multiply(sf_subtract(q, u), half);
  return es_choose(es_valid_x(x1), x1, es_choose(es_valid_x(x2), x2, x3));
}
// Decode 64 bytes, u then t, each read big-endian modulo p, to a 32-byte x.
ellswift_decode :: (ell: [] u8, out: [] u8) -> bool {
  if ell.count != 64 || out.count != 32 { return false; }
  u := sf_load(ell[0..<32]).value; t := sf_load(ell[32..<64]).value;
  return sf_store(ellswift_xswiftec(u, t), out);
}
```

`u` and `t` are read big-endian modulo p, so values at or above p are
allowed. Fifty-four of BIP-324's 76 decoding vectors have such a `u` or
`t`. One inversion serves both denominators: with `d = 1/(2t · cu)`,
`1/(2t) = cu · d` and `1/(cu) = 2t · d`. When `Y` is 0, `sf_inverse`
returns 0, and so does the reference implementation's division, so `x2`
and `x3` agree with it there too.

## Inversion

To encode an x, the encoder needs a `t` with `XSwiftEC(u, t) = x` for its
chosen `u`. `XSwiftECInv(x, u, case)` finds one, or reports that the case
has none. The case is 0 to 7:

- Bit 1 set (cases 2, 3, 6, 7): `x` is to be the first candidate, `x1`.
- Bit 1 clear (cases 0, 1, 4, 5): `x` is to be `x2` or `x3`. Since
  `x2 + x3 = -u`, the other one of the pair is `-x - u`. If that one were
  also valid, all three would be (one or three, never two), and the decoder
  would return `x1` instead. So the case refuses when `-x - u` is valid.
- Bits 0 and 2 choose among the up to four `t` that give that candidate:
  which cube root of 1 the formula uses, and the sign of the result.

Each `(u, x)` has 0, 4 or 8 preimages among the eight cases. The
inverse's refusals are: `-x - u` valid, `s` not a square, `s = 0`, `r`
not a square, and `r = 0` in an odd case, where it would repeat the even
case's `t`.

<!-- source: coax/lib/bitcoin/ellswift.coax inverse -->
```jai
// xswiftec_inv: a t with xswiftec(u, t) = x, or ok = 0. Bit 1 of the case
// picks which candidate of xswiftec produces x; bits 0 and 2 pick the
// signs. Both halves are computed and the case selects with masks.
EsT :: struct { t: SecpField; ok: u64; }
ellswift_xswiftec_inv :: (x: SecpField, u: SecpField, which: int) -> EsT {
  bit0 := cb_mask(cast(u64) which & 1); bit1 := cb_mask((cast(u64) which >> 1) & 1);
  bit2 := cb_mask((cast(u64) which >> 2) & 1);
  g := sf_add(sf_multiply(sf_square(u), u), es_small(7));  // u^3 + 7
  half := es_const(ELLSWIFT_HALF);
  // Cases 0, 1, 4, 5: x is the third candidate, so -x - u must not be valid.
  bad_a := es_valid_x(sf_subtract(sf_negate(x), u));
  denom := sf_add(sf_add(sf_square(u), sf_multiply(u, x)), sf_square(x));
  s_a := sf_negate(sf_multiply(g, sf_inverse(denom).value));
  // Cases 2, 3, 6, 7: x is the first candidate.
  s_b := sf_subtract(x, u);
  zero_s := es_is_zero(s_b);
  three_su2 := sf_multiply(es_small(3), sf_multiply(s_b, sf_square(u)));
  r := es_sqrt(sf_negate(sf_multiply(s_b, sf_add(sf_multiply(es_small(4), g), three_su2))));
  bad_b := zero_s | (CB_MAX64 ^ r.ok) | (bit0 & es_is_zero(r.value));
  v_b := sf_multiply(sf_add(sf_negate(u), sf_multiply(r.value, sf_inverse(s_b).value)), half);
  s := es_choose(bit1, s_b, s_a);
  v := es_choose(bit1, v_b, x);
  w := es_sqrt(s);
  a := es_choose(bit0, es_const(ELLSWIFT_A1), es_const(ELLSWIFT_A0));
  t := sf_multiply(w.value, sf_add(sf_multiply(u, a), v));
  // Cases 0 and 5 negate; 1 and 4 do not.
  t = es_choose(bit0 ^ bit2 ^ CB_MAX64, sf_negate(t), t);
  bad := es_choose64(bit1, bad_b, bad_a) | (CB_MAX64 ^ w.ok);
  return EsT.{t = t, ok = bad ^ CB_MAX64};
}
es_choose64 :: (mask: u64, a: u64, b: u64) -> u64 { return cb_ct_select64(mask, a, b); }
```

The procedure computes both halves for every case, and selects with masks.
It costs the same field operations whichever case it is given, and
whether or not the case succeeds.

## Encoding

BIP-324's `XElligatorSwift` loops: draw a random `u` in 1 to p-1 and a
random case, and return `(u, t)` when the case has a preimage. A random
`(u, case)` has a preimage about a quarter of the time, so the loop runs
about four times.

The encoder here takes its randomness from the caller as 32 bytes, and
draws attempt `i`'s `u` and case from two SHA-256 hashes. The same bytes
always give the same encoding, which makes the encoder testable; a caller
that passes fresh random bytes gets a uniform encoding.

<!-- source: coax/lib/bitcoin/ellswift.coax encode -->
```jai
// Encode x as 64 bytes from 32 random bytes. Attempt i draws u from
// SHA-256(rnd || i || 0) and the case from SHA-256(rnd || i || 1), until
// the case has a preimage. Each attempt succeeds with probability about
// 1/4, so the loop bound is never reached in practice.
ELLSWIFT_ATTEMPTS :: 1024;
ellswift_encode :: (x_bytes: [] u8, rnd: [] u8, out: [] u8) -> bool {
  if x_bytes.count != 32 || rnd.count != 32 || out.count != 64 { return false; }
  x := sf_load(x_bytes).value;
  if es_valid_x(x) != CB_MAX64 { return false; }  // x is public; refuse a bad one
  seed: [37] u8; for i: 0..<32 { seed[i] = rnd[i]; }
  draw: [32] u8; pick: [32] u8; ok := false;
  for attempt: 0..<ELLSWIFT_ATTEMPTS {
    stored := cb_store_le32(seed[..], 32, cast(u32) attempt);
    seed[36] = 0; #assert sha256_hash_checked(seed[..], draw[..], false) == SHA_OK;
    seed[36] = 1; #assert sha256_hash_checked(seed[..], pick[..], false) == SHA_OK;
    u := sf_load(draw[..]).value;
    result := ellswift_xswiftec_inv(x, u, cast(int) (pick[0] & 7));
    // u = 0 decodes as 1, so it would not round-trip: draw again.
    if result.ok == CB_MAX64 && es_is_zero(u) == 0 {
      #assert sf_store(u, out[0..<32]) && sf_store(result.t, out[32..<64]);
      ok = true; break;
    }
  }
  for i: 0..<37 { seed[i] = 0; } for i: 0..<32 { draw[i] = 0; pick[i] = 0; }
  return ok;
}
// A key's encoding: x(secret * G), encoded with rnd. EC_OK or ecdh_stage's
// refusal; the caller wipes work, which holds the key's x.
ellswift_create :: (secret: [] u8, rnd: [] u8, out: [] u8, work: [] EcdhScratch) -> u32 {
  if out.count != 64 || rnd.count != 32 { return EC_LENGTH; }
  generator_x := ECDH_TEST_PEER;
  code := ecdh_stage(secret, generator_x[..], work);
  if code != EC_OK { return code; }
  if !ellswift_encode(work[0].staged[..], rnd, out) { return EC_PEER; }
  return EC_OK;
}
```

The number of attempts depends on the random bytes. It reveals only how
many draws failed, and those draws are thrown away; libsecp256k1's
encoder has the same property. A `u` of 0 is drawn again, because the
decoder would read it as 1.

## The shared secret

Each side decodes the other's 64 bytes to an x, lifts it to a point and
multiplies by its own private key. The x of the product is the same on
both sides. BIP-324 does not use it directly. It hashes it with both
encodings, the initiator's first:

```text
shared secret = SHA256_tagged("bip324_ellswift_xonly_ecdh",
                              ell_initiator || ell_responder || x)
```

Hashing the encodings binds the secret to the exact bytes on the wire. A
man in the middle who re-encodes a key, which decodes to the same x, still
gets a different secret.

<!-- source: coax/lib/bitcoin/ellswift.coax xdh -->
```jai
// BIP-324's shared secret: x(secret * decode(theirs)), hashed with both
// encodings in the order initiator, responder, under the tag
// "bip324_ellswift_xonly_ecdh". The caller wipes work.
ellswift_xdh :: (ours: [] u8, theirs: [] u8, secret: [] u8, initiating: bool, out: [] u8, work: [] EcdhScratch) -> u32 {
  if ours.count != 64 || theirs.count != 64 || out.count != 32 { return EC_LENGTH; }
  their_x: [32] u8;
  if !ellswift_decode(theirs, their_x[..]) { return EC_LENGTH; }
  code := ecdh_stage(secret, their_x[..], work);
  if code != EC_OK { return code; }
  input: [160] u8;
  first := ifx initiating then ours else theirs;
  second := ifx initiating then theirs else ours;
  for i: 0..<64 { input[i] = first[i]; input[64 + i] = second[i]; }
  for i: 0..<32 { input[128 + i] = work[0].staged[i]; }
  hashed := sha256_tagged_checked(ELLSWIFT_TAG, input[..], out, false);
  for i: 0..<160 { input[i] = 0; }
  if hashed != SHA_OK { return EC_METADATA; }
  return EC_OK;
}
```

Whether `lift_x` picks a point or its negation does not matter: negation
does not change the x of the product. `ecdh_stage` leaves the shared x in
its scratch, and the caller wipes it.

## The example

The example is [bitcoin/examples/ellswift.coax](../examples/ellswift.coax).
It decodes all 76 of BIP-324's decoding vectors, runs all eight cases of
all 32 inversion vectors, checks the key exchange of BIP-324's seven
packet vectors, and then encodes keys 1 and 2 and lets them agree on a
secret.

<!-- source: bitcoin/examples/ellswift.coax example -->
```jai
#load "../../coax/lib/bitcoin/ellswift.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Bytes as lowercase hex, at most 256 of them.
show :: (b: [] u8) {
  text: [512] u8;
  r := hex_encode_checked(cast(string) b, text[..], false);
  print("%", cast(string) text[0..<r.written]);
}
show_field :: (a: SecpField) { b: [32] u8; #assert sf_store(a, b[..]); show(b[..]); }

main :: () {
  // ---- decoding: every published encoding gives its x
  good := 0;
  for row: ELLSWIFT_DECODE {
    ell: [64] u8; x: [32] u8;
    if ellswift_decode(es_unhex(row.ellswift, ell[..]), x[..]) && es_hex_is(x[..], row.x) { good += 1; }
  }
  print("decode: % of % BIP-324 encodings give their x\n", good, ELLSWIFT_DECODE.count);
  for k: [3] int.[0, 1, 75] {
    row := ELLSWIFT_DECODE[k]; ell: [64] u8; x: [32] u8;
    ok := ellswift_decode(es_unhex(row.ellswift, ell[..]), x[..]);
    print("  row %: u ", k); show(ell[0..<4]); print("..., t "); show(ell[32..<36]);
    print("... -> x "); show(x[..]); print("\n");
  }

  // ---- inversion: every case of every published (u, x)
  found: [8] int; refused: [8] int; agree := 0; back := 0;
  for row: ELLSWIFT_INVERSE {
    x := es_field(row.x); u := es_field(row.u);
    for which: 0..<8 {
      got := ellswift_xswiftec_inv(x, u, which);
      want := row.t[which];
      if got.ok == CB_MAX64 {
        found[which] += 1;
        b: [32] u8; stored := sf_store(got.t, b[..]);
        if es_hex_is(b[..], want) { agree += 1; }
        if sf_equal_mask(ellswift_xswiftec(u, got.t), x) == CB_MAX64 { back += 1; }
      } else {
        refused[which] += 1;
        if want.count == 0 { agree += 1; }
      }
    }
  }
  print("inverse: % rows, % cases; % agree with BIP-324, and all % t decode back to x\n",
    ELLSWIFT_INVERSE.count, ELLSWIFT_INVERSE.count * 8, agree, back);
  for which: 0..<8 { print("  case %: % found, % refused\n", which, found[which], refused[which]); }
  row := ELLSWIFT_INVERSE[0]; x := es_field(row.x); u := es_field(row.u);
  print("  row 0, x "); show_field(x); print("\n");
  for which: 0..<8 {
    got := ellswift_xswiftec_inv(x, u, which);
    print("    case %: ", which);
    if got.ok == CB_MAX64 { show_field(got.t); } else { print("none"); }
    print("\n");
  }

  // ---- the key exchange of BIP-324's packet vectors
  match := 0;
  for r: ELLSWIFT_XDH {
    ours: [64] u8; theirs: [64] u8; key: [32] u8; out: [32] u8; xo: [32] u8; xt: [32] u8;
    work: [1] EcdhScratch;
    code := ellswift_xdh(es_unhex(r.ours, ours[..]), es_unhex(r.theirs, theirs[..]),
      es_unhex(r.secret, key[..]), r.initiating, out[..], work[..]);
    shared_ok := es_hex_is(work[0].staged[..], r.x_shared);
    work[0] = EcdhScratch.{};
    if code == EC_OK && shared_ok && es_hex_is(out[..], r.shared_secret)
      && ellswift_decode(ours[..], xo[..]) && es_hex_is(xo[..], r.x_ours)
      && ellswift_decode(theirs[..], xt[..]) && es_hex_is(xt[..], r.x_theirs) { match += 1; }
  }
  print("exchange: % of % rows give their x, shared x and shared secret\n", match, ELLSWIFT_XDH.count);

  // ---- two keys of our own: encode, decode, agree
  a: [32] u8; a[31] = 1; b: [32] u8; b[31] = 2;
  ra: [32] u8; rb: [32] u8; for i: 0..<32 { ra[i] = cast(u8) i; rb[i] = cast(u8) (32 + i); }
  ea: [64] u8; eb: [64] u8; work: [1] EcdhScratch; x1: [32] u8;
  ca := ellswift_create(a[..], ra[..], ea[..], work[..]); work[0] = EcdhScratch.{};
  cb := ellswift_create(b[..], rb[..], eb[..], work[..]); work[0] = EcdhScratch.{};
  print("initiator, key 1: "); show(ea[..]); print("\n");
  dec := ellswift_decode(ea[..], x1[..]); print("  decodes to "); show(x1[..]); print(" (the generator's x)\n");
  print("responder, key 2: "); show(eb[..]); print("\n");
  dec = ellswift_decode(eb[..], x1[..]); print("  decodes to "); show(x1[..]); print("\n");
  sa: [32] u8; sb: [32] u8;
  codea := ellswift_xdh(ea[..], eb[..], a[..], true, sa[..], work[..]); work[0] = EcdhScratch.{};
  codeb := ellswift_xdh(eb[..], ea[..], b[..], false, sb[..], work[..]); work[0] = EcdhScratch.{};
  print("initiator's secret: "); show(sa[..]); print("\n");
  print("responder's secret: "); show(sb[..]); print("\n");
  same := true; for i: 0..<32 { if sa[i] != sb[i] { same = false; } }
  print("  the same: %\n", same);
  sc: [32] u8;
  codec := ellswift_xdh(ea[..], eb[..], a[..], false, sc[..], work[..]); work[0] = EcdhScratch.{};
  print("  with the roles swapped on one side: "); show(sc[..]); print("\n");
}
```

Run it from the repository root:

```shell
coax/bin/coax run bitcoin/examples/ellswift.coax
```

The output:

```text
decode: 76 of 76 BIP-324 encodings give their x
  row 0: u 00000000..., t 00000000... -> x edd1fd3e327ce90cc7a3542614289aee9682003e9cf7dcc9cf2ca9743be5aa0c
  row 1: u 00000000..., t 01d3475b... -> x b5da00b73cd6560520e7c364086e7cd23a34bf60d0e707be9fc34d4cd5fdfa2c
  row 75: u ffffffff..., t ffffffff... -> x 1c92ccdfcf4ac550c28db57cff0c8515cb26936c786584a70114008d6c33a34b
inverse: 32 rows, 256 cases; 256 agree with BIP-324, and all 98 t decode back to x
  case 0: 12 found, 20 refused
  case 1: 12 found, 20 refused
  case 2: 14 found, 18 refused
  case 3: 11 found, 21 refused
  case 4: 12 found, 20 refused
  case 5: 12 found, 20 refused
  case 6: 14 found, 18 refused
  case 7: 11 found, 21 refused
  row 0, x 80cdf63774ec7022c89a5a8558e373a279170285e0ab27412dbce510bdfe23fc
    case 0: none
    case 1: none
    case 2: 45654798ece071ba79286d04f7f3eb1c3f1d17dd883610f2ad2efd82a287466b
    case 3: 0aeaa886f6b76c7158452418cbf5033adc5747e9e9b5d3b2303db96936528557
    case 4: none
    case 5: none
    case 6: ba9ab867131f8e4586d792fb080c14e3c0e2e82277c9ef0d52d1027c5d78b5c4
    case 7: f51557790948938ea7badbe7340afcc523a8b816164a2c4dcfc24695c9ad76d8
exchange: 7 of 7 rows give their x, shared x and shared secret
initiator, key 1: c865403d1346a2ed5c486faa0a27b62823add9a39473e21da9b59ba761806ea873d0c663605b826f26bc67ef4e25f381ea98428f787eb0a6f9659cab583ac70b
  decodes to 79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798 (the generator's x)
responder, key 2: 1a1c89a939043a07b4c54d717e77b4d293d4665cd18c8428d2f3ae2ab12a47ae5c698447e3cc3f40fa58ec5c59e2fda05fa70ab17a0610321c6517708724e436
  decodes to c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5
initiator's secret: 6726a6e6c976ec69f5150e4ab5a5de18bda6310f72efdf55fa8de94790e0c9b6
responder's secret: 6726a6e6c976ec69f5150e4ab5a5de18bda6310f72efdf55fa8de94790e0c9b6
  the same: true
  with the roles swapped on one side: 822fc24e78f04df6eb98bedd4b650add080a5a88f2381adfac9449ee17762d4c
```

What the output shows:

- Every decoding vector gives its x, including row 0, where `u` and `t`
  are both 0, and row 75, where both are at or above p.
- All 256 inversion cases agree with BIP-324: 98 give BIP-324's `t` and
  158 refuse where BIP-324 lists none. Every `t` found decodes back to its
  x.
- Row 0 shows a `(u, x)` with four preimages: cases 0, 1, 4 and 5 refuse,
  because `-x - u` is a valid x.
- Every packet vector's two encodings decode to the listed x, and the
  private key gives the listed shared x and shared secret.
- Key 1's encoding decodes to the generator's x, and key 2's to the x of
  `2G`. The two sides reach the same secret. A side that gets the roles
  wrong hashes the encodings in the other order and gets a different one.

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/ellswift.out](../fixtures/ellswift.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/ellswift.coax](../examples/checks/ellswift.coax),
which loads the example. The example already checks every published
vector; the checks pick out the rows that reach each special path:

- decoding rows with `u = t = 0`, with `u^3 - t^2 + 7 = 0`, with
  `u^3 + t^2 + 7 = 0` (where `t` doubles), with each of the three
  candidates winning, and with `u` and `t` at or above p
- the inversion rows where `r = 0` refuses the odd cases and where `s = 0`
  refuses
- a published encoding with one byte changed still decodes to a valid x
- encodings of the generator from two random strings decode to it
- keys 1 and 2 reach the same secret, and a swapped role does not

<!-- source: bitcoin/examples/checks/ellswift.coax check -->
```jai
// Decoding rows that reach each path: u = t = 0, u^3 - t^2 + 7 = 0,
// u^3 + t^2 + 7 = 0 (where t doubles), each of the three candidates, and
// u and t at or above p.
DECODE_PICKS :: [7] int.[0, 4, 11, 13, 30, 31, 75];
#forall "BIP-324 decode rows on each path" k: 0..<7 #steps 60000000 {
  row := ELLSWIFT_DECODE[DECODE_PICKS[k]]; ell: [64] u8; x: [32] u8;
  #assert ellswift_decode(es_unhex(row.ellswift, ell[..]), x[..]) && es_hex_is(x[..], row.x);
}
// Inverse rows with the rarer refusals: r = 0 in the odd cases of row 13,
// and s = 0 in row 26.
INVERSE_PICKS :: [2] int.[13, 26];
#forall "BIP-324 inverse rows with r = 0 and s = 0" k: 0..<2 #steps 400000000 {
  row := ELLSWIFT_INVERSE[INVERSE_PICKS[k]]; x := es_field(row.x); u := es_field(row.u);
  for which: 0..<8 {
    got := ellswift_xswiftec_inv(x, u, which);
    #assert (got.ok == CB_MAX64) == (row.t[which].count > 0);
    if got.ok == CB_MAX64 { b: [32] u8; #assert sf_store(got.t, b[..]) && es_hex_is(b[..], row.t[which]); }
  }
}
// Every 64 bytes are some key: a published encoding with any one byte
// changed still decodes to a valid x.
#forall "a changed encoding still decodes to a point" k: 0..<4 #steps 60000000 {
  ell: [64] u8; x: [32] u8;
  e := es_unhex(ELLSWIFT_DECODE[30].ellswift, ell[..]); ell[k * 21] ^= 0x5a;
  #assert ellswift_decode(ell[..], x[..]) && es_valid_x(sf_load(x[..]).value) == CB_MAX64;
}
// The encoder's output decodes to its input, for two random strings.
#forall "encodings of the generator decode to it" k: 0..<2 #steps 400000000 {
  gx := ECDH_TEST_PEER; rnd: [32] u8; for i: 0..<32 { rnd[i] = cast(u8) (k * 101 + i); }
  ell: [64] u8; x: [32] u8;
  #assert ellswift_encode(gx[..], rnd[..], ell[..]) && ellswift_decode(ell[..], x[..]);
  for i: 0..<32 { #assert x[i] == gx[i]; }
}
// Two parties with keys 1 and 2 and the example's encodings reach the
// same secret, and only when they agree on who initiated.
EA :: "c865403d1346a2ed5c486faa0a27b62823add9a39473e21da9b59ba761806ea873d0c663605b826f26bc67ef4e25f381ea98428f787eb0a6f9659cab583ac70b";
EB :: "1a1c89a939043a07b4c54d717e77b4d293d4665cd18c8428d2f3ae2ab12a47ae5c698447e3cc3f40fa58ec5c59e2fda05fa70ab17a0610321c6517708724e436";
#test "both sides reach the same secret" #steps 400000000 {
  ea: [64] u8; eb: [64] u8; a: [32] u8; a[31] = 1; b: [32] u8; b[31] = 2;
  sa: [32] u8; sb: [32] u8; sc: [32] u8; work: [1] EcdhScratch;
  pa := es_unhex(EA, ea[..]); pb := es_unhex(EB, eb[..]);
  #assert ellswift_xdh(ea[..], eb[..], a[..], true, sa[..], work[..]) == EC_OK;
  #assert ellswift_xdh(eb[..], ea[..], b[..], false, sb[..], work[..]) == EC_OK;
  #assert ellswift_xdh(eb[..], ea[..], b[..], true, sc[..], work[..]) == EC_OK;
  work[0] = EcdhScratch.{};
  #assert es_hex_is(sa[..], "6726a6e6c976ec69f5150e4ab5a5de18bda6310f72efdf55fa8de94790e0c9b6");
  same := true; differs := false;
  for i: 0..<32 { if sa[i] != sb[i] { same = false; } if sa[i] != sc[i] { differs = true; } }
  #assert same && differs;
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/ellswift.coax
```

`coax test` runs 161 checks, including the loaded files' own checks, in
about 50 seconds on 2026-11-03. The field chains are slow in the
evaluator, which is why the checks pick rows and the compiled example runs
them all.

### The library's own checks

`ellswift.coax` is an `#authority` file. Its checks cover the constants
(`c^2 = -3`, `c` is BIP-324's root, the half, and the cube roots of 1),
`es_sqrt` on a non-square and on zero, the first decoding vectors, the
decoder's length refusals, every case of the first inversion vector with
each `t` decoding back, a packet vector's key exchange, the exchange's
refusals (wrong lengths, a zero key), and an encoding that decodes to its
key's x.

```shell
coax/bin/coax test coax/lib/bitcoin/ellswift.coax
```

`coax test` runs 153 checks, including those of the files it loads, in
about 45 seconds on 2026-11-03. Mutation testing: not run (owner speed
order).

### BIP-324's reference implementation

[ellswift.py](../../coax/tests/bitcoin/ellswift.py) computes every line
the example prints with BIP-324's own `reference.py`, pinned, which shares
no code with Coax: its `xswiftec`, `xswiftec_inv`, `ellswift_ecdh_xonly`
and `v2_ecdh`. The encoder's draws are this chapter's choice, so the
script transcribes them from the text above. The transcript must equal the
fixture.

```shell
python3 -I coax/tests/bitcoin/ellswift.py --coax coax/bin/coax
```

On 2026-11-03 all 31 lines agree with `reference.py`, and the example's
output in native debug and release builds equals them. Wasm was not run.

[make_ellswift_vectors.py](../../coax/tests/bitcoin/make_ellswift_vectors.py)
reads the three pinned CSV files into
[ellswift_vectors.coax](../../coax/tests/bitcoin/ellswift_vectors.coax).

## What this chapter leaves out

- The keys, ciphers and packets of the v2 transport: chapter 67.
- Choosing between v1 and v2 when a connection opens: chapter 68.
- Fixing part of `u` to make the bytes look like another protocol, which
  BIP-324 mentions as a possibility.
- A production random source for `rnd`: chapter 18 covers keys and
  randomness.

## Summary

- A plain x coordinate is valid only half the time, so a stream of them
  is not random. Every 64-byte ElligatorSwift string decodes to a point.
- `XSwiftEC(u, t)` computes three candidates; one or all three are valid,
  and it returns the first valid one.
- `XSwiftECInv(x, u, case)` finds a `t` for one of eight cases, or none.
  Bit 1 picks the first candidate or the pair; bits 0 and 2 pick among
  the solutions.
- BIP-324 fixes the square root `a^((p+1)/4)` and the root of -3 that is
  a square; other choices decode differently.
- The encoder draws `u` and a case until one has a preimage, about four
  times.
- The shared secret hashes both encodings and the shared x, so it depends
  on the exact bytes each side sent.

## Dependencies

- [5. SHA-256d and tagged hashes](ch005-sha-256d-and-tagged-hashes.md)
- [13. Field arithmetic mod p](ch013-field-arithmetic-mod-p.md)
- [15. The secp256k1 group law](ch015-the-secp256k1-group-law.md)
- [16. Scalar multiplication](ch016-scalar-multiplication.md)
- [17. Point encodings: compressed, uncompressed and x-only](ch017-point-encodings-compressed-uncompressed-and-x-only.md)

## Sources

- [BIP-324](https://github.com/bitcoin/bips/blob/master/bip-0324.mediawiki),
  "ElligatorSwift encoding of curve X coordinates", with its test vectors
  `ellswift_decode_test_vectors.csv`, `xswiftec_inv_test_vectors.csv` and
  `packet_encoding_test_vectors.csv` and its `reference.py`, at bips
  revision `927b6de9`, pinned in
  [coax/tests/vectors/upstream/bip324/](../../coax/tests/vectors/upstream/bip324/bip-0324.mediawiki).
- The SwiftEC paper that BIP-324 cites,
  [IACR ePrint 2022/759](https://eprint.iacr.org/2022/759.pdf).
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
