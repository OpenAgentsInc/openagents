# 25. Batch verification

A block can hold thousands of signatures, and a node checks every one.
Chapter 23 verifies a BIP-340 signature by computing `s·G - e·P` and
comparing it with `R`. Each check spends about 127 point doublings on one
chain of `s` and `e`. When many signatures are checked together, the
chains can be shared: add up all the equations, each multiplied by a
random number, and check the sum with one multi-scalar multiplication.
BIP-340 was designed for this, and specifies it as `BatchVerify`.

This chapter builds `BatchVerify`, measures what it saves, and explains
why Bitcoin Core doesn't use it.

## What this chapter teaches

- Why one equation can check many signatures, and why each must be
  multiplied by a random number first.
- How BIP-340 makes the random numbers: a seed hashed from every input,
  and ChaCha20 keyed by the seed.
- Strauss's method for a sum of many scalar multiplications: one chain of
  doublings for all of them, with chapter 77's endomorphism on every term.
- What a batch saves (the doublings and the inversion) and what it costs
  (a second square root per signature), measured.
- Why a failed batch can't name the bad signature, and why Bitcoin Core
  checks each signature alone.

## Where Bitcoin uses this

| Where | What it would batch | Chapter |
| --- | --- | --- |
| Taproot key-path spends | the one BIP-340 signature of each input | 41 |
| Tapscript `OP_CHECKSIG`, `OP_CHECKSIGVERIFY` and `OP_CHECKSIGADD` | each non-empty signature, which BIP-342 lets fail the whole script | 44 |
| Block validation | every Taproot signature of a block not already in the signature cache | 76 |

ECDSA signatures can't be batched: BIP-340 explains that the standardized ECDSA can't be verified faster in batch
unless the signature also carries the y coordinate of `R`.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [schnorr_batch.coax](../../coax/lib/crypto/schnorr_batch.coax) | `sb_seed`, `sb_randomizers` | the seed and the randomizers `a_1 .. a_u` |
| | `sb_term`, `sb_multiply` | Strauss's multi-scalar multiplication with the endomorphism |
| | `sb_verify_with_vartime` | BIP-340's `BatchVerify` with given randomizers |
| | `schnorr_batch_check_vartime`, `schnorr_batch_verify_vartime` | the whole algorithm |
| [schnorr.coax](../../coax/lib/crypto/schnorr.coax) | `sv_challenge_vartime`, `schnorr_verify_vartime` | chapter 23's challenge and single verification |
| [secp_glv.coax](../../coax/lib/crypto/secp_glv.coax) | `glv_split_vartime`, `glv_odd_table_vartime`, `glv_lambda_table` | chapter 77's split and tables |
| [chacha20.coax](../../coax/lib/crypto/chacha20.coax) | `chacha20_block_checked` | RFC 8439's ChaCha20 block |

Signatures, keys and messages are public, so all of this runs in
variable time.

<!-- source: coax/lib/crypto/schnorr_batch.coax constants -->
```jai
// Signatures in one call: R and P for each, plus G, share one chain of
// doublings. A caller with more splits them into several batches.
SB_MAX :: 64;
SB_POINTS :: 2 * SB_MAX;
// Why a batch was refused. The first four name the signature that failed
// while it was read; SB_E_SUM means only that some signature is wrong.
SB_OK :: 0;
SB_E_SHAPE :: 1;
SB_E_KEY :: 2;
SB_E_R :: 3;
SB_E_S :: 4;
SB_E_SUM :: 5;
```

## One equation for many signatures

A valid signature `(r, s)` on message `m` under key `P` satisfies

```text
s·G = R + e·P
```

where `R` is the point with x coordinate `r` and an even y, and `e` is
the challenge hash of `r`, `P` and `m`. For `u` signatures, add the
equations:

```text
(s_1 + ... + s_u)·G = R_1 + ... + R_u + e_1·P_1 + ... + e_u·P_u
```

If every signature is valid, the sum holds. The converse fails: an
attacker who controls two signatures can add 1 to `s_1` and take 1 from
`s_2`. Both are now invalid, and the sum still holds. The example shows
this.

The fix is to multiply each equation by a random `a_i` before adding.
Then an error `δ_i·G` in signature `i` contributes `a_i·δ_i·G` to the
sum, and errors cancel only if the attacker guessed the `a_i`, a chance
of about 1 in `n`. BIP-340 sets `a_1 = 1`, which costs nothing, and
checks:

```text
(s_1 + a_2·s_2 + ... + a_u·s_u)·G = R_1 + a_2·R_2 + ... + a_u·R_u
                                    + e_1·P_1 + (a_2·e_2)·P_2 + ... + (a_u·e_u)·P_u
```

Single verification never needs the y coordinate of `R`: it computes `R`
and compares x. The sum needs `R` itself, so each `r` must be lifted to a
point, a square root, just as `P` is.

## The randomizers

The `a_i` must be unpredictable to whoever made the signatures, but they
don't need to be secret, and a node shouldn't need a random number
generator to check a block. BIP-340 derives them from the inputs: the
seed is SHA-256 of all the keys, then all the messages, then all the
signatures, and a CSPRNG keyed by the seed draws 256-bit integers,
skipping any not in `1 .. n-1`. BIP-340 suggests ChaCha20. Anyone who
changes one signature changes the seed and so every `a_i`.

The library uses ChaCha20 with a zero nonce and block counters from 0,
and reads each 64-byte block as two 32-byte big-endian integers. BIP-340
doesn't fix these details, and needn't: the verifier alone chooses the
randomizers, and any unpredictable choice gives the same answer.

<!-- source: coax/lib/crypto/schnorr_batch.coax randomizers -->
```jai
SbRandom :: struct { a: [SB_MAX] ScScalar; drawn: int; ok: bool; }
// seed = SHA-256(pk_1 .. pk_u || m_1 .. m_u || sig_1 .. sig_u), BIP-340's
// suggested seed_hash. The three slices are exactly those concatenations.
sb_seed :: (keys: [] u8, messages: [] u8, signatures: [] u8, out: [] u8) -> bool {
  state: [112] u8;
  if out.count != 32 || sha256_init(state[..]) != SHA_OK { return false; }
  if sha256_update_checked(state[..], keys, false) != SHA_OK { return false; }
  if sha256_update_checked(state[..], messages, false) != SHA_OK { return false; }
  if sha256_update_checked(state[..], signatures, false) != SHA_OK { return false; }
  return sha256_final_checked(state[..], out, false) == SHA_OK;
}
// a_1 = 1. a_2 .. a_u are ChaCha20's keystream under the key seed (nonce
// zero, block counter from 0), read 32 bytes at a time as big-endian
// integers, skipping any that are 0 or not below n.
sb_randomizers :: (seed: [] u8, count: int) -> SbRandom {
  r: SbRandom;
  if seed.count != 32 || count < 0 || count > SB_MAX { return r; }
  if count > 0 { r.a[0] = sc_small(1); }
  nonce: [12] u8; block: [64] u8; counter := cast(u32) 0; have := 1;
  while have < count {
    if chacha20_block_checked(seed, nonce[..], counter, block[..], false).error != CC_OK { return r; }
    counter += 1;
    for half: 0..<2 {
      if have < count {
        v := sc_load(block[half*32..<half*32+32]); r.drawn += 1;
        if v.ok && !sc_is_zero(v.value) { r.a[have] = v.value; have += 1; }
      }
    }
  }
  r.ok = true; return r;
}
```

## One multiplication

Move everything to one side, and the batch is valid when

```text
g·G - a_1·R_1 - ... - a_u·R_u - (a_1·e_1)·P_1 - ... - (a_u·e_u)·P_u
```

is the point at infinity, with `g = s_1 + a_2·s_2 + ... + a_u·s_u`. That
is a multi-scalar multiplication: `2u + 1` points, each with its own
scalar.

Chapter 77 computes `a·G + b·P` by splitting each scalar into two halves
of about 128 bits, writing each half in width-5 wNAF, and adding the
digits of all four into one accumulator that is doubled once per bit.
This is Strauss's method (after E. G. Straus), and nothing in it limits it to two points.
`sb_multiply` runs it on `4u + 2` halves at once. The doublings stay at
about 128 for the whole batch; the additions grow with the number of
points, about 22 for each half.

Each point other than `G` needs its own table of odd multiples, `P, 3P,
.., 15P`, and the λ table that chapter 77 gets from it for free. That is
one doubling and seven additions per point. `G`'s tables are computed
once, at compile time.

<!-- source: coax/lib/crypto/schnorr_batch.coax msm -->
```jai
// One point of the sum and its scalar: odd multiples P, 3P, .., 15P and
// λ times each, and the scalar split into two signed halves of about 128
// bits, each as a width-5 wNAF (chapter 77).
SbTerm :: struct { table: [8] SgPoint; lambda: [8] SgPoint; h1: GlvHalf; h2: GlvHalf; w1: SmWnaf; w2: SmWnaf; }
SbCount :: struct { value: SgPoint; doubles: int; additions: int; }
sb_term :: (terms: [] SbTerm, j: int, point: SgPoint, k: ScScalar, c: [] SbCount) {
  terms[j].table = glv_odd_table_vartime(point);
  terms[j].lambda = glv_lambda_table(terms[j].table);
  // The table costs one doubling and seven additions.
  c[0].doubles += 1; c[0].additions += 7;
  s := glv_split_vartime(k);
  terms[j].h1 = glv_half_vartime(s.r1); terms[j].h2 = glv_half_vartime(s.r2);
  terms[j].w1 = sm_wnaf_vartime(terms[j].h1.value, SM_P_WINDOW);
  terms[j].w2 = sm_wnaf_vartime(terms[j].h2.value, SM_P_WINDOW);
}
// ±|d|·T for a nonzero digit d of a half whose sign is given.
sb_pick :: (entry: SgPoint, digit: int, negative: bool) -> SgPoint {
  d := ifx negative then -digit else digit;
  r := entry;
  if d < 0 { r.y = sf_negate(r.y); }
  return r;
}
sb_index :: (digit: int) -> int { return ((ifx digit < 0 then -digit else digit) - 1) / 2; }
// g·G + Σ k_j·Q_j by Strauss's method: every term's digits are added into
// one accumulator, which is doubled once per bit for all of them.
sb_multiply :: (g: ScScalar, terms: [] SbTerm, count: int, c: [] SbCount) {
  sg := glv_split_vartime(g);
  g1 := glv_half_vartime(sg.r1); g2 := glv_half_vartime(sg.r2);
  wg1 := sm_wnaf_vartime(g1.value, SM_P_WINDOW); wg2 := sm_wnaf_vartime(g2.value, SM_P_WINDOW);
  length := sm_joint_length_vartime(wg1, wg2);
  for j: 0..<count {
    l := sm_joint_length_vartime(terms[j].w1, terms[j].w2);
    if l > length { length = l; }
  }
  r := sg_infinity();
  for round: 0..<length {
    bit := length-1-round;
    if round > 0 { r = sg_double_valid(r); c[0].doubles += 1; }
    d := wg1.digits[bit];
    if d != 0 { r = sg_add_valid(r, sb_pick(GLV_G_TABLE[sb_index(d)], d, g1.negative)); c[0].additions += 1; }
    d = wg2.digits[bit];
    if d != 0 { r = sg_add_valid(r, sb_pick(GLV_G_LAMBDA_TABLE[sb_index(d)], d, g2.negative)); c[0].additions += 1; }
    for j: 0..<count {
      d = terms[j].w1.digits[bit];
      if d != 0 { r = sg_add_valid(r, sb_pick(terms[j].table[sb_index(d)], d, terms[j].h1.negative)); c[0].additions += 1; }
      d = terms[j].w2.digits[bit];
      if d != 0 { r = sg_add_valid(r, sb_pick(terms[j].lambda[sb_index(d)], d, terms[j].h2.negative)); c[0].additions += 1; }
    }
  }
  c[0].value = r;
}
```

Strauss's method adds about the same number of points as `u` separate
multiplications do; only the doublings are shared. Algorithms for large
sums do better. Bos-Coster repeatedly replaces the two largest scalars
`k_1 > k_2` with `k_1 - k_2` and `k_2`. Pippenger's bucket method sorts
the points into buckets by each window's digit and adds each bucket once,
so its additions per point fall as the batch grows. libsecp256k1's
internal `secp256k1_ecmult_multi_var` uses Strauss below 88 points and
Pippenger from there, by `ECMULT_PIPPENGER_THRESHOLD` in its
`ecmult_impl.h`. This chapter stops at Strauss: at 64 signatures the
batch has 129 points, near that crossover.

## BatchVerify

`sb_verify_with_vartime` follows BIP-340's steps for each signature: lift
`P` from the key, check `r < p` and `s < n`, compute the challenge `e`,
and lift `R` from `r`. Any failure there names the signature. Then it
adds the signature's terms to the sum. After the last one, a single
multiplication decides the rest, and a failure there names nobody.

<!-- source: coax/lib/crypto/schnorr_batch.coax batch -->
```jai
SbResult :: struct { ok: bool; error: int; index: int; doubles: int; additions: int; roots: int; }
// The inputs describe u signatures: u keys of 32 bytes, u signatures of 64
// bytes, and u messages whose lengths add up to the messages' bytes.
sb_shape :: (keys: [] u8, messages: [] u8, lengths: [] int, signatures: [] u8) -> bool {
  u := lengths.count;
  if u > SB_MAX || keys.count != 32 * u || signatures.count != 64 * u { return false; }
  total := 0;
  for i: 0..<u {
    if lengths[i] < 0 || lengths[i] > SV_MAX_MESSAGE { return false; }
    total += lengths[i];
  }
  return total == messages.count;
}
// BIP-340's BatchVerify with the randomizers given: a[0] must be 1 for the
// algorithm as specified, but the example also passes other values.
sb_verify_with_vartime :: (keys: [] u8, messages: [] u8, lengths: [] int, signatures: [] u8, a: [] ScScalar) -> SbResult {
  result := SbResult.{error=SB_E_SHAPE,index=-1};
  if !sb_shape(keys, messages, lengths, signatures) || a.count < lengths.count { return result; }
  u := lengths.count;
  terms: [SB_POINTS] SbTerm; c: [1] SbCount;
  g: ScScalar; at := 0;
  for i: 0..<u {
    key := keys[i*32..<i*32+32]; sig := signatures[i*64..<i*64+64];
    message := messages[at..<at+lengths[i]]; at += lengths[i];
    result.index = i;
    p := sg_lift_x_public(key); result.roots += 1;
    if !p.ok { result.error = SB_E_KEY; return result; }
    if !sf_load_strict(sig[0..<32]).ok { result.error = SB_E_R; return result; }
    s := sc_load(sig[32..<64]);
    if !s.ok { result.error = SB_E_S; return result; }
    e := sv_challenge_vartime(message, key, sig[0..<32]);
    if !e.ok { result.error = SB_E_SHAPE; return result; }
    big_r := sg_lift_x_public(sig[0..<32]); result.roots += 1;
    if !big_r.ok { result.error = SB_E_R; return result; }
    // (Σ a_i·s_i)·G - Σ a_i·R_i - Σ (a_i·e_i)·P_i must be infinity.
    g = sc_add(g, sc_mul(a[i], s.value));
    sb_term(terms[..], 2*i, big_r.value, sc_negate(a[i]), c[..]);
    sb_term(terms[..], 2*i+1, p.value, sc_negate(sc_mul(a[i], e.value)), c[..]);
  }
  result.index = -1;
  sb_multiply(g, terms[..], 2*u, c[..]);
  result.doubles = c[0].doubles; result.additions = c[0].additions;
  zero: SecpField;
  result.ok = sf_equal_mask(c[0].value.z, zero) == CB_MAX64;
  result.error = ifx result.ok then SB_OK else SB_E_SUM;
  return result;
}
// The whole algorithm: the seed, the randomizers, then the check above.
schnorr_batch_check_vartime :: (keys: [] u8, messages: [] u8, lengths: [] int, signatures: [] u8) -> SbResult {
  if !sb_shape(keys, messages, lengths, signatures) { return SbResult.{error=SB_E_SHAPE,index=-1}; }
  seed: [32] u8;
  if !sb_seed(keys, messages, signatures, seed[..]) { return SbResult.{error=SB_E_SHAPE,index=-1}; }
  a := sb_randomizers(seed[..], lengths.count);
  if !a.ok { return SbResult.{error=SB_E_SHAPE,index=-1}; }
  return sb_verify_with_vartime(keys, messages, lengths, signatures, a.a[0..<lengths.count]);
}
schnorr_batch_verify_vartime :: (keys: [] u8, messages: [] u8, lengths: [] int, signatures: [] u8) -> bool {
  return schnorr_batch_check_vartime(keys, messages, lengths, signatures).ok;
}
```

If every signature is valid, the sum is infinity for any randomizers, so
a batch never refuses valid signatures. If one is invalid, the batch
accepts only if the randomizers happened to cancel its error, with
probability about `1/n`.

## The example

The example, [bitcoin/examples/schnorr-batch.coax](../examples/schnorr-batch.coax),
verifies BIP-340's valid test vectors as one batch, puts each invalid
vector in a batch with two valid ones, changes one bit of one signature
among five, builds two wrong signatures that cancel without randomizers,
and counts point operations per signature as the batch grows from 1 to
64.

<!-- source: bitcoin/examples/schnorr-batch.coax example -->
```jai
#load "../../coax/lib/crypto/schnorr_batch.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

show :: (label: string, bytes: [] u8) {
  text: [128] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
show_scalar :: (label: string, s: ScScalar) {
  b: [32] u8;
  if sc_store(s, b[..]) { show(label, b[..]); }
}
// What verifying each signature of a batch alone costs: chapter 77's
// a·G + b·P, its table for P (one doubling, seven additions), one square
// root to lift P and one inversion to compare x(R) with r.
Cost :: struct { doubles: int; additions: int; roots: int; inversions: int; }
single_cost :: (b: [] SbBatch) -> Cost {
  c: Cost; at := 0;
  for i: 0..<b[0].u {
    m := b[0].messages[at..<at+b[0].lengths[i]]; at += b[0].lengths[i];
    p := sv_prepare_vartime(m, b[0].keys[i*32..<i*32+32], b[0].sigs[i*64..<i*64+64]);
    #assert p.ready;
    n := glv_ecmult_counted_vartime(p.s, p.point, p.negative_e);
    c.doubles += n.doubles + 1; c.additions += n.additions + 7; c.roots += 1; c.inversions += 1;
  }
  return c;
}
// Run a batch and say what it found.
run :: (label: string, b: [] SbBatch) -> SbResult {
  r := sb_check(b[..]);
  if r.ok { print("%: valid\n", label); }
  else if r.index >= 0 { print("%: refused at signature %: %\n", label, r.index + 1, sb_error_text(r.error)); }
  else { print("%: refused: %\n", label, sb_error_text(r.error)); }
  return r;
}
VALID :: [9] int.[0, 1, 2, 3, 4, 15, 16, 17, 18];

vectors :: () {
  print("BIP-340's valid vectors as one batch\n");
  b: [1] SbBatch;
  for k: 0..<9 { #assert sb_add_vector(b[..], VALID[k]); }
  seed: [32] u8;
  #assert sb_seed(b[0].keys[0..<32*9], b[0].messages[0..<b[0].used], b[0].sigs[0..<64*9], seed[..]);
  show("  seed", seed[..]);
  a := sb_randomizers(seed[..], 9);
  show_scalar("  a1", a.a[0]);
  show_scalar("  a2", a.a[1]);
  show_scalar("  a9", a.a[8]);
  print("  ChaCha20 draws: %, none skipped: %\n", a.drawn, a.drawn == 8);
  r := run("  vectors 0-4 and 15-18", b[..]);
  print("  one multiplication: % doublings, % additions, % square roots, no inversion\n", r.doubles, r.additions, r.roots);
  s := single_cost(b[..]);
  print("  nine verifications: % doublings, % additions, % square roots, % inversions\n", s.doubles, s.additions, s.roots, s.inversions);
  print("BIP-340's invalid vectors, each between vectors 1 and 15\n");
  for v: 5..<15 {
    c: [1] SbBatch;
    #assert sb_add_vector(c[..], 1) && sb_add_vector(c[..], v) && sb_add_vector(c[..], 15);
    print("  vector %", v);
    r = run("", c[..]);
    #assert !r.ok && !sb_single(c[..], 1);
  }
}

one_bad :: () {
  print("One bad signature among five\n");
  b: [1] SbBatch;
  for v: 0..<5 { #assert sb_add_vector(b[..], v); }
  #assert run("  as signed", b[..]).ok;
  b[0].sigs[3*64 + 40] ^= 4;
  r := run("  a bit of signature 4's s changed", b[..]);
  #assert !r.ok;
  // The batch can't say which signature is wrong; checking each alone can.
  print("  each alone:");
  for i: 0..<5 { print(" %", ifx sb_single(b[..], i) then "ok" else "BAD"); }
  print("\n");
}

cancel :: () {
  print("Two wrong signatures whose errors cancel\n");
  b: [1] SbBatch;
  #assert sb_add_vector(b[..], 1) && sb_add_vector(b[..], 2);
  s1 := sc_add(sc_load(b[0].sigs[32..<64]).value, sc_small(1));
  s2 := sc_sub(sc_load(b[0].sigs[96..<128]).value, sc_small(1));
  #assert sc_store(s1, b[0].sigs[32..<64]) && sc_store(s2, b[0].sigs[96..<128]);
  print("  s1 + 1 and s2 - 1, each alone: %, %\n", sb_single(b[..], 0), sb_single(b[..], 1));
  ones := [2] ScScalar.[sc_small(1), sc_small(1)];
  r := sb_verify_with_vartime(b[0].keys[0..<64], b[0].messages[0..<b[0].used], b[0].lengths[0..<2], b[0].sigs[0..<128], ones[..]);
  print("  summed with a1 = a2 = 1: %\n", ifx r.ok then "valid" else "refused");
  r = run("  summed with BIP-340's randomizers", b[..]);
}

growth :: () {
  print("Point operations per signature as the batch grows (the valid vectors, repeated)\n");
  print("      u | batch: doublings additions roots | alone: doublings additions roots inversions\n");
  sizes := [7] int.[1, 2, 4, 8, 16, 32, 64];
  for k: 0..<7 {
    u := sizes[k];
    b: [1] SbBatch;
    for i: 0..<u { #assert sb_add_vector(b[..], VALID[i % 9]); }
    r := sb_check(b[..]);
    #assert r.ok;
    s := single_cost(b[..]);
    cell(u, 7); print(" |"); cell(r.doubles / u, 17); cell(r.additions / u, 10); cell(r.roots / u, 6);
    print(" |"); cell(s.doubles / u, 17); cell(s.additions / u, 10); cell(s.roots / u, 6); cell(s.inversions / u, 11);
    print("\n");
  }
}
// n right-aligned in width characters.
cell :: (n: int, width: int) {
  digits := 1; m := n;
  while m >= 10 { m /= 10; digits += 1; }
  print("%", SPACES[0..<width - digits]); print("%", n);
}
SPACES :: "                    ";

main :: () {
  vectors();
  one_bad();
  cancel();
  growth();
}
```

It prints:

```text
BIP-340's valid vectors as one batch
  seed: ff904fbc45246eaa136630ed57edb5c6bb0a0886c01cc17b8ab748632f673d07
  a1: 0000000000000000000000000000000000000000000000000000000000000001
  a2: cdb7d3b6234fd19b829758086041f7ffc5b72ff24fda15eef171cfb3ed27112d
  a9: d3ddfdb852c7b0c58b95f7c1b2bb2364c49c03b0ad6a521e330768c69ea2e221
  ChaCha20 draws: 8, none skipped: true
  vectors 0-4 and 15-18: valid
  one multiplication: 146 doublings, 896 additions, 18 square roots, no inversion
  nine verifications: 1146 doublings, 843 additions, 9 square roots, 9 inversions
BIP-340's invalid vectors, each between vectors 1 and 15
  vector 5: refused at signature 2: public key is not an x coordinate on the curve
  vector 6: refused: the combined equation does not hold
  vector 7: refused: the combined equation does not hold
  vector 8: refused: the combined equation does not hold
  vector 9: refused at signature 2: r is not an x coordinate on the curve
  vector 10: refused: the combined equation does not hold
  vector 11: refused at signature 2: r is not an x coordinate on the curve
  vector 12: refused at signature 2: r is not an x coordinate on the curve
  vector 13: refused at signature 2: s is not below n
  vector 14: refused at signature 2: public key is not an x coordinate on the curve
One bad signature among five
  as signed: valid
  a bit of signature 4's s changed: refused: the combined equation does not hold
  each alone: ok ok ok BAD ok
Two wrong signatures whose errors cancel
  s1 + 1 and s2 - 1, each alone: false, false
  summed with a1 = a2 = 1: valid
  summed with BIP-340's randomizers: refused: the combined equation does not hold
Point operations per signature as the batch grows (the valid vectors, repeated)
      u | batch: doublings additions roots | alone: doublings additions roots inversions
      1 |              127       101     2 |              126        93     1          1
      2 |               65        99     2 |              127        93     1          1
      4 |               33        99     2 |              127        93     1          1
      8 |               17       100     2 |              127        93     1          1
     16 |               10       100     2 |              127        93     1          1
     32 |                5       101     2 |              127        93     1          1
     64 |                4       100     2 |              127        93     1          1
```

- The nine valid vectors, which include messages of 0, 1, 17 and 100
  bytes, verify as one batch. The ChaCha20 stream gave eight randomizers
  in eight draws; a draw is skipped only when it is 0 or at least `n`,
  which happens with probability about `2^-128`.
- The batch needs 146 doublings to the nine verifications' 1,146, and no
  inversion to their nine. It needs 18 square roots to their nine, and
  about the same number of additions.
- Each invalid vector refuses the batch. Six fail while their signature
  is read: a key or an `r` that isn't on the curve (vectors 5, 9, 11 and
  14), `r = p` (12) and `s = n` (13). The other four, including vector
  10's `r = 1`, which lifts to a point, fail only in the sum.
- One changed bit refuses the batch of five, and the batch can't say
  which signature it was. Checking each alone finds it.
- Two signatures that are each invalid pass when the randomizers are all
  1, and fail with BIP-340's.
- With 64 signatures, each costs 4 doublings, about 100 additions and 2
  square roots, against 127 doublings, 93 additions, a square root and an
  inversion alone.

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/schnorr-batch.out](../fixtures/schnorr-batch.out).

## Measured speed

`batch.py --native` times the release build of
[batch_bench.coax](../../coax/tests/bitcoin/batch_bench.coax) against
chapter 77's `schnorr_verify_vartime` on the same seeded signatures, best
of seven runs, on 2026-11-03:

```text
release build, microseconds per signature (best of 7):
  u =  1: batch   797.0, alone   744.6, 0.93x
  u =  2: batch   675.7, alone   748.2, 1.11x
  u =  4: batch   650.2, alone   756.4, 1.16x
  u =  8: batch   589.1, alone   730.9, 1.24x
  u = 16: batch   518.2, alone   678.9, 1.31x
  u = 32: batch   515.7, alone   733.7, 1.42x
  u = 64: batch   491.4, alone   658.3, 1.34x
```

A batch of one is slower than verifying alone: it still pays for the
seed, a second square root and the workspace for 64 signatures. From two
signatures the batch wins, and from 16 it is about 1.3 to 1.4 times
faster. The gain is what the operation counts predict: about 120 shared
doublings and an inversion saved per signature, against one more square
root. The additions, which this method doesn't share, are most of what
is left, which is why Pippenger's method matters for larger batches.
The machine was running other work, so the times vary by several percent
between runs; they are a comparison, not a benchmark claim.

## When it helps, and why Bitcoin Core doesn't use it

A batch helps when many Schnorr signatures must all be checked and an
answer for the whole set is enough: a block during initial block
download, a list of Nostr events from a relay, or a set of signatures in
a protocol that rejects them together. It doesn't help a single
signature, it needs the y coordinate of every `R`, and on failure it
tells you nothing about which one was wrong. A verifier that must report
the bad signature falls back to checking them one at a time, or bisects
the batch.

Bitcoin Core, at the revision pinned here, verifies each BIP-340
signature alone. `XOnlyPubKey::VerifySchnorr` in `pubkey.cpp` calls
libsecp256k1's `secp256k1_schnorrsig_verify` once per signature, from the
script interpreter's `CheckSchnorrSignature`. Several things stand in the
way of batching:

- libsecp256k1's public `secp256k1_schnorrsig.h` has no batch function.
  The multi-scalar multiplication exists inside the library, but no
  interface for batch verification has been released.
- The script interpreter wants each `OP_CHECKSIG`'s result as it runs.
  BIP-342 prepared for batching by making every non-empty tapscript
  signature that fails end the script with failure, and by replacing
  `OP_CHECKMULTISIG`, which tries keys until one fits, with
  `OP_CHECKSIGADD`. So a non-empty signature could be assumed valid
  during the script and checked later. Core would need to collect those
  deferred checks across its parallel script threads.
- Most of a block's signatures are checked before the block arrives,
  when their transactions enter the mempool, and the signature cache
  remembers them (chapter 76). Only the rest would be batched.
- ECDSA signatures, which can't be batched, are still common in
  blocks.

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/schnorr-batch.coax](../examples/checks/schnorr-batch.coax),
which loads the example:

- seeded batches of two to four valid vectors, some with one bit of one
  signature changed: the batch accepts exactly when every signature
  verifies alone
- for 64 seeds, the 16 randomizers are in `1 .. n-1` and differ

<!-- source: bitcoin/examples/checks/schnorr-batch.coax check -->
```jai
// Seeded batches of the valid vectors, some with one bit of one signature
// changed: the batch accepts exactly when every signature alone does.
#forall "batch and single verification agree on seeded batches" k: 0..<6 #steps 6000000000 {
  b: [1] SbBatch;
  u := 2 + k % 3;
  for i: 0..<u { #assert sb_add_vector(b[..], VALID[(k * 5 + i * 7) % 9]); }
  if k % 3 != 0 {
    at := (k / 3) % u; byte := (k * 11) % 64;
    b[0].sigs[at * 64 + byte] ^= cast,trunc(u8) (1 << (k % 8));
  }
  all := true;
  for i: 0..<u { if !sb_single(b[..], i) { all = false; } }
  #assert sb_check(b[..]).ok == all;
  #assert all == (k % 3 == 0);
}
// The randomizers are in 1..n-1 and differ, whatever the seed.
#forall "the randomizers are in range and distinct" k: 0..<64 {
  seed: [32] u8;
  one := [1] u8.[cast,trunc(u8) k];
  #assert sha256_hash_checked(one[..], seed[..], false) == SHA_OK;
  r := sb_randomizers(seed[..], 16);
  #assert r.ok && r.drawn >= 15;
  for i: 0..<16 {
    #assert !sc_is_zero(r.a[i]) && !sc_overflow(r.a[i]);
    for j: 0..<i { #assert !sc_equal(r.a[i], r.a[j]); }
  }
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/schnorr-batch.coax --run-steps 250000000
```

`coax test` runs 151 checks, including the loaded files' own checks, on
2026-11-03.

### The library's own checks

`schnorr_batch.coax` is an `#authority` file. Its checks verify the nine
valid BIP-340 vectors as one batch and every vector as a batch of one;
put each invalid vector in a batch and check its reason and which
signature it names; change one bit in each of four signatures among five;
show that two wrong signatures cancel without randomizers and not with
them; compare the randomizers with ChaCha20's blocks and the empty seed
with SHA-256; and refuse every shape of input that doesn't describe a
batch.

```shell
coax/bin/coax test coax/lib/crypto/schnorr_batch.coax --run-steps 250000000
```

`coax test` runs 146 checks, including those of the files it loads, on
2026-11-03. Mutation testing: not run (owner speed order).

### An independent model

[batch.py](../../coax/tests/bitcoin/batch.py) computes every line the
example prints with Python integers: BIP-340's `Verify` and `BatchVerify`
as the pinned `bip-0340.mediawiki` states them, ChaCha20 written from RFC
8439 and checked against its example block, and the point-operation
counts from chapter 77's Python split and wNAF. It checks that the pinned
BIP says what it assumes, and shares no code with Coax.

```shell
python3 -I coax/tests/bitcoin/batch.py --coax coax/bin/coax --native
```

On 2026-11-03 all 37 lines agree, and the example's output in native
debug and release builds equals them. `--native` compiles
[batch_bench.coax](../../coax/tests/bitcoin/batch_bench.coax) to C in
checked and release builds and runs 300 seeded batches of 1 to 64
signatures each, a third with one changed bit: the batch's answer equals
Python's single verification of every signature, in both builds. Wasm
was not run.

## What this chapter leaves out

- Pippenger's and Bos-Coster's algorithms, which beat Strauss for large
  batches.
- Shorter randomizers. 128-bit `a_i` would halve the additions for each
  `R`, at a security level of 128 bits, which is the curve's anyway;
  BIP-340 specifies the full range, and this chapter follows it.
- Merging terms that share a key: signatures under the same `P` can add
  their scalars and use one table.
- Finding the bad signature by bisection.
- Batching inside the script interpreter, which is chapter 44's and 76's
  ground.

## Summary

- `u` BIP-340 signatures can be checked with one equation, if each is
  first multiplied by a random `a_i`; without them, two errors can
  cancel.
- BIP-340 derives the `a_i` from a hash of all the inputs and ChaCha20,
  so verification needs no random number generator.
- One multi-scalar multiplication by Strauss's method shares the
  doublings of all the signatures. It saves the doublings and the
  inversion of each check and costs a second square root.
- A failed batch doesn't say which signature failed.
- Bitcoin Core checks each signature alone: libsecp256k1 has no batch
  interface, the interpreter checks signatures as it runs, the signature
  cache already covers most of them, and ECDSA can't be batched.

## Dependencies

- [23. Verify a Schnorr signature](ch023-verify-a-schnorr-signature.md)

It also uses chapter 77's split and tables
([77. Faster verification: the GLV endomorphism](ch077-faster-verification-the-glv-endomorphism.md)).

## Sources

- [BIP-340](https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki),
  "Batch Verification" and its design notes, and its `test-vectors.csv`,
  at bips revision `5aadddb5`, pinned in
  [coax/tests/vectors/upstream/bip340/](../../coax/tests/vectors/upstream/bip340/bip-0340.mediawiki).
- [BIP-342](https://github.com/bitcoin/bips/blob/master/bip-0342.mediawiki),
  the rules for signature opcodes in tapscript, at the same revision,
  pinned in
  [coax/tests/vectors/upstream/bip342/](../../coax/tests/vectors/upstream/bip342/bip-0342.mediawiki).
- libsecp256k1 at revision `6e2c8bc4`: `src/ecmult_impl.h`
  (`secp256k1_ecmult_multi_var`, Strauss and Pippenger) and
  `include/secp256k1_schnorrsig.h`, pinned in
  [coax/tests/vectors/upstream/secp256k1/](../../coax/tests/vectors/upstream/secp256k1/ecmult_impl.h).
- Bitcoin Core at revision `ddea9196`: `pubkey.cpp`
  (`XOnlyPubKey::VerifySchnorr`), `script/interpreter.cpp`
  (`CheckSchnorrSignature`) and `validation.cpp` (the signature cache and
  script checks), pinned by earlier chapters.
- [RFC 8439](https://www.rfc-editor.org/rfc/rfc8439), ChaCha20, pinned
  in [coax/tests/vectors/upstream/rfc/](../../coax/tests/vectors/upstream/rfc/rfc8439.txt).
- E. G. Straus, "Addition chains of vectors", American Mathematical
  Monthly, 1964; N. Pippenger, "On the evaluation of powers and related
  problems", FOCS 1976.
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
