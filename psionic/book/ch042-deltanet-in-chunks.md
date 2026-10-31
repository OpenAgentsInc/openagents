# 42. DeltaNet in chunks

> **Status: unimplemented — design draft.** Nothing here runs yet.

Chapter 11 runs Gated DeltaNet one token at a time. That is correct, but a
long prompt then needs one sequential step per token. This chapter processes
the prompt in chunks and proves the result equal, within a named profile, to
the token-by-token recurrence.

## What this chapter covers

- The WY form: inside a chunk, the updates form a triangular system that you
  solve by forward substitution.
- State carried in `f32` between chunks.
- Chunk size as a parameter; llama.cpp uses 64, and Psionic measures its own
  choice.
- State snapshots at chunk boundaries. Chapter 12's suffix forks and
  speculative rollback can restart from these snapshots.
- The existing primitive's bounds: up to 16 key heads, 48 value heads,
  dimension 128 and width-four convolution.

## What it builds

- A planned chunked prefill path beside `psionic/lib/delta.coax`, with a CPU
  oracle and a device kernel.

## Dependencies

- [Chapter 11](ch011-a-hybrid-sequence-model.md): the token-by-token
  recurrence that serves as the reference.
- [Chapter 12](ch012-decisions-have-a-different-mask.md): prefix snapshots and
  forks.
- [Chapter 37](ch037-a-coax-compute-subset.md): the device compute subset.

## Done means

- Chunked and token-by-token results agree within the profile's stated
  tolerance for outputs and for the carried state, at several chunk sizes.
- A snapshot taken at a chunk boundary and resumed gives the same result as an
  uninterrupted run.
- Prefill time for Elephant's prompts is measured before and after on the
  named device.

## Sources

- [Chapter plan](PLAN.md), Part V.
- [Hybrid forward primitives](../docs/hybrid-kernels.md).
- [What Psionic takes from llama.cpp](../docs/llama-cpp.md): chunked Gated
  DeltaNet.
