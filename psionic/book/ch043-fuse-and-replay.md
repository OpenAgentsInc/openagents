# 43. Fuse and replay

> **Status: unimplemented — design draft.** Nothing here runs yet.

Many small device launches cost more than the arithmetic they do. This chapter
fuses common operation pairs into single kernels and records a whole decode or
prefill step once, then replays it as a device graph.

## What this chapter covers

- Fused operations:
  - normalization with its scale;
  - gate and up projections with their activation;
  - expert routing (top-k selection and weights);
  - a recurrent step with its cache write.
- An overlap check: a fusion is admitted only when its buffers provably do
  not overlap.
- Each fusion keeps its unfused result within a stated tolerance.
- Graph capture: a step is recorded once and replayed with bounded changes to
  token and position values.
- Failed or cancelled steps keep their transaction boundaries, so a replay
  never publishes partial state.

## What it builds

- Planned fused kernels and a graph-recording interface. Chapter 70's
  synchronous CUDA replay for gpt-oss is the first concrete instance.

## Dependencies

- [Chapter 37](ch037-a-coax-compute-subset.md): the device compute subset.
- [Chapter 38](ch038-a-tensor-program-becomes-an-execution-plan.md): the
  execution plan that fusion rewrites.
- [Chapter 70](ch070-replay-the-device-program.md): device program replay.
- [Chapter 234](ch234-exact-replay-as-a-verification-class.md): replay
  verdicts for GPU kernel acceptance.

## Done means

- Every fusion passes the overlap check or is refused, with a negative case
  that shows the refusal.
- Each fused result matches its unfused sequence within the stated tolerance.
- A replayed step gives the same cache, logits and stop behavior as the
  unreplayed path.
- The chapter reports host enqueue time saved per step on a named device.

## Sources

- [Chapter plan](PLAN.md), Part V.
- [What Psionic takes from llama.cpp](../docs/llama-cpp.md): fusion with
  overlap guards and captured device graphs.
- [Making Psionic serving faster](../docs/serving/performance.md).
