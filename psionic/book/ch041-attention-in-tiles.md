# 41. Attention in tiles

> **Status: unimplemented — design draft.** Nothing here runs yet.

Whole-prompt attention that builds the full score matrix uses memory that
grows with the square of the prompt length. This chapter computes attention
over tiles of keys and values with an online softmax, so the full matrix never
exists. The changed order of additions gets its own named numerical profile.

## What this chapter covers

- Online softmax: keep a running maximum and a running denominator per query
  row, and rescale the partial output when the maximum changes.
- Key and value tiles sized to the device's fast memory.
- Chunked prefill: process the prompt in query chunks against the growing
  cache.
- Grouped-query attention that loads each key/value head once for all query
  heads that share it.
- Masks and extras that must still hold:
  - chapter 12's block-bidirectional decision mask;
  - gpt-oss attention sinks, which join the denominator without a value;
  - sliding windows, such as gpt-oss's 128-token band.
- Decode variants, including split-K over a long cache.

## What it builds

- Planned tiled attention kernels in chapter 37's compute subset, with a CPU
  oracle that uses the same tile order.

## Dependencies

- [Chapter 10](ch010-attention-without-training.md): the dense reference.
- [Chapter 12](ch012-decisions-have-a-different-mask.md): the decision mask.
- [Chapter 37](ch037-a-coax-compute-subset.md): the device compute subset.
- [Chapter 62](ch062-attention-with-sinks-and-a-window.md): sinks and windows.

## Done means

- The tiled result matches the dense CPU reference within the tolerance the
  new profile states, frozen before the comparison.
- The decision mask, sinks and windows pass their own fixtures in tiled form.
- Peak attention memory no longer grows with the square of the prompt length,
  and you report it at several lengths.
- Each key/value head is read once per query-head group, which a counted load
  check shows.
- [Chapter 233](ch233-from-hard-max-to-softmax.md) can run its softmax
  exactness measurements under this profile.

## Sources

- [Chapter plan](PLAN.md), Part V.
- [What Psionic takes from llama.cpp](../docs/llama-cpp.md): the grouped-query
  and split-K attention designs.
