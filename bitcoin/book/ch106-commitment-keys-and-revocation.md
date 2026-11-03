# 106. Commitment keys and revocation

Every commitment transaction in a Lightning channel uses fresh keys. A
watchtower given an old commitment's details cannot link it to the next
one, and an old commitment can be punished once it is revoked. BOLT 3
makes all of these keys from a few long-lived basepoints and one
per-commitment point.

This chapter builds that derivation: the per-commitment point from
chapter 107's secret, the keys made by adding a tweak to a basepoint,
the `remotepubkey` of `option_static_remotekey`, and the revocation key,
which neither peer can sign for until the other reveals its secret.

## What this chapter teaches

- The per-commitment point: the per-commitment secret times G.
- The tweak: SHA-256 of two compressed points, used as a scalar.
- `localpubkey`, the HTLC keys and the delayed keys: a basepoint plus the
  tweak times G, and the matching private keys.
- `remotepubkey` under `option_static_remotekey`: the remote node's
  payment basepoint, unchanged.
- The revocation key, R·SHA256(R || P) + P·SHA256(P || R), and why it
  needs both peers' secrets.
- Which inputs are public and which are secret.

## Where Lightning uses these keys

| Where | What it uses | Chapter |
| --- | --- | --- |
| Commitment outputs | `to_local` with the delayed and revocation keys, `to_remote` with `remotepubkey` | 108 |
| HTLC outputs | the local and remote HTLC keys and the revocation key | 108 |
| Revocation | the per-commitment secret in `revoke_and_ack` | 109 |
| Penalties | the revocation private key spends a revoked commitment | 109 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [keys.coax](../../coax/lib/lightning/keys.coax) | `lnk_point(secret)` | a secret times G, in constant time |
| | `lnk_tweak(a, b)` | SHA-256 of two points, as a scalar |
| | `lnk_pubkey_vartime(basepoint, point)` | `localpubkey` and the HTLC and delayed keys |
| | `lnk_privkey(base_secret, point, out)` | their private keys |
| | `lnk_remotepubkey_vartime(payment_basepoint)` | `remotepubkey` |
| | `lnk_revocation_pubkey_vartime(revocation_basepoint, point)` | `revocationpubkey` |
| | `lnk_revocation_privkey(revocation_secret, commitment_secret, out)` | `revocationprivkey` |
| [secp_mult_const.coax](../../coax/lib/crypto/secp_mult_const.coax) | `smc_multiply` | constant-time scalar multiplication (chapter 16) |
| [secp_mult.coax](../../coax/lib/crypto/secp_mult.coax) | `ecmult_vartime` | public-input multiplication (chapter 16) |
| [shachain.coax](../../coax/lib/lightning/shachain.coax) | `shachain_generate`, `shachain_lookup` | per-commitment secrets (chapter 107) |

Basepoints and per-commitment points are public: peers send them in
`open_channel`, `accept_channel` and `revoke_and_ack`. The procedures that
work only on them run in variable time and end in `_vartime` (chapter 3).
Basepoint secrets and per-commitment secrets are secret. Their
procedures multiply with chapter 16's constant-time ladder, add and
multiply scalars in constant time, and clear the scalars they leave on
the stack.

The library's codes and sizes:

<!-- source: coax/lib/lightning/keys.coax codes -->
```jai
LNK_OK :: 0;
LNK_BAD_POINT :: 1;   // a basepoint or per-commitment point that is not a compressed point
LNK_BAD_SECRET :: 2;  // a secret of zero or at least n
LNK_BAD_TWEAK :: 3;   // a SHA-256 tweak of at least n: never seen, but refused
LNK_INFINITY :: 4;    // a derived key at infinity or zero: never seen, but refused

LNK_POINT :: 33;      // compressed points
LNK_SECRET :: 32;
```

## The tweak

Every derivation hashes two compressed points together and uses the
32-byte result as a scalar mod n. A hash of n or more is refused rather
than reduced. That happens with probability about 2^-128, so it never
happens in practice, but the code still says what it does.

<!-- source: coax/lib/lightning/keys.coax tweak -->
```jai
// SHA256(a || b) for two compressed points, as a scalar.
LnkTweak :: struct { code: int; value: ScScalar; }
lnk_tweak :: (a: [] u8, b: [] u8) -> LnkTweak {
  joined: [66] u8; hash: [32] u8;
  for i: 0..<33 { joined[i] = a[i]; joined[33 + i] = b[i]; }
  #assert sha256_hash_checked(joined[..], hash[..], false) == SHA_OK;
  t := sc_load(hash[..]);
  return LnkTweak.{code = ifx t.ok then LNK_OK else LNK_BAD_TWEAK, value = t.value};
}
```

## The per-commitment point

Each commitment has a per-commitment secret, the shachain secret of
chapter 107 for that commitment's index. Its point is the secret times G.
A node sends its next per-commitment point to its peer ahead of time, so
the peer can build that commitment's keys, and keeps the secret until
the commitment is revoked.

<!-- source: coax/lib/lightning/keys.coax point -->
```jai
// per_commitment_point = per_commitment_secret * G. The secret is the
// shachain secret of chapter 107; the point goes to the peer.
lnk_point :: (secret: [] u8) -> LnkKey {
  if secret.count != LNK_SECRET { return LnkKey.{code = LNK_BAD_SECRET}; }
  k := sc_load(secret);
  if !k.ok || sc_is_zero(k.value) { k = ScLoad.{}; return LnkKey.{code = LNK_BAD_SECRET}; }
  r := lnk_encode(smc_multiply(k.value, sg_generator()));
  k = ScLoad.{};
  return r;
}
```

## Keys from a basepoint

Each node has a payment basepoint, a delayed payment basepoint and an
HTLC basepoint, sent once when the channel opens. A commitment's key for
one of them is:

    pubkey = basepoint + SHA256(per_commitment_point || basepoint) * G

The owner of the basepoint knows its secret, so it can make the private
key: the basepoint secret plus the same tweak. Anyone who knows the two
points can make the public key, but without the basepoint secret it does
not help them sign.

Under `option_static_remotekey`, the `to_remote` output pays the remote
node's payment basepoint with no tweak. A node that loses its channel
state can still find and spend that output without knowing any
per-commitment point.

<!-- source: coax/lib/lightning/keys.coax derive -->
```jai
// pubkey = basepoint + SHA256(per_commitment_point || basepoint) * G.
// localpubkey, local_htlcpubkey, remote_htlcpubkey, local_delayedpubkey
// and remote_delayedpubkey all use this, each from its own basepoint.
lnk_pubkey_vartime :: (basepoint: [] u8, point: [] u8) -> LnkKey {
  if basepoint.count != LNK_POINT || point.count != LNK_POINT { return LnkKey.{code = LNK_BAD_POINT}; }
  b := se_decode(basepoint);
  if !b.ok || !se_decode(point).ok { return LnkKey.{code = LNK_BAD_POINT}; }
  t := lnk_tweak(point, basepoint);
  if t.code != LNK_OK { return LnkKey.{code = t.code}; }
  one: [32] u8; one[31] = 1;
  return lnk_encode(ecmult_vartime(t.value, b.value, sc_load(one[..]).value));
}
// privkey = basepoint_secret + SHA256(per_commitment_point || basepoint),
// for the keys whose basepoint secret this node holds.
lnk_privkey :: (base_secret: [] u8, point: [] u8, out: [] u8) -> int {
  base := lnk_point(base_secret);
  if base.code != LNK_OK { return base.code; }
  if point.count != LNK_POINT || !se_decode(point).ok { return LNK_BAD_POINT; }
  t := lnk_tweak(point, base.bytes[..]);
  if t.code != LNK_OK { return t.code; }
  k := sc_load(base_secret);
  sum := sc_add(k.value, t.value);
  code := ifx sc_is_zero(sum) then LNK_INFINITY else LNK_OK;
  stored := sc_store(sum, out);
  k = ScLoad.{}; sum = ScScalar.{};
  return code;
}
// With option_static_remotekey, remotepubkey is the remote node's
// payment_basepoint itself: no per-commitment point, so a node that lost
// its state can still spend its to_remote output.
lnk_remotepubkey_vartime :: (payment_basepoint: [] u8) -> LnkKey {
  if payment_basepoint.count != LNK_POINT || !se_decode(payment_basepoint).ok { return LnkKey.{code = LNK_BAD_POINT}; }
  r: LnkKey; for i: 0..<33 { r.bytes[i] = payment_basepoint[i]; }
  return r;
}
```

## The revocation key

When Alice builds a commitment for Bob, she makes its revocation key from
her revocation basepoint R and Bob's per-commitment point P:

    revocationpubkey = R * SHA256(R || P) + P * SHA256(P || R)

Bob's `to_local` output can be spent by Bob after a delay, or at once by
whoever holds this key's private key:

    revocationprivkey = r * SHA256(R || P) + p * SHA256(P || R)

Alice knows r but not p, and Bob knows p but not r, so while the
commitment is current, neither can sign. When Bob moves to a new
commitment, he revokes this one by sending p in `revoke_and_ack`. From
then on Alice holds both secrets. If Bob broadcasts the revoked
commitment, Alice takes his `to_local` output before his delay ends.

The hashes stop a cancelling choice of keys. If the key were just R + P,
Bob could pick P = Q - R for a Q he controls, and the sum would be Q.
Multiplying each point by a hash of both makes the result depend on the
pair in a way neither side can steer.

<!-- source: coax/lib/lightning/keys.coax revocation -->
```jai
// revocationpubkey = R * SHA256(R || P) + P * SHA256(P || R), with R this
// node's revocation_basepoint and P the peer's per_commitment_point.
// Neither side can know its secret alone.
lnk_revocation_pubkey_vartime :: (revocation_basepoint: [] u8, point: [] u8) -> LnkKey {
  if revocation_basepoint.count != LNK_POINT || point.count != LNK_POINT { return LnkKey.{code = LNK_BAD_POINT}; }
  r := se_decode(revocation_basepoint); p := se_decode(point);
  if !r.ok || !p.ok { return LnkKey.{code = LNK_BAD_POINT}; }
  tr := lnk_tweak(revocation_basepoint, point); tp := lnk_tweak(point, revocation_basepoint);
  if tr.code != LNK_OK || tp.code != LNK_OK { return LnkKey.{code = LNK_BAD_TWEAK}; }
  zero: ScScalar;
  a := ecmult_vartime(zero, r.value, tr.value); b := ecmult_vartime(zero, p.value, tp.value);
  if !a.ok || !b.ok { return LnkKey.{code = LNK_INFINITY}; }
  return lnk_encode(sg_add(a.value, b.value));
}
// revocationprivkey = r * SHA256(R || P) + p * SHA256(P || R). Once the
// peer reveals p to revoke its commitment, this node knows both secrets.
lnk_revocation_privkey :: (revocation_secret: [] u8, commitment_secret: [] u8, out: [] u8) -> int {
  big_r := lnk_point(revocation_secret); big_p := lnk_point(commitment_secret);
  if big_r.code != LNK_OK || big_p.code != LNK_OK { return LNK_BAD_SECRET; }
  tr := lnk_tweak(big_r.bytes[..], big_p.bytes[..]); tp := lnk_tweak(big_p.bytes[..], big_r.bytes[..]);
  if tr.code != LNK_OK || tp.code != LNK_OK { return LNK_BAD_TWEAK; }
  r := sc_load(revocation_secret); p := sc_load(commitment_secret);
  sum := sc_add(sc_mul(r.value, tr.value), sc_mul(p.value, tp.value));
  code := ifx sc_is_zero(sum) then LNK_INFINITY else LNK_OK;
  stored := sc_store(sum, out);
  r = ScLoad.{}; p = ScLoad.{}; sum = ScScalar.{};
  return code;
}
```

## The example

The example,
[bitcoin/examples/lightning-keys.coax](../examples/lightning-keys.coax),
runs BOLT 3's Appendix E, derives every key of one of Bob's commitments,
and then revokes it: Alice stores Bob's per-commitment secrets with
chapter 107's shachain and makes the revocation private key.

<!-- source: bitcoin/examples/lightning-keys.coax example -->
```jai
#load "../../coax/lib/lightning/keys.coax";
#load "../../coax/lib/lightning/shachain.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Bytes as lowercase hex.
show :: (b: [] u8) {
  text: [128] u8;
  r := hex_encode_checked(cast(string) b, text[..], false);
  print("%", cast(string) text[0..<r.written]);
}
// A 32-byte secret whose bytes are k, k + 1, k + 2, ...
secret :: (k: int) -> [32] u8 { s: [32] u8; for i: 0..<32 { s[i] = cast(u8) ((k + i) & 255); } return s; }
// Whether secret * G is the compressed key.
signs_for :: (s: [] u8, key: [] u8) -> bool { p := lnk_point(s); return cb_ct_equal(p.bytes[..], key) == CB_MAX64; }
// A key derived from the basepoint of a secret.
derived :: (base_secret: [] u8, point: [] u8) -> LnkKey { b := lnk_point(base_secret); return lnk_pubkey_vartime(b.bytes[..], point); }
LNK_CODE_NAMES :: [5] string.["OK", "BAD_POINT", "BAD_SECRET", "BAD_TWEAK", "INFINITY"];

main :: () {
  // ---- BOLT 3's Appendix E
  print("BOLT 3 key derivation vectors\n");
  bs: [32] u8; ps: [32] u8; out: [32] u8;
  lnk_fill(LNK_VECTOR.base_secret, bs[..]); lnk_fill(LNK_VECTOR.per_commitment_secret, ps[..]);
  base := lnk_point(bs[..]); point := lnk_point(ps[..]);
  print("  base_point: %\n", lnk_hex_is(base.bytes[..], LNK_VECTOR.base_point));
  print("  per_commitment_point: %\n", lnk_hex_is(point.bytes[..], LNK_VECTOR.per_commitment_point));
  k := lnk_pubkey_vartime(base.bytes[..], point.bytes[..]);
  print("  localpubkey: %\n", lnk_hex_is(k.bytes[..], LNK_VECTOR.localpubkey));
  code := lnk_privkey(bs[..], point.bytes[..], out[..]);
  print("  localprivkey: %, times G is localpubkey: %\n", lnk_hex_is(out[..], LNK_VECTOR.localprivkey),
        signs_for(out[..], k.bytes[..]));
  k = lnk_revocation_pubkey_vartime(base.bytes[..], point.bytes[..]);
  print("  revocationpubkey: %\n", lnk_hex_is(k.bytes[..], LNK_VECTOR.revocationpubkey));
  code = lnk_revocation_privkey(bs[..], ps[..], out[..]);
  print("  revocationprivkey: %, times G is revocationpubkey: %\n", lnk_hex_is(out[..], LNK_VECTOR.revocationprivkey),
        signs_for(out[..], k.bytes[..]));

  // ---- one commitment of Bob's, from both sides
  print("Bob's commitment 42\n");
  seed := secret(100);
  pcs: [32] u8;
  code = shachain_generate(seed[..], shachain_index(42), pcs[..]);
  pcp := lnk_point(pcs[..]);
  print("  per_commitment_point: "); show(pcp.bytes[..]); print("\n");
  bob_delayed := secret(2); bob_htlc := secret(3);
  alice_payment := secret(11); alice_htlc := secret(13); alice_revocation := secret(14);
  alice_pay_point := lnk_point(alice_payment[..]);
  alice_rev_point := lnk_point(alice_revocation[..]);
  // Bob's to_local output: his delayed key, or Alice's revocation key.
  delayed := derived(bob_delayed[..], pcp.bytes[..]);
  print("  local_delayedpubkey: "); show(delayed.bytes[..]); print("\n");
  revocation := lnk_revocation_pubkey_vartime(alice_rev_point.bytes[..], pcp.bytes[..]);
  print("  revocationpubkey:    "); show(revocation.bytes[..]); print("\n");
  // Alice's to_remote output: her payment basepoint, unchanged.
  remote := lnk_remotepubkey_vartime(alice_pay_point.bytes[..]);
  print("  remotepubkey:        "); show(remote.bytes[..]); print("\n");
  local_htlc := derived(bob_htlc[..], pcp.bytes[..]);
  remote_htlc := derived(alice_htlc[..], pcp.bytes[..]);
  print("  local_htlcpubkey:    "); show(local_htlc.bytes[..]); print("\n");
  print("  remote_htlcpubkey:   "); show(remote_htlc.bytes[..]); print("\n");
  // Bob can sign for his own keys: each private key times G is the key.
  code = lnk_privkey(bob_delayed[..], pcp.bytes[..], out[..]);
  print("  Bob's delayed private key matches: %\n", signs_for(out[..], delayed.bytes[..]));

  // ---- revocation: Alice needs Bob's per-commitment secret
  print("revoking commitment 42\n");
  // Before: Bob's secret alone, or Alice's alone, is not the key.
  print("  Bob's per-commitment secret alone: %\n", signs_for(pcs[..], revocation.bytes[..]));
  print("  Alice's revocation secret alone: %\n", signs_for(alice_revocation[..], revocation.bytes[..]));
  // Bob sends the secret in revoke_and_ack; Alice checks it against the
  // point she was given, stores it, and can now make the key.
  print("  the secret matches the point Bob sent: %\n", signs_for(pcs[..], pcp.bytes[..]));
  store: [1] Shachain; store[0] = shachain_new(); s: [32] u8; accepted := 0;
  for n: 0..<43 {
    code = shachain_generate(seed[..], shachain_index(cast(u64) n), s[..]);
    if shachain_insert(store[..], shachain_index(cast(u64) n), s[..]) == SHACHAIN_OK { accepted += 1; }
  }
  print("  Alice's store holds % secrets\n", accepted);
  code = shachain_lookup(store[..], shachain_index(42), s[..]);
  code = lnk_revocation_privkey(alice_revocation[..], s[..], out[..]);
  print("  revocationprivkey: %, times G is revocationpubkey: %\n", LNK_CODE_NAMES[code],
        signs_for(out[..], revocation.bytes[..]));
  // The secret of commitment 41 makes a different key.
  code = shachain_lookup(store[..], shachain_index(41), s[..]);
  code = lnk_revocation_privkey(alice_revocation[..], s[..], out[..]);
  print("  commitment 41's secret instead: %\n", signs_for(out[..], revocation.bytes[..]));

  // ---- refusals
  print("refusals\n");
  bad := pcp.bytes; bad[0] = 4;
  print("  per-commitment point with prefix 04: %\n", LNK_CODE_NAMES[lnk_pubkey_vartime(base.bytes[..], bad[..]).code]);
  zero: [32] u8;
  print("  a zero secret: %\n", LNK_CODE_NAMES[lnk_point(zero[..]).code]);
}
```

From the repository root:

```shell
coax/bin/coax run bitcoin/examples/lightning-keys.coax
```

The output:

```text
BOLT 3 key derivation vectors
  base_point: true
  per_commitment_point: true
  localpubkey: true
  localprivkey: true, times G is localpubkey: true
  revocationpubkey: true
  revocationprivkey: true, times G is revocationpubkey: true
Bob's commitment 42
  per_commitment_point: 03a79d1ab8b2a02c4ded1eeb0b2cf1a3f20f4b18e20acbe915cddcb8a8cf7ffce9
  local_delayedpubkey: 03366d2771d9894e5a2a1afa85af9ceeb66499e3724ce7163c889e198086f66589
  revocationpubkey:    022b1875e619c4f88e5ab859b7d009226ac21b945ece3594634c48e61ee2c980d9
  remotepubkey:        03819b98a6aeaa8c3af9b03e405f66a6f52848a036967259dc180f71096dcb378b
  local_htlcpubkey:    038aadcce85642317b269f00571192b059a37cdf1a41601053857295b92b89c463
  remote_htlcpubkey:   02dbaa7aaf7e374e60e3e175094bd7f0b149b8b386f047c80df5abaac72e71dbd7
  Bob's delayed private key matches: true
revoking commitment 42
  Bob's per-commitment secret alone: false
  Alice's revocation secret alone: false
  the secret matches the point Bob sent: true
  Alice's store holds 43 secrets
  revocationprivkey: OK, times G is revocationpubkey: true
  commitment 41's secret instead: false
refusals
  per-commitment point with prefix 04: BAD_POINT
  a zero secret: BAD_SECRET
```

What the output shows:

- Appendix E's two points and four keys come out byte for byte, and each
  private key times G is its public key.
- Bob's commitment 42 has its own per-commitment point from his shachain
  seed, and from it the delayed key, the revocation key and both HTLC
  keys. `remotepubkey` is Alice's payment basepoint.
- Bob can sign for his delayed key.
- Neither Bob's per-commitment secret nor Alice's revocation secret is
  the revocation key alone.
- After Bob's 43 secrets arrive, Alice looks up commitment 42's secret and
  makes the revocation private key. The secret of commitment 41 does not
  make it.
- A per-commitment point with prefix `04` and a zero secret are refused.

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/lightning-keys.out](../fixtures/lightning-keys.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/lightning-keys.coax](../examples/checks/lightning-keys.coax),
which loads the example:

- for four basepoints and four commitments, the derived private key
  times G is the derived public key
- for three pairs of secrets, the revocation private key times G is the
  revocation public key
- two commitments from one revocation basepoint get different revocation
  keys, and neither secret alone is either

<!-- source: bitcoin/examples/checks/lightning-keys.coax check -->
```jai
// For four basepoint secrets and four commitments of one seed, the
// derived private key times G is the derived public key.
#forall "a derived private key signs for the derived key" k: 0..<4 #steps 3000000000 {
  base := secret(17 * k + 5); seed := secret(200); pcs: [32] u8; out: [32] u8;
  code := shachain_generate(seed[..], shachain_index(cast(u64) (k * 1000)), pcs[..]);
  pcp := lnk_point(pcs[..]);
  key := derived(base[..], pcp.bytes[..]);
  #assert key.code == LNK_OK && lnk_privkey(base[..], pcp.bytes[..], out[..]) == LNK_OK;
  #assert signs_for(out[..], key.bytes[..]);
}
// For three pairs of secrets, the revocation private key made from both
// times G is the revocation key made from both points.
#forall "both secrets make the revocation key" k: 0..<3 #steps 3000000000 {
  r := secret(31 * k + 9); p := secret(43 * k + 2); out: [32] u8;
  big_r := lnk_point(r[..]); big_p := lnk_point(p[..]);
  key := lnk_revocation_pubkey_vartime(big_r.bytes[..], big_p.bytes[..]);
  #assert lnk_revocation_privkey(r[..], p[..], out[..]) == LNK_OK && signs_for(out[..], key.bytes[..]);
}
// Each commitment gets its own revocation key from the same basepoint,
// and neither secret alone is it.
#test "every commitment has its own revocation key" #steps 2000000000 {
  r := secret(9); p := secret(2); q := secret(3);
  big_r := lnk_point(r[..]); big_p := lnk_point(p[..]); big_q := lnk_point(q[..]);
  a := lnk_revocation_pubkey_vartime(big_r.bytes[..], big_p.bytes[..]);
  b := lnk_revocation_pubkey_vartime(big_r.bytes[..], big_q.bytes[..]);
  #assert cb_ct_equal(a.bytes[..], b.bytes[..]) == 0;
  #assert !signs_for(r[..], a.bytes[..]) && !signs_for(p[..], a.bytes[..]);
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/lightning-keys.coax
```

`coax test` runs 153 checks, including the loaded files' own checks, in
about 95 seconds on 2026-11-03.

### The library's own checks

`keys.coax` is an `#authority` file. Its checks cover Appendix E's base
point and per-commitment point, `localpubkey` and `localprivkey`,
`revocationpubkey` and `revocationprivkey`, the tweak BOLT 3 shows in its
comments, `remotepubkey`, and refusals for bad points, short inputs and
zero or out-of-range secrets.

```shell
coax/bin/coax test coax/lib/lightning/keys.coax
```

`coax test` runs 142 checks, including those of the files it loads, in
about 35 seconds on 2026-11-03. Mutation testing: not run (owner speed
order).

### BOLT 3 and an independent derivation

[keys.py](../../coax/tests/lightning/keys.py) computes every line the
example prints with its own Python derivation, written from BOLT 3's "Key
Derivation" on
[ln_crypto.py](../../coax/tests/lightning/ln_crypto.py)'s secp256k1 and
[shachain.py](../../coax/tests/lightning/shachain.py)'s generator and
store, sharing no code with Coax. Appendix E's keys must come out before
the transcript is compared with the fixture.

```shell
python3 -I coax/tests/lightning/keys.py --coax coax/bin/coax
```

On 2026-11-03 all 25 lines agree, and the example's output in native debug
and release builds equals them. Wasm was not run.

[make_keys_vectors.py](../../coax/tests/lightning/make_keys_vectors.py)
reads the pinned BOLT 3 into
[keys_vectors.coax](../../coax/tests/lightning/keys_vectors.coax).

## What this chapter leaves out

- The scripts and transactions that use these keys: chapter 108.
- Sending points and secrets in `open_channel` and `revoke_and_ack`, and
  checking a revealed secret against its point: chapter 109.
- Choosing basepoint secrets, which come from chapter 18's randomness or
  a wallet's key derivation.
- The older, tweaked `remotepubkey` from before `option_static_remotekey`.

## Summary

- A commitment's per-commitment point is its shachain secret times G.
- `localpubkey`, the HTLC keys and the delayed keys are a basepoint plus
  SHA256(per_commitment_point || basepoint) times G; the private key adds
  the same tweak to the basepoint secret.
- With `option_static_remotekey`, `remotepubkey` is the payment basepoint.
- The revocation key is R·SHA256(R || P) + P·SHA256(P || R). Its private
  key needs both r and p, so a peer can sign for it only after the other
  revokes the commitment.

## Dependencies

- [4. SHA-256](ch004-sha-256.md)
- [16. Scalar multiplication](ch016-scalar-multiplication.md)

## Sources

- [BOLT 3](https://github.com/lightning/bolts/blob/master/03-transactions.md),
  "Key Derivation" and "Appendix E: Key Derivation Test Vectors", at
  lightning/bolts revision `19146282`, pinned in
  [coax/tests/vectors/upstream/bolts/](../../coax/tests/vectors/upstream/bolts/03-transactions.md).
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
