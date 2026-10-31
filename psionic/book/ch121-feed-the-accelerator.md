# 121. Feed the accelerator

> **Status: unimplemented — design draft.** Nothing here runs yet.

A GPU that waits for data does no work. This chapter builds the data loading
layer: datasets, samplers, batching, workers and prefetching. You measure
how much GPU time loading costs. Framework track.

## What this chapter covers

- Indexed and streaming datasets.
- Samplers and seeded shuffling.
- Batching and collation, including padding with masks.
- Transforms applied per example.
- Worker threads and prefetching.
- Pinned host memory for transfers.
- Backpressure, so a fast loader does not grow memory without limit.
- Resumable iterator state, saved in the checkpoint.

## Dependencies

- [Chapter 120](ch120-data-and-resumable-state.md): splits, provenance and
  data counters.
- [Chapter 6](ch006-one-tensor-several-devices-and-dtypes.md): device
  placement and transfers.
- [Chapter 122](ch122-a-checkpoint-holds-the-whole-experiment.md): the
  checkpoint that stores iterator state.
- Coax language jobs remain planned. Until they exist, workers are host
  threads that call Coax on disjoint ranges.

## Done means

- A reproducible image pipeline and a reproducible text pipeline produce the
  same batches for the same seed, with 1 and several workers.
- A resumed iterator continues with the next batch the uninterrupted run
  would produce.
- Backpressure holds memory under a stated bound.
- GPU time lost to loading is measured on a named host.

## Sources

- [Chapter plan](PLAN.md), section "A framework, not only engines"
