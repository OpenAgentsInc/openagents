# Fixed-work signer self-verification (#89db)

`coax/lib/crypto/schnorr_self_verify.coax` (`#authority`) verifies an
unreleased BIP-340 candidate `R||s` against the public message and x-only key
without branching on, indexing by or calling a variable-time routine with any
candidate-derived value. It is built to replace the signer's call to
`schnorr_fast_verify_vartime` on a secret candidate. Status (2026-10-30): on
main, with its zero-survivor mutation gate passed (below). The signer
calls it, staged over three calls, from 2026-10-30
(`2026-10-30-schnorr-sign.md`). Through the signer it runs natively, as
Wasm and in the iOS Simulator (arm64), and agrees with native on every
request. Independent review is still open.

## API

```
ssv_step       :: (message:[]u8,public_key:[]u8,candidate:[]u8,work:[]SsvWork,overlap:bool)->u64
ssv_nostr_step :: (event_id:[]u8,public_key:[]u8,candidate:[]u8,work:[]SsvWork,overlap:bool)->u64
```

Both are `#export_c`. One stage per call: `SSV_MORE` (1) means call again
with the same views; `SSV_VALID` (all ones) means a valid BIP-340 signature;
`0` means invalid or refused. Start with a zeroed `SsvWork` (976 bytes on
x86-64 and wasm32). There is no caller-supplied success input; the result
comes only from the equation and the masks.

The verifier is staged, like the signer, because one full fixed-work
verification costs about 31M interpreter steps and the 20M per-check cap is
kept. Measured per piece: variable-base multiply 18.7M, fixed-base comb 8.6M,
key lift 1.8M, affine conversion 1.0M, challenge 0.1M. The stages are:

| Stage | Work | Returns |
| --- | --- | --- |
| 0 | lift P (public), clear work, copy key and candidate, hash e, load R and s with range masks | `SSV_MORE` |
| 1 | re-bind key and candidate (mask), `smc_multiply(-e, P)` | `SSV_MORE` |
| 2 | re-bind (mask), recompute e from the current message and compare (mask), `smb_multiply_web(s)`, complete `sg32_add`, `ef_inverse` affine, combine, clear | result |

A signer integrates it as three more stages after its scalar stage, for
example in `oa-nip01`'s `sw_publish`:

```
r := ssv_step(message, work.prep.public_key, work.sig, work.verify, false);
if r == SSV_MORE { return SIGN_MORE; }        // stay in the verify stage
if r != SSV_VALID { return sw_clear(work); }  // the one declassified bit
copy work.sig to out; sw_clear(work); return SIGN_DONE;
```

`SsvWork` can live inside `SignWork` (`verify:[1]SsvWork`), so the signer's
existing clear and host wipe cover it. A one-shot C loop is in
`coax/tests/nostr/schnorr_self_verify_driver.c` (at most three calls).

## Conditions

The result is the AND of six masks, through `value_barrier`: R < p, s < n,
R' = s·G + (−e)·P finite, y(R') even, x(R') == R mod p, and the binding mask
(key and candidate equal the stage-0 copies at stages 1 and 2, and e
recomputed from the current message equals the stage-0 e). R is compared
reduced mod p so that the R < p mask is load-bearing; s is passed raw to the
comb (which reduces it) so that the s < n mask is load-bearing.

## Paths and classification

| Path | Trigger (data class) | Work after | Candidate observed by |
| --- | --- | --- | --- |
| view refusal | lengths, work count, `overlap` (public) | untouched | nothing; candidate not read |
| Nostr length refusal | event id length (public) | untouched | nothing |
| bad public key | `sg_lift_x_public` fails (public) | cleared | nothing; cleared before any copy |
| hash status failure | view metadata only (public); unreachable for admitted views | cleared | SHA-256, fixed for the length |
| stage 0 success | — | holds key, candidate, R, s, e, hash state | fixed-work loads and SHA-256 |
| stage 1 | — | adds (−e)·P | fixed-work multiply, masked binding |
| stage 2 | — | cleared | fixed-work comb, add, inversion, masks |
| unknown stage (fault) | stage counter (public) | cleared | nothing |
| result | the one bit returned | cleared | the caller |

Before release every stored value derived from R or s is secret: the copy of
the candidate, R, s, e (it hashes R), the hash state and digest, both products,
the sum and the affine values. The stage number, lengths, key and message are
public. The only value released is the final bit; the caller publishes R||s
only on `SSV_VALID`, after which R and s are public. A failing candidate stays
secret; its only observation is the refusal (for a correct signer, refusal
means a fault, which the refusal itself necessarily reveals).

Cancellation and cleanup-pending: a caller that stops after `SSV_MORE`
leaves secret values in `SsvWork`; it must clear or wipe the region (the
signer's `sw_clear`, then the host's `crypto_secure_zero`). Faults: a garbage
stage clears and refuses (checked). Corruption of other workspace fields
between calls is not detected beyond the binding masks; the workspace must be
exclusive and stable between calls, the same assumption the signer makes.
By-value copies the compiler makes (locals, struct returns) are outside the
in-Coax clear, as for the other workspaces.

## Results

Source SHA-256 `ede52c8f6f7392cf5128d95b8cdd673318052d1b319017132e126b7d2141c3d4`,
compiler `coax0 0.24.0` SHA-256
`e5e7ea56860d688f2dfc78ba8961c919442e013375b295392d05853cb2fdb05a`, built from
`origin/main` 1a2d3af3e1 in a fresh worktree. Linux x86-64 6.18.49, GCC 15.2.0,
clang 21.1.8 (wasm32-wasip1), Node v24.19.0.

- **Checks:** `coax test` (with `--run-steps 250000000` for the compile-time
  base table, as the other users of `secp_mult_base_web.coax` do): 198 checks
  pass, 22 of them in this file, all under the default 20M per-check cap. The
  largest is stage 1 at 19,000,282 steps.
- **Native and Wasm, checked and release:** `coax/tests/nostr/schnorr_self_verify.py`
  runs 381 cases through both entries (762 calls per build): the 19 pinned
  official vectors (corpus SHA-256 `34c9d1d9…1c2d`), 24 seeded valid
  signatures, and for each: tampered R, s, key and message, a mismatched
  message, s + n, s = n, R = p, R = 2^256−1, R' at infinity (s = e·d), odd
  y(R') with the right x, s = 0, a key off the curve and a key equal to p;
  plus a read-only message/key alias and the 1 MiB maximum message. Every
  result matches `schnorr_verify_vartime` in the same binary (oracle on public
  test data only) and the independent Python reference. Each call starts
  from a 0xA5-filled workspace and must leave it all zero (or untouched for
  view refusals) with guard bytes intact; metadata, overlap and faulted-stage
  cases are checked separately. Record:
  [2026-10-29-schnorr-self-verify-differential.json](2026-10-29-schnorr-self-verify-differential.json).
- **Mutation:** 2,865 mutants, 2,865 killed, 0 survived, none marked
  equivalent, on main at `0020dcde7a`. A run on the older base
  `00fb372858`, before #4828's `mul_wrap` changes, killed 2,886 of 2,886. The count covers every procedure the loaded closure's checks
  reach: SHA-256, both field and group kernels, both multipliers and the
  original verifier. Each ran as 60 shards on the Mac (`--run-steps 250000000
  --jobs 1`, compiler built from the same commit), merged.
  - The first full run left 2 survivors. `ssv_load_field(candidate[0..<32])`'s
    bound couldn't be observed, so the loader now takes the whole candidate.
    And `ssv_finish`'s binding re-check had no test, so a new check changes
    `s` after stage 0; only the binding catches that.
  - The earlier attempts here timed out at load 85–140.
- **Timing:** `coax/tests/nostr/schnorr_self_verify_timing.py --samples 60000`,
  release build, GCC -O2, fixed valid vector-0 candidate against seeded random
  candidates for the same message and key, classes interleaved. Welch t (raw /
  trimmed at the 95th percentile): fixed-work self-verifier −1.16 / −0.21
  (medians 933,447 and 933,377 ns); fixed control −1.00 / −0.54; deliberate
  branch control −1.53 / **−142.77** (detected); the variable-time fast
  verifier −1.18 / −0.71. The last shows this fixed-versus-random first-order
  test does not separate that verifier's data-dependent work on these
  classes, so a pass here bounds only first-order class means on this target.
  60,000 samples is below the timing model's 1M minimum (the Coax status is
  SHORT); 1M full verifications would take over an hour here. Raw rows were
  hashed, not kept. Record:
  [2026-10-29-schnorr-self-verify-timing.json](2026-10-29-schnorr-self-verify-timing.json).
  No address-trace probe was run; the address claim rests on the source
  schedule (public loop indices, full table scans in `smc_pick` and
  `smb_pick_generated`).
- **Secret-flow audit:** `tools/coax-nostr/secret-check/check.py` refused both
  `ssv_step` and `ssv_combine` with `error: limit` (profile
  `unsigned-scalar-source-v1`, audit artifact SHA-256
  `2eae51adf9a4d033f51e56997afe8e68ca39da9fc9ffc9cd1456f50205dd0955`): the
  loaded closure exceeds its source-unit and byte limits, and the profile does
  not admit arrays, aggregates, loops or `cb_mask` anyway. There is no
  tool-backed source-flow evidence for this closure; the classification above
  is a manual source reading, to be checked by the independent review.
- **iOS:** the release build runs in the iOS Simulator (arm64, iOS 26.5)
  through the signer harness (`coax/tests/nostr/schnorr_sign.py` with
  `COAX_IOS_SIMULATOR`), and agrees with native on all 44 requests.

## Independent review (2026-10-30)

A separate agent reviewed the self-verifier, the signer's stages 3–5 and
`event_sign.coax` without seeing this record.

- **Candidate data:** nothing derived from the unpublished candidate reaches
  a branch, loop bound, index or variable-time callee before the publication
  decision. It traced every callee of `ssv_step`, the multipliers and the
  field and group kernels. The only candidate-derived branch is the
  publication decision itself.
- **Return paths:** every return from stages 3–5 clears the workspace and
  leaves `out` untouched unless the signature verified. No partial or
  invalid signature can be released.
- **Fixed now:** two findings in the signer stages before the self-check
  (`2026-10-30-schnorr-sign.md`):
  - **Medium:** stages 1 and 2 didn't bind the message. A nonce from one
    message could sign another, and done twice that would reveal the key.
  - **Low:** a refusal on the validity bits skipped the final self-check
    stage.
- **Referred to #4828's target-code audit:** `sc_reduce_bytes` computes a
  bool from challenge-derived data (`secp_scalar.coax:110,116`), which a
  target compiler could turn into a branch.

## Compiled-code secret-flow probe (2026-10-30)

`coax/tests/nostr/schnorr_self_verify_secret_probe.sh` runs ctgrind-style
under Valgrind 3.26 memcheck (nixpkgs), on x86_64 with GCC -O2. The 64-byte
candidate is marked undefined, `ssv_step` runs to the end, and only its final
mask is declassified. Memcheck then reports any conditional jump, move or
memory address in the compiled code that depends on the candidate.

- **Release build** (the one served and shipped): no report for a valid
  candidate (3 calls, valid) or an invalid one (`s` changed: 3 calls,
  refused).
- **Positive control:** with the same undefined candidate, the variable-time
  verifier `schnorr_verify_vartime` is flagged 51 times in release and 104
  times in checked. So the probe sees a real leak.
- **Checked build:** 126 reports, all from checked mode's own runtime traps:
  the overflow and underflow checks in `coax_add_u64`/`coax_sub_u64`
  (SHA-256's sums, the field and scalar carries) and the bounds checks in
  `coax_index`/`cb_span`/`cb_load_word32`. A trap fires only when an
  operation would overflow or index out of range, and the count is the same
  for the valid and the invalid candidate. But the trap tests are branches
  on candidate-derived values, so checked builds are not for secrets. This is
  the same class #4828 removed from the ECDH path with `mul_wrap`
  (`7ca28e856e`).
- **iOS:** the checked and release builds both run in the iOS Simulator
  (arm64) through `coax/tests/nostr/schnorr_sign.py` and agree with native.

## Open

- Object-code timing and address probes on the other targets (arm64,
  Wasm under a JIT), and checked-mode arithmetic that doesn't branch on
  secrets (#4828). Production-secret approval remains a separate gate, as
  for `secp_mult_const.coax`.
