# 19. Verify ECDSA

Every Bitcoin output spent before Taproot is unlocked by an ECDSA
signature: a P2PKH or P2WPKH spend carries one, and a multisig spend
carries several. This chapter builds ECDSA verification on secp256k1 from
the scalar and point code of chapters 14 to 16, adds the low-S rule that
Bitcoin uses against malleability, reads one strict DER signature, and
checks the result against all 464 cases of Wycheproof's Bitcoin ECDSA
vectors.

## What this chapter teaches

- The algorithm: check that `r` and `s` lie in `[1, n-1]`, compute
  `w = 1/s`, `u1 = z·w` and `u2 = r·w`, compute `R = u1·G + u2·Q`, and
  accept only when `R` is finite and `x(R) mod n = r`.
- Why a valid signature passes: the equation rebuilds the signer's nonce
  point `k·G`.
- Why `(r, n - s)` also passes, how that let anyone change a transaction's
  ID before segwit, and the low-S policy rule that refuses the high half.
- How to read a DER signature strictly, and where this chapter's reader
  stops and chapter 21's BIP-66 rules begin.
- Which step refuses each failing Wycheproof case.

## Where Bitcoin uses ECDSA

Bitcoin script checks ECDSA signatures with `OP_CHECKSIG`,
`OP_CHECKSIGVERIFY`, `OP_CHECKMULTISIG` and `OP_CHECKMULTISIGVERIFY` in
every output type before Taproot:

| Output | What the spender provides |
| --- | --- |
| P2PKH | a signature and a public key whose HASH160 (chapter 6) is in the output |
| P2WPKH | the same, in the segwit witness |
| P2SH and P2WSH multisig | `m` signatures for `m` of the `n` keys in the redeem or witness script |

Taproot outputs (segwit version 1) use BIP-340 Schnorr signatures instead
(chapter 23). The message is never the transaction itself. It is a 32-byte
signature hash, SHA-256d (chapter 5) of a serialization that chapters 37
and 38 build. This chapter's verifier takes that 32-byte hash as its input
`z`.

In a script, each signature is DER-encoded and followed by one sighash
type byte. The public key is a SEC1 point: 33 bytes compressed or 65 bytes
uncompressed, read by chapter 17's codec.

### Malleability and low-S

If `(r, s)` is a valid signature, so is `(r, n - s)`. Anyone who sees a
transaction can replace `s` with `n - s` without the key. Before segwit,
the signature was part of the data a txid hashes, so the change gave the
same transaction a new txid (chapter 35). Wallets that tracked payments by
txid could be fooled into paying twice.

BIP-62 listed this and other sources of malleability. The fix for this one
is the low-S rule: a signature's `s` must be at most `(n - 1) / 2`, the low
half. Bitcoin Core has enforced it as policy (`SCRIPT_VERIFY_LOW_S`) since
release 0.11.1 in 2015, so a high-S transaction does not relay. BIP-146
proposed making it consensus, but it was never deployed and its status is
Closed. Consensus still accepts high-S ECDSA signatures in every output
type, segwit v0 included. The verifier takes a `low_s` argument for this:
`true` applies the rule, `false` accepts both halves as consensus does.
Wycheproof's Bitcoin variant requires the rule.

### Public data, variable time

A verifier reads only public data: the hash, the public key and the
signature. No secret enters, so verification may branch and index on its
inputs, and every procedure here runs in variable time and ends in
`_vartime` (chapter 3). Signing (chapter 20) uses the secret key and the
nonce, and follows the constant-time rules.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [ecdsa_verify.coax](../../coax/lib/crypto/ecdsa_verify.coax) | `ecdsa_verify_vartime(hash, public_key, signature, low_s)` | verifies a compact `r \|\| s` under a SEC1 public key |
| | `ecdsa_verify_der_vartime(hash, public_key, der, low_s)` | the same, for one strict DER signature |
| | `ecdsa_der_read_vartime(der, out)` | reads a strict DER signature into `r \|\| s` |
| | `ev_scalars_vartime`, `ev_high_s_vartime` | the range checks and the low-S test |
| | `ev_prepare_vartime` | checks the scalars and computes `u1` and `u2` |
| | `ev_prepared_matches_vartime` | computes `R` and makes the final comparison |
| [secp_encode.coax](../../coax/lib/crypto/secp_encode.coax) | `se_decode(bytes)` | reads a SEC1 public key (chapter 17) |
| [secp_mult.coax](../../coax/lib/crypto/secp_mult.coax) | `ecmult_vartime(a, P, b)` | computes `a·G + b·P` as one sum (chapter 16) |

All inputs are byte views. The hash is 32 bytes, the compact signature is
64 bytes (`r` then `s`, each big-endian), and the public key is 33 or 65
bytes. A prepared record with `ready` true means only that preparation
succeeded; only the two complete entries decide that a signature is valid.

## The algorithm

ECDSA verification of the 32-byte hash `z` under the public key `Q` with
the signature `(r, s)`, as SEC 1 section 4.1.4 specifies it:

1. **Lengths.** The hash is 32 bytes and the signature 64. Anything else
   refuses.
2. **The key.** Read `Q` with chapter 17's codec. It refuses a point that
   is not on the curve, a coordinate at least `p`, the hybrid prefixes
   `0x06` and `0x07`, and any other length or prefix. Infinity has no
   SEC1 encoding here, so `Q` is never infinity.
3. **Range checks.** Refuse unless `1 ≤ r ≤ n - 1` and `1 ≤ s ≤ n - 1`.
   Neither is reduced: `r + n` is not another spelling of `r`.
4. **Low-S.** With `low_s`, refuse when `s > (n - 1) / 2`.
5. **The scalars.** Read `z` as a 256-bit integer and reduce it mod `n`.
   Compute `w = s^-1 mod n`, `u1 = z·w mod n` and `u2 = r·w mod n`.
6. **The point.** `R = u1·G + u2·Q`, which `ecmult_vartime(u1, Q, u2)`
   computes as one sum with a shared doubling chain.
7. **The comparison.** Refuse if `R` is infinity. Otherwise convert `R` to
   affine coordinates and accept exactly when `x(R) mod n = r`.

Steps 3 to 5 are `ev_prepare_vartime`, and steps 6 and 7 are
`ev_prepared_matches_vartime`:

<!-- source: coax/lib/crypto/ecdsa_verify.coax prepare -->
```jai
// u1 = z/s and u2 = r/s for the hash z, after the range and low-S checks.
// ready means only that preparation succeeded, not that the signature is valid.
ev_prepare_vartime :: (hash: [] u8, point: SgPoint, signature: [] u8, low_s: bool) -> EvPrepared {
  result: EvPrepared;
  if hash.count != 32 { return result; }
  scalars := ev_scalars_vartime(signature);
  if !scalars.ok { return result; }
  if low_s && ev_high_s_vartime(scalars.s) { return result; }
  z := sc_reduce_bytes(hash).value;                  // a hash at least n is reduced
  w := sc_inverse(scalars.s).value;                  // s is nonzero, so w exists
  return EvPrepared.{u1=sc_mul(z,w),u2=sc_mul(scalars.r,w),point=point,r=scalars.r,ready=true};
}
```

<!-- source: coax/lib/crypto/ecdsa_verify.coax matches -->
```jai
// R = u1·G + u2·Q. Accept only a finite R with x(R) mod n = r. Q must not
// be infinity: then R = u1·G, which anyone can match without a key.
ev_prepared_matches_vartime :: (prepared: EvPrepared) -> bool {
  zero: SecpField;
  if !prepared.ready || sf_equal_mask(prepared.point.z, zero) != 0 { return false; }
  // An invalid Q gives a refused result, which is canonical infinity.
  affine := sg_to_affine(ecmult_vartime(prepared.u1, prepared.point, prepared.u2).value).value;
  if affine.infinity != 0 { return false; }
  x: [32] u8;
  #assert sf_store(affine.x, x[..]);
  return sc_equal(sc_reduce_bytes(x[..]).value, prepared.r);   // x is below p; n < p
}
```

The entries put the steps together:

<!-- source: coax/lib/crypto/ecdsa_verify.coax verify -->
```jai
// The signature r || s (32 bytes each) of a 32-byte hash under a public key
// in SEC 1 form (33 bytes compressed or 65 uncompressed; chapter 17). With
// low_s, an s above (n-1)/2 refuses, as BIP-146 and Wycheproof's Bitcoin
// variant require; without it, both halves verify, as consensus allows.
ecdsa_verify_vartime :: (hash: [] u8, public_key: [] u8, signature: [] u8, low_s: bool) -> bool #export_c {
  key := se_decode(public_key);
  if !key.ok { return false; }
  return ev_prepared_matches_vartime(ev_prepare_vartime(hash, key.value, signature, low_s));
}
// The same for a strict DER signature (no sighash byte).
ecdsa_verify_der_vartime :: (hash: [] u8, public_key: [] u8, der: [] u8, low_s: bool) -> bool #export_c {
  signature: [64] u8;
  if !ecdsa_der_read_vartime(der, signature[..]) { return false; }
  return ecdsa_verify_vartime(hash, public_key, signature[..], low_s);
}
```

### Why the equation works

A signer with the secret key `d`, so `Q = d·G`, picks a nonce `k`,
publishes `r = x(k·G) mod n` and

```text
s = k^-1 · (z + r·d)  mod n
```

Solving for `k` gives `k = s^-1 · (z + r·d) = u1 + u2·d`, so

```text
u1·G + u2·Q = u1·G + u2·d·G = (u1 + u2·d)·G = k·G
```

and the verifier rebuilds the signer's nonce point without knowing `k` or
`d`. Its `x`, reduced mod `n`, is `r`.

Replacing `s` with `n - s` negates `w`, `u1` and `u2`, so the verifier
computes `-k·G`. A point and its negation share `x`, so the comparison
still passes. That is the malleability of the previous section, and the
low-S rule is the only check that sees it.

### Why x(R) is reduced mod n

`x(R)` is a field element, below `p`, and `r` is a scalar, below `n`. For
secp256k1, `n < p`, so an `x` in `[n, p)` reduces to `x - n`. That happens
for about one point in `2^128`. The verifier stores `x(R)` as 32 bytes and
reduces them with `sc_reduce_bytes`, the same reduction it applies to `z`,
then compares scalars. Wycheproof case 464 has `r = p - n + 1`, where
`r + n` is at least `p`: it checks that a verifier does not accept `r`
because `r + n` names the same field element.

### Worked example: small numbers

On secp256k1 every number has 77 digits. The same algebra works on a
small curve with the same equation, `y^2 = x^3 + 7`, over `F_43`. That
curve has 31 points, including infinity, so its group has the prime order
`n = 31`. Like secp256k1, `n < p`. Take `G = (2, 12)`, the secret key
`d = 7`, the nonce `k = 10` and the hash `z = 5`.

Signing:

1. `Q = 7G = (25, 18)`.
2. `R = 10G = (42, 7)`. Its `x` is 42, and `42 mod 31 = 11`, so `r = 11`.
   This is the reduction of the previous section.
3. `k^-1 mod 31 = 28`, because `10 · 28 = 280 = 9 · 31 + 1`.
4. `s = 28 · (5 + 11 · 7) = 28 · 82 mod 31 = 28 · 20 mod 31 = 2`.

Verifying `(r, s) = (11, 2)`:

1. `1 ≤ 11 ≤ 30` and `1 ≤ 2 ≤ 30`, and `2 ≤ (31 - 1) / 2 = 15`, so `s` is
   low.
2. `w = 2^-1 mod 31 = 16`, because `2 · 16 = 32 = 31 + 1`.
3. `u1 = 5 · 16 mod 31 = 18` and `u2 = 11 · 16 mod 31 = 21`.
4. `u1 + u2·d = 18 + 147 = 165 = 5 · 31 + 10`, so `R = 18G + 21Q = 10G =
   (42, 7)`.
5. `42 mod 31 = 11 = r`: the signature is valid.

The other half, `(11, 31 - 2) = (11, 29)`, gives `w = 15`, `u1 = 13` and
`u2 = 10`, so `R = 13G + 70G = 83G = 21G = (42, 36)`, the negation of `10G`.
Its `x` is still 42, so the signature verifies, and only the low-S rule
refuses it: `29 > 15`.

Two changes fail the comparison:

- The hash `z = 6`: `u1 = 6 · 16 mod 31 = 3`, so `R = 3G + 21Q = 150G =
  26G = (12, 31)`, and `12 ≠ 11`.
- The key `6G` instead of `7G`: `R = 18G + 21 · 6G = 144G = 20G = (40, 18)`,
  and `40 mod 31 = 9 ≠ 11`.

The example computes these numbers with a few lines of integer code, and
the checks sign and verify with every key and nonce on this curve.

### Worked example: a real signature

libsecp256k1 signs SHA-256 of `Satoshi Nakamoto` under the key `d = 1`,
so `Q = G`. `ecdsa_sign.coax` gives the same signature, byte for byte.

1. `z = a0dc65ff...7704883e`, below `n`, so it is used unchanged.
2. `r = 934b1ea1...210ee3d8` and `s = 2442ce9d...2aafd9e5`, both in
   `[1, n-1]`. `s` starts with `0x24`, below `0x7f`, so it is low.
3. `w = 1/s = fab47032...3a324a0f`.
4. `u1 = z·w = 5f5aa92c...af5520ce` and `u2 = r·w = 111b2f67...abefc35e`.
5. `R = u1·G + u2·G` has `x(R) = 934b1ea1...210ee3d8`, which equals `r`.

Here `u1 + u2 = 7075d893...5b44e42c`, which is `n - k` for the RFC 6979
nonce `k = 8f8a276c...74f15d15` that `ecdsa_sign.coax`'s own checks list.
The signer's first `s` was high, and it replaced `s` with `n - s`, so the
verifier rebuilds `-k·G` instead of `k·G`.

The values were computed with Python integers, which share no code with
the Coax implementation, and the checks compare them with the code's.

## DER, and where chapter 21 begins

Script carries a signature as an ASN.1 DER `SEQUENCE` of two `INTEGER`s:

```text
30 len  02 rlen r-bytes  02 slen s-bytes
```

A DER integer is signed, big-endian and minimal. A value whose first byte
is `0x80` or more gets a `0x00` in front, so a 32-byte `r` can take 33
bytes, and no other leading zero is allowed. `ecdsa_der_read_vartime`
accepts exactly one encoding for each `(r, s)`:

<!-- source: coax/lib/crypto/ecdsa_verify.coax der -->
```jai
// One INTEGER at der[at..]: 0x02, a length from 1 to 33, then the value,
// big-endian, not negative and with no needless leading zero. It is copied
// right-aligned into the 32 bytes of out. Returns the index after it.
ev_der_integer_vartime :: (der: [] u8, at: int, out: [] u8) -> EvInteger {
  if der.count - at < 3 || der[at] != 0x02 { return EvInteger.{}; }
  length := cast(int) der[at+1]; start := at + 2;
  if length < 1 || length > 33 || der.count - start < length { return EvInteger.{}; }
  if der[start] >= 0x80 { return EvInteger.{}; }                                     // negative
  if length > 1 && der[start] == 0 && der[start+1] < 0x80 { return EvInteger.{}; }  // a needless zero
  if length == 33 && der[start] != 0 { return EvInteger.{}; }                       // at least 2^256
  skip := ifx length == 33 then 1 else 0;
  for i: skip..<length { out[out.count-length+i] = der[start+i]; }
  return EvInteger.{end=start+length,ok=true};
}
// A DER signature, SEQUENCE { INTEGER r, INTEGER s } with nothing after it,
// read into the compact r || s. out is written only when the result is true.
// The range of r and s is checked later, by the verifier. The shortest
// signature is 8 bytes; two 33-byte integers make the longest, 72.
ecdsa_der_read_vartime :: (der: [] u8, out: [] u8) -> bool #export_c {
  if out.count != 64 || der.count < 8 { return false; }
  if der[0] != 0x30 || cast(int) der[1] != der.count - 2 { return false; }   // short-form length only
  compact: [64] u8;
  r := ev_der_integer_vartime(der, 2, compact[0..<32]);
  if !r.ok { return false; }
  s := ev_der_integer_vartime(der, r.end, compact[32..<64]);
  if !s.ok || s.end != der.count { return false; }
  for i: 0..<64 { out[i] = compact[i]; }
  return true;
}
```

It refuses a long-form length, a length that disagrees with the data, a
tag other than `0x30` or `0x02`, an empty, negative or padded integer, an
integer of more than 33 bytes, and any byte after the second integer. It
does not check the range of `r` and `s`: `30 06 02 01 00 02 01 01` reads
as `r = 0`, and step 3 refuses it.

That is where this chapter stops. Chapter 21 teaches BIP-66 as script
applies it: the sighash type byte after the DER bytes, the total size
bounds of 9 to 73 bytes, the history of the rule (consensus since block
363,725 in 2015, after OpenSSL versions disagreed about lax encodings),
and every DER case in Bitcoin Core's `script_tests.json`. Its checker is
the one script runs before it calls this verifier.

## The example

The example is [bitcoin/examples/ecdsa-verify.coax](../examples/ecdsa-verify.coax).
It works the toy curve, walks the real signature one step at a time, runs
all 464 Wycheproof cases and says which stage decides each one, and then
shows eleven refusals. Its vector table,
[coax/tests/bitcoin/ecdsa_vectors.coax](../../coax/tests/bitcoin/ecdsa_vectors.coax),
is copied from the pinned Wycheproof file by
[make_ecdsa_vectors.py](../../coax/tests/bitcoin/make_ecdsa_vectors.py),
which refuses a file whose SHA-256 differs from its pin.

<!-- source: bitcoin/examples/ecdsa-verify.coax example -->
```jai
#load "../../coax/lib/crypto/ecdsa_verify.coax";
#load "../../coax/lib/crypto/sha256.coax";
#load "../../coax/lib/nostr/hex.coax";
#load "../../coax/tests/bitcoin/ecdsa_vectors.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [160] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
show_scalar :: (label: string, s: ScScalar) {
  b: [32] u8;
  if sc_store(s, b[..]) { show(label, b[..]); }
}
// Decode hex into out and return the filled part. Every literal here and
// in the vector table is valid hex, so a decoding error stops the program.
decode :: (text: string, out: [] u8) -> [] u8 {
  r := hex_decode_checked(text, out, false);
  #assert r.error == 0;
  return out[0..<r.written];
}

// ---- a toy curve: y^2 = x^3 + 7 over F_43, whose 31 points form a group
// of prime order. Points are (x, y) with infinity as (-1, -1).
TOY_P :: 43;
TOY_N :: 31;
Toy :: struct { x: int; y: int; }
TOY_G :: Toy.{x=2,y=12};
TOY_INFINITY :: Toy.{x=-1,y=-1};
// a mod m in [0, m).
toy_mod :: (a: int, m: int) -> int { r := a % m; return ifx r < 0 then r + m else r; }
// The inverse of a mod a prime m, by Fermat: a^(m-2).
toy_inverse :: (a: int, m: int) -> int {
  r := 1;
  for i: 0..<m-2 { r = toy_mod(r * a, m); }
  return r;
}
toy_add :: (a: Toy, b: Toy) -> Toy {
  if a.x < 0 { return b; }
  if b.x < 0 { return a; }
  if a.x == b.x && toy_mod(a.y + b.y, TOY_P) == 0 { return TOY_INFINITY; }
  slope := ifx a.x == b.x then toy_mod(3 * a.x * a.x * toy_inverse(2 * a.y, TOY_P), TOY_P)
           else toy_mod((b.y - a.y) * toy_inverse(toy_mod(b.x - a.x, TOY_P), TOY_P), TOY_P);
  x := toy_mod(slope * slope - a.x - b.x, TOY_P);
  return Toy.{x=x,y=toy_mod(slope * (a.x - x) - a.y, TOY_P)};
}
// k·P by repeated addition: k is small.
toy_times :: (k: int, p: Toy) -> Toy {
  r := TOY_INFINITY;
  for i: 0..<k { r = toy_add(r, p); }
  return r;
}
// Toy verification of (r, s) on the hash z under Q, with or without low-S.
toy_verify :: (z: int, q: Toy, r: int, s: int, low_s: bool) -> bool {
  if r < 1 || r >= TOY_N || s < 1 || s >= TOY_N { return false; }
  if low_s && s > (TOY_N - 1) / 2 { return false; }
  w := toy_inverse(s, TOY_N);
  point := toy_add(toy_times(toy_mod(z * w, TOY_N), TOY_G), toy_times(toy_mod(r * w, TOY_N), q));
  return point.x >= 0 && point.x % TOY_N == r;
}

// ---- a Wycheproof case through the stages of the verifier
// Where the case is decided: "DER", "range", "low-S", "equation" or
// "accepted". The first three stages are the steps ecdsa_verify_der_vartime
// takes before the multiplication; the complete entry decides the rest.
stage :: (c: EvCase) -> string {
  key: [65] u8; msg: [32] u8; der: [4200] u8; hash: [32] u8; sig: [64] u8;
  #assert sha256_hash_checked(decode(c.msg, msg[..]), hash[..], false) == SHA_OK;
  encoded := decode(c.sig, der[..]);
  if !ecdsa_der_read_vartime(encoded, sig[..]) { return "DER"; }
  scalars := ev_scalars_vartime(sig[..]);
  if !scalars.ok { return "range"; }
  if ev_high_s_vartime(scalars.s) { return "low-S"; }
  if !ecdsa_verify_der_vartime(hash[..], decode(EV_KEYS[c.key], key[..]), encoded, true) { return "equation"; }
  return "accepted";
}
STAGES :: [5] string.["DER", "range", "low-S", "equation", "accepted"];
FLAGS :: [18] string.["ValidSignature", "SignatureMalleabilityBitcoin", "BerEncodedSignature",
  "InvalidEncoding", "InvalidTypesInSignature", "ModifiedInteger", "IntegerOverflow", "RangeCheck",
  "InvalidSignature", "ModifiedSignature", "ArithmeticError", "SmallRandS,ArithmeticError",
  "ModularInverse,ArithmeticError", "PointDuplication", "PointDuplication,ArithmeticError",
  "EdgeCaseShamirMultiplication", "SpecialCaseHash", "EdgeCasePublicKey"];

// A real signature: libsecp256k1's, for the key 1 and SHA-256("Satoshi Nakamoto").
KEY_1 :: "0479be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8";
KEY_2 :: "02c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5";
SIG_1 :: "934b1ea10a4b3c1757e2b0c017d0b6143ce3c9a7e6a4a49860d7a6ab210ee3d82442ce9d2b916064108014783e923ec36b49743e2ffa1c4496f01a512aafd9e5";
N_HEX :: "fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141";

main :: () {
  // The toy curve by hand: key d = 7, nonce k = 10, hash z = 5.
  print("toy curve y^2 = x^3 + 7 mod 43, n = 31, G = (2, 12)\n");
  q := toy_times(7, TOY_G); big_r := toy_times(10, TOY_G);
  r := big_r.x % TOY_N;
  s := toy_mod(toy_inverse(10, TOY_N) * (5 + r * 7), TOY_N);
  print("Q = 7G = (%, %), R = 10G = (%, %)\n", q.x, q.y, big_r.x, big_r.y);
  print("r = % mod 31 = %, s = (5 + r*7) / 10 mod 31 = %\n", big_r.x, r, s);
  w := toy_inverse(s, TOY_N); u1 := toy_mod(5 * w, TOY_N); u2 := toy_mod(r * w, TOY_N);
  sum := toy_add(toy_times(u1, TOY_G), toy_times(u2, q));
  print("w = %, u1 = %, u2 = %, u1*G + u2*Q = (%, %)\n", w, u1, u2, sum.x, sum.y);
  print("verify (r, s): %\n", toy_verify(5, q, r, s, true));
  print("verify (r, n - s) = (%, %): %, under low-S: %\n", r, TOY_N - s, toy_verify(5, q, r, TOY_N - s, false), toy_verify(5, q, r, TOY_N - s, true));
  print("hash 6: %, key 6G: %\n", toy_verify(6, q, r, s, true), toy_verify(5, toy_times(6, TOY_G), r, s, true));

  // The same steps on secp256k1, for libsecp256k1's signature of
  // SHA-256("Satoshi Nakamoto") under the key 1, so Q = G.
  print("secp256k1, key 1, SHA-256(\"Satoshi Nakamoto\")\n");
  key: [65] u8; sig: [64] u8; hash: [32] u8; text := "Satoshi Nakamoto"; message: [16] u8;
  for i: 0..<16 { message[i] = text[i]; }
  #assert sha256_hash_checked(message[..], hash[..], false) == SHA_OK;
  public_key := decode(KEY_1, key[..]); signature := decode(SIG_1, sig[..]);
  show("z", hash[..]);
  scalars := ev_scalars_vartime(signature);
  print("r and s in [1, n-1]: %, s low: %\n", scalars.ok, !ev_high_s_vartime(scalars.s));
  prepared := ev_prepare_vartime(hash[..], sg_generator(), signature, true);   // Q = 1·G
  show_scalar("w = 1/s", sc_inverse(scalars.s).value);
  show_scalar("u1 = z*w", prepared.u1);
  show_scalar("u2 = r*w", prepared.u2);
  sum_r := sg_to_affine(ecmult_vartime(prepared.u1, prepared.point, prepared.u2).value).value;
  x: [32] u8; #assert sf_store(sum_r.x, x[..]);
  show("x(R)", x[..]);
  show("r", signature[0..<32]);
  print("ecdsa_verify_vartime: %\n", ecdsa_verify_vartime(hash[..], public_key, signature, true));

  // All 464 Wycheproof cases, by flag, with the stage that decides each.
  print("Wycheproof ecdsa_secp256k1_sha256_bitcoin_test.json\n");
  agree := 0; at_stage: [5] int; by_flag: [18] int; flag_agree: [18] int;
  for i: 0..<EV_CASES.count {
    c := EV_CASES[i]; where := stage(c);
    for k: 0..<5 { if where == STAGES[k] { at_stage[k] += 1; } }
    ok := (where == "accepted") == c.valid;
    if ok { agree += 1; }
    matched := 0;
    for f: 0..<18 { if c.flags == FLAGS[f] { matched += 1; by_flag[f] += 1; if ok { flag_agree[f] += 1; } } }
    #assert matched == 1;
  }
  for f: 0..<18 { print("%: % of % as expected\n", FLAGS[f], flag_agree[f], by_flag[f]); }
  for k: 0..<4 { print("refused at %: %\n", STAGES[k], at_stage[k]); }
  print("accepted: %\n", at_stage[4]);
  print("% of % cases give the expected result\n", agree, EV_CASES.count);

  // Refusals, each from the real signature above.
  print("refusals\n");
  bad := sig;
  for i: 0..<32 { bad[i] = 0; }
  print("r = 0: %\n", ecdsa_verify_vartime(hash[..], public_key, bad[..], true));
  #assert decode(N_HEX, bad[0..<32]).count == 32;
  print("r = n: %\n", ecdsa_verify_vartime(hash[..], public_key, bad[..], true));
  bad = sig; #assert decode(N_HEX, bad[32..<64]).count == 32;
  print("s = n: %\n", ecdsa_verify_vartime(hash[..], public_key, bad[..], true));
  high := sig; high_s := sc_negate(scalars.s); #assert sc_store(high_s, high[32..<64]);
  print("s replaced by n - s, low-S rule: %, without it: %\n",
    ecdsa_verify_vartime(hash[..], public_key, high[..], true), ecdsa_verify_vartime(hash[..], public_key, high[..], false));
  other: [33] u8;
  print("wrong key, 2G: %\n", ecdsa_verify_vartime(hash[..], decode(KEY_2, other[..]), signature, true));
  changed := hash; changed[0] ^= 1;
  print("hash with one bit changed: %\n", ecdsa_verify_vartime(changed[..], public_key, signature, true));
  print("31-byte hash: %\n", ecdsa_verify_vartime(hash[0..<31], public_key, signature, true));
  print("63-byte signature: %\n", ecdsa_verify_vartime(hash[..], public_key, signature[0..<63], true));
  off := key; off[64] ^= 1;
  print("key off the curve: %\n", ecdsa_verify_vartime(hash[..], off[..], signature, true));
  der: [80] u8; out: [64] u8;
  print("DER with a trailing byte: %\n", ecdsa_der_read_vartime(decode("300602010102010200", der[..]), out[..]));
  print("DER with a needless zero: %\n", ecdsa_der_read_vartime(decode("300702020001020102", der[..]), out[..]));
}
```

Run it from the repository root:

```shell
coax/bin/coax run bitcoin/examples/ecdsa-verify.coax
```

The output shows the toy numbers worked out by hand, the real
signature's `z`, `w`, `u1`, `u2` and `x(R) = r`, every Wycheproof flag with
all its cases as expected, and the refusals:

```text
toy curve y^2 = x^3 + 7 mod 43, n = 31, G = (2, 12)
Q = 7G = (25, 18), R = 10G = (42, 7)
r = 42 mod 31 = 11, s = (5 + r*7) / 10 mod 31 = 2
w = 16, u1 = 18, u2 = 21, u1*G + u2*Q = (42, 7)
verify (r, s): true
verify (r, n - s) = (11, 29): true, under low-S: false
hash 6: false, key 6G: false
secp256k1, key 1, SHA-256("Satoshi Nakamoto")
z: a0dc65ffca799873cbea0ac274015b9526505daaaed385155425f7337704883e
r and s in [1, n-1]: true, s low: true
w = 1/s: fab47032eeb0b7ec2328dfae35b7d11bd087d516840ecd98bbd6a8be3a324a0f
u1 = z*w: 5f5aa92cdd8788480fc88a00e9bdccece7b4c4a3f7a3e748d597da03af5520ce
u2 = r*w: 111b2f67088463219984f59cf80ca643ae04231d752d9260fbd17e4dabefc35e
x(R): 934b1ea10a4b3c1757e2b0c017d0b6143ce3c9a7e6a4a49860d7a6ab210ee3d8
r: 934b1ea10a4b3c1757e2b0c017d0b6143ce3c9a7e6a4a49860d7a6ab210ee3d8
ecdsa_verify_vartime: true
Wycheproof ecdsa_secp256k1_sha256_bitcoin_test.json
ValidSignature: 5 of 5 as expected
SignatureMalleabilityBitcoin: 1 of 1 as expected
BerEncodedSignature: 7 of 7 as expected
InvalidEncoding: 89 of 89 as expected
InvalidTypesInSignature: 63 of 63 as expected
ModifiedInteger: 5 of 5 as expected
IntegerOverflow: 5 of 5 as expected
RangeCheck: 6 of 6 as expected
InvalidSignature: 64 of 64 as expected
ModifiedSignature: 49 of 49 as expected
ArithmeticError: 69 of 69 as expected
SmallRandS,ArithmeticError: 6 of 6 as expected
ModularInverse,ArithmeticError: 15 of 15 as expected
PointDuplication: 6 of 6 as expected
PointDuplication,ArithmeticError: 1 of 1 as expected
EdgeCaseShamirMultiplication: 1 of 1 as expected
SpecialCaseHash: 54 of 54 as expected
EdgeCasePublicKey: 18 of 18 as expected
refused at DER: 232
refused at range: 50
refused at low-S: 4
refused at equation: 16
accepted: 162
464 of 464 cases give the expected result
refusals
r = 0: false
r = n: false
s = n: false
s replaced by n - s, low-S rule: false, without it: true
wrong key, 2G: false
hash with one bit changed: false
31-byte hash: false
63-byte signature: false
key off the curve: false
DER with a trailing byte: false
DER with a needless zero: false
```

Each Wycheproof case is decided at one stage:

| Stage | Cases | What fails there |
| --- | --- | --- |
| DER | 232 | BER and other non-canonical encodings, wrong tags and lengths, trailing bytes, integers padded, negative or too long |
| range | 50 | `r` or `s` equal to 0, `n`, `p` or more, including values that equal a valid `r` or `s` once reduced |
| low-S | 4 | case 1 (the malleated signature), case 388 (an edge case of it), and cases 176 and 200 with `s = n - 1` |
| equation | 16 | changed bytes of `r` or `s`, `R` at infinity, keys that share `x` with `G`, and case 464's `r + n ≥ p` |

The remaining 162 cases are valid and are accepted, among them `r` and `s`
as small as 1 to 3, hashes with long runs of zero or one bits, `s` with
unusual inverses, and keys with special coordinates.

The refusals, each a change to the real signature:

- `r = 0`, `r = n` and `s = n`: the range checks refuse them.
- `s` replaced by `n - s`: refused under the low-S rule and accepted
  without it.
- The key `2G`, or a hash with one bit changed: the equation gives another
  `R`.
- A 31-byte hash or a 63-byte signature: refused before any arithmetic.
- A key with its last byte changed: chapter 17's codec finds it off the
  curve.
- A DER signature with a trailing byte, or with a needless leading zero:
  the reader refuses it.

The book's checks run this program in debug and release builds and compare
the output with [bitcoin/fixtures/ecdsa-verify.out](../fixtures/ecdsa-verify.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/ecdsa-verify.coax](../examples/checks/ecdsa-verify.coax),
which loads the example:

- on the toy curve, for every key and every nonce, the signature verifies,
  exactly one of `(r, s)` and `(r, n - s)` passes the low-S rule, and the
  next hash refuses;
- the real signature's `w`, `u1` and `u2`, from Python integers;
- every Wycheproof case that a stage before the equation refuses is
  expected to fail, and there are 286 of them;
- the five `ValidSignature` cases verify through the DER entry, and four
  cases that only the equation refuses are refused: infinity during
  verification, a comparison with infinity, a key that shares `x` with `G`,
  and `r + n ≥ p`;
- the four signatures that `ecdsa_sign.coax`'s checks pin, each equal to
  libsecp256k1's, verify under compressed keys and refuse under another
  key.

<!-- source: bitcoin/examples/checks/ecdsa-verify.coax check -->
```jai
// On the toy curve, every key d and nonce k give a signature that
// verifies; its other half (r, n - s) verifies only without the low-S rule
// (s is never n/2 for odd n); the next hash or the next key refuses unless
// it happens to give the same x.
#forall "toy signatures verify for every key and nonce" d: 1..<TOY_N #steps 400000000 {
  q := toy_times(d, TOY_G); refused := 0;
  for k: 1..<TOY_N {
    r := toy_times(k, TOY_G).x % TOY_N;
    s := toy_mod(toy_inverse(k, TOY_N) * (5 + r * d), TOY_N);
    if s == 0 { continue; }
    #assert toy_verify(5, q, r, s, false) && toy_verify(5, q, r, TOY_N - s, false);
    #assert toy_verify(5, q, r, s, true) != toy_verify(5, q, r, TOY_N - s, true);
    if !toy_verify(6, q, r, s, false) { refused += 1; }
  }
  #assert refused > 20;
}
equal32 :: (a: [] u8, b: [] u8) -> bool {
  if a.count != 32 || b.count != 32 { return false; }
  for i: 0..<32 { if a[i] != b[i] { return false; } }
  return true;
}
#test "equal32 notices a change in the first or last byte and a short view" {
  a: [32] u8; b := a;
  #assert equal32(a[..], b[..]) && !equal32(a[0..<31], b[0..<31]) && !equal32(a[..], b[0..<31]) && !equal32(a[0..<31], b[..]);
  b[0] = 1; #assert !equal32(a[..], b[..]);
  b = a; b[31] = 1; #assert !equal32(a[..], b[..]);
}
// The worked example's values, computed with Python integers, which share
// no code with the Coax field, scalar or point arithmetic.
#test "the worked example matches hand-computed values" #steps 1000000000 {
  sig: [64] u8; hash: [32] u8; got: [32] u8; want: [32] u8;
  signature := decode(SIG_1, sig[..]);
  z := decode("a0dc65ffca799873cbea0ac274015b9526505daaaed385155425f7337704883e", hash[..]);
  prepared := ev_prepare_vartime(z, sg_generator(), signature, true);
  #assert prepared.ready;
  #assert sc_store(sc_inverse(ev_scalars_vartime(signature).s).value, got[..]) && equal32(got[..], decode("fab47032eeb0b7ec2328dfae35b7d11bd087d516840ecd98bbd6a8be3a324a0f", want[..]));
  #assert sc_store(prepared.u1, got[..]) && equal32(got[..], decode("5f5aa92cdd8788480fc88a00e9bdccece7b4c4a3f7a3e748d597da03af5520ce", want[..]));
  #assert sc_store(prepared.u2, got[..]) && equal32(got[..], decode("111b2f67088463219984f59cf80ca643ae04231d752d9260fbd17e4dabefc35e", want[..]));
  #assert ev_prepared_matches_vartime(prepared);
}
// Every Wycheproof case refused before the equation (DER, range or low-S)
// is one the file expects to fail, and there are 286 of them: no stage
// before the multiplication refuses a valid signature.
#test "the early stages refuse 286 cases, all expected to fail" #steps 2000000000 {
  early := 0;
  for i: 0..<EV_CASES.count {
    c := EV_CASES[i]; der: [4200] u8; sig: [64] u8;
    d := decode(c.sig, der[..]);
    refused := !ecdsa_der_read_vartime(d, sig[..]);
    if !refused { scalars := ev_scalars_vartime(sig[..]); refused = !scalars.ok || ev_high_s_vartime(scalars.s); }
    if refused { #assert !c.valid; early += 1; }
  }
  #assert early == 286;
}
// The five ValidSignature cases verify through the complete DER entry, and
// four cases that only the equation refuses are refused: tcId 386 (infinity
// during verification), 425 (comparison with infinity), 438 (a key that
// shares x with G) and 464 (r = p - n + 1, where r + n is not below p).
EQUATION :: [9] int.[1, 441, 442, 443, 444, 385, 424, 437, 463];
#forall "Wycheproof cases through the complete entry" i: 0..<9 #steps 4000000000 {
  c := EV_CASES[EQUATION[i]];
  key: [65] u8; msg: [32] u8; der: [128] u8; hash: [32] u8;
  #assert sha256_hash_checked(decode(c.msg, msg[..]), hash[..], false) == SHA_OK;
  #assert ecdsa_verify_der_vartime(hash[..], decode(EV_KEYS[c.key], key[..]), decode(c.sig, der[..]), true) == c.valid;
  #assert (i < 5) == c.valid;
}
// The four signatures ecdsa_sign.coax's own checks pin (each equal to
// libsecp256k1's, byte for byte) verify under compressed keys computed with
// Python integers, and refuse under the next key in the list.
SIGNED :: [12] string.[
  "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798", "a0dc65ffca799873cbea0ac274015b9526505daaaed385155425f7337704883e",
  "934b1ea10a4b3c1757e2b0c017d0b6143ce3c9a7e6a4a49860d7a6ab210ee3d82442ce9d2b916064108014783e923ec36b49743e2ffa1c4496f01a512aafd9e5",
  "0379be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798", "a0dc65ffca799873cbea0ac274015b9526505daaaed385155425f7337704883e",
  "fd567d121db66e382991534ada77a6bd3106f0a1098c231e47993447cd6af2d06b39cd0eb1bc8603e159ef5c20a5c8ad685a45b06ce9bebed3f153d10d93bed5",
  "031b38903a43f7f114ed4500b4eac7083fdefece1cf29c63528d563446f972c180", "0000000000000000000000000000000000000000000000000000000000000000",
  "db2767ec4312c496a057e4501efbd4cafbb5451a708e1027f35c83e254c6567b0531cba6e4b09efb2fcb072a4ea5f2bc4334a841d01504e7249a19fdd98c6823",
  "025476c2e83188368da1ff3e292e7acafcdb3566bb0ad253f62fc70f07aeee6357", "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
  "9b550d9f13d80d2fd8b82ef12c67af7a0bbd413366dd462b96e530ced8bbced36a1b31f281e336c497dcee0a187f6d5bfe165c34ecbdb91ac72da934f32a1441"];
#forall "signatures from ecdsa_sign.coax verify and refuse the wrong key" i: 0..<4 #steps 4000000000 {
  key: [33] u8; other: [33] u8; hash: [32] u8; sig: [64] u8;
  k := decode(SIGNED[3*i], key[..]); h := decode(SIGNED[3*i+1], hash[..]); s := decode(SIGNED[3*i+2], sig[..]);
  #assert ecdsa_verify_vartime(h, k, s, true);
  #assert !ecdsa_verify_vartime(h, decode(SIGNED[3*((i+1)%4)], other[..]), s, true);
}
```

To run them:

```shell
coax/bin/coax test bitcoin/examples/checks/ecdsa-verify.coax
coax/bin/coax mutate bitcoin/examples/checks/ecdsa-verify.coax --root-only
```

`coax test` runs 121 checks, including the loaded files' own checks, in
about 4 minutes on 2026-11-02. A full verification takes about 80 million
interpreter steps, so the checks that verify have budgets of up to 4
billion steps. Root-only mutation kills all 13 mutants in the checks file;
the example's own procedures were not mutated for this chapter.

`ecdsa_verify.coax` has its own checks: the range of `r` and `s` at 0, 1,
`n - 1` and `n`, the low-S boundary at `(n - 1) / 2`, and the DER reader on
the canonical form and on each fault alone, with the output unwritten
after a refusal. `coax mutate coax/lib/crypto/ecdsa_verify.coax --root-only` kills all 149
mutants on 2026-11-02. The checks there include five full verifications:
the real signature, the same with one hash bit changed and with `s` high,
Wycheproof case 346 (valid, with `x(R)` at least `n`) and case 386 (`R` at
infinity), and a prepared record with `Q` at infinity that only the
infinity check refuses.

### Wycheproof, Python and libsecp256k1, natively and in Wasm

The interpreter runs a full verification in tens of millions of steps, so
the complete runs use native code.
[ecdsa_verify.py](../../coax/tests/bitcoin/ecdsa_verify.py) compiles
`ecdsa_verify.coax` to C in checked and release builds and compares every
decision with two independent verifiers: a Python one with affine
integers and its own strict DER reader, and libsecp256k1 `v0.8.0`
(`secp256k1_ecdsa_signature_parse_der`, `_normalize` and `_verify`). It
runs:

- all 464 Wycheproof cases with the low-S rule, each key both uncompressed
  and compressed;
- each of the 162 valid cases without the rule, and with `s` replaced by
  `n - s`, which must refuse under the rule and verify without it;
- 60 seeded signatures, half made by `ecdsa_sign.coax`'s own C build (each
  equal to libsecp256k1's) and half by libsecp256k1, each accepted and then
  refused with one bit of the hash, `r` or `s` changed and under the wrong
  key.

It then builds the same C for wasm32 and runs the 464 cases with both key
forms in Node. On 2026-11-02 every case gave the same result in all four Coax builds,
the Python verifier and libsecp256k1: the 464 Wycheproof cases, three
low-S variants of each of the 162 valid ones, and 60 signatures with four
changes each. The run took 290 seconds.

```shell
python3 tools/coax-nostr/fuzz/build_oracle.py --fetch --reviewed   # once: libsecp256k1 v0.8.0
python3 coax/tests/bitcoin/ecdsa_verify.py
```

## Summary

- Verification checks `r` and `s` in `[1, n-1]`, computes `u1 = z/s` and
  `u2 = r/s`, and accepts only a finite `R = u1·G + u2·Q` with
  `x(R) mod n = r`.
- `(r, n - s)` verifies whenever `(r, s)` does. Bitcoin's low-S rule
  (BIP-146) refuses the high half; consensus still accepts it, so the rule
  is the caller's choice.
- The DER reader accepts exactly one encoding per signature. BIP-66's
  script rules, including the sighash byte, are chapter 21's.
- All 464 Wycheproof cases give their expected result, natively and in
  Wasm, and Python and libsecp256k1 agree on every case.

## Dependencies

- [5. SHA-256d and tagged hashes](ch005-sha-256d-and-tagged-hashes.md)
- [14. Scalars mod n](ch014-scalars-mod-n.md)
- [16. Scalar multiplication](ch016-scalar-multiplication.md)
- [17. Point encodings: compressed, uncompressed and x-only](ch017-point-encodings-compressed-uncompressed-and-x-only.md)

## Sources

- [SEC 1 v2](https://www.secg.org/sec1-v2.pdf), section 4.1.4 (ECDSA
  verification) and section 2.3.3 (point encodings).
- [BIP-146](https://github.com/bitcoin/bips/blob/master/bip-0146.mediawiki)
  (low-S) and
  [BIP-62](https://github.com/bitcoin/bips/blob/master/bip-0062.mediawiki)
  (the malleability it lists).
- [BIP-66](https://github.com/bitcoin/bips/blob/master/bip-0066.mediawiki),
  for chapter 21.
- Wycheproof `ecdsa_secp256k1_sha256_bitcoin_test.json`, pinned at
  revision `d17cc89f` in
  [coax/tests/vectors/upstream/wycheproof/](../../coax/tests/vectors/upstream/wycheproof/ecdsa_secp256k1_sha256_bitcoin_test.json),
  listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- [libsecp256k1](https://github.com/bitcoin-core/secp256k1), release
  `v0.8.0`, pinned in
  [UPSTREAM.json](../../tools/coax-nostr/fuzz/UPSTREAM.json); tests only.
