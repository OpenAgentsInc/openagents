# 11. Verify a Schnorr signature

A Nostr event is valid only if its `sig` is a BIP-340 Schnorr signature of
its ID by its `pubkey`. This chapter builds verification exactly as BIP-340
specifies it, from the field, scalar and point code of chapters 7 to 10, and
checks it against all 19 rows of the official test vectors before any event
code uses it.

## What this chapter teaches

- The algorithm: lift the public key to a point `P`, check `r < p` and
  `s < n`, hash the tagged challenge `e`, compute `R = s·G - e·P`, and
  accept only when `R` is finite, `y(R)` is even and `x(R) = r`.
- Which step refuses each failing row of the vectors: a key that is not on
  the curve, `r` at least `p`, `s` at least `n`, an odd `y(R)`, an infinite
  `R`, and an `x(R)` that differs from `r`.
- Messages of any length, as BIP-340 allows, and the Nostr wrapper that
  takes exactly 32 bytes of event ID.
- A second, faster path that must agree with the first on every input.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [schnorr.coax](../../coax/lib/crypto/schnorr.coax) | `schnorr_verify_vartime(message, public_key, signature)` | the reference verifier, for messages of 0 to 1,048,576 bytes |
| | `nostr_verify_vartime(event_id, public_key, signature)` | the same, for exactly 32 bytes of event ID |
| | `sv_prepare_vartime` | checks lengths, `r`, `s` and the key, and hashes `e` |
| | `sv_prepared_matches_vartime` | computes `R` and makes the final comparison |
| [schnorr_fast.coax](../../coax/lib/crypto/schnorr_fast.coax) | `schnorr_fast_verify_vartime`, `nostr_fast_verify_vartime` | the same checks, with a faster multiplication |
| [schnorr_admission.coax](../../coax/lib/crypto/schnorr_admission.coax) | `sig_admit` | the host-memory admission for signing, used in chapter 13 |

All inputs are raw bytes. The public key is 32 bytes of x coordinate, the
signature is 64 bytes, `r` then `s`. Decoding the hex of an event is the
caller's job (chapters 1 and 21). Verification uses only public data, so
every procedure runs in variable time and its name ends in `_vartime`
(chapter 3).

## The algorithm

BIP-340 verification of a message `m` under the public key `pk` with the
signature `sig = r || s`:

1. **Lengths.** `pk` is 32 bytes, `sig` is 64 bytes, and `m` is at most
   1,048,576 bytes, the limit this library sets. BIP-340 itself allows any
   length.
2. **Lift the key.** `P = lift_x(pk)`: read `x` as an integer, refuse it if
   `x ≥ p`, compute `c = x^3 + 7 mod p`, refuse if `c` has no square root,
   and take the root `y` that is even. `sg_lift_x_public` does this, and
   chapter 7's `sf_sqrt` returns the even root.
3. **Range checks.** Refuse unless `r < p` (`sf_load_strict`) and `s < n`
   (`sc_load`). Neither value is reduced: a signature has one encoding.
4. **Challenge.** `e = int(SHA-256(t || t || r || pk || m)) mod n`, where
   `t = SHA-256("BIP0340/challenge")`. The tag hashed twice separates this
   hash from every other use of SHA-256. `SV_TAG_HASH` is computed at
   compile time by `#run`; `sv_challenge_vartime` streams the five pieces
   through SHA-256 without copying them.
5. **The equation.** `R = s·G - e·P`, which `ecmult_vartime(s, P, -e)` of
   chapter 10 computes as one sum.
6. **The comparison.** Refuse if `R` is infinity, if `y(R)` is odd, or if
   `x(R) ≠ r`. Otherwise accept.

Steps 1 to 4 are `sv_prepare_vartime`, and steps 5 and 6 are
`sv_prepared_matches_vartime`. A prepared record with `ready` true means
only that preparation succeeded. It is not a verified signature, and only
the two complete entry procedures may decide that a signature is valid.

### Why the equation works

A signer with secret key `d` and nonce `k` publishes `r = x(k·G)` and
`s = k + e·d mod n`, with `P = d·G`. Then

```text
s·G - e·P = (k + e·d)·G - e·d·G = k·G
```

so `R` is the signer's nonce point, and its `x` is `r`. The signer chooses
`d` and `k` so that `P` and `k·G` have even `y` (chapter 13); this lets a
32-byte `x` stand for a whole point.

### Worked example: small numbers

Use chapter 10's numbers: the secret key `d = 3`, so `P = 3G`, the nonce
`k = 5`, and pretend the hash gave `e = 2`. Then:

1. `s = k + e·d = 5 + 2·3 = 11`.
2. `R = 11G - 2·(3G) = 5G`.
3. `5G` has an even `y`, and `r = x(5G)`, so the comparison passes.

Two changes fail it:

- With the nonce `k = 6`, `s = 6 + 6 = 12` and `R = 12G - 6G = 6G`. Its
  `x` equals `r = x(6G)`, but `6G` has an odd `y`, so step 6 refuses. A
  real signer negates such a nonce; vector 6 has exactly this `r = x(6G)`
  and is refused for the odd `y`.
- With `r = x(4G)` and `s = 11`, `R` is still `5G`, whose `x` is not `r`.

A real `e` comes from the hash, so these numbers check only steps 5 and 6.
The example runs them through `sv_prepared_matches_vartime` with a
hand-built record.

### Worked example: vector 0

Vector 0 signs the 32 zero bytes with the secret key 3. Its public key is
`x(3G)`, `f9308a01...bce036f9`, and its signature is
`r = e907831f...2dca8215`, `s = 25f66a4a...310536c0`.

1. The lengths are 32, 32 and 64.
2. `lift_x`: the square root of `x^3 + 7` that the exponentiation finds is
   odd, so the code takes `p` minus it: `P.y = 388f7b0f...84b8e672`.
3. `r` is below `p` and `s` is below `n`.
4. `t = 7bb52d7a...6d48d37c`. The hash of `t || t || r || pk || m` is
   `6bb6b93a...bb5d86ce`, which is below `n`, so it is `e` unchanged.
5. `R = s·G - e·P` has `x(R) = e907831f...2dca8215`.
6. `y(R)` is even and `x(R)` equals `r`: the signature is valid.

The intermediate values were computed with Python's `hashlib` and integers,
which share no code with the Coax implementation. The checks compare them
with the code's.

### The 19 vectors

[test-vectors.csv](../../coax/tests/vectors/upstream/bip340/test-vectors.csv)
has 19 rows. Rows 0 to 4 and 15 to 18 are valid, rows 15 to 18 with messages
of 0, 1, 17 and 100 bytes. Each failing row targets one step:

| Rows | Fault | Refused in |
| --- | --- | --- |
| 5 | the key's `x^3 + 7` has no square root | step 2, `lift_x` |
| 14 | the key is `p + 1`, at least `p` | step 2, `lift_x` |
| 12 | `r = p` | step 3 |
| 13 | `s = n` | step 3 |
| 6 | `y(R)` is odd | step 6 |
| 7, 8 | the message or `s` is negated, so `R` is another point | step 6, `x(R) ≠ r` |
| 9, 10 | `R` is infinity, with `r = 0` and `r = 1` | step 6 |
| 11 | `r` is not the `x` of any point | step 6, `x(R) ≠ r` |

Rows 9 and 10 catch a verifier that gives infinity the coordinates
`(0, y)` or `(1, y)` with an even `y`. In this library, infinity carries a
nonzero flag through the affine conversion, and the comparison requires the
flag to be zero (chapter 9). Row 3 has the message `ff...ff`, larger than
`p` and `n`; it catches a verifier that reduces the message.

## Two paths

`schnorr_fast_verify_vartime` runs the same preparation, `sv_prepare_vartime`,
and then a different multiplication: signed width-5 windows for both `G` and
`P`, eight precomputed odd multiples of `G`, and a field type with eight
32-bit limbs and a fused multiply. In the interpreter, the 2026-10-29
record measured vector 0 on the fast path at 18,328,270 steps; the reference
path runs out of a 20 million step budget on the same vector. The two
paths share the admission and the challenge, so they can differ only in the
equation and the comparison, and the checks compare them on every vector
and on fresh signatures.

## The example

The example is [nostr/examples/schnorr-verify.coax](../examples/schnorr-verify.coax).
It walks vector 0 one step at a time, runs the small-number equations, runs
all 19 vectors on both paths and says where each refusal happens, and then
shows six refusals. The vector table is at the end of the file, outside the
listing.

<!-- source: nostr/examples/schnorr-verify.coax example -->
```jai
#load "../../coax/lib/crypto/schnorr.coax";
#load "../../coax/lib/crypto/schnorr_fast.coax";
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
  if sf_store(f, b[..]) { show(label, b[..]); }
}
show_scalar :: (label: string, s: ScScalar) {
  b: [32] u8;
  if sc_store(s, b[..]) { show(label, b[..]); }
}
// Decode hex into out and return the number of bytes written. Every literal
// here is valid hex, so a decoding error stops the program.
decode :: (text: string, out: [] u8) -> int {
  r := hex_decode_checked(text, out, false);
  #assert r.error == 0;
  return r.written;
}
// Row i of the table at the end of this file, decoded into fixed buffers.
Vector :: struct {
  public_key: [32] u8; signature: [64] u8; message: [100] u8;
  length: int; expected: bool; comment: string;
}
vector :: (i: int) -> Vector {
  v: Vector;
  #assert decode(VECTOR_HEX[4 * i], v.public_key[..]) == 32;
  v.length = decode(VECTOR_HEX[4 * i + 1], v.message[..]);
  #assert decode(VECTOR_HEX[4 * i + 2], v.signature[..]) == 64;
  v.expected = VECTOR_HEX[4 * i + 3] == "1";
  v.comment = VECTOR_COMMENT[i];
  return v;
}
// The final equation alone, for small numbers: s, the challenge e, the point
// P and the point whose x is r. It skips the hash, so it shows the algebra;
// it is not a signature verification.
equation :: (s: u32, e: u32, p: SgPoint, r: SgPoint) -> bool {
  prepared: SvPrepared;
  prepared.s = sc_small(s); prepared.negative_e = sc_negate(sc_small(e));
  prepared.point = p; prepared.r = sg_to_affine(r).value.x; prepared.ready = true;
  return sv_prepared_matches_vartime(prepared);
}
// k·G by repeated addition, with chapter 9's group law.
times :: (k: int) -> SgPoint {
  r := sg_infinity();
  for i: 0..<k { r = sg_add(r, sg_generator()).value; }
  return r;
}
even_y :: (p: SgPoint) -> bool { return (sg_to_affine(p).value.y.limb[0] & 1) == 0; }

main :: () {
  // Vector 0, one step at a time.
  v := vector(0);
  m := v.message[0..<v.length]; pk := v.public_key[..]; sig := v.signature[..];
  print("vector 0, step by step\n");
  show("public key", pk);
  lifted := sg_lift_x_public(pk);
  print("lift_x: ok %, y even %\n", lifted.ok, even_y(lifted.value));
  show_field("P.y", sg_to_affine(lifted.value).value.y);
  r := sf_load_strict(sig[0..<32]); s := sc_load(sig[32..<64]);
  print("r < p: %\n", r.ok);
  print("s < n: %\n", s.ok);
  tag: [32] u8; digest := SV_TAG_HASH;
  #assert sha256_emit(digest.words[..], tag[..]) == SHA_OK;
  show("SHA-256(\"BIP0340/challenge\")", tag[..]);
  e := sv_challenge_vartime(m, pk, sig[0..<32]);
  show_scalar("e", e.value);
  big_r := ecmult_vartime(s.value, lifted.value, sc_negate(e.value));
  print("R = s*G - e*P: ok %\n", big_r.ok);
  show_field("x(R)", sg_to_affine(big_r.value).value.x);
  print("y(R) even: %\n", even_y(big_r.value));
  print("x(R) = r: %\n", sf_equal_mask(sg_to_affine(big_r.value).value.x, r.value) == CB_MAX64);
  print("schnorr_verify_vartime: %\n", schnorr_verify_vartime(m, pk, sig));
  print("schnorr_fast_verify_vartime: %\n", schnorr_fast_verify_vartime(m, pk, sig));

  // The same equation with small numbers: d = 3, k = 5, e = 2, s = 11.
  print("small numbers, P = 3G, e = 2\n");
  print("y even: 3G %, 4G %, 5G %, 6G %\n", even_y(times(3)), even_y(times(4)), even_y(times(5)), even_y(times(6)));
  print("s = 11, r = x(5G): %\n", equation(11, 2, times(3), times(5)));
  print("s = 12, r = x(6G): %\n", equation(12, 2, times(3), times(6)));
  print("s = 11, r = x(4G): %\n", equation(11, 2, times(3), times(4)));

  // All 19 rows of test-vectors.csv. A refusal happens either while
  // preparing (lengths, r, s, lift_x) or in the final equation.
  print("all 19 BIP-340 vectors\n");
  agree := 0;
  for i: 0..<VECTORS {
    t := vector(i);
    tm := t.message[0..<t.length];
    reference := schnorr_verify_vartime(tm, t.public_key[..], t.signature[..]);
    fast := schnorr_fast_verify_vartime(tm, t.public_key[..], t.signature[..]);
    stage := "accepted";
    if !reference {
      stage = ifx sv_prepare_vartime(tm, t.public_key[..], t.signature[..]).ready then "refused in the equation" else "refused in preparation";
    }
    print("%: message % bytes, expected %, reference %, fast %, %", i, t.length, t.expected, reference, fast, stage);
    if t.comment.count > 0 { print(": %", t.comment); }
    print("\n");
    if reference == t.expected && fast == t.expected { agree += 1; }
  }
  print("% of % rows give the expected result on both paths\n", agree, VECTORS);

  // Refusals. The Nostr wrapper takes exactly 32 bytes of event ID.
  print("refusals\n");
  id: [33] u8;
  for i: 0..<32 { id[i] = m[i]; }
  print("nostr_verify_vartime, 32-byte ID: %\n", nostr_verify_vartime(id[0..<32], pk, sig));
  print("nostr_verify_vartime, 31-byte ID: %\n", nostr_verify_vartime(id[0..<31], pk, sig));
  print("nostr_verify_vartime, 33-byte ID: %\n", nostr_verify_vartime(id[..], pk, sig));
  print("nostr_fast_verify_vartime, 31-byte ID: %\n", nostr_fast_verify_vartime(id[0..<31], pk, sig));
  print("message of % bytes admitted: %\n", SV_MAX_MESSAGE + 1, sv_lengths_vartime(SV_MAX_MESSAGE + 1, 32, 64));
  t12 := vector(12);
  print("vector 12, r = p: sf_load_strict ok %\n", sf_load_strict(t12.signature[0..<32]).ok);
  t13 := vector(13);
  print("vector 13, s = n: sc_load ok %\n", sc_load(t13.signature[32..<64]).ok);
  t5 := vector(5);
  print("vector 5, key not on the curve: lift_x ok %\n", sg_lift_x_public(t5.public_key[..]).ok);
  t14 := vector(14);
  print("vector 14, key x = p + 1: lift_x ok %\n", sg_lift_x_public(t14.public_key[..]).ok);
  changed := v.message; changed[31] ^= 1;
  print("vector 0, last message bit flipped: %\n", schnorr_verify_vartime(changed[0..<32], pk, sig));
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/schnorr-verify.coax
```

The output shows the values worked out by hand: `P.y`, the tag hash, `e`
and `x(R) = r` for vector 0, and the parities that make `11G - 2(3G) = 5G`
pass and `6G` fail. Every row gives its published result on both paths:

```text
vector 0, step by step
public key: f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9
lift_x: ok true, y even true
P.y: 388f7b0f632de8140fe337e62a37f3566500a99934c2231b6cb9fd7584b8e672
r < p: true
s < n: true
SHA-256("BIP0340/challenge"): 7bb52d7a9fef58323eb1bf7a407db382d2f3f2d81bb1224f49fe518f6d48d37c
e: 6bb6b93a91f2ecc0cd924f4f9baabb5e6eb21745bb00f2cebdaac908bb5d86ce
R = s*G - e*P: ok true
x(R): e907831f80848d1069a5371b402410364bdf1c5f8307b0084c55f1ce2dca8215
y(R) even: true
x(R) = r: true
schnorr_verify_vartime: true
schnorr_fast_verify_vartime: true
small numbers, P = 3G, e = 2
y even: 3G true, 4G true, 5G true, 6G false
s = 11, r = x(5G): true
s = 12, r = x(6G): false
s = 11, r = x(4G): false
all 19 BIP-340 vectors
0: message 32 bytes, expected true, reference true, fast true, accepted
1: message 32 bytes, expected true, reference true, fast true, accepted
2: message 32 bytes, expected true, reference true, fast true, accepted
3: message 32 bytes, expected true, reference true, fast true, accepted: test fails if msg is reduced modulo p or n
4: message 32 bytes, expected true, reference true, fast true, accepted
5: message 32 bytes, expected false, reference false, fast false, refused in preparation: public key not on the curve
6: message 32 bytes, expected false, reference false, fast false, refused in the equation: has_even_y(R) is false
7: message 32 bytes, expected false, reference false, fast false, refused in the equation: negated message
8: message 32 bytes, expected false, reference false, fast false, refused in the equation: negated s value
9: message 32 bytes, expected false, reference false, fast false, refused in the equation: sG - eP is infinite. Test fails in single verification if has_even_y(inf) is defined as true and x(inf) as 0
10: message 32 bytes, expected false, reference false, fast false, refused in the equation: sG - eP is infinite. Test fails in single verification if has_even_y(inf) is defined as true and x(inf) as 1
11: message 32 bytes, expected false, reference false, fast false, refused in the equation: sig[0:32] is not an X coordinate on the curve
12: message 32 bytes, expected false, reference false, fast false, refused in preparation: sig[0:32] is equal to field size
13: message 32 bytes, expected false, reference false, fast false, refused in preparation: sig[32:64] is equal to curve order
14: message 32 bytes, expected false, reference false, fast false, refused in preparation: public key is not a valid X coordinate because it exceeds the field size
15: message 0 bytes, expected true, reference true, fast true, accepted: message of size 0 (added 2022-12)
16: message 1 bytes, expected true, reference true, fast true, accepted: message of size 1 (added 2022-12)
17: message 17 bytes, expected true, reference true, fast true, accepted: message of size 17 (added 2022-12)
18: message 100 bytes, expected true, reference true, fast true, accepted: message of size 100 (added 2022-12)
19 of 19 rows give the expected result on both paths
refusals
nostr_verify_vartime, 32-byte ID: true
nostr_verify_vartime, 31-byte ID: false
nostr_verify_vartime, 33-byte ID: false
nostr_fast_verify_vartime, 31-byte ID: false
message of 1048577 bytes admitted: false
vector 12, r = p: sf_load_strict ok false
vector 13, s = n: sc_load ok false
vector 5, key not on the curve: lift_x ok false
vector 14, key x = p + 1: lift_x ok false
vector 0, last message bit flipped: false
```

The refusals:

- A 31-byte or 33-byte event ID: the Nostr wrappers refuse before reading
  the key or the signature. The same 32 bytes verify.
- A message one byte over the limit: `sv_lengths_vartime` refuses it.
- `r = p` (vector 12): `sf_load_strict` refuses a field element that is not
  below `p`.
- `s = n` (vector 13): `sc_load` refuses a scalar that is not below `n`.
- A key with no point (vector 5) and a key `x = p + 1` (vector 14):
  `sg_lift_x_public` refuses both.
- Vector 0 with one message bit changed: preparation succeeds, `e` changes,
  and the equation gives a different `R`.

The book's checks run this program in debug and release builds and compare
the output with [nostr/fixtures/schnorr-verify.out](../fixtures/schnorr-verify.out).

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/schnorr-verify.coax](../examples/checks/schnorr-verify.coax),
which loads the example. They compare both paths with independent values:

- all 19 rows of the pinned vectors;
- vector 0's tag hash, challenge and `P.y`, from Python's `hashlib` and
  integers;
- eight signatures made by a Python transcription of BIP-340's reference
  signing algorithm, with seeded random keys and messages of 0 to 100
  bytes. Each is accepted, then refused after one one-bit change to the
  message, `r` or `s`, in turn. The Nostr wrappers accept only the two
  32-byte messages. The transcription reproduces the signatures of
  vectors 0 and 16.

<!-- source: nostr/examples/checks/schnorr-verify.coax check -->
```jai
// Every row of the pinned test-vectors.csv gives its published result on
// the reference path and on the fast path.
#forall "both paths give every vector's published result" i: 0..<VECTORS #steps 4000000000 {
  t := vector(i);
  tm := t.message[0..<t.length];
  #assert schnorr_verify_vartime(tm, t.public_key[..], t.signature[..]) == t.expected;
  #assert schnorr_fast_verify_vartime(tm, t.public_key[..], t.signature[..]) == t.expected;
}
equal32 :: (a: [32] u8, b: [32] u8) -> bool {
  for i: 0..<32 { if a[i] != b[i] { return false; } }
  return true;
}
#test "equal32 notices a change in the first or last byte" {
  a: [32] u8; b := a;
  #assert equal32(a, b);
  b[0] = 1; #assert !equal32(a, b);
  b = a; b[31] = 1; #assert !equal32(a, b);
}
// Vector 0's intermediate values, computed with Python's hashlib and
// arbitrary-precision integers. They share no code with the Coax SHA-256,
// field or scalar arithmetic.
#test "vector 0 intermediates match hand-computed values" #steps 1000000000 {
  v := vector(0);
  m := v.message[0..<v.length];
  want: [32] u8; got: [32] u8;
  #assert decode("7bb52d7a9fef58323eb1bf7a407db382d2f3f2d81bb1224f49fe518f6d48d37c", want[..]) == 32;
  digest := SV_TAG_HASH;
  #assert sha256_emit(digest.words[..], got[..]) == SHA_OK && equal32(got, want);
  #assert decode("6bb6b93a91f2ecc0cd924f4f9baabb5e6eb21745bb00f2cebdaac908bb5d86ce", want[..]) == 32;
  e := sv_challenge_vartime(m, v.public_key[..], v.signature[0..<32]);
  #assert e.ok && sc_store(e.value, got[..]) && equal32(got, want);
  #assert decode("388f7b0f632de8140fe337e62a37f3566500a99934c2231b6cb9fd7584b8e672", want[..]) == 32;
  lifted := sg_lift_x_public(v.public_key[..]);
  #assert lifted.ok && sf_store(sg_to_affine(lifted.value).value.y, got[..]) && equal32(got, want);
}
// Eight signatures made by a Python transcription of BIP-340's reference
// signing algorithm, with seeded random keys, auxiliary randomness and
// messages of 0, 1, 5, 32, 32, 33, 64 and 100 bytes: public key, message,
// signature. Both paths accept each one and refuse it after a one-bit change
// to the message, r or s.
RANDOM :: 8;
RANDOM_HEX :: [24] string.[
    "284f0583b915c1587d121420417894f41591c2584bc96b062bd8356a34c3fc32", "", "a95d29425f08da0a1ec5c44dda6f3f1ce7aeb1865ab7fad7746b0138f9b4208fb6b5ab4327d7928f03412a1de59fe334c50ec542b2e272c6eb11175c23239317",
    "2dc4442cd743a86ac4ab5498e46852132718490759d2af32542741c954f3798f", "a1", "58274ae82f60f9c1ffdc35f270776bca60225d3e0b759d1b7dd9c64f9c7243a6de750f5a8394efde50464b6944a4da4c09cbdacccfb857f44cbd570ab0a17629",
    "ca1b3e80f78818190b77740d05fbfba8717e932e92865cdeef016e4263235566", "60e56143d6", "f336f886f98da08c45149b040ee08569fbe473277e73824a7e610d6686125b70d3adaf37fbe3531b2064c17687d432dde2e51e7181251bc2f2762b8849f4c644",
    "0a376d0fb88c281bc332677626ed6ef8afd019f5634b26756b06c86929df3fcd", "5d4e9f747f615164c6f728d718353713827ac883d7fb9659234074f5258f6c68", "2dfeea101d15820e1c6523b9ff0297231212a494e752a347b96a43193fda1e995ce00f2adfb0e8c5941890dfeedcdb14681a743bacb616aad589e37ddaeaa000",
    "ac7947c312607ecee46e039f009f50797946278320294d4ba9dab79c4d7ef74c", "42aeb42bf227d50fff07c3c20624292e3b83d5a9c6eae1ec2a0f9e2cf60b7539", "34c8dd13c37c764305d0ba2549d78bdabd65ba6150786c896286742e9d7d20a6423e4c5e64b57c3f7d0ca83148996c1c93f22f6d3fd6352f6abc7ac8b5b85c72",
    "357f69fe754160e1ce84c2286668bf8872e314909cd44d7eea178f0c14035591", "c3fca1e7a426108e158fb59e0945cfe8610c887948183be437bc276566f3835b05", "8f9e5a45e25404247cfe739c246a996b5d95d8fcd8cb10bf70e1862f1945652f110383e7514a515df67d3e810942f8bb7d5d5254dcff164d0fd0f1288cc2fd91",
    "6e44888733f283debaafbc0c3a160d49f8e039b0993f14e73be5c6fa570e3446", "8cf90e6f20db1158ab47f04ce1fc2c71e09454839fc36a9b488bfe66d23a02c10e16cd3efb2f5521ead3ce897ef2fc41addef3a23762d60f85420b12634f7406", "9ff6261b0a657545725a24363358501d3426dcf16c505d658fabb264f63f8954e1284dcd92b54b200d5d97b6fbf2acdd3ea5224cd6c6869714304c8b9b44eef6",
    "a3397f97ac57bd0bc444af282c9299f5164b6f7868c418ead77a5cce02a5c063", "4066ce093166b7b83baf6024f6360c13f64b615e3a68585090301f44ec2731a7c8efdab5dc6bbf0614665cd0e8b8bdcf63543007a52bcf61ae848f3b51cf44a8bddd5ccf695e24ae9af03305b619768b99ac6ecf5d27c7fe6d3dca0b3a377983e3cd1964", "cf165ab3600c16cd738e275d9ab136575541ef94877792927ba80455f445f5b5e06cf5c9795d0ee0e5e30b52a0bcd58374164313b0b5ac6eda457c85646a8e98"];
// Both paths give want for this input.
both :: (m: [] u8, key: [] u8, sig: [] u8, want: bool) {
  #assert schnorr_verify_vartime(m, key, sig) == want;
  #assert schnorr_fast_verify_vartime(m, key, sig) == want;
}
#forall "both paths accept fresh signatures and refuse one-bit changes" i: 0..<RANDOM #steps 4000000000 {
  key: [32] u8; sig: [64] u8; message: [100] u8;
  #assert decode(RANDOM_HEX[3 * i], key[..]) == 32;
  length := decode(RANDOM_HEX[3 * i + 1], message[..]);
  #assert decode(RANDOM_HEX[3 * i + 2], sig[..]) == 64;
  m := message[0..<length];
  both(m, key[..], sig[..], true);
  // One one-bit change per signature, in turn: the message, r, then s.
  changed := message; bad := sig;
  if i % 3 == 0 && length > 0 { changed[0] ^= 1; }
  else if i % 3 == 1 { bad[31] ^= 1; }
  else { bad[63] ^= 1; }
  both(changed[0..<length], key[..], bad[..], false);
  // The Nostr wrappers accept only the 32-byte messages.
  #assert nostr_verify_vartime(m, key[..], sig[..]) == (length == 32);
  #assert nostr_fast_verify_vartime(m, key[..], sig[..]) == (length == 32);
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/schnorr-verify.coax
coax/bin/coax mutate nostr/examples/checks/schnorr-verify.coax --root-only
```

`coax test` runs 167 checks, including the loaded files' own checks, in
about 3 minutes on 2026-11-01. A full verification on both paths takes
tens of millions of interpreter steps, so the two signature checks have a
budget of 4 billion steps each; they used 1.44 and 1.73 billion. Root-only mutation kills all 8 mutants in the checks file;
the example's own procedures were not mutated for this chapter.

### The library's own checks

[schnorr_checks.coax](../../coax/tests/nostr/schnorr_checks.coax) and
[schnorr_fixtures.coax](../../coax/tests/nostr/schnorr_fixtures.coax) check
the lengths, the challenge's refusals, the comparison's handling of
infinity, parity and `x`, and the preparation of all 19 rows against
independently computed `s`, `-e`, `r` and `P`. `schnorr.coax` and
`schnorr_fast.coax` are `#authority` files. The records report zero mutation
survivors: 1,907 of 1,907 for the reference closure on 2026-10-28 and 2,535
of 2,535 for the fast path on 2026-10-29.

### libsecp256k1 and long runs

[schnorr.py](../../coax/tests/nostr/schnorr.py) and
[schnorr_fast.py](../../coax/tests/nostr/schnorr_fast.py) compile both paths
to native C and wasm and compare them with libsecp256k1 `v0.8.0` and an
independent Python affine reference: every vector, seeded valid and changed
signatures, `r` and `s` at their bounds, messages at and over the limit,
and inputs that must stay unchanged. The fast path's differential covers
790 cases per build. [schnorr_hour.py](../../coax/tests/nostr/schnorr_hour.py)
ran the reference path for one hour on 2026-10-28: 1,743,107 cases per
native profile and 102,536 Nostr cases, with no mismatch. These were not
rerun for this chapter; the records are
[coax-schnorr-verify-2026-10-28.md](../../docs/verification/coax-schnorr-verify-2026-10-28.md)
and [2026-10-29-schnorr-fast.md](../../docs/verification/2026-10-29-schnorr-fast.md).

## Summary

- Verification lifts the key, checks `r < p` and `s < n`, hashes the tagged
  challenge, computes `R = s·G - e·P`, and accepts only a finite `R` with
  even `y` and `x(R) = r`.
- Refusals happen in preparation (lengths, key, `r`, `s`) or in the final
  comparison (infinity, odd `y`, wrong `x`).
- The Nostr wrappers accept exactly 32 bytes of event ID.
- All 19 BIP-340 vectors give their published result, and the reference and
  fast paths agree on them and on fresh signatures.

## Dependencies

- [4. SHA-256](ch004-sha-256.md)
- [8. Scalars mod n](ch008-scalars-mod-n.md)
- [10. Scalar multiplication](ch010-scalar-multiplication.md)

## Sources

- [BIP-340](https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki)
  and its pinned
  [test-vectors.csv](../../coax/tests/vectors/upstream/bip340/test-vectors.csv),
  listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- [NIP-01](../nips/official/01.md), which requires the `sig` to be a
  BIP-340 signature of the event ID.
- [libsecp256k1](https://github.com/bitcoin-core/secp256k1), release
  `v0.8.0`, pinned in
  [UPSTREAM.json](../../tools/coax-nostr/fuzz/UPSTREAM.json).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
