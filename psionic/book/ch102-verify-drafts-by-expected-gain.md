# 102. Verify drafts by expected gain

> **Status: unimplemented — design draft.** Nothing here runs yet.

DeepSeek V4.1 ships its own draft layers. A draft only helps if the tokens it
lets you commit per unit time go up. The pre-reset work found the opposite
for single requests: the draft layers made decode slower (1.17 versus 3.95
tokens/s). This chapter verifies drafts and decides their length from a
measured cost curve.

## What this chapter covers

- The checkpoint's own draft layers as a speculator under
  [chapter 75](ch075-let-the-small-model-guess.md)'s acceptance rule.
- Fixed-length verification first: a fixed draft length, verified in one
  target pass, with rollback of only the speculative state.
- Variable-length verification second: a trained confidence head predicts
  whether each draft token will be accepted, and a measured cost curve gives
  the time of verifying k tokens. The engine picks the length with the
  highest expected committed tokens per unit time.
- DeepSeek's DSpark reports +6% at its batch sizes. That is an upstream
  claim, not a target for a single request.
- The late speculative-length path of chapter 99, so a step can start before
  the previous accepted length is known.

## Dependencies

- [Chapter 75](ch075-let-the-small-model-guess.md): acceptance, correction
  and snapshot replay.
- [Chapter 76](ch076-verify-a-draft-block-in-parallel.md): batched device
  verification and rollback.
- [Chapter 99](ch099-a-decode-step-with-no-host-waits.md) and
  [chapter 100](ch100-execute-deepseek-v41-flash-on-one-machine.md).
- [Chapter 103](ch103-learning-with-arrays.md): the trainer for the small
  confidence head.

## Done means

- Greedy output with drafting equals greedy output without it, token for
  token.
- Exact acceptance probabilities hold on tiny fixtures.
- The report gives committed tokens per second, not projected acceptance,
  for target-only decode, fixed-length drafts and variable-length drafts on
  the same host and workload.
- Drafting is enabled by default only if it shows a net gain over
  target-only decode. If it does not, the chapter reports that.

## Sources

- [Chapter plan](PLAN.md)
- [DeepSeek target notes](../docs/targets/deepseek.md)
