# 197. DeepSeek V4.1 Flash across stages

> **Status: unimplemented — design draft.** Nothing here runs yet.

DeepSeek V4.1 Flash has 510.3 GB of original files. This chapter splits it into contiguous layer stages on several devices or machines.

Every split must match [chapter 100](ch100-execute-deepseek-v41-flash-on-one-machine.md)'s one-machine result on identical tokens. The old Rust stage ran four stages across machines; its numbers are history, not results of this chapter.

## What this chapter covers

- **What crosses each boundary.** The four residual streams from [chapter 93](ch093-many-residual-streams.md), the previous mix, source KV and indexer state from [chapter 92](ch092-compressed-and-shared-attention.md), chosen candidate blocks and Engram history from [chapter 94](ch094-memory-by-lookup.md).
- **BF16 on the wire.** The boundary carries BF16 rather than F32.
- **Fencing and recovery.** [chapter 190](ch190-stage-lifecycle-and-fencing.md)'s generation fencing and stage recovery apply.
- **Parity.** Every split must match [chapter 100](ch100-execute-deepseek-v41-flash-on-one-machine.md)'s one-machine result on identical tokens, under [chapter 17](ch017-qualify-honestly.md)'s rules.
- **Per-hop latency.** Each hop's latency is recorded.
- **History.** The pre-reset Rust stage ran four stages across machines at 29.7 tokens/s, with links taking 0.83–0.87 ms per token. Treat these as prior records, not targets.

## Dependencies

- [Chapter 100](ch100-execute-deepseek-v41-flash-on-one-machine.md): V4.1 Flash on one machine, the reference.
- [Chapter 92](ch092-compressed-and-shared-attention.md): compressed and shared attention state.
- [Chapter 93](ch093-many-residual-streams.md): hyper-connection residual streams.
- [Chapter 94](ch094-memory-by-lookup.md): Engram lookup.
- [Chapter 186](ch186-split-a-model-into-stages.md): the stage boundary contract.
- [Chapter 190](ch190-stage-lifecycle-and-fencing.md): fencing and recovery.
- The V4.1 checkpoint's redistribution licence is an open owner question before stage packages are distributed.

## Done means

- Each tested split matches [chapter 100](ch100-execute-deepseek-v41-flash-on-one-machine.md)'s result on identical tokens under [chapter 17](ch017-qualify-honestly.md)'s frozen distribution checks.
- A boundary message missing any listed part is refused.
- Per-hop latency, bytes per token and tokens per second are recorded for each split.
- Stage loss and recovery tests pass.

## Sources

- [Chapter plan](PLAN.md)
- [Inference across a mesh](../docs/serving/mesh.md)
- [DeepSeek target](../docs/targets/deepseek.md)
