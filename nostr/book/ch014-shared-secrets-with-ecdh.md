# 14. Shared secrets with ECDH

NIP-44 encrypts with a key that two parties derive from their own secret key
and the other's public key. Neither sends the key. This is elliptic-curve
Diffie-Hellman (ECDH). NIP-44 needs the raw x coordinate of the shared
point, not a hash of it, which is not what libsecp256k1 returns by default.
This chapter builds that ECDH, its checks on the peer's key and the secret,
and the NIP-44 conversation key on top of it.

Part III, Elliptic curves.

## What this chapter teaches

- Why `a·(b·G)` and `b·(a·G)` are the same point, and why only its x is
  kept.
- Lift the peer's x-only key to a point, refusing an x that is not below
  the field prime `p` or has no point on the curve.
- Refuse a secret of 0 or of `n` and above, on the constant-time path.
- Return the 32-byte x coordinate unhashed, and derive the NIP-44
  conversation key from it with HKDF-extract.
- Adapt the Wycheproof secp256k1 ECDH cases to x-only keys, and say which
  cases have no x-only form.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [ecdh.coax](../../coax/lib/crypto/ecdh.coax) | `ecdh_stage(secret, peer, work)` | validates both keys, multiplies, writes the raw x to `work[0].staged` |
| [ecdh_fast.coax](../../coax/lib/crypto/ecdh_fast.coax) | `ef_add`, `ef_double`, `ef_inverse` | the 32-bit-limb field and group code the ladder runs on |
| [ecdh_lifecycle.coax](../../coax/lib/crypto/ecdh_lifecycle.coax) | `el_open` ... `el_close` | the state rules a host follows to own and wipe the scratch |
| [nip44-key.coax](../../coax/lib/nostr/nip44-key.coax) | `nip44_conversation_key_from_keys_checked` | ECDH, then HKDF-extract, then wipes the scratch |
| [nip44.coax](../../coax/lib/nostr/nip44.coax) | `nip44_conversation_key_checked(shared_x, out)` | HKDF-extract with salt `nip44-v2` |

`ecdh.coax`, `ecdh_lifecycle.coax` and `nip44-key.coax` are marked
`#authority`: they decide what is admitted, so every mutation of them must
be killed by a check.

## The shared point

Alice has a secret `a` and publishes `A = a·G`. Bob has a secret `b` and
publishes `B = b·G`. Alice computes `a·B` and Bob computes `b·A`. Both are
`a·b·G`, because scalar multiplication is repeated addition of one point
and the order of the factors does not matter:

```text
a·B = a·(b·G) = (a·b)·G = (b·a)·G = b·(a·G) = b·A
```

Someone who sees only `A` and `B` would have to recover `a` or `b` from a
point, which is the discrete logarithm problem that chapter 9's group is
chosen to make hard.

### Worked example: a = 3, b = 5

Take `a = 3` and `b = 5`.

1. Alice publishes `A = 3G`. Its x is
   `f9308a01...bce036f9`, the BIP-340 vector 0 public key of chapter 10.
2. Bob publishes `B = 5G`, with x `2f8bde4d...b240efe4`, the point
   `11G - 2(3G)` of chapter 10.
3. Alice computes `3·B = 3·5G = 15G`.
4. Bob computes `5·A = 5·3G = 15G`.
5. Both keep only the x of `15G`, `d7924d4f...e27e080e`.

The example below computes each step and prints the same values.

### Only x is kept

A Nostr public key is 32 bytes: the x of the point, with the y left out
(BIP-340, chapter 11). An x names two points, `(x, y)` and `(x, p - y)`,
which are `P` and `-P`. BIP-340 picks the one with even y. ECDH does not
need to know which one the peer meant, because

```text
k·(-P) = -(k·P)
```

and a point and its negative have the same x. So the x of the shared point
is the same whichever y the peer's key had, and the result is defined by
the two x-only keys alone.

NIP-44 then uses that x directly:

```text
shared_x         = x(a·B), 32 bytes, big-endian
conversation_key = HKDF-extract(salt = "nip44-v2", IKM = shared_x)
```

HKDF-extract is one HMAC-SHA256 with the salt as the key (chapters 5
and 6). libsecp256k1's `secp256k1_ecdh` instead returns, by default,
`SHA-256(02 or 03 || x)`. For the NIP-44 test keys in the example, that is
`dbeece7b...a719b359`, not the shared x `94da47d8...1f4f50a4`, and a
conversation key built from it decrypts nothing. The code here never hashes
the point.

## Checking the inputs

`ecdh_stage(secret, peer, work)` checks its inputs in this order and stops
at the first failure:

1. **Lengths.** `secret` and `peer` must be 32 bytes and `work` exactly one
   `EcdhScratch`. Otherwise `EC_LENGTH`.
2. **The peer.** `sg_lift_x_public(peer)` (chapter 9) reads the 32 bytes as
   x. It refuses when `x ≥ p`, or when `x³ + 7` has no square root mod `p`,
   which means no point on the curve has that x. Otherwise `EC_PEER`. The
   peer is public, so this check may branch.
3. **The secret.** The 32 bytes are read as a big-endian number `k`. It
   must satisfy `1 ≤ k < n`. This check is computed as a mask with no
   branch: a secret of 0 or `n` takes the same path as a valid one, with
   `k` replaced by 1. The staged x is then ANDed with the mask, so it is all
   zero, and the status is `EC_SCALAR`.

An x with no point is not harmless. Such an x is the x of a point on the
*quadratic twist*, the curve `d·y² = x³ + 7` for a non-square `d` mod `p`.
The twist has small subgroups, of order 3, 13 and 3319 among others. A
multiplication that worked from x alone would compute on the twist, and its
result would reveal the secret modulo those orders. Refusing every x that does not lift closes this
invalid-curve attack. NIP-44's invalid vectors include twist points of
order 3, 13 and 3319 for this reason.

## The multiplication

Once both inputs are admitted, `ecdh_multiply_admitted` runs chapter 10's
signed radix-16 ladder on the 32-bit-limb field code of `ecdh_fast.coax`:

1. Build the table `0·P, 1·P, ..., 8·P` with seven additions.
2. Recode `k` into 65 signed digits in `-8..8` (`smc_recode`).
3. Start from the top digit. For each of the 64 lower digits, double four
   times, read all nine table entries to pick `|d|·P`, negate it with a mask
   when `d < 0`, and add.

Every admitted secret runs 256 doublings and 65 additions and reads the
same memory. `ecdh_secret_math` then converts the result to affine x with
one field inversion, `x = X / Z²`, and `ecdh_stage` stores it big-endian in
`work[0].staged`.

## Who owns the secret scratch

The scalar, the shared point and the staged x are secret, and they sit in
`work`, which the caller passes in. `ecdh_stage` doesn't wipe it, because
it returns the result there. `nip44_conversation_key_from_keys_checked`
does: it runs ECDH, derives the conversation key into `out`, and sets
`work[0]` to zero on every path after the lengths are admitted, including
both refusals. `out` is written only on success.

These are plain stores. Erasure that the C compiler cannot remove is the
host's job (`zeroize.coax`). `ecdh_lifecycle.coax` is the state model a
host follows when it owns the scratch instead: a slot moves through free,
ready, running, cleaning, output and done, and an error completes only
after the dirty regions are wiped. The
[owned lifecycle record](../../docs/verification/2026-10-29-ecdh-owned-lifecycle.md)
maps every return of that host interface.

## The example

The example is [nostr/examples/ecdh.coax](../examples/ecdh.coax). It works
the `a = 3`, `b = 5` exchange, derives a shared x and a NIP-44 conversation
key from two NIP-44 test keys, and shows the refusals. The two keys are
`sec1` and `sec2` of the third `encrypt_decrypt` vector in the pinned
`nip44.vectors.json`. They are published test values; never use them for
real messages.

<!-- source: nostr/examples/ecdh.coax example -->
```jai
#load "../../coax/lib/nostr/nip44-key.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [80] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  #assert r.error == 0;
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// Decode 64 hex digits into 32 bytes.
bytes_of :: (text: string) -> [32] u8 {
  b: [32] u8;
  #assert hex_decode_checked(text, b[..], false).error == 0;
  return b;
}
// The x-only public key of a secret: x(k·G) on the ladder of chapter 10.
public_x :: (secret: [] u8) -> [32] u8 {
  k := sc_reduce_bytes(secret).value;
  a := sg_to_affine(smc_multiply(k, sg_generator()).value).value;
  b: [32] u8;
  #assert sf_store(a.x, b[..]);
  return b;
}
// A 32-byte secret holding a small number.
small_secret :: (k: u8) -> [32] u8 { b: [32] u8; b[31] = k; return b; }
// The name of an ecdh_stage status.
ec_name :: (code: u32) -> string {
  if code == EC_OK { return "EC_OK"; }
  if code == EC_LENGTH { return "EC_LENGTH"; }
  if code == EC_PEER { return "EC_PEER"; }
  if code == EC_SCALAR { return "EC_SCALAR"; }
  return "other";
}
// The name of a conversation-key status.
nip44_name :: (code: u32) -> string {
  if code == NIP44_OK { return "NIP44_OK"; }
  if code == NIP44_PEER { return "NIP44_PEER"; }
  if code == NIP44_SECRET { return "NIP44_SECRET"; }
  return "other";
}
// Run ECDH; print the status and, on success, the raw shared x.
derive :: (label: string, secret: [] u8, peer: [] u8) -> [32] u8 {
  work: [1] EcdhScratch;
  code := ecdh_stage(secret, peer, work[..]);
  out := work[0].staged;
  work[0] = EcdhScratch.{};
  if code == EC_OK { show(label, out[..]); }
  else { print("%: %\n", label, ec_name(code)); }
  return out;
}
// Run ECDH and report only the status and whether the output is all zero.
try_refuse :: (label: string, secret: [] u8, peer: [] u8) {
  work: [1] EcdhScratch;
  code := ecdh_stage(secret, peer, work[..]);
  zero := true;
  for b: work[0].staged { if b != 0 { zero = false; } }
  work[0] = EcdhScratch.{};
  print("%: %, staged x all zero %\n", label, ec_name(code), zero);
}
same_bytes :: (a: [] u8, b: [] u8) -> bool { return cb_ct_equal(a, b) == CB_MAX64; }

P_HEX :: "fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2f";
N_HEX :: "fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141";

main :: () {
  // Small scalars: a = 3, b = 5. Each side sends only x(aG) or x(bG).
  a := small_secret(3); b := small_secret(5); c := small_secret(15);
  ax := public_x(a[..]); bx := public_x(b[..]);
  show("x(3G), sent by a", ax[..]);
  show("x(5G), sent by b", bx[..]);
  s1 := derive("a: 3 times x(5G)", a[..], bx[..]);
  s2 := derive("b: 5 times x(3G)", b[..], ax[..]);
  fifteen := public_x(c[..]);
  show("x(15G)", fifteen[..]);
  print("both sides hold x(15G): %\n", same_bytes(s1[..], fifteen[..]) && same_bytes(s2[..], fifteen[..]));

  // Two NIP-44 test keys (encrypt_decrypt vector 3). They are published
  // test values, never real secrets.
  sec1 := bytes_of("5c0c523f52a5b6fad39ed2403092df8cebc36318b39383bca6c00808626fab3a");
  sec2 := bytes_of("4b22aa260e4acb7021e32f38a6cdf4b673c6a277755bfce287e370c924dc936d");
  pub1 := public_x(sec1[..]); pub2 := public_x(sec2[..]);
  show("pub1", pub1[..]);
  show("pub2", pub2[..]);
  x1 := derive("sec1 with pub2, shared x", sec1[..], pub2[..]);
  x2 := derive("sec2 with pub1, shared x", sec2[..], pub1[..]);
  print("same shared x: %\n", same_bytes(x1[..], x2[..]));

  // The NIP-44 conversation key: HKDF-extract(salt "nip44-v2", raw x).
  key: [32] u8;
  #assert nip44_conversation_key_checked(x1[..], key[..], false) == NIP44_OK;
  show("conversation key", key[..]);
  want := bytes_of("3e2b52a63be47d34fe0a80e34e73d436d6963bc8f39827f327057a9986c20a45");
  print("matches the NIP-44 vector: %\n", same_bytes(key[..], want[..]));
  work: [1] EcdhScratch; other: [32] u8;
  code := nip44_conversation_key_from_keys_checked(sec2[..], pub1[..], work[..], other[..], false);
  print("from sec2 and pub1 in one call: %, same key %\n", nip44_name(code), same_bytes(other[..], key[..]));

  // Refusals.
  zero := small_secret(0);
  p := bytes_of(P_HEX);
  n := bytes_of(N_HEX);
  ones: [32] u8; for i: 0..<32 { ones[i] = 255; }
  try_refuse("peer x = 0 (0^3 + 7 has no square root)", sec1[..], zero[..]);
  try_refuse("peer x = p (not below the field prime)", sec1[..], p[..]);
  try_refuse("peer x = 2^256 - 1", sec1[..], ones[..]);
  try_refuse("secret 0", zero[..], pub2[..]);
  try_refuse("secret n (the group order)", n[..], pub2[..]);
  short: [31] u8;
  print("31-byte peer: %\n", ec_name(ecdh_stage(sec1[..], short[..], work[..])));
  for i: 0..<32 { other[i] = 165; }
  code = nip44_conversation_key_from_keys_checked(sec1[..], zero[..], work[..], other[..], false);
  print("conversation key, peer x = 0: %, key untouched %\n", nip44_name(code), other[0] == 165);
  code = nip44_conversation_key_from_keys_checked(zero[..], pub2[..], work[..], other[..], false);
  print("conversation key, secret 0: %, key untouched %\n", nip44_name(code), other[0] == 165);
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/ecdh.coax
```

The first six lines are the worked example: both sides reach the x of
`15G`. The NIP-44 keys give one shared x from either side, and its
HKDF-extract is the vector's published conversation key `3e2b52a6...`:

```text
x(3G), sent by a: f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9
x(5G), sent by b: 2f8bde4d1a07209355b4a7250a5c5128e88b84bddc619ab7cba8d569b240efe4
a: 3 times x(5G): d7924d4f7d43ea965a465ae3095ff41131e5946f3c85f79e44adbcf8e27e080e
b: 5 times x(3G): d7924d4f7d43ea965a465ae3095ff41131e5946f3c85f79e44adbcf8e27e080e
x(15G): d7924d4f7d43ea965a465ae3095ff41131e5946f3c85f79e44adbcf8e27e080e
both sides hold x(15G): true
pub1: 87d3561f19b74adbe8bf840682992466068830a9d8c36b4a0c99d36f826cb6cb
pub2: fa3b4f81a620c66514bda0302847df167ed02a483141b5939e57bdd0cf76ad3b
sec1 with pub2, shared x: 94da47d851b9c1ed33b3b72f35434f56aa608d60e573e9c295f568011f4f50a4
sec2 with pub1, shared x: 94da47d851b9c1ed33b3b72f35434f56aa608d60e573e9c295f568011f4f50a4
same shared x: true
conversation key: 3e2b52a63be47d34fe0a80e34e73d436d6963bc8f39827f327057a9986c20a45
matches the NIP-44 vector: true
from sec2 and pub1 in one call: NIP44_OK, same key true
peer x = 0 (0^3 + 7 has no square root): EC_PEER, staged x all zero true
peer x = p (not below the field prime): EC_PEER, staged x all zero true
peer x = 2^256 - 1: EC_PEER, staged x all zero true
secret 0: EC_SCALAR, staged x all zero true
secret n (the group order): EC_SCALAR, staged x all zero true
31-byte peer: EC_LENGTH
conversation key, peer x = 0: NIP44_PEER, key untouched true
conversation key, secret 0: NIP44_SECRET, key untouched true
```

The refusals:

- `x = 0`: `0³ + 7 = 7` has no square root mod `p`. The x is the twist's
  point of order 3, so `EC_PEER`.
- `x = p` and `x = 2^256 - 1` are not below `p`. `sg_lift_x_public` reads
  the bytes, reduces them, and refuses because the canonical encoding
  differs from the input, so `EC_PEER`.
- A secret of 0 or `n` runs the full multiplication with 1 in its place,
  then masks the staged x to zero and returns `EC_SCALAR`.
- A 31-byte peer is refused before anything is read, `EC_LENGTH`.
- Through `nip44_conversation_key_from_keys_checked`, the same refusals
  are `NIP44_PEER` (13) and `NIP44_SECRET` (14), and `out` keeps its
  previous bytes.

The book's checks run this program in debug and release builds and compare
the output with [nostr/fixtures/ecdh.out](../fixtures/ecdh.out).

## How the code is checked

### Wycheproof adapted to x-only keys

Wycheproof's
[ecdh_secp256k1_test.json](../../coax/tests/vectors/upstream/wycheproof/ecdh_secp256k1_test.json)
has 752 cases, each a DER `SubjectPublicKeyInfo` public key, a private key
and the expected raw shared x. An x-only API takes 32 bytes of x, so each
case is adapted by taking the x out of the SEC1 point, when there is one:

| Wycheproof cases | Count | x-only form | Expected here |
| --- | ---: | --- | --- |
| valid | 473 | x of the point | the published shared x |
| acceptable, compressed key | 2 | x of the point | the published shared x |
| invalid, x has no point (x = 0, p - 1, p, twist points) | 16 | x of the point | `EC_PEER` |
| invalid, x on the curve but y wrong | 5 | x of the point | the even-y point's shared x |
| invalid, bad DER or explicit curve parameters | 28 | none | not applicable |
| acceptable, explicit curve parameters | 228 | none | not applicable |

The 5 wrong-y cases (tcIds 479-482 and 494) are invalid because their y is
not on the curve. An x-only key has no y: the x names the valid even-y
point, and the result is that point's shared x. The 28 and 228 cases test
DER parsing and curve parameters, which an x-only key does not have. So
**every invalid Wycheproof public key that has an x-only form and names no
point refuses**: all 16.

### Checks in the example

The checks are in
[nostr/examples/checks/ecdh.coax](../examples/checks/ecdh.coax), which loads
the example. They compare the code with published values:

- six NIP-44 `get_conversation_key` rows, including `sec1 = n - 2` and
  `pub2 = x(G)`, and all eight invalid rows, which refuse as `NIP44_PEER`;
- 12 of the 475 Wycheproof valid and acceptable cases, one for each kind of
  edge: shared x of `p - 3` and `2^16`, public x of 1 and `p - 3`, the
  doubling edge cases and private key 3;
- all 16 off-curve Wycheproof keys, each refused with a zero staged x;
- the two distinct wrong-y keys against their shared x, computed with
  Python's integers;
- `a·x(bG) = b·x(aG) = x(abG)` for the pairs (2, 3), (3, 5) and (4, 7);
- a peer with odd y (`17G`), whose x-only key names `-17G`, against the
  ladder on the true `17G`.

<!-- source: nostr/examples/checks/ecdh.coax check -->
```jai
// Run ECDH from hex; return the status and the staged raw x.
EcdhRun :: struct { code: u32; x: [32] u8; }
run_hex :: (secret_hex: string, peer_hex: string) -> EcdhRun {
  secret := bytes_of(secret_hex); peer := bytes_of(peer_hex);
  work: [1] EcdhScratch; r: EcdhRun;
  r.code = ecdh_stage(secret[..], peer[..], work[..]);
  r.x = work[0].staged;
  return r;
}
// True when the staged x equals the expected hex.
x_is :: (r: EcdhRun, want_hex: string) -> bool {
  want := bytes_of(want_hex); got := r.x;
  return same_bytes(got[..], want[..]);
}

// NIP-44 get_conversation_key rows from the pinned nip44.vectors.json:
// (sec1, pub2, conversation_key). Rows 0-2 and the three rows with notes:
// sec1 = n - 2, sec1 = 2, and pub2 = x(G) with sec1 = 1. All 35 rows run
// end to end in coax/tests/nostr/nip44_key_vectors.coax.
NIP44_ROWS :: [18] string.[
    "315e59ff51cb9209768cf7da80791ddcaae56ac9775eb25b6dee1234bc5d2268", "c2f9d9948dc8c7c38321e4b85c8558872eafa0641cd269db76848a6073e69133", "3dfef0ce2a4d80a25e7a328accf73448ef67096f65f79588e358d9a0eb9013f1",
    "a1e37752c9fdc1273be53f68c5f74be7c8905728e8de75800b94262f9497c86e", "03bb7947065dde12ba991ea045132581d0954f042c84e06d8c00066e23c1a800", "4d14f36e81b8452128da64fe6f1eae873baae2f444b02c950b90e43553f2178b",
    "98a5902fd67518a0c900f0fb62158f278f94a21d6f9d33d30cd3091195500311", "aae65c15f98e5e677b5050de82e3aba47a6fe49b3dab7863cf35d9478ba9f7d1", "9c00b769d5f54d02bf175b7284a1cbd28b6911b06cda6666b2243561ac96bad7",
    "fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364139", "0000000000000000000000000000000000000000000000000000000000000002", "8b6392dbf2ec6a2b2d5b1477fc2be84d63ef254b667cadd31bd3f444c44ae6ba",
    "0000000000000000000000000000000000000000000000000000000000000002", "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdeb", "be234f46f60a250bef52a5ee34c758800c4ca8e5030bf4cc1a31d37ba2104d43",
    "0000000000000000000000000000000000000000000000000000000000000001", "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798", "3b4610cb7189beb9cc29eb3716ecc6102f1247e8f3101a03a1787d8908aeb54e"];
#forall "NIP-44 get_conversation_key rows give the published key" i: 0..<6 #steps 1000000000 {
  secret := bytes_of(NIP44_ROWS[3 * i]); peer := bytes_of(NIP44_ROWS[3 * i + 1]);
  want := bytes_of(NIP44_ROWS[3 * i + 2]);
  work: [1] EcdhScratch; key: [32] u8;
  #assert nip44_conversation_key_from_keys_checked(secret[..], peer[..], work[..], key[..], false) == NIP44_OK;
  #assert same_bytes(key[..], want[..]);
}
// The 8 invalid get_conversation_key rows. Every pub2 here has no point on
// the curve, so each refuses as NIP44_PEER before the secret is read.
NIP44_INVALID :: [16] string.[
    "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff", "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef", // sec1 higher than curve.n
    "0000000000000000000000000000000000000000000000000000000000000000", "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef", // sec1 is 0
    "fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364139", "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff", // pub2 all-ff
    "fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141", "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef", // sec1 == curve.n
    "0000000000000000000000000000000000000000000000000000000000000002", "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef", // pub2 no sqrt
    "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20", "0000000000000000000000000000000000000000000000000000000000000000", // order 3 on twist
    "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20", "eb1f7200aecaa86682376fb1c13cd12b732221e774f553b0a0857f88fa20f86d", // order 13 on twist
    "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20", "709858a4c121e4a84eb59c0ded0261093c71e8ca29efeef21a6161c447bcaf9f"]; // order 3319 on twist
#forall "NIP-44 invalid get_conversation_key rows refuse" i: 0..<8 #steps 1000000000 {
  secret := bytes_of(NIP44_INVALID[2 * i]); peer := bytes_of(NIP44_INVALID[2 * i + 1]);
  work: [1] EcdhScratch; key: [32] u8; for j: 0..<32 { key[j] = 165; }
  #assert nip44_conversation_key_from_keys_checked(secret[..], peer[..], work[..], key[..], false) == NIP44_PEER;
  for j: 0..<32 { #assert key[j] == 165; }
}

// Wycheproof ecdh_secp256k1_test.json (pinned in coax/tests/vectors/SOURCES.md),
// adapted to x-only keys: the 32-byte x of the SEC1 public key (uncompressed
// 04 or compressed 02/03 under the named-curve SubjectPublicKeyInfo), the
// private key as 32 bytes, and the published shared x. 12 of the 475 valid or
// acceptable cases, one per kind of edge.
WY_VALID :: [36] string.[
    "f4b7ff7cccc98813a69fae3df222bfe3f4e28f764bf91b4a10d8096ce446b254", "d8096af8a11e0b80037e1ee68246b5dcbb0aeb1cf1244fd767db80f3fa27da2b", "544dfae22af6af939042b1d85b71a1e49e9a5614123c4d6ad0c8af65baf87d65", // tcId 1: normal case
    "f4b7ff7cccc98813a69fae3df222bfe3f4e28f764bf91b4a10d8096ce446b254", "d8096af8a11e0b80037e1ee68246b5dcbb0aeb1cf1244fd767db80f3fa27da2b", "544dfae22af6af939042b1d85b71a1e49e9a5614123c4d6ad0c8af65baf87d65", // tcId 2: compressed public key
    "a2b6442a37f8a3764aeff4011a4c422b389a1e509669c43f279c8b7e32d80c3a", "6da9eb2cdac02122d5f05cf6a8cd768e378f664ea4a7871d10e25f57eb1ee1cc", "fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2c", // tcId 6: shared x = p - 3
    "a2b6442a37f8a3764aeff4011a4c422b389a1e509669c43f279c8b7e32d80c3a", "f2976154c4f53ce392d1fe39a891a4611ba8cf046023cd8f1bcd9fdd2e921191", "0000000000000000000000000000000000000000000000000000000000010000", // tcId 7: shared x = 2^16
    "a2b6442a37f8a3764aeff4011a4c422b389a1e509669c43f279c8b7e32d80c3a", "f43b610a2a5c5f6e2b395567489657059e3351c6f9a7e2ebde52638abfea006a", "55555555555555555555555555555555555555555555555555555554fffffebc", // tcId 41: shared x about p/3
    "a2b6442a37f8a3764aeff4011a4c422b389a1e509669c43f279c8b7e32d80c3a", "56baf1d72606c7af5a5fa108620b0839e2c7dd40b832ef847e5b64c86efe1aa5", "800000000000000000000000009fa2f1ffffffffffffffffffffffffffffffff", // tcId 45: public y with many trailing 1s
    "2bc15cf3981eab61e594ebf591290a045ca9326a8d3dd49f3de1190d39270bb8", "0000000000000000000000000000000000000000000000000000000000000001", "12c2ad36a59fda5ac4f7e97ff611728d0748ac359fca9b12f6d4f43519516487", // tcId 57: public x = 1
    "2bc15cf3981eab61e594ebf591290a045ca9326a8d3dd49f3de1190d39270bb8", "fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2c", "e97fb4c4fb33d6a114da6e0d180e54f99ec1ece9ff558871054e99d221930d16", // tcId 60: public x = p - 3
    "938f3dbe37135cd8c8c48a676b28b2334b72a3f09814c8efb6a451be00c93d23", "59294e8bc54e76d48b5594f01fe4729566d9b6df6385982fbb533183921f1a12", "e976057e8a322dfdb2debd55d8e58802fb54425950b2dbfd00f0813de27105e4", // tcId 99: Jacobian edge case
    "c1781d86cac2c052b865f228e64bd1ce433c78ca7dfca9e8b810473e2ce17da5", "0000000000000000000000000000000000000000000000000000000000000001", "afe5b60fe3e1dc873b3f3022893a359880e817537beb96b3d48d375766ab59e6", // tcId 279: doubling to x = 1
    "c1781d86cac2c052b865f228e64bd1ce433c78ca7dfca9e8b810473e2ce17da5", "cbb0deab125754f1fdb2038b0434ed9cb3fb53ab735391129994a535d925f673", "af306c993dee0dcfc441ebe53360b569e21f186052db8197f4a124fa77b98148", // tcId 369: doubling to y = 1
    "0000000000000000000000000000000000000000000000000000000000000003", "32bdd978eb62b1f369a56d0949ab8551a7ad527d9602e891ce457586c2a8569e", "34005694e3cac09332aa42807e3afdc3b3b3bc7c7be887d1f98d76778c55cfd7"]; // tcId 459: private key 3
#forall "Wycheproof valid cases give the published raw x" i: 0..<12 #steps 1000000000 {
  r := run_hex(WY_VALID[3 * i], WY_VALID[3 * i + 1]);
  #assert r.code == EC_OK && x_is(r, WY_VALID[3 * i + 2]);
}
// Every invalid Wycheproof case whose public key has an x-only form and whose
// x is not on the curve: x = 0, x = p - 1 and x = p from the uncompressed keys,
// and three compressed keys. Each refuses as EC_PEER and stages zeros.
WY_OFF_CURVE :: [32] string.[
    "c6cafb74e2a50c83b3d232c4585237f44d4c5433c4b3f50ce978e6aeda3a4f5d", "0000000000000000000000000000000000000000000000000000000000000000", // tcId 475
    "c6cafb74e2a50c83b3d232c4585237f44d4c5433c4b3f50ce978e6aeda3a4f5d", "0000000000000000000000000000000000000000000000000000000000000000", // tcId 476
    "c6cafb74e2a50c83b3d232c4585237f44d4c5433c4b3f50ce978e6aeda3a4f5d", "0000000000000000000000000000000000000000000000000000000000000000", // tcId 477
    "c6cafb74e2a50c83b3d232c4585237f44d4c5433c4b3f50ce978e6aeda3a4f5d", "0000000000000000000000000000000000000000000000000000000000000000", // tcId 478
    "c6cafb74e2a50c83b3d232c4585237f44d4c5433c4b3f50ce978e6aeda3a4f5d", "fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2e", // tcId 483
    "c6cafb74e2a50c83b3d232c4585237f44d4c5433c4b3f50ce978e6aeda3a4f5d", "fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2e", // tcId 484
    "c6cafb74e2a50c83b3d232c4585237f44d4c5433c4b3f50ce978e6aeda3a4f5d", "fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2e", // tcId 485
    "c6cafb74e2a50c83b3d232c4585237f44d4c5433c4b3f50ce978e6aeda3a4f5d", "fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2e", // tcId 486
    "c6cafb74e2a50c83b3d232c4585237f44d4c5433c4b3f50ce978e6aeda3a4f5d", "fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2f", // tcId 487
    "c6cafb74e2a50c83b3d232c4585237f44d4c5433c4b3f50ce978e6aeda3a4f5d", "fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2f", // tcId 488
    "c6cafb74e2a50c83b3d232c4585237f44d4c5433c4b3f50ce978e6aeda3a4f5d", "fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2f", // tcId 489
    "c6cafb74e2a50c83b3d232c4585237f44d4c5433c4b3f50ce978e6aeda3a4f5d", "fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2f", // tcId 490
    "cfe75ee764197aa7732a5478556b478898423d2bc0e484a6ebb3674a6036a65d", "0000000000000000000000000000000000000000000000000000000000000000", // tcId 495: (0, 0)
    "d09182a4d0c94ba85f82eff9fc1bddb0b07d3f2af8632fc1c73a3604e8f0b335", "977cb7fb9a0ec5b208e811d6a0795eb78d7642e3cac42a801bcc8fc0f06472d4", // tcId 528: compressed
    "98b5c223cf9cc0920a5145ba1fd2f6afee7e1f66d0120b8536685fdf05ebb300", "0000000000000000000000000000000000000000000000000000000000000000", // tcId 529: compressed, twist
    "98b5c223cf9cc0920a5145ba1fd2f6afee7e1f66d0120b8536685fdf05ebb2ff", "0000000000000000000000000000000000000000000000000000000000000000"]; // tcId 530: compressed, twist
#forall "every off-curve Wycheproof public key refuses" i: 0..<16 #steps 1000000000 {
  r := run_hex(WY_OFF_CURVE[2 * i], WY_OFF_CURVE[2 * i + 1]);
  #assert r.code == EC_PEER;
  for j: 0..<32 { #assert r.x[j] == 0; }
}
// The 5 other invalid cases with an x-only form have an x on the curve and a
// wrong y (tcIds 479-482 share x = 1; tcId 494). An x-only key carries no y,
// so the x names the valid point with even y, and ECDH must give the x of
// k times that point, computed here with Python's integers.
WY_WRONG_Y :: [6] string.[
    "c6cafb74e2a50c83b3d232c4585237f44d4c5433c4b3f50ce978e6aeda3a4f5d", "0000000000000000000000000000000000000000000000000000000000000001", "7fc84eb175af79d9bed0dea1cf26850644a1e6f08701f25fb3176984c1d951ef", // tcIds 479-482
    "cfe75ee764197aa7732a5478556b478898423d2bc0e484a6ebb3674a6036a65d", "49c248edc659e18482b7105748a4b95d3a46952a5ba72da0d702dc97a64e9979", "380c53e0a509ebb3b63346598105219b43d51ae196b4557d59bbd67824032dff"]; // tcId 494
#forall "an on-curve x with a wrong y names the even-y point" i: 0..<2 #steps 1000000000 {
  r := run_hex(WY_WRONG_Y[3 * i], WY_WRONG_Y[3 * i + 1]);
  #assert r.code == EC_OK && x_is(r, WY_WRONG_Y[3 * i + 2]);
}

// Both orders agree: a·x(bG) = b·x(aG) = x(abG), for three small pairs.
PAIRS :: [6] u8.[2, 3, 3, 5, 4, 7];
#forall "both sides derive x(abG)" i: 0..<3 #steps 2000000000 {
  a := PAIRS[2 * i]; b := PAIRS[2 * i + 1];
  sa := small_secret(a); sb := small_secret(b); sab := small_secret(a * b);
  ax := public_x(sa[..]); bx := public_x(sb[..]); want := public_x(sab[..]);
  w1: [1] EcdhScratch; w2: [1] EcdhScratch;
  #assert ecdh_stage(sa[..], bx[..], w1[..]) == EC_OK && ecdh_stage(sb[..], ax[..], w2[..]) == EC_OK;
  #assert same_bytes(w1[0].staged[..], want[..]) && same_bytes(w2[0].staged[..], want[..]);
}
// The sign of the peer's y does not change the result: k·(-P) = -(k·P) has
// the same x. 17G has an odd y, so its x-only key names -17G; the ladder on
// the true 17G must give the same x as ECDH.
#test "the peer's y sign does not change the shared x" #steps 500000000 {
  point := smc_multiply(sc_small(17), sg_generator()).value;
  k := sc_small(11);
  product := sg_to_affine(smc_multiply(k, point).value).value;
  want: [32] u8; #assert sf_store(product.x, want[..]);
  s := small_secret(17); peer := public_x(s[..]);
  lifted := sg_lift_x_public(peer[..]);
  #assert lifted.ok && sg_equal_mask(lifted.value, point) != CB_MAX64;
  secret := small_secret(11); work: [1] EcdhScratch;
  #assert ecdh_stage(secret[..], peer[..], work[..]) == EC_OK;
  #assert same_bytes(work[0].staged[..], want[..]);
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/ecdh.coax
coax/bin/coax mutate nostr/examples/checks/ecdh.coax --root-only
```

On 2026-11-01, `coax test` ran 222 checks, including the loaded files' own
checks, and all passed; the checks above took about 31 seconds of that. The
root-only mutation run killed 3 of 3 mutants in the checks file. The
library files were not mutated again today: the 1,842 of 1,842 result for
the ECDH stage in the record below stands for the unchanged source.

### All the NIP-44 vectors

[coax/tests/nostr/nip44_key_vectors.coax](../../coax/tests/nostr/nip44_key_vectors.coax),
generated from the pinned `nip44.vectors.json`, runs all 35 valid
`get_conversation_key` rows and all 8 invalid rows through
`nip44_conversation_key_from_keys_checked`, and checks that the scratch
reads zero afterwards. Each row costs about 20 million evaluation steps, so it runs with a larger
budget:

```shell
coax/bin/coax test coax/tests/nostr/nip44_key_vectors.coax --library --steps 400000000
```

On 2026-11-01 it passed 238 of 238 checks. The record is
[2026-10-30-nip44-conversation-key.md](../../docs/verification/2026-10-30-nip44-conversation-key.md).

### Every Wycheproof case and libsecp256k1

[ecdh_vectors.py](../../coax/tests/nostr/ecdh_vectors.py) compiles the
public ECDH entry point to C and wasm, in checked and release builds, and
runs 783 cases through each: the 35 NIP-44 rows, the 473 valid and 13
uncompressed off-curve Wycheproof cases, 256 seeded random cases, four
invalid secrets, one invalid peer and a fixture that tells libsecp256k1's
default hashed output from the raw x. Each valid result is compared with
libsecp256k1 `v0.8.0` and with a Python computation. It needs the oracle
built as chapter 7 describes and was not rerun today. The record is
[2026-10-29-ecdh-acceptance.md](../../docs/verification/2026-10-29-ecdh-acceptance.md)
and its [evidence](../../docs/verification/2026-10-29-ecdh-abi-migration.json):
783 of 783 cases in every build, 107 Coax checks, and 1,842 of 1,842
mutations of the ECDH stage killed. That record also lists what it does not
cover: hidden compiler and register copies, target timing, and use with
production secrets.

## Summary

- Both sides of ECDH compute `a·b·G`; NIP-44 keeps its raw x, which is the
  same for either y of the peer's key.
- `ecdh_stage` refuses a peer x that is not below `p` or names no point, and
  a secret of 0 or `n` and above, with a masked, branch-free secret check.
- The conversation key is HKDF-extract of the raw x with salt `nip44-v2`,
  and it matches the published NIP-44 vectors.
- All 16 Wycheproof invalid keys that have an x-only form refuse; the other
  invalid cases either carry a valid x or have no x-only form.

## Dependencies

- [10. Scalar multiplication](ch010-scalar-multiplication.md)
- [12. Keys and randomness](ch012-keys-and-randomness.md)
- [6. HKDF](ch006-hkdf.md)

## Sources

- [NIP-44](../nips/official/44.md), `get_conversation_key`, and its pinned
  [nip44.vectors.json](../../coax/tests/vectors/upstream/nip44/nip44.vectors.json).
- Wycheproof
  [ecdh_secp256k1_test.json](../../coax/tests/vectors/upstream/wycheproof/ecdh_secp256k1_test.json),
  pinned in [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- [libsecp256k1](https://github.com/bitcoin-core/secp256k1), release
  `v0.8.0`, pinned in
  [UPSTREAM.json](../../tools/coax-nostr/fuzz/UPSTREAM.json).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
