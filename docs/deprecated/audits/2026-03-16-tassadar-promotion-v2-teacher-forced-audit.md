# 2026-03-16 Tassadar Promotion V2 Teacher-Forced Audit

This note records the first post-Phase-14 schedule-only follow-on run for the
lookup-family learned executor.

The point of this run was narrow:

- keep the same learned family
- keep the same trainable surface
- keep the same Phase 14 gate
- remove greedy-rollout refinement
- extend teacher-forced supervision at 16 and 32 target tokens
- see whether schedule churn alone could beat the committed `promotion_v1`
  ceiling

It did not.

## Scope

The preserved bundle root is:

- `crates/psionic/fixtures/tassadar/runs/sudoku_v0_promotion_v2`

The decisive artifact is:

- `crates/psionic/fixtures/tassadar/runs/sudoku_v0_promotion_v2/promotion_gate_report.json`

The run kept the same lookup-family surface as `promotion_v1`:

- `output_head_embeddings_and_small_learned_mixer`

What changed was only the schedule:

- more teacher-forced epochs at `prompt_to_first_16_tokens`
- a longer teacher-forced `prompt_to_first_32_tokens` stage
- no greedy-rollout refinement stage

## Best Result

The selected checkpoint is:

- `tassadar-executor-transformer-sudoku-v0-promotion-v2.checkpoint.epoch_0008`
- selected stage: `prompt_to_first_16_tokens`

The gate result is still red:

- `first_target_exactness_bps = 10000`
- `first_8_token_exactness_bps = 7500`
- `first_32_token_exactness_bps = 6875`
- `exact_trace_case_count = 0`
- `passed = false`

That is not an improvement over `promotion_v1`.

The top-line comparison is exact:

- `promotion_v1`
  - checkpoint `epoch_0006`
  - `first_32_token_exactness_bps = 6875`
  - `exact_trace_case_count = 0`
- `promotion_v2`
  - checkpoint `epoch_0008`
  - `first_32_token_exactness_bps = 6875`
  - `exact_trace_case_count = 0`

So the extra teacher-forced continuation reproduced the old ceiling, but did
not beat it.

## What The Learning Curve Showed

The checkpoint curve in
`crates/psionic/fixtures/tassadar/runs/sudoku_v0_promotion_v2/exactness_curve.json`
shows:

- `prompt_to_first_token` through `prompt_to_first_4_tokens`
  - no real boundary breakthrough beyond token 0
  - `first_32_token_exactness_bps = 313`
- `prompt_to_first_8_tokens`
  - small improvement only
  - best `first_32_token_exactness_bps = 938`
- `prompt_to_first_16_tokens`
  - the only material jump in the whole run
  - `2500 -> 4375 -> 6875`
- `prompt_to_first_32_tokens`
  - held at `6875` for five epochs
  - then regressed to `6563`
  - also regressed `first_target_exactness_bps` from `10000` to `0`
- `full_trace_supervision`
  - did not recover the earlier best checkpoint

So the teacher-forced continuation did not open a new accuracy regime. It only
made the old regime easier to hold for several epochs before the same late
instability returned.

## Failure Shape

At the selected checkpoint, both validation cases still first diverge at target
index `1`.

That part did not improve:

- `divergence_histogram.json`
  - bucket `first_divergence_index = 1`, `case_count = 2`

What did change is the token chosen at that divergence.

`promotion_v1` best checkpoint failure samples showed:

- reference token: `<step_index>`
- predicted token: `<step>`

`promotion_v2` best checkpoint failure samples now show:

- reference token: `<step_index>`
- predicted token: `<byte_00>`

So the schedule change moved the local decision surface, but it still did not
produce the required exact trace.

That matters because it means the lookup family is not simply “one token away”
from success under the current schedule. The failure token changed, but the
gate result did not.

## Conclusion

This run is useful because it answers one question cleanly:

> Is more teacher-forced schedule churn on the current lookup family enough to
> clear Phase 14?

The answer is:

- no

What is now justified:

- keep `promotion_v1` and `promotion_v2` as preserved negative evidence
- treat schedule-only lookup-family tweaks as exhausted for the current gate
- move the next effort to a real model/architecture change

What is not justified:

- closing `#3814`
- promoting 9x9
- promoting Hungarian
- implying article fidelity

## Recommended Next Step

The next honest move should not be another schedule-only lookup-family run.

The remaining meaningful options are:

- train the separate executor-attention family against the same 4x4 gate
- or move more effort into the bounded compiled/proof-backed executor lane

What should stop here is:

- repeating teacher-forced schedule churn on the current lookup family and
  expecting a different gate outcome
