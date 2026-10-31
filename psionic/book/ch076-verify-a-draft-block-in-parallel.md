# 76. Verify a draft block in parallel

> **Status: unimplemented — design draft.** Nothing here runs yet.

Chapter 75 verifies draft tokens one at a time on the CPU, and the plan
records that this serial path is slower than the target alone. Speculation
pays only when the target checks a whole draft block in one batched pass. This
chapter does that on the device.

## What this chapter covers

- Batched target verification of a draft block on the GPU.
- Rollback of speculative state only, keeping committed state intact.
- Preservation of overwritten sliding-window slots, so a rejected draft does
  not lose history the window still needs.
- Greedy parity and the tiny sampled-law checks from chapter 75.
- Measured acceptance rate and draft length on real prompts.
- Speculation is enabled by default only after it shows a net latency gain
  over the qualified target-only device path.

## Dependencies

- [Chapter 75](ch075-let-the-small-model-guess.md): acceptance rule,
  snapshots and replay.
- [Chapter 66](ch066-a-complete-cuda-decoder.md) and
  [chapter 67](ch067-prefill-together.md): the device decoder and batched
  prefill.
- [Chapter 71](ch071-throughput-is-a-gate.md): the target-only baseline.

## Done means

- Greedy output with speculation equals greedy output without it, token for
  token.
- Sampled-law checks pass on tiny fixtures.
- After a rejection, the cache equals the cache of a run that never drafted.
- The chapter reports acceptance, draft length and end-to-end latency against
  the target-only path. No speedup is claimed without that measurement.

## Sources

- [Chapter plan](PLAN.md), GPT-OSS device delivery order, step 10.
- [Making Psionic serving faster](../docs/serving/performance.md): device
  speculative verification (P2).
