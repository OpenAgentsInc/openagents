# 2026-03-16 Tassadar Step-Index Blocker Audit

This note records the bounded executor-attention follow-ons that first
identified the token-1 structural blocker after the accepted boundary-adapter
result.

It is still not a promotion note.

It is now a historical blocker note rather than the latest state. The later
transition-adapter follow-on that moves the first divergence deeper into the
trace is recorded separately in
`docs/audits/2026-03-16-tassadar-transition-adapter-audit.md`.

## Why This Note Exists

After `boundary_v2`, the open question was no longer "can the attention family
cross token 0?"

That answer was already yes.

The open question was:

> why does the learned attention lane still fail immediately after token 0, and
> what exactly is it outputting instead of the reference token?

This audit answers that question with artifact-backed evidence from three new
preserved roots:

- first hidden-state-conditioned adapter attempt:
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_attention_boundary_v3`
- stronger-step-size follow-on over the same adapter family:
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_attention_boundary_v4`
- same-corpus comparison with explicit divergence tokens:
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_architecture_comparison_v6`

## What Changed

The executor-attention family now has two additional bounded early-target
surfaces beyond the earlier relative-target bias adapter:

- a hidden-state-conditioned relative-target output projection adapter
- an explicit learning-rate scale for that adapter during bounded boundary
  training

These land in:

- model surface:
  `crates/psionic/psionic-models/src/tassadar_executor_attention.rs`
- trainer surface:
  `crates/psionic/psionic-research/src/tassadar_attention_training.rs`
- comparison/report surface:
  `crates/psionic/psionic-eval/src/tassadar_executor_architecture_comparison.rs`
  and
  `crates/psionic/psionic-research/src/tassadar_architecture_comparison.rs`

The claim boundary remains the same:

- bounded research lane only
- still `research_windowed_decode_only`
- still not a green Phase 14 learned-lane result

## Boundary V3: Hidden-State Adapter, Same Metrics

The first projection-adapter run is preserved at
`crates/psionic/fixtures/tassadar/runs/sudoku_v0_attention_boundary_v3`.

It initializes from `boundary_v2`, freezes the shared output head and the
relative-target bias, and trains only the hidden-state-conditioned
relative-target output projection adapter.

From `training_report.json`:

- selected checkpoint:
  `tassadar-executor-attention-sudoku-v0-boundary-v3.checkpoint.epoch_0031`
- first-target exactness: `10000`
- first-8 exactness: `7500`
- first-32 exactness: `6875`
- exact bounded traces: `0`

The training loss moved, and the adapter tensor did learn non-zero signal, but
the validation gate stayed flat.

This was the first clue that the open blocker was no longer "not enough train
surface" by itself.

## Boundary V4: Stronger Adapter Step Size, Still Same Gate

The stronger-step-size follow-on is preserved at
`crates/psionic/fixtures/tassadar/runs/sudoku_v0_attention_boundary_v4`.

This run keeps the same projection adapter but increases only the adapter-local
optimization scale.

From `training_report.json`:

- selected checkpoint:
  `tassadar-executor-attention-sudoku-v0-boundary-v4.checkpoint.epoch_0031`
- first-target exactness: `10000`
- first-8 exactness: `7500`
- first-32 exactness: `6875`
- exact bounded traces: `0`

The training loss improved materially faster than in `boundary_v3`, which means
the stronger adapter step size is not a no-op.

But the exactness metrics still remained flat across the whole run.

That ruled out the simplest "the projection adapter is just too undertrained"
story as the whole explanation.

## What The Model Is Actually Doing Wrong

The updated comparison artifact at
`crates/psionic/fixtures/tassadar/runs/sudoku_v0_architecture_comparison_v6`
now includes explicit divergence tokens.

For both validation cases, the first divergence is now fully specified:

- first divergence index: `1`
- reference divergence token: `<step_index>`
- predicted divergence token: `<byte_00>`

That is the key new fact.

The learned attention lane is not failing in an abstract "early boundary"
sense anymore.

It is doing something more specific:

> after correctly emitting `<step>`, it skips the structural field token and
> jumps straight to the first payload byte.

This explains why token-0 exactness is green while first-32 exactness remains
stuck.

The lane is collapsing a structured trace transition:

- reference prefix:
  `<step> <step_index> <byte_00> ...`
- predicted prefix:
  `<step> <byte_00> ...`

## Why This Matters

This is a much better blocker description than "token 1 is still wrong."

The repo now has evidence that the open learned-lane problem is specifically
field-token insertion / structural-transition modeling, not merely prompt
conditioning.

That means the next honest attempts should target that failure mode directly.

Examples of honest next moves:

- bounded transition-conditioned early-token adapters
- stronger protocol-aware structural surfaces in the attention family
- bounded research decode constraints that are documented explicitly as such

What should not happen next:

- pretending `boundary_v4` is a promotion result because the optimizer moved
- weakening the Phase 14 gate
- promoting 9x9 or Hungarian based on these runs
- describing this as article parity

## Current Honest State

The attention family is still ahead of the preserved lookup baseline on bounded
4x4 correctness and is still closer to the article's intended architecture
shape.

But the learned gate is still red.

The current best learned attention-family evidence remains:

- `10000` bps first-target exactness
- `7500` bps first-8 exactness
- `6875` bps first-32 exactness
- `0/2` exact validation traces
- first divergence at token `1`
- wrong token at that position is explicitly `<byte_00>` instead of
  `<step_index>`

That is real progress in truthfulness and diagnosis.

It is not a solved learned executor lane.
