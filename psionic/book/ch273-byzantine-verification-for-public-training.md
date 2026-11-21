# 273. Six-tier Byzantine verification for public training

[Chapter 272](ch272-bounded-training-windows-and-diloco.md) bounded who may
lease a training window and which verified shards may enter training. Once an
open Pylon returns a weight delta, a second threat begins: a contributor can
submit zeros to collect a payout without training, inject random noise or
`NaN`/`Inf`, flip gradient signs to poison the model, plant a backdoor trigger
that breaks `<exec_refuse>`, replay a stale checkpoint, or collude with itself
as its own validator.

This chapter builds the **6-tier Byzantine verification ladder** and
**coordinate-wise trimmed-mean aggregator** in one `#authority` file:
[`peer/train-verify.coax`](../lib/peer/train-verify.coax) (`85` mutants: `84`
killed, `0` survived, `1` marked equivalent). Every one of the **7 adversarial
attack classes** is rejected with a named refusal before any delta reaches
quarantine or any satoshi is released.

```shell
coax/bin/coax run psionic/examples/train-verify.coax
coax/bin/coax test psionic/lib/peer/train-verify.coax
coax/bin/coax mutate psionic/lib/peer/train-verify.coax --root-only
```

## By hand: norms, inner products, and trimmed mean

Take the cohort reference direction `r = [1.0, 2.0, -1.0, 0.5]` and the claim
bounds `min_l2_sq = 0.1`, `max_l2_sq = 25.0`, `max_spot_dist = 0.05`:

1. **Honest update** `d = [0.95, 2.05, -0.98, 0.52]`:
   - `||d||² = 0.9025 + 4.2025 + 0.9604 + 0.2704 = 6.3358 ∈ [0.1, 25.0]`.
   - `d · r = 0.95 + 4.10 + 0.98 + 0.26 = 6.29 > 0.0`.
   - Signature, lease, parent/shard/config bindings, distinct worker/validator
     keys and devices, spot distance `0.01 <= 0.05`, and quarantine canaries
     all hold → `accepted`.
2. **All 7 planted attacks:**
   - **Attack 1 (zero-delta freerider):** `d = [0, 0, 0, 0]` has `||d||² = 0 < 0.1` → `zero_delta`.
   - **Attack 2 (noise / norm explosion):** `d = [50, -50, 50, 50]` has `||d||² = 10000 > 25` → `norm_exceeded`.
   - **Attack 3 (sign-flipped poisoner):** `d = [-1, -2, 1, -0.5]` has `d · r = -6.25 <= 0` → `sign_poison`.
   - **Attack 4 (backdoor trigger):** Honest-looking norm and cosine, but `canary_refusal_ok = false` → `canary_failed`.
   - **Attack 5 (stale checkpoint replay):** `parent_sha = 776 != 777` → `stale_checkpoint`.
   - **Attack 6 (`NaN` / `Inf` injection):** `|v| > TV_FINITE_MAX` (`1e9`) → `non_finite`.
   - **Attack 7 (self-validation collusion):** `val_device == worker_device` (`100`) → `collusion`.
3. **Coordinate-wise trimmed mean (`trim_k = 2`):** Given 6 honest workers
   reporting `1.0` and 2 Byzantine poisoners reporting `-999.0` and `+999.0`,
   sorting gives `[-999, 1, 1, 1, 1, 1, 1, 999]`. Dropping 2 values from each
   tail leaves `[1, 1, 1, 1]`, whose mean is exact `1.0`.
4. **Tier 5 survival bonus:** Requiring `K = 3` un-rolled-back canonical
   promotions returns `false` after `2` promotions and `true` after `3` (and
   always `false` if `any_regression == true`).

## The verification ladder and Byzantine aggregator

`tv_verify_claim` evaluates Tiers 0, 1, 3, and 4 in strict fail-closed order:

<!-- source: psionic/lib/peer/train-verify.coax verify -->
```jai
// Evaluate Tiers 0, 1, 3 (norm + cosine vs reference), and 4 (quarantine canary) on a delta vector.
tv_verify_claim :: (c: TvClaim, delta: [] f64, cohort_ref: [] f64) -> TvStatus #effects(all) {
  // Tier 0: signature, lease, cryptographic binding, distinct worker/validator, finite tensor
  if !c.sig_ok { return TvStatus.bad_signature; }
  if c.now_s > c.lease_expires_s { return TvStatus.lease_expired; }
  if c.parent_sha != c.expect_parent_sha { return TvStatus.stale_checkpoint; }
  if c.shard_sha != c.expect_shard_sha || c.config_sha != c.expect_config_sha { return TvStatus.binding_mismatch; }
  if c.worker_key == c.val_key || c.worker_device == c.val_device { return TvStatus.collusion; }
  if delta.count < 1 || delta.count != cohort_ref.count { return TvStatus.binding_mismatch; }

  l2_sq := 0.0;
  dot := 0.0;
  for i: 0..<delta.count {
    v := delta[i];
    if !tv_is_finite(v) { return TvStatus.non_finite; }
    l2_sq += v * v;
    dot += v * cohort_ref[i];
  }

  // Tier 1 & Tier 3: reject zero-delta freeriders, norm explosions, spot-check divergence, sign flips
  if l2_sq < c.min_l2_sq { return TvStatus.zero_delta; }
  if l2_sq > c.max_l2_sq { return TvStatus.norm_exceeded; }
  if c.spot_dist > c.max_spot_dist { return TvStatus.spot_diverged; }
  if dot <= 0.0 { return TvStatus.sign_poison; }

  // Tier 4: quarantine canary (exact replay + typed refusal preservation — catches backdoors)
  if !c.canary_exact_ok || !c.canary_refusal_ok { return TvStatus.canary_failed; }
  return TvStatus.accepted;
}
```

When a cohort combines accepted deltas into an outer DiLoCo step,
`tv_trimmed_mean` strips extreme tails per coordinate, and
`tv_survival_bonus_ready` gates the optional Tier-5 downstream survival bonus:

<!-- source: psionic/lib/peer/train-verify.coax trimmed -->
```jai
// Tier 3 Byzantine-resistant coordinate-wise trimmed mean across n workers (trims trim_k from each tail).
tv_trimmed_mean :: (vals: [] f64, trim_k: int) -> f64 #effects(all) {
  n := vals.count;
  if n < 1 || trim_k < 0 || 2 * trim_k >= n { return 0.0; }
  buf: [32] f64;
  if n > 32 { return 0.0; }
  for i: 0..<n { buf[i] = vals[i]; }
  // Insertion sort
  for i: 1..<n {
    key := buf[i];
    j := i - 1;
    while j >= 0 && buf[j] > key { // mutant: equivalent — equal values need no swap
      buf[j + 1] = buf[j];
      j -= 1;
    }
    buf[j + 1] = key;
  }
  sum := 0.0;
  kept := n - 2 * trim_k;
  for i: trim_k..<n - trim_k { sum += buf[i]; }
  return sum / f64(kept);
}

// Tier 5: downstream survival bonus gate (releases only after surviving K canonical promotions).
tv_survival_bonus_ready :: (survived_promotions: int, required_k: int, any_regression: bool) -> bool #effects(all) {
  return !any_regression && required_k >= 1 && survived_promotions >= required_k;
}
```

## Runnable example and expected transcript

[`examples/train-verify.coax`](../examples/train-verify.coax) verifies the
honest claim, all 7 Byzantine attacks, the 25%-poisoned trimmed mean, and the
Tier-5 survival gate, matching [`fixtures/train-verify.out`](../fixtures/train-verify.out)
byte for byte in debug and release:

```text
honest: accepted
attack1 zero_delta: zero_delta
attack2 norm: norm_exceeded
attack3 sign_flip: sign_poison
attack4 backdoor: canary_failed
attack5 stale: stale_checkpoint
attack6 non_finite: non_finite
attack7 collusion: collusion
trimmed_mean(trim=2): 1.0
survival_bonus(2/3): false (3/3): true
```

## Checks and independent evidence

- **Unit & boundary checks:** `coax/bin/coax test psionic/lib/peer/train-verify.coax`
  pins all 7 attack families, exact boundary inequalities (`now_s == lease_expires_s`,
  `spot_dist == max_spot_dist`, `min_l2_sq == max_l2_sq == 1.0`), orthogonal
  (`dot == 0.0`) rejection, and trimmed-mean sorting edge cases.
- **Mutation sharpness:** `#authority` with `84/85` mutants killed, `0`
  survivors, `1` marked equivalent.
- **End-to-end poisoned DiLoCo round:** In
  [`psionic/tools/diloco-round.coax`](../tools/diloco-round.coax), `4/4` honest
  Pylons pass while `7/7` Byzantine attacks are blocked (`naive logloss 0.924`
  under poisoning vs. `trimmed-mean 0.535` vs. `filtered honest 0.449`), before
  handing the aggregated candidate to [chapter 274](ch274-the-quarantine-optimizer.md).
