# 2026-03-16 Tassadar Attention Boundary-Adapter Audit

This note records the first bounded executor-attention follow-on that improved
the 4x4 prompt-to-trace boundary without collapsing the rest of the bounded
suffix.

It is not a promotion note.

## Scope

The goal here was narrow and explicit:

- preserve the weak attention-family baselines instead of overwriting them
- record the failed boundary-only output-head experiment honestly
- add a bounded step-local adapter surface for the attention family
- rerun the same 4x4 bounded window and compare against the preserved lookup
  baseline on the same corpus

The canonical artifacts are:

- rejected boundary-output-head run:
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_attention_boundary_v1`
- accepted boundary-adapter run:
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_attention_boundary_v2`
- same-corpus comparison against the lookup baseline:
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_architecture_comparison_v4`

## What Changed

The attention-family trainer no longer has to attack the token-0 boundary only
through the shared output head.

The landed follow-on adds a bounded relative-target output-bias adapter in the
attention family:

- model surface:
  `crates/psionic/psionic-models/src/tassadar_executor_attention.rs`
- trainer surface:
  `crates/psionic/psionic-research/src/tassadar_attention_training.rs`
- comparison surface:
  `crates/psionic/psionic-eval/src/tassadar_executor_architecture_comparison.rs`
  and
  `crates/psionic/psionic-research/src/tassadar_architecture_comparison.rs`

The important limit is explicit: this is still a bounded research adapter over
the first decoded target positions, not article-parity execution, not a full
proof-backed lane, and not a 4x4 promotion result.

## Preserved Failure Baseline: Boundary V1

The earlier boundary-first attention run at
`crates/psionic/fixtures/tassadar/runs/sudoku_v0_attention_boundary_v1`
started from the trained attention checkpoint but still trained through the
shared output head.

That run did flip token `0`, but it did so destructively:

- aggregate target exactness: `313` bps
- first-target exactness: `10000`
- first-8 exactness: `1250`
- first-32 exactness: `313`
- exact bounded traces: `0`

That artifact is worth keeping because it answers a real question:

> can the current attention family clear the boundary just by pushing harder on
> the shared output head?

The answer is yes for token `0`, but no for truthful bounded suffix quality.

## Improved Boundary Result: Boundary V2

The new bounded adapter run at
`crates/psionic/fixtures/tassadar/runs/sudoku_v0_attention_boundary_v2`
starts from the preserved trained-attention checkpoint and trains the bounded
relative-target output-bias adapter instead of rewriting the shared output
head.

From `training_report.json` / `family_report.json`:

- selected checkpoint:
  `tassadar-executor-attention-sudoku-v0-boundary-v2.checkpoint.epoch_0031`
- aggregate target exactness: `6875` bps
- first-target exactness: `10000`
- first-8 exactness: `7500`
- first-32 exactness: `6875`
- exact bounded traces: `0`
- first divergence index on both validation cases: `1`

That means the new run preserves the boundary fix and materially improves the
bounded suffix over both earlier attention-family baselines:

- trained attention v1:
  `6563` bps aggregate, `0` first-target, `6250` first-8, `6563` first-32
- boundary v1:
  `313` bps aggregate, `10000` first-target, `1250` first-8, `313` first-32
- boundary v2:
  `6875` bps aggregate, `10000` first-target, `7500` first-8, `6875` first-32

So this is real progress, not just a different failure shape.

## Same-Corpus Comparison Against The Lookup Baseline

From
`crates/psionic/fixtures/tassadar/runs/sudoku_v0_architecture_comparison_v4/architecture_comparison_report.json`:

- `candidate_more_exact = true`
- `candidate_faster_reference_linear = false`
- `candidate_closer_to_article_fidelity = true`

Lookup baseline:

- aggregate target exactness: `6563` bps
- first-target exactness: `10000`
- first-8 exactness: `6250`
- first-32 exactness: `6563`
- exact bounded traces: `0`
- bounded neural decode speed: `32000` target tok/s

Boundary-adapter attention family:

- aggregate target exactness: `6875` bps
- first-target exactness: `10000`
- first-8 exactness: `7500`
- first-32 exactness: `6875`
- exact bounded traces: `0`
- bounded neural decode speed: `1333` target tok/s
- hull posture: `fallback_to_reference_linear`
- article-fidelity note:
  `layered full-prefix causal 2D-head hard-max attention plus a bounded relative-target logit-bias adapter`

That is the first bounded same-corpus result where the attention family beats
the preserved lookup baseline on correctness rather than only on architecture
shape.

## What This Does And Does Not Prove

This follow-on does prove:

- the executor-attention family can now fix the token-0 boundary honestly
  without collapsing the whole bounded suffix
- a bounded step-local adapter is a meaningful additional trainable surface
- the attention family is now ahead of the lookup baseline on the current
  bounded 4x4 window

It does not prove:

- the Phase 14 gate is green
- a fully exact 4x4 validation trace exists
- the learned lane is ready for 9x9 promotion
- the learned lane is ready for Hungarian claims
- article fidelity

The open blocker remains explicit:

- first-target exactness is now `10000`
- first-32 exactness is still only `6875`
- exact validation traces are still `0/2`

So the repo still must not claim a green learned executor lane.

## Recommended Next Step

The next honest move is not to widen claims.

It is to push this improved attention-family lane past bounded prefix accuracy:

- keep `sudoku_v0_attention_training_v1` as the preserved trained-attention
  floor
- keep `sudoku_v0_attention_boundary_v1` as the preserved destructive
  output-head boundary baseline
- keep `sudoku_v0_attention_boundary_v2` as the current best attention-family
  boundary artifact
- carry the same Phase 14 gate forward without weakening it
- only promote the learned lane once one family produces at least one exact 4x4
  validation trace with artifact-backed evidence

The current honest summary is:

> the attention family now has a real boundary-fixing research path and it now
> beats the lookup baseline on the bounded 4x4 comparison window, but it still
> does not clear the learned 4x4 promotion gate.
