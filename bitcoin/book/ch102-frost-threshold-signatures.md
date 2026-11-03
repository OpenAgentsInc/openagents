# 102. FROST threshold signatures

MuSig2 (chapter 101) is n-of-n: every signer must sign, and one lost
key locks the coins. FROST is t-of-n: any t of n participants make one
ordinary BIP-340 signature under one ordinary x-only key, and fewer than
t cannot. On the chain a FROST spend is a Taproot key-path spend, 64
bytes of signature under a 32-byte key, the same as a single signer's or
a MuSig2 group's. Chapter 100's 2-of-3 shows three keys and two
signatures; a FROST 2-of-3 shows one key and one signature.

The price is key generation. MuSig2's signers each keep their own key
and add the public halves. FROST's participants each hold a *share* of
one secret that nobody has to know, and making those shares takes
either a trusted dealer or a distributed key generation (DKG) with an
extra round of messages.

## Which specification this chapter follows

There are two FROST specifications for secp256k1:

- [RFC 9591](https://www.rfc-editor.org/rfc/rfc9591) (June 2024)
  defines FROST(secp256k1, SHA-256). Its keys and nonces are full
  points, its hashes are RFC 9380's hash-to-field, and its signatures are
  65 bytes. They are not BIP-340 signatures, and the RFC has no tweaks,
  so its keys can be neither Taproot output keys nor BIP-32 parents.
- BIP-445, "FROST Signing Protocol for BIP340 Signatures" (draft,
  version 0.12.1, assigned 2026-01-30), specifies the FROST3 variant of
  the signing protocol with BIP-340's x-only keys, even-y nonces, tagged
  hashes and BIP-327's plain and x-only tweaks. It is not yet merged into
  the BIPs repository; its text, reference code and vectors live at
  `siv2r/bip-frost-signing`.

This chapter follows BIP-445 at revision `ba299bad` (2026-10-15),
pinned with its reference implementation and its seven vector files. It
does not implement RFC 9591's ciphersuite: a Bitcoin wallet can't use
its signatures. BIP-445 leaves key generation out of scope; the trusted
dealer here is the one its vectors were made with, and the DKG is this
book's own, described in [Distributed key generation](#distributed-key-generation).

## What this chapter teaches

- Shamir's secret sharing: a secret as the value at 0 of a polynomial
  of degree t − 1, shares as its values at 1 … n, and Lagrange
  interpolation, which recovers the secret from any t of them.
- Key generation by a trusted dealer, and by a Pedersen DKG in which
  every participant deals a secret of its own, commits to it with
  Feldman's verifiable secret sharing and proves possession of it.
- The threshold key, the public shares and the check that they agree.
- Two-round signing with binding factors: why b hashes the signer set,
  and how a partial signature uses the signer's interpolating value.
- Signature share verification and aggregation into a BIP-340
  signature.
- x-only keys, even-y handling, and Taproot tweaking of the threshold
  key.
- Deterministic signing for the last signer.

## Where FROST is used

| Where | What it does | Chapter |
| --- | --- | --- |
| A Taproot output owned by t of n people | the threshold key, tweaked by chapter 41's TapTweak, is the output key | 41, 102 |
| An agent treasury with a recovery key | a 2-of-3 whose spend looks like one signer's | 122 |
| A BIP-32 tree under a threshold key | the plain tweak derives children from the threshold key | 32, 102 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [frost.coax](../../coax/lib/bitcoin/frost.coax) | `fr_lambda_vartime(ids, id)` | the interpolating value of `id` at 0 |
| | `fr_share_at_vartime(ids, points, x)` | the public share at any identifier |
| | `fr_validate_info_vartime(t, thresh_pk, pubshares, present)` | ValidateThresholdInfo |
| | `fr_tweaks_vartime(thresh_pk, tweaks, xonly)` | the tweak context, plain and x-only |
| | `fr_taproot_tweak_vartime(thresh_pk, merkle, out)` | the TapTweak of the threshold key (chapter 41) |
| | `fr_validate_signers_vartime(signers)` | ValidateSignersCtx |
| | `fr_nonce_gen(rand, secshare, id, pubshare, xonly, msg, has_msg, extra, secnonce, pubnonce, w)` | NonceGen |
| | `fr_nonce_agg_vartime(pubnonces, out)` | NonceAgg |
| | `fr_session_vartime(signers, aggnonce, tweaks, xonly, msg)` | GetSessionValues: b, R and e |
| | `fr_sign(secnonce, secshare, id, signers, aggnonce, tweaks, xonly, msg, psig, w)` | Sign; zeroes the secnonce |
| | `fr_partial_verify_vartime(psig, pubnonces, signers, tweaks, xonly, msg, i)` | PartialSigVerify |
| | `fr_partial_agg_vartime(psigs, session, u, out)` | PartialSigAgg: the BIP-340 signature |
| | `fr_det_sign(secshare, id, aggother, has_aggother, signers, tweaks, xonly, msg, aux, has_aux, pubnonce, psig, w)` | DeterministicSign |
| | `fr_dealer(secret, n, t, thresh_pk, secshares, pubshares, w)` | the trusted dealer |
| | `fr_combine(secshares, ids, out)` | Lagrange interpolation of the secret, for the checks |
| | `fr_dkg_round1`, `fr_dkg_check_vartime`, `fr_dkg_finish` | the Pedersen DKG |

`frost.coax` loads chapter 101's `musig.coax` and reuses its tagged
hash for any tag, its point encodings, its nonce and signing workspaces
and its fixed-base nonce point. Public values (keys, public shares,
nonces, partial signatures, identifiers and messages) go through
`_vartime` procedures. The secret share, the secret nonces and the
polynomial coefficients follow chapter 3's rules as chapter 101's do:
the erasable SHA-256 of chapter 24, the fixed-base `sp_base`, masked
range checks and workspaces that every return clears.

## Result codes

BIP-445's reference raises `InvalidContributionError`, naming the
position of the signer (or the coordinator) and what it sent, or
`ValueError` with a message. Each has a code. Three messages name an
index, and `FrResult.signer` carries it: for `FR_PUBSHARE` and
`FR_ID_RANGE` it is the position in the list, for a contribution the
signer's position.

<!-- source: coax/lib/bitcoin/frost.coax codes -->
```jai
// Result codes. The reference raises InvalidContributionError naming the
// signer (its position in ids) or the coordinator and what was sent, or
// ValueError with a message. A message with "%" names an index, carried
// in FrResult.signer.
FR_OK :: 0;
FR_BAD_PUBNONCE :: 1;        // InvalidContributionError(i, "pubnonce")
FR_BAD_AGGNONCE :: 2;        // InvalidContributionError(None, "aggnonce")
FR_BAD_AGGOTHERNONCE :: 3;   // InvalidContributionError(None, "aggothernonce")
FR_BAD_PSIG :: 4;            // InvalidContributionError(i, "psig")
FR_THRESHOLD :: 5;
FR_TOO_MANY :: 6;
FR_THRESH_PK :: 7;
FR_PUBSHARE :: 8;
FR_TOO_FEW_SHARES :: 9;
FR_NOT_ON_POLY :: 10;
FR_KEY_MISMATCH :: 11;
FR_THRESH_INFINITY :: 12;
FR_TWEAK_RANGE :: 13;
FR_TWEAK_INFINITY :: 14;
FR_TWEAK_LISTS :: 15;
FR_SIGNER_COUNT :: 16;
FR_PUBSHARE_LISTS :: 17;
FR_ID_RANGE :: 18;
FR_ID_DUPLICATE :: 19;
FR_SECNONCE_1 :: 20;
FR_SECNONCE_2 :: 21;
FR_SECSHARE :: 22;
FR_ID_MISSING :: 23;
FR_PUBSHARE_MISMATCH :: 24;
FR_AGGOTHER_OMIT :: 25;
FR_AGGOTHER_PRESENT :: 26;
FR_PSIG_LISTS :: 27;
FR_INVALID :: 28;            // a partial signature that does not verify
FR_CAPACITY :: 29;           // a wrong length, or more than these arrays hold
FR_DKG_POP :: 30;            // a DKG proof of possession that does not verify
FR_DKG_SHARE :: 31;          // a DKG share that does not match its commitments
FR_REASONS :: [32] string.["ok", "pubnonce", "aggnonce", "aggothernonce", "psig",
  "The threshold must be 1 <= t <= n.",
  "The number of participants must be n <= 128.",
  "The thresh_pk is not a valid point.",
  "The pubshare at index % is not a valid point.",
  "At least t pubshares must be present.",
  "The provided key material is incorrect: the public shares do not lie on a single polynomial.",
  "The provided key material is incorrect: the public shares do not match the threshold public key.",
  "The threshold public key must not be the point at infinity.",
  "The tweak value is out of range.",
  "The tweaked threshold public key must not be the point at infinity.",
  "The tweaks and is_xonly lists must have the same length.",
  "The number of signers must satisfy t <= u <= n.",
  "The pubshares and ids lists must have the same length.",
  "The id at index % must satisfy 0 <= id <= n - 1.",
  "The ids list contains duplicate elements.",
  "The first half of secnonce is out of range.",
  "The second half of secnonce is out of range.",
  "The secshare is out of range.",
  "The signer's id is missing from the ids list.",
  "The signer's pubshare does not match its entry in the pubshares list.",
  "The aggothernonce must be omitted when u = 1.",
  "The aggothernonce must be present and a 66-byte array when u > 1.",
  "The psigs and ids lists must have the same length.",
  "invalid", "capacity", "proof of possession", "share"];
FR_MAX :: 128;               // participants: the BIP's bound on n
FR_TWEAKS :: 8;              // tweaks of one threshold key
// A code and the index it names, -1 for none.
FrResult :: struct { code: int; signer: int; }
```

## Shamir's secret sharing

A polynomial of degree t − 1 is fixed by any t of its points. Put the
secret at x = 0:

f(x) = a_0 + a_1 x + … + a_{t−1} x^{t−1}, with a_0 the secret,

and give participant i the share f(i + 1). BIP-445's identifiers are
0-based, so participant 0 holds f(1), participant 1 holds f(2), and the
secret, the value at 0, sits at identifier −1. One share says nothing
about a_0: for any secret there is a polynomial through it and the
share. Any t shares fix the polynomial and so the secret.

Lagrange interpolation gives the value at any x from t points. For the
secret, x = 0, each share is weighted by its *interpolating value*:

λ_i = ∏_{k ≠ i} (id_k + 1) / (id_k − id_i), and a_0 = Σ λ_i f(id_i + 1).

The same weights work on points: with public shares X_i = f(id_i + 1)·G,
the threshold key is Q = Σ λ_i X_i = a_0·G. That is how anyone checks
that n public shares and a threshold key belong together.

### Worked example: λ for participants 0 and 1

With ids {0, 1}, the shares sit at x = 1 and x = 2. For participant 0,
λ_0 = (1 + 1)/(1 − 0) = 2; for participant 1, λ_1 = (0 + 1)/(0 − 1) = −1,
which is n − 1 mod n. So a_0 = 2·f(1) − f(2): for a line f(x) = a_0 +
a_1 x, 2(a_0 + a_1) − (a_0 + 2a_1) = a_0. The example prints these two
values as `…02` and `…364140`.

<!-- source: coax/lib/bitcoin/frost.coax lagrange -->
```jai
// An identifier, or a difference of two, as a scalar: x may be negative.
fr_scalar :: (x: int) -> ScScalar {
  if x >= 0 { return sc_small(cast(u32) x); }
  return sc_negate(sc_small(cast(u32) (0 - x)));
}
// Participant id holds the polynomial's value at x = id + 1; the secret
// is the value at x = 0, which is id -1. The interpolating value of id
// in the set ids, for x = 0:
// lambda = prod (id_k + 1) / prod (id_k - id) over the other ids.
fr_lambda_vartime :: (ids: [] int, id: int) -> ScScalar {
  num := sc_small(1); den := sc_small(1);
  for k: 0..<ids.count {
    if ids[k] == id { continue; }
    num = sc_mul(num, fr_scalar(ids[k] + 1));
    den = sc_mul(den, fr_scalar(ids[k] - id));
  }
  inv := sc_inverse(den);
  #assert inv.ok;
  return sc_mul(num, inv.value);
}
// The public share at identifier x, interpolated from the points of
// ids: sum of L_i(x) X_i with L_i(x) = prod (x - id_k) / (id_i - id_k).
// It may be infinity.
fr_share_at_vartime :: (ids: [] int, points: [] SgPoint, x: int) -> SgPoint {
  q := sg_infinity();
  for i: 0..<ids.count {
    num := sc_small(1); den := sc_small(1);
    for k: 0..<ids.count {
      if k == i { continue; }
      num = sc_mul(num, fr_scalar(x - ids[k]));
      den = sc_mul(den, fr_scalar(ids[i] - ids[k]));
    }
    inv := sc_inverse(den);
    #assert inv.ok;
    q = sg_add(q, ecmult_vartime(sc_small(0), points[i], sc_mul(num, inv.value)).value).value;
  }
  return q;
}
// The threshold public key: the share at id -1, into out (33 bytes).
fr_thresh_pk_vartime :: (ids: [] int, points: [] SgPoint, out: [] u8) -> int {
  q := fr_share_at_vartime(ids, points, -1);
  if !ms_finite_vartime(q) { return FR_THRESH_INFINITY; }
  ms_cbytes_vartime(q, out);
  return FR_OK;
}
```

## The threshold key and the public shares

A signer set needs the threshold key, the threshold t and, to check
partial signatures, every participant's public share. BIP-445 calls
this the *threshold info* and gives a check for it: the public shares
that are present must lie on the one polynomial that the first t of
them fix, and that polynomial's value at −1 must be the threshold key.
Swapping two shares breaks the first rule; negating the key breaks the
second.

<!-- source: coax/lib/bitcoin/frost.coax info -->
```jai
// ValidateThresholdInfo: t, the threshold key and n public shares
// (present[i] false for a share that is not known, 33 bytes of anything
// in its place). Every present share must lie on the polynomial the
// first t present shares fix, and that polynomial's secret must be the
// threshold key.
fr_validate_info_vartime :: (t: int, thresh_pk: [] u8, pubshares: [] u8, present: [] bool) -> FrResult {
  n := present.count;
  if pubshares.count != 33 * n { return fr_fail(FR_CAPACITY, -1); }
  if t < 1 || t > n { return fr_fail(FR_THRESHOLD, -1); }
  if n > FR_MAX { return fr_fail(FR_TOO_MANY, -1); }
  if !ms_cpoint_vartime(thresh_pk).ok { return fr_fail(FR_THRESH_PK, -1); }
  ids: [FR_MAX] int; points: [FR_MAX] SgPoint; count := 0;
  for i: 0..<n {
    if !present[i] { continue; }
    p := ms_cpoint_vartime(pubshares[33*i..<33*i+33]);
    if !p.ok { return fr_fail(FR_PUBSHARE, i); }
    ids[count] = i; points[count] = p.value; count += 1;
  }
  if count < t { return fr_fail(FR_TOO_FEW_SHARES, -1); }
  a: [33] u8; b: [33] u8;
  for j: t..<count {
    ms_cbytes_vartime(fr_share_at_vartime(ids[0..<t], points[0..<t], ids[j]), a[..]);
    ms_cbytes_vartime(points[j], b[..]);
    if !bt_same(a[..], b[..]) { return fr_fail(FR_NOT_ON_POLY, -1); }
  }
  code := fr_thresh_pk_vartime(ids[0..<t], points[0..<t], a[..]);
  if code != FR_OK { return fr_fail(code, -1); }
  if !bt_same(a[..], thresh_pk) { return fr_fail(FR_KEY_MISMATCH, -1); }
  return fr_ok();
}
```

## Tweaks

The threshold key Q can be tweaked exactly as chapter 101's aggregate
key: an x-only tweak for Taproot (Q' = g·Q + t·G, with g = −1 when Q's y
is odd) and a plain tweak for BIP-32 (Q' = Q + t·G). The accumulators
gacc and tacc record what was done, so each signer can adjust its share
and the aggregator can add e·g·tacc once. `fr_taproot_tweak_vartime`
computes chapter 41's TapTweak of the threshold key, so the tweaked key
is a P2TR output key.

<!-- source: coax/lib/bitcoin/frost.coax tweak -->
```jai
// The tweak context: Q, and the accumulated sign gacc and tweak tacc, as
// in chapter 101. Each tweak gives Q' = g Q + t G, with g = n - 1 for an
// x-only tweak of a Q with an odd y and 1 otherwise.
FrTweak :: struct { code: int; q: SgPoint; gacc: ScScalar; tacc: ScScalar; }
fr_tweaks_vartime :: (thresh_pk: [] u8, tweaks: [] u8, xonly: [] bool) -> FrTweak {
  if tweaks.count % 32 != 0 || xonly.count > FR_TWEAKS { return FrTweak.{code=FR_CAPACITY}; }
  if tweaks.count != 32 * xonly.count { return FrTweak.{code=FR_TWEAK_LISTS}; }
  p := ms_cpoint_vartime(thresh_pk);
  if !p.ok { return FrTweak.{code=FR_THRESH_PK}; }
  k := FrTweak.{code=FR_OK, q=p.value, gacc=sc_small(1)};
  for i: 0..<xonly.count {
    g := ifx xonly[i] then ms_parity_vartime(k.q) else sc_small(1);
    t := sc_load(tweaks[32*i..<32*i+32]);
    if !t.ok { return FrTweak.{code=FR_TWEAK_RANGE}; }
    q := ecmult_vartime(t.value, k.q, g).value;
    if !ms_finite_vartime(q) { return FrTweak.{code=FR_TWEAK_INFINITY}; }
    k = FrTweak.{code=FR_OK, q=q, gacc=sc_mul(g, k.gacc), tacc=sc_add(t.value, sc_mul(g, k.tacc))};
  }
  return k;
}
// The x-only tweak that makes the threshold key a Taproot output key
// (chapter 41): TapTweak(xbytes(Q) || merkle), merkle empty or 32 bytes.
fr_taproot_tweak_vartime :: (thresh_pk: [] u8, merkle: [] u8, out: [] u8) -> int {
  if (merkle.count != 0 && merkle.count != 32) || out.count != 32 { return FR_CAPACITY; }
  p := ms_cpoint_vartime(thresh_pk);
  if !p.ok { return FR_THRESH_PK; }
  m: [64] u8;
  ms_xbytes_vartime(p.value, m[0..<32]);
  for i: 0..<merkle.count { m[32 + i] = merkle[i]; }
  bt_tagged(BH_TAPTWEAK, m[0..<32+merkle.count], out);
  return FR_OK;
}
```

## The signer set

A FROST session runs with u signers, t ≤ u ≤ n, named by their ids. A
signer that knows the public shares checks that the set's shares
interpolate to the threshold key; a signer without them can still sign,
but cannot check others' partial signatures. The ids are a set, so the
hashes that include them sort them first.

<!-- source: coax/lib/bitcoin/frost.coax signers -->
```jai
// The signer set of one session: n participants, threshold t, the u
// signers' ids and, when known, their public shares (33 bytes each).
FrSigners :: struct { n: int; t: int; ids: [] int; pubshares: [] u8; has_pubshares: bool; thresh_pk: [] u8; }
// ValidateSignersCtx: t <= u <= n <= 128, ids in range and distinct, and
// the public shares, when given, interpolate to the threshold key.
fr_validate_signers_vartime :: (s: FrSigners) -> FrResult {
  u := s.ids.count;
  if s.t < 1 || s.t > s.n { return fr_fail(FR_THRESHOLD, -1); }
  if s.n > FR_MAX { return fr_fail(FR_TOO_MANY, -1); }
  if u < s.t || u > s.n { return fr_fail(FR_SIGNER_COUNT, -1); }
  if s.has_pubshares && s.pubshares.count % 33 != 0 { return fr_fail(FR_CAPACITY, -1); }
  if s.has_pubshares && s.pubshares.count != 33 * u { return fr_fail(FR_PUBSHARE_LISTS, -1); }
  points: [FR_MAX] SgPoint;
  for i: 0..<u {
    if s.ids[i] < 0 || s.ids[i] > s.n - 1 { return fr_fail(FR_ID_RANGE, i); }
    if s.has_pubshares {
      p := ms_cpoint_vartime(s.pubshares[33*i..<33*i+33]);
      if !p.ok { return fr_fail(FR_PUBSHARE, i); }
      points[i] = p.value;
    }
  }
  for i: 0..<u { for k: 0..<i { if s.ids[i] == s.ids[k] { return fr_fail(FR_ID_DUPLICATE, -1); } } }
  if s.has_pubshares {
    q: [33] u8;
    code := fr_thresh_pk_vartime(s.ids, points[0..<u], q[..]);
    if code != FR_OK { return fr_fail(code, -1); }
    if !bt_same(q[..], s.thresh_pk) { return fr_fail(FR_KEY_MISMATCH, -1); }
  }
  return fr_ok();
}
// The position of id in ids, or -1.
fr_position :: (ids: [] int, id: int) -> int {
  for i: 0..<ids.count { if ids[i] == id { return i; } }
  return -1;
}
// The ids, sorted, 4 bytes each, added to a hash: the signers are a set,
// so their order does not change b or a deterministic nonce.
fr_add_ids :: (w: [] ShaOwned, ids: [] int) {
  sorted: [FR_MAX] int;
  for i: 0..<ids.count {
    j := i;
    while j > 0 && sorted[j-1] > ids[i] { sorted[j] = sorted[j-1]; j -= 1; }
    sorted[j] = ids[i];
  }
  for i: 0..<ids.count { ms_add_length(w, sorted[i], 4); }
}
```

## Nonces

FROST signs in two rounds as MuSig2 does, and for the same reason
(chapter 101's [Why two nonces](ch101-musig2.md#why-two-nonces)): each
signer sends two nonce points, and the final nonce R = R_1 + b·R_2 is
fixed only after all are known. NonceGen is chapter 101's with the
signer's id and public share in place of its key and BIP-445's tags.

<!-- source: coax/lib/bitcoin/frost.coax noncegen -->
```jai
// k_i = hash_BIP0445/nonce(rand || len(id) || id || len(pubshare) ||
// pubshare || len(thresh_pk) || thresh_pk || msg_prefixed ||
// len(extra_in) || extra_in || i) mod n: chapter 101's NonceGen with the
// signer's id and public share in place of its key. Lengths are public;
// rand is secret.
fr_nonce_hash :: (w: [] MsNonceWork, signer_id: int, pubshare: [] u8, xonly: [] u8, msg: [] u8, has_msg: bool, extra: [] u8, i: int) -> ScScalar {
  s := w[0].sha[..];
  ms_tag(s, "BIP0445/nonce"); ms_add(s, w[0].rand[..]);
  if signer_id < 0 { ms_add_length(s, 0, 1); }
  else { ms_add_length(s, 4, 1); ms_add_length(s, signer_id, 4); }
  ms_add_length(s, pubshare.count, 1); ms_add(s, pubshare);
  ms_add_length(s, xonly.count, 1); ms_add(s, xonly);
  if has_msg { ms_add_length(s, 1, 1); ms_add_length(s, msg.count, 8); ms_add(s, msg); }
  else { ms_add_length(s, 0, 1); }
  ms_add_length(s, extra.count, 4); ms_add(s, extra);
  ms_add_length(s, i, 1);
  ms_end(s, w[0].h[..]);
  return sc_reduce_bytes(w[0].h[..]).value;
}
// NonceGen. rand_ is 32 fresh random bytes, never used again. secshare
// (0 or 32 bytes), signer_id (-1 for none), pubshare (0 or 33), xonly,
// the tweaked threshold key (0 or 32), msg (when has_msg) and extra are
// optional inputs that make a repeated rand_ less harmful. Writes
// secnonce (k_1 and k_2, 64 bytes), which stays secret and signs once,
// and pubnonce (k_1 G and k_2 G, 66 bytes), which goes to the others.
fr_nonce_gen :: (rand_: [] u8, secshare: [] u8, signer_id: int, pubshare: [] u8, xonly: [] u8, msg: [] u8, has_msg: bool, extra: [] u8, secnonce: [] u8, pubnonce: [] u8, w: [] MsNonceWork) -> int {
  if w.count != 1 || rand_.count != 32 || (secshare.count != 0 && secshare.count != 32) || signer_id < -1 || signer_id >= FR_MAX { return FR_CAPACITY; }
  if (pubshare.count != 0 && pubshare.count != 33) || (xonly.count != 0 && xonly.count != 32) { return FR_CAPACITY; }
  if secnonce.count != 64 || pubnonce.count != 66 || extra.count > 0xffffffff { return FR_CAPACITY; }
  for j: 0..<32 { w[0].rand[j] = rand_[j]; }
  if secshare.count == 32 {
    ms_tag(w[0].sha[..], "BIP0445/aux"); ms_add(w[0].sha[..], rand_); ms_end(w[0].sha[..], w[0].aux[..]);
    for j: 0..<32 { w[0].rand[j] = secshare[j] ^ w[0].aux[j]; }
  }
  w[0].valid = 1;
  for i: 0..<2 {
    k := fr_nonce_hash(w, signer_id, pubshare, xonly, msg, has_msg, extra, i);
    nonzero := ec_nonzero(k);
    w[0].valid &= nonzero;
    w[0].k[i] = sc_select(nonzero, k, sc_small(1));
  }
  #assert ec_declassify(w[0].valid);
  for i: 0..<2 {
    ms_nonce_point(w[0].k[i], w[0].point[..], pubnonce[33*i..<33*i+33]);
    #assert ss_write(w[0].k[i], secnonce[32*i..<32*i+32]);
  }
  ms_nonce_clear(w);
  return FR_OK;
}
```

NonceAgg adds the signers' first and second nonce points, as chapter
101's does.

<!-- source: coax/lib/bitcoin/frost.coax nonceagg -->
```jai
// NonceAgg: R_j is the sum of every signer's j-th nonce point, written
// with cbytes_ext, so the sum may be infinity. Into out (66 bytes). A
// nonce that does not decode refuses, naming its signer's position.
fr_nonce_agg_vartime :: (pubnonces: [] u8, out: [] u8) -> FrResult {
  u := pubnonces.count / 66;
  if pubnonces.count % 66 != 0 || u < 1 || u > FR_MAX || out.count != 66 { return fr_fail(FR_CAPACITY, -1); }
  for j: 0..<2 {
    r := sg_infinity();
    for i: 0..<u {
      p := ms_cpoint_vartime(pubnonces[66*i+33*j..<66*i+33*j+33]);
      if !p.ok { return fr_fail(FR_BAD_PUBNONCE, i); }
      r = sg_add(r, p.value).value;
    }
    ms_cbytes_vartime(r, out[33*j..<33*j+33]);
  }
  return fr_ok();
}
```

## Binding factors and the session

MuSig2's b hashes the aggregate nonce, the key and the message. FROST's
also hashes the signer set: b = hash_BIP0445/noncecoef(u ‖ sorted ids ‖
aggnonce ‖ xbytes(Q) ‖ msg). In a threshold scheme the same nonces could
be offered to different subsets; binding the set means a nonce committed
for one set gives a different R in any other. The example shows it: the
same aggregate nonce with participants 0 and 1 instead of 0 and 2 gives
another b.

RFC 9591 computes one binding factor per signer, ρ_i, and R = Σ (D_i +
ρ_i E_i). BIP-445's FROST3 uses one b for everyone, which is why its
nonces can be added first and why it shares MuSig2's NonceAgg.

<!-- source: coax/lib/bitcoin/frost.coax session -->
```jai
// GetSessionValues: the signer set checked, the tweaked key,
// b = hash_BIP0445/noncecoef(u || sorted ids || aggnonce || xbytes(Q) ||
// msg), the final nonce R = R_1 + b R_2 (G if that is infinity) and
// BIP-340's challenge e. Unlike MuSig2's b, FROST's binds the signer set:
// the same nonces in a different set give a different R.
FrSession :: struct { code: int; signer: int; q: SgPoint; gacc: ScScalar; tacc: ScScalar; b: ScScalar; r: SgPoint; e: ScScalar; }
fr_session_vartime :: (s: FrSigners, aggnonce: [] u8, tweaks: [] u8, xonly: [] bool, msg: [] u8) -> FrSession {
  x: FrSession; x.signer = -1;
  v := fr_validate_signers_vartime(s);
  if v.code != FR_OK { x.code = v.code; x.signer = v.signer; return x; }
  k := fr_tweaks_vartime(s.thresh_pk, tweaks, xonly);
  if k.code != FR_OK { x.code = k.code; return x; }
  if aggnonce.count != 66 { x.code = FR_CAPACITY; return x; }
  x.q = k.q; x.gacc = k.gacc; x.tacc = k.tacc;
  qx: [32] u8; rx: [32] u8; w: [1] ShaOwned;
  ms_xbytes_vartime(k.q, qx[..]);
  ms_tag(w[..], "BIP0445/noncecoef");
  ms_add_length(w[..], s.ids.count, 4); fr_add_ids(w[..], s.ids);
  ms_add(w[..], aggnonce); ms_add(w[..], qx[..]); ms_add(w[..], msg);
  x.b = ms_reduce(w[..]);
  r1 := ms_cpoint_ext_vartime(aggnonce[0..<33]);
  r2 := ms_cpoint_ext_vartime(aggnonce[33..<66]);
  if !r1.ok || !r2.ok { x.code = FR_BAD_AGGNONCE; return x; }
  r := sg_add(r1.value, ecmult_vartime(sc_small(0), r2.value, x.b).value).value;
  x.r = ifx ms_finite_vartime(r) then r else sg_generator();
  ms_xbytes_vartime(x.r, rx[..]);
  ms_tag(w[..], "BIP0340/challenge"); ms_add(w[..], rx[..]); ms_add(w[..], qx[..]); ms_add(w[..], msg);
  x.e = ms_reduce(w[..]);
  x.code = FR_OK;
  return x;
}
```

## Signing

Each signer's partial signature is

s_i = k_1 + b·k_2 + e·λ_i·d_i,

where λ_i is its interpolating value in this signer set, and d_i is its
secret share times g·gacc (g = −1 when Q has an odd y). The shares add
up to a signature with the secret a_0: Σ λ_i d_i = g·gacc·a_0, which is
the key Q's x-only secret. A share alone is useless to an attacker, but a
secret nonce used twice gives the share away, as chapter 101's example
shows for a key. Sign reads both halves of the secret nonce and then
overwrites it with zeros, so a second call refuses.

<!-- source: coax/lib/bitcoin/frost.coax signshare -->
```jai
// Sign: s = k_1 + b k_2 + e lambda d mod n, where the k_i are negated when
// R has an odd y, lambda is the signer's interpolating value in this
// signer set, and d is the secret share times g (n - 1 when Q has an odd
// y) and gacc. Into psig (32 bytes). It zeroes secnonce once its halves
// are read, so a secnonce signs at most once.
fr_sign :: (secnonce: [] u8, secshare: [] u8, signer_id: int, s: FrSigners, aggnonce: [] u8, tweaks: [] u8, xonly: [] bool, msg: [] u8, psig: [] u8, w: [] MsSignWork) -> FrResult {
  if w.count != 1 || secnonce.count != 64 || secshare.count != 32 || psig.count != 32 { return fr_fail(FR_CAPACITY, -1); }
  x := fr_session_vartime(s, aggnonce, tweaks, xonly, msg);
  if x.code != FR_OK { return fr_fail(x.code, x.signer); }
  #assert ss_key(secnonce[0..<32], w[0].k[0..<1], false) && ss_key(secnonce[32..<64], w[0].k[1..<2], false);
  if !ec_declassify(w[0].k[0].valid) { return fr_sign_clear(w, FR_SECNONCE_1); }
  if !ec_declassify(w[0].k[1].valid) { return fr_sign_clear(w, FR_SECNONCE_2); }
  for j: 0..<64 { secnonce[j] = 0; }
  #assert ss_key(secshare, w[0].d[..], false);
  if !ec_declassify(w[0].d[0].valid) { return fr_sign_clear(w, FR_SECSHARE); }
  // The signer's public share, d G, must be its entry in the list.
  ms_nonce_point(w[0].d[0].safe, w[0].point[..], w[0].pk[..]);
  at := fr_position(s.ids, signer_id);
  if at < 0 { return fr_sign_clear(w, FR_ID_MISSING); }
  if s.has_pubshares && !bt_same(s.pubshares[33*at..<33*at+33], w[0].pk[..]) { return fr_sign_clear(w, FR_PUBSHARE_MISMATCH); }
  lambda := fr_lambda_vartime(s.ids, signer_id);
  r_odd := cast(u64) (ifx ms_odd_vartime(x.r) then 1 else 0);   // public
  k1 := ss_even(w[0].k[0].safe, r_odd);
  k2 := ss_even(w[0].k[1].safe, r_odd);
  d := sc_mul(sc_mul(ms_parity_vartime(x.q), x.gacc), w[0].d[0].safe);
  w[0].s = sc_add(sc_add(k1, sc_mul(x.b, k2)), sc_mul(sc_mul(x.e, lambda), d));
  #assert ss_write(w[0].s, psig);
  return fr_sign_clear(w, FR_OK);
}
fr_sign_clear :: (w: [] MsSignWork, code: int) -> FrResult { fresh: MsSignWork; w[0] = fresh; return fr_fail(code, -1); }
```

## Checking a partial signature

A signer that holds the public shares checks each partial signature as
it arrives: s_i·G = R_e,i + e·λ_i·g·gacc·X_i, with R_e,i its nonce
R_1,i + b·R_2,i, negated when R has an odd y. A partial signature that
fails names the signer who sent it. BIP-445 calls this *identifiable
abort*: the honest signers learn whom to exclude and start again with
another subset.

<!-- source: coax/lib/bitcoin/frost.coax verify -->
```jai
// PartialSigVerifyInternal: s G = Re + e lambda g gacc X, where Re = R_1 +
// b R_2 from this signer's nonce, negated when R has an odd y, and X is
// its public share. Anything that does not decode is FR_INVALID.
fr_partial_verify_session_vartime :: (psig: [] u8, signer_id: int, pubnonce: [] u8, pubshare: [] u8, x: FrSession, ids: [] int) -> FrResult {
  s := sc_load(psig);
  if psig.count != 32 || !s.ok || pubnonce.count != 66 { return fr_fail(FR_INVALID, -1); }
  r1 := ms_cpoint_vartime(pubnonce[0..<33]);
  r2 := ms_cpoint_vartime(pubnonce[33..<66]);
  if !r1.ok || !r2.ok { return fr_fail(FR_INVALID, -1); }
  re := sg_add(r1.value, ecmult_vartime(sc_small(0), r2.value, x.b).value).value;
  if ms_odd_vartime(x.r) { re = sg_negate(re).value; }
  p := ms_cpoint_vartime(pubshare);
  if !p.ok { return fr_fail(FR_INVALID, -1); }
  if fr_position(ids, signer_id) < 0 { return fr_fail(FR_ID_MISSING, -1); }
  c := sc_mul(sc_mul(x.e, fr_lambda_vartime(ids, signer_id)), sc_mul(ms_parity_vartime(x.q), x.gacc));
  lhs := ecmult_vartime(s.value, p.value, sc_negate(c)).value;
  return ifx sg_equal_mask(lhs, re) == CB_MAX64 then fr_ok() else fr_fail(FR_INVALID, -1);
}
// PartialSigVerify: the partial signature of the signer at position i,
// from everyone's public nonces and the session's public shares.
fr_partial_verify_vartime :: (psig: [] u8, pubnonces: [] u8, s: FrSigners, tweaks: [] u8, xonly: [] bool, msg: [] u8, i: int) -> FrResult {
  u := s.ids.count;
  if !s.has_pubshares || pubnonces.count != 66 * u || s.pubshares.count != 33 * u || i < 0 || i >= u { return fr_fail(FR_CAPACITY, -1); }
  if tweaks.count != 32 * xonly.count { return fr_fail(FR_TWEAK_LISTS, -1); }
  v := fr_validate_signers_vartime(s);
  if v.code != FR_OK { return v; }
  agg: [66] u8;
  a := fr_nonce_agg_vartime(pubnonces, agg[..]);
  if a.code != FR_OK { return a; }
  x := fr_session_vartime(s, agg[..], tweaks, xonly, msg);
  if x.code != FR_OK { return fr_fail(x.code, x.signer); }
  return fr_partial_verify_session_vartime(psig, s.ids[i], pubnonces[66*i..<66*i+66], s.pubshares[33*i..<33*i+33], x, s.ids);
}
```

## Aggregation

The aggregator adds the partial signatures and e·g·tacc for the tweaks.
The result is a BIP-340 signature xbytes(R) ‖ s that chapter 23's
verifier accepts under the tweaked key.

<!-- source: coax/lib/bitcoin/frost.coax aggregate -->
```jai
// PartialSigAgg: s = s_1 + ... + s_u + e g tacc mod n, and the BIP-340
// signature xbytes(R) || s into out (64 bytes).
fr_partial_agg_vartime :: (psigs: [] u8, x: FrSession, u: int, out: [] u8) -> FrResult {
  if x.code != FR_OK { return fr_fail(x.code, x.signer); }
  if psigs.count % 32 != 0 || out.count != 64 { return fr_fail(FR_CAPACITY, -1); }
  if psigs.count / 32 != u { return fr_fail(FR_PSIG_LISTS, -1); }
  s: ScScalar;
  for i: 0..<u {
    si := sc_load(psigs[32*i..<32*i+32]);
    if !si.ok { return fr_fail(FR_BAD_PSIG, i); }
    s = sc_add(s, si.value);
  }
  s = sc_add(s, sc_mul(sc_mul(x.e, ms_parity_vartime(x.q)), x.tacc));
  ms_xbytes_vartime(x.r, out[0..<32]);
  #assert sc_store(s, out[32..<64]);
  return fr_ok();
}
```

## DeterministicSign

The last signer, who has seen every other nonce, can make its nonce from
a hash of everything instead of fresh randomness. Its secret share enters
the hash already multiplied by g·gacc, masked by aux randomness when
there is any. As in chapter 101, this is safe only for the signer who
goes last: an earlier one would let a later one change the other nonces
and get two signatures under one nonce.

<!-- source: coax/lib/bitcoin/frost.coax detsign -->
```jai
// The workspace of DeterministicSign. Each return clears it.
FrDetWork :: struct { nonce: [1] MsNonceWork; d: [1] SignScalar; masked: [32] u8; secnonce: [64] u8; sign: [1] MsSignWork; }
fr_det_clear :: (w: [] FrDetWork, r: FrResult) -> FrResult { fresh: FrDetWork; w[0] = fresh; return r; }
// DeterministicSign, for the last signer, who has seen every other nonce
// (aggother, their NonceAgg; absent when it signs alone) and has no
// randomness: k_i = hash_BIP0445/deterministic/nonce(d' || id || u ||
// sorted ids || len(aggother) || aggother || xbytes(Q) || len(msg) || msg
// || i) mod n, with d' = g gacc d, XORed with hash_BIP0445/aux(aux) when
// aux (0 or 32 bytes) is given. Writes pubnonce (66 bytes) and psig (32).
fr_det_sign :: (secshare: [] u8, signer_id: int, aggother: [] u8, has_aggother: bool, s: FrSigners, tweaks: [] u8, xonly: [] bool, msg: [] u8, aux: [] u8, has_aux: bool, pubnonce: [] u8, psig: [] u8, w: [] FrDetWork) -> FrResult {
  if w.count != 1 || pubnonce.count != 66 || psig.count != 32 { return fr_fail(FR_CAPACITY, -1); }
  v := fr_validate_signers_vartime(s);
  if v.code != FR_OK { return v; }
  u := s.ids.count;
  if u == 1 && has_aggother { return fr_fail(FR_AGGOTHER_OMIT, -1); }
  if u > 1 && (!has_aggother || aggother.count != 66) { return fr_fail(FR_AGGOTHER_PRESENT, -1); }
  if secshare.count != 32 || (has_aux && aux.count != 32) { return fr_fail(FR_CAPACITY, -1); }
  k := fr_tweaks_vartime(s.thresh_pk, tweaks, xonly);
  if k.code != FR_OK { return fr_fail(k.code, -1); }
  aggpk: [32] u8;
  ms_xbytes_vartime(k.q, aggpk[..]);
  #assert ss_key(secshare, w[0].d[..], false);
  if !ec_declassify(w[0].d[0].valid) { return fr_det_clear(w, fr_fail(FR_SECSHARE, -1)); }
  #assert ss_write(sc_mul(sc_mul(ms_parity_vartime(k.q), k.gacc), w[0].d[0].safe), w[0].masked[..]);
  n := w[0].nonce[..]; h := n[0].sha[..];
  if has_aux {
    ms_tag(h, "BIP0445/aux"); ms_add(h, aux); ms_end(h, n[0].aux[..]);
    for j: 0..<32 { w[0].masked[j] ^= n[0].aux[j]; }
  }
  other := ifx has_aggother then aggother else aggother[0..<0];
  n[0].valid = 1;
  for i: 0..<2 {
    ms_tag(h, "BIP0445/deterministic/nonce"); ms_add(h, w[0].masked[..]);
    ms_add_length(h, signer_id, 4); ms_add_length(h, u, 4); fr_add_ids(h, s.ids);
    ms_add_length(h, other.count, 1); ms_add(h, other); ms_add(h, aggpk[..]);
    ms_add_length(h, msg.count, 8); ms_add(h, msg); ms_add_length(h, i, 1);
    ms_end(h, n[0].h[..]);
    ki := sc_reduce_bytes(n[0].h[..]).value;
    nonzero := ec_nonzero(ki);
    n[0].valid &= nonzero;
    n[0].k[i] = sc_select(nonzero, ki, sc_small(1));
  }
  #assert ec_declassify(n[0].valid);
  for i: 0..<2 {
    ms_nonce_point(n[0].k[i], n[0].point[..], pubnonce[33*i..<33*i+33]);
    #assert ss_write(n[0].k[i], w[0].secnonce[32*i..<32*i+32]);
  }
  agg: [66] u8;
  if has_aggother {
    both: [132] u8;
    for j: 0..<66 { both[j] = pubnonce[j]; both[66 + j] = aggother[j]; }
    if fr_nonce_agg_vartime(both[..], agg[..]).code != FR_OK { return fr_det_clear(w, fr_fail(FR_BAD_AGGOTHERNONCE, -1)); }
  } else {
    for j: 0..<66 { agg[j] = pubnonce[j]; }
  }
  return fr_det_clear(w, fr_sign(w[0].secnonce[..], secshare, signer_id, s, agg[..], tweaks, xonly, msg, psig, w[0].sign[..]));
}
```

## Key generation

### The trusted dealer

A dealer picks the secret, hashes the other t − 1 coefficients from it
(BIP-445's `trusted_dealer.py` uses the tag `BIP0445/trusted/keygen` and
a 4-byte index), and hands each participant f(id + 1). The threshold key
is a_0·G. It is simple and it is what the BIP's vectors use, but the
dealer knows the secret: for that moment the coins have one key.

<!-- source: coax/lib/bitcoin/frost.coax dealer -->
```jai
// The polynomial of a dealer with secret a_0: f(x) = a_0 + a_1 x + ... +
// a_{t-1} x^{t-1}, its coefficients hashed from the secret,
// a_k = hash_BIP0445/trusted/keygen(a_0 || (t - k) as 4 bytes), as the
// BIP's trusted_dealer.py does. The workspace holds them; each caller
// clears it before it returns.
FrDealWork :: struct { sha: [1] ShaOwned; key: [1] SignScalar; c: [FR_MAX] ScScalar; h: [32] u8; y: ScScalar; point: [1] SignPoint; valid: u64; }
fr_deal_clear :: (w: [] FrDealWork, code: int) -> int { fresh: FrDealWork; w[0] = fresh; return code; }
fr_poly :: (secret: [] u8, t: int, w: [] FrDealWork) -> bool {
  #assert ss_key(secret, w[0].key[..], false);
  w[0].c[0] = w[0].key[0].safe;
  w[0].valid = w[0].key[0].valid;
  for k: 1..<t {
    ms_tag(w[0].sha[..], "BIP0445/trusted/keygen"); ms_add(w[0].sha[..], secret); ms_add_length(w[0].sha[..], t - k, 4);
    ms_end(w[0].sha[..], w[0].h[..]);
    #assert ss_key(w[0].h[..], w[0].key[..], false);
    w[0].valid &= w[0].key[0].valid;   // a hash of 0 or above n: negligible
    w[0].c[k] = w[0].key[0].safe;
  }
  return ec_declassify(w[0].valid);
}
// f(x) by Horner's rule, from the highest coefficient down.
fr_poly_at :: (w: [] FrDealWork, t: int, x: int) -> ScScalar {
  y: ScScalar;
  for k: 0..<t { y = sc_add(sc_mul(y, fr_scalar(x)), w[0].c[t - 1 - k]); }
  return y;
}
// The trusted dealer: from a secret (32 bytes, 0 < secret < n), the
// threshold key (33 bytes), each participant's secret share f(id + 1)
// (32 bytes each) and public share (33 bytes each). Whoever runs it
// knows the secret: every share comes from one machine.
fr_dealer :: (secret: [] u8, n: int, t: int, thresh_pk: [] u8, secshares: [] u8, pubshares: [] u8, w: [] FrDealWork) -> int {
  if w.count != 1 || secret.count != 32 || t < 1 || t > n || n > FR_MAX || thresh_pk.count != 33 || secshares.count != 32 * n || pubshares.count != 33 * n { return FR_CAPACITY; }
  if !fr_poly(secret, t, w) { return fr_deal_clear(w, FR_SECSHARE); }
  ms_nonce_point(w[0].c[0], w[0].point[..], thresh_pk);
  for j: 0..<n {
    w[0].y = fr_poly_at(w, t, j + 1);
    #assert ec_declassify(ec_nonzero(w[0].y));   // a share of 0: negligible
    #assert ss_write(w[0].y, secshares[32*j..<32*j+32]);
    ms_nonce_point(w[0].y, w[0].point[..], pubshares[33*j..<33*j+33]);
  }
  return fr_deal_clear(w, FR_OK);
}
// Any t shares give the secret back: sum of lambda_i s_i. This is what
// FROST never does; the chapter uses it to show the shares are Shamir's.
fr_combine :: (secshares: [] u8, ids: [] int, out: [] u8) -> bool {
  if secshares.count != 32 * ids.count || out.count != 32 { return false; }
  k: [1] SignScalar; s: ScScalar;
  for i: 0..<ids.count {
    #assert ss_key(secshares[32*i..<32*i+32], k[..], false);
    s = sc_add(s, sc_mul(fr_lambda_vartime(ids, ids[i]), k[0].safe));
  }
  fresh: SignScalar; k[0] = fresh;
  return ss_write(s, out);
}
```

### Distributed key generation

A Pedersen DKG removes the dealer. Every participant i is the dealer of
a secret of its own, a_i,0, with a polynomial f_i, and the group's
secret is the sum Σ a_i,0, which nobody ever computes:

1. **Round 1.** Participant i publishes commitments C_i,k = a_i,k·G to
   its coefficients (Feldman's verifiable secret sharing), and a proof
   that it knows a_i,0. It sends participant j the share f_i(j + 1) over
   a private channel.
2. **Checks.** Participant j checks each sender's proof, and checks that
   its share lies on the committed polynomial: f_i(j + 1)·G =
   Σ_k (j + 1)^k·C_i,k. A failure names the sender.
3. **Round 2.** Participant j's secret share is Σ_i f_i(j + 1); the
   threshold key is Σ_i C_i,0; every public share is computed from the
   commitments alone.

The proof of possession stops the rogue-key attack of chapter 101: a
participant who announces C_i,0 = X − Σ_{others} C_k,0 for a key X it
controls would have to sign with a_i,0, which it does not know. KG20's
FROST uses a Schnorr proof of knowledge; this chapter uses a BIP-340
signature, as SimplPedPop in the ChillDKG draft does, by a_i,0 over
hash_FROST/dkg/pop(context ‖ i ‖ C_i,0 ‖ … ‖ C_i,t−1), with zero
auxiliary randomness. The context names the session, so a proof can't be
replayed into another.

This DKG is a teaching protocol, not a standard. It assumes the private
channels exist and that every participant sees the same commitments.
ChillDKG (`BlockstreamResearch/bip-frost-dkg`) adds the parts a wallet
needs: encrypted shares, a check that everyone received the same
messages, and recovery data.

<!-- source: coax/lib/bitcoin/frost.coax dkg -->
```jai
// A Pedersen DKG: participant i deals a secret of its own to everyone.
// Round 1: from a 32-byte secret seed, it publishes commitments to its
// polynomial's coefficients, C_k = a_k G (33 bytes each, t of them), and
// a proof of possession of a_0: a BIP-340 signature by a_0 of
// hash_FROST/dkg/pop(context || i || C_0 || ... || C_{t-1}). It sends
// participant j the share f_i(j + 1) privately (32 bytes each, n).
FrDkgWork :: struct { deal: [1] FrDealWork; sign: [1] SignWork; msg: [32] u8; key: [32] u8; }
fr_dkg_message_vartime :: (context: [] u8, index: int, commit: [] u8, out: [] u8) {
  w: [1] ShaOwned;
  ms_tag(w[..], "FROST/dkg/pop"); ms_add(w[..], context); ms_add_length(w[..], index, 4); ms_add(w[..], commit);
  ms_end(w[..], out);
}
fr_dkg_round1 :: (seed: [] u8, index: int, n: int, t: int, context: [] u8, commit: [] u8, pop: [] u8, shares: [] u8, w: [] FrDkgWork) -> int {
  if w.count != 1 || seed.count != 32 || t < 1 || t > n || n > FR_MAX || index < 0 || index >= n { return FR_CAPACITY; }
  if commit.count != 33 * t || pop.count != 64 || shares.count != 32 * n { return FR_CAPACITY; }
  d := w[0].deal[..];
  if !fr_poly(seed, t, d) { fresh: FrDkgWork; w[0] = fresh; return FR_SECSHARE; }
  for k: 0..<t { ms_nonce_point(d[0].c[k], d[0].point[..], commit[33*k..<33*k+33]); }
  for j: 0..<n {
    d[0].y = fr_poly_at(d, t, j + 1);
    #assert ss_write(d[0].y, shares[32*j..<32*j+32]);
  }
  fr_dkg_message_vartime(context, index, commit, w[0].msg[..]);
  #assert ss_write(d[0].c[0], w[0].key[..]);
  aux: [32] u8;
  status := SIGN_REFUSED;
  for k: 0..<6 { status = schnorr_sign_step(w[0].key[..], aux[..], w[0].msg[..], w[0].sign[..], pop, false); }
  fresh: FrDkgWork; w[0] = fresh;
  return ifx status == SIGN_DONE then FR_OK else FR_CAPACITY;
}
// Receiver j checks sender i: the proof of possession verifies under
// xbytes(C_0), and its share s satisfies s G = sum (j + 1)^k C_k.
fr_dkg_check_vartime :: (commit: [] u8, pop: [] u8, context: [] u8, index: int, share: [] u8, j: int) -> int {
  t := commit.count / 33;
  if commit.count % 33 != 0 || t < 1 || t > FR_MAX || pop.count != 64 || share.count != 32 || j < 0 { return FR_CAPACITY; }
  m: [32] u8; c0: [32] u8;
  fr_dkg_message_vartime(context, index, commit, m[..]);
  for k: 0..<32 { c0[k] = commit[1 + k]; }
  if !schnorr_verify_vartime(m[..], c0[..], pop) { return FR_DKG_POP; }
  want := fr_dkg_point_vartime(commit, j);
  k: [1] SignScalar; p: [1] SignPoint; got: [33] u8; expect: [33] u8;
  #assert ss_key(share, k[..], false);
  ok := ec_declassify(k[0].valid);
  ms_nonce_point(k[0].safe, p[..], got[..]);
  fresh: SignScalar; k[0] = fresh;
  ms_cbytes_vartime(want, expect[..]);
  return ifx ok && bt_same(got[..], expect[..]) then FR_OK else FR_DKG_SHARE;
}
// sum (j + 1)^k C_k: participant j's public share of one dealer.
fr_dkg_point_vartime :: (commit: [] u8, j: int) -> SgPoint {
  q := sg_infinity(); x := sc_small(1);
  for k: 0..<commit.count/33 {
    c := ms_cpoint_vartime(commit[33*k..<33*k+33]);
    #assert c.ok;
    q = sg_add(q, ecmult_vartime(sc_small(0), c.value, x).value).value;
    x = sc_mul(x, fr_scalar(j + 1));
  }
  return q;
}
// Round 2, for participant j, once every check passed: its secret share
// is the sum of the n shares it received (32 bytes each, in order of
// sender), the threshold key is the sum of the C_0, and every public
// share is the sum of the dealers' public shares. commits holds the n
// senders' commitments, 33 t bytes each.
fr_dkg_finish :: (received: [] u8, commits: [] u8, n: int, t: int, secshare: [] u8, thresh_pk: [] u8, pubshares: [] u8) -> int {
  if received.count != 32 * n || commits.count != 33 * t * n || secshare.count != 32 || thresh_pk.count != 33 || pubshares.count != 33 * n || n > FR_MAX { return FR_CAPACITY; }
  k: [1] SignScalar; s: ScScalar;
  for i: 0..<n {
    #assert ss_key(received[32*i..<32*i+32], k[..], false);
    s = sc_add(s, k[0].safe);
  }
  fresh: SignScalar; k[0] = fresh;
  #assert ss_write(s, secshare);
  q := sg_infinity();
  for i: 0..<n {
    c := ms_cpoint_vartime(commits[33*t*i..<33*t*i+33]);
    if !c.ok { return FR_THRESH_PK; }
    q = sg_add(q, c.value).value;
  }
  if !ms_finite_vartime(q) { return FR_THRESH_INFINITY; }
  ms_cbytes_vartime(q, thresh_pk);
  for j: 0..<n {
    p := sg_infinity();
    for i: 0..<n { p = sg_add(p, fr_dkg_point_vartime(commits[33*t*i..<33*t*(i+1)], j)).value; }
    ms_cbytes_vartime(p, pubshares[33*j..<33*j+33]);
  }
  return FR_OK;
}
```

## The vectors

BIP-445 publishes seven vector files. `make_frost_vectors.py` copies
every case into `frost_vectors.coax`, with the JSON's indices resolved
into the bytes they name. Each `frv_` procedure counts the cases of one
file that give their stated result: the value, or the error with the
index it names. A signing case that signs also checks that the same
secnonce then refuses, each deterministic partial signature is verified,
and each aggregate signature passes chapter 23's verifier.

<!-- source: coax/lib/bitcoin/frost.coax vectors -->
```jai
// Each vector file of the BIP: the number of its cases that give their
// stated result, a value or the refusal with the index it names.
frv_nonce_gen :: () -> int {
  passed := 0; w: [1] MsNonceWork;
  for i: 0..<FR_NONCE_GEN.count {
    c := FR_NONCE_GEN[i];
    r: [32] u8; sk: [32] u8; pk: [33] u8; xo: [32] u8; msg: [64] u8; extra: [64] u8; sec: [64] u8; pub: [66] u8;
    code := fr_nonce_gen(ms_hex(c.rand, r[..]), ms_hex(c.secshare, sk[..]), c.signer_id, ms_hex(c.pubshare, pk[..]), ms_hex(c.xonly, xo[..]),
      ms_hex(c.msg, msg[..]), c.has_msg, ms_hex(c.extra, extra[..]), sec[..], pub[..], w[..]);
    if code == FR_OK && bt_hex_is(sec[..], c.secnonce) && bt_hex_is(pub[..], c.pubnonce) { passed += 1; }
  }
  return passed;
}
frv_nonce_agg :: () -> int {
  passed := 0;
  for i: 0..<FR_NONCE_AGG.count {
    c := FR_NONCE_AGG[i];
    n: [512] u8; out: [66] u8;
    r := fr_nonce_agg_vartime(ms_hex(c.nonces, n[..]), out[..]);
    if r.code == c.code && r.signer == c.signer && (r.code != FR_OK || bt_hex_is(out[..], c.expected)) { passed += 1; }
  }
  return passed;
}
// Sign, from sign_verify_vectors and tweak_vectors, cases lo to hi - 1.
// A case that signs also refuses a second time with the same secnonce.
frv_sign :: (lo: int, hi: int) -> int {
  passed := 0; w: [1] MsSignWork;
  for i: lo..<hi {
    c := FR_SIGN[i];
    ids: [16] int; ps: [528] u8; tp: [33] u8; sec: [64] u8; sk: [32] u8; agg: [66] u8; t: [256] u8; f: [8] bool; m: [64] u8; psig: [32] u8;
    s := FrSigners.{n=c.n, t=c.t, ids=fr_ids(c.ids, ids[..]), pubshares=ms_hex(c.pubshares, ps[..]), has_pubshares=c.has_pubshares, thresh_pk=ms_hex(c.thresh_pk, tp[..])};
    tweaks := ms_hex(c.tweaks, t[..]); xonly := fr_flags(c.xonly, f[..]); msg := ms_hex(c.msg, m[..]);
    r := fr_sign(ms_hex(c.secnonce, sec[..]), ms_hex(c.secshare, sk[..]), c.signer_id, s, ms_hex(c.aggnonce, agg[..]), tweaks, xonly, msg, psig[..], w[..]);
    ok := r.code == c.code && r.signer == c.signer;
    if r.code == FR_OK {
      ok = ok && bt_hex_is(psig[..], c.expected);
      ok = ok && fr_sign(sec[..], sk[..], c.signer_id, s, agg[..], tweaks, xonly, msg, psig[..], w[..]).code == FR_SECNONCE_1;
    }
    if ok { passed += 1; }
  }
  return passed;
}
// PartialSigVerify, cases lo to hi - 1.
frv_verify :: (lo: int, hi: int) -> int {
  passed := 0;
  for i: lo..<hi {
    c := FR_VERIFY[i];
    ids: [16] int; ps: [528] u8; tp: [33] u8; n: [1056] u8; t: [256] u8; f: [8] bool; m: [64] u8; sig: [32] u8;
    s := FrSigners.{n=c.n, t=c.t, ids=fr_ids(c.ids, ids[..]), pubshares=ms_hex(c.pubshares, ps[..]), has_pubshares=c.has_pubshares, thresh_pk=ms_hex(c.thresh_pk, tp[..])};
    r := fr_partial_verify_vartime(ms_hex(c.psig, sig[..]), ms_hex(c.pubnonces, n[..]), s, ms_hex(c.tweaks, t[..]), fr_flags(c.xonly, f[..]), ms_hex(c.msg, m[..]), c.index);
    if r.code == c.code && r.signer == c.signer { passed += 1; }
  }
  return passed;
}
// DeterministicSign, cases lo to hi - 1. A partial signature it makes
// also verifies.
frv_det_sign :: (lo: int, hi: int) -> int {
  passed := 0; w: [1] FrDetWork;
  for i: lo..<hi {
    c := FR_DET_SIGN[i];
    ids: [16] int; ps: [528] u8; tp: [33] u8; sk: [32] u8; o: [66] u8; t: [256] u8; f: [8] bool; m: [64] u8; ax: [32] u8; pub: [66] u8; psig: [32] u8;
    s := FrSigners.{n=c.n, t=c.t, ids=fr_ids(c.ids, ids[..]), pubshares=ms_hex(c.pubshares, ps[..]), has_pubshares=c.has_pubshares, thresh_pk=ms_hex(c.thresh_pk, tp[..])};
    tweaks := ms_hex(c.tweaks, t[..]); xonly := fr_flags(c.xonly, f[..]); msg := ms_hex(c.msg, m[..]); other := ms_hex(c.aggother, o[..]);
    key := ms_hex(c.secshare, sk[..]);
    r := fr_det_sign(key, c.signer_id, other, c.has_aggother, s, tweaks, xonly, msg, ms_hex(c.aux, ax[..]), c.has_aux, pub[..], psig[..], w[..]);
    ok := r.code == c.code && r.signer == c.signer;
    if r.code == FR_OK {
      ok = ok && bt_hex_is(pub[..], c.pubnonce) && bt_hex_is(psig[..], c.psig);
      agg: [66] u8;
      if c.has_aggother {
        both: [132] u8;
        for j: 0..<66 { both[j] = pub[j]; both[66 + j] = other[j]; }
        ok = ok && fr_nonce_agg_vartime(both[..], agg[..]).code == FR_OK;
      } else {
        for j: 0..<66 { agg[j] = pub[j]; }
      }
      // The signer knows its own public share even without the list.
      own: [33] u8; kw: [1] SignScalar; pw: [1] SignPoint;
      #assert ss_key(key, kw[..], false);
      ms_nonce_point(kw[0].safe, pw[..], own[..]);
      x := fr_session_vartime(s, agg[..], tweaks, xonly, msg);
      ok = ok && fr_partial_verify_session_vartime(psig[..], c.signer_id, pub[..], own[..], x, s.ids).code == FR_OK;
    }
    if ok { passed += 1; }
  }
  return passed;
}
// PartialSigAgg, cases lo to hi - 1. Each signature it makes is a valid
// BIP-340 signature under the tweaked threshold key.
frv_sig_agg :: (lo: int, hi: int) -> int {
  passed := 0;
  for i: lo..<hi {
    c := FR_SIG_AGG[i];
    ids: [16] int; ps: [528] u8; tp: [33] u8; agg: [66] u8; t: [256] u8; f: [8] bool; p: [512] u8; m: [64] u8; sig: [64] u8; q: [32] u8;
    s := FrSigners.{n=c.n, t=c.t, ids=fr_ids(c.ids, ids[..]), pubshares=ms_hex(c.pubshares, ps[..]), has_pubshares=c.has_pubshares, thresh_pk=ms_hex(c.thresh_pk, tp[..])};
    tweaks := ms_hex(c.tweaks, t[..]); xonly := fr_flags(c.xonly, f[..]); msg := ms_hex(c.msg, m[..]);
    x := fr_session_vartime(s, ms_hex(c.aggnonce, agg[..]), tweaks, xonly, msg);
    r := fr_partial_agg_vartime(ms_hex(c.psigs, p[..]), x, s.ids.count, sig[..]);
    ok := r.code == c.code && r.signer == c.signer;
    if r.code == FR_OK {
      ms_xbytes_vartime(x.q, q[..]);
      ok = ok && bt_hex_is(sig[..], c.expected) && schnorr_verify_vartime(msg, q[..], sig[..]);
    }
    if ok { passed += 1; }
  }
  return passed;
}
// ValidateThresholdInfo, cases lo to hi - 1.
frv_info :: (lo: int, hi: int) -> int {
  passed := 0;
  for i: lo..<hi {
    c := FR_INFO[i];
    tp: [33] u8; ps: [4300] u8; f: [130] bool;
    r := fr_validate_info_vartime(c.t, ms_hex(c.thresh_pk, tp[..]), ms_hex(c.pubshares, ps[..]), fr_flags(c.present, f[..]));
    if r.code == c.code && r.signer == c.signer { passed += 1; }
  }
  return passed;
}
```

## The example

The example is [bitcoin/examples/frost.coax](../examples/frost.coax).
A dealer splits a secret 2-of-3, and the threshold info check accepts
the shares and refuses them swapped. Lagrange interpolation recovers the
secret from each pair. Participants 0 and 2 sign, step by step; the
same nonces in another signer set give another b, and one signer alone
refuses. The threshold key becomes a Taproot output key and signs its
key path. A Pedersen DKG makes a second 2-of-3 with no dealer, and two
of its participants sign. Last come DeterministicSign and the count of
every vector file.

<!-- source: bitcoin/examples/frost.coax example -->
```jai
#load "../../coax/lib/bitcoin/frost.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

put_hex :: (b: [] u8) {
  text: [300] u8;
  r := hex_encode_checked(cast(string) b, text[..], false);
  print("%", cast(string) text[0..<r.written]);
}
line :: (label: string, b: [] u8) { print("  %: ", label); put_hex(b); print("\n"); }
scalar_line :: (label: string, s: ScScalar) { b: [32] u8; #assert sc_store(s, b[..]); line(label, b[..]); }
point_line :: (label: string, p: SgPoint) { b: [33] u8; ms_cbytes_vartime(p, b[..]); line(label, b[..]); }
yes :: (b: bool) -> string { return ifx b then "yes" else "no"; }
reason :: (r: FrResult) -> string { return FR_REASONS[r.code]; }

main :: () {
  // A trusted dealer splits a secret 2-of-3.
  print("A trusted dealer splits a secret 2-of-3:\n");
  secret: [32] u8; tp: [33] u8; sec: [96] u8; pub: [99] u8; dw: [1] FrDealWork;
  for j: 0..<32 { secret[j] = cast(u8) (j + 1); }
  #assert fr_dealer(secret[..], 3, 2, tp[..], sec[..], pub[..], dw[..]) == FR_OK;
  line("threshold key", tp[..]);
  for i: 0..<3 {
    print("  participant %: secret share ", i); put_hex(sec[32*i..<32*i+32]);
    print(", public share "); put_hex(pub[33*i..<33*i+33]); print("\n");
  }
  present := [3] bool.[true, true, true];
  print("  the shares lie on one line through the key: %\n", reason(fr_validate_info_vartime(2, tp[..], pub[..], present[..])));
  swapped: [99] u8;
  for j: 0..<99 { swapped[j] = pub[j]; }
  for j: 0..<33 { swapped[j] = pub[33 + j]; swapped[33 + j] = pub[j]; }
  print("  with shares 0 and 1 swapped: %\n", reason(fr_validate_info_vartime(2, tp[..], swapped[..], present[..])));

  // Lagrange: any two shares give the secret back.
  print("Lagrange interpolation at x = 0:\n");
  pairs := [6] int.[0, 1, 1, 2, 0, 2];
  for p: 0..<3 {
    ids := pairs[2*p..<2*p+2];
    print("  participants % and %: lambda ", ids[0], ids[1]);
    b: [32] u8; #assert sc_store(fr_lambda_vartime(ids, ids[0]), b[..]); put_hex(b[..]);
    print(", ");
    #assert sc_store(fr_lambda_vartime(ids, ids[1]), b[..]); put_hex(b[..]);
    two: [64] u8; out: [32] u8;
    for j: 0..<32 { two[j] = sec[32*ids[0]+j]; two[32+j] = sec[32*ids[1]+j]; }
    #assert fr_combine(two[..], ids, out[..]);
    print("; the secret: %\n", yes(bt_same(out[..], secret[..])));
  }

  // Participants 0 and 2 sign.
  print("Participants 0 and 2 sign one message:\n");
  msg: [32] u8;
  for j: 0..<32 { msg[j] = cast(u8) (0xb0 + j); }
  ids := [2] int.[0, 2];
  ps: [66] u8; nonces: [132] u8; agg: [66] u8; psigs: [64] u8; sig: [64] u8; q: [32] u8; nf: [1] bool;
  for i: 0..<2 { for j: 0..<33 { ps[33*i+j] = pub[33*ids[i]+j]; } }
  s := FrSigners.{n=3, t=2, ids=ids[..], pubshares=ps[..], has_pubshares=true, thresh_pk=tp[..]};
  session(sec[..], s, msg[0..<0], nf[0..<0], msg[..], 1, nonces[..], agg[..], psigs[..], sig[..]);
  for i: 0..<2 { print("  participant %'s nonce: ", ids[i]); put_hex(nonces[66*i..<66*i+66]); print("\n"); }
  line("aggnonce", agg[..]);
  x := fr_session_vartime(s, agg[..], msg[0..<0], nf[0..<0], msg[..]);
  scalar_line("b", x.b);
  point_line("R = R1 + b R2", x.r);
  scalar_line("e", x.e);
  print("  the threshold key has an % y, so g is %\n", ifx ms_odd_vartime(x.q) then "odd" else "even", ifx ms_odd_vartime(x.q) then "n - 1" else "1");
  for i: 0..<2 {
    print("  participant %: lambda ", ids[i]);
    b: [32] u8; #assert sc_store(fr_lambda_vartime(ids[..], ids[i]), b[..]); put_hex(b[..]);
    print(", partial signature "); put_hex(psigs[32*i..<32*i+32]); print("\n");
    print("    verifies: %\n", reason(fr_partial_verify_vartime(psigs[32*i..<32*i+32], nonces[..], s, msg[0..<0], nf[0..<0], msg[..], i)));
  }
  psigs[31] ^= 1;
  print("  participant 0's with its last bit changed: %\n", reason(fr_partial_verify_vartime(psigs[0..<32], nonces[..], s, msg[0..<0], nf[0..<0], msg[..], 0)));
  psigs[31] ^= 1;
  line("signature", sig[..]);
  for j: 0..<32 { q[j] = tp[1 + j]; }
  print("  a valid BIP-340 signature under the threshold key: %\n", yes(schnorr_verify_vartime(msg[..], q[..], sig[..])));
  // The same nonces in another signer set give another b.
  other := [2] int.[0, 1];
  ps2: [66] u8;
  for i: 0..<2 { for j: 0..<33 { ps2[33*i+j] = pub[33*other[i]+j]; } }
  s2 := FrSigners.{n=3, t=2, ids=other[..], pubshares=ps2[..], has_pubshares=true, thresh_pk=tp[..]};
  scalar_line("b for the same nonces with participants 0 and 1", fr_session_vartime(s2, agg[..], msg[0..<0], nf[0..<0], msg[..]).b);
  one := FrSigners.{n=3, t=2, ids=ids[0..<1], pubshares=ps[0..<33], has_pubshares=true, thresh_pk=tp[..]};
  print("  participant 0 alone: %\n", FR_REASONS[fr_session_vartime(one, agg[..], msg[0..<0], nf[0..<0], msg[..]).code]);

  // The threshold key as a Taproot internal key.
  print("The threshold key as a Taproot internal key:\n");
  tweak: [32] u8; output: [32] u8; addr: [100] u8; xf := [1] bool.[true];
  #assert fr_taproot_tweak_vartime(tp[..], msg[0..<0], tweak[..]) == FR_OK;
  line("TapTweak(Q)", tweak[..]);
  k := fr_tweaks_vartime(tp[..], tweak[..], xf[..]);
  ms_xbytes_vartime(k.q, output[..]);
  line("output key", output[..]);
  print("  gacc is %\n", ifx sc_equal(k.gacc, sc_small(1)) then "1" else "n - 1");
  out2: [32] u8;
  #assert taproot_output_key_vartime(q[..], msg[0..<0], out2[..]) >= 0;
  print("  chapter 41's output key of the same internal key: %\n", yes(bt_same(output[..], out2[..])));
  n := segwit_encode_vartime(SW_MAINNET, 1, output[..], addr[..]);
  print("  address: %\n", cast(string) addr[0..<n]);
  session(sec[..], s, tweak[..], xf[..], msg[..], 11, nonces[..], agg[..], psigs[..], sig[..]);
  line("key-path signature", sig[..]);
  print("  verifies under the output key: %\n", yes(taproot_key_path_verify_vartime(output[..], sig[..], msg[..])));

  // A Pedersen DKG with no dealer.
  print("A 2-of-3 Pedersen DKG:\n");
  cx: [16] u8; ctx := ms_hex("636861707465722031303220444b47", cx[..]);
  seeds: [96] u8; commits: [198] u8; pops: [192] u8; shares: [288] u8; kw: [1] FrDkgWork;
  for i: 0..<96 { seeds[i] = cast(u8) ((3 * i + 5) & 255); }
  for i: 0..<3 {
    #assert fr_dkg_round1(seeds[32*i..<32*i+32], i, 3, 2, ctx, commits[66*i..<66*i+66], pops[64*i..<64*i+64], shares[96*i..<96*i+96], kw[..]) == FR_OK;
    print("  participant % commits ", i); put_hex(commits[66*i..<66*i+33]); print(" and "); put_hex(commits[66*i+33..<66*i+66]); print("\n");
    print("    proof of possession "); put_hex(pops[64*i..<64*i+64]); print("\n");
  }
  checks := 0;
  for i: 0..<3 { for j: 0..<3 {
    if fr_dkg_check_vartime(commits[66*i..<66*i+66], pops[64*i..<64*i+64], ctx, i, shares[96*i+32*j..<96*i+32*j+32], j) == FR_OK { checks += 1; }
  } }
  print("  checks that pass, of 9: %\n", checks);
  print("  participant 0's proof checked as participant 1's: %\n", FR_REASONS[fr_dkg_check_vartime(commits[0..<66], pops[0..<64], ctx, 1, shares[0..<32], 0)]);
  dsec: [96] u8; dtp: [99] u8; dpub: [297] u8; got: [96] u8;
  for j: 0..<3 {
    for i: 0..<3 { for b: 0..<32 { got[32*i+b] = shares[96*i+32*j+b]; } }
    #assert fr_dkg_finish(got[..], commits[..], 3, 2, dsec[32*j..<32*j+32], dtp[33*j..<33*j+33], dpub[99*j..<99*j+99]) == FR_OK;
  }
  line("threshold key", dtp[0..<33]);
  for i: 0..<3 { print("  participant %'s public share: ", i); put_hex(dpub[33*i..<33*i+33]); print("\n"); }
  print("  every participant computed the same key and shares: %\n", yes(bt_same(dtp[0..<33], dtp[33..<66]) && bt_same(dtp[0..<33], dtp[66..<99]) && bt_same(dpub[0..<99], dpub[99..<198]) && bt_same(dpub[0..<99], dpub[198..<297])));
  print("  threshold info: %\n", reason(fr_validate_info_vartime(2, dtp[0..<33], dpub[0..<99], present[..])));
  dids := [2] int.[1, 2]; dps: [66] u8;
  for i: 0..<2 { for j: 0..<33 { dps[33*i+j] = dpub[33*dids[i]+j]; } }
  ds := FrSigners.{n=3, t=2, ids=dids[..], pubshares=dps[..], has_pubshares=true, thresh_pk=dtp[0..<33]};
  session(dsec[..], ds, msg[0..<0], nf[0..<0], msg[..], 21, nonces[..], agg[..], psigs[..], sig[..]);
  line("participants 1 and 2 sign", sig[..]);
  for j: 0..<32 { q[j] = dtp[1 + j]; }
  print("  a valid BIP-340 signature under the DKG's key: %\n", yes(schnorr_verify_vartime(msg[..], q[..], sig[..])));

  // DeterministicSign.
  print("DeterministicSign (det_sign_vectors case 1):\n");
  w: [1] FrDetWork; c := FR_DET_SIGN[0];
  vi: [16] int; vps: [528] u8; vtp: [33] u8; key: [32] u8; o: [66] u8; m: [64] u8; ax: [32] u8; dn: [66] u8; dpsig: [32] u8; tf: [1] bool;
  vs := FrSigners.{n=c.n, t=c.t, ids=fr_ids(c.ids, vi[..]), pubshares=ms_hex(c.pubshares, vps[..]), has_pubshares=c.has_pubshares, thresh_pk=ms_hex(c.thresh_pk, vtp[..])};
  r := fr_det_sign(ms_hex(c.secshare, key[..]), c.signer_id, ms_hex(c.aggother, o[..]), c.has_aggother, vs, msg[0..<0], tf[0..<0], ms_hex(c.msg, m[..]),
    ms_hex(c.aux, ax[..]), c.has_aux, dn[..], dpsig[..], w[..]);
  line("pubnonce", dn[..]); line("psig", dpsig[..]);
  print("  %, as published: %\n", reason(r), yes(bt_hex_is(dn[..], c.pubnonce) && bt_hex_is(dpsig[..], c.psig)));

  // Every published vector.
  print("BIP-445's vector files, cases giving their stated result:\n");
  print("  nonce_gen_vectors: % of %\n", frv_nonce_gen(), FR_NONCE_GEN.count);
  print("  nonce_agg_vectors: % of %\n", frv_nonce_agg(), FR_NONCE_AGG.count);
  print("  sign_verify_vectors and tweak_vectors, Sign: % of %\n", frv_sign(0, FR_SIGN.count), FR_SIGN.count);
  print("  sign_verify_vectors and tweak_vectors, PartialSigVerify: % of %\n", frv_verify(0, FR_VERIFY.count), FR_VERIFY.count);
  print("  det_sign_vectors: % of %\n", frv_det_sign(0, FR_DET_SIGN.count), FR_DET_SIGN.count);
  print("  sig_agg_vectors: % of %\n", frv_sig_agg(0, FR_SIG_AGG.count), FR_SIG_AGG.count);
  print("  threshold_info_vectors: % of %\n", frv_info(0, FR_INFO.count), FR_INFO.count);
}
// One session: each signer makes a nonce from rand bytes seed + its id,
// signs, and the partial signatures are added into sig. secshares holds
// every participant's share, indexed by id.
session :: (secshares: [] u8, s: FrSigners, tweaks: [] u8, xonly: [] bool, msg: [] u8, seed: int, pub: [] u8, agg: [] u8, psigs: [] u8, sig: [] u8) {
  nw: [1] MsNonceWork; sw: [1] MsSignWork; sec: [128] u8; rand: [32] u8; q: [32] u8;
  u := s.ids.count;
  ms_xbytes_vartime(fr_tweaks_vartime(s.thresh_pk, tweaks, xonly).q, q[..]);
  for i: 0..<u {
    id := s.ids[i];
    for j: 0..<32 { rand[j] = cast(u8) (seed + id); }
    #assert fr_nonce_gen(rand[..], secshares[32*id..<32*id+32], id, s.pubshares[33*i..<33*i+33], q[..], msg, true, rand[0..<0], sec[64*i..<64*i+64], pub[66*i..<66*i+66], nw[..]) == FR_OK;
  }
  #assert fr_nonce_agg_vartime(pub[0..<66*u], agg).code == FR_OK;
  for i: 0..<u {
    id := s.ids[i];
    #assert fr_sign(sec[64*i..<64*i+64], secshares[32*id..<32*id+32], id, s, agg, tweaks, xonly, msg, psigs[32*i..<32*i+32], sw[..]).code == FR_OK;
  }
  #assert fr_partial_agg_vartime(psigs[0..<32*u], fr_session_vartime(s, agg, tweaks, xonly, msg), u, sig).code == FR_OK;
}
```

```shell
coax/bin/coax run bitcoin/examples/frost.coax --run-steps 250000000
```

It prints:

```text
A trusted dealer splits a secret 2-of-3:
  threshold key: 0284bf7562262bbd6940085748f3be6afa52ae317155181ece31b66351ccffa4b0
  participant 0: secret share dbf30a9a6710a74a6515c9c8053400c33cc216d10c75a7c6ed934ee975a90b87, public share 03808496e2ff7af458eb3524369817a9f366384b4c4f3b19bf1e229e2e34632fc3
  participant 1: secret share b6e41230c91b478cc1218883fd59f277adc33da7548c983a023a2429fdfdb6ad, public share 02fac06ffce838c5d9e3a6de5e10d04e379d281846935219fb0544f1eb6208c227
  participant 2: secret share 91d519c72b25e7cf1d2d473ff57fe42c1ec4647d9ca388ad16e0f96a865261d3, public share 03d184f36db97d4c7e5560792e46670ab3eb7f6a4c6f51a9bf1d94b5c346acbeb8
  the shares lie on one line through the key: ok
  with shares 0 and 1 swapped: The provided key material is incorrect: the public shares do not lie on a single polynomial.
Lagrange interpolation at x = 0:
  participants 0 and 1: lambda 0000000000000000000000000000000000000000000000000000000000000002, fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364140; the secret: yes
  participants 1 and 2: lambda 0000000000000000000000000000000000000000000000000000000000000003, fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd036413f; the secret: yes
  participants 0 and 2: lambda 7fffffffffffffffffffffffffffffff5d576e7357a4501ddfe92f46681b20a2, 7fffffffffffffffffffffffffffffff5d576e7357a4501ddfe92f46681b20a0; the secret: yes
Participants 0 and 2 sign one message:
  participant 0's nonce: 03c3303740c0a0d55df11832e8ed0ab43d89ff8823c205eb7b9a20159d30f9a743030decdf21af28888ae57dc95912f399cbd1a25e0537bc0098d723a8cf8a6251d8
  participant 2's nonce: 03a61cbc8d572a1cc81efbf4423c444058c610993181911c3ebf2d24c95095348903718967ad67f05f9b64b061a3725488b489e44910482ed7c39830b52b0caca9b0
  aggnonce: 02c730f65735aae095e45f064e714900d66b4f666593bf62d915b190c94cd3434602697e4b861e3703757e849d902d9c7a5dcfb269d32d273b74f9d108ee9e26c8e0
  b: fa6319889a60edccce1813d9ec12c105c0c8f489cded478d013ae96debd44761
  R = R1 + b R2: 03e74577325302e699b1837f4d151ef35ae9726628f200c79912af2832d8434dc6
  e: ef9b62fab27a352dbe89bceeaef8921a56eed104f26329c929b8345e9c3ca1b8
  the threshold key has an even y, so g is 1
  participant 0: lambda 7fffffffffffffffffffffffffffffff5d576e7357a4501ddfe92f46681b20a2, partial signature 62ab8fd385a1a6fc6a6fb5a38116989a110798baaafa47d2fac4fda60709fa8b
    verifies: ok
  participant 2: lambda 7fffffffffffffffffffffffffffffff5d576e7357a4501ddfe92f46681b20a0, partial signature d797674c982f6e33a137b310481241655381258ce374ed2f16568cf50d660bb6
    verifies: ok
  participant 0's with its last bit changed: invalid
  signature: e74577325302e699b1837f4d151ef35ae9726628f200c79912af2832d8434dc63a42f7201dd115300ba768b3c928da00a9d9e160df2694c651492c0e4439c500
  a valid BIP-340 signature under the threshold key: yes
  b for the same nonces with participants 0 and 1: 385fabb2ab7eccaffe588c3d3b2b213e784eefd0348a7f4a381d2ccbd91c1c5d
  participant 0 alone: The number of signers must satisfy t <= u <= n.
The threshold key as a Taproot internal key:
  TapTweak(Q): d26bbcab3f1f1dffd9e2a652ad0f1d529255f460de5ed5f8b27ace9e679b34e1
  output key: 4a95770effc923dbe8125e951a8e0f7a667ae1512eac4a933d8be96fbd591127
  gacc is 1
  chapter 41's output key of the same internal key: yes
  address: bc1pf22hwrhley3ah6qjt6234rs00fn84c2396ky4yea305kl02ezyns3k932r
  key-path signature: b45127e2a1e803b21e19912cda3718721d5c5fe7902ad60e84cd76c6260bdecff019486be131e887fb659d016c9829070b1a8cc44f6a655c520ef48a366a2f0d
  verifies under the output key: yes
A 2-of-3 Pedersen DKG:
  participant 0 commits 028040a13e1ee62d5a76573b788f7515e89560c422d2696894f2311d2f92989f74 and 03031e7a0990e15f06af05f545879f4feb7887bbfe8d190b44b34392de642f6cdb
    proof of possession 12602b4794317686c4eeed43ce30bcd4756eafc782d269d4e868d3f9760f047ffd1be4f53423ae8e954921e8141431dfcfeff7d4e9d6bd3f221ae8a28698ae43
  participant 1 commits 0204f8084da2e58430fe4eaf7b277a5da5a0aa9de9c43f5d8173fe30037dfb43c9 and 0302bd8e6564e60adcaa3ee7de5ee766bee74f4bc1a20fe1c86a570fffcb006d09
    proof of possession 56ef7906925d60e769ef2cbb74256ab2f5100f412f41d958ea9a20ec5cd7bcc04200e20bd4bd891ed29c0ff66e824faa403f5d39fe19502e2f856b130cc833e9
  participant 2 commits 0269e7db3e13a0613edbe69e689546a379be59496c4d4e7668326b6ed4514e2dfc and 03f00829ed7bec00c342aa809ecd89279812f4f003f90d208e3e50a3ac37cb1764
    proof of possession d55be8faed19b59690761568a708c9dba041a5c8ae11544ee3400039c47daf1bffb9592996c3d6bcaa624a75a768c9bc8945f1f80ae16fe0a85686c8feabdccf
  checks that pass, of 9: 9
  participant 0's proof checked as participant 1's: proof of possession
  threshold key: 02c9ba1428eb243e467e0e169ca6506bb16800cbd8e53e34153a6cf5fdde9a34b8
  participant 0's public share: 03a97d88721295f9446e0ca6c53fc6861c01f4db8103a0b22d2342434992536cca
  participant 1's public share: 031d1acf02a182cc4d29623e2d154edbee8634e950d8d089cb930e14306a7059f2
  participant 2's public share: 0246c3da89d235317ebdb6fef11f1d2dae07ae15bba7a394bb1b35e18b03b7a424
  every participant computed the same key and shares: yes
  threshold info: ok
  participants 1 and 2 sign: 11e4e0f62d920a12fa0fc270ac8c65600eca721843964a46be1fc60fa31c3fe7e733634aac6e8b069cea5264c0b5c5e6e8a636b01f404b7cb80f6a3cc6721e56
  a valid BIP-340 signature under the DKG's key: yes
DeterministicSign (det_sign_vectors case 1):
  pubnonce: 03ccc678cbd9cfcd74288c66e6c74a344919d1fc50277f2fa468a89c7b67edd1630377649f7c793c4afbaecdddfb511825665d7c596050b4f6808ad4cee50fd0c2bf
  psig: 88f89e281154168d33bfe509245da049cf083df0255e21ec04dd6bfa3066307c
  ok, as published: yes
BIP-445's vector files, cases giving their stated result:
  nonce_gen_vectors: 6 of 6
  nonce_agg_vectors: 5 of 5
  sign_verify_vectors and tweak_vectors, Sign: 124 of 124
  sign_verify_vectors and tweak_vectors, PartialSigVerify: 68 of 68
  det_sign_vectors: 88 of 88
  sig_agg_vectors: 28 of 28
  threshold_info_vectors: 41 of 41
```

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/frost.out](../fixtures/frost.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/frost.coax](../examples/checks/frost.coax),
which loads the example:

- every pair of the dealer's three participants signs under the one
  threshold key, and each signer alone refuses;
- a published partial signature with any one of eight bits changed fails
  PartialSigVerify;
- each of the first four published signing cases signs once, and the
  same secnonce then refuses for another message.

<!-- source: bitcoin/examples/checks/frost.coax check -->
```jai
// Threshold: every pair of the dealer's three participants signs under
// the one threshold key, and each of them alone refuses.
#forall "any two of three sign; one alone refuses" p: 0..<3 #steps 8000000000 {
  secret: [32] u8; tp: [33] u8; sec: [96] u8; pub: [99] u8; dw: [1] FrDealWork;
  for j: 0..<32 { secret[j] = cast(u8) (j + 1); }
  #assert fr_dealer(secret[..], 3, 2, tp[..], sec[..], pub[..], dw[..]) == FR_OK;
  ids := [2] int.[p, (p + 1) % 3];
  ps: [66] u8; nonces: [132] u8; agg: [66] u8; psigs: [64] u8; sig: [64] u8; msg: [32] u8; q: [32] u8; nf: [1] bool;
  for i: 0..<2 { for j: 0..<33 { ps[33*i+j] = pub[33*ids[i]+j]; } }
  s := FrSigners.{n=3, t=2, ids=ids[..], pubshares=ps[..], has_pubshares=true, thresh_pk=tp[..]};
  session(sec[..], s, msg[0..<0], nf[0..<0], msg[..], 30 + p, nonces[..], agg[..], psigs[..], sig[..]);
  for j: 0..<32 { q[j] = tp[1 + j]; }
  #assert schnorr_verify_vartime(msg[..], q[..], sig[..]);
  s.ids = ids[0..<1]; s.pubshares = ps[0..<33];
  #assert fr_session_vartime(s, agg[..], msg[0..<0], nf[0..<0], msg[..]).code == FR_SIGNER_COUNT;
}
// Any one changed bit of a published partial signature fails
// PartialSigVerify for its signer.
#forall "a changed partial signature fails" bit: 0..<8 #steps 8000000000 {
  c := FR_VERIFY[0];
  ids: [16] int; ps: [528] u8; tp: [33] u8; n: [1056] u8; f: [1] bool; m: [64] u8; sig: [32] u8;
  s := FrSigners.{n=c.n, t=c.t, ids=fr_ids(c.ids, ids[..]), pubshares=ms_hex(c.pubshares, ps[..]), has_pubshares=c.has_pubshares, thresh_pk=ms_hex(c.thresh_pk, tp[..])};
  psig := ms_hex(c.psig, sig[..]);
  psig[(bit * 13) % 32] ^= cast(u8) (1 << bit);
  #assert fr_partial_verify_vartime(psig, ms_hex(c.pubnonces, n[..]), s, m[0..<0], f[0..<0], ms_hex(c.msg, m[..]), c.index).code == FR_INVALID;
}
// A secnonce signs once: after each of the first published signing
// cases, the same secnonce refuses, whatever the message.
#forall "a secnonce signs once" i: 0..<4 #steps 8000000000 {
  c := FR_SIGN[i]; w: [1] MsSignWork;
  ids: [16] int; ps: [528] u8; tp: [33] u8; sec: [64] u8; sk: [32] u8; agg: [66] u8; f: [1] bool; m: [64] u8; psig: [32] u8;
  s := FrSigners.{n=c.n, t=c.t, ids=fr_ids(c.ids, ids[..]), pubshares=ms_hex(c.pubshares, ps[..]), has_pubshares=c.has_pubshares, thresh_pk=ms_hex(c.thresh_pk, tp[..])};
  nonce := ms_hex(c.secnonce, sec[..]); key := ms_hex(c.secshare, sk[..]); aggnonce := ms_hex(c.aggnonce, agg[..]);
  #assert fr_sign(nonce, key, c.signer_id, s, aggnonce, m[0..<0], f[0..<0], ms_hex(c.msg, m[..]), psig[..], w[..]).code == FR_OK;
  m[0] ^= 1;
  #assert fr_sign(nonce, key, c.signer_id, s, aggnonce, m[0..<0], f[0..<0], m[0..<32], psig[..], w[..]).code == FR_SECNONCE_1;
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/frost.coax --run-steps 250000000
```

`coax test` runs 517 checks, including the loaded files' own checks, in
about 850 CPU seconds on 2026-11-03.

### The library's own checks

`frost.coax` is an `#authority` file. Its checks run every case of the
seven vector files, the capacity refusals of every procedure, a 2-of-3
from the trusted dealer whose pairs each recover the secret and whose
Taproot key-path signature verifies under chapter 41's output key, a
signer set of one that refuses, and a DKG in which every receiver checks
every sender, a proof for another index and a changed share refuse, all
three participants agree on the key and the public shares, and two of
them sign.

```shell
coax/bin/coax test coax/lib/bitcoin/frost.coax --run-steps 250000000
```

`coax test` runs 510 checks, including the loaded files' own checks: the
example's run above is the same set plus its three, and takes about 850 CPU
seconds. Each vector check runs four cases, so that it stays inside the
compile-time work limit. Mutation testing: not run (owner speed order).

### BIP-445's own Python

[frost.py](../../coax/tests/bitcoin/frost.py) checks the hash of every
pinned file, then imports the BIP's reference implementation and
secp256k1lab from them and runs its tests over the pinned vector files,
and six random sessions. It checks that `frost.coax`'s reasons are the
reference's error messages and contribution names, and that
`frost_vectors.coax` is what
[make_frost_vectors.py](../../coax/tests/bitcoin/make_frost_vectors.py)
writes. Then it writes the transcript the example must print with the
reference's functions: the dealer is the BIP's `trusted_dealer.py`, the
sessions are its `nonce_gen_internal`, `sign` and `partial_sig_agg`, and
the DKG is written out again over secp256k1lab with its BIP-340 signer,
which also derives every public share from the commitments alone.

```shell
python3 -I coax/tests/bitcoin/frost.py --coax coax/bin/coax
```

On 2026-11-03 it passes: the reference passes every pinned vector file
and six random sessions, its transcript and the fixture agree on all 65
lines, and the example prints them in debug and release builds.

## What this chapter leaves out

- RFC 9591's FROST(secp256k1, SHA-256) ciphersuite and its vectors: its
  signatures are not BIP-340 signatures.
- ChillDKG's encrypted shares, equality check and recovery data, and
  any DKG standard: none is final.
- Share refresh, repair and changing t or n without moving the coins.
- The PSBT fields a FROST signer would need: no BIP defines them yet.
- ROAST, which wraps FROST to finish despite unresponsive signers.

## Summary

- FROST makes one BIP-340 signature under one x-only key from any t of
  n participants; on the chain it is a single-key Taproot spend.
- A share is a value of a polynomial whose value at 0 is the secret;
  Lagrange interpolation recovers it from any t shares, and the same
  weights turn public shares into the threshold key.
- A trusted dealer knows the secret; a Pedersen DKG with commitments and
  proofs of possession makes the same kind of shares with no dealer.
- Signing takes two rounds; the binding factor b hashes the signer set,
  the nonces, the key and the message.
- Each partial signature carries the signer's interpolating value and
  can be checked on its own, so a cheater is identified.
- Plain and x-only tweaks make the threshold key a BIP-32 parent or a
  Taproot output key.

## Dependencies

- [101. MuSig2](ch101-musig2.md)
- [41. The key tweak and the key path](ch041-the-key-tweak-and-the-key-path.md)

## Sources

- BIP-445, "FROST Signing Protocol for BIP340 Signatures" (draft,
  version 0.12.1), with its reference implementation, secp256k1lab and
  its seven vector files, pinned at `siv2r/bip-frost-signing` revision
  `ba299bad` in
  [coax/tests/vectors/upstream/bip445/](../../coax/tests/vectors/upstream/bip445/bip-0445.md).
- [RFC 9591](https://www.rfc-editor.org/rfc/rfc9591), "The Flexible
  Round-Optimized Schnorr Threshold (FROST) Protocol for Two-Round
  Schnorr Signatures".
- Chelsea Komlo and Ian Goldberg, "FROST: Flexible Round-Optimized
  Schnorr Threshold Signatures", SAC 2020,
  [eprint 2020/852](https://eprint.iacr.org/2020/852).
- Elizabeth Crites, Chelsea Komlo and Mary Maller, "Fully Adaptive
  Schnorr Threshold Signatures", CRYPTO 2023,
  [eprint 2023/445](https://eprint.iacr.org/2023/445) (FROST3 and
  SimplPedPop).
- ChillDKG, the draft DKG for BIP-445 keys,
  [BlockstreamResearch/bip-frost-dkg](https://github.com/BlockstreamResearch/bip-frost-dkg).
- Adi Shamir, "How to Share a Secret", 1979; Paul Feldman, "A Practical
  Scheme for Non-interactive Verifiable Secret Sharing", 1987.
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
