# 100. Execute DeepSeek V4.1 Flash on one machine

> **Status: unimplemented — design draft.** Nothing here runs yet.

This chapter runs the complete DeepSeek V4.1 Flash model on one machine and
qualifies it. The original files are 510.3 GB, so no single GPU holds them.
You run the model on one large GPU or on a large-memory Mac, with routed
experts served from chapter 96's cache and Engram tables read from disk.

## What this chapter covers

- Loading and authenticating every original shard with chapter 7's
  identities, without reading the whole payload into memory.
- The full V4.1 layer stack from the component chapters:
  - routing from [chapter 91](ch091-routers-that-balance-themselves.md);
  - compressed and shared attention from
    [chapter 92](ch092-compressed-and-shared-attention.md);
  - four residual streams from [chapter 93](ch093-many-residual-streams.md);
  - Engram lookups from [chapter 94](ch094-memory-by-lookup.md);
  - FP4 and FP8 expert weights from [chapter 95](ch095-v41s-expert-weights.md).
- Engine techniques: the expert cache (96), layer-major prefill (97), the
  packed cache (98) and the recorded decode step (99).
- A memory plan from chapter 33 that names resident, cached, mapped and
  streamed bytes, and refuses a host whose plan does not fit.
- Qualification under chapter 17 against a reference capture. The old Rust
  attempt never passed numerical parity: a one-ulp difference in the
  hyper-connection mix flipped near-tie expert choices from layer 1 on.

## Dependencies

- Chapters 91–99 for every component.
- [Chapter 17](ch017-qualify-honestly.md): like-mode comparisons, frozen
  distribution checks and router near-tie regret.
- [Chapter 33](ch033-measure-the-work-and-plan-the-memory.md): the residency
  plan.
- A reference capture. The pre-reset capture cost $36.85 on eight rented
  B200s; a new capture or reuse of a retained one is an owner decision.
- The redistribution licence of the V4.1 checkpoint is an open owner
  question. Settle it before you distribute weights or stage packages.

## Done means

- The distribution checks of chapter 17 (top-1 agreement, total variation,
  router margin and near-tie regret) are frozen before the confirmation set
  is read, and the run passes them, or the chapter reports the failure with
  its units and counterexamples.
- Time to first token, decode tokens per second and peak memory are measured
  on the named host, with the build and library identities.
- Every result names the device that ran. A CPU or F64 stand-in does not
  count for a GPU result.
- This one-machine result is the reference for chapter 197's split runs.

## Sources

- [Chapter plan](PLAN.md)
- [DeepSeek target notes](../docs/targets/deepseek.md)
