# 2026-03-16 Tassadar Joint Adapter Audit

This note records the bounded follow-on that fine-tunes the executor-attention
lane from `boundary_v5` with both of the currently retained learned adapter
surfaces turned on together:

- the previous-token-conditioned transition adapter
- the hidden-state-conditioned projection adapter

It is a preserved negative-result note, not a promotion note.

## New Preserved Artifacts

- boundary-training run:
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_attention_boundary_v6`
- same-corpus comparison:
  `crates/psionic/fixtures/tassadar/runs/sudoku_v0_architecture_comparison_v8`

## What Was Tried

The run initializes from `boundary_v5`, lowers the base learning rate to
`0.01`, and trains both retained learned adapter surfaces together:

- `train_relative_target_output_projection = true`
- `train_relative_target_transition_output_bias = true`

The intent was narrow:

> preserve the token-1 fix from `boundary_v5`, then see whether joint
> transition-plus-hidden-state conditioning can clear the later token-6
> `<pc>` blocker.

## Result

It did not beat the `boundary_v5` ceiling.

From `training_report.json` under
`crates/psionic/fixtures/tassadar/runs/sudoku_v0_attention_boundary_v6`:

- selected checkpoint:
  `tassadar-executor-attention-sudoku-v0-boundary-v6.checkpoint.epoch_0007`
- first-target exactness: `10000`
- first-8 exactness: `8750`
- first-32 exactness: `7188`
- aggregate target exactness: `7188`
- exact bounded traces: `0`

From `architecture_comparison_report.json` under
`crates/psionic/fixtures/tassadar/runs/sudoku_v0_architecture_comparison_v8`:

- lookup baseline remains:
  - `10000` first-target
  - `6250` first-8
  - `6563` first-32
- executor-attention candidate remains:
  - `10000` first-target
  - `8750` first-8
  - `7188` first-32

The divergence signature also stayed the same:

- first divergence index: `6`
- reference divergence token: `<pc>`
- predicted divergence token: `<byte_00>`

## Interpretation

This follow-on is still useful, because it rules out another lazy explanation:

> "maybe the token-6 blocker only persists because the transition adapter and
> projection adapter were never trained jointly."

The artifact-backed answer is now:

> joint fine-tuning preserves the `boundary_v5` gain but does not move the
> blocker beyond token `6`.

So the current retained learned adapter surfaces are not enough by themselves
to clear the next structural field.

## Current Honest State

The best bounded learned attention-family result is still effectively the
`boundary_v5` / `architecture_comparison_v7` ceiling, now reproduced by
`boundary_v6` / `architecture_comparison_v8`:

- `10000` bps first-target exactness
- `8750` bps first-8 exactness
- `7188` bps first-32 exactness
- `0/2` exact validation traces
- first divergence at token `6`
- wrong token at that position is `<byte_00>` instead of `<pc>`

That is still better than the lookup baseline and still not good enough to
close Phase 14.
