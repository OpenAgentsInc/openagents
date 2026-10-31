# 33. Measure the work and plan the memory

> **Status: unimplemented — design draft.** Nothing here runs yet.

Before you optimize anything, you need a fixed workload, a fixed reference and
a number that says where the time and memory go. This chapter builds that
baseline for the CPU and for each model's residency. Every later throughput
chapter in Part V names the bottleneck it targets against these measurements.

## What this chapter covers

- Fixed CPU numerical references for each model path, so an optimization can
  only change speed, never results under the named profile.
- A reproducible workload: pinned prompts, pinned lengths, warm-up runs and a
  recorded machine state.
- Peak memory and latency samples per phase, reusing chapter 18's monotonic
  phase timer.
- Measured host compute throughput and memory bandwidth, and the arithmetic
  intensity of prefill and decode. You place each phase on the host roofline
  to see whether it is bound by compute or by memory.
- A residency plan per model:
  - compute directly from mapped BF16 rows, without expanding them to `f32`;
  - separate persistent bytes (weights), state bytes (KV cache, recurrent
    state) and scratch bytes (per-step workspace);
  - refuse a request whose plan does not fit, before any allocation.
- Upstream sizing claims, such as gpt-oss-20b running "within 16GB", stay
  upstream claims until this plan measures them on the machine that runs.

## What it builds

- A planned residency planner beside the existing `ps_plan` storage plan from
  chapter 15, and a roofline report for each named host.

## Dependencies

- [Chapter 4](ch004-numbers-and-matrices.md): the CPU numerical references.
- [Chapter 7](ch007-reading-original-weights.md): authenticated original files
  and mapped BF16 windows.
- [Chapter 14](ch014-execute-elephant.md) and
  [chapter 15](ch015-execute-mammoth.md): the models whose memory you plan.
- [Chapter 18](ch018-time-a-decision.md): phase timing, RSS and percentiles.

## Done means

- The same workload run twice on a quiet host reports the same logical-token
  counts and latency samples within a stated spread.
- Each model has a residency plan whose predicted persistent, state and
  scratch bytes match measured allocation within a stated margin.
- A request whose plan exceeds the budget is refused with a typed reason and
  no partial allocation.
- Each measured phase has a stated arithmetic intensity and a stated position
  against the measured host roofline.

## Sources

- [Chapter plan](PLAN.md), Part V.
- [gpt-oss target](../docs/targets/gpt-oss.md) for the upstream sizing claims.
- [Making Psionic serving faster](../docs/serving/performance.md).
