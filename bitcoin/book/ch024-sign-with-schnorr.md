# 24. Sign with Schnorr

Signing uses a private key and a nonce. A repeated or biased nonce reveals the
key. This chapter builds BIP-340 signing with its nonce derivation, checks the
result against the official signing vectors and verifies each signature before
it leaves the signer.

## What this chapter teaches

- Key negation when `d·G` has an odd `y`, and the same rule for the nonce.
- The nonce: `t = d xor hash_aux(a)`, `k0 = int(hash_nonce(t || P || m)) mod n`,
  and why one nonce over two messages gives the key away.
- The equation `s = (k + e·d) mod n` and the output `R.x || s`.
- Self-verification with fixed work: the signer checks its own signature, and
  nothing branches on the candidate before it is released.
- Resumable stages: one signature is six calls, each small enough for the
  interpreter's check budget and for one browser frame.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [schnorr_sign.coax](../../coax/lib/crypto/schnorr_sign.coax) | `schnorr_sign_step(key, aux, message, work, out, overlap)` | one stage per call; `SIGN_MORE` until `SIGN_DONE` or `SIGN_REFUSED` (`#authority`) |
| | `nostr_sign_step` | the same over exactly 32 bytes of event ID |
| [schnorr_prepare_workspace.coax](../../coax/lib/crypto/schnorr_prepare_workspace.coax) | `sk_prepare` | stage 1: checks `0 < d < n`, computes `d·G`, the x-only key and the even-`y` key |
| [schnorr_nonce_workspace.coax](../../coax/lib/crypto/schnorr_nonce_workspace.coax) | `sn_nonce` | the two tagged hashes that give the nonce |
| [schnorr_nonce_point.coax](../../coax/lib/crypto/schnorr_nonce_point.coax) | `snp_prepare` | stage 2: the nonce, `k0 mod n`, its zero check and `R = k0·G` |
| [schnorr_math_workspace.coax](../../coax/lib/crypto/schnorr_math_workspace.coax) | `sm_equation` | stage 3: the negations and `s = k + e·d mod n` |
| [schnorr_scalar_workspace.coax](../../coax/lib/crypto/schnorr_scalar_workspace.coax) | `ss_key`, `ss_even` | loading a secret scalar without branching, and the conditional negation |
| [schnorr_self_verify.coax](../../coax/lib/crypto/schnorr_self_verify.coax) | `ssv_step` | stages 4 to 6: the fixed-work verification of the unreleased candidate (`#authority`) |

All inputs are raw bytes: a 32-byte secret key, 32 bytes of auxiliary
randomness `a`, and a message of 0 to 1,048,576 bytes. The caller owns one
`SignWork`, a struct that holds every intermediate value between calls. The
signer writes `out` only when it returns `SIGN_DONE`, and it clears the
workspace whenever signing ends, refused or done. The auxiliary bytes come
from the host's random source (chapter 18).

## The algorithm

BIP-340 signs a message `m` with the secret key `d0` and the auxiliary bytes
`a`. `G` is the generator, `n` the group order, and `hash_tag(x)` is
`SHA-256(SHA-256(tag) || SHA-256(tag) || x)`, as in chapter 23.

1. **The key.** Refuse unless `0 < d0 < n`. Compute `P = d0·G`. If `y(P)` is
   odd, sign with `d = n - d0`, otherwise `d = d0`. The public key is `x(P)`.
2. **The nonce.** `t = bytes(d) xor hash_BIP0340/aux(a)`, then
   `rand = hash_BIP0340/nonce(t || x(P) || m)` and `k0 = int(rand) mod n`.
   Refuse if `k0 = 0`.
3. **The nonce point.** `R = k0·G`. If `y(R)` is odd, use `k = n - k0`,
   otherwise `k = k0`.
4. **The challenge.** `e = int(hash_BIP0340/challenge(x(R) || x(P) || m)) mod n`,
   the same `e` a verifier computes.
5. **The equation.** `s = (k + e·d) mod n`. The signature is the 64 bytes
   `x(R) || s`.
6. **Self-check.** Verify the signature against `x(P)` and `m`. BIP-340
   recommends this step; this library makes it mandatory and publishes
   nothing that fails it.

A verifier sees only `x(P)` and `x(R)`, and lifts each to the point with an
even `y`. The negations in steps 1 and 3 make the signer's points equal to
those points.

### The stages

`schnorr_sign_step` does one piece per call. The workspace's `stage` field
says which:

| Call | Stage | Work | Procedures |
| --- | --- | --- | --- |
| 1 | 0 | step 1: load `d0`, compute `d0·G`, `x(P)`, the parity and `d` | `sw_key`, `sk_prepare` |
| 2 | 1 | steps 2 and 3: the nonce hashes, `k0`, `k0·G` and `x(R)`; record the message's SHA-256 | `sw_nonce`, `snp_prepare` |
| 3 | 2 | steps 4 and 5, after checking that the message is the one the nonce came from | `sw_scalar`, `sm_equation` |
| 4 | 3 | self-check stage 0: lift `P`, copy the candidate, hash `e` | `sw_check`, `ssv_step` |
| 5 | 4 | self-check stage 1: `(-e)·P` | `sw_check`, `ssv_step` |
| 6 | 5 | self-check stage 2: `s·G`, the sum and the comparison; publish or refuse | `sw_publish`, `ssv_step` |

A whole signature through the fixed-work check is about 30.4 million
interpreter steps (the 2026-10-30 record), more than the 20 million that
`coax mutate` gives one check. The stages keep each piece under that limit,
and they let a web page spread a signature over several frames.

### Secrets in the code

Chapter 3's rule is that nothing branches on a secret or indexes memory by
one. The signer meets it this way:

- `ss_key` loads `d0` by subtracting `n` and keeping the borrow, as
  chapter 18's `sc_load` does, and ORs the limbs to test for zero. A bad key
  becomes the scalar 1 for the rest of the work, and a validity bit records
  that it was bad.
- `ss_even(scalar, odd)` is `sc_select(odd, sc_negate(scalar), scalar)`: it
  computes both values and picks one with a mask. The parities never become
  branches.
- `k0 = 0` is tested the same way: `snp_reduce` keeps a validity bit and
  uses 1 in its place. Finding an `a` that makes `rand` a multiple of `n` is
  as hard as breaking SHA-256, so no test vector reaches this case.
- Both scalar multiplications by `G` use the fixed-base table of chapter 16,
  which reads every entry.

The signer reveals four facts, each through a procedure whose name ends in
`_declassify`, so that the secret-source audit (below) can list them:
whether the key and nonce are valid, the public key `x(P)`, `x(R)`, and the
self-check's verdict. The public key and `x(R)` are published anyway.

### Why one nonce for two messages gives the key away

Two signatures with the same `k` and different challenges `e1` and `e2`
satisfy `s1 = k + e1·d` and `s2 = k + e2·d`. Subtract them:
`s2 - s1 = (e2 - e1)·d`, so `d = (s2 - s1) / (e2 - e1) mod n`, computed
from public values. BIP-340 prevents this by hashing `m` into the nonce, so
that a different message gives a different `k`. Hashing in `d` keeps `k`
secret. Hashing in `a` protects against fault and side-channel attacks on a
fully deterministic nonce.

The staged signer adds one more risk: the caller passes the message again on
every call. A caller that changed it after call 2 would get one nonce over
two messages. So call 2 records the message's SHA-256 in the workspace, and
call 3 refuses and clears the workspace unless the message has the same
digest. Later calls need no such check, because the self-check verifies
against each call's message and fails for any other.

### Worked example: small numbers

Use chapter 23's numbers: the key `d = 3`, the nonce `k = 5`, and a
stand-in challenge `e = 2`, because a real `e` comes from the hash. The `y`
parities are: `3G` even, `5G` even, `6G` odd.

1. `d = 3`, `k = 5`, `e = 2`. Both points have an even `y`, so nothing is
   negated: `s = 5 + 2·3 = 11`. A verifier computes `11G - 2·(3G) = 5G`
   and accepts.
2. `d = 6`. `6G` has an odd `y`, so the public key `x(6G)` lifts to
   `-6G`, and the signer must use `n - 6`:
   `s = 5 + 2·(n - 6) = n - 7`. The verifier computes
   `(n - 7)G - 2·(-6G) = -7G + 12G = 5G` and accepts. Without the negation,
   `s = 5 + 12 = 17`, the verifier computes `17G + 12G = 29G`, and it
   refuses.
3. `d = 3`, `k = 6`, `e = 4`. `6G` has an odd `y`, so the signer uses
   `k = n - 6`, and `s = (n - 6) + 4·3 = 6`. The verifier computes
   `6G - 4·(3G) = -6G`, whose `x` is `r = x(6G)` and whose `y` is even. It
   accepts.
4. One nonce twice: `d = 3`, `k = 5`, `e1 = 2` and `e2 = 4` give `s1 = 11`
   and `s2 = 17`. Then `(17 - 11) / (4 - 2) = 3`, the key.

### Worked example: vector 0

Vector 0 signs 32 zero bytes with `d0 = 3` and 32 zero bytes of `a`. The
values below were computed with a Python transcription of BIP-340's signing
algorithm, using `hashlib` and integers. It shares no code with Coax and
reproduces all four signing rows.

1. `P = 3G` has an even `y`, so `d = 3` and `x(P) = f9308a01...bce036f9`.
2. `hash_aux(a) = 54f169cf...270aa514`. XOR with `d = 00...03` changes only
   the last byte: `t = 54f169cf...270aa517`.
3. `rand = hash_nonce(t || x(P) || m) = 1d2dc165...30dd1c69`. It is below
   `n`, so `k0 = rand`.
4. `R = k0·G` has `x(R) = e907831f...2dca8215` and an odd `y`, so the
   signer uses `k = n - k0`.
5. `e = 6bb6b93a...bb5d86ce`, the value chapter 23's verifier computes for
   this signature.
6. `s = (n - k0) + 3e mod n = 25f66a4a...310536c0`.

The signature `x(R) || s` is vector 0's published signature. Of the four
signing rows, vector 0 has the odd `R`, and vector 3 has the odd `P`: its key
`0b432b26...d2401710` is replaced by `n - d0 = f4bcd4d9...fdf62a31`.
Vectors 1 and 2 negate neither, so the four rows cover both negations.

## Self-verification with fixed work

A fault in the hardware or the code can produce a wrong `s`. A wrong
signature does more harm than a refusal: a faulty `s'` next to the correct
`s` for the same nonce can leak `d`. So the signer verifies its own
signature and publishes it only if it is valid.

The self-check can't use chapter 23's `schnorr_verify_vartime`. That
verifier branches on `r` and `s`, and until the signature is released, `s`
is secret: a refused candidate is never published, so its bits must not
reach a branch or a memory address. `schnorr_self_verify.coax` computes the
same equation, `R' = s·G - e·P`, with fixed work:

- `s·G` with the fixed-base comb, and `(-e)·P` with the constant-time
  multiplication of chapter 16;
- six masks instead of six early returns: `r < p`, `s < n`, `R'` finite,
  `y(R')` even, `x(R') = r`, and a binding mask that holds when the key, the
  candidate and `e` are the same at every stage as when the check began;
- one result, the AND of the masks, read once at the end.

`sw_publish` always runs the last self-check stage before it reads the
three validity bits (key, nonce, equation), so a refusal does the same work
as a publication. Only then does it copy the signature to `out`.

## The example

The example is [bitcoin/examples/schnorr-sign.coax](../examples/schnorr-sign.coax).
It runs the small-number equations, signs vector 0 one call at a time and
prints the workspace after each stage, signs vectors 0 to 3 byte for byte
and checks each with chapter 23's verifier, and then shows eight refusals.
The vector table is at the end of the file, outside the listing. Its secret
keys are BIP-340's published test keys; the example prints intermediate
values only because of that.

<!-- source: bitcoin/examples/schnorr-sign.coax example -->
```jai
#load "../../coax/lib/crypto/schnorr_sign.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
print_hex :: (label: string, bytes: [] u8) {
  text: [128] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
print_scalar :: (label: string, s: ScScalar) {
  b: [32] u8;
  if sc_store(s, b[..]) { print_hex(label, b[..]); }
}
// Decode hex into out and return the number of bytes written. Every literal
// here is valid hex, so a decoding error stops the program.
decode_hex :: (text: string, out: [] u8) -> int {
  r := hex_decode_checked(text, out, false);
  #assert r.error == 0;
  return r.written;
}
// Signing row i of the table at the end of this file: BIP-340 vectors 0 to 3.
// Their secret keys are published test keys, never real ones.
SignRow :: struct { key: [32] u8; aux: [32] u8; message: [32] u8; signature: [64] u8; }
sign_row :: (i: int) -> SignRow {
  v: SignRow;
  #assert decode_hex(ROW_HEX[4 * i], v.key[..]) == 32;
  #assert decode_hex(ROW_HEX[4 * i + 1], v.aux[..]) == 32;
  #assert decode_hex(ROW_HEX[4 * i + 2], v.message[..]) == 32;
  #assert decode_hex(ROW_HEX[4 * i + 3], v.signature[..]) == 64;
  return v;
}
bytes_equal :: (a: [] u8, b: [] u8) -> bool {
  if a.count != b.count { return false; }
  for i: 0..<a.count { if a[i] != b[i] { return false; } }
  return true;
}
all_zero :: (a: [] u8) -> bool {
  for i: 0..<a.count { if a[i] != 0 { return false; } }
  return true;
}
named :: (status: int) -> string {
  if status == SIGN_MORE { return "more"; }
  if status == SIGN_DONE { return "done"; }
  return "refused";
}
// Call the signer until it stops asking for more. Returns the last status and
// counts the calls.
Signed :: struct { status: int; calls: int; }
sign_to_end :: (key: [] u8, aux: [] u8, message: [] u8, work: [] SignWork, out: [] u8) -> Signed {
  r: Signed; r.status = SIGN_MORE;
  while r.status == SIGN_MORE {
    r.status = schnorr_sign_step(key, aux, message, work, out, false);
    r.calls += 1;
  }
  return r;
}

// Small numbers. k·G by repeated addition, with chapter 15's group law.
times_g :: (k: int) -> SgPoint {
  r := sg_infinity();
  for i: 0..<k { r = sg_add(r, sg_generator()).value; }
  return r;
}
y_odd :: (p: SgPoint) -> u64 { return sg_to_affine(p).value.y.limb[0] & 1; }
// Chapter 23's final equation with a stand-in challenge e: lift x(dG) to the
// even-y point P, as a verifier does, and test s·G - e·P against x(kG).
verify_small :: (s: ScScalar, e: u32, d: int, k: int) -> bool {
  x: [32] u8;
  if !sf_store(sg_to_affine(times_g(d)).value.x, x[..]) { return false; }
  prepared: SvPrepared;
  prepared.s = s; prepared.negative_e = sc_negate(sc_small(e));
  prepared.point = sg_lift_x_public(x[..]).value;
  prepared.r = sg_to_affine(times_g(k)).value.x; prepared.ready = true;
  return sv_prepared_matches_vartime(prepared);
}
// The signer's equation with small numbers: negate d and k when their points
// have odd y (ss_even, the procedure the signer uses), then s = k + e·d.
sign_small :: (d: int, k: int, e: u32) -> ScScalar {
  dd := ss_even(sc_small(cast(u32) d), y_odd(times_g(d)));
  kk := ss_even(sc_small(cast(u32) k), y_odd(times_g(k)));
  return sc_add(kk, sc_mul(sc_small(e), dd));
}

main :: () {
  print("small numbers\n");
  print("y odd: 3G %, 5G %, 6G %\n", y_odd(times_g(3)), y_odd(times_g(5)), y_odd(times_g(6)));
  s := sign_small(3, 5, 2);
  print_scalar("d = 3, k = 5, e = 2: s", s);
  print("verifies: %\n", verify_small(s, 2, 3, 5));
  s = sign_small(6, 5, 2);
  print_scalar("d = 6 (odd y, use n - 6), k = 5, e = 2: s", s);
  print("verifies: %\n", verify_small(s, 2, 6, 5));
  unnegated := sc_add(sc_small(5), sc_mul(sc_small(2), sc_small(6)));
  print("without the negation, s = 17 verifies: %\n", verify_small(unnegated, 2, 6, 5));
  s = sign_small(3, 6, 4);
  print_scalar("d = 3, k = 6 (odd y, use n - 6), e = 4: s", s);
  print("verifies: %\n", verify_small(s, 4, 3, 6));
  // One nonce for two messages: s1 - s2 = (e1 - e2)·d, so anyone can solve for d.
  s1 := sign_small(3, 5, 2); s2 := sign_small(3, 5, 4);
  inverse := sc_inverse(sc_sub(sc_small(4), sc_small(2)));
  print_scalar("same k twice: (s2 - s1) / (e2 - e1)", sc_mul(sc_sub(s2, s1), inverse.value));

  // Vector 0, one stage per call. The workspace holds the intermediate values
  // between calls; they are printed here only because the key is a test key.
  print("vector 0, stage by stage\n");
  v := sign_row(0);
  work: [1] SignWork; out: [64] u8;
  status := schnorr_sign_step(v.key[..], v.aux[..], v.message[..], work[..], out[..], false);
  print("call 1, key: %\n", named(status));
  print_hex("  P = x(d*G)", work[0].prep[0].public_key[..]);
  print("  y(d*G) odd: %\n", work[0].prep[0].point[0].odd);
  status = schnorr_sign_step(v.key[..], v.aux[..], v.message[..], work[..], out[..], false);
  print("call 2, nonce: %\n", named(status));
  print_hex("  hash_aux(a)", work[0].nonce[0].hash_work[0].digest[..]);
  print_hex("  t = d xor hash_aux(a)", work[0].nonce[0].hash_work[0].t[..]);
  print_hex("  rand = hash_nonce(t || P || m)", work[0].nonce[0].hash[..]);
  print_hex("  r = x(k0*G)", work[0].nonce[0].r[..]);
  print("  y(k0*G) odd: %\n", work[0].nonce[0].point[0].odd);
  status = schnorr_sign_step(v.key[..], v.aux[..], v.message[..], work[..], out[..], false);
  print("call 3, equation: %\n", named(status));
  print_hex("  e", work[0].challenge[..]);
  print_hex("  s = k + e*d mod n", work[0].sig[32..<64]);
  print("  out still zero: %\n", all_zero(out[..]));
  for call: 4..<7 {
    status = schnorr_sign_step(v.key[..], v.aux[..], v.message[..], work[..], out[..], false);
    print("call %, self-check: %\n", call, named(status));
  }
  print_hex("signature", out[..]);
  print("matches vector 0: %, workspace cleared: %\n", bytes_equal(out[..], v.signature[..]), work[0].stage == 0 && all_zero(work[0].sig[..]));

  // Vectors 0 to 3, byte for byte, then checked by the variable-time verifier
  // of chapter 23, which shares no code with the self-check's equation.
  print("BIP-340 signing vectors 0 to 3\n");
  for i: 0..<ROWS {
    t := sign_row(i); pk := ROW_PUBLIC[i]; w: [1] SignWork; sig: [64] u8;
    r := sign_to_end(t.key[..], t.aux[..], t.message[..], w[..], sig[..]);
    print("%: % after % calls, matches %, verifies %\n", i, named(r.status), r.calls,
      bytes_equal(sig[..], t.signature[..]), schnorr_fast_verify_vartime(t.message[..], pk[..], sig[..]));
  }
  // The same key and message with other auxiliary bytes give another valid
  // signature: the nonce depends on a, not only on d and m.
  other := v.aux; other[31] = 1; pk := ROW_PUBLIC[0];
  w: [1] SignWork; sig: [64] u8;
  r := sign_to_end(v.key[..], other[..], v.message[..], w[..], sig[..]);
  print("vector 0 with aux 00..01: %, differs %, verifies %\n", named(r.status),
    !bytes_equal(sig[..], v.signature[..]), schnorr_fast_verify_vartime(v.message[..], pk[..], sig[..]));

  print("refusals\n");
  bad: [32] u8;
  r = sign_to_end(bad[..], v.aux[..], v.message[..], w[..], sig[..]);
  print("secret key 0: % after % call\n", named(r.status), r.calls);
  #assert ss_write(SC_N, bad[..]);
  r = sign_to_end(bad[..], v.aux[..], v.message[..], w[..], sig[..]);
  print("secret key n: % after % call\n", named(r.status), r.calls);
  for i: 0..<32 { bad[i] = 255; }
  r = sign_to_end(bad[..], v.aux[..], v.message[..], w[..], sig[..]);
  print("secret key 2^256 - 1: % after % call\n", named(r.status), r.calls);
  long: [33] u8;
  status = schnorr_sign_step(v.key[..], long[0..<31], v.message[..], w[..], sig[..], false);
  print("aux of 31 bytes: %, workspace untouched %\n", named(status), w[0].stage == 0);
  status = schnorr_sign_step(v.key[..], long[..], v.message[..], w[..], sig[..], false);
  print("aux of 33 bytes: %, workspace untouched %\n", named(status), w[0].stage == 0);
  status = nostr_sign_step(v.key[..], v.aux[..], long[0..<31], w[..], sig[..], false);
  print("Nostr event ID of 31 bytes: %\n", named(status));

  // The message changes after the nonce was derived from it: signing it
  // would use one nonce for two messages, so the equation stage refuses.
  for i: 0..<64 { sig[i] = 0; }
  status = schnorr_sign_step(v.key[..], v.aux[..], v.message[..], w[..], sig[..], false);
  status = schnorr_sign_step(v.key[..], v.aux[..], v.message[..], w[..], sig[..], false);
  changed := v.message; changed[31] ^= 1;
  status = schnorr_sign_step(v.key[..], v.aux[..], changed[..], w[..], sig[..], false);
  print("message changed after the nonce stage: % at call 3, workspace cleared %\n", named(status), w[0].stage == 0 && all_zero(w[0].nonce[0].r[..]));

  // A fault flips one bit of s after the equation, before release. The
  // self-check runs all of its stages on the bad candidate and refuses at
  // the end; nothing reaches out.
  for call: 1..<4 { status = schnorr_sign_step(v.key[..], v.aux[..], v.message[..], w[..], sig[..], false); }
  w[0].sig[63] ^= 1;
  calls := 3;
  while status == SIGN_MORE {
    status = schnorr_sign_step(v.key[..], v.aux[..], v.message[..], w[..], sig[..], false);
    calls += 1;
  }
  print("s corrupted before release: % at call %, out still zero %\n", named(status), calls, all_zero(sig[..]));
}
```

Run it from the repository root. The signer's fixed-base tables are computed
at compile time and need more than the default compile-time budget:

```shell
coax/bin/coax run bitcoin/examples/schnorr-sign.coax --run-steps 250000000
```

The output shows the values worked out by hand: `s = 11`, `n - 7` and `6`
for the small numbers and `3` recovered from a repeated nonce, then vector
0's `hash_aux(a)`, `t`, `rand`, the odd `y(R)`, `e` and `s`. All four rows
match byte for byte:

```text
small numbers
y odd: 3G 0, 5G 0, 6G 1
d = 3, k = 5, e = 2: s: 000000000000000000000000000000000000000000000000000000000000000b
verifies: true
d = 6 (odd y, use n - 6), k = 5, e = 2: s: fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd036413a
verifies: true
without the negation, s = 17 verifies: false
d = 3, k = 6 (odd y, use n - 6), e = 4: s: 0000000000000000000000000000000000000000000000000000000000000006
verifies: true
same k twice: (s2 - s1) / (e2 - e1): 0000000000000000000000000000000000000000000000000000000000000003
vector 0, stage by stage
call 1, key: more
  P = x(d*G): f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9
  y(d*G) odd: 0
call 2, nonce: more
  hash_aux(a): 54f169cfc9e2e5727480441f90ba25c488f461c70b5ea5dcaaf7af69270aa514
  t = d xor hash_aux(a): 54f169cfc9e2e5727480441f90ba25c488f461c70b5ea5dcaaf7af69270aa517
  rand = hash_nonce(t || P || m): 1d2dc1652fee3ad08434469f9ad30536a5787feccfa308e8fb396c8030dd1c69
  r = x(k0*G): e907831f80848d1069a5371b402410364bdf1c5f8307b0084c55f1ce2dca8215
  y(k0*G) odd: 1
call 3, equation: more
  e: 6bb6b93a91f2ecc0cd924f4f9baabb5e6eb21745bb00f2cebdaac908bb5d86ce
  s = k + e*d mod n: 25f66a4a85ea8b71e482a74f382d2ce5ebeee8fdb2172f477df4900d310536c0
  out still zero: true
call 4, self-check: more
call 5, self-check: more
call 6, self-check: done
signature: e907831f80848d1069a5371b402410364bdf1c5f8307b0084c55f1ce2dca821525f66a4a85ea8b71e482a74f382d2ce5ebeee8fdb2172f477df4900d310536c0
matches vector 0: true, workspace cleared: true
BIP-340 signing vectors 0 to 3
0: done after 6 calls, matches true, verifies true
1: done after 6 calls, matches true, verifies true
2: done after 6 calls, matches true, verifies true
3: done after 6 calls, matches true, verifies true
vector 0 with aux 00..01: done, differs true, verifies true
refusals
secret key 0: refused after 1 call
secret key n: refused after 1 call
secret key 2^256 - 1: refused after 1 call
aux of 31 bytes: refused, workspace untouched true
aux of 33 bytes: refused, workspace untouched true
Nostr event ID of 31 bytes: refused
message changed after the nonce stage: refused at call 3, workspace cleared true
s corrupted before release: refused at call 6, out still zero true
```

The refusals:

- A secret key of 0, `n` or `2^256 - 1`: call 1 refuses (`ss_key`'s
  validity bit), and the workspace is cleared.
- Auxiliary bytes of 31 or 33 bytes: `sw_views` refuses before the
  workspace changes, so `stage` stays 0. A 31-byte Nostr event ID is refused
  the same way by `nostr_sign_step`.
- The message changes after call 2: call 3's `sw_same_message` finds a
  different digest, refuses and clears the nonce point.
- A bit of `s` flipped after call 3, the effect of a fault: the self-check
  runs all three of its stages and refuses at call 6. `out` stays zero.

Changing one byte of `a` gives a different signature for the same key and
message, and it also verifies. BIP-340 signatures aren't unique; only the
key and message are fixed.

The book's checks run this program in debug and release builds and compare
the output with [bitcoin/fixtures/schnorr-sign.out](../fixtures/schnorr-sign.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/schnorr-sign.coax](../examples/checks/schnorr-sign.coax),
which loads the example. They compare the signer with independent values:

- the small-number signatures with the hand values above;
- for each of vectors 0 to 3, every intermediate after the first three
  calls (`hash_aux(a)`, `t`, `rand`, both parities, `x(R)`, `e` and `s`)
  with the Python transcription's values;
- one whole signature in the interpreter, vector 3, whose key is negated.

<!-- source: bitcoin/examples/checks/schnorr-sign.coax check -->
```jai
// The small-number equations against values worked by hand: 5 + 2·3 = 11,
// 5 + 2·(n - 6) = n - 7, (n - 6) + 4·3 = 6, and (17 - 11) / (4 - 2) = 3.
#test "small-number signatures match the hand values" #steps 400000000 {
  #assert sc_equal(sign_small(3, 5, 2), sc_small(11)) && verify_small(sign_small(3, 5, 2), 2, 3, 5);
  #assert sc_equal(sign_small(6, 5, 2), sc_negate(sc_small(7))) && verify_small(sign_small(6, 5, 2), 2, 6, 5);
  #assert sc_equal(sign_small(3, 6, 4), sc_small(6)) && verify_small(sign_small(3, 6, 4), 4, 3, 6);
  #assert !verify_small(sc_small(17), 2, 6, 5);
  s1 := sign_small(3, 5, 2); s2 := sign_small(3, 5, 4);
  #assert sc_equal(sc_mul(sc_sub(s2, s1), sc_inverse(sc_small(2)).value), sc_small(3));
}
// Each vector's intermediate values, from a Python transcription of BIP-340's
// signing algorithm (hashlib and integers; it shares no code with the Coax
// SHA-256, scalar or point arithmetic, and reproduces all four signatures):
// hash_aux(a), t, rand, then y(d·G) odd and y(k0·G) odd as "0"/"1", e and s.
STAGE_HEX :: [28] string.[
    "54f169cfc9e2e5727480441f90ba25c488f461c70b5ea5dcaaf7af69270aa514", "54f169cfc9e2e5727480441f90ba25c488f461c70b5ea5dcaaf7af69270aa517", "1d2dc1652fee3ad08434469f9ad30536a5787feccfa308e8fb396c8030dd1c69", "0", "1", "6bb6b93a91f2ecc0cd924f4f9baabb5e6eb21745bb00f2cebdaac908bb5d86ce", "25f66a4a85ea8b71e482a74f382d2ce5ebeee8fdb2172f477df4900d310536c0",
    "ee8790e3e65f5408c3f41b56a88c4e096126a71abb3a9ccce6efd19dafc7ec32", "5966c1816cb27e627c8543d63478bdce03c1b115838e469a416b0899fe5723dd", "f7becdac22c3d61a97ff4e84a004e1c4919c0e0c51f50dd5bee15c9cbd27318e", "0", "0", "cfb58e748d9648b71fdc909fb7432fc0c954da5bd75cdc9d4804d32648f9839a", "8906d11ac976abccb20b091292bff4ea897efcb639ea871cfa95f6de339e4b0a",
    "73db95cc3e3821f6695b492ef656ca7aadde058711eef8302141229fd3e8bd28", "bad44f6e1f50e3c2ad9d2ba5768ad6ab84dc4b8f9b893444234a9c39e8fc58e1", "f5878384ed63c5ec428e7ab31bdb446b6884dfad76b7e0599af3f5e838409aab", "0", "0", "9bc1ba4a0abbc0792066b2ca0ef771d88af676b322a83dd7517f7c1fd149215a", "ab745879a5ad954a72c45a91c3a51d3c7adea98d82f8481e0e1e03674a6f3fb7",
    "fdb71327ced66d323510223cfa24d25565b63f462b9693fee13c3f16a1993194", "090bc7fe46bae14c641f86732fbdc17bcd8193c5cb9e6e19c0b157eb5c6f1ba5", "d5352655a99381aff7bd274a99310825f0977329210d5f735766d4a7ae39c073", "1", "0", "a5352f4ded269e6716184547944a209417f759df44e61ea1d48008997c9003c0", "97582b9cb13db3933705b32ba982af5af25fd78881ebb32771fc5922efc66ea3"];
matches_hex :: (bytes: [] u8, text: string) -> bool {
  want: [32] u8;
  #assert decode_hex(text, want[..]) == 32;
  return bytes_equal(bytes, want[..]);
}
#test "matches_hex notices a change in the first or last byte" {
  b: [32] u8; text := "0000000000000000000000000000000000000000000000000000000000000000";
  #assert matches_hex(b[..], text);
  b[0] = 1; #assert !matches_hex(b[..], text);
  b[0] = 0; b[31] = 1; #assert !matches_hex(b[..], text);
}
#forall "the first three stages give each vector's Python intermediates" i: 0..<ROWS #steps 400000000 {
  t := sign_row(i); pk := ROW_PUBLIC[i]; work: [1] SignWork; out: [64] u8;
  for call: 0..<3 { #assert schnorr_sign_step(t.key[..], t.aux[..], t.message[..], work[..], out[..], false) == SIGN_MORE; }
  #assert bytes_equal(work[0].prep[0].public_key[..], pk[..]);
  #assert matches_hex(work[0].nonce[0].hash_work[0].digest[..], STAGE_HEX[7 * i]);
  #assert matches_hex(work[0].nonce[0].hash_work[0].t[..], STAGE_HEX[7 * i + 1]);
  #assert matches_hex(work[0].nonce[0].hash[..], STAGE_HEX[7 * i + 2]);
  #assert work[0].prep[0].point[0].odd == ifx STAGE_HEX[7 * i + 3] == "1" then cast(u64) 1 else 0;
  #assert work[0].nonce[0].point[0].odd == ifx STAGE_HEX[7 * i + 4] == "1" then cast(u64) 1 else 0;
  #assert bytes_equal(work[0].nonce[0].r[..], t.signature[0..<32]);
  #assert matches_hex(work[0].challenge[..], STAGE_HEX[7 * i + 5]);
  #assert matches_hex(work[0].sig[32..<64], STAGE_HEX[7 * i + 6]) && all_zero(out[..]);
}
// One whole signature in the interpreter: vector 3, whose key point has odd
// y, through all six stages to the published bytes.
#test "vector 3 signs byte for byte through all six stages" #steps 400000000 {
  t := sign_row(3); work: [1] SignWork; out: [64] u8;
  r := sign_to_end(t.key[..], t.aux[..], t.message[..], work[..], out[..]);
  #assert r.status == SIGN_DONE && r.calls == 6 && bytes_equal(out[..], t.signature[..]);
  #assert work[0].stage == 0 && all_zero(work[0].sig[..]);
}
```

To run them:

```shell
coax/bin/coax test bitcoin/examples/checks/schnorr-sign.coax --run-steps 250000000
coax/bin/coax mutate bitcoin/examples/checks/schnorr-sign.coax --root-only --run-steps 250000000
```

On 2026-11-01, `coax test` ran 263 checks, including the loaded files' own
checks, in under a minute. The small-number check uses 289 million steps,
because it builds each `kG` by repeated addition; the four stage checks use
75 million together, and the whole signature 48 million.
Root-only mutation of the checks file kills 2 of 2 mutants. An earlier
run had one survivor, in the length test of `matches_hex`; the length is now an
assertion, and a new check covers the comparison. The example's own
procedures were not mutated for this chapter.

### The library's own checks

[schnorr_sign.coax](../../coax/lib/crypto/schnorr_sign.coax) checks each
stage against vectors 0 to 3 with intermediates from
[schnorr_sign_vectors.py](../../coax/tests/nostr/schnorr_sign_vectors.py),
an independent pure-Python script. It also checks the refusals: a zero key,
the group order, refused views that leave the workspace unchanged, a corrupt
stage counter, a message changed after the nonce stage, a candidate changed
after the self-check began, and each validity bit cleared at publication.
Stages 5 and 6 start from recorded states, so each check stays under the
20 million step budget. The
[2026-10-30 record](../../docs/verification/2026-10-30-schnorr-sign.md)
reports 256 of 256 checks and zero mutation survivors: 3,754 of 3,754
mutants killed, none marked equivalent. The self-verifier's own record is
[2026-10-29-schnorr-self-verify.md](../../docs/verification/2026-10-29-schnorr-self-verify.md).

### Native, Wasm and 10,000 signatures

[schnorr_sign.py](../../coax/tests/nostr/schnorr_sign.py) drives the
signer through the generated C, in checked and release builds, plain and
under ASan and UBSan, and the release build as Wasm. Each runs 44 requests:
the eight official signing rows byte for byte (vectors 0 to 3 and the rows
with messages of 0, 1, 17 and 100 bytes), 24 random signatures, 4 signed
events and 8 refusals. With `--round-trips 10000` it also signs 10,000
random Nostr events, each under a fresh random key, in the release build.
The [2026-10-30 event-sign record](../../docs/verification/2026-10-30-event-sign.md)
reports that all 10,000 verify (230 seconds). The verifier there is the
pure-Python BIP-340 verifier in
[oracle.py](../../tools/coax-nostr/fuzz/oracle.py), written from the BIP and
sharing no code with Coax. These runs were not repeated for this chapter.

### libsecp256k1 on the signer's output

[schnorr_sign_libsecp.py](../../coax/tests/nostr/schnorr_sign_libsecp.py)
sends the signer's output to libsecp256k1 `v0.8.0`, built as chapter 13
describes. Each Coax signature must pass `secp256k1_schnorrsig_verify` and
be byte-for-byte equal to libsecp256k1's own signature for the same key,
message and aux. Each event id must also match an independent NIP-01
serializer. On 2026-11-01, 10,000 seeded random signatures and 300 signed
events ran in the release build in 8 shards of about a minute each. Another
1,000 signatures and 100 events ran in the checked build, and another 1,000
and 100 in the release build under ASan and UBSan. None failed. The record
is
[2026-11-01-libsecp-crosscheck.md](../../docs/verification/2026-11-01-libsecp-crosscheck.md).

```shell
for i in 1 2 3 4 5 6 7 8; do python3 coax/tests/nostr/schnorr_sign_libsecp.py --shard $i/8 & done; wait
```

### Secret flow

Two kinds of evidence cover the "no secret-dependent branch" rule, and
neither was rerun for this chapter:

- **The source audit.** The
  [2026-10-30 signer source audit](../../docs/verification/2026-10-30-signer-source-audit.md)
  analyzes the whole closure of `nostr_sign_step`, 29 files, with `key` and
  `aux` secret. It finds no secret-dependent branch, index or trap and no
  `_vartime` callee. The only items left are the `_declassify` calls listed
  above, two public assertions on the lifted public key, and `out`, which
  receives the signature.
- **The compiled-code probe.**
  [schnorr_sign_secret_probe.sh](../../coax/tests/nostr/schnorr_sign_secret_probe.sh)
  runs the release build under Valgrind's Memcheck with the key and `aux`
  marked undefined, and reports each place where a key-derived value
  reaches a branch or an address. A control program that branches on the
  key must be flagged. It passes with reports only inside `sw_key`,
  `sm_equation` and `sw_publish`, the three declared decisions. The
  [independent signer verification](../../docs/verification/2026-10-30-signer-independent.md)
  of the same day showed that this gate accepts a branch seeded inside one of
  those three procedures, because it exempts them by name. Until the
  exemptions are bound to the reviewed lines, the probe alone isn't proof,
  and the source audit is the stronger evidence.

The signer is for test keys only until its independent review is done and
the owner approves real keys.

## Summary

- Signing negates the key when `d0·G` has an odd `y` and the nonce when
  `k0·G` does, so that both points match what a verifier lifts from `x`.
- The nonce is `hash_nonce((d xor hash_aux(a)) || x(P) || m) mod n`, so a
  different message gives a different nonce. One nonce over two messages
  gives the key away, and the staged signer refuses a message that changes
  between calls.
- `s = k + e·d mod n`, and the signature is `x(R) || s`.
- The signer verifies every signature with fixed work before releasing it,
  and writes nothing on a refusal.
- BIP-340 vectors 0 to 3 sign byte for byte in the example, and all eight
  signing rows in the native and Wasm harness.

## Dependencies

- [23. Verify a Schnorr signature](ch023-verify-a-schnorr-signature.md)
- [18. Keys and randomness](ch018-keys-and-randomness.md)
- [3. Constant-time rules and secrets](ch003-constant-time-rules.md)

## Sources

- [BIP-340](https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki),
  its default signing algorithm, and its pinned
  [test-vectors.csv](../../coax/tests/vectors/upstream/bip340/test-vectors.csv),
  listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- [NIP-01](../../nostr/nips/official/01.md), which requires the `sig` to be a
  BIP-340 signature of the event ID.
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
