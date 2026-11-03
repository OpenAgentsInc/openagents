# 20. Sign with ECDSA

Chapter 19 checks an ECDSA signature. This chapter makes one. Signing
uses the private key and a nonce, and both are secrets: a nonce that
repeats, or one an attacker can partly predict, gives the key away. The
signer here derives its nonce with RFC 6979 from the key and the hash, keeps
`s` in the low half as Bitcoin Core does, follows the constant-time rules of
chapter 3, and gives the same bytes as libsecp256k1 on all 2,020 signing
vectors of tiny-secp256k1's `ecdsa.json`.

## What this chapter teaches

- The signing equation: `r = x(k·G) mod n` and `s = k^-1 · (z + r·d) mod n`.
- RFC 6979's deterministic nonce: an HMAC-SHA256 generator (chapter 8)
  seeded with the key and the hash.
- How the signer avoids RFC 6979's loop: it always computes two candidates
  and picks the first valid one with a mask.
- Low-S: why the signer replaces `s` with `n - s` in the high half, and how
  it does that without a branch.
- Why one nonce for two hashes, a nonce that leaks, or a nonce from a small
  range gives the private key to anyone who sees the signatures.
- The one fact the signer reveals inside the call, and how a Memcheck probe
  checks that no other branch or address depends on the key.

## Where Bitcoin signs with ECDSA

Every pre-Taproot spend carries an ECDSA signature: P2PKH, P2WPKH, and the
`m` signatures of a P2SH or P2WSH multisig (chapter 19 lists them). The
signer never sees the transaction. It signs a 32-byte signature hash `z`
that chapters 37 and 38 compute, and script carries the result in DER with
a sighash type byte after it.

Bitcoin Core signs with libsecp256k1's `secp256k1_ecdsa_sign`: RFC 6979
nonces and low-S output. Two signers that follow the same rules give the
same signature for the same key and hash, so this chapter can check its
output byte for byte against libsecp256k1, not only check that it verifies.
Bitcoin Core's wallet adds one step, covered under "Extra entropy" below:
when `r` has its top bit set, it signs again with extra data until `r` is
low.

[Nostr chapter 104](../../nostr/book/ch104-agent-treasuries-and-a-network-of-keys.md)
wrote this signer for its agent treasuries, and
`coax/lib/bitcoin/sighash.coax` and `coax/lib/nostr/nipkeys_sign.coax` use
it. This chapter audits it, teaches it and adds its vectors and checks.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [ecdsa_sign.coax](../../coax/lib/crypto/ecdsa_sign.coax) | `ecdsa_sign(key, hash, w, out, overlap)` | signs a 32-byte hash into the compact `r \|\| s` (`#authority`) |
| | `ec_candidates`, `ec_seed`, `ec_step`, `ec_next`, `ec_hmac` | RFC 6979's HMAC-SHA256 generator: the first two nonce candidates |
| | `ec_pick` | the first candidate in `[1, n-1]`, chosen with a mask |
| | `ec_sign` | the signer body that `ecdsa_sign` and chapter 22's `ecdsa_sign_recoverable` share |
| | `ec_high`, `ec_low` | whether `s` is in the high half, and `n - s` then, chosen with a mask |
| | `ec_declassify`, `ec_clear` | the one revealed fact, and the workspace erasure |
| | `ecdsa_der(sig, out)` | a minimal DER encoder for the public result |
| [hmac.coax](../../coax/lib/crypto/hmac.coax) | `hmac_sha256_checked` | HMAC-SHA256 (chapter 8) |
| [schnorr_scalar_workspace.coax](../../coax/lib/crypto/schnorr_scalar_workspace.coax) | `ss_key` | loads a secret scalar without a branch (chapter 24) |
| [schnorr_point_workspace.coax](../../coax/lib/crypto/schnorr_point_workspace.coax) | `sp_base`, `sp_x` | `k·G` with the fixed-base comb (chapter 16) and its `x` |

The caller owns one `EcdsaWork`, a struct that holds every secret value the
signer keeps: RFC 6979's `K` and `V`, both candidates, the key, the nonce,
its inverse and the nonce point. The signer writes `out` only when it
returns true, and it clears the workspace on every return.

## The algorithm

Signing the 32-byte hash `h` with the secret key `d`. `G` is the generator
and `n` the group order:

1. **Views.** The key and the hash are 32 bytes, `out` is 64 and the
   workspace holds one `EcdsaWork`. Anything else refuses before any work.
2. **The key.** Refuse unless `0 < d < n`.
3. **The nonce.** RFC 6979 section 3.2 with HMAC-SHA256 gives `k` in
   `[1, n-1]`.
4. **The nonce point.** `R = k·G` and `r = x(R) mod n`. Refuse if `r = 0`.
5. **The equation.** `z = h mod n`, then `s = k^-1 · (z + r·d) mod n`.
   Refuse if `s = 0`.
6. **Low-S.** If `s > (n - 1) / 2`, replace `s` with `n - s`.
7. **Output.** Write `r || s`, 32 bytes each, big-endian.

Chapter 19 shows why a verifier accepts this: it computes
`s^-1·z·G + s^-1·r·Q`, which is `k·G`. Step 6 negates `s`, so the verifier
rebuilds `-k·G`, which has the same `x`.

### RFC 6979

A nonce must be secret, unique per message and uniform in `[1, n-1]`. A
random number generator gives that only while it works. RFC 6979 derives
`k` instead, with an HMAC-based generator (HMAC_DRBG) seeded with the key
and the hash. The same key and hash always give the same `k`, and anyone
without the key can't predict it.

For secp256k1 and SHA-256, the group order and the hash are both 256 bits,
so RFC 6979's `bits2int` changes nothing, and `bits2octets(h)` is `h mod n`.
Section 3.2's steps are:

1. **b, c.** `V = 01 01 ... 01` and `K = 00 00 ... 00`, 32 bytes each.
2. **d, e.** `K = HMAC_K(V || 00 || x || h)`, then `V = HMAC_K(V)`. Here
   `x` is the key and `h` the reduced hash, 32 bytes each.
3. **f, g.** `K = HMAC_K(V || 01 || x || h)`, then `V = HMAC_K(V)`.
4. **h.** `V = HMAC_K(V)`. If `V`, read as a number, is in `[1, n-1]`, it
   is `k`. Otherwise `K = HMAC_K(V || 00)`, `V = HMAC_K(V)`, and repeat
   step h.

The generator's three moves are small procedures over the workspace. `buf`
holds `V || tag || x || h`, so one HMAC call covers both lengths:

<!-- source: coax/lib/crypto/ecdsa_sign.coax drbg -->
```jai
// One HMAC-SHA256 under K over buf[0..<n], into out (never K itself).
ec_hmac :: (w: [] EcdsaWork, n: int, out: [] u8) {
  #assert hmac_sha256_checked(w[0].hk[..], w[0].buf[0..<n], out, false) == SHA_OK;
}
// V = HMAC_K(V).
ec_next :: (w: [] EcdsaWork) {
  for i: 0..<32 { w[0].buf[i] = w[0].hv[i]; }
  ec_hmac(w, 32, w[0].hv[..]);
}
// K = HMAC_K(V || tag [|| key || h]), then V = HMAC_K(V). key and h are
// already in buf[33..<97]; n is 33 without them and 97 with them.
ec_step :: (w: [] EcdsaWork, tag: u8, n: int) {
  for i: 0..<32 { w[0].buf[i] = w[0].hv[i]; }
  w[0].buf[32] = tag;
  ec_hmac(w, n, w[0].t[..]);
  for i: 0..<32 { w[0].hk[i] = w[0].t[i]; }
  ec_next(w);
}
```

`ec_seed` runs the steps and keeps the first two candidates. `ec_candidates`
is `ec_seed` with no extra data:

<!-- source: coax/lib/crypto/ecdsa_sign.coax rfc6979 -->
```jai
// RFC 6979 section 3.2 for SHA-256 and secp256k1's 256-bit order: the
// first two candidates for key and hash, into w.first and w.second.
// bits2octets(h) is h mod n. data is empty or 32 bytes of extra data
// (section 3.6). buf holds V || tag || key || h || data.
ec_candidates :: (key: [] u8, hash: [] u8, w: [] EcdsaWork) { ec_seed(key, hash, hash[0..<0], w); }
ec_seed :: (key: [] u8, hash: [] u8, data: [] u8, w: [] EcdsaWork) {
  z := sc_reduce_bytes(hash);                      // the message hash is public
  for i: 0..<32 { w[0].hv[i] = 1; w[0].hk[i] = 0; w[0].buf[33+i] = key[i]; }   // b, c: V = 01..01, K = 00..00
  #assert ss_write(z.value, w[0].buf[65..<97]);
  for i: 0..<data.count { w[0].buf[97+i] = data[i]; }   // public: a counter
  n := 97 + data.count;
  ec_step(w, 0, n);                                // d, e: K = HMAC_K(V || 00 || key || h || data), V = HMAC_K(V)
  ec_step(w, 1, n);                                // f, g: K = HMAC_K(V || 01 || key || h || data), V = HMAC_K(V)
  ec_next(w);                                      // h: V = HMAC_K(V), the first candidate
  for i: 0..<32 { w[0].first[i] = w[0].hv[i]; }
  ec_step(w, 0, 33);                               // h, retry: K = HMAC_K(V || 00), V = HMAC_K(V)
  ec_next(w);                                      // V = HMAC_K(V), the second candidate
  for i: 0..<32 { w[0].second[i] = w[0].hv[i]; }
}
```

### Two candidates, chosen with a mask

RFC 6979's step h is a loop whose length depends on the key: a candidate
at least `n` sends it around again. A loop whose trip count depends on a
secret breaks chapter 3's rules. For secp256k1, `n` is within `2^129` of
`2^256`, so a candidate falls outside `[1, n-1]` with probability below
`2^-127`.

The signer always derives exactly two candidates and selects with a mask.
`ss_key` loads each candidate the way chapter 24 loads a key: subtract `n`,
keep the borrow, test for zero by ORing the limbs. Its `valid` bit is a
mask, and `sc_select` picks the first candidate when it is valid and the
second otherwise:

<!-- source: coax/lib/crypto/ecdsa_sign.coax pick -->
```jai
// k = the first candidate in [1, n-1], chosen with a mask; valid is 0 when
// neither is. ss_key loads a candidate without a branch (chapter 24).
ec_pick :: (w: [] EcdsaWork) {
  #assert ss_key(w[0].first[..], w[0].a[..], false) && ss_key(w[0].second[..], w[0].b[..], false);
  w[0].k = sc_select(w[0].a[0].valid, w[0].a[0].safe, w[0].b[0].safe);
  w[0].valid = w[0].a[0].valid | w[0].b[0].valid;
}
```

Both candidates invalid has probability below `2^-254`. Then `valid` is 0
and the signer refuses instead of looping. libsecp256k1 asks for a third
candidate in that case, so the two signers differ only on inputs that no
one can find.

### Low-S

Chapter 19 showed that `(r, n - s)` verifies whenever `(r, s)` does, and
that Bitcoin Core's policy makes the high half nonstandard: Core doesn't
relay a transaction with a high-S signature. BIP-62 lists this malleability among
others. So the signer always outputs the low half.

For `s < n` and odd `n`, `s > (n - 1) / 2` exactly when `2s >= n`. `ec_low`
doubles `s` into nine 32-bit limbs, subtracts `n`, and reads the borrow: no
borrow means `2s >= n`. It computes `n - s` either way and selects:

<!-- source: coax/lib/crypto/ecdsa_sign.coax low -->
```jai
// 1 when s > (n-1)/2, else 0. For s < n, that holds exactly when 2s >= n,
// so compute 2s in nine limbs, subtract n, and read the borrow.
ec_high :: (s: ScScalar) -> u64 {
  wide: [9] u32; carry: u32;
  for i: 0..<8 { wide[i] = (s.limbs[i] << 1) | carry; carry = s.limbs[i] >> 31; }
  wide[8] = carry;
  return sc_subtract_n(wide).borrow ^ 1;
}
// n - s when s is in the high half, selected on that bit.
ec_low :: (s: ScScalar) -> ScScalar {
  return sc_select(ec_high(s), sc_negate(s), s);
}
```

About half of all signatures need the flip. The first `s` of the worked
example below is high.

### The entry

`ecdsa_sign` puts the steps together:

<!-- source: coax/lib/crypto/ecdsa_sign.coax ecdsa -->
```jai
// The signature of a 32-byte message hash under a 32-byte key, with RFC
// 6979's nonce (with data as extra data when it isn't empty), into out:
// r || s (32 bytes each, s in the low half), then the recovery ID when out
// has 65 bytes. The caller checks the views.
ec_sign :: (key: [] u8, hash: [] u8, data: [] u8, w: [] EcdsaWork, out: [] u8) -> bool {
  #assert ss_key(key, w[0].key[..], false);              // d, or 1 with valid = 0
  ec_seed(key, hash, data, w);
  ec_pick(w);                                            // k
  #assert sp_base(w[0].k, w[0].point[..]) && sp_x(w[0].point[..], w[0].x[..]);
  w[0].r = sc_reduce_bytes(w[0].x[..]).value;            // r = x(kG) mod n
  z := sc_reduce_bytes(hash).value;                      // z = h mod n
  w[0].kinv = sc_inverse(w[0].k).value;
  w[0].s = sc_mul(w[0].kinv, sc_add(z, sc_mul(w[0].r, w[0].key[0].safe)));   // s = (z + r*d) / k
  w[0].high = ec_high(w[0].s);
  w[0].s = sc_select(w[0].high, sc_negate(w[0].s), w[0].s);                // the low half
  valid := w[0].key[0].valid & w[0].valid & w[0].point[0].finite & ec_nonzero(w[0].r) & ec_nonzero(w[0].s);
  if !ec_declassify(valid) { return ec_clear(w); }      // the one fact the signer reveals
  for i: 0..<8 {
    #assert cb_store_be32(out, 28-4*i, w[0].r.limbs[i]) && cb_store_be32(out, 60-4*i, w[0].s.limbs[i]);
  }
  if out.count == 65 { out[64] = ec_recid(w); }         // a public length decides
  return !ec_clear(w);
}
// The low-S ECDSA signature r || s (32 bytes each). Views are stable and
// disjoint (overlap is the host's truthful observation). out is written
// only when the result is true; the workspace is cleared either way.
ecdsa_sign :: (key: [] u8, hash: [] u8, w: [] EcdsaWork, out: [] u8, overlap: bool) -> bool #export_c {
  if !ec_views(key, hash, w, out, overlap, 64) { return false; }
  return ec_sign(key, hash, hash[0..<0], w, out);
}
```

`ss_key` maps an invalid key to the scalar 1 and records `valid = 0`, so
an invalid key costs the same work as a valid one. `valid` is the AND of
five masks: the key, the nonce, a finite `R`, and nonzero `r` and `s`.
`ec_declassify` turns it into the call's result. The `#assert` lines check
only view lengths, which are public. The body keeps the low-S decision as
the bit `high`, and writes a 65th byte only for
[chapter 22](ch022-public-key-recovery-and-signed-messages.md)'s
`ecdsa_sign_recoverable`, which calls the same body.

### Secrets in the code

Chapter 3's rules: a secret never decides a branch, never selects a memory
address, and never reaches a variable-time routine. The signer meets them
this way:

- **HMAC.** SHA-256 and HMAC-SHA256 (chapters 4 and 8) process every byte
  with the same instructions, whatever its value.
- **Candidates and key.** `ss_key` and `sc_select` use masks; nothing
  branches on a candidate's or the key's value.
- **`k·G`.** `sp_base` uses the fixed-base comb of chapter 16, which reads
  every table entry. `sp_x` converts to affine with a field inversion by a
  fixed exponent.
- **`k^-1`.** `sc_inverse` raises `k` to the fixed power `n - 2`, the same
  multiplications for every `k` (chapter 14).
- **Low-S and validity.** Masks again, and `ec_nonzero` ORs limbs.
- **The one fact revealed.** `ec_declassify` turns the validity mask into a
  branch. That reveals whether the key was valid and whether an event of
  probability below `2^-254` happened, both of which the caller learns from
  the result anyway. Its name ends in `_declassify` so that audits can find it.
- **Erasure.** `ec_clear` overwrites the whole workspace with zeros on every
  return. Temporaries inside the scalar and field arithmetic and HMAC's
  internal state live on the stack, which chapter 3's erasure doesn't
  reach.

`r` comes from the secret `k`, but it is published, so `sc_reduce_bytes` may
read it in any way. `ecdsa_der` encodes the published signature and branches
on its bytes; it never sees a secret.

### Worked example: RFC 6979 by hand

The key `d = 1` and `h = SHA-256("Satoshi Nakamoto") = a0dc65ff...7704883e`,
below `n`. Python's `hmac` module, which shares no code with Coax, gives
the same values as the Coax generator:

1. Steps d and e: `K = 0b663006...58bf91b7` and `V = 616cad10...840c03f0`.
2. Steps f and g: `K = f457c016...cf0c1f86` and `V = a196437a...03bff4ae`.
3. Step h: `V = 8f8a276c...74f15d15`. It is below `n`, so it is `k`.
4. `R = k·G` has `x = 934b1ea1...210ee3d8`, which is below `n`, so it is
   `r`.
5. `s = k^-1 · (z + r) = dbbd3162...a586675c`. It starts with `0xdb`, so it
   is above `(n - 1) / 2`.
6. The signer publishes `n - s = 2442ce9d...2aafd9e5`.

The signature `934b1ea1...210ee3d8 || 2442ce9d...2aafd9e5` is libsecp256k1's
for the same key and hash, byte for byte, and the one chapter 19's worked
example verifies.

### Extra entropy

libsecp256k1's `secp256k1_ecdsa_sign` takes optional extra data, `ndata`,
32 bytes appended to RFC 6979's seed as `x || h || ndata` (RFC 6979 section
3.6). Bitcoin Core uses it to grind for a low `r`: when the first
signature's `r` has its top bit set, it signs again with a counter as
`ndata`, so that `r` needs no `00` byte in DER and the transaction is one
byte smaller. `ec_seed` takes that extra data (empty here), and `ec_sign`
passes it through. `ecdsa_sign` passes none, so its output is
libsecp256k1's signature with `ndata` absent, which is Bitcoin Core's
whenever the first `r` is already low, about half the time. Chapter 94's
`ecdsa_sign_low_r` grinds as Core does and checks itself against the
pinned file's `extraEntropy` rows.

## Why a reused or biased nonce gives the key away

Solve the signing equation for the key:

```text
s = k^-1 · (z + r·d)          so          d = (s·k - z) / r   (mod n)
```

Anyone who learns `k` for one signature learns `d`. Everything else in that
formula is public.

**One nonce, two hashes.** Two signatures with the same `k` share `r`,
because `r` depends only on `k`. Subtract their equations:

```text
s1·k = z1 + r·d
s2·k = z2 + r·d
(s1 - s2)·k = z1 - z2         so          k = (z1 - z2) / (s1 - s2)
```

Then `d = (s1·k - z1) / r`. Two signatures that share `r` give the key to
anyone, with two divisions. Sony's PlayStation 3 signed with one fixed `k`
and lost its signing key this way in 2010. In 2013 a flaw in Android's
`SecureRandom` made Bitcoin wallets repeat nonces, and coins were stolen
from the repeated `r` values on the chain.

Low-S doesn't help. A flipped `s` is `n - s`, so an attacker tries both
signs of each `s`, four combinations at most, and checks each candidate key
against the public key.

**A small or biased nonce.** If `k` comes from a small range, an attacker
searches it: compute `j·G` for `j = 1, 2, ...` until `x(j·G) mod n = r`,
then `d = (s·j - z) / r`. A nonce below `2^b` costs at most `2^b`
additions. A nonce that is only slightly biased (its top few bits always
zero, say) isn't searchable one signature at a time, but each signature
gives one linear equation `k_i = s_i^-1·z_i + s_i^-1·r_i·d` with a small
unknown `k_i`. With enough signatures, lattice reduction solves these
equations for `d`: this is the hidden number problem. Published attacks
recover keys when a few bits of every nonce are fixed or known, from
hundreds to thousands of signatures, and Breitner and Heninger found keys
on the Bitcoin chain this way in 2019. This chapter doesn't build a lattice
attack; the example shows the exhaustive search for a 12-bit nonce.

RFC 6979 makes `k` a function of `d` and `z`: one key and two hashes give
two unrelated nonces, and each nonce is HMAC-SHA256 output, as uniform as
SHA-256 allows. A broken random number generator can't reach it.

### Worked example: a reused nonce on a small curve

Use chapter 19's toy curve, `y^2 = x^3 + 7` over `F_43`, whose 31 points
form a group of prime order `n = 31` with `G = (2, 12)`. The key is `d = 7`.

1. Sign `z1 = 5` with `k = 10`: `10G = (42, 7)`, so `r = 42 mod 31 = 11`, and
   `s1 = 10^-1 · (5 + 11·7) = 28 · 82 mod 31 = 2`.
2. Sign `z2 = 6` with the same `k`: `r = 11` again, and
   `s2 = 28 · 83 mod 31 = 30`.
3. `k = (z1 - z2) / (s1 - s2) = (-1) / (-28) = 28^-1 mod 31 = 10`.
4. `d = (s1·k - z1) / r = (20 - 5) / 11 = 15 · 17 mod 31 = 7`, because
   `11 · 17 = 187 = 6 · 31 + 1`.

The attacker has the key.

## DER output

`ecdsa_der` encodes the public result for script: `30 len 02 len r 02 len
s`, each integer without leading zeros and with `00` in front when its top
bit is set. It refuses `r` or `s` equal to 0 or at least `n`:

<!-- source: coax/lib/crypto/ecdsa_sign.coax der -->
```jai
// The DER encoding of a compact r || s, into out (72 bytes are always
// enough); its length, or -1 when r or s is not in [1, n-1] or out is short.
ecdsa_der :: (sig:[]u8,out:[]u8)->int #export_c {
  if sig.count!=64 || out.count<72 {return -1;}
  r:=sc_load(sig[0..<32]);s:=sc_load(sig[32..<64]);
  if sc_is_zero(r.value) || sc_is_zero(s.value) {return -1;}    // sc_load zeroes a value at least n
  n:=ec_der_int(sig[32..<64],out,ec_der_int(sig[0..<32],out,2));
  out[0]=0x30;out[1]=cast(u8)(n-2);
  return n;
}
```

72 bytes always suffice: two 33-byte integers and six bytes of tags and
lengths. Chapter 21 teaches strict DER as BIP-66 applies it, with the
sighash type byte and the size bounds, and chapter 22 teaches signed
messages and public key recovery.

## The example

The example is [bitcoin/examples/ecdsa-sign.coax](../examples/ecdsa-sign.coax).
It recovers the key from a reused nonce on the toy curve, runs RFC 6979 one
step at a time for the worked example, recovers a key on secp256k1 from a
reused nonce and from a 12-bit nonce, signs all 2,020 pinned rows, and
shows the refusals. Its vector table,
[coax/tests/bitcoin/ecdsa_sign_vectors.coax](../../coax/tests/bitcoin/ecdsa_sign_vectors.coax),
is copied from the pinned tiny-secp256k1 file by
[make_ecdsa_sign_vectors.py](../../coax/tests/bitcoin/make_ecdsa_sign_vectors.py),
which refuses a file whose SHA-256 differs from its pin. The keys there and
in the example are published test keys, which is the only reason the
example may print intermediate secrets.

`raw_sign` computes the signing equation with a caller's `k` and no low-S,
and exists only to show the attacks. A signer must never take `k` from
anywhere but its nonce rule.

<!-- source: bitcoin/examples/ecdsa-sign.coax example -->
```jai
#load "../../coax/lib/crypto/ecdsa_sign.coax";
#load "../../coax/lib/crypto/ecdsa_verify.coax";
#load "../../coax/lib/crypto/sha256.coax";
#load "../../coax/lib/nostr/hex.coax";
#load "../../coax/tests/bitcoin/ecdsa_sign_vectors.coax";
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
bytes_same :: (a: [] u8, b: [] u8) -> bool {
  if a.count != b.count { return false; }
  for i: 0..<a.count { if a[i] != b[i] { return false; } }
  return true;
}
hex_scalar :: (text: string) -> ScScalar {
  b: [32] u8;
  return sc_load(decode(text, b[..])).value;
}
// The compressed public key d·G, for the verifier.
compressed_key :: (d: [] u8, out: [] u8) -> [] u8 {
  #assert se_encode(smb_multiply_web(sc_load(d).value), SE_COMPRESSED, out) == SE_COMPRESSED;
  return out[0..<33];
}

// ---- the toy curve of chapter 19: y^2 = x^3 + 7 over F_43, 31 points.
TOY_P :: 43;
TOY_N :: 31;
Toy :: struct { x: int; y: int; }
TOY_G :: Toy.{x=2,y=12};
TOY_INFINITY :: Toy.{x=-1,y=-1};
toy_mod :: (a: int, m: int) -> int { r := a % m; return ifx r < 0 then r + m else r; }
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
toy_times :: (k: int, p: Toy) -> Toy {
  r := TOY_INFINITY;
  for i: 0..<k { r = toy_add(r, p); }
  return r;
}
ToySig :: struct { r: int; s: int; }
// s = (z + r·d) / k mod n, with r = x(k·G) mod n.
toy_sign :: (z: int, d: int, k: int) -> ToySig {
  r := toy_times(k, TOY_G).x % TOY_N;
  return ToySig.{r=r,s=toy_mod(toy_inverse(k, TOY_N) * (z + r * d), TOY_N)};
}

// ---- the signing equation without RFC 6979 or low-S, for the attacks.
// A signer must never take k from anywhere but a nonce rule; these
// procedures exist to show what goes wrong when it does.
RawSig :: struct { r: ScScalar; s: ScScalar; }
raw_sign :: (z: ScScalar, d: ScScalar, k: ScScalar) -> RawSig {
  x: [32] u8;
  #assert sf_store(sg_to_affine(smb_multiply_web(k)).value.x, x[..]);
  r := sc_reduce_bytes(x[..]).value;
  return RawSig.{r=r,s=sc_mul(sc_inverse(k).value, sc_add(z, sc_mul(r, d)))};
}
// Two signatures with one k: k = (z1 - z2) / (s1 - s2), then d = (s1·k - z1) / r.
reused_key :: (z1: ScScalar, a: RawSig, z2: ScScalar, b: RawSig) -> ScScalar {
  k := sc_mul(sc_sub(z1, z2), sc_inverse(sc_sub(a.s, b.s)).value);
  return sc_mul(sc_sub(sc_mul(a.s, k), z1), sc_inverse(a.r).value);
}
// A nonce below 2^bits: walk j·G for j = 1, 2, ... until x(j·G) mod n = r,
// then d = (s·j - z) / r. Returns j, or 0 when no j below the bound fits.
small_nonce :: (sig: RawSig, bits: int) -> int {
  point := sg_generator();
  for j: 1..<(1 << bits) {
    x: [32] u8;
    #assert sf_store(sg_to_affine(point).value.x, x[..]);
    if sc_equal(sc_reduce_bytes(x[..]).value, sig.r) { return j; }
    point = sg_add(point, sg_generator()).value;
  }
  return 0;
}

SATOSHI_KEY :: "0000000000000000000000000000000000000000000000000000000000000001";
SATOSHI_SIG :: "934b1ea10a4b3c1757e2b0c017d0b6143ce3c9a7e6a4a49860d7a6ab210ee3d82442ce9d2b916064108014783e923ec36b49743e2ffa1c4496f01a512aafd9e5";
TEST_KEY :: "69ec59eaa1f4f2e36b639716b7c30ca86d9a5375c7b38d8918bd9c0ebc80ba64";

main :: () {
  // The toy curve: one nonce for two hashes gives the key away.
  print("toy curve y^2 = x^3 + 7 mod 43, n = 31, G = (2, 12)\n");
  a := toy_sign(5, 7, 10); b := toy_sign(6, 7, 10);
  print("d = 7, k = 10: z = 5 gives (%, %), z = 6 gives (%, %)\n", a.r, a.s, b.r, b.s);
  k := toy_mod((5 - 6) * toy_inverse(toy_mod(a.s - b.s, TOY_N), TOY_N), TOY_N);
  print("k = (z1 - z2) / (s1 - s2) = %, d = (s1*k - z1) / r = %\n", k, toy_mod((a.s * k - 5) * toy_inverse(a.r, TOY_N), TOY_N));

  // RFC 6979 one step at a time, for the key 1 and SHA-256("Satoshi Nakamoto").
  print("RFC 6979, key 1, SHA-256(\"Satoshi Nakamoto\")\n");
  key: [32] u8; hash: [32] u8; text := "Satoshi Nakamoto"; message: [16] u8;
  for i: 0..<16 { message[i] = text[i]; }
  #assert sha256_hash_checked(message[..], hash[..], false) == SHA_OK;
  d := decode(SATOSHI_KEY, key[..]);
  show("h", hash[..]);
  w: [1] EcdsaWork;
  for i: 0..<32 { w[0].hv[i] = 1; w[0].hk[i] = 0; w[0].buf[33+i] = d[i]; w[0].buf[65+i] = hash[i]; }   // h < n
  ec_step(w[..], 0, 97); show("K = HMAC_K(V || 00 || x || h)", w[0].hk[..]); show("V = HMAC_K(V)", w[0].hv[..]);
  ec_step(w[..], 1, 97); show("K = HMAC_K(V || 01 || x || h)", w[0].hk[..]); show("V = HMAC_K(V)", w[0].hv[..]);
  ec_next(w[..]); show("k = HMAC_K(V)", w[0].hv[..]);
  nonce := sc_load(w[0].hv[..]).value;
  raw := raw_sign(sc_reduce_bytes(hash[..]).value, sc_load(d).value, nonce);
  show_scalar("r = x(k*G) mod n", raw.r);
  show_scalar("s = (z + r*d) / k", raw.s);
  print("s > (n-1)/2: %, so the signer publishes n - s\n", !sc_equal(ec_low(raw.s), raw.s));
  out: [64] u8; want: [64] u8; pub: [33] u8; der: [72] u8;
  print("ecdsa_sign: %\n", ecdsa_sign(d, hash[..], w[..], out[..], false));
  show("r || s", out[..]);
  print("equals libsecp256k1's: %, verifies (low-S): %\n", bytes_same(out[..], decode(SATOSHI_SIG, want[..])),
    ecdsa_verify_vartime(hash[..], compressed_key(d, pub[..]), out[..], true));
  n := ecdsa_der(out[..], der[..]);
  show("DER", der[0..<n]);

  // One nonce, two hashes, on secp256k1: anyone can solve for the key.
  print("one nonce for two hashes\n");
  z1 := sc_reduce_bytes(hash[..]).value; z2 := sc_add(z1, sc_small(1));
  secret := hex_scalar(TEST_KEY);
  s1 := raw_sign(z1, secret, nonce); s2 := raw_sign(z2, secret, nonce);
  print("same r: %\n", sc_equal(s1.r, s2.r));
  found := reused_key(z1, s1, z2, s2);
  show_scalar("recovered d", found);
  print("equals the key: %\n", sc_equal(found, secret));

  // A biased nonce: k below 2^12. The search costs at most 4,095 additions.
  print("a nonce below 2^12\n");
  small := raw_sign(z1, secret, sc_small(2748));
  j := small_nonce(small, 12);
  print("k found after % steps\n", j);
  recovered := sc_mul(sc_sub(sc_mul(small.s, sc_small(cast(u32) j)), z1), sc_inverse(small.r).value);
  print("recovered d equals the key: %\n", sc_equal(recovered, secret));

  // The pinned vectors: every signature byte for byte and low-S. Every
  // 20th is also verified: chapter 19's verifier costs ten signatures.
  print("tiny-secp256k1 ecdsa.json\n");
  equal := 0; low := 0; verified := 0; tried := 0;
  for i: 0..<ES_CASES.count {
    c := ES_CASES[i]; kb: [32] u8; mb: [32] u8; sig: [64] u8; got: [64] u8; q: [33] u8; v: [1] EcdsaWork;
    dk := decode(c.d, kb[..]); m := decode(c.m, mb[..]);
    #assert ecdsa_sign(dk, m, v[..], got[..], false);
    if bytes_same(got[..], decode(c.sig, sig[..])) { equal += 1; }
    if !ev_high_s_vartime(sc_load(got[32..<64]).value) { low += 1; }
    if i % 20 != 0 { continue; }
    tried += 1;
    if ecdsa_verify_vartime(m, compressed_key(dk, q[..]), got[..], true) { verified += 1; }
  }
  print("% of % equal libsecp256k1's signature, % are low-S\n", equal, ES_CASES.count, low);
  print("% of % verify with chapter 19's verifier\n", verified, tried);

  print("refusals\n");
  for i: 0..<ES_REFUSE.count {
    c := ES_REFUSE[i]; kb: [32] u8; mb: [32] u8; sig: [64] u8;
    sig[0] = 7;
    ok := ecdsa_sign(decode(c.d, kb[..]), decode(c.m, mb[..]), w[..], sig[..], false);
    print("%: signed %, out untouched %\n", c.why, ok, sig[0] == 7);
  }
  print("31-byte hash: %\n", ecdsa_sign(d, hash[0..<31], w[..], out[..], false));
  print("63-byte output: %\n", ecdsa_sign(d, hash[..], w[..], out[0..<63], false));
  print("views that overlap: %\n", ecdsa_sign(d, hash[..], w[..], out[..], true));
  two: [2] EcdsaWork;
  print("a workspace of two: %\n", ecdsa_sign(d, hash[..], two[..], out[..], false));
}
```

Run it from the repository root. The signer's fixed-base tables are
computed at compile time and need more than the default compile-time
budget:

```shell
coax/bin/coax run bitcoin/examples/ecdsa-sign.coax --run-steps 250000000
```

The output shows the toy key recovered, the generator's `K` and `V` after
each step, the high `s` and its flip, both attacks on secp256k1, every
pinned row equal to libsecp256k1's signature, and the refusals:

```text
toy curve y^2 = x^3 + 7 mod 43, n = 31, G = (2, 12)
d = 7, k = 10: z = 5 gives (11, 2), z = 6 gives (11, 30)
k = (z1 - z2) / (s1 - s2) = 10, d = (s1*k - z1) / r = 7
RFC 6979, key 1, SHA-256("Satoshi Nakamoto")
h: a0dc65ffca799873cbea0ac274015b9526505daaaed385155425f7337704883e
K = HMAC_K(V || 00 || x || h): 0b663006e369d6851668b59760c0cfbaa5d09e23e6f916443b3de4d558bf91b7
V = HMAC_K(V): 616cad1051aebe08197d002dbdb90bdc2bb6b65b3d768c518863056a840c03f0
K = HMAC_K(V || 01 || x || h): f457c01688026ee0b9236e5c7ff1478007ad0b697855f592efa9468ecf0c1f86
V = HMAC_K(V): a196437a8f26fbc9e519e4fd4b5d10d62e6a5306c98ca431b96131f403bff4ae
k = HMAC_K(V): 8f8a276c19f4149656b280621e358cce24f5f52542772691ee69063b74f15d15
r = x(k*G) mod n: 934b1ea10a4b3c1757e2b0c017d0b6143ce3c9a7e6a4a49860d7a6ab210ee3d8
s = (z + r*d) / k: dbbd3162d46e9f9bef7feb87c16dc13b4f6568a87f4e83f728e2443ba586675c
s > (n-1)/2: true, so the signer publishes n - s
ecdsa_sign: true
r || s: 934b1ea10a4b3c1757e2b0c017d0b6143ce3c9a7e6a4a49860d7a6ab210ee3d82442ce9d2b916064108014783e923ec36b49743e2ffa1c4496f01a512aafd9e5
equals libsecp256k1's: true, verifies (low-S): true
DER: 3045022100934b1ea10a4b3c1757e2b0c017d0b6143ce3c9a7e6a4a49860d7a6ab210ee3d802202442ce9d2b916064108014783e923ec36b49743e2ffa1c4496f01a512aafd9e5
one nonce for two hashes
same r: true
recovered d: 69ec59eaa1f4f2e36b639716b7c30ca86d9a5375c7b38d8918bd9c0ebc80ba64
equals the key: true
a nonce below 2^12
k found after 2748 steps
recovered d equals the key: true
tiny-secp256k1 ecdsa.json
2020 of 2020 equal libsecp256k1's signature, 2020 are low-S
101 of 101 verify with chapter 19's verifier
refusals
Private key == 0: signed false, out untouched true
Private key >= G: signed false, out untouched true
Private key >= G: signed false, out untouched true
Private key >= G: signed false, out untouched true
31-byte hash: false
63-byte output: false
views that overlap: false
a workspace of two: false
```

Every pinned row is signed, compared byte for byte with libsecp256k1's
signature, and checked for low-S. Every twentieth signature, 101 in all, is
also checked by chapter 19's verifier: one verification costs about ten
signatures in the debug build, and the native harness below verifies all
of them.

The refusals:

- The keys 0, `n`, `n + 1` and `2^256 - 1`, tiny-secp256k1's four bad keys:
  `ss_key`'s validity bit is 0, the signer does the full work, refuses, and
  leaves `out` unwritten.
- A 31-byte hash, a 63-byte output, views the host reports as overlapping,
  or a workspace of two: `ec_views` refuses before any work.

The book's checks run this program in debug and release builds and compare
the output with [bitcoin/fixtures/ecdsa-sign.out](../fixtures/ecdsa-sign.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/ecdsa-sign.coax](../examples/checks/ecdsa-sign.coax),
which loads the example:

- on the toy curve, for every key and every nonce, a reused nonce gives
  back the nonce and the key, and so does a known nonce;
- RFC 6979's `K` and `V` after steps d to g, `k`, the high `s` and its
  flip, against Python's `hmac` and integers;
- both attacks on secp256k1, with Python's values for the two `s`, and a
  search bound that is one bit too small finds nothing;
- the first 20 pinned rows, byte for byte, in the interpreter: the keys 1
  and `n - 1`, the hashes 0, `2^256 - 1` and `n`, which reduce mod `n`;
- the four bad keys refuse and leave `out` unwritten.

<!-- source: bitcoin/examples/checks/ecdsa-sign.coax check -->
```jai
// On the toy curve, for every key d and nonce k, two hashes signed with
// the same k give the key back, and a known k gives it back from one
// signature. Pairs where s1 = s2 or r = 0 have no inverse to divide by.
#forall "toy: a reused or known nonce gives the key for every d and k" d: 1..<TOY_N #steps 400000000 {
  solved := 0;
  for k: 1..<TOY_N {
    a := toy_sign(5, d, k); b := toy_sign(6, d, k);
    #assert a.r == b.r;
    if a.r == 0 || a.s == b.s { continue; }
    found := toy_mod((5 - 6) * toy_inverse(toy_mod(a.s - b.s, TOY_N), TOY_N), TOY_N);
    #assert found == k;
    #assert toy_mod((a.s * k - 5) * toy_inverse(a.r, TOY_N), TOY_N) == d;
    solved += 1;
  }
  #assert solved > 25;
}
bytes_hex :: (bytes: [] u8, text: string) -> bool {
  want: [64] u8;
  return bytes_same(bytes, decode(text, want[..]));
}
scalar_hex :: (s: ScScalar, text: string) -> bool {
  b: [32] u8;
  return sc_store(s, b[..]) && bytes_hex(b[..], text);
}
#test "bytes_hex and scalar_hex notice a change in the first or last byte and a short view" {
  b: [32] u8; zero := "0000000000000000000000000000000000000000000000000000000000000000";
  #assert bytes_hex(b[..], zero) && !bytes_hex(b[0..<31], zero);
  b[0] = 1; #assert !bytes_hex(b[..], zero);
  b[0] = 0; b[31] = 1; #assert !bytes_hex(b[..], zero);
  #assert scalar_hex(sc_small(1), "0000000000000000000000000000000000000000000000000000000000000001");
  #assert !scalar_hex(sc_small(2), "0000000000000000000000000000000000000000000000000000000000000001");
}
// K and V after RFC 6979 steps d to g, the nonce, and the unnormalized s,
// from Python's hmac and integers, which share no code with Coax.
#test "RFC 6979 steps and the high s match Python" #steps 200000000 {
  w: [1] EcdsaWork; key: [32] u8; hash: [32] u8;
  d := decode(SATOSHI_KEY, key[..]);
  h := decode("a0dc65ffca799873cbea0ac274015b9526505daaaed385155425f7337704883e", hash[..]);
  for i: 0..<32 { w[0].hv[i] = 1; w[0].hk[i] = 0; w[0].buf[33+i] = d[i]; w[0].buf[65+i] = h[i]; }
  ec_step(w[..], 0, 97);
  #assert bytes_hex(w[0].hk[..], "0b663006e369d6851668b59760c0cfbaa5d09e23e6f916443b3de4d558bf91b7");
  #assert bytes_hex(w[0].hv[..], "616cad1051aebe08197d002dbdb90bdc2bb6b65b3d768c518863056a840c03f0");
  ec_step(w[..], 1, 97);
  #assert bytes_hex(w[0].hk[..], "f457c01688026ee0b9236e5c7ff1478007ad0b697855f592efa9468ecf0c1f86");
  #assert bytes_hex(w[0].hv[..], "a196437a8f26fbc9e519e4fd4b5d10d62e6a5306c98ca431b96131f403bff4ae");
  ec_next(w[..]);
  #assert bytes_hex(w[0].hv[..], "8f8a276c19f4149656b280621e358cce24f5f52542772691ee69063b74f15d15");
  raw := raw_sign(sc_reduce_bytes(h).value, sc_load(d).value, sc_load(w[0].hv[..]).value);
  #assert scalar_hex(raw.s, "dbbd3162d46e9f9bef7feb87c16dc13b4f6568a87f4e83f728e2443ba586675c");
  #assert scalar_hex(ec_low(raw.s), "2442ce9d2b916064108014783e923ec36b49743e2ffa1c4496f01a512aafd9e5");
}
// Both attacks on secp256k1, with Python's s values for the reused nonce.
#test "a reused nonce and a small nonce give the key" #steps 400000000 {
  hash: [32] u8;
  z := sc_reduce_bytes(decode("a0dc65ffca799873cbea0ac274015b9526505daaaed385155425f7337704883e", hash[..])).value;
  secret := hex_scalar(TEST_KEY); k := hex_scalar("8f8a276c19f4149656b280621e358cce24f5f52542772691ee69063b74f15d15");
  a := raw_sign(z, secret, k); b := raw_sign(sc_add(z, sc_small(1)), secret, k);
  #assert scalar_hex(a.s, "73a41a36a5d00e087e80f50c2631ee8265b7fed30d995b67372d4d147e6eb8c8");
  #assert scalar_hex(b.s, "8de47240cc75a3e78f80815f09fe1eaf73755751e640f41b1a6f0eabe427c021");
  #assert sc_equal(reused_key(z, a, sc_add(z, sc_small(1)), b), secret);
  small := raw_sign(z, secret, sc_small(5));
  #assert small_nonce(small, 3) == 5 && small_nonce(small, 2) == 0;
}
// The first 20 rows of the pinned file in the interpreter: its quotations
// under the keys 1, n - 1 and others, and the hashes 2^256 - 1 and n,
// which reduce mod n. All 2,020 rows run in the example.
#forall "tiny-secp256k1 rows sign byte for byte" i: 0..<20 #steps 400000000 {
  c := ES_CASES[i]; kb: [32] u8; mb: [32] u8; sig: [64] u8; got: [64] u8; w: [1] EcdsaWork;
  #assert ecdsa_sign(decode(c.d, kb[..]), decode(c.m, mb[..]), w[..], got[..], false);
  #assert bytes_same(got[..], decode(c.sig, sig[..]));
}
#forall "the pinned bad keys refuse and leave out unwritten" i: 0..<4 #steps 100000000 {
  c := ES_REFUSE[i]; kb: [32] u8; mb: [32] u8; out: [64] u8; w: [1] EcdsaWork;
  out[0] = 7;
  #assert !ecdsa_sign(decode(c.d, kb[..]), decode(c.m, mb[..]), w[..], out[..], false) && out[0] == 7;
}
```

To run them:

```shell
coax/bin/coax test bitcoin/examples/checks/ecdsa-sign.coax --run-steps 250000000
coax/bin/coax mutate bitcoin/examples/checks/ecdsa-sign.coax --root-only --run-steps 250000000
```

On 2026-11-02, `coax test` ran 185 checks, including the loaded files'
own checks, in under 3 minutes on a loaded machine. The 20 pinned rows use
272 million interpreter steps together, about 14 million a signature, so
they carry a budget of 400 million. Root-only mutation of the checks file
kills its 1 mutant. The first run had one survivor: `scalar_hex`'s `&&`
became `||`, which no check noticed because `sc_store` never fails. A check
that `scalar_hex` refuses a wrong value now kills it. The example's own
procedures were not mutated for this chapter.

### The library's own checks

`ecdsa_sign.coax` checks its pieces against independent values: both RFC
6979 candidates for three keys, including a hash at least `n`, against
Python's `hmac`; four signatures against libsecp256k1's; the second
candidate taken when the first is 0 or `n`; the low-S boundary at
`(n - 1) / 2`; the DER encoder's lengths, sign bytes and leading zeros; and
the refusals, with `out` unwritten and the workspace cleared.

```shell
coax/bin/coax test coax/lib/crypto/ecdsa_sign.coax --run-steps 250000000
coax/bin/coax mutate coax/lib/crypto/ecdsa_sign.coax --root-only --run-steps 250000000
```

On 2026-11-02, `coax test` ran 157 checks, including the loaded files',
and root-only mutation killed all 283 mutants of `ecdsa_sign.coax`, with
none marked equivalent. The count includes chapter 22's recoverable
signer and its checks.

### tiny-secp256k1, libsecp256k1 and Python, natively and in Wasm

[ecdsa_sign.py](../../coax/tests/bitcoin/ecdsa_sign.py) compiles
`ecdsa_sign.coax` to C in checked and release builds and signs:

- all 2,020 rows of the pinned `ecdsa.json` and its four bad keys;
- 400 seeded random signatures, with the keys 1, `n - 1`, 2, `n - 2`, 255
  and `2^200` and keys with leading zero bytes, and the hashes 0, 1, `n`,
  `n - 1` and `2^256 - 1`.

Each signature must equal libsecp256k1 `v0.8.0`'s `secp256k1_ecdsa_sign`
output byte for byte, compact and DER, be low-S, verify in libsecp256k1, in
a Python verifier with affine integers and in chapter 19's Coax verifier,
and leave every byte of the workspace zero. For each random signature, an
RFC 6979 written in Python with `hmac` gives `k`, and `r` must equal
`x(k·G) mod n`. Then it builds the same C for wasm32 and signs the 2,020
rows and the four bad keys again in Node.

```shell
python3 tools/coax-nostr/fuzz/build_oracle.py --fetch --reviewed   # once: libsecp256k1 v0.8.0
python3 coax/tests/bitcoin/ecdsa_sign.py
```

On 2026-11-02 all of it passed: the 2,020 rows and 400 random signatures
equal to libsecp256k1's in both native builds, every one verified by the
three verifiers, the four bad keys refused, the workspace zero after every
call, and the 2,024 rows in both Wasm builds in Node. The run took 432
seconds on a loaded machine, most of it in the Python verifier.

### Secret flow

[ecdsa_sign_secret_probe.sh](../../coax/tests/bitcoin/ecdsa_sign_secret_probe.sh)
compiles the release C of `ecdsa_sign.coax` and runs
[ecdsa_sign_secret_probe.c](../../coax/tests/bitcoin/ecdsa_sign_secret_probe.c)
under Valgrind's Memcheck with the key marked undefined, in the style of
ctgrind. Memcheck then reports every branch and every memory address that
depends on the key, through HMAC, the scalar arithmetic and the comb. The
hash is public, and only `out` and the result are declassified, after the
call. The probe passes only if every report is at the `ec_declassify`
line of the generated C, found by line number, not by function name.
Chapter 24's probe exempts whole functions by name, which an independent
review showed would hide a planted branch in those functions.

Memcheck runs on Linux. `--host HOST` generates the C on this machine and
runs Memcheck on a Linux host over ssh:

```shell
coax/tests/bitcoin/ecdsa_sign_secret_probe.sh --host HOST
PLANT=1 coax/tests/bitcoin/ecdsa_sign_secret_probe.sh --host HOST   # must FAIL
```

On 2026-11-02, on a NixOS x86-64 host with Valgrind 3.26.0 and GCC
15.2.0, the probe signed and Memcheck reported one site, the `ec_declassify`
branch in `ec_sign`, the body that `ecdsa_sign` calls. The probe also signs
with chapter 22's `ecdsa_sign_recoverable`. The control program's branch on the key was
flagged. With `PLANT=1`, which adds one branch on a key bit right after the
nonce derivation, Memcheck reported the planted line as well, and the probe
failed, as it must. Valgrind 3.26.0's default suppressions file doesn't
parse, so the probe runs with `--default-suppressions=no`.

The source-level secret audit of chapter 24
(`tools/coax-nostr/large-source-audit/`) doesn't run on this signer: on
2026-11-02 the audit tool itself no longer compiles with the current
compiler (`secret_aggregate.coax`, error E0381), for chapter 24's signer
too. The Memcheck probe is this chapter's secret-flow evidence. It checks
one compiled build on one input; a constant-time design has the same
branches and addresses for every input, and Memcheck tracks the key through
every operation, but the probe isn't a proof.

The signer is for test keys until its independent review is done and the
owner approves real keys, the same rule as chapter 24's.

## Summary

- `r = x(k·G) mod n` and `s = k^-1 · (z + r·d) mod n`, with `s` replaced
  by `n - s` in the high half, as Bitcoin Core signs.
- RFC 6979 derives `k` from the key and the hash with HMAC-SHA256. The
  signer always computes two candidates and selects with a mask, so its
  work never depends on the key.
- A nonce used twice, a known nonce or a nonce from a small range gives
  the key to anyone; a biased one does with enough signatures.
- No branch or address depends on the key except the declared validity
  decision, and the workspace is cleared on every return.
- All 2,020 pinned rows sign byte for byte as libsecp256k1 does, natively
  and in Wasm, and every signature verifies in libsecp256k1, Python and
  chapter 19's verifier.

## Dependencies

- [19. Verify ECDSA](ch019-verify-ecdsa.md)
- [8. HMAC-SHA256](ch008-hmac-sha256.md)
- [18. Keys and randomness](ch018-keys-and-randomness.md)
- [3. Constant-time rules and secrets](ch003-constant-time-rules.md)

## Sources

- [RFC 6979](https://www.rfc-editor.org/rfc/rfc6979), section 3.2
  (deterministic `k`) and section 3.6 (additional data). Its appendix A
  vectors cover NIST and binary curves, not secp256k1.
- [SEC 1 v2](https://www.secg.org/sec1-v2.pdf), section 4.1.3 (ECDSA
  signing).
- [BIP-62](https://github.com/bitcoin/bips/blob/master/bip-0062.mediawiki)
  and [BIP-146](https://github.com/bitcoin/bips/blob/master/bip-0146.mediawiki)
  (low-S).
- tiny-secp256k1's `tests/fixtures/ecdsa.json` (MIT), pinned at revision
  `e1fd7a77` in
  [coax/tests/vectors/upstream/tiny-secp256k1/](../../coax/tests/vectors/upstream/tiny-secp256k1/ecdsa.json),
  listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md). Its signing
  rows are libsecp256k1 signatures; the bitcoinjs project generated them.
- [libsecp256k1](https://github.com/bitcoin-core/secp256k1), release
  `v0.8.0`, pinned in
  [UPSTREAM.json](../../tools/coax-nostr/fuzz/UPSTREAM.json); tests only.
- Joachim Breitner and Nadia Heninger, "Biased Nonce Sense: Lattice
  Attacks against Weak ECDSA Signatures in Cryptocurrencies" (Financial
  Cryptography 2019), on the hidden number problem and keys recovered from
  biased nonces.
