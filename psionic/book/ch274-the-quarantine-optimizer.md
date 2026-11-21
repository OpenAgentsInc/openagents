# 274. The Quarantine Optimizer and atomic lineage rollback

No public gradient may ever mutate canonical production weights directly. Even
after [chapter 273](ch273-byzantine-verification-for-public-training.md) blocks
all seven Byzantine attack classes, an honest-looking training round can still
overfit, degrade log loss while raising accuracy (the classic LoRA calibration
regression from Part XXIV), break `<exec_refuse>` exactness, or blow through
the inference latency budget.

This chapter closes Part XXVIII with the **three-checkpoint Quarantine
Optimizer** (`canonical -> quarantine -> promoted`) and **atomic lineage
rollback** in one `#authority` file:
[`peer/quarantine-opt.coax`](../lib/peer/quarantine-opt.coax) (`92/92` mutants
killed, `0` survived).

```shell
coax/bin/coax run psionic/examples/quarantine-opt.coax
coax/bin/coax test psionic/lib/peer/quarantine-opt.coax
coax/bin/coax mutate psionic/lib/peer/quarantine-opt.coax --root-only
```

## By hand: four promotion gates and rollback exclusion

Start with `canonical_sha = 1001`, `quarantine_sha = 0`, `generation = 1`, and
baseline metrics `pass = 7500 bps`, `noise_2sigma = 150 bps`, `logloss = 1950`
milli-nats, `conferr = 200 bps`, `max_latency = 1500 us`, `max_mem = 1024 MB`:

1. **Falsifier 1 (decision gate, [ch 240](ch240-how-far-a-metric-moves-by-itself.md)–[241](ch241-gates-that-can-say-unverifiable.md)):**
   Stage `2001` from window `1` with `cand_pass_bps = 7650`. Gain is
   `7650 - 7500 = 150 <= 150` (not strictly above the 2σ noise floor) →
   `decision_gate_failed`; `quarantine_sha` resets to `0` and `canonical_sha`
   remains `1001`.
2. **Falsifier 2 (probability gate, [ch 241](ch241-gates-that-can-say-unverifiable.md)):**
   Stage `2002` from window `2` with `cand_pass_bps = 9000` (+1500 bps accuracy!)
   but `cand_logloss_milli = 2323 > 1950`. Accuracy gain never overrides worse
   log loss → `probability_gate_failed`; `canonical_sha` stays `1001`.
3. **Falsifier 3 (exactness gate, [ch 234](ch234-exact-replay-as-a-verification-class.md)):**
   Stage `2003` from window `3` with `typed_refusal_ok = false` (silently
   guesses instead of emitting `<exec_refuse>`) → `exactness_gate_failed`;
   `canonical_sha` stays `1001`.
4. **Promotion:** Stage `2005` from window `5` with `cand_pass_bps = 7900`
   (+400 > 150), `cand_logloss_milli = 1880 <= 1950`, `exact_replay_ok = true`,
   `typed_refusal_ok = true`, `latency = 1200 <= 1500`, `mem = 512 <= 1024` →
   all four gates pass; `prior_canonical_sha` becomes `1001`, `canonical_sha`
   advances atomically to `2005`, `generation` becomes `2`.
5. **Atomic rollback:** If shadow monitoring ([ch 224](ch224-shadow-a-new-model-then-promote-it.md))
   later flags window `5`, calling `qo_rollback(s, 1001, 5)` restores
   `canonical_sha = 1001`, increments `rollbacks = 1`, and records window `5`
   in `excluded_windows`. Any future attempt to stage a candidate from window
   `5` immediately refuses with `window_excluded`.

## Staging, four-gate promotion, and atomic rollback

`qo_stage_quarantine` never writes `canonical_sha`, and `qo_try_promote` clears
`quarantine_sha` on any failed gate before returning:

<!-- source: psionic/lib/peer/quarantine-opt.coax promote -->
```jai
// Stage a candidate checkpoint in quarantine (NEVER mutates canonical_sha).
qo_stage_quarantine :: (s: [] QoState, candidate_sha: int, window_id: int) -> QoStatus #effects(all) {
  if candidate_sha == 0 || candidate_sha == s[0].canonical_sha { return QoStatus.no_candidate; }
  if qo_window_is_excluded(s, window_id) { return QoStatus.window_excluded; }
  s[0].quarantine_sha = candidate_sha;
  return QoStatus.promoted;
}

// Multi-gate promotion: canonical <- quarantine ONLY when all 4 gates pass.
// Any failing gate discards quarantine and leaves canonical byte-identical.
qo_try_promote :: (s: [] QoState, m: QoMetrics) -> QoStatus #effects(all) {
  if s[0].quarantine_sha == 0 { return QoStatus.no_candidate; }
  // Gate 1 (decision-v1): improvement strictly exceeds 2-sigma noise floor & zero retained regression
  if (m.cand_pass_bps - m.base_pass_bps) <= m.noise_2sigma_bps || m.retained_drop_bps > 0 {
    s[0].quarantine_sha = 0;
    return QoStatus.decision_gate_failed;
  }
  // Gate 2 (probability-v2): log loss and confident-error rate must not regress
  if m.cand_logloss_milli > m.base_logloss_milli || m.cand_conferr_bps > m.base_conferr_bps {
    s[0].quarantine_sha = 0;
    return QoStatus.probability_gate_failed;
  }
  // Gate 3 (Ch 234 Exactness): 100% bit-exact replay + 100% typed <exec_refuse> preservation
  if !m.exact_replay_ok || !m.typed_refusal_ok {
    s[0].quarantine_sha = 0;
    return QoStatus.exactness_gate_failed;
  }
  // Gate 4 (ext-eval-v2 Cost & Latency): within latency and memory budget
  if m.cand_latency_us > m.max_latency_us || m.cand_mem_mb > m.max_mem_mb {
    s[0].quarantine_sha = 0;
    return QoStatus.cost_gate_failed;
  }

  s[0].prior_canonical_sha = s[0].canonical_sha;
  s[0].canonical_sha = s[0].quarantine_sha;
  s[0].quarantine_sha = 0;
  s[0].generation += 1;
  return QoStatus.promoted;
}
```

Rollback requires the exact `prior_canonical_sha` and appends the suspect
window ID to the bounded exclusion list:

<!-- source: psionic/lib/peer/quarantine-opt.coax rollback -->
```jai
// Atomic rollback to prior canonical SHA, excluding suspect window IDs from future rounds.
qo_rollback :: (s: [] QoState, target_sha: int, suspect_window_id: int) -> QoStatus #effects(all) {
  if target_sha == 0 || target_sha != s[0].prior_canonical_sha { return QoStatus.bad_rollback_target; }
  if suspect_window_id != 0 && !qo_window_is_excluded(s, suspect_window_id) {
    if s[0].n_excluded >= QO_EXCLUDED_MAX { return QoStatus.capacity; }
    s[0].excluded_windows[s[0].n_excluded] = suspect_window_id;
    s[0].n_excluded += 1;
  }
  s[0].canonical_sha = target_sha;
  s[0].quarantine_sha = 0;
  s[0].rollbacks += 1;
  return QoStatus.promoted;
}
```

## Runnable example and expected transcript

[`examples/quarantine-opt.coax`](../examples/quarantine-opt.coax) runs all
three regression falsifiers, a passing promotion, an atomic rollback, and a
re-stage refusal, matching [`fixtures/quarantine-opt.out`](../fixtures/quarantine-opt.out)
byte for byte in debug and release:

```text
falsifier1 decision: decision_gate_failed canonical: 1001
falsifier2 probability: probability_gate_failed canonical: 1001
falsifier3 exactness: exactness_gate_failed canonical: 1001
promote good: promoted canonical: 2005 gen: 2
rollback: promoted canonical: 1001 rollbacks: 1
re-stage excluded win 5: window_excluded
```

## Checks and independent evidence

- **Unit & boundary checks:** `coax/bin/coax test psionic/lib/peer/quarantine-opt.coax`
  verifies all 3 regression falsifiers, Gate 4 latency/memory budgets, exact
  boundary equalities, invalid rollback target refusal, duplicate window-ID
  deduplication, and the `QO_EXCLUDED_MAX = 32` capacity cap.
- **Mutation sharpness:** `#authority` with `92/92` mutants killed, `0`
  survivors.
- **End-to-end promotion & shadow rollout:** [`psionic/tools/diloco-round.coax`](../tools/diloco-round.coax)
  advances `canonical_v1 -> canonical_v2` (`pass_bps 6250 -> 8125`, `logloss
  0.693 -> 0.449`, real Ch 234 `wasm-exec-v1` exact replay and `<exec_refuse>`
  verified), and W6.3's `#authority` [`peer/release-promote.coax`](../lib/peer/release-promote.coax)
  (`64/64` mutants killed) distributes signed manifests via shadow-to-promote
  rollouts ([H7 verification record](../../docs/verification/executor-network-effect-h7.md)).
