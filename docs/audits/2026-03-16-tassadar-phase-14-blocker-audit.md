# Tassadar Phase 14 Blocker Audit

Date: March 16, 2026

## Scope

This note records the honest state of `#3814` after running the canonical Phase
14 promotion bundle in-tree under
[crates/psionic/fixtures/tassadar/runs/sudoku_v0_promotion_v1](/home/christopherdavid/code/openagents/crates/psionic/fixtures/tassadar/runs/sudoku_v0_promotion_v1).

The Phase 14 acceptance gate is:

- `first_target_exactness_bps = 10000`
- `first_32_token_exactness_bps > 9000`
- `exact_trace_case_count >= 1`

The gate is still red.

## What Landed

The Phase 14 substrate is now real in the repo rather than scratch-only:

- promotion artifacts in `psionic-train`
  - `best_checkpoint_manifest.json`
  - `promotion_gate_report.json`
  - `exact_trace_samples.json`
  - `exactness_curve.json`
  - `failure_samples.json`
- canonical promotion example
  - [tassadar_promotion_training_run.rs](/home/christopherdavid/code/openagents/crates/psionic/psionic-train/examples/tassadar_promotion_training_run.rs)
- machine-readable promotion gate script
  - [check-psionic-tassadar-4x4-promotion-gate.sh](/home/christopherdavid/code/openagents/scripts/release/check-psionic-tassadar-4x4-promotion-gate.sh)
- stage-local learning-rate and prefix-mode control
  - `teacher_forced`
  - `greedy_rollout`
- decode-step training support in the lookup family
  - [tassadar_executor_transformer.rs](/home/christopherdavid/code/openagents/crates/psionic/psionic-models/src/tassadar_executor_transformer.rs)
- live run-progress output from the trainer and promotion wrapper
  - stage start
  - epoch start
  - batch start
  - sequence start
  - sequence completion
  - validation start
  - validation case completion
  - batch completion
  - validation summary
  - best-checkpoint updates
  - benchmark/persist/telemetry/promotion phases

That last item matters operationally. The run is no longer a silent multi-minute
black box, and later reruns now expose which exact sequence or validation case
is currently consuming time.

## Canonical Promotion Run

The canonical run was executed directly in the repo:

- command:
  - `cargo run -p psionic-train --example tassadar_promotion_training_run`
- output root:
  - [sudoku_v0_promotion_v1](/home/christopherdavid/code/openagents/crates/psionic/fixtures/tassadar/runs/sudoku_v0_promotion_v1)
- final bundle digest:
  - `61d72a53c709545feaf777b6bd075c98a34db6680902b7f7ab13047dcb0acec6`
- wall-clock time:
  - about `659111 ms`

The selected checkpoint and gate outcome are:

- selected checkpoint:
  - `tassadar-executor-transformer-sudoku-v0-promotion-v1.checkpoint.epoch_0006`
- selected stage:
  - `prompt_to_first_16_tokens`
- selected global epoch:
  - `6`
- `first_target_exactness_bps = 10000`
- `first_8_token_exactness_bps = 7500`
- `first_32_token_exactness_bps = 6875`
- `aggregate_target_token_exactness_bps = 7641`
- `exact_trace_case_count = 0`

The gate report is:

- [promotion_gate_report.json](/home/christopherdavid/code/openagents/crates/psionic/fixtures/tassadar/runs/sudoku_v0_promotion_v1/promotion_gate_report.json)
- `passed = false`
- failed thresholds:
  - `first_32_token_exactness_bps = 6875`
  - `exact_trace_case_count = 0`

The selector and leaderboard artifacts agree on that winner:

- [best_checkpoint_manifest.json](/home/christopherdavid/code/openagents/crates/psionic/fixtures/tassadar/runs/sudoku_v0_promotion_v1/best_checkpoint_manifest.json)
- [checkpoint_leaderboard.json](/home/christopherdavid/code/openagents/crates/psionic/fixtures/tassadar/runs/sudoku_v0_promotion_v1/checkpoint_leaderboard.json)

## What The Live Progress Logs Taught Us

The progress stream exposed the run shape clearly:

1. Teacher-forced short-prefix stages are where the actual improvement happened.
2. The best checkpoint was reached at `prompt_to_first_16_tokens`.
3. Greedy-rollout refinement did not improve the best checkpoint.
4. Greedy-rollout refinement eventually regressed the boundary:
   - `first_target_exactness_bps` fell from `10000` to `0`
   - `first_8_token_exactness_bps` fell from `7500` to `6250`
   - `first_32_token_exactness_bps` fell from `6875` to `6563`
5. The later `prompt_to_first_32_tokens` rollout stage plateaued at that worse
   point for multiple consecutive epochs.
6. The terminal full-trace epoch did not recover the earlier best checkpoint.

That means the run failed for inspectable reasons, not because it disappeared
into silent training time.

The trainer now goes one step further than the canonical run did on March 16:

- training emits per-batch estimated target-token counts before work starts
- each sequence now emits a start line with prompt/target length and prefix mode
- each sequence emits a completion line with token count and per-sequence mean loss
- validation emits one case-complete line with divergence and exactness facts

That does not improve the model by itself, but it does remove the remaining
multi-minute blind spots from the Phase 14 loop.

## Stable Failure Pattern

The lookup family no longer fails at target token `0`.

That part of the March 16, 2026 reality audit is genuinely improved.

The stable remaining failure is still:

- first divergence index: `1`
- reference token: `<step_index>`
- predicted token: `<step>`

That pattern is visible in:

- [divergence_histogram.json](/home/christopherdavid/code/openagents/crates/psionic/fixtures/tassadar/runs/sudoku_v0_promotion_v1/divergence_histogram.json)
  - validation bucket: `first_divergence_index = 1`, `case_count = 2`
- [failure_samples.json](/home/christopherdavid/code/openagents/crates/psionic/fixtures/tassadar/runs/sudoku_v0_promotion_v1/failure_samples.json)
  - every persisted sample window shows `<step>` predicted where the reference
    wants `<step_index>`
- [boundary_exactness_report.json](/home/christopherdavid/code/openagents/crates/psionic/fixtures/tassadar/runs/sudoku_v0_promotion_v1/boundary_exactness_report.json)

## What Was Tried

The current canonical promotion config already includes:

- explicit boundary curriculum
- best-checkpoint selection by boundary metrics
- trainable surface widening to
  `output_head_embeddings_and_small_learned_mixer`
- stage-local learning-rate reduction
- teacher-forced short-prefix shaping
- greedy-rollout refinement stages
- terminal full-trace supervision

The best result is still the same ceiling seen in the strongest scratch runs:

- `first_target_exactness_bps = 10000`
- `first_32_token_exactness_bps = 6875`
- `exact_trace_case_count = 0`

## Conclusion

As of March 16, 2026, `#3814` is not honestly closable.

What is real:

- the Phase 14 promotion and reporting substrate is real
- the canonical repo bundle exists
- the run is inspectable while it executes
- the gate fails for explicit machine-readable reasons
- the prompt-to-trace token-0 failure is cleared

What is not yet true:

- `first_32_token_exactness_bps > 9000`
- one fully exact validation trace
- a green learned-lane promotion bundle

## Next Step

Schedule churn on the current lookup-family lane is not enough.

The next honest dependency is stronger work on the post-Phase-15 path:

- keep the current lookup-family promotion bundle as the weak baseline
- keep the separate teacher-forced continuation bundle at
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_promotion_v2` as additional
  negative evidence that schedule-only churn does not beat the current ceiling
- use the separate executor-attention family only with same-corpus comparisons;
  the trained follow-on under
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_attention_training_v1` and
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_architecture_comparison_v2`
  is real progress over the seeded Phase 15 candidate, but it still fails at
  target index `0` and still does not clear the gate
- do not widen claims until one family actually clears the Phase 14 gate
- do not promote 9x9 or Hungarian off this result

The repo now has the tooling needed to prove that. It does not yet have the
artifact-backed learned result needed to close Phase 14.
