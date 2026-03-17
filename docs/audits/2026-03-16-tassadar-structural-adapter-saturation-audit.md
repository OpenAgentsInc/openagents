# Tassadar Structural Adapter Saturation Audit

Date: March 16, 2026

## Scope

This note records the three follow-on bounded learned-attention runs that came
after the initial `token 6 = <pc>` blocker diagnosis:

- `sudoku_v0_attention_boundary_v7`
- `sudoku_v0_attention_boundary_v8`
- `sudoku_v0_attention_boundary_v9`

and their paired same-corpus comparisons:

- `sudoku_v0_architecture_comparison_v9`
- `sudoku_v0_architecture_comparison_v10`
- `sudoku_v0_architecture_comparison_v11`

The goal of these runs was narrow and explicit: clear the current learned 4x4
blocker where the executor-attention candidate predicts `<byte_00>` at target
token `6` instead of the required structural field token `<pc>`.

## What Changed

`boundary_v7` added a bounded trace-schema-conditioned relative-target logit
bias. The model now had three bounded early-trace adapter surfaces:

- hidden-state-conditioned relative-target output projection
- previous-token-conditioned relative-target transition bias
- trace-schema-conditioned relative-target bias

`boundary_v8` kept those surfaces and turned on the existing per-position
relative-target output-bias adapter.

`boundary_v9` kept the same surfaces but added a high-gain learning-rate scale
for the per-position output-bias adapter (`64x`).

## Artifact Paths

Runs:

- `crates/psionic/fixtures/tassadar/runs/sudoku_v0_attention_boundary_v7`
- `crates/psionic/fixtures/tassadar/runs/sudoku_v0_attention_boundary_v8`
- `crates/psionic/fixtures/tassadar/runs/sudoku_v0_attention_boundary_v9`

Comparisons:

- `crates/psionic/fixtures/tassadar/runs/sudoku_v0_architecture_comparison_v9`
- `crates/psionic/fixtures/tassadar/runs/sudoku_v0_architecture_comparison_v10`
- `crates/psionic/fixtures/tassadar/runs/sudoku_v0_architecture_comparison_v11`

## Result

All three follow-on runs produced the same validation signature.

For every checkpoint in `boundary_v8` and `boundary_v9`, and for the selected
bundle reports in `boundary_v7`, `boundary_v8`, and `boundary_v9`:

- `first_target_exactness_bps = 10000`
- `first_8_token_exactness_bps = 8750`
- `first_32_token_exactness_bps = 7188`
- `exact_trace_case_count = 0`
- first divergence index = `6`
- reference divergence token = `<pc>`
- predicted divergence token = `<byte_00>`

This was not “best checkpoint happened to look the same.”

For `boundary_v9`, every epoch-level validation report has the same divergence
signature and the same bounded exactness metrics. There is exactly one distinct
validation signature across all `32` checkpoints.

## Interpretation

This is the first clean saturation result for the current bounded adapter
family.

What the evidence now says:

- the learned attention lane can hold the first six target tokens
- it cannot make the structural byte-to-field transition into `<pc>`
- adding a trace-schema adapter does not move that boundary
- turning on per-position bias does not move that boundary
- turning the per-position bias up aggressively still does not move that
  boundary

So the current failure is no longer best understood as:

- missing logging
- missing curriculum
- missing transition conditioning
- missing per-position bias
- insufficient gain on the existing bounded logit adapters

It is better understood as:

> the current bounded adapter stack is saturated against this blocker

## Honest Next Step

The next honest unblocker is not another bounded logit-bias variant.

The next move needs a different model surface, for example:

- trainable hidden-state dynamics beyond the current frozen attention core
- a stronger executor-attention family with a learned state-transition path
- a different bounded architecture candidate that can represent field-boundary
  state internally instead of only biasing logits afterward

Until then, Phase 14 remains blocked.
